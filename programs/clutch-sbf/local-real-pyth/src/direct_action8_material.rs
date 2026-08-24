//! Chain-derived unsigned construction for current Direct action 8.
//!
//! This module deliberately does not consume the generic Direct V1 material
//! builder or a browser-supplied keeper cursor.  It hostile-decodes one
//! finalized `0xb1/v3` root and every body reachable from that root, derives
//! the nonempty/NoCandidate branch, and emits only the physically routed
//! `80/1/8` request.  The output remains an untrusted, blockhash-free operator
//! projection; the SBF handler reloads and authenticates the same accounts.

use crate::action_material::{
    ActionFreshnessBoundaryV1, CanonicalAccountRoleV1, CanonicalActionMaterialErrorV1,
    CanonicalActionMaterialV1,
};
use crate::collateral_release_catalog::{
    AuthenticatedCurrentCollateralReleaseV1, CurrentCollateralReleaseCatalogV1,
    FinalizedCollateralReleaseFrameV1,
};
use crate::rpc_index::{
    finalized_exact_account_snapshot_request_v1,
    CanonicalFamily, CanonicalIntentCoordinate, FinalizedAccountSnapshotV1,
    FinalizedExactAccountSnapshotRequestV1, FinalizedSnapshotReceiptV1,
    IndexedProgramRelease, ObservedRpcAccount, RpcCommitment, RpcIndexPlan,
    RpcObservationSource,
};
use crate::transaction_builder::{
    ExactEquation, IntegerUnit, OwnedInstructionDraft, ProtocolTransactionBuilder, SemanticOwner,
};
use crate::workflow_graph::{
    CanonicalActionCoordinate, ExplicitOperatorReleaseManifest, PlannedWorkflowNode,
    ResumableWorkflowCursor, WorkflowLane, WorkflowPosition,
};
use clutch_collateral_adapter_v2::CollateralPolicyV2;
use clutch_direct_market_runtime::codec_v1::{
    DIRECT_ACTION_REPLAY_BODY_BYTES_V1, DIRECT_RESERVATION_BODY_BYTES_V1,
    DIRECT_SELECTION_BODY_BYTES_V1,
};
use clutch_direct_market_runtime::codec_v3::{
    authenticate_direct_root_transition_body_v3,
    decode_direct_action_replay_body_for_transition_v3,
    decode_direct_reservation_body_for_transition_v3,
    decode_direct_selection_body_for_transition_v3,
    encode_direct_action_replay_body_into_transition_v3,
    encode_direct_reservation_body_into_transition_v3,
    encode_direct_selection_body_into_transition_v3, write_direct_root_transition_body_v3,
    AuthenticatedDirectRootTransitionV3,
};
use clutch_direct_market_runtime::lifecycle_v2::{
    bind_direct_candidate_work_batch_v2, finalize_direct_selection_v2,
    prepare_direct_candidate_work_batch_v2, prepare_direct_economic_terminal_v2,
    AuthenticatedDirectEconomicTerminalV2, DirectRootReplayTransitionV2,
};
use clutch_direct_market_runtime::current_v3::DirectCurrentGeneralAuthorityV2;
use clutch_direct_market_runtime::fee_v2::DirectFeePolicyV2;
use clutch_direct_market_runtime::settlement_v1::DirectEndpointPrestateV1;
use clutch_direct_market_runtime::{
    DirectActionReplayV1, DirectHashBackendV1, DirectMarketActionV1, DirectMarketErrorV1,
    DirectRootPhaseV1, DirectSelectionPhaseV1, DirectSelectionV1, DirectTerminalReasonV1,
};
use clutch_general_v2_contract::{
    project_general_position_replay_prestate_v1, GeneralPositionReplayPrestateV1, Id32,
    MarketBindingV4, MarketRuntimeV3AccountV1, GENERAL_REPLAY_ACCOUNT_V1_BYTES,
    MARKET_BINDING_ACCOUNT_BYTES_V4, MARKET_BINDING_SEED_DOMAIN_V1,
    MARKET_RUNTIME_ACCOUNT_BYTES, MARKET_RUNTIME_SEED_DOMAIN_V1,
};
use clutch_liveness::runtime_adapter_v1::{
    plan_runtime_transition_v1, RuntimePersistedAccountViewV1, RuntimeReceiptKindV1,
    RuntimeReceiptObservationV1, RuntimeTransferRoleV1, RuntimeTransitionActionV1,
    RuntimeTransitionIntentV1,
};
use clutch_liveness::runtime_v1::{
    RuntimeCompartmentKindV1, RuntimeCompartmentV1, RuntimeLivenessPolicyV1,
    RUNTIME_LIVENESS_ACCOUNT_BYTES_V1, RUNTIME_LIVENESS_POLICY_BYTES_V1,
};
use clutch_liveness::Id as LivenessId;
use clutch_owner_settlement::AuthenticatedPositionV3;
use clutch_product_series::{FixedCodec, MarketGenesisProfileV2, MarketInstancePreimageV2};
use clutch_retirement::{
    PositionAccountV3, PositionPurposeV3, PositionV3Sha256Backend, ReplayV3Envelope,
    ReplayV3HashBackend, ReplayV3Lifecycle, POSITION_V3_BYTES, POSITION_V3_PDA_PREFIX,
    PURPOSE_REPLAY_V3_PDA_PREFIX,
};
use clutch_solana_layout::artifact::ArtifactKind;
use clutch_solana_layout::direct_market_v1::{
    DirectActionReplayAccountV1, DirectReservationAccountV1, DirectSelectionAccountV1,
};
use clutch_solana_layout::direct_market_v3::DirectMarketRootAccountV3;
use clutch_solana_layout::registry::{
    DirectMarketAction, DIRECT_ACTION_REPLAY_ACCOUNT_BYTES, DIRECT_MARKET_FAMILY_TAG,
    DIRECT_MARKET_FAMILY_VERSION, DIRECT_MARKET_ROOT_ACCOUNT_BYTES_V3,
    DIRECT_RESERVATION_ACCOUNT_BYTES, DIRECT_SELECTION_ACCOUNT_BYTES,
};
use clutch_solana_layout::{account_len, ProfileAccount, RealmAccount};
use sha2::{Digest, Sha256};
use solana_address::Address;
use solana_instruction::AccountMeta;
use std::collections::BTreeSet;
use std::str::FromStr;

pub type Result<T> = core::result::Result<T, CanonicalActionMaterialErrorV1>;

/// Semantic release row required by the checked operator manifest.
pub const DIRECT_ACTION8_OWNER_PACKAGE_V2: &str = "clutch-direct-market-runtime";
/// Exact current action-8 chain-material schema.
pub const DIRECT_ACTION8_OWNER_SCHEMA_V2: &str =
    "dragons-clutch/direct/finalize-selection-chain-material/v2";
/// Checked bounded lifetime for a finalized action-8 observation. A caller
/// cannot widen this release-owned operator policy.
pub const DIRECT_ACTION8_MAXIMUM_VALIDITY_SLOTS_V2: u64 = 32;

const DIRECT_ACTION8_WORKFLOW_DOMAIN_V2: &[u8] =
    b"dragons-clutch/operator/direct-action8-workflow/v2\0";
const DIRECT_ACTION8_SNAPSHOT_DOMAIN_V2: &[u8] =
    b"dragons-clutch/operator/direct-action8-snapshot/v2\0";
const DIRECT_ACTION8_POSTCONDITION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/operator/direct-action8-execution-clock-postcondition/v2\0";

const SEED_REALM_V1: &[u8] = b"dragons-clutch:realm:v1";
const SEED_PROFILE_V1: &[u8] = b"dragons-clutch:profile:v1";
const SEED_POLICY_V1: &[u8] = b"dragons-clutch:policy:v1";
const SEED_PRODUCT_ARTIFACT_V1: &[u8] = b"dc:product-artifact:v1";
const SEED_DIRECT_ROOT_V3: &[u8] = b"dc:direct-market-root:v3";
const SEED_DIRECT_REPLAY_V1: &[u8] = b"dc:direct-action-replay:v1";
const SEED_DIRECT_SELECTION_V1: &[u8] = b"dc:direct-selection:v1";

/// State-owned action-8 branch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectAction8BranchV2 {
    /// One to three retained candidates were exhaustively verified.
    Nonempty,
    /// The canonical Selection contains no candidate.
    NoCandidate,
}

/// Internal projected postwrite used to derive the typed transition contract.
#[derive(Clone, Debug, Eq, PartialEq)]
struct DirectAction8PostimageV2 {
    account: Address,
    lamports: u64,
    data: Vec<u8>,
}

/// Data predicate realized after the hostile execution Clock is known.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DirectAction8DataPostconditionV2 {
    /// The handler must leave this account's execution-prestate bytes intact.
    Preserved,
    /// The handler must write these exact current semantic bytes.
    Exact(Vec<u8>),
}

/// One realized transition predicate. Lamports are expressed as a signed
/// delta from the actual execution prebalance, never as a guessed absolute
/// balance from the earlier discovery snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectAction8RealizedPostconditionV2 {
    account: Address,
    lamport_delta: i128,
    data: DirectAction8DataPostconditionV2,
}

impl DirectAction8RealizedPostconditionV2 {
    #[must_use]
    pub const fn account(&self) -> Address { self.account }

    #[must_use]
    pub const fn lamport_delta(&self) -> i128 { self.lamport_delta }

    #[must_use]
    pub const fn data(&self) -> &DirectAction8DataPostconditionV2 { &self.data }
}

/// Typed transition contract for postwrites whose exact semantic bytes depend
/// on the hostile execution Clock read by the SBF handler.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectAction8SymbolicPostconditionV2 {
    contract_id: [u8; 32],
    writable_accounts: Vec<Address>,
}

impl DirectAction8SymbolicPostconditionV2 {
    #[must_use]
    pub const fn contract_id(&self) -> [u8; 32] { self.contract_id }

    #[must_use]
    pub fn writable_accounts(&self) -> &[Address] { &self.writable_accounts }
}

/// Exact action-8 artifact bound to prestate, release, roles, and the symbolic
/// execution-Clock transition contract. Exact data successors and lamport
/// deltas are realized only against an execution-context witness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectAction8CanonicalMaterialV2 {
    branch: DirectAction8BranchV2,
    sequence: u64,
    snapshot_receipt_id: [u8; 32],
    driver_data_sha256: [u8; 32],
    driver_lamports: u64,
    dependency_facts: Vec<DependencyFactV2>,
    postcondition: DirectAction8SymbolicPostconditionV2,
    canonical: CanonicalActionMaterialV1,
}

/// Opaque joined owner-scan and exact-address snapshot admitted by the current
/// Direct action-8 planner. Callers cannot assemble this from account DTOs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectAction8FinalizedSnapshotV2 {
    snapshot: FinalizedAccountSnapshotV1,
}

/// Exhaustive current action-8 material set for one finalized snapshot.
/// Operator admission consumes this batch as a whole, so callers cannot select
/// a friendly subset of ready roots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectAction8OperatorBatchV2 {
    snapshot_receipt_id: [u8; 32],
    release_key: String,
    observed_slot: u64,
    valid_before_slot: u64,
    materials: Vec<DirectAction8CanonicalMaterialV2>,
}

impl DirectAction8OperatorBatchV2 {
    #[must_use]
    pub const fn snapshot_receipt_id(&self) -> [u8; 32] { self.snapshot_receipt_id }

    #[must_use]
    pub fn release_key(&self) -> &str { &self.release_key }

    #[must_use]
    pub const fn observed_slot(&self) -> u64 { self.observed_slot }

    #[must_use]
    pub const fn valid_before_slot(&self) -> u64 { self.valid_before_slot }

    #[must_use]
    pub fn materials(&self) -> &[DirectAction8CanonicalMaterialV2] { &self.materials }
}

impl DirectAction8FinalizedSnapshotV2 {
    #[must_use]
    pub const fn receipt(&self) -> &FinalizedSnapshotReceiptV1 {
        self.snapshot.receipt()
    }

    #[must_use]
    pub fn account_count(&self) -> usize { self.snapshot.accounts().len() }
}

/// Admit the exact dependency reread only when it shares the discovery scan's
/// finalized context slot. The owner scan discovers the exhaustive ready-root
/// set; the exact response rereads every selected program and external role at
/// that same slot. A transport must retry rather than join mixed slots.
pub fn join_direct_action8_finalized_snapshots_v2(
    program_scan: &FinalizedAccountSnapshotV1,
    exact_context: Option<&FinalizedAccountSnapshotV1>,
) -> Result<DirectAction8FinalizedSnapshotV2> {
    if program_scan.accounts().iter().any(|account| {
        account.provenance.source != RpcObservationSource::FinalizedScan
    }) {
        return invalid();
    }
    let snapshot = if let Some(exact) = exact_context {
        if exact.receipt().cluster_key() != program_scan.receipt().cluster_key()
            || exact.receipt().release_key() != program_scan.receipt().release_key()
            || exact.receipt().slot() != program_scan.receipt().slot()
            || exact.accounts().iter().any(|account| {
                !matches!(
                    account.provenance.source,
                    RpcObservationSource::FinalizedExactAccountSnapshot { .. }
                )
            })
        {
            return invalid();
        }
        let program_owner = program_scan
            .accounts()
            .first()
            .map(|account| account.owner)
            .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
        for account in exact.accounts().iter().filter(|account| account.owner == program_owner) {
            let discovered = find_account(program_scan.accounts(), account.address)?;
            if discovered.owner != account.owner
                || discovered.lamports != account.lamports
                || discovered.executable != account.executable
                || discovered.rent_epoch != account.rent_epoch
                || discovered.data != account.data
            {
                return invalid();
            }
        }
        exact.clone()
    } else {
        program_scan.clone()
    };
    require_unique_finalized_snapshot(&snapshot)?;
    Ok(DirectAction8FinalizedSnapshotV2 { snapshot })
}

/// Plan the sole cross-owner acquisition required by all currently-ready
/// Direct action-8 roots in one finalized Clutch owner scan. The address set
/// is derived from hostile-decoded Selection and Candidate bodies: keeper,
/// immutable Candidate payer, sorted unique retained-candidate refund owners,
/// and every program-owned dependency selected by the discovery scan.
pub fn plan_direct_action8_context_snapshot_v2(
    plan: &RpcIndexPlan,
    releases: &[IndexedProgramRelease],
    collateral_catalog: &CurrentCollateralReleaseCatalogV1<'_>,
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    program_scan: &FinalizedAccountSnapshotV1,
    request_id: u64,
) -> Result<Option<FinalizedExactAccountSnapshotRequestV1>> {
    manifest
        .validate()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    if builder.payer() == Address::default()
        || program_scan.accounts().iter().any(|account| {
            account.provenance.source != RpcObservationSource::FinalizedScan
        })
    {
        return invalid();
    }
    require_unique_finalized_snapshot(program_scan)?;
    let release = unique_clutch_snapshot_release(
        releases,
        manifest,
        program_scan.receipt(),
    )?;
    let planned_release = plan
        .release(&release.key())
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    if plan.cluster.key() != program_scan.receipt().cluster_key()
        || planned_release != release
        || builder.clutch_program() != release.program_id
        || builder.clutch_release_sha256() != release.elf_sha256
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidRelease);
    }
    if program_scan.accounts().is_empty() { return Ok(None); }

    let accounts = program_scan.accounts();
    let mut addresses = BTreeSet::new();
    for root_account in current_ready_root_accounts(accounts, release)? {
        let root_frame = DirectMarketRootAccountV3::decode(&root_account.data)
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
        let root = authenticate_direct_root_transition_body_v3(
            root_frame.semantic_body(),
            &OperatorDirectSha256V2,
        )
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
        let selection_account = find_account(
            accounts,
            Address::new_from_array(root.selection_account()),
        )?;
        require_program_state(selection_account, release, DIRECT_SELECTION_ACCOUNT_BYTES)?;
        let selection_frame = decode_selection_frame(selection_account)?;
        let selection = decode_direct_selection_body_for_transition_v3(
            selection_frame.semantic_body(),
            &root,
        )
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
        if !direct_action8_ready(&root, selection, program_scan.receipt().slot()) {
            continue;
        }
        let replay_account = find_account(
            accounts,
            Address::new_from_array(root.action_replay_account()),
        )?;
        let candidate = authenticate_candidate_plane(release, accounts, &root)?;
        for account in [
            root_account,
            replay_account,
            selection_account,
            candidate.policy_account,
            candidate.candidate_account,
        ] {
            addresses.insert(account.address);
        }
        addresses.insert(builder.payer());
        addresses.insert(candidate.payer);
        let mut index = 0u8;
        while index < selection.candidate_count() {
            addresses.insert(Address::new_from_array(
                selection
                    .candidate_submitter(index)
                    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
            ));
            index = index
                .checked_add(1)
                .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
        }
        if selection.candidate_count() == 0 {
            let graph = authenticate_current_general_graph_v2(
                collateral_catalog,
                release,
                accounts,
                &root,
            )?;
            for account in [
                graph.realm_account,
                graph.profile_account,
                graph.policy_account,
                graph.binding_account,
                graph.runtime_account,
                graph.instance_account,
                graph.genesis_account,
            ] {
                addresses.insert(account.address);
            }
            addresses.extend([
                graph.token_program,
                graph.token_program_data,
                graph.token_release_artifact,
            ]);
            let mut endpoint_index = 0u8;
            while endpoint_index < selection.reservation_count() {
                let endpoint = authenticate_current_endpoint_v2(
                    release,
                    accounts,
                    &root,
                    selection,
                    &graph,
                    endpoint_index,
                )?;
                for account in [
                    endpoint.reservation_account,
                    endpoint.position_account,
                    endpoint.replay_account,
                ] {
                    addresses.insert(account.address);
                }
                endpoint_index = endpoint_index
                    .checked_add(1)
                    .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
            }
        }
    }
    if addresses.is_empty() {
        return Ok(None);
    }
    finalized_exact_account_snapshot_request_v1(
        plan,
        &release.key(),
        request_id,
        program_scan.receipt().slot(),
        addresses.into_iter().collect(),
    )
    .map(Some)
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)
}

impl DirectAction8CanonicalMaterialV2 {
    #[must_use]
    pub const fn branch(&self) -> DirectAction8BranchV2 { self.branch }
    #[must_use]
    pub const fn sequence(&self) -> u64 { self.sequence }
    #[must_use]
    pub const fn snapshot_receipt_id(&self) -> [u8; 32] { self.snapshot_receipt_id }
    #[must_use]
    pub const fn postcondition(&self) -> &DirectAction8SymbolicPostconditionV2 {
        &self.postcondition
    }
    #[must_use]
    pub const fn canonical(&self) -> &CanonicalActionMaterialV1 { &self.canonical }
    #[must_use]
    pub fn into_canonical(self) -> CanonicalActionMaterialV1 { self.canonical }

    pub(crate) const fn driver_data_sha256(&self) -> [u8; 32] {
        self.driver_data_sha256
    }

    pub(crate) const fn driver_lamports(&self) -> u64 { self.driver_lamports }

    pub(crate) fn dependency_facts(&self) -> &[DependencyFactV2] {
        &self.dependency_facts
    }
}

/// Allocation-free SHA-256 backend shared by every hostile Direct decoder.
#[derive(Clone, Copy, Debug, Default)]
struct OperatorDirectSha256V2;

impl DirectHashBackendV1 for OperatorDirectSha256V2 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        let mut hash = Sha256::new();
        for part in parts { hash.update(part); }
        hash.finalize().into()
    }
}

impl PositionV3Sha256Backend for OperatorDirectSha256V2 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        DirectHashBackendV1::sha256_parts(self, &[domain, body])
    }
}

impl ReplayV3HashBackend for OperatorDirectSha256V2 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        DirectHashBackendV1::sha256_parts(self, parts)
    }
}

fn invalid<T>() -> Result<T> { Err(CanonicalActionMaterialErrorV1::InvalidPlan) }

#[allow(clippy::too_many_arguments)]
pub(crate) fn construct_direct_action8_material_v2(
    releases: &[IndexedProgramRelease],
    collateral_catalog: &CurrentCollateralReleaseCatalogV1<'_>,
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    direct_root: Address,
    finalized_snapshot: &DirectAction8FinalizedSnapshotV2,
) -> Result<DirectAction8CanonicalMaterialV2> {
    construct_direct_action8_material_at_execution_slot_v2(
        releases,
        collateral_catalog,
        manifest,
        builder,
        direct_root,
        finalized_snapshot,
        finalized_snapshot.receipt().slot(),
        DIRECT_ACTION8_MAXIMUM_VALIDITY_SLOTS_V2,
    )
    .map(|(material, _)| material)
}

/// Realize the typed symbolic transition after an execution Clock slot is
/// known. A confirmation path compares exact semantic successors plus these
/// deltas against the actual execution pre/post witness; discovery balances
/// are never treated as execution prebalances.
#[allow(clippy::too_many_arguments)]
pub fn project_direct_action8_postconditions_for_execution_slot_v2(
    material: &DirectAction8CanonicalMaterialV2,
    releases: &[IndexedProgramRelease],
    collateral_catalog: &CurrentCollateralReleaseCatalogV1<'_>,
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    finalized_snapshot: &DirectAction8FinalizedSnapshotV2,
    execution_slot: u64,
) -> Result<Vec<DirectAction8RealizedPostconditionV2>> {
    let freshness = material.canonical.freshness();
    if finalized_snapshot.receipt().receipt_id() != material.snapshot_receipt_id
        || execution_slot < freshness.observed_slot
        || execution_slot >= freshness.valid_before_slot
    {
        return invalid();
    }
    let (recomputed, postimages) = construct_direct_action8_material_at_execution_slot_v2(
        releases,
        collateral_catalog,
        manifest,
        builder,
        material.canonical.driver_account(),
        finalized_snapshot,
        execution_slot,
        freshness.maximum_validity_slots,
    )?;
    if recomputed != *material { return invalid(); }
    let accounts = finalized_snapshot.snapshot.accounts();
    postimages
        .into_iter()
        .map(|postimage| {
            let before = find_account(accounts, postimage.account)?;
            let lamport_delta = i128::from(postimage.lamports) - i128::from(before.lamports);
            let data = if postimage.data == before.data {
                DirectAction8DataPostconditionV2::Preserved
            } else {
                DirectAction8DataPostconditionV2::Exact(postimage.data)
            };
            Ok(DirectAction8RealizedPostconditionV2 {
                account: postimage.account,
                lamport_delta,
                data,
            })
        })
        .collect()
}

/// Build the sole current Direct action-8 operator artifact from one bounded,
/// finalized, unordered account snapshot. `transition_slot` is used only to
/// derive an internal realization of the symbolic transition; it is never
/// serialized into the empty action-8 payload.
#[allow(clippy::too_many_arguments)]
fn construct_direct_action8_material_at_execution_slot_v2(
    releases: &[IndexedProgramRelease],
    collateral_catalog: &CurrentCollateralReleaseCatalogV1<'_>,
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    direct_root: Address,
    finalized_snapshot: &DirectAction8FinalizedSnapshotV2,
    transition_slot: u64,
    maximum_validity_slots: u64,
) -> Result<(DirectAction8CanonicalMaterialV2, Vec<DirectAction8PostimageV2>)> {
    let finalized_accounts = finalized_snapshot.snapshot.accounts();
    let finalized_receipt = finalized_snapshot.snapshot.receipt();
    manifest.validate().map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    if direct_root == Address::default()
        || finalized_receipt.slot() == 0
        || maximum_validity_slots == 0
        || finalized_accounts.is_empty()
    {
        return invalid();
    }
    require_unique_finalized_snapshot(&finalized_snapshot.snapshot)?;
    let root_account = find_account(finalized_accounts, direct_root)?;
    let release = unique_release_for_account(releases, root_account)?;
    require_clutch_release(release, manifest)?;
    if builder.clutch_program() != release.program_id
        || builder.clutch_release_sha256() != release.elf_sha256
        || builder.payer() == Address::default()
    {
        return Err(CanonicalActionMaterialErrorV1::FeePayerMismatch);
    }
    let coordinate = CanonicalIntentCoordinate {
        family_tag: DIRECT_MARKET_FAMILY_TAG,
        family_version: DIRECT_MARKET_FAMILY_VERSION,
        local_action: DirectMarketAction::FinalizeSelection.tag(),
    };
    if release.enabled_intents.binary_search(&coordinate).is_err()
        || !release.families.contains(&CanonicalFamily::Direct)
    {
        return Err(CanonicalActionMaterialErrorV1::CoordinateDisabled);
    }
    let semantic_owner = unique_direct_action8_owner(manifest)?;
    let root_frame = DirectMarketRootAccountV3::decode(&root_account.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let root_transition = authenticate_direct_root_transition_body_v3(
        root_frame.semantic_body(),
        &OperatorDirectSha256V2,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    require_program_state(root_account, release, DIRECT_MARKET_ROOT_ACCOUNT_BYTES_V3)?;
    require_pda(
        direct_root,
        root_frame.bump(),
        &[
            SEED_DIRECT_ROOT_V3,
            &root_transition.market_instance_id(),
            &root_transition.generation().to_le_bytes(),
        ],
        release.program_id,
    )?;
    if root_transition.direct_root_account() != direct_root.to_bytes()
        || root_transition.selection_account() == [0; 32]
        || root_transition.action_replay_account() == [0; 32]
    {
        return invalid();
    }

    let replay_address = Address::new_from_array(root_transition.action_replay_account());
    let selection_address = Address::new_from_array(root_transition.selection_account());
    let replay_account = find_account(finalized_accounts, replay_address)?;
    let selection_account = find_account(finalized_accounts, selection_address)?;
    require_program_state(replay_account, release, DIRECT_ACTION_REPLAY_ACCOUNT_BYTES)?;
    require_program_state(selection_account, release, DIRECT_SELECTION_ACCOUNT_BYTES)?;
    let replay_frame = decode_replay_frame(replay_account)?;
    let selection_frame = decode_selection_frame(selection_account)?;
    require_pda(
        replay_address,
        replay_frame.bump(),
        &[SEED_DIRECT_REPLAY_V1, direct_root.as_ref()],
        release.program_id,
    )?;
    require_pda(
        selection_address,
        selection_frame.bump(),
        &[SEED_DIRECT_SELECTION_V1, direct_root.as_ref()],
        release.program_id,
    )?;
    let replay = decode_direct_action_replay_body_for_transition_v3(
        replay_frame.semantic_body(),
        &root_transition,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let mut selection = decode_direct_selection_body_for_transition_v3(
        selection_frame.semantic_body(),
        &root_transition,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let sequence = replay.next_action_sequence();
    if sequence == 0
        || selection.account() != selection_address.to_bytes()
        || finalized_receipt.slot() < root_account.provenance.slot
        || !direct_action8_ready(&root_transition, selection, finalized_receipt.slot())
    {
        return invalid();
    }
    let root_rent = root_transition.root_rent();
    let replay_rent = replay.rent();
    let root_floor = root_rent
        .principal_lamports
        .checked_add(root_rent.donation_floor_lamports)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let replay_floor = replay_rent
        .principal_lamports
        .checked_add(replay_rent.donation_floor_lamports)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if root_account.lamports < root_floor || replay_account.lamports < replay_floor {
        return invalid();
    }
    let selection_floor = selection
        .rent()
        .principal_lamports
        .checked_add(selection.rent().donation_floor_lamports)
        .and_then(|value| {
            root_transition
                .outstanding_candidate_bond_lamports(selection)
                .ok()
                .and_then(|bond| value.checked_add(bond))
        })
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if selection_account.lamports < selection_floor { return invalid(); }

    let candidate = authenticate_candidate_plane(
        release,
        finalized_accounts,
        &root_transition,
    )?;
    let branch = if selection.candidate_count() == 0 {
        DirectAction8BranchV2::NoCandidate
    } else {
        DirectAction8BranchV2::Nonempty
    };
    let mut state = DirectRootReplayTransitionV2::authenticate(root_transition, replay)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let prepared = match branch {
        DirectAction8BranchV2::Nonempty => prepare_nonempty(
            release,
            finalized_accounts,
            transition_slot,
            builder.payer(),
            root_account,
            replay_account,
            selection_account,
            root_frame.bump(),
            replay_frame.bump(),
            selection_frame.bump(),
            &mut state,
            &mut selection,
            candidate,
            sequence,
        )?,
        DirectAction8BranchV2::NoCandidate => prepare_no_candidate(
            collateral_catalog,
            release,
            finalized_accounts,
            transition_slot,
            builder.payer(),
            root_account,
            replay_account,
            selection_account,
            root_frame.bump(),
            replay_frame.bump(),
            selection_frame.bump(),
            &mut state,
            selection,
            candidate,
            sequence,
        )?,
    };
    let maximum_boundary = finalized_receipt
        .slot()
        .checked_add(maximum_validity_slots)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidFreshness)?;
    let valid_before_slot = maximum_boundary.min(state.root().schedule().selection_deadline_slot);
    if valid_before_slot <= finalized_receipt.slot() {
        return Err(CanonicalActionMaterialErrorV1::InvalidFreshness);
    }
    let freshness = ActionFreshnessBoundaryV1 {
        observed_slot: finalized_receipt.slot(),
        valid_before_slot,
        maximum_validity_slots,
    };
    let cursor = direct_action8_cursor(
        direct_root,
        state.root().generation(),
        branch,
        sequence,
        finalized_receipt,
        &prepared.dependencies,
    )?;
    let dependency_facts = prepared.dependencies.clone();
    if prepared.metas.len() != prepared.roles.len()
        || prepared
            .metas
            .iter()
            .zip(&prepared.roles)
            .any(|(account, role)| {
                account.pubkey != role.address()
                    || account.is_writable != role.writable()
                    || account.is_signer != role.signer()
            })
    {
        return invalid();
    }
    let draft = OwnedInstructionDraft::enabled_direct_finalize_selection_request_v2(
        semantic_owner,
        release.program_id,
        prepared.metas,
        builder.payer(),
        prepared.equations,
        sequence,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let unsigned_transaction = builder
        .build_atomic(&[draft])
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let planned = PlannedWorkflowNode {
        manifest_sha256: manifest.manifest_sha256,
        cursor,
        coordinate: CanonicalActionCoordinate::Direct(DirectMarketAction::FinalizeSelection),
        unsigned_transaction,
        reload_authoritative_accounts: true,
    };
    let postcondition = symbolic_postcondition(
        branch,
        sequence,
        &prepared.postimages,
    )?;
    let canonical = CanonicalActionMaterialV1::from_chain_derived_direct_v2(
        release,
        coordinate,
        direct_root,
        root_account.provenance.slot,
        cursor,
        freshness,
        builder.payer(),
        prepared.roles,
        planned,
        postcondition.contract_id,
    )?;
    let postimages = prepared.postimages;
    let material = DirectAction8CanonicalMaterialV2 {
        branch,
        sequence,
        snapshot_receipt_id: finalized_receipt.receipt_id(),
        driver_data_sha256: Sha256::digest(&root_account.data).into(),
        driver_lamports: root_account.lamports,
        dependency_facts,
        postcondition,
        canonical,
    };
    Ok((material, postimages))
}

/// Exhaustively enumerate every finalized, currently-ready Direct action-8
/// root in one bounded scan. The scan supplies no root hint: roots are found
/// only by exact current owner/frame bytes, then filtered by hostile-decoded
/// root and Selection phases. Any ready root whose complete dependency graph
/// cannot be reconstructed fails the whole enumeration instead of being
/// silently omitted or allowing a launcher to choose a friendlier subset.
pub fn enumerate_direct_action8_material_v2(
    releases: &[IndexedProgramRelease],
    collateral_catalog: &CurrentCollateralReleaseCatalogV1<'_>,
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    finalized_snapshot: &DirectAction8FinalizedSnapshotV2,
) -> Result<DirectAction8OperatorBatchV2> {
    let finalized_accounts = finalized_snapshot.snapshot.accounts();
    manifest
        .validate()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    require_unique_finalized_snapshot(&finalized_snapshot.snapshot)?;
    let release = unique_clutch_snapshot_release(
        releases,
        manifest,
        finalized_snapshot.snapshot.receipt(),
    )?;
    let roots = current_ready_root_accounts(finalized_accounts, release)?;
    let mut output = Vec::new();
    for root_account in roots {
        let root_frame = DirectMarketRootAccountV3::decode(&root_account.data)
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
        let root = authenticate_direct_root_transition_body_v3(
            root_frame.semantic_body(),
            &OperatorDirectSha256V2,
        )
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
        let selection_account = find_account(
            finalized_accounts,
            Address::new_from_array(root.selection_account()),
        )?;
        require_program_state(selection_account, release, DIRECT_SELECTION_ACCOUNT_BYTES)?;
        let selection_frame = decode_selection_frame(selection_account)?;
        let selection = decode_direct_selection_body_for_transition_v3(
            selection_frame.semantic_body(),
            &root,
        )
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
        if !direct_action8_ready(&root, selection, finalized_snapshot.receipt().slot()) {
            continue;
        }
        output.push(construct_direct_action8_material_v2(
            releases,
            collateral_catalog,
            manifest,
            builder,
            root_account.address,
            finalized_snapshot,
        )?);
    }
    let observed_slot = finalized_snapshot.receipt().slot();
    let valid_before_slot = if output.is_empty() {
        observed_slot
            .checked_add(1)
            .ok_or(CanonicalActionMaterialErrorV1::InvalidFreshness)?
    } else {
        output
            .iter()
            .map(|material| material.canonical.freshness().valid_before_slot)
            .min()
            .ok_or(CanonicalActionMaterialErrorV1::InvalidFreshness)?
    };
    Ok(DirectAction8OperatorBatchV2 {
        snapshot_receipt_id: finalized_snapshot.receipt().receipt_id(),
        release_key: release.key(),
        observed_slot,
        valid_before_slot,
        materials: output,
    })
}

struct PreparedAction8V2 {
    metas: Vec<AccountMeta>,
    roles: Vec<CanonicalAccountRoleV1>,
    equations: Vec<ExactEquation>,
    dependencies: Vec<DependencyFactV2>,
    postimages: Vec<DirectAction8PostimageV2>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct DependencyFactV2 {
    pub(crate) address: [u8; 32],
    pub(crate) owner: [u8; 32],
    pub(crate) data_sha256: [u8; 32],
    pub(crate) lamports: u64,
    pub(crate) slot: u64,
}

fn dependency(account: &ObservedRpcAccount) -> DependencyFactV2 {
    DependencyFactV2 {
        address: account.address.to_bytes(),
        owner: account.owner.to_bytes(),
        data_sha256: Sha256::digest(&account.data).into(),
        lamports: account.lamports,
        slot: account.provenance.slot,
    }
}

fn require_unique_finalized_snapshot(
    finalized_snapshot: &FinalizedAccountSnapshotV1,
) -> Result<()> {
    let accounts = finalized_snapshot.accounts();
    let receipt = finalized_snapshot.receipt();
    let mut addresses = BTreeSet::new();
    for account in accounts {
        if account.address == Address::default()
            || account.provenance.commitment != RpcCommitment::Finalized
            || !matches!(
                account.provenance.source,
                RpcObservationSource::FinalizedScan
                    | RpcObservationSource::FinalizedExactAccountSnapshot { .. }
            )
            || account.provenance.cluster_key != receipt.cluster_key()
            || account.provenance.release_key != receipt.release_key()
            || account.provenance.slot == 0
            || account.provenance.slot > receipt.slot()
            || !addresses.insert(account.address)
        {
            return invalid();
        }
    }
    Ok(())
}

fn find_account(
    accounts: &[ObservedRpcAccount],
    address: Address,
) -> Result<&ObservedRpcAccount> {
    let mut found = accounts.iter().filter(|account| account.address == address);
    let account = found.next().ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if found.next().is_some() { return invalid(); }
    Ok(account)
}

fn unique_release_for_account<'a>(
    releases: &'a [IndexedProgramRelease],
    account: &ObservedRpcAccount,
) -> Result<&'a IndexedProgramRelease> {
    let mut found = releases.iter().filter(|release| {
        release.key() == account.provenance.release_key && release.program_id == account.owner
    });
    let release = found.next().ok_or(CanonicalActionMaterialErrorV1::InvalidRelease)?;
    if found.next().is_some() { return Err(CanonicalActionMaterialErrorV1::InvalidRelease); }
    release.validate().map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    Ok(release)
}

fn unique_clutch_snapshot_release<'a>(
    releases: &'a [IndexedProgramRelease],
    manifest: &ExplicitOperatorReleaseManifest,
    receipt: &FinalizedSnapshotReceiptV1,
) -> Result<&'a IndexedProgramRelease> {
    let mut matching = releases.iter().filter(|release| {
        release.key() == receipt.release_key()
            && release.program_id == manifest.clutch.program_id
            && release.program_data == manifest.clutch.program_data
            && release.deployment_slot == manifest.clutch.deployment_slot
            && release.elf_sha256 == manifest.clutch.elf_sha256
            && release.release_manifest_sha256 == manifest.manifest_sha256
    });
    let release = matching
        .next()
        .ok_or(CanonicalActionMaterialErrorV1::InvalidRelease)?;
    if matching.next().is_some() {
        return Err(CanonicalActionMaterialErrorV1::InvalidRelease);
    }
    release
        .validate()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    require_clutch_release(release, manifest)?;
    Ok(release)
}

fn current_ready_root_accounts<'a>(
    accounts: &'a [ObservedRpcAccount],
    release: &IndexedProgramRelease,
) -> Result<Vec<&'a ObservedRpcAccount>> {
    let mut roots = accounts
        .iter()
        .filter(|account| {
            account.owner == release.program_id
                && account.provenance.release_key == release.key()
                && account.data.len() == DIRECT_MARKET_ROOT_ACCOUNT_BYTES_V3
                && account.data.first()
                    == Some(&clutch_solana_layout::registry::DIRECT_MARKET_ROOT_ACCOUNT_TAG)
                && account.data.get(1)
                    == Some(
                        &clutch_solana_layout::registry::DIRECT_MARKET_ROOT_ACCOUNT_VERSION_V3,
                    )
        })
        .collect::<Vec<_>>();
    roots.sort_by_key(|account| account.address);
    if roots.windows(2).any(|pair| pair[0].address == pair[1].address) {
        return invalid();
    }
    Ok(roots)
}

fn direct_action8_ready(
    root: &AuthenticatedDirectRootTransitionV3,
    selection: DirectSelectionV1,
    observed_slot: u64,
) -> bool {
    if observed_slot == 0 || observed_slot >= root.schedule().selection_deadline_slot {
        return false;
    }
    match (root.phase(), selection.phase()) {
        (DirectRootPhaseV1::Verifying, DirectSelectionPhaseV1::Verifying) => {
            (selection.candidate_count() == 0
                && selection.verification_cursor() == 0
                && selection.reservation_count() == 2)
                || (selection.candidate_count() != 0
                    && selection.verification_cursor() == selection.candidate_count())
        }
        _ => false,
    }
}

fn require_clutch_release(
    release: &IndexedProgramRelease,
    manifest: &ExplicitOperatorReleaseManifest,
) -> Result<()> {
    if release.program_id != manifest.clutch.program_id
        || release.program_data != manifest.clutch.program_data
        || release.deployment_slot != manifest.clutch.deployment_slot
        || release.elf_sha256 != manifest.clutch.elf_sha256
        || release.release_manifest_sha256 != manifest.manifest_sha256
    {
        return Err(CanonicalActionMaterialErrorV1::ReleaseMismatch);
    }
    Ok(())
}

fn unique_direct_action8_owner(
    manifest: &ExplicitOperatorReleaseManifest,
) -> Result<SemanticOwner> {
    let mut found = manifest.semantic_releases.iter().filter(|owner| {
        owner.package == DIRECT_ACTION8_OWNER_PACKAGE_V2
            && owner.schema == DIRECT_ACTION8_OWNER_SCHEMA_V2
    });
    let owner = found.next().ok_or(CanonicalActionMaterialErrorV1::InvalidRelease)?;
    if found.next().is_some() { return Err(CanonicalActionMaterialErrorV1::InvalidRelease); }
    Ok(owner.clone())
}

fn require_program_state(
    account: &ObservedRpcAccount,
    release: &IndexedProgramRelease,
    exact_bytes: usize,
) -> Result<()> {
    if account.owner != release.program_id
        || account.provenance.release_key != release.key()
        || account.executable
        || account.data.len() != exact_bytes
    {
        return invalid();
    }
    Ok(())
}

fn require_pda(
    observed: Address,
    observed_bump: u8,
    seeds: &[&[u8]],
    program_id: Address,
) -> Result<()> {
    let (expected, bump) = Address::find_program_address(seeds, &program_id);
    if observed != expected || observed_bump != bump { return invalid(); }
    Ok(())
}

fn decode_replay_frame(
    account: &ObservedRpcAccount,
) -> Result<DirectActionReplayAccountV1> {
    let bytes: &[u8; DIRECT_ACTION_REPLAY_ACCOUNT_BYTES] = account
        .data
        .as_slice()
        .try_into()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    DirectActionReplayAccountV1::decode(bytes)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)
}

fn decode_selection_frame(
    account: &ObservedRpcAccount,
) -> Result<DirectSelectionAccountV1> {
    let bytes: &[u8; DIRECT_SELECTION_ACCOUNT_BYTES] = account
        .data
        .as_slice()
        .try_into()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    DirectSelectionAccountV1::decode(bytes)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)
}

fn decode_reservation_frame(
    account: &ObservedRpcAccount,
) -> Result<DirectReservationAccountV1> {
    let bytes: &[u8; DIRECT_RESERVATION_ACCOUNT_BYTES] = account
        .data
        .as_slice()
        .try_into()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    DirectReservationAccountV1::decode(bytes)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)
}

fn role(
    label: &'static str,
    address: Address,
    writable: bool,
    signer: bool,
) -> Result<CanonicalAccountRoleV1> {
    CanonicalAccountRoleV1::new(label, address, writable, signer)
}

fn meta(address: Address, writable: bool, signer: bool) -> AccountMeta {
    if writable {
        AccountMeta::new(address, signer)
    } else {
        AccountMeta::new_readonly(address, signer)
    }
}

fn clock_sysvar() -> Result<Address> {
    Address::from_str("SysvarC1ock11111111111111111111111111111111")
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)
}

fn push_dependency_unique(
    dependencies: &mut Vec<DependencyFactV2>,
    account: &ObservedRpcAccount,
) {
    let fact = dependency(account);
    if !dependencies.iter().any(|existing| existing.address == fact.address) {
        dependencies.push(fact);
    }
}

fn set_postimage(
    postimages: &mut Vec<DirectAction8PostimageV2>,
    account: Address,
    lamports: u64,
    data: Vec<u8>,
) -> Result<()> {
    if account == Address::default() { return invalid(); }
    if let Some(existing) = postimages.iter_mut().find(|value| value.account == account) {
        existing.lamports = lamports;
        existing.data = data;
    } else {
        postimages.push(DirectAction8PostimageV2 { account, lamports, data });
    }
    Ok(())
}

fn observed_post_lamports(
    postimages: &[DirectAction8PostimageV2],
    account: &ObservedRpcAccount,
) -> u64 {
    postimages
        .iter()
        .find(|value| value.account == account.address)
        .map_or(account.lamports, |value| value.lamports)
}

fn sort_postimages(postimages: &mut [DirectAction8PostimageV2]) {
    postimages.sort_by_key(|value| value.account);
}

fn write_current_direct_postimages(
    root_account: &ObservedRpcAccount,
    replay_account: &ObservedRpcAccount,
    selection_account: &ObservedRpcAccount,
    root_bump: u8,
    replay_bump: u8,
    selection_bump: u8,
    state: &DirectRootReplayTransitionV2,
    selection: DirectSelectionV1,
    selection_lamports: u64,
    postimages: &mut Vec<DirectAction8PostimageV2>,
) -> Result<()> {
    let mut root_data = root_account.data.clone();
    write_direct_root_transition_body_v3(
        state.root(),
        &mut root_data[4..],
        &OperatorDirectSha256V2,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if DirectMarketRootAccountV3::decode(&root_data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
        .bump()
        != root_bump
    {
        return invalid();
    }
    let mut replay_data = replay_account.data.clone();
    replay_data[2] = replay_bump;
    encode_direct_action_replay_body_into_transition_v3(
        state.replay(),
        state.root(),
        &mut replay_data[4..],
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let mut selection_data = selection_account.data.clone();
    selection_data[2] = selection_bump;
    encode_direct_selection_body_into_transition_v3(
        selection,
        state.root(),
        &mut selection_data[4..],
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    set_postimage(
        postimages,
        root_account.address,
        root_account.lamports,
        root_data,
    )?;
    set_postimage(
        postimages,
        replay_account.address,
        replay_account.lamports,
        replay_data,
    )?;
    set_postimage(
        postimages,
        selection_account.address,
        selection_lamports,
        selection_data,
    )
}

#[allow(clippy::too_many_arguments)]
fn prepare_nonempty(
    release: &IndexedProgramRelease,
    accounts: &[ObservedRpcAccount],
    observed_slot: u64,
    keeper: Address,
    root_account: &ObservedRpcAccount,
    replay_account: &ObservedRpcAccount,
    selection_account: &ObservedRpcAccount,
    root_bump: u8,
    replay_bump: u8,
    selection_bump: u8,
    state: &mut DirectRootReplayTransitionV2,
    selection: &mut DirectSelectionV1,
    candidate: CandidatePlaneV2<'_>,
    sequence: u64,
) -> Result<PreparedAction8V2> {
    let bond_before = state
        .root()
        .outstanding_candidate_bond_lamports(*selection)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let effects = finalize_direct_selection_v2(
        state,
        selection,
        sequence,
        observed_slot,
        &OperatorDirectSha256V2,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if effects.candidate_bond_movement.is_some() { return invalid(); }
    let refunds = effects
        .candidate_bond_refunds
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if refunds.total_lamports != bond_before
        || refunds.refund_count == 0
        || refunds.refund_count > 3
    {
        return invalid();
    }
    let selection_lamports = selection_account
        .lamports
        .checked_sub(refunds.total_lamports)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if state
        .root()
        .outstanding_candidate_bond_lamports(*selection)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
        != 0
    {
        return invalid();
    }

    let mut metas = vec![
        meta(root_account.address, true, false),
        meta(replay_account.address, true, false),
        meta(selection_account.address, true, false),
        meta(clock_sysvar()?, false, false),
    ];
    let mut roles = vec![
        role("direct-root-v2", root_account.address, true, false)?,
        role("direct-action-replay-v1", replay_account.address, true, false)?,
        role("direct-selection-v1", selection_account.address, true, false)?,
        role("clock-sysvar", clock_sysvar()?, false, false)?,
    ];
    let mut dependencies = Vec::new();
    for account in [root_account, replay_account, selection_account] {
        push_dependency_unique(&mut dependencies, account);
    }
    let mut postimages = Vec::new();
    let mut prior = None;
    let mut index = 0usize;
    while index < usize::from(refunds.refund_count) {
        let refund = refunds.refunds[index]
            .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
        let address = Address::new_from_array(refund.recipient);
        if address == Address::default() || prior.is_some_and(|value| value >= address) {
            return invalid();
        }
        prior = Some(address);
        let observed = find_account(accounts, address)?;
        if observed.executable
            || [root_account.address, replay_account.address, selection_account.address,
                clock_sysvar()?]
                .contains(&address)
        {
            return invalid();
        }
        let credited = observed_post_lamports(&postimages, observed)
            .checked_add(refund.lamports)
            .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
        set_postimage(&mut postimages, address, credited, observed.data.clone())?;
        push_dependency_unique(&mut dependencies, observed);
        metas.push(meta(address, true, false));
        roles.push(role("candidate-bond-refund-owner", address, true, false)?);
        index += 1;
    }
    if refunds.refunds[index..].iter().any(Option::is_some) { return invalid(); }

    let prefix_addresses = metas.iter().map(|value| value.pubkey).collect::<Vec<_>>();
    if prefix_addresses[..4]
        .iter()
        .enumerate()
        .any(|(left, address)| prefix_addresses[..left].contains(address))
        || prefix_addresses
            .iter()
            .any(|address| {
                *address == candidate.policy_account.address
                    || *address == candidate.candidate_account.address
            })
        || prefix_addresses[..4].contains(&keeper)
        || prefix_addresses[..4].contains(&candidate.payer)
        || candidate.policy_account.address == candidate.candidate_account.address
        || candidate.policy_account.address == keeper
        || candidate.policy_account.address == candidate.payer
        || candidate.candidate_account.address == keeper
        || candidate.candidate_account.address == candidate.payer
    {
        return invalid();
    }

    let keeper_account = find_account(accounts, keeper)?;
    let payer_account = find_account(accounts, candidate.payer)?;
    let keeper_balance_before = observed_post_lamports(&postimages, keeper_account);
    let payer_balance_before = if keeper == candidate.payer {
        keeper_balance_before
    } else {
        observed_post_lamports(&postimages, payer_account)
    };
    let work = project_candidate_work_v2(
        release,
        accounts,
        keeper,
        state,
        selection,
        candidate,
        keeper_balance_before,
        payer_balance_before,
    )?;
    for (label, address, writable, signer) in [
        ("candidate-liveness-policy", candidate.policy_account.address, false, false),
        ("candidate-liveness-account", candidate.candidate_account.address, true, false),
        ("keeper", keeper, true, true),
        ("candidate-liveness-payer", candidate.payer, true, false),
    ] {
        metas.push(meta(address, writable, signer));
        roles.push(role(label, address, writable, signer)?);
    }
    for account in [
        candidate.policy_account,
        candidate.candidate_account,
        keeper_account,
        payer_account,
    ] {
        push_dependency_unique(&mut dependencies, account);
    }
    set_postimage(
        &mut postimages,
        candidate.candidate_account.address,
        work.candidate_lamports,
        work.data,
    )?;
    set_postimage(
        &mut postimages,
        keeper,
        work.keeper_lamports,
        keeper_account.data.clone(),
    )?;
    set_postimage(
        &mut postimages,
        candidate.payer,
        work.payer_lamports,
        payer_account.data.clone(),
    )?;
    write_current_direct_postimages(
        root_account,
        replay_account,
        selection_account,
        root_bump,
        replay_bump,
        selection_bump,
        state,
        *selection,
        selection_lamports,
        &mut postimages,
    )?;
    sort_postimages(&mut postimages);
    Ok(PreparedAction8V2 {
        metas,
        roles,
        equations: vec![
            ExactEquation {
                name: "selection-bond-principal-refunded-exactly".to_string(),
                unit: IntegerUnit::Lamports,
                left: u128::from(bond_before),
                right: u128::from(refunds.total_lamports),
            },
            ExactEquation {
                name: "candidate-work-capital-disposition-exactly".to_string(),
                unit: IntegerUnit::Lamports,
                left: u128::from(
                    candidate
                        .candidate_account
                        .lamports
                        .checked_sub(work.candidate_lamports)
                        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?,
                ),
                right: u128::from(
                    work.keeper_payment_lamports
                        .checked_add(work.payer_refund_lamports)
                        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?,
                ),
            },
        ],
        dependencies,
        postimages,
    })
}

#[allow(clippy::too_many_arguments)]
fn prepare_no_candidate(
    collateral_catalog: &CurrentCollateralReleaseCatalogV1<'_>,
    release: &IndexedProgramRelease,
    accounts: &[ObservedRpcAccount],
    observed_slot: u64,
    keeper: Address,
    root_account: &ObservedRpcAccount,
    replay_account: &ObservedRpcAccount,
    selection_account: &ObservedRpcAccount,
    root_bump: u8,
    replay_bump: u8,
    selection_bump: u8,
    state: &mut DirectRootReplayTransitionV2,
    selection: DirectSelectionV1,
    candidate: CandidatePlaneV2<'_>,
    sequence: u64,
) -> Result<PreparedAction8V2> {
    if selection.candidate_count() != 0 || selection.reservation_count() != 2 {
        return invalid();
    }
    let bond_before = state
        .root()
        .outstanding_candidate_bond_lamports(selection)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if bond_before != 0 { return invalid(); }
    let graph = authenticate_current_general_graph_v2(
        collateral_catalog,
        release,
        accounts,
        state.root(),
    )?;
    let token_program_account = find_account(accounts, graph.token_program)?;
    let token_programdata_account = find_account(accounts, graph.token_program_data)?;
    let token_release_artifact_account = find_account(accounts, graph.token_release_artifact)?;
    let policy = CollateralPolicyV2::decode(&graph.policy_account.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let selected = collateral_catalog
        .select(release, policy)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    let refreshed = AuthenticatedCurrentCollateralReleaseV1::authenticate(
        selected.entry().adapter(),
        selected.entry().program(),
        release.program_id,
        FinalizedCollateralReleaseFrameV1 {
            release_artifact: token_release_artifact_account,
            program: token_program_account,
            programdata: token_programdata_account,
        },
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    refreshed
        .select_for(release, policy)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    let mut endpoint_receipts: [Option<CurrentEndpointV2<'_>>; 2] = [None, None];
    let mut endpoints = [None; 2];
    let endpoint_count = usize::from(selection.reservation_count());
    let mut index = 0usize;
    while index < endpoint_count {
        let endpoint = authenticate_current_endpoint_v2(
            release,
            accounts,
            state.root(),
            selection,
            &graph,
            u8::try_from(index).map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
        )?;
        endpoints[index] = Some(endpoint.prestate);
        endpoint_receipts[index] = Some(endpoint);
        index += 1;
    }
    let authority = OperatorNoCandidateAuthorityV2 {
        root_semantic_id: state.root().root_semantic_id(),
        replay: state.replay(),
        selection: &selection,
        endpoints: &endpoints,
        fee_policy: state.root().fee_policy(),
        realm: state.root().realm_id(),
        sequence,
        slot: observed_slot,
    };
    let realm = state.root().realm_id();
    let plan = prepare_direct_economic_terminal_v2(
        &authority,
        state,
        selection,
        endpoints,
        realm,
        None,
        None,
        None,
        DirectTerminalReasonV1::NoCandidate,
        sequence,
        observed_slot,
        &OperatorDirectSha256V2,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if plan.endpoint_count != selection.reservation_count()
        || plan.candidate_bond_refunds.is_some()
    {
        return invalid();
    }

    let clock = clock_sysvar()?;
    let fixed = [
        ("direct-root-v2", root_account.address, true, false),
        ("direct-action-replay-v1", replay_account.address, true, false),
        ("direct-selection-v1", selection_account.address, true, false),
        ("realm-v1", graph.realm_account.address, false, false),
        ("collateral-profile-v2", graph.profile_account.address, false, false),
        ("collateral-policy-v2", graph.policy_account.address, false, false),
        ("collateral-token-program", graph.token_program, false, false),
        ("general-market-binding-v4", graph.binding_account.address, false, false),
        ("general-market-runtime-v3", graph.runtime_account.address, false, false),
        ("market-instance-v2-artifact", graph.instance_account.address, false, false),
        ("market-genesis-profile-v2-artifact", graph.genesis_account.address, false, false),
        ("clock-sysvar", clock, false, false),
    ];
    let mut distinct = BTreeSet::new();
    if fixed.iter().any(|(_, address, _, _)| {
        *address == Address::default() || !distinct.insert(*address)
    }) {
        return invalid();
    }
    let mut metas = Vec::new();
    let mut roles = Vec::new();
    for (label, address, writable, signer) in fixed {
        metas.push(meta(address, writable, signer));
        roles.push(role(label, address, writable, signer)?);
    }
    let mut dependencies = Vec::new();
    for account in [
        root_account,
        replay_account,
        selection_account,
        graph.realm_account,
        graph.profile_account,
        graph.policy_account,
        graph.binding_account,
        graph.runtime_account,
        graph.instance_account,
        graph.genesis_account,
    ] {
        push_dependency_unique(&mut dependencies, account);
    }
    for account in [
        token_release_artifact_account,
        token_program_account,
        token_programdata_account,
    ] {
        push_dependency_unique(&mut dependencies, account);
    }
    let mut first_position = None;
    let mut first_replay = None;
    index = 0;
    while index < endpoint_count {
        let endpoint = endpoint_receipts[index]
            .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
        let triple = [
            endpoint.reservation_account.address,
            endpoint.position_account.address,
            endpoint.replay_account.address,
        ];
        if triple.iter().any(|address| fixed.iter().any(|(_, fixed, _, _)| fixed == address))
            || triple[0] == triple[1]
            || triple[0] == triple[2]
            || triple[1] == triple[2]
        {
            return invalid();
        }
        if index == 0 {
            first_position = Some(triple[1]);
            first_replay = Some(triple[2]);
        } else {
            let left = endpoint_receipts[0]
                .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
            let left_reservation = left.reservation_account.address;
            let left_position = left.position_account.address;
            let left_replay = left.replay_account.address;
            if left_reservation == triple[0]
                || (first_position == Some(triple[1])) != (first_replay == Some(triple[2]))
                || left_reservation == triple[1]
                || left_reservation == triple[2]
                || triple[0] == left_position
                || triple[0] == left_replay
                || left_position == triple[2]
                || left_replay == triple[1]
            {
                return invalid();
            }
        }
        distinct.insert(triple[0]);
        distinct.insert(triple[1]);
        distinct.insert(triple[2]);
        for (label, account) in [
            ("direct-reservation-v1", endpoint.reservation_account),
            ("general-position-v3", endpoint.position_account),
            ("general-position-replay-v3", endpoint.replay_account),
        ] {
            metas.push(meta(account.address, true, false));
            roles.push(role(label, account.address, true, false)?);
            push_dependency_unique(&mut dependencies, account);
        }
        index += 1;
    }

    for address in [
        candidate.policy_account.address,
        candidate.candidate_account.address,
        keeper,
        candidate.payer,
    ] {
        if distinct.contains(&address) { return invalid(); }
    }
    if candidate.policy_account.address == candidate.candidate_account.address
        || candidate.policy_account.address == keeper
        || candidate.policy_account.address == candidate.payer
        || candidate.candidate_account.address == keeper
        || candidate.candidate_account.address == candidate.payer
    {
        return invalid();
    }
    let keeper_account = find_account(accounts, keeper)?;
    let payer_account = find_account(accounts, candidate.payer)?;
    let work = project_candidate_work_v2(
        release,
        accounts,
        keeper,
        state,
        &plan.selection,
        candidate,
        keeper_account.lamports,
        if keeper == candidate.payer {
            keeper_account.lamports
        } else {
            payer_account.lamports
        },
    )?;
    for (label, address, writable, signer) in [
        ("candidate-liveness-policy", candidate.policy_account.address, false, false),
        ("candidate-liveness-account", candidate.candidate_account.address, true, false),
        ("keeper", keeper, true, true),
        ("candidate-liveness-payer", candidate.payer, true, false),
    ] {
        metas.push(meta(address, writable, signer));
        roles.push(role(label, address, writable, signer)?);
    }
    for account in [
        candidate.policy_account,
        candidate.candidate_account,
        keeper_account,
        payer_account,
    ] {
        push_dependency_unique(&mut dependencies, account);
    }
    if metas.len() != 22 { return invalid(); }

    let mut postimages = Vec::new();
    index = 0;
    while index < endpoint_count {
        let source = endpoint_receipts[index]
            .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
        let post = plan.endpoints[index]
            .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
        let mut reservation_data = source.reservation_account.data.clone();
        encode_direct_reservation_body_into_transition_v3(
            post.reservation_post,
            state.root(),
            &mut reservation_data[4..],
        )
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
        reservation_data[2] = source.reservation_bump;
        set_postimage(
            &mut postimages,
            source.reservation_account.address,
            source.reservation_account.lamports,
            reservation_data,
        )?;
        let position_data = post
            .position_poststate
            .semantic
            .encode()
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
            .to_vec();
        set_postimage(
            &mut postimages,
            source.position_account.address,
            source.position_account.lamports,
            position_data,
        )?;
        set_postimage(
            &mut postimages,
            source.replay_account.address,
            source.replay_account.lamports,
            post.replay_transition.replay_poststate_body().to_vec(),
        )?;
        index += 1;
    }
    set_postimage(
        &mut postimages,
        candidate.candidate_account.address,
        work.candidate_lamports,
        work.data,
    )?;
    set_postimage(
        &mut postimages,
        keeper,
        work.keeper_lamports,
        keeper_account.data.clone(),
    )?;
    set_postimage(
        &mut postimages,
        candidate.payer,
        work.payer_lamports,
        payer_account.data.clone(),
    )?;
    write_current_direct_postimages(
        root_account,
        replay_account,
        selection_account,
        root_bump,
        replay_bump,
        selection_bump,
        state,
        plan.selection,
        selection_account.lamports,
        &mut postimages,
    )?;
    sort_postimages(&mut postimages);
    Ok(PreparedAction8V2 {
        metas,
        roles,
        equations: vec![
            ExactEquation {
                name: "no-candidate-selection-bond-principal-is-zero".to_string(),
                unit: IntegerUnit::Lamports,
                left: 0,
                right: 0,
            },
            ExactEquation {
                name: "candidate-work-capital-disposition-exactly".to_string(),
                unit: IntegerUnit::Lamports,
                left: u128::from(
                    candidate
                        .candidate_account
                        .lamports
                        .checked_sub(work.candidate_lamports)
                        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?,
                ),
                right: u128::from(
                    work.keeper_payment_lamports
                        .checked_add(work.payer_refund_lamports)
                        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?,
                ),
            },
        ],
        dependencies,
        postimages,
    })
}

#[derive(Clone, Copy)]
struct CandidatePlaneV2<'a> {
    policy_account: &'a ObservedRpcAccount,
    candidate_account: &'a ObservedRpcAccount,
    candidate: RuntimeCompartmentV1,
    payer: Address,
}

struct CandidateWorkProjectionV2 {
    data: Vec<u8>,
    candidate_lamports: u64,
    keeper_lamports: u64,
    payer_lamports: u64,
    keeper_payment_lamports: u64,
    payer_refund_lamports: u64,
}

fn authenticate_candidate_plane<'a>(
    release: &IndexedProgramRelease,
    accounts: &'a [ObservedRpcAccount],
    root: &AuthenticatedDirectRootTransitionV3,
) -> Result<CandidatePlaneV2<'a>> {
    let binding = root.candidate_liveness();
    let policy_address = Address::new_from_array(binding.policy_account);
    let candidate_address = Address::new_from_array(binding.candidate_account);
    let policy_account = find_account(accounts, policy_address)?;
    let candidate_account = find_account(accounts, candidate_address)?;
    require_program_state(policy_account, release, RUNTIME_LIVENESS_POLICY_BYTES_V1)?;
    require_program_state(candidate_account, release, RUNTIME_LIVENESS_ACCOUNT_BYTES_V1)?;
    RuntimeLivenessPolicyV1::decode(&policy_account.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let candidate = RuntimeCompartmentV1::decode(&candidate_account.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let policy_data_id: [u8; 32] = Sha256::digest(&policy_account.data).into();
    if policy_data_id != binding.policy_data_id
        || candidate.kind != RuntimeCompartmentKindV1::Candidate
        || candidate.identity.policy_id.bytes() != root.candidate_liveness_policy_id()
        || candidate.identity.lifecycle_id.bytes() != binding.global_lifecycle_id
        || candidate.identity.account_id.bytes() != binding.candidate_account
        || candidate.identity.owner.bytes() != binding.candidate_semantic_owner
        || candidate.identity.neutral_sink.bytes() != root.neutral_lamport_sink()
        || candidate.identity.generation != binding.candidate_generation
        || candidate.quote_schedule_id.bytes() != binding.candidate_quote_schedule_id
        || candidate.receipt_program_id.bytes() != binding.candidate_receipt_program_id
        || candidate.receipt_program_id.bytes() != release.program_id.to_bytes()
        || candidate_account.lamports
            < candidate
                .expected_account_balance_lamports()
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
    {
        return invalid();
    }
    let payer = Address::new_from_array(candidate.identity.payer.bytes());
    if payer == Address::default() { return invalid(); }
    Ok(CandidatePlaneV2 {
        policy_account,
        candidate_account,
        candidate,
        payer,
    })
}

fn project_candidate_work_v2(
    release: &IndexedProgramRelease,
    accounts: &[ObservedRpcAccount],
    keeper: Address,
    state: &mut DirectRootReplayTransitionV2,
    selection: &DirectSelectionV1,
    candidate_plane: CandidatePlaneV2<'_>,
    keeper_balance_before: u64,
    payer_balance_before: u64,
) -> Result<CandidateWorkProjectionV2> {
    if keeper == Address::default()
        || keeper == candidate_plane.policy_account.address
        || keeper == candidate_plane.candidate_account.address
        || candidate_plane.payer == candidate_plane.policy_account.address
        || candidate_plane.payer == candidate_plane.candidate_account.address
        || state.root().action_replay_account()
            == candidate_plane.policy_account.address.to_bytes()
        || state.root().action_replay_account()
            == candidate_plane.candidate_account.address.to_bytes()
    {
        return invalid();
    }
    let keeper_account = find_account(accounts, keeper)?;
    let payer_account = find_account(accounts, candidate_plane.payer)?;
    if keeper_account.executable || payer_account.executable { return invalid(); }
    let binding = state.root().candidate_liveness();
    let policy = RuntimeLivenessPolicyV1::decode(&candidate_plane.policy_account.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let candidate_pre_data_id: [u8; 32] =
        Sha256::digest(&candidate_plane.candidate_account.data).into();
    if state.replay().candidate_liveness_completed_calls() == 0
        && candidate_pre_data_id != binding.candidate_data_id
    {
        return invalid();
    }
    let batch = prepare_direct_candidate_work_batch_v2(
        state,
        Some(selection),
        DirectMarketActionV1::FinalizeSelection,
        candidate_plane.candidate.completed_calls,
        candidate_plane.candidate.last_work_receipt_id.bytes(),
        candidate_pre_data_id,
        keeper.to_bytes(),
        &OperatorDirectSha256V2,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let mut data = candidate_plane.candidate_account.data.clone();
    let mut candidate_lamports = candidate_plane.candidate_account.lamports;
    let mut keeper_payment_lamports = 0u64;
    let mut payer_refund_lamports = 0u64;
    let expected_program = LivenessId::from_bytes(release.program_id.to_bytes());
    let policy_address = LivenessId::from_bytes(candidate_plane.policy_account.address.to_bytes());
    let mut index = 0u8;
    while index < batch.receipt_count() {
        let receipt = batch
            .receipt(index, binding, &OperatorDirectSha256V2)
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
        let balance_after = candidate_lamports
            .checked_sub(receipt.call_ceiling_lamports())
            .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
        let receipt_id = LivenessId::from_bytes(receipt.receipt_id());
        let intent = RuntimeTransitionIntentV1 {
            action: RuntimeTransitionActionV1::SpendWork,
            kind: RuntimeCompartmentKindV1::Candidate,
            policy_id: LivenessId::from_bytes(state.root().candidate_liveness_policy_id()),
            lifecycle_id: LivenessId::from_bytes(binding.global_lifecycle_id),
            account_id: LivenessId::from_bytes(binding.candidate_account),
            semantic_owner: LivenessId::from_bytes(binding.candidate_semantic_owner),
            quote_schedule_id: LivenessId::from_bytes(binding.candidate_quote_schedule_id),
            receipt_id,
            keeper: LivenessId::from_bytes(keeper.to_bytes()),
            generation: binding.candidate_generation,
            call_ordinal: receipt.call_ordinal(),
            call_ceiling_lamports: receipt.call_ceiling_lamports(),
            keeper_payment_lamports: receipt.keeper_payment_lamports(),
            flags: 0,
        };
        let observation = RuntimeReceiptObservationV1 {
            receipt_account_id: LivenessId::from_bytes(state.root().action_replay_account()),
            receipt_account_owner_program_id: expected_program,
            receipt_id,
            receipt_kind: RuntimeReceiptKindV1::WorkCompleted,
            compartment_kind: RuntimeCompartmentKindV1::Candidate,
            semantic_owner: LivenessId::from_bytes(binding.candidate_semantic_owner),
            lifecycle_id: LivenessId::from_bytes(binding.global_lifecycle_id),
            quote_schedule_id: LivenessId::from_bytes(binding.candidate_quote_schedule_id),
            generation: binding.candidate_generation,
            call_ordinal: receipt.call_ordinal(),
            call_ceiling_lamports: receipt.call_ceiling_lamports(),
        };
        let transition = plan_runtime_transition_v1(
            expected_program,
            policy_address,
            RuntimePersistedAccountViewV1 {
                account_id: policy_address,
                owner_program_id: expected_program,
                lamports: candidate_plane.policy_account.lamports,
                data: &candidate_plane.policy_account.data,
                writable: false,
            },
            RuntimePersistedAccountViewV1 {
                account_id: LivenessId::from_bytes(
                    candidate_plane.candidate_account.address.to_bytes(),
                ),
                owner_program_id: expected_program,
                lamports: candidate_lamports,
                data: &data,
                writable: true,
            },
            intent,
            Some(observation),
            balance_after,
        )
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
        if !transition.write_account_data
            || transition.close_account
            || transition.account_balance_before != candidate_lamports
            || transition.account_balance_after != balance_after
        {
            return invalid();
        }
        for movement in transition.transfers() {
            match movement.role {
                RuntimeTransferRoleV1::KeeperPayment
                    if movement.destination == LivenessId::from_bytes(keeper.to_bytes()) =>
                {
                    keeper_payment_lamports = keeper_payment_lamports
                        .checked_add(movement.lamports)
                        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
                }
                RuntimeTransferRoleV1::PayerWorkRefund
                    if movement.destination
                        == LivenessId::from_bytes(candidate_plane.payer.to_bytes()) =>
                {
                    payer_refund_lamports = payer_refund_lamports
                        .checked_add(movement.lamports)
                        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
                }
                _ => return invalid(),
            }
        }
        data.copy_from_slice(&transition.post_account_data);
        candidate_lamports = balance_after;
        index = index
            .checked_add(1)
            .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    }
    if keeper_payment_lamports != batch.total_keeper_payment_lamports()
        || payer_refund_lamports != batch.total_payer_refund_lamports()
        || candidate_plane
            .candidate_account
            .lamports
            .checked_sub(candidate_lamports)
            != Some(batch.total_call_ceiling_lamports())
    {
        return invalid();
    }
    if keeper == candidate_plane.payer && keeper_balance_before != payer_balance_before {
        return invalid();
    }
    let keeper_lamports = keeper_balance_before
        .checked_add(keeper_payment_lamports)
        .and_then(|balance| {
            if keeper == candidate_plane.payer {
                balance.checked_add(payer_refund_lamports)
            } else {
                Some(balance)
            }
        })
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let payer_lamports = if keeper == candidate_plane.payer {
        keeper_lamports
    } else {
        payer_balance_before
            .checked_add(payer_refund_lamports)
            .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?
    };
    bind_direct_candidate_work_batch_v2(state, batch, &OperatorDirectSha256V2)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let projected = RuntimeCompartmentV1::decode(&data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if projected.completed_calls != batch.completed_calls_after()
        || projected.last_work_receipt_id.bytes() != batch.last_receipt_id()
        || policy.policy_id.bytes() != state.root().candidate_liveness_policy_id()
    {
        return invalid();
    }
    Ok(CandidateWorkProjectionV2 {
        data,
        candidate_lamports,
        keeper_lamports,
        payer_lamports,
        keeper_payment_lamports,
        payer_refund_lamports,
    })
}

struct CurrentGeneralGraphV2<'a> {
    realm_account: &'a ObservedRpcAccount,
    profile_account: &'a ObservedRpcAccount,
    policy_account: &'a ObservedRpcAccount,
    token_program: Address,
    token_program_data: Address,
    token_release_artifact: Address,
    binding_account: &'a ObservedRpcAccount,
    runtime_account: &'a ObservedRpcAccount,
    instance_account: &'a ObservedRpcAccount,
    genesis_account: &'a ObservedRpcAccount,
}

#[derive(Clone, Copy)]
struct CurrentEndpointV2<'a> {
    reservation_account: &'a ObservedRpcAccount,
    position_account: &'a ObservedRpcAccount,
    replay_account: &'a ObservedRpcAccount,
    reservation_bump: u8,
    prestate: DirectEndpointPrestateV1,
}

fn authenticate_current_endpoint_v2<'a>(
    release: &IndexedProgramRelease,
    accounts: &'a [ObservedRpcAccount],
    root: &AuthenticatedDirectRootTransitionV3,
    selection: DirectSelectionV1,
    graph: &CurrentGeneralGraphV2<'_>,
    index: u8,
) -> Result<CurrentEndpointV2<'a>> {
    let reservation_address = Address::new_from_array(
        selection
            .reservation_account(index)
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
    );
    let reservation_account = find_account(accounts, reservation_address)?;
    require_program_state(reservation_account, release, DIRECT_RESERVATION_ACCOUNT_BYTES)?;
    let reservation_frame = decode_reservation_frame(reservation_account)?;
    require_pda(
        reservation_address,
        reservation_frame.bump(),
        &[
            b"dc:direct-reservation:v1",
            &root.direct_root_account(),
            &decode_direct_reservation_body_for_transition_v3(
                reservation_frame.semantic_body(),
                root,
            )
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
            .order_id(),
        ],
        release.program_id,
    )?;
    let reservation = decode_direct_reservation_body_for_transition_v3(
        reservation_frame.semantic_body(),
        root,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let reservation_semantic_id = root
        .child_reservation_semantic_id(reservation, &OperatorDirectSha256V2)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let reservation_floor = reservation
        .rent()
        .principal_lamports
        .checked_add(reservation.rent().donation_floor_lamports)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if reservation.account() != reservation_address.to_bytes()
        || reservation_semantic_id
            != selection
                .reservation_semantic_id(index)
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
        || reservation_account.lamports < reservation_floor
    {
        return invalid();
    }
    let owner = reservation.owner();
    let purpose = [u8::from(PositionPurposeV3::General)];
    let (position_address, _) = Address::find_program_address(
        &[
            POSITION_V3_PDA_PREFIX,
            &root.market_instance_id(),
            &owner,
            &purpose,
            graph.runtime_account.address.as_ref(),
        ],
        &release.program_id,
    );
    let position_account = find_account(accounts, position_address)?;
    require_program_state(position_account, release, POSITION_V3_BYTES)?;
    let position = PositionAccountV3::decode(&position_account.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    require_pda(
        position_address,
        position.stored_bump(),
        &[
            POSITION_V3_PDA_PREFIX,
            &root.market_instance_id(),
            &owner,
            &purpose,
            graph.runtime_account.address.as_ref(),
        ],
        release.program_id,
    )?;
    let replay_address = Address::find_program_address(
        &[
            PURPOSE_REPLAY_V3_PDA_PREFIX,
            position_address.as_ref(),
            &purpose,
            graph.runtime_account.address.as_ref(),
        ],
        &release.program_id,
    )
    .0;
    let replay_account = find_account(accounts, replay_address)?;
    require_program_state(replay_account, release, GENERAL_REPLAY_ACCOUNT_V1_BYTES)?;
    let replay_envelope = ReplayV3Envelope::decode(&replay_account.data, &OperatorDirectSha256V2)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let replay_bump = Address::find_program_address(
        &[
            PURPOSE_REPLAY_V3_PDA_PREFIX,
            position_address.as_ref(),
            &purpose,
            graph.runtime_account.address.as_ref(),
        ],
        &release.program_id,
    )
    .1;
    let fields = position.fields();
    let position_floor = fields
        .rent
        .refundable_live_principal
        .checked_add(fields.rent.permanent_tombstone_principal)
        .and_then(|value| value.checked_add(fields.rent.donation_floor))
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if fields.market_instance_id.bytes() != root.market_instance_id()
        || fields.realm_id.bytes() != root.realm_id()
        || fields.collateral_policy_id.bytes() != root.collateral_policy_id()
        || fields.collateral_release_id.bytes() != root.collateral_release_id()
        || fields.owner.bytes() != owner
        || fields.controller.bytes() != owner
        || fields.replay_account.bytes() != replay_address.to_bytes()
        || fields.purpose != PositionPurposeV3::General
        || fields.purpose_binding_id.bytes() != graph.runtime_account.address.to_bytes()
        || fields.outcome_count != root.outcome_count()
        || position_account.lamports < position_floor
    {
        return invalid();
    }
    let semantic_id = position
        .semantic_id(&OperatorDirectSha256V2)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
        .bytes();
    let authenticated = AuthenticatedPositionV3 {
        account: position_address.to_bytes(),
        general_market_runtime: graph.runtime_account.address.to_bytes(),
        semantic: position,
        semantic_id,
        account_authenticated: true,
        semantic_id_authenticated: true,
        market_binding_authenticated: true,
        writable: true,
    };
    let position_replay = project_general_position_replay_prestate_v1(
        Id32::new(replay_address.to_bytes())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
        replay_bump,
        replay_envelope.header().next_sequence(),
        &replay_account.data,
        authenticated,
        &OperatorDirectSha256V2,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    Ok(CurrentEndpointV2 {
        reservation_account,
        position_account,
        replay_account,
        reservation_bump: reservation_frame.bump(),
        prestate: DirectEndpointPrestateV1 {
            reservation,
            position_replay,
        },
    })
}

fn account_data_identity(
    domain: &[u8],
    account: &ObservedRpcAccount,
) -> Result<[u8; 32]> {
    if domain.is_empty() || account.address == Address::default() || account.data.is_empty() {
        return invalid();
    }
    let id: [u8; 32] = Sha256::new()
        .chain_update(domain)
        .chain_update(account.address.to_bytes())
        .chain_update(&account.data)
        .finalize()
        .into();
    if id == [0; 32] { return invalid(); }
    Ok(id)
}

fn authenticate_current_general_graph_v2<'a>(
    collateral_catalog: &CurrentCollateralReleaseCatalogV1<'_>,
    release: &IndexedProgramRelease,
    accounts: &'a [ObservedRpcAccount],
    root: &AuthenticatedDirectRootTransitionV3,
) -> Result<CurrentGeneralGraphV2<'a>> {
    const BINDING_DATA_DOMAIN: &[u8] =
        b"dragons-clutch/general-market/binding-data/v4\0";
    const RUNTIME_DATA_DOMAIN: &[u8] =
        b"dragons-clutch/general-market/runtime-data/v3\0";

    let realm_address = Address::find_program_address(
        &[SEED_REALM_V1, &root.realm_id()],
        &release.program_id,
    )
    .0;
    let profile_address = Address::find_program_address(
        &[SEED_PROFILE_V1, &root.realm_id(), &root.collateral_profile_id()],
        &release.program_id,
    )
    .0;
    let policy_address = Address::find_program_address(
        &[
            SEED_POLICY_V1,
            &root.collateral_profile_id(),
            &root.collateral_policy_id(),
        ],
        &release.program_id,
    )
    .0;
    let realm_account = find_account(accounts, realm_address)?;
    let profile_account = find_account(accounts, profile_address)?;
    let policy_account = find_account(accounts, policy_address)?;
    require_program_state(realm_account, release, account_len::REALM)?;
    require_program_state(profile_account, release, account_len::PROFILE)?;
    require_program_state(
        policy_account,
        release,
        clutch_collateral_adapter_v2::COLLATERAL_POLICY_V2_BYTES,
    )?;
    let realm = RealmAccount::decode(&realm_account.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let profile = ProfileAccount::decode(&profile_account.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let policy = CollateralPolicyV2::decode(&policy_account.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let policy_id = policy
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if realm.realm.bytes() != root.realm_id()
        || realm.profile.bytes() != root.collateral_profile_id()
        || profile.realm.bytes() != root.realm_id()
        || profile.profile.bytes() != root.collateral_profile_id()
        || profile.collateral_policy_id.bytes() != root.collateral_policy_id()
        || profile.collateral_policy_id.bytes() != policy_id.bytes()
        || profile.adapter_release_id.bytes() != policy.adapter_release.bytes()
        || policy.adapter_release.bytes() != root.collateral_release_id()
    {
        return invalid();
    }
    require_pda(
        realm_address,
        realm.stored_bump,
        &[SEED_REALM_V1, &root.realm_id()],
        release.program_id,
    )?;
    let token_release = collateral_catalog
        .select(release, policy)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidRelease)?;
    let token_program = token_release.entry().program().program_id;

    let binding_address = Address::new_from_array(root.general_market_binding_account());
    let runtime_address = Address::new_from_array(root.general_market_runtime_account());
    let binding_account = find_account(accounts, binding_address)?;
    let runtime_account = find_account(accounts, runtime_address)?;
    require_program_state(binding_account, release, MARKET_BINDING_ACCOUNT_BYTES_V4)?;
    require_program_state(runtime_account, release, MARKET_RUNTIME_ACCOUNT_BYTES)?;
    let binding = MarketBindingV4::decode(&binding_account.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let runtime = MarketRuntimeV3AccountV1::decode(&runtime_account.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let market = binding.base().base();
    require_pda(
        binding_address,
        market.stored_bump,
        &[MARKET_BINDING_SEED_DOMAIN_V1, &root.market_instance_id()],
        release.program_id,
    )?;
    require_pda(
        runtime_address,
        runtime.stored_bump,
        &[MARKET_RUNTIME_SEED_DOMAIN_V1, binding_address.as_ref()],
        release.program_id,
    )?;
    let binding_floor = binding
        .rent()
        .refundable_principal
        .checked_add(binding.rent().donation_floor)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let runtime_floor = runtime
        .rent
        .refundable_principal
        .checked_add(runtime.rent.donation_floor)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if market.market.bytes() != runtime_address.to_bytes()
        || runtime.market_binding.bytes() != binding_address.to_bytes()
        || runtime.market_instance_v2_id.bytes() != root.market_instance_id()
        || market.market_instance_v2_id.bytes() != root.market_instance_id()
        || market.outcome_count != root.outcome_count()
        || market.relation_policy_id.bytes() != root.relation_policy_id()
        || market.price_measure_policy_v1_id.bytes() != root.price_policy_id()
        || market.neutral_sink.bytes() != root.neutral_lamport_sink()
        || market.price_scale != root.price_scale()
        || binding.base().batch_policy_id().bytes()
            != root.fee_policy().batch_policy_id
        || binding.authority().product_generation() != root.generation()
        || binding_account.lamports < binding_floor
        || runtime_account.lamports < runtime_floor
    {
        return invalid();
    }

    let instance_id = root.market_instance_id();
    let instance_address = Address::find_program_address(
        &[SEED_PRODUCT_ARTIFACT_V1, &[ArtifactKind::MarketInstancePreimageV2.byte()], &instance_id],
        &release.program_id,
    )
    .0;
    let instance_account = find_account(accounts, instance_address)?;
    require_program_state(instance_account, release, MarketInstancePreimageV2::ENCODED_LEN)?;
    let instance = MarketInstancePreimageV2::decode(&instance_account.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if instance
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
        .bytes()
        != instance_id
        || policy.admit_market_cap(instance.collateral_cap).is_err()
    {
        return invalid();
    }
    let genesis_id = instance.market_genesis_profile_id.content_id().bytes();
    let genesis_address = Address::find_program_address(
        &[SEED_PRODUCT_ARTIFACT_V1, &[ArtifactKind::MarketGenesisProfileV2.byte()], &genesis_id],
        &release.program_id,
    )
    .0;
    let genesis_account = find_account(accounts, genesis_address)?;
    require_program_state(genesis_account, release, MarketGenesisProfileV2::ENCODED_LEN)?;
    let genesis = MarketGenesisProfileV2::decode(&genesis_account.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if genesis
        .id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
        .content_id()
        .bytes()
        != genesis_id
        || genesis.realm_id.bytes() != root.realm_id()
        || genesis.profile_id.bytes() != root.collateral_profile_id()
        || genesis.relation_policy_id.bytes() != root.relation_policy_id()
        || genesis.fee_policy_id.bytes() != root.fee_policy().revenue_policy_v2_digest
        || genesis.price_measure_policy_id.content_id().bytes() != root.price_policy_id()
        || market.market_genesis_profile_v2_id.bytes() != genesis_id
    {
        return invalid();
    }
    let authority = binding.authority();
    let treasury_replay = Address::find_program_address(
        &[
            PURPOSE_REPLAY_V3_PDA_PREFIX,
            &authority.treasury_position_account().bytes(),
            &[u8::from(PositionPurposeV3::General)],
            runtime_address.as_ref(),
        ],
        &release.program_id,
    )
    .0;
    let direct_general = DirectCurrentGeneralAuthorityV2 {
        general_market_binding_account: binding_address.to_bytes(),
        general_market_binding_v4_data_id: account_data_identity(
            BINDING_DATA_DOMAIN,
            binding_account,
        )?,
        general_market_runtime_account: runtime_address.to_bytes(),
        general_market_runtime_data_id: account_data_identity(
            RUNTIME_DATA_DOMAIN,
            runtime_account,
        )?,
        revenue_policy_record_account: authority.revenue_policy_record_account().bytes(),
        revenue_policy_record_v2_id: authority.revenue_policy_record_v2_id().bytes(),
        revenue_policy_v2_digest: authority.revenue_policy_v2_digest().bytes(),
        treasury_owner: authority.treasury_owner().bytes(),
        treasury_position_derivation_policy_v2_id:
            authority.treasury_position_derivation_policy_v2_id().bytes(),
        treasury_position_account: authority.treasury_position_account().bytes(),
        treasury_replay_account: treasury_replay.to_bytes(),
        treasury_service_ledger_account: authority.treasury_service_ledger_account().bytes(),
    };
    if direct_general
        .semantic_id(&OperatorDirectSha256V2)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
        != root.current_general_authority_id()
        || authority.revenue_policy_v2_digest().bytes()
            != root.fee_policy().revenue_policy_v2_digest
        || authority.revenue_policy_record_v2_id().bytes()
            != root.fee_policy().revenue_policy_record_v2_id
        || authority.treasury_owner().bytes() != root.fee_policy().treasury_owner
        || authority.treasury_position_derivation_policy_v2_id().bytes()
            != root.fee_policy().treasury_position_derivation_policy_v2_id
    {
        return invalid();
    }
    Ok(CurrentGeneralGraphV2 {
        realm_account,
        profile_account,
        policy_account,
        token_program,
        token_program_data: token_release.entry().program().program_data,
        token_release_artifact: token_release.entry().artifact_account(),
        binding_account,
        runtime_account,
        instance_account,
        genesis_account,
    })
}

#[derive(Clone, Copy)]
struct OperatorNoCandidateAuthorityV2<'a> {
    root_semantic_id: [u8; 32],
    replay: DirectActionReplayV1,
    selection: &'a DirectSelectionV1,
    endpoints: &'a [Option<DirectEndpointPrestateV1>; 2],
    fee_policy: DirectFeePolicyV2,
    realm: [u8; 32],
    sequence: u64,
    slot: u64,
}

impl AuthenticatedDirectEconomicTerminalV2 for OperatorNoCandidateAuthorityV2<'_> {
    fn authenticate_terminal_v2(
        &self,
        state: &DirectRootReplayTransitionV2,
        selection: DirectSelectionV1,
        ordered_endpoints: &[Option<DirectEndpointPrestateV1>; 2],
        fee_policy: DirectFeePolicyV2,
        realm: [u8; 32],
        batch_policy: Option<&clutch_batch::FrozenPolicyV1>,
        revenue_policy: Option<
            &clutch_batch_policy_identity::revenue_policy_v2::RevenuePolicyV2,
        >,
        fee_terminal: Option<clutch_direct_market_runtime::fee_v1::DirectFeeTerminalV1>,
        treasury: Option<clutch_direct_market_runtime::settlement_v1::DirectFeeTreasuryPrestateV1>,
        reason: DirectTerminalReasonV1,
        consumed_sequence: u64,
        observed_slot: u64,
    ) -> core::result::Result<(), DirectMarketErrorV1> {
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
            && reason == DirectTerminalReasonV1::NoCandidate
            && consumed_sequence == self.sequence
            && observed_slot == self.slot
        {
            Ok(())
        } else {
            Err(DirectMarketErrorV1::UnauthenticatedAuthority)
        }
    }
}

fn direct_action8_cursor(
    root: Address,
    generation: u64,
    branch: DirectAction8BranchV2,
    sequence: u64,
    finalized_receipt: &FinalizedSnapshotReceiptV1,
    dependencies: &[DependencyFactV2],
) -> Result<ResumableWorkflowCursor> {
    if generation == 0 || sequence == 0 || dependencies.is_empty() { return invalid(); }
    let mut ordered = dependencies.to_vec();
    ordered.sort();
    ordered.dedup();
    if ordered.len() != dependencies.len() { return invalid(); }
    let mut snapshot = Sha256::new();
    snapshot.update(DIRECT_ACTION8_SNAPSHOT_DOMAIN_V2);
    snapshot.update(finalized_receipt.receipt_id());
    snapshot.update(finalized_receipt.slot().to_le_bytes());
    snapshot.update(
        u64::try_from(ordered.len())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
            .to_le_bytes(),
    );
    for fact in &ordered {
        if fact.slot > finalized_receipt.slot() { return invalid(); }
        snapshot.update(fact.address);
        snapshot.update(fact.owner);
        snapshot.update(fact.data_sha256);
        snapshot.update(fact.lamports.to_le_bytes());
        snapshot.update(fact.slot.to_le_bytes());
    }
    let observed_state_sha256: [u8; 32] = snapshot.finalize().into();
    let mut workflow = Sha256::new();
    workflow.update(DIRECT_ACTION8_WORKFLOW_DOMAIN_V2);
    workflow.update(root.to_bytes());
    workflow.update(generation.to_le_bytes());
    workflow.update([match branch {
        DirectAction8BranchV2::Nonempty => 1,
        DirectAction8BranchV2::NoCandidate => 2,
    }]);
    let workflow_id = workflow.finalize().into();
    Ok(ResumableWorkflowCursor {
        workflow_id,
        lane: WorkflowLane::Candidate,
        generation,
        position: WorkflowPosition {
            phase: u16::from(DirectMarketAction::FinalizeSelection.tag()),
            item: sequence,
        },
        observed_state_sha256,
    })
}

fn symbolic_postcondition(
    branch: DirectAction8BranchV2,
    sequence: u64,
    postimages: &[DirectAction8PostimageV2],
) -> Result<DirectAction8SymbolicPostconditionV2> {
    if postimages.is_empty() { return invalid(); }
    let mut prior = None;
    let mut hash = Sha256::new();
    hash.update(DIRECT_ACTION8_POSTCONDITION_DOMAIN_V2);
    hash.update([match branch {
        DirectAction8BranchV2::Nonempty => 1,
        DirectAction8BranchV2::NoCandidate => 2,
    }]);
    hash.update(sequence.to_le_bytes());
    hash.update(
        u64::try_from(postimages.len())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
            .to_le_bytes(),
    );
    let mut writable_accounts = Vec::with_capacity(postimages.len());
    for postimage in postimages {
        if postimage.account == Address::default()
            || prior.is_some_and(|address| address >= postimage.account)
        {
            return invalid();
        }
        prior = Some(postimage.account);
        hash.update(postimage.account.to_bytes());
        writable_accounts.push(postimage.account);
    }
    let contract_id = hash.finalize().into();
    if contract_id == [0; 32] { return invalid(); }
    Ok(DirectAction8SymbolicPostconditionV2 {
        contract_id,
        writable_accounts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn address(byte: u8) -> Address { Address::new_from_array([byte; 32]) }

    #[test]
    fn symbolic_postcondition_refuses_noncanonical_postwrite_order() {
        let ordered = vec![
            DirectAction8PostimageV2 {
                account: address(1),
                lamports: 7,
                data: Vec::new(),
            },
            DirectAction8PostimageV2 {
                account: address(2),
                lamports: 9,
                data: vec![3],
            },
        ];
        assert_ne!(
            symbolic_postcondition(DirectAction8BranchV2::Nonempty, 1, &ordered)
                .unwrap()
                .contract_id(),
            [0; 32],
        );
        let reversed = vec![ordered[1].clone(), ordered[0].clone()];
        assert_eq!(
            symbolic_postcondition(DirectAction8BranchV2::Nonempty, 1, &reversed),
            Err(CanonicalActionMaterialErrorV1::InvalidPlan),
        );
    }

}
