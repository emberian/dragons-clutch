//! Chain-derived unsigned material for current Direct actions 9 through 13.
//!
//! Every account suffix is derived from one finalized b1/v3 root, its b3
//! replay, and either its authenticated b2 Selection or the unique fresh b2
//! PDA used by the missed-freeze action-10 partition. Browser inputs cannot
//! choose a terminal reason, fee policy, endpoint, refund recipient, liveness
//! row, replay ordinal, or signed transaction field.

use crate::action_material::{
    chain_derived_direct_role_v2, finish_chain_derived_direct_material_v2,
    ActionFreshnessBoundaryV1, CanonicalActionMaterialErrorV1,
    CanonicalActionMaterialV1,
};
use crate::direct_candidate_material::{
    authenticate_snapshot_set, decode_clock, decode_direct_state, require_rent, sha256,
    DecodedDirectSnapshotV1, OperatorSha256V1,
};
use crate::operatord::KeeperActionSelection;
use crate::rpc_index::{IndexedProgramRelease, ObservedRpcAccount, RpcCommitment};
use crate::transaction_builder::{ExactEquation, IntegerUnit, ProtocolTransactionBuilder};
use crate::workflow_graph::ExplicitOperatorReleaseManifest;
use crate::workflow_graph::WorkflowLane;
use clutch_batch::Side;
use clutch_batch_policy_identity::{
    batch_policy_digest, decode_batch_policy,
    revenue_policy_v2::{
        decode_revenue_policy_v2, revenue_policy_record_v2_id,
        revenue_policy_v2_digest,
    },
};
use clutch_client_contract::direct_market::DirectMarketClientPayloadV1;
use clutch_direct_market_runtime::codec_v3::{
    decode_direct_reservation_body_for_transition_v3,
    AuthenticatedDirectRootTransitionV3,
};
use clutch_direct_market_runtime::lifecycle_v2::{
    derive_direct_candidate_bond_refund_shape_v2,
    DirectCandidateBondRefundShapeV2, DirectRootReplayTransitionV2,
};
use clutch_direct_market_runtime::reservation_v1::DirectReservationV1;
use clutch_direct_market_runtime::selection_v1::DirectSelectionV1;
use clutch_direct_market_runtime::{
    build_direct_retirement_transfer_v1, DirectActionReplayV1, DirectHashBackendV1,
    DirectReplayPhaseV1, DirectRetirementSourceV1,
    DirectRetirementTransferV1, DirectRootPhaseV1, DirectTerminalReasonV1,
};
use clutch_general_v2_contract::{
    project_general_position_replay_prestate_v1, Id32, MarketBindingV4,
    MarketRuntimeV3AccountV1, MARKET_BINDING_SEED_DOMAIN_V1,
    MARKET_RUNTIME_SEED_DOMAIN_V1,
};
use clutch_owner_settlement::AuthenticatedPositionV3;
use clutch_liveness::{
    RuntimeCompartmentKindV1, RuntimeCompartmentV1, RuntimeLivenessPolicyV1,
    RUNTIME_LIVENESS_ACCOUNT_BYTES_V1, RUNTIME_LIVENESS_POLICY_BYTES_V1,
};
use clutch_product_series::{
    ContentId, DirectGlobalLivenessPhaseV2, FixedCodec, MarketFamilyV1,
    MarketGenesisProfileV2, MarketInstancePreimageV2, MarketLifecyclePhaseV3,
    SeriesMarketLinkPhaseV3,
};
use clutch_retirement::{
    project_general_position_v3, AdapterPositionMarketBindingV3,
    AdapterPositionPurposeBindingV3, Identity32V1, PositionAccountV3,
    PositionLifecycleV3, PositionPurposeV3, ReplayV3Lifecycle,
};
use clutch_solana_layout::artifact::ArtifactKind;
use clutch_solana_layout::collateral::{verify_collateral_binding, TOKEN_2022_PROGRAM};
use clutch_solana_layout::direct_market_v1::DirectReservationAccountV1;
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV3, ProductDirectGlobalLivenessAccountV2,
    SeriesMarketLinkAccountV3, PRODUCT_DIRECT_GLOBAL_LIVENESS_PDA_PREFIX_V2,
};
use clutch_solana_layout::registry::{
    DirectMarketAction, DIRECT_ACTION_REPLAY_ACCOUNT_BYTES,
    DIRECT_RESERVATION_ACCOUNT_BYTES, DIRECT_SELECTION_ACCOUNT_BYTES,
};
use clutch_solana_layout::revenue::{
    RevenuePolicyRecordV2, TreasuryServiceLedgerV1,
};
use clutch_solana_layout::{ProfileAccount, RealmAccount};
use sha2::{Digest, Sha256};
use solana_address::Address;
use solana_instruction::AccountMeta;
use solana_rent::Rent;
use std::str::FromStr;

const DIRECT_RESERVATION_SEED_V1: &[u8] = b"dc:direct-reservation:v1";
const DIRECT_SELECTION_SEED_V1: &[u8] = b"dc:direct-selection:v1";
const DIRECT_REPLAY_SEED_V1: &[u8] = b"dc:direct-action-replay:v1";
const REALM_SEED: &[u8] = b"realm";
const PROFILE_SEED: &[u8] = b"profile";
const POLICY_SEED: &[u8] = b"policy";
const BATCH_POLICY_SEED: &[u8] = b"dragons-clutch:batch-policy:v1";
const REVENUE_POLICY_SEED: &[u8] = b"dragons-clutch:revenue-policy:v1";
const TREASURY_SERVICE_LEDGER_SEED_V1: &[u8] = b"treasury-service-v1";
const PRICE_GRID_SEED_V1: &[u8] = b"dragons-clutch:grid:v1";
const PRODUCT_ARTIFACT_SEED: &[u8] = b"dc:product-artifact:v1";
const GENERAL_BINDING_DATA_DOMAIN_V4: &[u8] =
    b"dragons-clutch/general-market/binding-data/v4\0";
const GENERAL_RUNTIME_DATA_DOMAIN_V3: &[u8] =
    b"dragons-clutch/general-market/runtime-data/v3\0";

/// One exact b4/Position/Replay triple in Selection order.
#[derive(Clone, Copy, Debug)]
pub struct DirectTerminalEndpointSnapshotV2<'a> {
    pub reservation: &'a ObservedRpcAccount,
    pub position: &'a ObservedRpcAccount,
    pub replay: &'a ObservedRpcAccount,
}

/// Current General V4 account graph common to actions 9 through 12.
#[derive(Clone, Copy, Debug)]
pub struct DirectTerminalMarketSnapshotV2<'a> {
    pub realm: &'a ObservedRpcAccount,
    pub profile: &'a ObservedRpcAccount,
    pub collateral_policy: &'a ObservedRpcAccount,
    pub token_2022_program: &'a ObservedRpcAccount,
    pub market_binding_v4: &'a ObservedRpcAccount,
    pub market_runtime_v3: &'a ObservedRpcAccount,
    pub market_instance_v2: &'a ObservedRpcAccount,
    pub market_genesis_v2: &'a ObservedRpcAccount,
    pub clock: &'a ObservedRpcAccount,
}

/// Exact shared Candidate liveness suffix. Keeper may alias the immutable
/// payer; neither identity is selected by a browser request.
#[derive(Clone, Copy, Debug)]
pub struct DirectTerminalLivenessSnapshotV2<'a> {
    pub policy: &'a ObservedRpcAccount,
    pub candidate: &'a ObservedRpcAccount,
    pub keeper: &'a ObservedRpcAccount,
    pub immutable_payer: &'a ObservedRpcAccount,
}

/// RevenuePolicyV2 and counted General-treasury suffix used only by action 9.
#[derive(Clone, Copy, Debug)]
pub struct DirectFeeSettlementSnapshotV2<'a> {
    pub batch_policy: &'a ObservedRpcAccount,
    pub revenue_record: &'a ObservedRpcAccount,
    pub revenue_policy_preimage: &'a ObservedRpcAccount,
    pub treasury_position: &'a ObservedRpcAccount,
    pub treasury_replay: &'a ObservedRpcAccount,
    pub treasury_service_ledger: &'a ObservedRpcAccount,
}

/// Exact existing-b2 terminal observation. Active endpoint and refund prefixes
/// are derived from hostile-decoded b2 bytes; inactive array members must be
/// absent.
#[derive(Clone, Copy, Debug)]
pub struct DirectExistingTerminalSnapshotV2<'a> {
    pub root: &'a ObservedRpcAccount,
    pub action_replay: &'a ObservedRpcAccount,
    pub selection: &'a ObservedRpcAccount,
    pub market: DirectTerminalMarketSnapshotV2<'a>,
    pub endpoints: [Option<DirectTerminalEndpointSnapshotV2<'a>>; 2],
    pub fee: Option<DirectFeeSettlementSnapshotV2<'a>>,
    pub bond_refund_owners: [Option<&'a ObservedRpcAccount>; 3],
    pub liveness: DirectTerminalLivenessSnapshotV2<'a>,
}

/// Action-10 observation for an Open root which missed the freeze boundary.
/// Product price artifacts are present because b2 is created in the same
/// instruction. No existing Selection bytes or caller-selected reason exist.
#[derive(Clone, Copy, Debug)]
pub struct DirectMissedFreezeSnapshotV2<'a> {
    pub root: &'a ObservedRpcAccount,
    pub action_replay: &'a ObservedRpcAccount,
    pub fresh_selection: &'a ObservedRpcAccount,
    pub selection_payer: &'a ObservedRpcAccount,
    pub system_program: &'a ObservedRpcAccount,
    pub rent_sysvar: &'a ObservedRpcAccount,
    pub clock: &'a ObservedRpcAccount,
    pub compiler_bundle_v7: &'a ObservedRpcAccount,
    pub native_claim_basis: &'a ObservedRpcAccount,
    pub price_measure_policy: &'a ObservedRpcAccount,
    pub market_genesis_v2: &'a ObservedRpcAccount,
    pub price_grid: &'a ObservedRpcAccount,
    pub realm: &'a ObservedRpcAccount,
    pub profile: &'a ObservedRpcAccount,
    pub collateral_policy: &'a ObservedRpcAccount,
    pub token_2022_program: &'a ObservedRpcAccount,
    pub market_binding_v4: &'a ObservedRpcAccount,
    pub market_runtime_v3: &'a ObservedRpcAccount,
    pub market_instance_v2: &'a ObservedRpcAccount,
    pub endpoints: [Option<DirectTerminalEndpointSnapshotV2<'a>>; 2],
    pub liveness: DirectTerminalLivenessSnapshotV2<'a>,
}

/// Exhaustive current terminal branch. The variant is checked against b1/v3
/// phase and Clock; it is not serialized into the Direct payload.
#[derive(Clone, Copy, Debug)]
pub enum DirectTerminalSnapshotV2<'a> {
    Existing(DirectExistingTerminalSnapshotV2<'a>),
    MissedFreeze(DirectMissedFreezeSnapshotV2<'a>),
}

/// Exact integer facts derived before emitting the unsigned draft.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectTerminalEconomicsV2 {
    action: DirectMarketAction,
    reason: DirectTerminalReasonV1,
    endpoint_count: u8,
    bond_refund_count: u8,
    bond_refund_lamports: u64,
    charged_fee_atoms: u64,
    buyer_rebate_atoms: u64,
    seller_rebate_atoms: u64,
    treasury_atoms: u64,
    refunded_headroom_atoms: u64,
    selection_rent_principal_lamports: u64,
    selection_prefund_donation_lamports: u64,
}

impl DirectTerminalEconomicsV2 {
    pub const fn action(self) -> DirectMarketAction { self.action }
    pub const fn reason(self) -> DirectTerminalReasonV1 { self.reason }
    pub const fn endpoint_count(self) -> u8 { self.endpoint_count }
    pub const fn bond_refund_count(self) -> u8 { self.bond_refund_count }
    pub const fn bond_refund_lamports(self) -> u64 { self.bond_refund_lamports }
    pub const fn charged_fee_atoms(self) -> u64 { self.charged_fee_atoms }
    pub const fn buyer_rebate_atoms(self) -> u64 { self.buyer_rebate_atoms }
    pub const fn seller_rebate_atoms(self) -> u64 { self.seller_rebate_atoms }
    pub const fn treasury_atoms(self) -> u64 { self.treasury_atoms }
    pub const fn refunded_headroom_atoms(self) -> u64 {
        self.refunded_headroom_atoms
    }
    pub const fn selection_rent_principal_lamports(self) -> u64 {
        self.selection_rent_principal_lamports
    }
    pub const fn selection_prefund_donation_lamports(self) -> u64 {
        self.selection_prefund_donation_lamports
    }
}

/// Release-bound unsigned material plus independently inspectable exact
/// terminal economics. This type carries no signer or recent blockhash.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectTerminalActionMaterialV2 {
    canonical: CanonicalActionMaterialV1,
    economics: DirectTerminalEconomicsV2,
}

/// Finalized current observations for action 13. Every optional prefix is
/// cardinality-checked against hostile-decoded b1/v3 and b2 state.
#[derive(Clone, Copy, Debug)]
pub struct DirectFamilyRetirementSnapshotV3<'a> {
    pub product_root_v3: &'a ObservedRpcAccount,
    pub series_link_v3: &'a ObservedRpcAccount,
    pub root: &'a ObservedRpcAccount,
    pub action_replay: &'a ObservedRpcAccount,
    pub selection: &'a ObservedRpcAccount,
    pub resolution_v5: &'a ObservedRpcAccount,
    pub clock: &'a ObservedRpcAccount,
    pub neutral_sink: &'a ObservedRpcAccount,
    pub reservations: [Option<&'a ObservedRpcAccount>; 2],
    pub rent_refund_owners: [Option<&'a ObservedRpcAccount>; 5],
    pub product_direct_global_liveness_v2: &'a ObservedRpcAccount,
    pub liveness: DirectTerminalLivenessSnapshotV2<'a>,
}

/// Exact source/refund/surplus facts committed by the action-13 draft.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectFamilyRetirementEconomicsV3 {
    source_count: u8,
    refund_count: u8,
    refundable_principal_lamports: u64,
    surplus_lamports: u64,
    retirement_transfer_id: [u8; 32],
    product_family_terminal_sequence: u32,
}

impl DirectFamilyRetirementEconomicsV3 {
    pub const fn source_count(self) -> u8 { self.source_count }
    pub const fn refund_count(self) -> u8 { self.refund_count }
    pub const fn refundable_principal_lamports(self) -> u64 {
        self.refundable_principal_lamports
    }
    pub const fn surplus_lamports(self) -> u64 { self.surplus_lamports }
    pub const fn retirement_transfer_id(self) -> [u8; 32] {
        self.retirement_transfer_id
    }
    pub const fn product_family_terminal_sequence(self) -> u32 {
        self.product_family_terminal_sequence
    }
}

/// Release-bound action-13 material. Product RootV3 and LinkV3 are derived
/// from the Direct authority and never supplied as browser-selectable facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectFamilyRetirementActionMaterialV3 {
    canonical: CanonicalActionMaterialV1,
    economics: DirectFamilyRetirementEconomicsV3,
}

impl DirectFamilyRetirementActionMaterialV3 {
    pub const fn canonical(&self) -> &CanonicalActionMaterialV1 { &self.canonical }
    pub const fn economics(&self) -> DirectFamilyRetirementEconomicsV3 {
        self.economics
    }
}

impl DirectTerminalActionMaterialV2 {
    pub const fn canonical(&self) -> &CanonicalActionMaterialV1 { &self.canonical }
    pub const fn economics(&self) -> DirectTerminalEconomicsV2 { self.economics }
}

/// Derive the next current action-9..12 transaction solely from finalized
/// b1/v3 descendants and immutable General/Product observations.
#[allow(clippy::too_many_arguments)]
pub fn construct_next_direct_terminal_material_v2(
    release: &IndexedProgramRelease,
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    operator_selection: &KeeperActionSelection,
    freshness: ActionFreshnessBoundaryV1,
    snapshot: DirectTerminalSnapshotV2<'_>,
) -> Result<DirectTerminalActionMaterialV2, CanonicalActionMaterialErrorV1> {
    match snapshot {
        DirectTerminalSnapshotV2::Existing(value) => construct_existing(
            release, manifest, builder, operator_selection, freshness, value,
        ),
        DirectTerminalSnapshotV2::MissedFreeze(value) => construct_missed_freeze(
            release, manifest, builder, operator_selection, freshness, value,
        ),
    }
}

/// Construct action 13 only from the exact current Product/Direct archive.
/// The resulting account order matches the SBF composer: Product RootV3,
/// LinkV3, b1, b3, b2, ResolutionV5, Clock, neutral sink, the b2-ordered b4
/// prefix, sorted/coalesced rent owners, `0xba/v2`, and the Candidate suffix.
#[allow(clippy::too_many_arguments)]
pub fn construct_direct_family_retirement_material_v3(
    release: &IndexedProgramRelease,
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    operator_selection: &KeeperActionSelection,
    freshness: ActionFreshnessBoundaryV1,
    snapshot: DirectFamilyRetirementSnapshotV3<'_>,
) -> Result<DirectFamilyRetirementActionMaterialV3, CanonicalActionMaterialErrorV1> {
    authenticate_family_retirement_observations(release, freshness, snapshot)?;
    let decoded = decode_direct_state(
        release,
        snapshot.root,
        snapshot.action_replay,
        snapshot.selection,
    )?;
    let root = decoded.state.root();
    let replay = decoded.state.replay();
    let sequence = replay.next_action_sequence();
    let _observed_slot = decode_clock(snapshot.clock)?;
    if root.phase() != DirectRootPhaseV1::Terminal
        || root.terminal_reason().is_none()
        || replay.phase() != DirectReplayPhaseV1::Active
        || replay.candidate_liveness_completed_calls() != 7
        || replay.candidate_liveness_pending()
        || replay.family_terminal_receipt_id() != [0; 32]
        || decoded.selection.reservation_count() != root.live_reservations()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let family_terminal_sequence = authenticate_product_family_preterminal_v3(
        release,
        root,
        snapshot.product_root_v3,
        snapshot.series_link_v3,
    )?;
    authenticate_resolution_v5(release, root, snapshot.resolution_v5)?;
    let reservations = authenticate_terminal_reservation_archives(
        release,
        root,
        &decoded.selection,
        snapshot.root,
        snapshot.reservations,
    )?;
    let retirement = derive_family_retirement_transfer(
        root,
        replay,
        &decoded.selection,
        snapshot,
        reservations,
    )?;
    authenticate_family_refund_owners(snapshot.rent_refund_owners, retirement)?;
    let candidate_balance = authenticate_liveness(
        release,
        root,
        replay,
        snapshot.liveness,
    )?;
    authenticate_product_direct_manifest_preterminal_v2(
        release,
        root,
        family_terminal_sequence,
        snapshot.product_direct_global_liveness_v2,
        snapshot.liveness.candidate,
    )?;
    authenticate_terminal_cursor(
        release,
        operator_selection,
        freshness,
        root,
        sequence,
        DirectMarketAction::RetireTerminal,
    )?;
    authenticate_family_retirement_aliases(snapshot, retirement)?;

    let mut accounts = Vec::with_capacity(20);
    let mut roles = Vec::with_capacity(20);
    for (account, label, writable) in [
        (snapshot.product_root_v3, "product-root-v3", true),
        (snapshot.series_link_v3, "series-market-link-v3", false),
        (snapshot.root, "direct-root", true),
        (snapshot.action_replay, "direct-replay", true),
        (snapshot.selection, "direct-selection", true),
        (snapshot.resolution_v5, "direct-resolution-v5", false),
        (snapshot.clock, "clock-sysvar", false),
        (snapshot.neutral_sink, "neutral-sink", true),
    ] {
        push(&mut accounts, &mut roles, account, label, writable, false);
    }
    for reservation in snapshot
        .reservations
        .into_iter()
        .take(usize::from(root.live_reservations()))
    {
        push(
            &mut accounts,
            &mut roles,
            reservation.ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?,
            "direct-reservation",
            true,
            false,
        );
    }
    for refund in snapshot
        .rent_refund_owners
        .into_iter()
        .take(usize::from(retirement.refund_count))
    {
        push(
            &mut accounts,
            &mut roles,
            refund.ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?,
            "rent-refund-owner",
            true,
            false,
        );
    }
    push(
        &mut accounts,
        &mut roles,
        snapshot.product_direct_global_liveness_v2,
        "product-direct-global-liveness-v2",
        true,
        false,
    );
    push_liveness(&mut accounts, &mut roles, snapshot.liveness);

    let source_lamports = retirement_source_lamports(retirement)?;
    let refundable_principal_lamports = retirement_refund_lamports(retirement)?;
    let equations = vec![
        ExactEquation {
            name: "Direct archive lamport conservation".into(),
            unit: IntegerUnit::Lamports,
            left: u128::from(source_lamports),
            right: u128::from(refundable_principal_lamports)
                + u128::from(retirement.surplus_lamports),
        },
        ExactEquation {
            name: "Candidate compartment exact prestate balance".into(),
            unit: IntegerUnit::Lamports,
            left: u128::from(snapshot.liveness.candidate.lamports),
            right: u128::from(candidate_balance),
        },
    ];
    let canonical = finish_chain_derived_direct_material_v2(
        release,
        manifest,
        builder,
        operator_selection,
        freshness,
        DirectMarketAction::RetireTerminal,
        sequence,
        accounts,
        vec![snapshot.liveness.keeper.address],
        roles,
        equations,
        DirectMarketClientPayloadV1::empty(DirectMarketAction::RetireTerminal)
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
    )?;
    let retirement_transfer_id = retirement
        .semantic_id(&OperatorSha256V1)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    Ok(DirectFamilyRetirementActionMaterialV3 {
        canonical,
        economics: DirectFamilyRetirementEconomicsV3 {
            source_count: retirement.source_count,
            refund_count: retirement.refund_count,
            refundable_principal_lamports,
            surplus_lamports: retirement.surplus_lamports,
            retirement_transfer_id,
            product_family_terminal_sequence: family_terminal_sequence,
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn construct_existing(
    release: &IndexedProgramRelease,
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    operator_selection: &KeeperActionSelection,
    freshness: ActionFreshnessBoundaryV1,
    snapshot: DirectExistingTerminalSnapshotV2<'_>,
) -> Result<DirectTerminalActionMaterialV2, CanonicalActionMaterialErrorV1> {
    authenticate_existing_observations(release, freshness, snapshot)?;
    let decoded = decode_direct_state(
        release,
        snapshot.root,
        snapshot.action_replay,
        snapshot.selection,
    )?;
    let observed_slot = decode_clock(snapshot.market.clock)?;
    let (action, reason) = derive_existing_action(&decoded, observed_slot)?;
    let collateral_mint =
        authenticate_market_graph(release, decoded.state.root(), snapshot.market)?;
    let reservations = authenticate_endpoints(
        release,
        &decoded,
        snapshot.endpoints,
        decoded.selection.reservation_count(),
    )?;
    let refunds = derive_direct_candidate_bond_refund_shape_v2(
        &decoded.state,
        decoded.selection,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let selection_rent = decoded.selection.rent();
    let selection_minimum = selection_rent
        .principal_lamports
        .checked_add(selection_rent.donation_floor_lamports)
        .and_then(|value| {
            value.checked_add(refunds.map_or(0, |refunds| refunds.total_lamports))
        })
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if snapshot.selection.lamports < selection_minimum {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    authenticate_refund_accounts(snapshot.bond_refund_owners, refunds)?;
    authenticate_existing_aliases(
        snapshot,
        decoded.selection.reservation_count(),
        refunds.map_or(0, |value| value.refund_count),
    )?;
    let candidate_balance = authenticate_liveness(
        release,
        decoded.state.root(),
        decoded.state.replay(),
        snapshot.liveness,
    )?;
    authenticate_terminal_cursor(
        release,
        operator_selection,
        freshness,
        decoded.state.root(),
        decoded.state.replay().next_action_sequence(),
        action,
    )?;

    let fee = match (action, snapshot.fee) {
        (DirectMarketAction::SettlePair, Some(value)) => Some(authenticate_fee(
            release,
            decoded.state.root(),
            decoded.selection,
            reservations,
            value,
        )?),
        (DirectMarketAction::SettlePair, None) => {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan)
        }
        (_, None) => None,
        (_, Some(_)) => return Err(CanonicalActionMaterialErrorV1::InvalidPlan),
    };

    let mut accounts = Vec::with_capacity(31);
    let mut roles = Vec::with_capacity(31);
    push_existing_fixed(&mut accounts, &mut roles, snapshot)?;
    push_endpoints(&mut accounts, &mut roles, snapshot.endpoints)?;
    if let Some(value) = snapshot.fee {
        push_fee(&mut accounts, &mut roles, value);
    }
    push_refunds(
        &mut accounts,
        &mut roles,
        snapshot.bond_refund_owners,
        refunds,
    )?;
    push_liveness(&mut accounts, &mut roles, snapshot.liveness);
    let equations = terminal_equations(
        snapshot.selection,
        snapshot.liveness.candidate,
        candidate_balance,
        refunds,
        fee,
        collateral_mint,
    )?;
    let sequence = decoded.state.replay().next_action_sequence();
    let canonical = finish_chain_derived_direct_material_v2(
        release,
        manifest,
        builder,
        operator_selection,
        freshness,
        action,
        sequence,
        accounts,
        vec![snapshot.liveness.keeper.address],
        roles,
        equations,
        DirectMarketClientPayloadV1::empty(action)
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
    )?;
    let (charged_fee_atoms, buyer_rebate_atoms, seller_rebate_atoms, treasury_atoms,
        refunded_headroom_atoms) = match fee {
        Some(value) => (
            value.terminal.charged_fee_atoms,
            value.terminal.buyer_rebate_atoms,
            value.terminal.seller_rebate_atoms,
            value.terminal.treasury_atoms,
            value.terminal.refunded_headroom_atoms,
        ),
        None => (0, 0, 0, 0, 0),
    };
    Ok(DirectTerminalActionMaterialV2 {
        canonical,
        economics: DirectTerminalEconomicsV2 {
            action,
            reason,
            endpoint_count: decoded.selection.reservation_count(),
            bond_refund_count: refunds.map_or(0, |value| value.refund_count),
            bond_refund_lamports: refunds.map_or(0, |value| value.total_lamports),
            charged_fee_atoms,
            buyer_rebate_atoms,
            seller_rebate_atoms,
            treasury_atoms,
            refunded_headroom_atoms,
            selection_rent_principal_lamports: 0,
            selection_prefund_donation_lamports: 0,
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn construct_missed_freeze(
    release: &IndexedProgramRelease,
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    operator_selection: &KeeperActionSelection,
    freshness: ActionFreshnessBoundaryV1,
    snapshot: DirectMissedFreezeSnapshotV2<'_>,
) -> Result<DirectTerminalActionMaterialV2, CanonicalActionMaterialErrorV1> {
    authenticate_missed_observations(release, freshness, snapshot)?;
    let state = decode_open_root_replay(
        release,
        snapshot.root,
        snapshot.action_replay,
    )?;
    let root = state.root();
    let sequence = state.replay().next_action_sequence();
    let observed_slot = decode_clock(snapshot.clock)?;
    if root.phase() != DirectRootPhaseV1::Open
        || observed_slot < root.schedule().submission_closes_slot
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    authenticate_fresh_selection(release, snapshot, root)?;
    let rent: Rent = bincode::deserialize(&snapshot.rent_sysvar.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let selection_rent_principal = rent
        .try_minimum_balance(DIRECT_SELECTION_ACCOUNT_BYTES)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    authenticate_market_graph(
        release,
        root,
        DirectTerminalMarketSnapshotV2 {
            realm: snapshot.realm,
            profile: snapshot.profile,
            collateral_policy: snapshot.collateral_policy,
            token_2022_program: snapshot.token_2022_program,
            market_binding_v4: snapshot.market_binding_v4,
            market_runtime_v3: snapshot.market_runtime_v3,
            market_instance_v2: snapshot.market_instance_v2,
            market_genesis_v2: snapshot.market_genesis_v2,
            clock: snapshot.clock,
        },
    )?;
    authenticate_missed_price_graph(release, root, snapshot)?;
    let reservations = authenticate_open_endpoints(
        release,
        root,
        snapshot.root,
        snapshot.endpoints,
    )?;
    if reservations.iter().flatten().count() != usize::from(root.live_reservations()) {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    authenticate_missed_aliases(snapshot, root.live_reservations())?;
    let candidate_balance = authenticate_liveness(
        release,
        root,
        state.replay(),
        snapshot.liveness,
    )?;
    authenticate_terminal_cursor(
        release,
        operator_selection,
        freshness,
        root,
        sequence,
        DirectMarketAction::LapseEmpty,
    )?;

    let mut accounts = Vec::with_capacity(29);
    let mut roles = Vec::with_capacity(29);
    for (account, label, writable, signer) in [
        (snapshot.root, "direct-root", true, false),
        (snapshot.action_replay, "direct-replay", true, false),
        (snapshot.fresh_selection, "fresh-direct-selection", true, false),
        (snapshot.selection_payer, "selection-rent-payer", true, true),
        (snapshot.system_program, "system-program", false, false),
        (snapshot.rent_sysvar, "rent-sysvar", false, false),
        (snapshot.clock, "clock-sysvar", false, false),
        (snapshot.compiler_bundle_v7, "compiler-bundle-v6", false, false),
        (snapshot.native_claim_basis, "native-claim-basis", false, false),
        (snapshot.price_measure_policy, "price-measure-policy", false, false),
        (snapshot.market_genesis_v2, "market-genesis-v2", false, false),
        (snapshot.price_grid, "price-grid", false, false),
        (snapshot.realm, "realm", false, false),
        (snapshot.profile, "collateral-profile", false, false),
        (snapshot.collateral_policy, "collateral-policy", false, false),
        (snapshot.token_2022_program, "token-2022-program", false, false),
        (snapshot.market_binding_v4, "general-market-binding-v4", false, false),
        (snapshot.market_runtime_v3, "general-market-runtime-v3", false, false),
        (snapshot.market_instance_v2, "market-instance-v2", false, false),
    ] {
        push(&mut accounts, &mut roles, account, label, writable, signer);
    }
    push_endpoints(&mut accounts, &mut roles, snapshot.endpoints)?;
    push_liveness(&mut accounts, &mut roles, snapshot.liveness);
    let required_signers = if snapshot.selection_payer.address
        == snapshot.liveness.keeper.address
    {
        vec![snapshot.selection_payer.address]
    } else {
        vec![snapshot.selection_payer.address, snapshot.liveness.keeper.address]
    };
    let canonical = finish_chain_derived_direct_material_v2(
        release,
        manifest,
        builder,
        operator_selection,
        freshness,
        DirectMarketAction::LapseEmpty,
        sequence,
        accounts,
        required_signers,
        roles,
        vec![
            ExactEquation {
                name: "Candidate compartment exact prestate balance".into(),
                unit: IntegerUnit::Lamports,
                left: u128::from(snapshot.liveness.candidate.lamports),
                right: u128::from(candidate_balance),
            },
            ExactEquation {
                name: "fresh Selection rent plus preserved hostile prefund".into(),
                unit: IntegerUnit::Lamports,
                left: u128::from(selection_rent_principal)
                    + u128::from(snapshot.fresh_selection.lamports),
                right: u128::from(selection_rent_principal)
                    + u128::from(snapshot.fresh_selection.lamports),
            },
        ],
        DirectMarketClientPayloadV1::empty(DirectMarketAction::LapseEmpty)
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
    )?;
    Ok(DirectTerminalActionMaterialV2 {
        canonical,
        economics: DirectTerminalEconomicsV2 {
            action: DirectMarketAction::LapseEmpty,
            reason: DirectTerminalReasonV1::MissedFreezeLapse,
            endpoint_count: root.live_reservations(),
            bond_refund_count: 0,
            bond_refund_lamports: 0,
            charged_fee_atoms: 0,
            buyer_rebate_atoms: 0,
            seller_rebate_atoms: 0,
            treasury_atoms: 0,
            refunded_headroom_atoms: 0,
            selection_rent_principal_lamports: selection_rent_principal,
            selection_prefund_donation_lamports: snapshot.fresh_selection.lamports,
        },
    })
}

fn derive_existing_action(
    decoded: &DecodedDirectSnapshotV1,
    slot: u64,
) -> Result<(DirectMarketAction, DirectTerminalReasonV1), CanonicalActionMaterialErrorV1> {
    let root = decoded.state.root();
    let selection = decoded.selection;
    let schedule = root.schedule();
    match (root.phase(), selection.phase()) {
        (DirectRootPhaseV1::FrozenEmpty, clutch_direct_market_runtime::selection_v1::DirectSelectionPhaseV1::FrozenEmpty)
            if slot >= schedule.submission_closes_slot =>
        {
            Ok((DirectMarketAction::LapseEmpty, DirectTerminalReasonV1::EmptyLapse))
        }
        (DirectRootPhaseV1::SubmissionOpen, clutch_direct_market_runtime::selection_v1::DirectSelectionPhaseV1::SubmissionOpen)
            | (DirectRootPhaseV1::Verifying, clutch_direct_market_runtime::selection_v1::DirectSelectionPhaseV1::Verifying)
            if slot >= schedule.selection_deadline_slot =>
        {
            Ok((DirectMarketAction::LapseUnselected, DirectTerminalReasonV1::UnselectedLapse))
        }
        (DirectRootPhaseV1::Selected, clutch_direct_market_runtime::selection_v1::DirectSelectionPhaseV1::Selected)
            if slot < schedule.settlement_deadline_slot =>
        {
            Ok((DirectMarketAction::SettlePair, DirectTerminalReasonV1::Settled))
        }
        (DirectRootPhaseV1::Selected, clutch_direct_market_runtime::selection_v1::DirectSelectionPhaseV1::Selected)
            if slot >= schedule.settlement_deadline_slot =>
        {
            Ok((DirectMarketAction::LapseSelected, DirectTerminalReasonV1::SelectedLapse))
        }
        _ => Err(CanonicalActionMaterialErrorV1::InvalidPlan),
    }
}

fn authenticate_market_graph(
    release: &IndexedProgramRelease,
    root: &AuthenticatedDirectRootTransitionV3,
    market: DirectTerminalMarketSnapshotV2<'_>,
) -> Result<Address, CanonicalActionMaterialErrorV1> {
    for account in [
        market.realm,
        market.profile,
        market.collateral_policy,
        market.market_binding_v4,
        market.market_runtime_v3,
        market.market_instance_v2,
        market.market_genesis_v2,
    ] {
        require_program_account(release, account)?;
    }
    if !market.token_2022_program.executable
        || market.token_2022_program.address.to_bytes() != TOKEN_2022_PROGRAM
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let realm = RealmAccount::decode(&market.realm.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let profile = ProfileAccount::decode(&market.profile.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let collateral = verify_collateral_binding(&market.collateral_policy.data, &profile)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let realm_pda = pda(release.program_id, &[REALM_SEED, &root.realm_id()]);
    let profile_pda = pda(
        release.program_id,
        &[PROFILE_SEED, &root.realm_id(), &root.collateral_profile_id()],
    );
    let policy_pda = pda(
        release.program_id,
        &[POLICY_SEED, &root.collateral_profile_id(), &root.collateral_policy_id()],
    );
    if realm.realm.bytes() != root.realm_id()
        || realm.profile.bytes() != root.collateral_profile_id()
        || realm.stored_bump != realm_pda.1
        || profile.realm.bytes() != root.realm_id()
        || profile.profile.bytes() != root.collateral_profile_id()
        || profile.collateral_policy_id.bytes() != root.collateral_policy_id()
        || profile.adapter_release_id.bytes() != root.collateral_release_id()
        || market.realm.address != realm_pda.0
        || market.profile.address != profile_pda.0
        || market.collateral_policy.address != policy_pda.0
        || collateral.collateral.token_program != TOKEN_2022_PROGRAM
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let binding = MarketBindingV4::decode(&market.market_binding_v4.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let runtime = MarketRuntimeV3AccountV1::decode(&market.market_runtime_v3.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let base = binding.base().base();
    let current = root.current_general();
    let binding_pda = pda(
        release.program_id,
        &[MARKET_BINDING_SEED_DOMAIN_V1, &root.market_instance_id()],
    );
    let runtime_pda = pda(
        release.program_id,
        &[
            MARKET_RUNTIME_SEED_DOMAIN_V1,
            market.market_binding_v4.address.as_ref(),
        ],
    );
    let binding_rent = binding.rent();
    let binding_rent_floor = binding_rent
        .refundable_principal
        .checked_add(binding_rent.donation_floor)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let runtime_rent_floor = runtime
        .rent
        .refundable_principal
        .checked_add(runtime.rent.donation_floor)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if market.market_binding_v4.address.to_bytes() != current.general_market_binding_account
        || market.market_runtime_v3.address.to_bytes() != current.general_market_runtime_account
        || account_data_id(
            GENERAL_BINDING_DATA_DOMAIN_V4,
            market.market_binding_v4,
        ) != current.general_market_binding_v5_data_id
        || account_data_id(
            GENERAL_RUNTIME_DATA_DOMAIN_V3,
            market.market_runtime_v3,
        ) != current.general_market_runtime_data_id
        || base.market_instance_v2_id.bytes() != root.market_instance_id()
        || base.market.bytes() != market.market_runtime_v3.address.to_bytes()
        || base.stored_bump != binding_pda.1
        || base.relation_policy_id.bytes() != root.relation_policy_id()
        || base.price_measure_policy_v1_id.bytes() != root.price_policy_id()
        || base.neutral_sink.bytes() != root.neutral_lamport_sink()
        || base.outcome_count != root.outcome_count()
        || base.price_scale != root.price_scale()
        || binding.base().batch_policy_id().bytes() != root.fee_policy().batch_policy_id
        || base.market_genesis_profile_v2_id.bytes()
            != content_id::<MarketGenesisProfileV2>(market.market_genesis_v2, ArtifactKind::MarketGenesisProfileV2)?
        || runtime.market_binding.bytes() != market.market_binding_v4.address.to_bytes()
        || runtime.market_instance_v2_id.bytes() != root.market_instance_id()
        || runtime.stored_bump != runtime_pda.1
        || market.market_binding_v4.address != binding_pda.0
        || market.market_runtime_v3.address != runtime_pda.0
        || market.market_binding_v4.lamports < binding_rent_floor
        || market.market_runtime_v3.lamports < runtime_rent_floor
        || content_id::<MarketInstancePreimageV2>(market.market_instance_v2, ArtifactKind::MarketInstancePreimageV2)?
            != root.market_instance_id()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let genesis = MarketGenesisProfileV2::decode(&market.market_genesis_v2.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let market_instance = MarketInstancePreimageV2::decode(&market.market_instance_v2.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    collateral
        .check_market_cap(market_instance.collateral_cap)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if genesis.realm_id.bytes() != root.realm_id()
        || genesis.profile_id.bytes() != root.collateral_profile_id()
        || genesis.relation_policy_id.bytes() != root.relation_policy_id()
        || genesis.price_measure_policy_id.content_id().bytes() != root.price_policy_id()
        || genesis.fee_policy_id.bytes() != root.fee_policy().revenue_policy_v2_digest
        || market_instance.market_genesis_profile_id.content_id().bytes()
            != genesis.id().map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
                .content_id().bytes()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    Ok(Address::new_from_array(collateral.collateral.mint))
}

fn authenticate_endpoints(
    release: &IndexedProgramRelease,
    decoded: &DecodedDirectSnapshotV1,
    supplied: [Option<DirectTerminalEndpointSnapshotV2<'_>>; 2],
    count: u8,
) -> Result<[Option<DirectReservationV1>; 2], CanonicalActionMaterialErrorV1> {
    let mut output = [None; 2];
    for index in 0..2usize {
        match (index < usize::from(count), supplied[index]) {
            (true, Some(endpoint)) => {
                let reservation = authenticate_reservation(
                    release,
                    decoded.state.root(),
                    decoded.root_account_address(),
                    endpoint,
                )?;
                if reservation.account()
                    != decoded.selection.reservation_account(index as u8)
                        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
                    || decoded.state.root().child_reservation_semantic_id(
                        reservation,
                        &OperatorSha256V1,
                    ).map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
                        != decoded.selection.reservation_semantic_id(index as u8)
                            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
                {
                    return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
                }
                output[index] = Some(reservation);
            }
            (false, None) => {}
            _ => return Err(CanonicalActionMaterialErrorV1::InvalidPlan),
        }
    }
    Ok(output)
}

trait DecodedRootAddressV2 {
    fn root_account_address(&self) -> Address;
}

impl DecodedRootAddressV2 for DecodedDirectSnapshotV1 {
    fn root_account_address(&self) -> Address {
        Address::new_from_array(self.state.root().direct_root_account())
    }
}

fn authenticate_open_endpoints(
    release: &IndexedProgramRelease,
    root: &AuthenticatedDirectRootTransitionV3,
    root_account: &ObservedRpcAccount,
    supplied: [Option<DirectTerminalEndpointSnapshotV2<'_>>; 2],
) -> Result<[Option<DirectReservationV1>; 2], CanonicalActionMaterialErrorV1> {
    let mut output = [None; 2];
    for index in 0..2usize {
        match (index < usize::from(root.live_reservations()), supplied[index]) {
            (true, Some(endpoint)) => {
                let reservation = authenticate_reservation(
                    release,
                    root,
                    root_account.address,
                    endpoint,
                )?;
                if reservation.account()
                    != root.reservation_account(index as u8)
                        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
                    || root.child_reservation_semantic_id(reservation, &OperatorSha256V1)
                        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
                        != root.reservation_semantic_id(index as u8)
                            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
                {
                    return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
                }
                output[index] = Some(reservation);
            }
            (false, None) => {}
            _ => return Err(CanonicalActionMaterialErrorV1::InvalidPlan),
        }
    }
    Ok(output)
}

fn authenticate_reservation(
    release: &IndexedProgramRelease,
    root: &AuthenticatedDirectRootTransitionV3,
    root_account: Address,
    endpoint: DirectTerminalEndpointSnapshotV2<'_>,
) -> Result<DirectReservationV1, CanonicalActionMaterialErrorV1> {
    require_program_account(release, endpoint.reservation)?;
    require_program_account(release, endpoint.position)?;
    require_program_account(release, endpoint.replay)?;
    let bytes: &[u8; DIRECT_RESERVATION_ACCOUNT_BYTES] = endpoint
        .reservation
        .data
        .as_slice()
        .try_into()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let frame = DirectReservationAccountV1::decode(bytes)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let reservation = decode_direct_reservation_body_for_transition_v3(
        frame.semantic_body(),
        root,
    ).map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let expected = pda(
        release.program_id,
        &[DIRECT_RESERVATION_SEED_V1, root_account.as_ref(), &reservation.order_id()],
    );
    if endpoint.reservation.address != expected.0
        || frame.bump() != expected.1
        || reservation.account() != endpoint.reservation.address.to_bytes()
        || reservation.position_account() != endpoint.position.address.to_bytes()
        || reservation.position_replay_account() != endpoint.replay.address.to_bytes()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    require_rent(reservation.rent(), endpoint.reservation.lamports)?;
    authenticate_position_replay(release, root, reservation, endpoint)?;
    Ok(reservation)
}

fn authenticate_position_replay(
    release: &IndexedProgramRelease,
    root: &AuthenticatedDirectRootTransitionV3,
    reservation: DirectReservationV1,
    endpoint: DirectTerminalEndpointSnapshotV2<'_>,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    authenticate_general_position_replay_projection(
        release,
        root,
        reservation.owner(),
        reservation.position_generation(),
        endpoint.position,
        endpoint.replay,
    )
}

fn authenticate_general_position_replay_projection(
    release: &IndexedProgramRelease,
    root: &AuthenticatedDirectRootTransitionV3,
    owner: [u8; 32],
    generation: u64,
    position_account: &ObservedRpcAccount,
    replay_account: &ObservedRpcAccount,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    let position = PositionAccountV3::decode(&position_account.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let fields = position.fields();
    let purpose = [u8::from(PositionPurposeV3::General)];
    let expected_position = pda(
        release.program_id,
        &[
            clutch_retirement::POSITION_V3_PDA_PREFIX,
            &root.market_instance_id(),
            &owner,
            &purpose,
            &root.general_market_runtime_account(),
        ],
    );
    let expected_replay = pda(
        release.program_id,
        &[
            clutch_retirement::PURPOSE_REPLAY_V3_PDA_PREFIX,
            position_account.address.as_ref(),
            &purpose,
            &root.general_market_runtime_account(),
        ],
    );
    let replay = clutch_retirement::ReplayV3Envelope::decode(
        &replay_account.data,
        &OperatorSha256V1,
    ).map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let header = replay.header();
    let position_rent = position.rent();
    let position_rent_floor = position_rent
        .refundable_live_principal
        .checked_add(position_rent.permanent_tombstone_principal)
        .and_then(|value| value.checked_add(position_rent.donation_floor))
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let replay_rent = header.rent();
    let replay_rent_floor = replay_rent
        .refundable_principal()
        .checked_add(replay_rent.donation_floor())
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if position_account.address != expected_position.0
        || position.stored_bump() != expected_position.1
        || replay_account.address != expected_replay.0
        || header.stored_bump() != expected_replay.1
        || position_account.lamports < position_rent_floor
        || replay_account.lamports < replay_rent_floor
        || fields.lifecycle != PositionLifecycleV3::Open
        || fields.purpose != PositionPurposeV3::General
        || fields.generation != generation
        || fields.market_instance_id.bytes() != root.market_instance_id()
        || fields.realm_id.bytes() != root.realm_id()
        || fields.collateral_policy_id.bytes() != root.collateral_policy_id()
        || fields.collateral_release_id.bytes() != root.collateral_release_id()
        || fields.owner.bytes() != owner
        || fields.controller.bytes() != owner
        || fields.replay_account.bytes() != replay_account.address.to_bytes()
        || fields.purpose_binding_id.bytes() != root.general_market_runtime_account()
        || header.lifecycle() != ReplayV3Lifecycle::Live
        || header.position_account().bytes() != position_account.address.to_bytes()
        || header.replay_account().bytes() != replay_account.address.to_bytes()
        || header.position_generation() != fields.generation
        || header.purpose() != PositionPurposeV3::General
        || header.purpose_binding_id().bytes() != root.general_market_runtime_account()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let owner = Identity32V1::new(owner)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    project_general_position_v3(
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
            owner,
            controller: owner,
            purpose_binding_id: Identity32V1::new(root.general_market_runtime_account())
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
        },
    ).map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let semantic_id = position.semantic_id(&OperatorSha256V1)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
        .bytes();
    let authenticated = AuthenticatedPositionV3 {
        account: position_account.address.to_bytes(),
        general_market_runtime: root.general_market_runtime_account(),
        semantic: position,
        semantic_id,
        account_authenticated: true,
        semantic_id_authenticated: true,
        market_binding_authenticated: true,
        writable: true,
    };
    project_general_position_replay_prestate_v1(
        Id32::new(replay_account.address.to_bytes())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
        expected_replay.1,
        header.next_sequence(),
        &replay_account.data,
        authenticated,
        &OperatorSha256V1,
    ).map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedDirectTerminalFeeV2 {
    terminal: clutch_direct_market_runtime::fee_v1::DirectFeeTerminalV1,
    maximum_fee_atoms: u64,
}

fn authenticate_fee(
    release: &IndexedProgramRelease,
    root: &AuthenticatedDirectRootTransitionV3,
    selection: DirectSelectionV1,
    reservations: [Option<DirectReservationV1>; 2],
    fee: DirectFeeSettlementSnapshotV2<'_>,
) -> Result<AuthenticatedDirectTerminalFeeV2, CanonicalActionMaterialErrorV1> {
    for account in [
        fee.batch_policy,
        fee.revenue_record,
        fee.treasury_position,
        fee.treasury_replay,
        fee.treasury_service_ledger,
    ] {
        require_program_account(release, account)?;
    }
    let batch = decode_batch_policy(&fee.batch_policy.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let revenue = decode_revenue_policy_v2(&fee.revenue_policy_preimage.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let policy = root.fee_policy();
    policy.binds_policies(root.realm_id(), &batch, &revenue)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let batch_id = batch_policy_digest(&batch)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?.0;
    let record = RevenuePolicyRecordV2::decode(&fee.revenue_record.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    record.binds_policy(&revenue)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let general = root.current_general();
    let record_id = revenue_policy_record_v2_id(root.realm_id(), &revenue)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?.0;
    let revenue_id = revenue_policy_v2_digest(&revenue)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?.0;
    let record_pda = pda(release.program_id, &[REVENUE_POLICY_SEED, &root.realm_id()]);
    let record_rent_floor = record
        .terminal_payer_principal
        .checked_add(record.terminal_donation_floor)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if batch_id != policy.batch_policy_id
        || fee.batch_policy.address
            != pda(release.program_id, &[BATCH_POLICY_SEED, &root.direct_epoch_semantics_id(), &batch_id]).0
        || fee.revenue_record.address.to_bytes() != general.revenue_policy_record_account
        || fee.revenue_record.address != record_pda.0
        || record.stored_bump != record_pda.1
        || fee.revenue_record.lamports < record_rent_floor
        || record.realm.bytes() != root.realm_id()
        || record_id != general.revenue_policy_record_v2_id
        || revenue_id != general.revenue_policy_v2_digest
        || fee.treasury_position.address.to_bytes() != general.treasury_position_account
        || fee.treasury_replay.address.to_bytes() != general.treasury_replay_account
        || fee.treasury_service_ledger.address.to_bytes()
            != general.treasury_service_ledger_account
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let ledger = TreasuryServiceLedgerV1::decode(&fee.treasury_service_ledger.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let ledger_pda = pda(
        release.program_id,
        &[
            TREASURY_SERVICE_LEDGER_SEED_V1,
            &root.market_instance_id(),
            fee.treasury_position.address.as_ref(),
        ],
    );
    let ledger_rent_floor = ledger
        .refundable_rent_principal
        .checked_add(ledger.donation_floor)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if ledger.realm.bytes() != root.realm_id()
        || ledger.revenue_policy_record_account.bytes() != fee.revenue_record.address.to_bytes()
        || ledger.revenue_policy_record_v2_id.bytes() != record_id
        || ledger.market_instance_v2_id.bytes() != root.market_instance_id()
        || ledger.treasury_owner.bytes() != general.treasury_owner
        || ledger.treasury_position_account.bytes() != fee.treasury_position.address.to_bytes()
        || ledger.admitted_epoch_count != ledger.settled_epoch_count.saturating_add(1)
        || fee.treasury_service_ledger.address != ledger_pda.0
        || ledger.stored_bump != ledger_pda.1
        || fee.treasury_service_ledger.lamports < ledger_rent_floor
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    authenticate_general_position_replay_projection(
        release,
        root,
        general.treasury_owner,
        ledger.treasury_position_generation,
        fee.treasury_position,
        fee.treasury_replay,
    )?;
    let pair = selection.selected_pair().ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let mut buyer = None;
    let mut seller = None;
    for reservation in reservations.into_iter().flatten() {
        match reservation.side() {
            Side::Buy if buyer.is_none() => buyer = Some(reservation),
            Side::Sell if seller.is_none() => seller = Some(reservation),
            _ => return Err(CanonicalActionMaterialErrorV1::InvalidPlan),
        }
    }
    let buyer = buyer.ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let seller = seller.ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if buyer.order_id() != pair.buy_order_id()
        || seller.order_id() != pair.sell_order_id()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let terminal = policy.assess_terminal_buyer(
        pair.quantity(),
        pair.outcome(),
        pair.outcome_count(),
        pair.price_scale(),
        selection.price(),
        buyer.position_account(),
        seller.position_account(),
        buyer.maximum_fee_atoms(),
        root.realm_id(),
        &batch,
        &revenue,
    ).map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    Ok(AuthenticatedDirectTerminalFeeV2 {
        terminal,
        maximum_fee_atoms: buyer.maximum_fee_atoms(),
    })
}

fn authenticate_liveness(
    release: &IndexedProgramRelease,
    root: &AuthenticatedDirectRootTransitionV3,
    replay: DirectActionReplayV1,
    value: DirectTerminalLivenessSnapshotV2<'_>,
) -> Result<u64, CanonicalActionMaterialErrorV1> {
    require_program_account(release, value.policy)?;
    require_program_account(release, value.candidate)?;
    require_operator(value.keeper)?;
    if value.immutable_payer.executable || value.immutable_payer.address == Address::default() {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let binding = root.candidate_liveness();
    let policy = RuntimeLivenessPolicyV1::decode(&value.policy.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let candidate = RuntimeCompartmentV1::decode(&value.candidate.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    candidate.validate_against_policy(policy)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let expected_balance = candidate.expected_account_balance_lamports()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let expected_completed_calls = binding
        .first_call_ordinal
        .checked_sub(1)
        .and_then(|value| value.checked_add(replay.candidate_liveness_completed_calls()))
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let expected_last_receipt = replay.candidate_liveness_last_receipt_id();
    let initial_data_id_mismatch = replay.candidate_liveness_completed_calls() == 0
        && sha256(&value.candidate.data) != binding.candidate_data_id;
    if value.policy.data.len() != RUNTIME_LIVENESS_POLICY_BYTES_V1
        || value.candidate.data.len() != RUNTIME_LIVENESS_ACCOUNT_BYTES_V1
        || value.policy.address.to_bytes() != binding.policy_account
        || value.candidate.address.to_bytes() != binding.candidate_account
        || sha256(&value.policy.data) != binding.policy_data_id
        || candidate.kind != RuntimeCompartmentKindV1::Candidate
        || candidate.identity.lifecycle_id.bytes() != binding.global_lifecycle_id
        || candidate.identity.owner.bytes() != binding.candidate_semantic_owner
        || candidate.identity.payer.bytes() != value.immutable_payer.address.to_bytes()
        || candidate.identity.neutral_sink.bytes() != root.neutral_lamport_sink()
        || candidate.identity.generation != binding.candidate_generation
        || candidate.quote_schedule_id.bytes() != binding.candidate_quote_schedule_id
        || candidate.receipt_program_id.bytes() != release.program_id.to_bytes()
        || binding.candidate_receipt_program_id != release.program_id.to_bytes()
        || candidate.completed_calls != expected_completed_calls
        || (candidate.completed_calls == 0)
            != (candidate.last_work_receipt_id.bytes() == [0; 32])
        || (replay.candidate_liveness_completed_calls() != 0
            && candidate.last_work_receipt_id.bytes() != expected_last_receipt)
        || initial_data_id_mismatch
        || expected_balance != value.candidate.lamports
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    Ok(expected_balance)
}

fn authenticate_refund_accounts(
    supplied: [Option<&ObservedRpcAccount>; 3],
    expected: Option<DirectCandidateBondRefundShapeV2>,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    let count = expected.map_or(0usize, |value| usize::from(value.refund_count));
    for index in 0..3usize {
        match (index < count, supplied[index]) {
            (true, Some(account)) => {
                let refund = expected
                    .and_then(|value| value.refunds[index])
                    .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
                let order_is_invalid = index != 0
                    && match supplied[index - 1] {
                        Some(prior) => prior.address >= account.address,
                        None => true,
                    };
                if account.address.to_bytes() != refund.recipient
                    || account.executable
                    || order_is_invalid
                {
                    return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
                }
            }
            (false, None) => {}
            _ => return Err(CanonicalActionMaterialErrorV1::InvalidPlan),
        }
    }
    Ok(())
}

fn authenticate_existing_aliases(
    snapshot: DirectExistingTerminalSnapshotV2<'_>,
    endpoint_count: u8,
    refund_count: u8,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    let fixed = [
        snapshot.root.address,
        snapshot.action_replay.address,
        snapshot.selection.address,
        snapshot.market.realm.address,
        snapshot.market.profile.address,
        snapshot.market.collateral_policy.address,
        snapshot.market.token_2022_program.address,
        snapshot.market.market_binding_v4.address,
        snapshot.market.market_runtime_v3.address,
        snapshot.market.market_instance_v2.address,
        snapshot.market.market_genesis_v2.address,
        snapshot.market.clock.address,
    ];
    require_pairwise_distinct(&fixed)?;
    authenticate_endpoint_aliases(&fixed, snapshot.endpoints, endpoint_count)?;

    let mut semantic = fixed.to_vec();
    for endpoint in snapshot.endpoints.into_iter().take(usize::from(endpoint_count)).flatten() {
        semantic.extend([
            endpoint.reservation.address,
            endpoint.position.address,
            endpoint.replay.address,
        ]);
    }
    if let Some(fee) = snapshot.fee {
        let policy = [
            fee.batch_policy.address,
            fee.revenue_record.address,
            fee.revenue_policy_preimage.address,
        ];
        require_pairwise_distinct(&policy)?;
        if policy.iter().any(|address| semantic.contains(address)) {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
        semantic.extend(policy);
        if fee.treasury_position.address == fee.treasury_replay.address {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
        for address in fixed.iter().chain(policy.iter()) {
            if *address == fee.treasury_position.address
                || *address == fee.treasury_replay.address
            {
                return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
            }
        }
        for endpoint in snapshot.endpoints.into_iter().take(usize::from(endpoint_count)).flatten() {
            let position_alias = endpoint.position.address == fee.treasury_position.address;
            let replay_alias = endpoint.replay.address == fee.treasury_replay.address;
            if position_alias != replay_alias
                || endpoint.reservation.address == fee.treasury_position.address
                || endpoint.reservation.address == fee.treasury_replay.address
                || endpoint.position.address == fee.treasury_replay.address
                || endpoint.replay.address == fee.treasury_position.address
            {
                return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
            }
        }
        if semantic.contains(&fee.treasury_service_ledger.address)
            || fee.treasury_service_ledger.address == fee.treasury_position.address
            || fee.treasury_service_ledger.address == fee.treasury_replay.address
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
        semantic.extend([
            fee.treasury_position.address,
            fee.treasury_replay.address,
            fee.treasury_service_ledger.address,
        ]);
    }
    let semantic_prefix = semantic.clone();
    for refund in snapshot
        .bond_refund_owners
        .into_iter()
        .take(usize::from(refund_count))
        .flatten()
    {
        if semantic_prefix.contains(&refund.address) {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
        semantic.push(refund.address);
    }
    if semantic.contains(&snapshot.liveness.policy.address)
        || semantic.contains(&snapshot.liveness.candidate.address)
        || snapshot.liveness.policy.address == snapshot.liveness.candidate.address
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    for recipient in [snapshot.liveness.keeper.address, snapshot.liveness.immutable_payer.address] {
        if semantic_prefix.contains(&recipient)
            || recipient == snapshot.liveness.policy.address
            || recipient == snapshot.liveness.candidate.address
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
    }
    Ok(())
}

fn authenticate_missed_aliases(
    snapshot: DirectMissedFreezeSnapshotV2<'_>,
    endpoint_count: u8,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    let fixed = [
        snapshot.root.address,
        snapshot.action_replay.address,
        snapshot.fresh_selection.address,
        snapshot.selection_payer.address,
        snapshot.system_program.address,
        snapshot.rent_sysvar.address,
        snapshot.clock.address,
        snapshot.compiler_bundle_v7.address,
        snapshot.native_claim_basis.address,
        snapshot.price_measure_policy.address,
        snapshot.market_genesis_v2.address,
        snapshot.price_grid.address,
        snapshot.realm.address,
        snapshot.profile.address,
        snapshot.collateral_policy.address,
        snapshot.token_2022_program.address,
        snapshot.market_binding_v4.address,
        snapshot.market_runtime_v3.address,
        snapshot.market_instance_v2.address,
    ];
    require_pairwise_distinct(&fixed)?;
    authenticate_endpoint_aliases(&fixed, snapshot.endpoints, endpoint_count)?;
    let mut semantic = fixed.to_vec();
    for endpoint in snapshot.endpoints.into_iter().take(usize::from(endpoint_count)).flatten() {
        semantic.extend([
            endpoint.reservation.address,
            endpoint.position.address,
            endpoint.replay.address,
        ]);
    }
    if semantic.contains(&snapshot.liveness.policy.address)
        || semantic.contains(&snapshot.liveness.candidate.address)
        || snapshot.liveness.policy.address == snapshot.liveness.candidate.address
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    for recipient in [snapshot.liveness.keeper.address, snapshot.liveness.immutable_payer.address] {
        if recipient == snapshot.liveness.policy.address
            || recipient == snapshot.liveness.candidate.address
            || (recipient != snapshot.selection_payer.address && semantic.contains(&recipient))
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
    }
    Ok(())
}

fn authenticate_endpoint_aliases(
    fixed: &[Address],
    endpoints: [Option<DirectTerminalEndpointSnapshotV2<'_>>; 2],
    endpoint_count: u8,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    for endpoint in endpoints.into_iter().take(usize::from(endpoint_count)).flatten() {
        let triple = [
            endpoint.reservation.address,
            endpoint.position.address,
            endpoint.replay.address,
        ];
        require_pairwise_distinct(&triple)?;
        if triple.iter().any(|address| fixed.contains(address)) {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
    }
    if endpoint_count == 2 {
        let left = endpoints[0].ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
        let right = endpoints[1].ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
        let positions_alias = left.position.address == right.position.address;
        let replays_alias = left.replay.address == right.replay.address;
        if left.reservation.address == right.reservation.address
            || positions_alias != replays_alias
            || left.reservation.address == right.position.address
            || left.reservation.address == right.replay.address
            || right.reservation.address == left.position.address
            || right.reservation.address == left.replay.address
            || left.position.address == right.replay.address
            || left.replay.address == right.position.address
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
    }
    Ok(())
}

fn require_pairwise_distinct(
    addresses: &[Address],
) -> Result<(), CanonicalActionMaterialErrorV1> {
    for (index, address) in addresses.iter().enumerate() {
        if addresses[..index].contains(address) {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
    }
    Ok(())
}

fn authenticate_terminal_cursor(
    release: &IndexedProgramRelease,
    selection: &KeeperActionSelection,
    freshness: ActionFreshnessBoundaryV1,
    root: &AuthenticatedDirectRootTransitionV3,
    sequence: u64,
    action: DirectMarketAction,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    let binding = root.candidate_liveness();
    let mut expected_dependencies = vec![
        Address::new_from_array(root.direct_root_account()),
        Address::new_from_array(root.action_replay_account()),
    ];
    if root.selection_account() != [0; 32] {
        expected_dependencies.push(Address::new_from_array(root.selection_account()));
    }
    expected_dependencies.extend([
        Address::new_from_array(binding.policy_account),
        Address::new_from_array(binding.candidate_account),
    ]);
    if selection.release_key != release.key()
        || selection.account.to_bytes() != root.direct_root_account()
        || selection.action != crate::action_material::direct_selection_action(action)
        || selection.observed_commitment != RpcCommitment::Finalized
        || selection.effective_commitment != RpcCommitment::Finalized
        || selection.cursor.workflow_id == [0; 32]
        || selection.cursor.lane != WorkflowLane::Candidate
        || selection.cursor.generation != root.generation()
        || selection.cursor.position.phase != u16::from(action.tag())
        || selection.cursor.position.item != sequence
        || selection.account_slot != freshness.observed_slot
        || selection.cursor.observed_state_sha256 == [0; 32]
        || selection.dependencies.len() != expected_dependencies.len()
        || expected_dependencies
            .iter()
            .any(|address| !selection.dependencies.contains(address))
    {
        return Err(CanonicalActionMaterialErrorV1::WrongSelection);
    }
    Ok(())
}

fn authenticate_fresh_selection(
    release: &IndexedProgramRelease,
    snapshot: DirectMissedFreezeSnapshotV2<'_>,
    root: &AuthenticatedDirectRootTransitionV3,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    let expected = pda(
        release.program_id,
        &[DIRECT_SELECTION_SEED_V1, snapshot.root.address.as_ref()],
    );
    if snapshot.fresh_selection.address != expected.0
        || snapshot.fresh_selection.owner != Address::default()
        || snapshot.fresh_selection.executable
        || !snapshot.fresh_selection.data.is_empty()
        || root.selection_account() != [0; 32]
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    require_operator(snapshot.selection_payer)?;
    require_system(snapshot.system_program)?;
    require_sysvar(snapshot.rent_sysvar, "SysvarRent111111111111111111111111111111111")
}

fn authenticate_missed_price_graph(
    release: &IndexedProgramRelease,
    root: &AuthenticatedDirectRootTransitionV3,
    snapshot: DirectMissedFreezeSnapshotV2<'_>,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    for account in [
        snapshot.compiler_bundle_v7,
        snapshot.native_claim_basis,
        snapshot.price_measure_policy,
        snapshot.market_genesis_v2,
        snapshot.market_instance_v2,
        snapshot.price_grid,
    ] {
        require_program_account(release, account)?;
    }
    let bundle_id = content_id::<clutch_product_series::CompiledProductSeriesBundleV7>(
        snapshot.compiler_bundle_v7,
        ArtifactKind::CompiledProductSeriesBundleV7,
    )?;
    if bundle_id != root.compiler_bundle_v7_id() {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let bundle = clutch_product_series::CompiledProductSeriesBundleV7::decode(
        &snapshot.compiler_bundle_v7.data,
    ).map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if content_id::<clutch_product_series::NativeClaimBasisV1>(
        snapshot.native_claim_basis,
        ArtifactKind::NativeClaimBasisV1,
    )? != bundle.native_claim_basis_id.content_id().bytes()
        || content_id::<clutch_product_series::PriceMeasurePolicyV1>(
            snapshot.price_measure_policy,
            ArtifactKind::PriceMeasurePolicyV1,
        )? != bundle.price_measure_policy_id.content_id().bytes()
        || content_id::<MarketGenesisProfileV2>(
            snapshot.market_genesis_v2,
            ArtifactKind::MarketGenesisProfileV2,
        )? != bundle.market_genesis_profile_id.content_id().bytes()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let grid = clutch_solana_layout::PriceGridAccount::decode(&snapshot.price_grid.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let genesis = MarketGenesisProfileV2::decode(&snapshot.market_genesis_v2.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let basis = clutch_product_series::NativeClaimBasisV1::decode(
        &snapshot.native_claim_basis.data,
    ).map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let price_policy = clutch_product_series::PriceMeasurePolicyV1::decode(
        &snapshot.price_measure_policy.data,
    ).map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    genesis.validate_bindings(&basis, &price_policy)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if grid.realm.0 != root.realm_id()
        || grid.grid.0 != genesis.price_grid_id.bytes()
        || grid.price_scale != root.price_scale()
        || snapshot.price_grid.address
            != pda(
                release.program_id,
                &[PRICE_GRID_SEED_V1, &grid.realm.0, &grid.grid.0],
            ).0
        || bundle.price_measure_policy_id.content_id().bytes() != root.price_policy_id()
        || genesis.realm_id.bytes() != root.realm_id()
        || genesis.profile_id.bytes() != root.collateral_profile_id()
        || genesis.relation_policy_id.bytes() != root.relation_policy_id()
        || genesis.fee_policy_id.bytes() != root.fee_policy().revenue_policy_v2_digest
        || genesis.candidate_lifecycle_policy_id.bytes()
            != root.candidate_lifecycle_policy_id()
        || genesis.candidate_liveness_policy_id.bytes()
            != root.candidate_liveness_policy_id()
        || basis.outcome_count != root.outcome_count()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    Ok(())
}

fn decode_open_root_replay(
    release: &IndexedProgramRelease,
    root_account: &ObservedRpcAccount,
    replay_account: &ObservedRpcAccount,
) -> Result<DirectRootReplayTransitionV2, CanonicalActionMaterialErrorV1> {
    require_program_account(release, root_account)?;
    require_program_account(release, replay_account)?;
    let root_frame = clutch_solana_layout::direct_market_v3::DirectMarketRootAccountV3::decode(
        &root_account.data,
    ).map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let root = clutch_direct_market_runtime::codec_v3::authenticate_direct_root_transition_body_v3(
        root_frame.semantic_body(),
        &OperatorSha256V1,
    ).map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let generation = root.generation().to_le_bytes();
    let expected_root = pda(
        release.program_id,
        &[b"dc:direct-market-root:v3", &root.market_instance_id(), &generation],
    );
    if root_account.address != expected_root.0
        || root_frame.bump() != expected_root.1
        || root.direct_root_account() != root_account.address.to_bytes()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let replay_bytes: &[u8; DIRECT_ACTION_REPLAY_ACCOUNT_BYTES] = replay_account
        .data.as_slice().try_into()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let replay_frame = clutch_solana_layout::direct_market_v1::DirectActionReplayAccountV1::decode(
        replay_bytes,
    ).map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let replay = clutch_direct_market_runtime::codec_v3::decode_direct_action_replay_body_for_transition_v3(
        replay_frame.semantic_body(),
        &root,
    ).map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let expected_replay = pda(
        release.program_id,
        &[DIRECT_REPLAY_SEED_V1, root_account.address.as_ref()],
    );
    if replay_account.address != expected_replay.0
        || replay_frame.bump() != expected_replay.1
        || root.action_replay_account() != replay_account.address.to_bytes()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let state = DirectRootReplayTransitionV2::authenticate(root, replay)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    require_rent(state.root().root_rent(), root_account.lamports)?;
    require_rent(state.replay().rent(), replay_account.lamports)?;
    Ok(state)
}

fn terminal_equations(
    selection: &ObservedRpcAccount,
    candidate: &ObservedRpcAccount,
    candidate_balance: u64,
    refunds: Option<DirectCandidateBondRefundShapeV2>,
    fee: Option<AuthenticatedDirectTerminalFeeV2>,
    collateral_mint: Address,
) -> Result<Vec<ExactEquation>, CanonicalActionMaterialErrorV1> {
    let mut equations = Vec::with_capacity(4);
    equations.push(ExactEquation {
        name: "Candidate compartment exact prestate balance".into(),
        unit: IntegerUnit::Lamports,
        left: u128::from(candidate.lamports),
        right: u128::from(candidate_balance),
    });
    if let Some(refunds) = refunds {
        let after = selection.lamports.checked_sub(refunds.total_lamports)
            .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
        equations.push(ExactEquation {
            name: "selection retained-bond principal conservation".into(),
            unit: IntegerUnit::Lamports,
            left: u128::from(selection.lamports),
            right: u128::from(after) + u128::from(refunds.total_lamports),
        });
    }
    if let Some(fee) = fee {
        equations.push(ExactEquation {
            name: "Direct terminal fee Hamilton allocation".into(),
            unit: IntegerUnit::CollateralAtoms { mint: collateral_mint },
            left: u128::from(fee.terminal.charged_fee_atoms),
            right: u128::from(fee.terminal.buyer_rebate_atoms)
                + u128::from(fee.terminal.seller_rebate_atoms)
                + u128::from(fee.terminal.treasury_atoms),
        });
        equations.push(ExactEquation {
            name: "Direct buyer fee headroom release".into(),
            unit: IntegerUnit::CollateralAtoms { mint: collateral_mint },
            left: u128::from(fee.maximum_fee_atoms),
            right: u128::from(fee.terminal.charged_fee_atoms)
                + u128::from(fee.terminal.refunded_headroom_atoms),
        });
    }
    Ok(equations)
}

fn push_existing_fixed(
    accounts: &mut Vec<AccountMeta>,
    roles: &mut Vec<crate::action_material::CanonicalAccountRoleV1>,
    snapshot: DirectExistingTerminalSnapshotV2<'_>,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    for (account, label, writable) in [
        (snapshot.root, "direct-root", true),
        (snapshot.action_replay, "direct-replay", true),
        (snapshot.selection, "direct-selection", true),
        (snapshot.market.realm, "realm", false),
        (snapshot.market.profile, "collateral-profile", false),
        (snapshot.market.collateral_policy, "collateral-policy", false),
        (snapshot.market.token_2022_program, "token-2022-program", false),
        (snapshot.market.market_binding_v4, "general-market-binding-v4", false),
        (snapshot.market.market_runtime_v3, "general-market-runtime-v3", false),
        (snapshot.market.market_instance_v2, "market-instance-v2", false),
        (snapshot.market.market_genesis_v2, "market-genesis-v2", false),
        (snapshot.market.clock, "clock-sysvar", false),
    ] {
        push(accounts, roles, account, label, writable, false);
    }
    Ok(())
}

fn push_endpoints(
    accounts: &mut Vec<AccountMeta>,
    roles: &mut Vec<crate::action_material::CanonicalAccountRoleV1>,
    endpoints: [Option<DirectTerminalEndpointSnapshotV2<'_>>; 2],
) -> Result<(), CanonicalActionMaterialErrorV1> {
    for endpoint in endpoints.into_iter().flatten() {
        push(accounts, roles, endpoint.reservation, "direct-reservation", true, false);
        push(accounts, roles, endpoint.position, "general-position-v3", true, false);
        push(accounts, roles, endpoint.replay, "general-replay-v3", true, false);
    }
    Ok(())
}

fn push_fee(
    accounts: &mut Vec<AccountMeta>,
    roles: &mut Vec<crate::action_material::CanonicalAccountRoleV1>,
    fee: DirectFeeSettlementSnapshotV2<'_>,
) {
    for (account, label, writable) in [
        (fee.batch_policy, "batch-policy", false),
        (fee.revenue_record, "revenue-policy-record-v2", false),
        (fee.revenue_policy_preimage, "revenue-policy-v2-preimage", false),
        (fee.treasury_position, "treasury-position-v3", true),
        (fee.treasury_replay, "treasury-replay-v3", true),
        (fee.treasury_service_ledger, "treasury-service-ledger", true),
    ] {
        push(accounts, roles, account, label, writable, false);
    }
}

fn push_refunds(
    accounts: &mut Vec<AccountMeta>,
    roles: &mut Vec<crate::action_material::CanonicalAccountRoleV1>,
    supplied: [Option<&ObservedRpcAccount>; 3],
    refunds: Option<DirectCandidateBondRefundShapeV2>,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    let count = refunds.map_or(0usize, |value| usize::from(value.refund_count));
    for account in supplied.into_iter().take(count) {
        push(
            accounts,
            roles,
            account.ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?,
            "candidate-bond-refund-owner",
            true,
            false,
        );
    }
    Ok(())
}

fn push_liveness(
    accounts: &mut Vec<AccountMeta>,
    roles: &mut Vec<crate::action_material::CanonicalAccountRoleV1>,
    value: DirectTerminalLivenessSnapshotV2<'_>,
) {
    for (account, label, writable, signer) in [
        (value.policy, "candidate-liveness-policy", false, false),
        (value.candidate, "candidate-compartment", true, false),
        (value.keeper, "keeper", true, true),
        (value.immutable_payer, "candidate-immutable-payer", true, false),
    ] {
        push(accounts, roles, account, label, writable, signer);
    }
}

fn push(
    accounts: &mut Vec<AccountMeta>,
    roles: &mut Vec<crate::action_material::CanonicalAccountRoleV1>,
    account: &ObservedRpcAccount,
    label: &'static str,
    writable: bool,
    signer: bool,
) {
    accounts.push(AccountMeta {
        pubkey: account.address,
        is_signer: signer,
        is_writable: writable,
    });
    roles.push(chain_derived_direct_role_v2(
        label,
        account.address,
        writable,
        signer,
    ));
}

fn authenticate_existing_observations(
    release: &IndexedProgramRelease,
    freshness: ActionFreshnessBoundaryV1,
    value: DirectExistingTerminalSnapshotV2<'_>,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    let mut accounts = vec![
        value.root,
        value.action_replay,
        value.selection,
        value.market.realm,
        value.market.profile,
        value.market.collateral_policy,
        value.market.token_2022_program,
        value.market.market_binding_v4,
        value.market.market_runtime_v3,
        value.market.market_instance_v2,
        value.market.market_genesis_v2,
        value.market.clock,
        value.liveness.policy,
        value.liveness.candidate,
        value.liveness.keeper,
        value.liveness.immutable_payer,
    ];
    for endpoint in value.endpoints.into_iter().flatten() {
        accounts.extend([endpoint.reservation, endpoint.position, endpoint.replay]);
    }
    if let Some(fee) = value.fee {
        accounts.extend([
            fee.batch_policy,
            fee.revenue_record,
            fee.revenue_policy_preimage,
            fee.treasury_position,
            fee.treasury_replay,
            fee.treasury_service_ledger,
        ]);
    }
    accounts.extend(value.bond_refund_owners.into_iter().flatten());
    authenticate_snapshot_set(release, freshness, &accounts)
}

fn authenticate_missed_observations(
    release: &IndexedProgramRelease,
    freshness: ActionFreshnessBoundaryV1,
    value: DirectMissedFreezeSnapshotV2<'_>,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    let mut accounts = vec![
        value.root,
        value.action_replay,
        value.fresh_selection,
        value.selection_payer,
        value.system_program,
        value.rent_sysvar,
        value.clock,
        value.compiler_bundle_v7,
        value.native_claim_basis,
        value.price_measure_policy,
        value.market_genesis_v2,
        value.price_grid,
        value.realm,
        value.profile,
        value.collateral_policy,
        value.token_2022_program,
        value.market_binding_v4,
        value.market_runtime_v3,
        value.market_instance_v2,
        value.liveness.policy,
        value.liveness.candidate,
        value.liveness.keeper,
        value.liveness.immutable_payer,
    ];
    for endpoint in value.endpoints.into_iter().flatten() {
        accounts.extend([endpoint.reservation, endpoint.position, endpoint.replay]);
    }
    authenticate_snapshot_set(release, freshness, &accounts)
}

fn require_program_account(
    release: &IndexedProgramRelease,
    account: &ObservedRpcAccount,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    if account.owner != release.program_id || account.executable {
        Err(CanonicalActionMaterialErrorV1::InvalidPlan)
    } else {
        Ok(())
    }
}

fn require_operator(account: &ObservedRpcAccount) -> Result<(), CanonicalActionMaterialErrorV1> {
    if account.address == Address::default()
        || account.owner != Address::default()
        || account.executable
        || !account.data.is_empty()
    {
        Err(CanonicalActionMaterialErrorV1::InvalidPlan)
    } else {
        Ok(())
    }
}

fn require_system(account: &ObservedRpcAccount) -> Result<(), CanonicalActionMaterialErrorV1> {
    if account.address != Address::default() || !account.executable {
        Err(CanonicalActionMaterialErrorV1::InvalidPlan)
    } else {
        Ok(())
    }
}

fn require_sysvar(
    account: &ObservedRpcAccount,
    address: &str,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    let address = Address::from_str(address)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let owner = Address::from_str("Sysvar1111111111111111111111111111111111111")
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if account.address != address || account.owner != owner || account.executable {
        Err(CanonicalActionMaterialErrorV1::InvalidPlan)
    } else {
        Ok(())
    }
}

fn account_data_id(domain: &[u8], account: &ObservedRpcAccount) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(account.address.as_ref());
    hash.update(&account.data);
    hash.finalize().into()
}

fn content_id<T: FixedCodec>(
    account: &ObservedRpcAccount,
    kind: ArtifactKind,
) -> Result<[u8; 32], CanonicalActionMaterialErrorV1>
where
    T: ProductContentIdentityV2,
{
    let value = T::decode(&account.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let id = value.content_identity()?;
    let kind_seed = [kind.byte()];
    let expected = pda(
        account.owner,
        &[PRODUCT_ARTIFACT_SEED, &kind_seed, &id],
    );
    if account.address != expected.0 {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    Ok(id)
}

trait ProductContentIdentityV2: FixedCodec {
    fn content_identity(&self) -> Result<[u8; 32], CanonicalActionMaterialErrorV1>;
}

impl ProductContentIdentityV2 for MarketInstancePreimageV2 {
    fn content_identity(&self) -> Result<[u8; 32], CanonicalActionMaterialErrorV1> {
        self.id().map(|value| value.content_id().bytes())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)
    }
}

impl ProductContentIdentityV2 for MarketGenesisProfileV2 {
    fn content_identity(&self) -> Result<[u8; 32], CanonicalActionMaterialErrorV1> {
        self.id().map(|value| value.content_id().bytes())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)
    }
}

impl ProductContentIdentityV2 for clutch_product_series::CompiledProductSeriesBundleV7 {
    fn content_identity(&self) -> Result<[u8; 32], CanonicalActionMaterialErrorV1> {
        self.id().map(|value| value.content_id().bytes())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)
    }
}

impl ProductContentIdentityV2 for clutch_product_series::NativeClaimBasisV1 {
    fn content_identity(&self) -> Result<[u8; 32], CanonicalActionMaterialErrorV1> {
        self.id().map(|value| value.content_id().bytes())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)
    }
}

impl ProductContentIdentityV2 for clutch_product_series::PriceMeasurePolicyV1 {
    fn content_identity(&self) -> Result<[u8; 32], CanonicalActionMaterialErrorV1> {
        self.id().map(|value| value.content_id().bytes())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)
    }
}

fn authenticate_family_retirement_observations(
    release: &IndexedProgramRelease,
    freshness: ActionFreshnessBoundaryV1,
    snapshot: DirectFamilyRetirementSnapshotV3<'_>,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    let mut accounts = vec![
        snapshot.product_root_v3,
        snapshot.series_link_v3,
        snapshot.root,
        snapshot.action_replay,
        snapshot.selection,
        snapshot.resolution_v5,
        snapshot.clock,
        snapshot.neutral_sink,
        snapshot.product_direct_global_liveness_v2,
        snapshot.liveness.policy,
        snapshot.liveness.candidate,
        snapshot.liveness.keeper,
        snapshot.liveness.immutable_payer,
    ];
    accounts.extend(snapshot.reservations.into_iter().flatten());
    accounts.extend(snapshot.rent_refund_owners.into_iter().flatten());
    authenticate_snapshot_set(release, freshness, &accounts)
}

fn authenticate_product_family_preterminal_v3(
    release: &IndexedProgramRelease,
    direct: &AuthenticatedDirectRootTransitionV3,
    product_root_account: &ObservedRpcAccount,
    series_link_account: &ObservedRpcAccount,
) -> Result<u32, CanonicalActionMaterialErrorV1> {
    require_program_account(release, product_root_account)?;
    require_program_account(release, series_link_account)?;
    let product = MarketLifecycleRootAccountV3::decode(&product_root_account.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let link = SeriesMarketLinkAccountV3::decode(&series_link_account.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let market = direct.market_instance_id();
    let generation = direct.generation();
    let product_pda = pda(
        release.program_id,
        &[b"dc:market-lifecycle-root:v1", &market, &generation.to_le_bytes()],
    );
    let series_plan = direct.series_plan_v5_id();
    let ordinal = direct.series_ordinal();
    let link_pda = pda(
        release.program_id,
        &[b"dc:series-market-link:v1", &series_plan, &ordinal.to_le_bytes()],
    );
    let product_binding = product.state.binding_ref();
    let link_binding = link.state.binding_ref();
    let family = product.state.product_families().family(MarketFamilyV1::Direct);
    let counts = family.counts();
    let link_accounted = link
        .state
        .rent_principal_lamports()
        .checked_add(link.state.current_donation_lamports())
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if product_root_account.address != product_pda.0
        || product.stored_bump != product_pda.1
        || product_root_account.address.to_bytes() != direct.product_root_account()
        || product_root_account.lamports < product.rent_principal_lamports
        || product_binding.market_instance_id.bytes() != market
        || product_binding.generation != generation
        || product_binding
            .id()
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
            .bytes()
            != direct.product_market_binding_v3_id()
        || !matches!(
            product.state.phase(),
            MarketLifecyclePhaseV3::Active | MarketLifecyclePhaseV3::Retiring
        )
        || series_link_account.address != link_pda.0
        || link.stored_bump != link_pda.1
        || series_link_account.address.to_bytes() != direct.series_link_account()
        || series_link_account.lamports < link_accounted
        || link_binding.series_plan_id.bytes() != series_plan
        || link_binding.ordinal != ordinal
        || link_binding.market_instance_id.bytes() != market
        || link_binding.generation != generation
        || link_binding.market_root_account_id.bytes() != product_root_account.address.to_bytes()
        || link_binding.market_binding_id.bytes() != direct.product_market_binding_v3_id()
        || link_binding
            .id()
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
            .bytes()
            != direct.series_link_binding_v3_id()
        || !matches!(
            link.state.phase(),
            SeriesMarketLinkPhaseV3::Active | SeriesMarketLinkPhaseV3::Retiring
        )
        || counts.live == 0
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    Ok(counts.terminal)
}

fn authenticate_resolution_v5(
    release: &IndexedProgramRelease,
    root: &AuthenticatedDirectRootTransitionV3,
    account: &ObservedRpcAccount,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    use clutch_collateral_adapter_v2::{Id as CollateralId, ResolutionStateV5, ResolutionV5};
    require_program_account(release, account)?;
    let resolution = ResolutionV5::decode(&account.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let expected = pda(
        release.program_id,
        &[b"dc:resolution:v5", &root.market_instance_id()],
    );
    let minimum = resolution
        .rent
        .refundable_principal()
        .checked_add(resolution.rent.donation_floor())
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let account_id = CollateralId::from_bytes(account.address.to_bytes());
    resolution
        .semantic_id(&OperatorSha256V1)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    resolution
        .data_id(account_id)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if account.address != expected.0
        || resolution.stored_bump != expected.1
        || account.address.to_bytes() != root.resolution_account()
        || account.lamports < minimum
        || resolution.state != ResolutionStateV5::Finalized
        || resolution.facts.market_instance_id.bytes() != root.market_instance_id()
        || resolution.facts.generation != root.generation()
        || resolution.facts.outcome_count != root.outcome_count()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    Ok(())
}

fn authenticate_terminal_reservation_archives(
    release: &IndexedProgramRelease,
    root: &AuthenticatedDirectRootTransitionV3,
    selection: &DirectSelectionV1,
    root_account: &ObservedRpcAccount,
    supplied: [Option<&ObservedRpcAccount>; 2],
) -> Result<[Option<DirectReservationV1>; 2], CanonicalActionMaterialErrorV1> {
    let count = usize::from(root.live_reservations());
    let mut output = [None; 2];
    for index in 0..2usize {
        match (index < count, supplied[index]) {
            (true, Some(account)) => {
                require_program_account(release, account)?;
                let bytes: &[u8; DIRECT_RESERVATION_ACCOUNT_BYTES] = account
                    .data
                    .as_slice()
                    .try_into()
                    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
                let frame = DirectReservationAccountV1::decode(bytes)
                    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
                let reservation = decode_direct_reservation_body_for_transition_v3(
                    frame.semantic_body(),
                    root,
                )
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
                let expected = pda(
                    release.program_id,
                    &[
                        DIRECT_RESERVATION_SEED_V1,
                        root_account.address.as_ref(),
                        &reservation.order_id(),
                    ],
                );
                let bounded = u8::try_from(index)
                    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
                if account.address != expected.0
                    || frame.bump() != expected.1
                    || reservation.account() != account.address.to_bytes()
                    || reservation.account()
                        != selection
                            .reservation_account(bounded)
                            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
                    || root
                        .child_reservation_semantic_id(reservation, &OperatorSha256V1)
                        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
                        != selection
                            .reservation_semantic_id(bounded)
                            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
                    || reservation.terminal_receipt_id() != selection.terminal_receipt_id()
                {
                    return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
                }
                require_rent(reservation.rent(), account.lamports)?;
                output[index] = Some(reservation);
            }
            (false, None) => {}
            _ => return Err(CanonicalActionMaterialErrorV1::InvalidPlan),
        }
    }
    Ok(output)
}

fn derive_family_retirement_transfer(
    root: &AuthenticatedDirectRootTransitionV3,
    replay: DirectActionReplayV1,
    selection: &DirectSelectionV1,
    snapshot: DirectFamilyRetirementSnapshotV3<'_>,
    reservations: [Option<DirectReservationV1>; 2],
) -> Result<DirectRetirementTransferV1, CanonicalActionMaterialErrorV1> {
    let mut sources = [None; 5];
    sources[0] = Some(DirectRetirementSourceV1 {
        account: snapshot.root.address.to_bytes(),
        rent: root.root_rent(),
        observed_lamports: snapshot.root.lamports,
    });
    sources[1] = Some(DirectRetirementSourceV1 {
        account: snapshot.action_replay.address.to_bytes(),
        rent: replay.rent(),
        observed_lamports: snapshot.action_replay.lamports,
    });
    sources[2] = Some(DirectRetirementSourceV1 {
        account: snapshot.selection.address.to_bytes(),
        rent: selection.rent(),
        observed_lamports: snapshot.selection.lamports,
    });
    for index in 0..usize::from(root.live_reservations()) {
        let reservation = reservations[index]
            .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
        let account = snapshot.reservations[index]
            .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
        sources[3 + index] = Some(DirectRetirementSourceV1 {
            account: account.address.to_bytes(),
            rent: reservation.rent(),
            observed_lamports: account.lamports,
        });
    }
    build_direct_retirement_transfer_v1(sources, snapshot.neutral_sink.address.to_bytes())
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)
}

fn authenticate_family_refund_owners(
    supplied: [Option<&ObservedRpcAccount>; 5],
    retirement: DirectRetirementTransferV1,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    for index in 0..5usize {
        match (index < usize::from(retirement.refund_count), supplied[index]) {
            (true, Some(account)) => {
                let refund = retirement.refunds[index]
                    .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
                if account.executable || account.address.to_bytes() != refund.recipient {
                    return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
                }
            }
            (false, None) => {}
            _ => return Err(CanonicalActionMaterialErrorV1::InvalidPlan),
        }
    }
    Ok(())
}

fn authenticate_product_direct_manifest_preterminal_v2(
    release: &IndexedProgramRelease,
    root: &AuthenticatedDirectRootTransitionV3,
    family_terminal_sequence: u32,
    account: &ObservedRpcAccount,
    candidate: &ObservedRpcAccount,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    require_program_account(release, account)?;
    let manifest = ProductDirectGlobalLivenessAccountV2::decode(&account.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let state = &manifest.state;
    let market = root.market_instance_id();
    let generation = root.generation();
    let expected = pda(
        release.program_id,
        &[
            PRODUCT_DIRECT_GLOBAL_LIVENESS_PDA_PREFIX_V2,
            &market,
            &generation.to_le_bytes(),
        ],
    );
    let minimum = manifest
        .rent_principal_lamports
        .checked_add(state.manifest_initial_donation_lamports())
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let candidate_id = state
        .compartment_account(RuntimeCompartmentKindV1::Candidate.index())
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if account.address != expected.0
        || manifest.stored_bump != expected.1
        || account.address.to_bytes() != root.product_global_liveness_account()
        || account.lamports < minimum
        || state.phase() != DirectGlobalLivenessPhaseV2::Active
        || state.account_id().bytes() != account.address.to_bytes()
        || state.market_instance_id().bytes() != market
        || state.generation() != generation
        || state.lifecycle_root_account().bytes() != root.product_root_account()
        || state.activated_market_binding_id().bytes() != root.product_market_binding_v3_id()
        || state.global_bundle_binding_id().bytes()
            != root.product_global_liveness_binding_id()
        || state
            .work_quote_id()
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
            .bytes()
            != root.direct_work_quote_id()
        || state.live_allocations() != 1
        || state.retired_allocations() != family_terminal_sequence
        || candidate_id.bytes() != candidate.address.to_bytes()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    Ok(())
}

fn authenticate_family_retirement_aliases(
    snapshot: DirectFamilyRetirementSnapshotV3<'_>,
    retirement: DirectRetirementTransferV1,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    let mut fixed = vec![
        snapshot.product_root_v3.address,
        snapshot.series_link_v3.address,
        snapshot.root.address,
        snapshot.action_replay.address,
        snapshot.selection.address,
        snapshot.resolution_v5.address,
        snapshot.clock.address,
        snapshot.neutral_sink.address,
        snapshot.product_direct_global_liveness_v2.address,
        snapshot.liveness.policy.address,
        snapshot.liveness.candidate.address,
    ];
    fixed.extend(
        snapshot
            .reservations
            .into_iter()
            .take(usize::from(retirement.source_count.saturating_sub(3)))
            .flatten()
            .map(|account| account.address),
    );
    require_pairwise_distinct(&fixed)?;
    for refund in snapshot
        .rent_refund_owners
        .into_iter()
        .take(usize::from(retirement.refund_count))
        .flatten()
    {
        if fixed.contains(&refund.address) {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
    }
    for account in [snapshot.liveness.keeper, snapshot.liveness.immutable_payer] {
        if fixed.contains(&account.address) {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
    }
    Ok(())
}

fn retirement_source_lamports(
    retirement: DirectRetirementTransferV1,
) -> Result<u64, CanonicalActionMaterialErrorV1> {
    let mut total = 0u64;
    for source in retirement
        .sources
        .into_iter()
        .take(usize::from(retirement.source_count))
    {
        total = total
            .checked_add(
                source
                    .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?
                    .observed_lamports,
            )
            .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    }
    Ok(total)
}

fn retirement_refund_lamports(
    retirement: DirectRetirementTransferV1,
) -> Result<u64, CanonicalActionMaterialErrorV1> {
    let mut total = 0u64;
    for refund in retirement
        .refunds
        .into_iter()
        .take(usize::from(retirement.refund_count))
    {
        total = total
            .checked_add(
                refund
                    .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?
                    .lamports,
            )
            .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    }
    Ok(total)
}

fn pda(program: Address, seeds: &[&[u8]]) -> (Address, u8) {
    Address::find_program_address(seeds, &program)
}

#[cfg(test)]
mod source_contract_tests {
    use super::*;

    #[test]
    fn terminal_action_mapping_is_deadline_partitioned() {
        assert_ne!(DirectMarketAction::SettlePair.tag(), DirectMarketAction::LapseSelected.tag());
        assert_ne!(DirectTerminalReasonV1::Settled.byte(), DirectTerminalReasonV1::SelectedLapse.byte());
    }

    #[test]
    fn fee_and_lapse_account_suffixes_cannot_alias_by_width() {
        let endpoints = 2usize;
        let refunds = 3usize;
        assert_eq!(12 + 3 * endpoints + 6 + refunds + 4, 31);
        assert_eq!(12 + 3 * endpoints + refunds + 4, 25);
    }

    #[test]
    fn missed_freeze_has_no_existing_selection_or_refund_suffix() {
        for endpoints in 0usize..=2 {
            assert_eq!(19 + 3 * endpoints + 4, 23 + 3 * endpoints);
        }
    }
}
