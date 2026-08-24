//! Server-owned acquisition and materialization for current Direct actions 2–7.
//!
//! The browser never selects an action body, account, semantic identity, or
//! signer. A configured payer and the finalized canonical index select work;
//! this module plans one address-exact `getMultipleAccounts`, hostile-decodes
//! its response, proves the scheduled cursor bytes are unchanged at that later
//! finalized slot, and invokes the typed action owner. Output remains unsigned
//! and blockhash-free.

use crate::account_index::CanonicalAccountIndex;
use crate::action_material::{
    direct_action_from_selection, ActionFreshnessBoundaryV1,
    CanonicalActionMaterialErrorV1, CanonicalActionMaterialV1,
};
use crate::direct_candidate_material::{
    construct_direct_candidate_submission_v1,
    construct_next_direct_candidate_verification_v1,
    DirectCandidateSubmissionSnapshotV1, DirectCandidateVerificationSnapshotV1,
};
use crate::direct_order_material::{
    construct_direct_admit_order_v2, construct_direct_cancel_order_v2,
    construct_direct_freeze_book_v2, derive_direct_admit_reservation_address_v2,
    finalized_dependency_digest, DirectAdmitOrderSnapshotV2,
    DirectCancelOrderSnapshotV2, DirectFreezeBookSnapshotV2,
    DirectReservationAuthoritySnapshotV2,
};
use crate::operatord::{KeeperActionSelection, OperatorJsonApi, ResumableKeeperSelector};
use crate::rpc_index::{
    decode_exact_finalized_account_batch_v1, CanonicalIntentCoordinate,
    FinalizedExactAccountV1, IndexedProgramRelease, ObservedRpcAccount,
    PlannedExactAccountBatchV1, RpcCommitment,
};
use crate::transaction_builder::{ProtocolTransactionBuilder, TransactionTransport};
use crate::workflow_graph::{ExplicitOperatorReleaseManifest, WorkflowLane};
use clutch_collateral_adapter_v2::CollateralPolicyV2;
use clutch_direct_market_runtime::codec_v2::authenticate_direct_root_transition_body_v2;
use clutch_direct_market_runtime::DirectHashBackendV1;
use clutch_general_v2_contract::MarketBindingV4;
use clutch_liveness::RuntimeCompartmentV1;
use clutch_product_series::{
    CompiledProductSeriesBundleV6, FixedCodec, MarketGenesisProfileV2,
};
use clutch_retirement::{PositionAccountV3, POSITION_V3_PDA_PREFIX};
use clutch_solana_layout::artifact::ArtifactKind;
use clutch_solana_layout::direct_market_v2::DirectMarketRootAccountV2;
use clutch_solana_layout::registry::{
    DirectMarketAction, DIRECT_MARKET_FAMILY_TAG, DIRECT_MARKET_FAMILY_VERSION,
};
use sha2::{Digest, Sha256};
use solana_address::Address;
use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

const PRODUCT_ARTIFACT_SEED_V1: &[u8] = b"dc:product-artifact:v1";
const REALM_SEED_V1: &[u8] = b"dragons-clutch:realm:v1";
const PROFILE_SEED_V1: &[u8] = b"dragons-clutch:profile:v1";
const COLLATERAL_POLICY_SEED_V1: &[u8] = b"dragons-clutch:policy:v1";
const PRICE_GRID_SEED_V1: &[u8] = b"dragons-clutch:grid:v1";

pub type Result<T> = core::result::Result<T, DirectOperatorServerErrorV2>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectOperatorServerErrorV2 {
    InvalidConfiguration,
    Selection,
    MissingChainAccount,
    MalformedChainAccount,
    Acquisition,
    Material,
}

impl core::fmt::Display for DirectOperatorServerErrorV2 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidConfiguration => "Direct operator server configuration is invalid",
            Self::Selection => "finalized Direct state did not select this action",
            Self::MissingChainAccount => "a chain-derived Direct dependency is absent",
            Self::MalformedChainAccount => "a chain-derived Direct dependency was refused",
            Self::Acquisition => "the exact finalized Direct account batch was refused",
            Self::Material => "the typed Direct action material constructor refused the batch",
        })
    }
}

impl std::error::Error for DirectOperatorServerErrorV2 {}

/// One opaque acquisition job owned by the server. Role ordering and cursor
/// dependencies are private; the transport receives only the exact RPC body.
#[derive(Clone, Debug)]
pub struct PlannedDirectOperatorMaterialV2 {
    selection: KeeperActionSelection,
    action: DirectMarketAction,
    request: PlannedExactAccountBatchV1,
    roles: Vec<Address>,
    cursor_dependencies: Vec<Address>,
    maximum_validity_slots: u64,
}

impl PlannedDirectOperatorMaterialV2 {
    #[must_use]
    pub const fn request(&self) -> &PlannedExactAccountBatchV1 { &self.request }
    #[must_use]
    pub const fn action(&self) -> DirectMarketAction { self.action }
}

/// Server-owned output ready to bind through
/// `OperatorJsonApi::with_direct_operator_materials`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DirectOperatorMaterialSetV2 {
    materials: Vec<CanonicalActionMaterialV1>,
}

impl DirectOperatorMaterialSetV2 {
    #[must_use]
    pub fn materials(&self) -> &[CanonicalActionMaterialV1] { &self.materials }

    pub fn push(&mut self, material: CanonicalActionMaterialV1) -> Result<()> {
        if self.materials.iter().any(|prior| {
            prior.driver_account() == material.driver_account()
                && prior.coordinate() == material.coordinate()
                && prior.cursor() == material.cursor()
        }) {
            return Err(DirectOperatorServerErrorV2::Material);
        }
        self.materials.push(material);
        self.materials.sort_by_key(|material| {
            (material.driver_account(), material.coordinate().local_action)
        });
        Ok(())
    }

    /// Register this exact opaque set in the daemon's read-only projection.
    /// No material bytes can be supplied by the browser at this boundary.
    #[must_use]
    pub fn read_only_api<'a>(
        &'a self,
        index: &'a CanonicalAccountIndex,
        selector: ResumableKeeperSelector,
        operator: Address,
    ) -> Result<OperatorJsonApi<'a>> {
        if operator == Address::default()
            || self
                .materials
                .iter()
                .any(|material| material.fee_payer() != operator)
        {
            return Err(DirectOperatorServerErrorV2::InvalidConfiguration);
        }
        OperatorJsonApi::with_direct_operator_materials(
            index,
            selector,
            &self.materials,
            operator,
        )
        .map_err(|_| DirectOperatorServerErrorV2::InvalidConfiguration)
    }
}

/// Select every currently routed Direct action 2–7 and build one bounded exact
/// finalized batch request for each. Capability-absent coordinates are not
/// requested or advertised.
#[allow(clippy::too_many_arguments)]
pub fn plan_current_direct_operator_materials_v2(
    index: &CanonicalAccountIndex,
    selector: ResumableKeeperSelector,
    operator: Address,
    request_id_start: u64,
    maximum_validity_slots: u64,
) -> Result<Vec<PlannedDirectOperatorMaterialV2>> {
    if operator == Address::default()
        || request_id_start == 0
        || maximum_validity_slots == 0
        || maximum_validity_slots > 512
    {
        return Err(DirectOperatorServerErrorV2::InvalidConfiguration);
    }
    let mut selections = selector
        .select_direct_operator_reservations_v2(index, RpcCommitment::Finalized, operator)
        .map_err(|_| DirectOperatorServerErrorV2::Selection)?;
    selections.extend(
        selector
            .select(index, RpcCommitment::Finalized)
            .map_err(|_| DirectOperatorServerErrorV2::Selection)?
            .into_iter()
            .filter(|selection| {
                direct_action_from_selection(selection.action).is_some_and(|action| {
                    matches!(
                        action,
                        DirectMarketAction::FreezeBook
                            | DirectMarketAction::SubmitCandidate
                            | DirectMarketAction::BeginVerification
                            | DirectMarketAction::VerifyCandidate
                    )
                })
            }),
    );
    selections.sort_by_key(|selection| {
        (
            selection.account,
            selection.cursor.position.phase,
            selection.cursor.position.item,
        )
    });
    selections.dedup_by(|left, right| {
        left.account == right.account
            && left.cursor.position == right.cursor.position
            && left.action == right.action
    });
    let mut output = Vec::with_capacity(selections.len());
    for (offset, selection) in selections.into_iter().enumerate() {
        let action = direct_action_from_selection(selection.action)
            .ok_or(DirectOperatorServerErrorV2::Selection)?;
        let release = index
            .release(&selection.release_key)
            .ok_or(DirectOperatorServerErrorV2::Selection)?;
        let coordinate = CanonicalIntentCoordinate {
            family_tag: DIRECT_MARKET_FAMILY_TAG,
            family_version: DIRECT_MARKET_FAMILY_VERSION,
            local_action: action.tag(),
        };
        if release.enabled_intents.binary_search(&coordinate).is_err() {
            continue;
        }
        let (roles, cursor_dependencies) = plan_direct_role_addresses_v2(
            index,
            release,
            &selection,
            operator,
        )?;
        let mut exact = Vec::new();
        let mut seen = BTreeSet::new();
        for address in roles.iter().chain(&cursor_dependencies) {
            if seen.insert(*address) {
                exact.push(*address);
            }
        }
        let request_id = request_id_start
            .checked_add(u64::try_from(offset).map_err(|_| {
                DirectOperatorServerErrorV2::InvalidConfiguration
            })?)
            .ok_or(DirectOperatorServerErrorV2::InvalidConfiguration)?;
        let request = index
            .acquisition_plan()
            .exact_finalized_account_batch_v1(
                &selection.release_key,
                request_id,
                selection.account_slot,
                exact,
            )
            .map_err(|_| DirectOperatorServerErrorV2::Acquisition)?;
        output.push(PlannedDirectOperatorMaterialV2 {
            selection,
            action,
            request,
            roles,
            cursor_dependencies,
            maximum_validity_slots,
        });
    }
    Ok(output)
}

/// Decode one exact finalized response, prove it still matches the scheduled
/// cursor, and invoke the sole typed constructor for the selected action.
#[allow(clippy::too_many_arguments)]
pub fn materialize_current_direct_operator_v2(
    index: &CanonicalAccountIndex,
    manifest: &ExplicitOperatorReleaseManifest,
    operator: Address,
    transport: TransactionTransport,
    plan: &PlannedDirectOperatorMaterialV2,
    response: &serde_json::Value,
    receive_sequence_start: u64,
) -> Result<CanonicalActionMaterialV1> {
    let release = index
        .release(&plan.selection.release_key)
        .ok_or(DirectOperatorServerErrorV2::Selection)?;
    let decoded = decode_exact_finalized_account_batch_v1(
        index.acquisition_plan(),
        &plan.request,
        response,
        receive_sequence_start,
    )
    .map_err(|_| DirectOperatorServerErrorV2::Acquisition)?;
    let batch = ExactBatchAccountsV2::new(decoded)?;
    let slot = batch.slot()?;
    let valid_before_slot = slot
        .checked_add(plan.maximum_validity_slots)
        .ok_or(DirectOperatorServerErrorV2::InvalidConfiguration)?;
    let freshness = ActionFreshnessBoundaryV1 {
        observed_slot: slot,
        valid_before_slot,
        maximum_validity_slots: plan.maximum_validity_slots,
    };
    let mut dependencies = plan
        .cursor_dependencies
        .iter()
        .map(|address| batch.present(*address))
        .collect::<Result<Vec<_>>>()?;
    if matches!(
        plan.action,
        DirectMarketAction::FreezeBook
            | DirectMarketAction::SubmitCandidate
            | DirectMarketAction::BeginVerification
            | DirectMarketAction::VerifyCandidate
    ) {
        dependencies.sort_by_key(|account| account.address);
    }
    let dependency_id = finalized_dependency_digest(&dependencies)
        .map_err(|_| DirectOperatorServerErrorV2::Selection)?;
    if dependency_id != plan.selection.cursor.observed_state_sha256
        || plan.selection.cursor.lane != WorkflowLane::Candidate
        || plan.selection.observed_commitment != RpcCommitment::Finalized
        || plan.selection.effective_commitment != RpcCommitment::Finalized
        || plan.selection.dependencies.len() != plan.cursor_dependencies.len()
        || plan.cursor_dependencies.iter().any(|address| {
            !plan.selection.dependencies.contains(address)
        })
    {
        return Err(DirectOperatorServerErrorV2::Selection);
    }
    let selection = plan.selection.clone();
    let builder = ProtocolTransactionBuilder::new(
        operator,
        release.program_id,
        release.elf_sha256,
        transport,
    )
    .map_err(|_| DirectOperatorServerErrorV2::InvalidConfiguration)?;
    construct_from_roles_v2(
        release,
        manifest,
        &builder,
        &selection,
        freshness,
        plan.action,
        &plan.roles,
        &batch,
    )
}

#[allow(clippy::too_many_arguments)]
fn construct_from_roles_v2(
    release: &IndexedProgramRelease,
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    selection: &KeeperActionSelection,
    freshness: ActionFreshnessBoundaryV1,
    action: DirectMarketAction,
    roles: &[Address],
    batch: &ExactBatchAccountsV2,
) -> Result<CanonicalActionMaterialV1> {
    let account = |index: usize| -> Result<&ObservedRpcAccount> {
        let address = roles
            .get(index)
            .copied()
            .ok_or(DirectOperatorServerErrorV2::MalformedChainAccount)?;
        batch.present(address)
    };
    let material = match action {
        DirectMarketAction::AdmitOrder => {
            let fresh = batch.fresh(*roles.get(2).ok_or(
                DirectOperatorServerErrorV2::MalformedChainAccount,
            )?)?;
            let peer = if roles.len() == 20 { Some(account(19)?) } else { None };
            construct_direct_admit_order_v2(
                release,
                manifest,
                builder,
                selection,
                freshness,
                DirectAdmitOrderSnapshotV2 {
                    root: account(0)?, replay: account(1)?, fresh_reservation: fresh,
                    actor_payer: account(3)?,
                    authority: DirectReservationAuthoritySnapshotV2 {
                        position_v3: account(4)?, position_replay_v3: account(5)?,
                        realm: account(6)?, collateral_profile: account(7)?,
                        collateral_policy: account(8)?, token_program: account(9)?,
                        general_market_binding_v4: account(10)?,
                        general_market_runtime: account(11)?,
                        market_instance_v2: account(12)?, market_genesis_v2: account(17)?,
                    },
                    system_program: account(13)?, rent_sysvar: account(14)?,
                    clock: account(15)?, compiler_bundle_v6: account(16)?,
                    price_grid: account(18)?, existing_peer: peer,
                },
            )?.canonical().clone()
        }
        DirectMarketAction::CancelOrder => {
            let authority = DirectReservationAuthoritySnapshotV2 {
                position_v3: account(4)?, position_replay_v3: account(5)?,
                realm: account(6)?, collateral_profile: account(7)?,
                collateral_policy: account(8)?, token_program: account(9)?,
                general_market_binding_v4: account(10)?,
                general_market_runtime: account(11)?, market_instance_v2: account(12)?,
                market_genesis_v2: account(13)?,
            };
            construct_direct_cancel_order_v2(
                release,
                manifest,
                builder,
                selection,
                freshness,
                DirectCancelOrderSnapshotV2 {
                    root: account(0)?, replay: account(1)?, reservation: account(2)?,
                    actor_payer: account(3)?, authority,
                    neutral_sink: account(14)?, clock: account(15)?,
                },
            )?.canonical().clone()
        }
        DirectMarketAction::FreezeBook => {
            let count = roles.len().checked_sub(16)
                .ok_or(DirectOperatorServerErrorV2::MalformedChainAccount)?;
            if count > 2 { return Err(DirectOperatorServerErrorV2::MalformedChainAccount); }
            let mut reservations = [None; 2];
            for index in 0..count { reservations[index] = Some(account(12 + index)?); }
            let suffix = 12 + count;
            let fresh = batch.fresh(roles[2])?;
            construct_direct_freeze_book_v2(
                release,
                manifest,
                builder,
                selection,
                freshness,
                DirectFreezeBookSnapshotV2 {
                    root: account(0)?, replay: account(1)?, fresh_selection: fresh,
                    creation_payer: account(3)?, system_program: account(4)?,
                    rent_sysvar: account(5)?, clock: account(6)?,
                    compiler_bundle_v6: account(7)?, native_claim_basis: account(8)?,
                    price_measure_policy: account(9)?, market_genesis_v2: account(10)?,
                    price_grid: account(11)?, reservations,
                    liveness_policy: account(suffix)?, candidate_compartment: account(suffix + 1)?,
                    keeper: account(suffix + 2)?, candidate_payer: account(suffix + 3)?,
                },
            )?.canonical().clone()
        }
        DirectMarketAction::SubmitCandidate => {
            construct_direct_candidate_submission_v1(
                release,
                manifest,
                builder,
                selection,
                freshness,
                DirectCandidateSubmissionSnapshotV1 {
                    root: account(0)?, replay: account(1)?, selection: account(2)?,
                    clock: account(3)?, submitter: account(4)?, system_program: account(5)?,
                    cursor_liveness_policy: account(6)?,
                    cursor_candidate_compartment: account(7)?,
                    evicted_refund_owner: None,
                },
            )?.canonical().clone()
        }
        DirectMarketAction::BeginVerification | DirectMarketAction::VerifyCandidate => {
            construct_next_direct_candidate_verification_v1(
                release,
                manifest,
                builder,
                selection,
                freshness,
                DirectCandidateVerificationSnapshotV1 {
                    root: account(0)?, replay: account(1)?, selection: account(2)?,
                    clock: account(3)?, liveness_policy: account(4)?,
                    candidate_compartment: account(5)?, keeper: account(6)?,
                    candidate_payer: account(7)?,
                },
            )?.canonical().clone()
        }
        _ => return Err(DirectOperatorServerErrorV2::Selection),
    };
    Ok(material)
}

fn plan_direct_role_addresses_v2(
    index: &CanonicalAccountIndex,
    release: &IndexedProgramRelease,
    selection: &KeeperActionSelection,
    operator: Address,
) -> Result<(Vec<Address>, Vec<Address>)> {
    let action = direct_action_from_selection(selection.action)
        .ok_or(DirectOperatorServerErrorV2::Selection)?;
    let root_account = current(index, selection, selection.account)?;
    let root_frame = DirectMarketRootAccountV2::decode(&root_account.data)
        .map_err(|_| DirectOperatorServerErrorV2::MalformedChainAccount)?;
    let root = authenticate_direct_root_transition_body_v2(
        root_frame.semantic_body(),
        &ServerDirectHashV2,
    )
    .map_err(|_| DirectOperatorServerErrorV2::MalformedChainAccount)?;
    let replay = Address::new_from_array(root.action_replay_account());
    let selection_account = Address::new_from_array(root.selection_account());
    let realm = pda(&[REALM_SEED_V1, &root.realm_id()], release.program_id);
    let profile = pda(
        &[PROFILE_SEED_V1, &root.realm_id(), &root.collateral_profile_id()],
        release.program_id,
    );
    let collateral = pda(
        &[COLLATERAL_POLICY_SEED_V1, &root.collateral_profile_id(), &root.collateral_policy_id()],
        release.program_id,
    );
    let collateral_account = current(index, selection, collateral)?;
    let collateral_value = CollateralPolicyV2::decode(&collateral_account.data)
        .map_err(|_| DirectOperatorServerErrorV2::MalformedChainAccount)?;
    let token_program = Address::new_from_array(collateral_value.token_program.bytes());
    let binding = Address::new_from_array(root.general_market_binding_account());
    let runtime = Address::new_from_array(root.general_market_runtime_account());
    let binding_account = current(index, selection, binding)?;
    let binding_value = MarketBindingV4::decode(&binding_account.data)
        .map_err(|_| DirectOperatorServerErrorV2::MalformedChainAccount)?;
    let genesis_id = binding_value.base().base().market_genesis_profile_v2_id.bytes();
    let market_instance = artifact(ArtifactKind::MarketInstancePreimageV2, root.market_instance_id(), release.program_id);
    let genesis = artifact(ArtifactKind::MarketGenesisProfileV2, genesis_id, release.program_id);
    let bundle = artifact(ArtifactKind::CompiledProductSeriesBundleV6, root.compiler_bundle_v6_id(), release.program_id);
    let genesis_account = current(index, selection, genesis)?;
    let genesis_value = MarketGenesisProfileV2::decode(&genesis_account.data)
        .map_err(|_| DirectOperatorServerErrorV2::MalformedChainAccount)?;
    let grid = pda(
        &[PRICE_GRID_SEED_V1, &root.realm_id(), &genesis_value.price_grid_id.bytes()],
        release.program_id,
    );
    let purpose = [u8::from(clutch_retirement::PositionPurposeV3::General)];
    let (position, position_replay, position_account) = if matches!(
        action,
        DirectMarketAction::AdmitOrder | DirectMarketAction::CancelOrder
    ) {
        let position = pda(
            &[POSITION_V3_PDA_PREFIX, &root.market_instance_id(), operator.as_ref(), &purpose, runtime.as_ref()],
            release.program_id,
        );
        let position_account = current(index, selection, position)?;
        let position_value = PositionAccountV3::decode(&position_account.data)
            .map_err(|_| DirectOperatorServerErrorV2::MalformedChainAccount)?;
        (
            position,
            Address::new_from_array(position_value.fields().replay_account.bytes()),
            Some(position_account),
        )
    } else {
        (Address::default(), Address::default(), None)
    };
    let system = Address::default();
    let clock = parse("SysvarC1ock11111111111111111111111111111111")?;
    let rent = parse("SysvarRent111111111111111111111111111111111")?;
    let liveness = root.candidate_liveness();
    let liveness_policy = Address::new_from_array(liveness.policy_account);
    let candidate = Address::new_from_array(liveness.candidate_account);
    let candidate_account = current(index, selection, candidate)?;
    let candidate_value = RuntimeCompartmentV1::decode(&candidate_account.data)
        .map_err(|_| DirectOperatorServerErrorV2::MalformedChainAccount)?;
    let candidate_payer = Address::new_from_array(candidate_value.identity.payer.bytes());
    let reservations = (0..root.live_reservations())
        .map(|index| root.reservation_account(index)
            .map(Address::new_from_array)
            .map_err(|_| DirectOperatorServerErrorV2::MalformedChainAccount))
        .collect::<Result<Vec<_>>>()?;
    let roles = match action {
        DirectMarketAction::AdmitOrder => {
            // Token-program executability is reauthenticated by the final
            // exact batch. The preliminary address derivation needs only the
            // immutable program identity selected by CollateralPolicyV2.
            let preliminary_token = ObservedRpcAccount {
                address: token_program,
                owner: Address::default(),
                lamports: 1,
                executable: true,
                rent_epoch: 0,
                data: Vec::new(),
                provenance: root_account.provenance.clone(),
            };
            let authority = DirectReservationAuthoritySnapshotV2 {
                realm: current(index, selection, realm)?,
                collateral_profile: current(index, selection, profile)?,
                collateral_policy: collateral_account,
                token_program: &preliminary_token,
                general_market_binding_v4: binding_account,
                general_market_runtime: current(index, selection, runtime)?,
                market_instance_v2: current(index, selection, market_instance)?,
                market_genesis_v2: genesis_account,
                position_v3: position_account.ok_or(
                    DirectOperatorServerErrorV2::MissingChainAccount,
                )?,
                position_replay_v3: current(index, selection, position_replay)?,
            };
            let peer = reservations.first()
                .map(|address| current(index, selection, *address))
                .transpose()?;
            let freshness = ActionFreshnessBoundaryV1 {
                observed_slot: selection.account_slot,
                valid_before_slot: selection.account_slot.checked_add(1)
                    .ok_or(DirectOperatorServerErrorV2::InvalidConfiguration)?,
                maximum_validity_slots: 1,
            };
            let fresh = derive_direct_admit_reservation_address_v2(
                release, freshness, root_account, current(index, selection, replay)?, operator,
                authority, current(index, selection, bundle)?, current(index, selection, grid)?, peer,
            ).map_err(|_| DirectOperatorServerErrorV2::MalformedChainAccount)?;
            let mut roles = vec![
                selection.account, replay, fresh, operator, position, position_replay,
                realm, profile, collateral, token_program, binding, runtime, market_instance,
                system, rent, clock, bundle, genesis, grid,
            ];
            if let Some(peer) = reservations.first() { roles.push(*peer); }
            roles
        }
        DirectMarketAction::CancelOrder => {
            let target = selection.dependencies.last().copied()
                .filter(|address| reservations.contains(address))
                .ok_or(DirectOperatorServerErrorV2::Selection)?;
            vec![selection.account, replay, target, operator, position, position_replay,
                realm, profile, collateral, token_program, binding, runtime, market_instance,
                genesis, Address::new_from_array(root.neutral_lamport_sink()), clock]
        }
        DirectMarketAction::FreezeBook => {
            let selection_fresh = pda(&[b"dc:direct-selection:v1", selection.account.as_ref()], release.program_id);
            let bundle_account = current(index, selection, bundle)?;
            let bundle_value = CompiledProductSeriesBundleV6::decode(&bundle_account.data)
                .map_err(|_| DirectOperatorServerErrorV2::MalformedChainAccount)?;
            let basis = artifact(ArtifactKind::NativeClaimBasisV1, bundle_value.native_claim_basis_id.bytes(), release.program_id);
            let price_policy = artifact(ArtifactKind::PriceMeasurePolicyV1, bundle_value.price_measure_policy_id.bytes(), release.program_id);
            let mut roles = vec![selection.account, replay, selection_fresh, operator, system,
                rent, clock, bundle, basis, price_policy, genesis, grid];
            roles.extend(reservations.iter().copied());
            roles.extend([liveness_policy, candidate, operator, candidate_payer]);
            roles
        }
        DirectMarketAction::SubmitCandidate => {
            vec![selection.account, replay, selection_account, clock, operator, system,
                liveness_policy, candidate]
        }
        DirectMarketAction::BeginVerification | DirectMarketAction::VerifyCandidate => {
            vec![selection.account, replay, selection_account, clock,
                liveness_policy, candidate, operator, candidate_payer]
        }
        _ => return Err(DirectOperatorServerErrorV2::Selection),
    };
    let cursor = match action {
        DirectMarketAction::AdmitOrder => {
            let mut values = vec![selection.account, replay, position, position_replay];
            values.extend(reservations.iter().copied());
            values
        }
        DirectMarketAction::CancelOrder => vec![selection.account, replay, position, position_replay, roles[2]],
        DirectMarketAction::FreezeBook => {
            let mut values = vec![selection.account, replay];
            values.extend(reservations.iter().copied());
            values.extend([liveness_policy, candidate]);
            values
        }
        DirectMarketAction::SubmitCandidate
        | DirectMarketAction::BeginVerification
        | DirectMarketAction::VerifyCandidate => {
            vec![selection.account, replay, selection_account, liveness_policy, candidate]
        }
        _ => return Err(DirectOperatorServerErrorV2::Selection),
    };
    Ok((roles, cursor))
}

struct ExactBatchAccountsV2 {
    accounts: BTreeMap<Address, ObservedRpcAccount>,
    absences: BTreeSet<Address>,
}

impl ExactBatchAccountsV2 {
    fn new(values: Vec<FinalizedExactAccountV1>) -> Result<Self> {
        let mut accounts = BTreeMap::new();
        let mut absences = BTreeSet::new();
        for value in values {
            match value {
                FinalizedExactAccountV1::Present(account) => {
                    if accounts.insert(account.address, account).is_some() {
                        return Err(DirectOperatorServerErrorV2::Acquisition);
                    }
                }
                FinalizedExactAccountV1::Absent { address, provenance } => {
                    if !absences.insert(address)
                        || accounts.insert(address, ObservedRpcAccount {
                            address,
                            owner: Address::default(),
                            lamports: 0,
                            executable: false,
                            rent_epoch: 0,
                            data: Vec::new(),
                            provenance,
                        }).is_some()
                    {
                        return Err(DirectOperatorServerErrorV2::Acquisition);
                    }
                }
            }
        }
        Ok(Self { accounts, absences })
    }

    fn slot(&self) -> Result<u64> {
        let mut slots = self.accounts.values().map(|account| account.provenance.slot);
        let slot = slots.next().ok_or(DirectOperatorServerErrorV2::Acquisition)?;
        if slot == 0 || slots.any(|candidate| candidate != slot) {
            Err(DirectOperatorServerErrorV2::Acquisition)
        } else { Ok(slot) }
    }

    fn present(&self, address: Address) -> Result<&ObservedRpcAccount> {
        if self.absences.contains(&address) {
            return Err(DirectOperatorServerErrorV2::MissingChainAccount);
        }
        self.accounts.get(&address).ok_or(DirectOperatorServerErrorV2::MissingChainAccount)
    }

    fn fresh(&self, address: Address) -> Result<&ObservedRpcAccount> {
        self.accounts.get(&address).ok_or(DirectOperatorServerErrorV2::MissingChainAccount)
    }
}

fn current<'a>(
    index: &'a CanonicalAccountIndex,
    selection: &KeeperActionSelection,
    address: Address,
) -> Result<&'a ObservedRpcAccount> {
    let value = index.current(address, RpcCommitment::Finalized)
        .ok_or(DirectOperatorServerErrorV2::MissingChainAccount)?;
    if value.account.provenance.release_key != selection.release_key
        || value.account.provenance.slot != selection.account_slot
    {
        return Err(DirectOperatorServerErrorV2::MissingChainAccount);
    }
    Ok(&value.account)
}

fn artifact(kind: ArtifactKind, id: [u8; 32], program: Address) -> Address {
    pda(&[PRODUCT_ARTIFACT_SEED_V1, &[kind.byte()], &id], program)
}

fn pda(seeds: &[&[u8]], program: Address) -> Address {
    Address::find_program_address(seeds, &program).0
}

fn parse(value: &str) -> Result<Address> {
    Address::from_str(value).map_err(|_| DirectOperatorServerErrorV2::InvalidConfiguration)
}

#[derive(Clone, Copy, Debug, Default)]
struct ServerDirectHashV2;

impl DirectHashBackendV1 for ServerDirectHashV2 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        let mut hash = Sha256::new();
        for part in parts { hash.update(part); }
        hash.finalize().into()
    }
}

impl From<CanonicalActionMaterialErrorV1> for DirectOperatorServerErrorV2 {
    fn from(_: CanonicalActionMaterialErrorV1) -> Self { Self::Material }
}
