//! Read-only JSON projection for a transport-owned operator daemon.
//!
//! This module deliberately has no socket, RPC, wallet, signer, transaction
//! submission, or mutation endpoint. A host may route HTTP requests here only
//! after it has populated [`CanonicalAccountIndex`] from the bounded acquisition
//! plans in `rpc_index`.

use crate::account_index::{
    CanonicalAccountIndex, CanonicalAccountKind, DecodeState, IndexedAccountVersion, IndexedBranch,
};
use crate::rpc_index::RpcCommitment;
use crate::rpc_index::public_rpc_endpoint_binding;
use crate::workflow_graph::{ResumableWorkflowCursor, WorkflowLane, WorkflowPosition};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use solana_address::Address;
use std::collections::BTreeSet;
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeeperSelectionError {
    ZeroWorkflow,
    InvalidCapacity,
    IncompleteCanonicalHint,
}

impl core::fmt::Display for KeeperSelectionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::ZeroWorkflow => "keeper selector has a zero workflow identity",
            Self::InvalidCapacity => "keeper selector capacity is invalid",
            Self::IncompleteCanonicalHint => {
                "canonical projection cannot produce an exact resumable cursor"
            }
        })
    }
}

impl std::error::Error for KeeperSelectionError {}

/// Deterministic admission policy for untrusted index projections.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResumableKeeperSelector {
    pub workflow_id: [u8; 32],
    pub maximum_actions: usize,
}

/// One cursor which can be supplied directly to the unsigned workflow graph.
/// The graph must still reload and authenticate every semantic input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeeperActionSelection {
    pub account: Address,
    pub release_key: String,
    pub action: &'static str,
    pub cursor: ResumableWorkflowCursor,
    pub account_slot: u64,
    pub observed_commitment: RpcCommitment,
    pub effective_commitment: RpcCommitment,
    pub branch: IndexedBranch,
    pub dependencies: Vec<Address>,
}

impl ResumableKeeperSelector {
    pub fn validate(self) -> Result<(), KeeperSelectionError> {
        if self.workflow_id == [0; 32] {
            return Err(KeeperSelectionError::ZeroWorkflow);
        }
        if self.maximum_actions == 0 || self.maximum_actions > 4_096 {
            return Err(KeeperSelectionError::InvalidCapacity);
        }
        Ok(())
    }

    /// Select stable, bounded cursor candidates. This is scheduling only: a
    /// returned action is never evidence that the onchain transition will pass.
    pub fn select(
        self,
        index: &CanonicalAccountIndex,
        commitment: RpcCommitment,
    ) -> Result<Vec<KeeperActionSelection>, KeeperSelectionError> {
        self.validate()?;
        let mut selections = Vec::new();
        let accounts = index.current_accounts(commitment);
        let frontier = KeeperFrontier::from_accounts(&accounts);
        for &version in &accounts {
            let Some(hint) = version.projection.keeper_hint else {
                continue;
            };
            let Some(lane) = hint.lane else {
                // Some families expose useful operator hints but do not yet
                // have a corresponding lane in the unsigned workflow graph.
                continue;
            };
            if frontier.supersedes(version) {
                continue;
            }
            let generation = resolve_generation(index, version, commitment)
                .ok_or(KeeperSelectionError::IncompleteCanonicalHint)?;
            if generation == 0 {
                return Err(KeeperSelectionError::IncompleteCanonicalHint);
            }
            let position = resolve_position(index, version, commitment, hint.position)
                .ok_or(KeeperSelectionError::IncompleteCanonicalHint)?;
            let dependencies = dependency_versions(&accounts, version);
            let observed_state_sha256 = dependency_digest(commitment, &dependencies);
            if observed_state_sha256 == [0; 32] {
                return Err(KeeperSelectionError::IncompleteCanonicalHint);
            }
            selections.push(KeeperActionSelection {
                account: version.account.address,
                release_key: version.account.provenance.release_key.clone(),
                action: hint.action,
                cursor: ResumableWorkflowCursor {
                    workflow_id: self.workflow_id,
                    lane,
                    generation,
                    position,
                    observed_state_sha256,
                },
                account_slot: version.account.provenance.slot,
                observed_commitment: version.account.provenance.commitment,
                effective_commitment: commitment,
                branch: version.branch.clone(),
                dependencies: dependencies
                    .iter()
                    .map(|dependency| dependency.account.address)
                    .collect(),
            });
        }
        selections.sort_by(|left, right| {
            (
                left.cursor.lane,
                left.cursor.generation,
                left.cursor.position.phase,
                left.cursor.position.item,
                left.account,
                left.action,
            )
                .cmp(&(
                    right.cursor.lane,
                    right.cursor.generation,
                    right.cursor.position.phase,
                    right.cursor.position.item,
                    right.account,
                    right.action,
                ))
        });
        selections.truncate(self.maximum_actions);
        Ok(selections)
    }
}

fn dependency_versions<'a>(
    accounts: &[&'a IndexedAccountVersion],
    driver: &'a IndexedAccountVersion,
) -> Vec<&'a IndexedAccountVersion> {
    let scope = driver.projection.primary_binding;
    let driver_address = driver.account.address.to_bytes();
    accounts
        .iter()
        .copied()
        .filter(|candidate| {
            if candidate.account.address == driver.account.address {
                return true;
            }
            match driver.projection.kind {
                CanonicalAccountKind::GeneralMarketRuntime => false,
                CanonicalAccountKind::PositionV3 => {
                    candidate.projection.kind == CanonicalAccountKind::ReplayV3
                        && candidate.projection.primary_binding == Some(driver_address)
                }
                CanonicalAccountKind::ReplayV3 => {
                    candidate.account.address.to_bytes()
                        == driver.projection.primary_binding.unwrap_or([0; 32])
                }
                _ => scope.is_some_and(|binding| {
                    candidate.account.address.to_bytes() == binding
                        || candidate.projection.primary_binding == Some(binding)
                }),
            }
        })
        .collect()
}

fn dependency_digest(
    commitment: RpcCommitment,
    dependencies: &[&IndexedAccountVersion],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"dragons-clutch/operator-index-observation/v2-lamports-rent-data-length");
    hash.update([match commitment {
        RpcCommitment::Processed => 1,
        RpcCommitment::Finalized => 2,
    }]);
    for dependency in dependencies {
        hash.update(dependency.account.address.to_bytes());
        hash.update(dependency.account.owner.to_bytes());
        hash.update(dependency.account.provenance.slot.to_le_bytes());
        hash.update(dependency.account.provenance.receive_sequence.to_le_bytes());
        hash.update(dependency.account.lamports.to_le_bytes());
        hash.update(dependency.account.rent_epoch.to_le_bytes());
        hash.update(
            u64::try_from(dependency.account.data.len())
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        hash.update(dependency.data_sha256);
        let release = dependency.account.provenance.release_key.as_bytes();
        hash.update(
            u64::try_from(release.len())
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        hash.update(release);
        match &dependency.branch {
            IndexedBranch::FinalizedScan => hash.update([0]),
            IndexedBranch::Processed { blockhash } => {
                hash.update([1]);
                let blockhash = blockhash.as_bytes();
                hash.update(
                    u64::try_from(blockhash.len())
                        .unwrap_or(u64::MAX)
                        .to_le_bytes(),
                );
                hash.update(blockhash);
            }
        }
    }
    hash.finalize().into()
}

#[derive(Default)]
struct KeeperFrontier {
    open_sources: BTreeSet<[u8; 32]>,
}

impl KeeperFrontier {
    fn from_accounts(accounts: &[&IndexedAccountVersion]) -> Self {
        let mut frontier = Self::default();
        for version in accounts {
            match version.projection.kind {
                CanonicalAccountKind::SourceOpenRawPage => {
                    if let Some(binding) = version.projection.primary_binding {
                        frontier.open_sources.insert(binding);
                    }
                }
                _ => {}
            }
        }
        frontier
    }

    fn supersedes(&self, version: &IndexedAccountVersion) -> bool {
        match version.projection.kind {
            CanonicalAccountKind::SourceHead => version
                .projection
                .primary_binding
                .is_some_and(|binding| self.open_sources.contains(&binding)),
            _ => false,
        }
    }
}

fn resolve_generation(
    index: &CanonicalAccountIndex,
    version: &IndexedAccountVersion,
    commitment: RpcCommitment,
) -> Option<u64> {
    version.projection.generation.or_else(|| {
        let epoch = Address::new_from_array(version.projection.primary_binding?);
        let epoch = index.current(epoch, commitment)?;
        (epoch.projection.kind == CanonicalAccountKind::GeneralEpoch)
            .then_some(epoch.projection.generation)
            .flatten()
    })
}

fn resolve_position(
    index: &CanonicalAccountIndex,
    version: &IndexedAccountVersion,
    commitment: RpcCommitment,
    position: WorkflowPosition,
) -> Option<WorkflowPosition> {
    let needs_candidate_ordinal = matches!(
        version.projection.kind,
        CanonicalAccountKind::GeneralCandidateFeed
            | CanonicalAccountKind::GeneralCandidateFeedStage
            | CanonicalAccountKind::GeneralClearWork
    ) && matches!(position.phase, 4 | 5 | 9);
    if !needs_candidate_ordinal {
        return Some(position);
    }
    let node = Address::new_from_array(version.projection.secondary_binding?);
    let node = index.current(node, commitment)?;
    if node.projection.kind != CanonicalAccountKind::GeneralAdmissionNode {
        return None;
    }
    Some(WorkflowPosition {
        phase: position.phase,
        item: node.projection.keeper_hint?.position.item,
    })
}

#[derive(Clone, Debug, PartialEq)]
pub struct OperatorJsonResponse {
    pub status: u16,
    pub body: Value,
}

/// Pure request router for the operator's untrusted read model.
pub struct OperatorJsonApi<'index> {
    index: &'index CanonicalAccountIndex,
    selector: ResumableKeeperSelector,
}

impl<'index> OperatorJsonApi<'index> {
    #[must_use]
    pub const fn new(
        index: &'index CanonicalAccountIndex,
        selector: ResumableKeeperSelector,
    ) -> Self {
        Self { index, selector }
    }

    /// Handle one already-bounded HTTP request target. No endpoint mutates
    /// index state, constructs a signature, or submits a transaction.
    #[must_use]
    pub fn handle(&self, method: &str, target: &str) -> OperatorJsonResponse {
        if method != "GET" {
            return response(405, json!({"error": "read-only API accepts GET only"}));
        }
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        match path {
            "/v1/health" => response(
                200,
                json!({
                    "status": "ready",
                    "cluster": self.index.cluster_key(),
                    "projectionAuthority": "untrusted",
                    "signing": false,
                    "submission": false
                }),
            ),
            "/v1/releases" => self.releases(),
            "/v1/session" if query.is_empty() => self.session(),
            "/v1/accounts" => match commitment_query(query) {
                Ok(commitment) => self.accounts(commitment),
                Err(error) => response(400, json!({"error": error})),
            },
            "/v1/keeper/next" => match commitment_query(query) {
                Ok(commitment) => self.keeper(commitment),
                Err(error) => response(400, json!({"error": error})),
            },
            "/v1/forks" => self.forks(),
            _ if path.starts_with("/v1/accounts/") => {
                let address = path.trim_start_matches("/v1/accounts/");
                match (Address::from_str(address), commitment_query(query)) {
                    (Ok(address), Ok(commitment)) => self.account(address, commitment),
                    (Err(_), _) => response(400, json!({"error": "invalid account address"})),
                    (_, Err(error)) => response(400, json!({"error": error})),
                }
            }
            _ => response(404, json!({"error": "unknown read-only endpoint"})),
        }
    }

    fn releases(&self) -> OperatorJsonResponse {
        let releases: Vec<Value> = self
            .index
            .releases()
            .iter()
            .map(|release| {
                json!({
                    "releaseKey": release.key(),
                    "programId": release.program_id.to_string(),
                    "programData": release.program_data.to_string(),
                    "elfSha256": hex32(release.elf_sha256),
                    "deploymentSlot": release.deployment_slot.to_string(),
                    "releaseManifestSha256": hex32(release.release_manifest_sha256),
                    "capabilityProfileId": hex32(release.capability_profile_id),
                    "sourceCommit": release.source_commit,
                    "enabledIntents": release.enabled_intents.iter().map(|intent| json!({
                        "familyTag": intent.family_tag.to_string(),
                        "familyVersion": intent.family_version.to_string(),
                        "localAction": intent.local_action.to_string()
                    })).collect::<Vec<_>>(),
                    "families": release.families.iter().map(|family| family.name()).collect::<Vec<_>>()
                })
            })
            .collect();
        response(
            200,
            json!({"cluster": self.index.cluster_key(), "authorityEligible": false, "releases": releases}),
        )
    }

    /// Project one restart-safe read-only session identity from canonical,
    /// finalized account decodes. The caller supplies no account roles or
    /// cursors: every persisted identity below is owned by an onchain account
    /// codec or by the immutable checked release/transport binding.
    fn session(&self) -> OperatorJsonResponse {
        let plan = self.index.acquisition_plan();
        let [release] = plan.releases.as_slice() else {
            return response(
                409,
                json!({
                    "schema": "dragons-clutch/operator-read-only-session-unavailable/v1",
                    "status": "unavailable",
                    "reason": "a canonical browser session requires exactly one checked release"
                }),
            );
        };
        let accounts = self.index.current_accounts(RpcCommitment::Finalized);
        let cursors = match self.selector.select(self.index, RpcCommitment::Finalized) {
            Ok(cursors) => cursors,
            Err(error) => {
                return response(
                    409,
                    json!({
                        "schema": "dragons-clutch/operator-read-only-session-unavailable/v1",
                        "status": "unavailable",
                        "reason": error.to_string()
                    }),
                );
            }
        };
        let http = public_rpc_endpoint_binding(&plan.cluster.rpc_http_url);
        let websocket = public_rpc_endpoint_binding(&plan.cluster.rpc_websocket_url);
        let session_id = read_only_session_id(
            &plan.cluster.cluster_name,
            &plan.cluster.genesis_hash,
            self.selector.workflow_id,
            http.binding_sha256,
            websocket.binding_sha256,
            release,
            &accounts,
            &cursors,
        );
        response(
            200,
            json!({
                "schema": "dragons-clutch/operator-read-only-session-manifest/v1",
                "status": "ready",
                "sessionId": hex32(session_id),
                "projectionAuthority": "untrusted-canonical-codec-projection",
                "authorityEligible": false,
                "signing": false,
                "submission": false,
                "commitment": "finalized",
                "transport": {
                    "clusterName": plan.cluster.cluster_name,
                    "genesisHash": plan.cluster.genesis_hash,
                    "clusterKey": plan.cluster.key(),
                    "rpcHttpEndpoint": {
                        "redacted": http.redacted,
                        "bindingSha256": hex32(http.binding_sha256)
                    },
                    "rpcWebsocketEndpoint": {
                        "redacted": websocket.redacted,
                        "bindingSha256": hex32(websocket.binding_sha256)
                    }
                },
                "release": {
                    "releaseKey": release.key(),
                    "programId": release.program_id.to_string(),
                    "programData": release.program_data.to_string(),
                    "deploymentSlot": release.deployment_slot.to_string(),
                    "elfSha256": hex32(release.elf_sha256),
                    "releaseManifestSha256": hex32(release.release_manifest_sha256),
                    "capabilityProfileId": hex32(release.capability_profile_id),
                    "sourceCommit": release.source_commit,
                    "decoderSet": crate::account_index::CANONICAL_ACCOUNT_DECODER_SET
                },
                "canonicalAccounts": accounts.iter().map(|version| session_account_json(version)).collect::<Vec<_>>(),
                "restart": {
                    "semantics": "reload every named account through its canonical codec and reauthenticate all joins before using a cursor",
                    "identitySource": "finalized onchain account bodies plus immutable checked release and RPC bindings",
                    "accountCount": accounts.len().to_string(),
                    "cursorCount": cursors.len().to_string(),
                    "cursors": cursors.iter().map(selection_json).collect::<Vec<_>>()
                }
            }),
        )
    }

    fn accounts(&self, commitment: RpcCommitment) -> OperatorJsonResponse {
        let accounts: Vec<Value> = self
            .index
            .current_accounts(commitment)
            .into_iter()
            .map(|version| account_json(version, commitment))
            .collect();
        response(
            200,
            json!({
                "cluster": self.index.cluster_key(),
                "effectiveCommitment": commitment.name(),
                "finalityDisposition": if commitment == RpcCommitment::Processed { "nonfinal-rollbackable" } else { "finalized-projection" },
                "authorityEligible": false,
                "accounts": accounts
            }),
        )
    }

    fn account(&self, address: Address, commitment: RpcCommitment) -> OperatorJsonResponse {
        match self.index.current(address, commitment) {
            Some(version) => response(200, account_json(version, commitment)),
            None => match self.index.finalized_absence(address) {
                Some(absence) => response(
                    404,
                    json!({
                        "error": "account absent from a later finalized release scan",
                        "releaseKey": absence.release_key.as_str(),
                        "finalizedAbsenceSlot": absence.slot.to_string(),
                        "receiveSequence": absence.receive_sequence.to_string()
                    }),
                ),
                None => response(
                    404,
                    json!({"error": "account not present at requested commitment"}),
                ),
            },
        }
    }

    fn keeper(&self, commitment: RpcCommitment) -> OperatorJsonResponse {
        match self.selector.select(self.index, commitment) {
            Ok(selections) => response(
                200,
                json!({
                    "effectiveCommitment": commitment.name(),
                    "authorityEligible": false,
                    "actions": selections.iter().map(selection_json).collect::<Vec<_>>()
                }),
            ),
            Err(error) => response(409, json!({"error": error.to_string()})),
        }
    }

    fn forks(&self) -> OperatorJsonResponse {
        let nodes: Vec<Value> = self
            .index
            .forks()
            .nodes()
            .into_iter()
            .map(|node| {
                json!({
                    "slot": node.slot.to_string(),
                    "parentSlot": node.parent_slot.to_string(),
                    "blockhash": node.blockhash,
                    "previousBlockhash": node.previous_blockhash,
                    "receiveSequence": node.receive_sequence.to_string()
                })
            })
            .collect();
        let finalized = self
            .index
            .forks()
            .finalized_root()
            .map(|(slot, blockhash)| json!({"slot": slot.to_string(), "blockhash": blockhash}));
        response(
            200,
            json!({
                "finalizedRoot": finalized,
                "authorityEligible": false,
                "processedTopology": true,
                "frozenSlots": self.index.forks().frozen_slots().into_iter().map(|slot| slot.to_string()).collect::<Vec<_>>(),
                "deadSlots": self.index.forks().dead_slots().into_iter().map(|slot| slot.to_string()).collect::<Vec<_>>(),
                "nodes": nodes
            }),
        )
    }
}

fn read_only_session_id(
    cluster_name: &str,
    genesis_hash: &str,
    workflow_id: [u8; 32],
    rpc_http_binding: [u8; 32],
    rpc_websocket_binding: [u8; 32],
    release: &crate::rpc_index::IndexedProgramRelease,
    accounts: &[&IndexedAccountVersion],
    cursors: &[KeeperActionSelection],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"dragons-clutch/operator-read-only-session-manifest/v1");
    hash_text(&mut hash, cluster_name);
    hash_text(&mut hash, genesis_hash);
    hash_text(
        &mut hash,
        crate::account_index::CANONICAL_ACCOUNT_DECODER_SET,
    );
    hash_text(&mut hash, &release.key());
    hash.update(rpc_http_binding);
    hash.update(rpc_websocket_binding);
    hash.update(release.program_id.to_bytes());
    hash.update(release.program_data.to_bytes());
    hash.update(release.deployment_slot.to_le_bytes());
    hash.update(release.elf_sha256);
    hash.update(release.release_manifest_sha256);
    hash.update(release.capability_profile_id);
    hash_text(&mut hash, &release.source_commit);
    hash.update(workflow_id);
    hash.update(u64::try_from(accounts.len()).unwrap_or(u64::MAX).to_le_bytes());
    for version in accounts {
        hash.update(version.account.address.to_bytes());
        hash.update(version.account.owner.to_bytes());
        hash_text(&mut hash, &version.account.provenance.release_key);
        hash.update(version.account.provenance.slot.to_le_bytes());
        hash.update(version.account.provenance.receive_sequence.to_le_bytes());
        hash.update(version.account.lamports.to_le_bytes());
        hash.update(version.account.rent_epoch.to_le_bytes());
        hash.update(
            u64::try_from(version.account.data.len())
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        hash.update(version.data_sha256);
        hash_optional_u8(&mut hash, version.account.data.first().copied());
        hash_optional_u8(&mut hash, version.account.data.get(1).copied());
        hash_text(&mut hash, version.projection.family.name());
        hash_text(&mut hash, version.projection.kind.name());
        hash_optional_u64(&mut hash, version.projection.generation);
        hash_optional_32(&mut hash, version.projection.primary_binding);
        hash_optional_32(&mut hash, version.projection.secondary_binding);
        match version.projection.decode_state {
            DecodeState::Canonical => hash.update([0]),
            DecodeState::RequiresContext(requirement) => {
                hash.update([1]);
                hash_text(&mut hash, requirement);
            }
        }
    }
    hash.update(u64::try_from(cursors.len()).unwrap_or(u64::MAX).to_le_bytes());
    for selection in cursors {
        hash.update(selection.account.to_bytes());
        hash_text(&mut hash, &selection.release_key);
        hash_text(&mut hash, selection.action);
        hash.update(selection.account_slot.to_le_bytes());
        hash.update(selection.cursor.workflow_id);
        hash_text(&mut hash, lane_name(selection.cursor.lane));
        hash.update(selection.cursor.generation.to_le_bytes());
        hash.update(selection.cursor.position.phase.to_le_bytes());
        hash.update(selection.cursor.position.item.to_le_bytes());
        hash.update(selection.cursor.observed_state_sha256);
        hash.update(u64::try_from(selection.dependencies.len()).unwrap_or(u64::MAX).to_le_bytes());
        for dependency in &selection.dependencies {
            hash.update(dependency.to_bytes());
        }
    }
    hash.finalize().into()
}

fn hash_text(hash: &mut Sha256, value: &str) {
    hash.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_le_bytes());
    hash.update(value.as_bytes());
}

fn hash_optional_u64(hash: &mut Sha256, value: Option<u64>) {
    match value {
        Some(value) => {
            hash.update([1]);
            hash.update(value.to_le_bytes());
        }
        None => hash.update([0]),
    }
}

fn hash_optional_u8(hash: &mut Sha256, value: Option<u8>) {
    match value {
        Some(value) => hash.update([1, value]),
        None => hash.update([0]),
    }
}

fn hash_optional_32(hash: &mut Sha256, value: Option<[u8; 32]>) {
    match value {
        Some(value) => {
            hash.update([1]);
            hash.update(value);
        }
        None => hash.update([0]),
    }
}

fn session_account_json(version: &IndexedAccountVersion) -> Value {
    let decode = match version.projection.decode_state {
        DecodeState::Canonical => json!({"status": "canonical"}),
        DecodeState::RequiresContext(requirement) => {
            json!({"status": "requires-context", "requirement": requirement})
        }
    };
    json!({
        "address": version.account.address.to_string(),
        "owner": version.account.owner.to_string(),
        "releaseKey": version.account.provenance.release_key,
        "slot": version.account.provenance.slot.to_string(),
        "receiveSequence": version.account.provenance.receive_sequence.to_string(),
        "lamports": version.account.lamports.to_string(),
        "rentEpoch": version.account.rent_epoch.to_string(),
        "dataBytes": version.account.data.len().to_string(),
        "dataSha256": hex32(version.data_sha256),
        "accountTag": version.account.data.first().copied().map(|value| value.to_string()),
        "accountVersion": version.account.data.get(1).copied().map(|value| value.to_string()),
        "family": version.projection.family.name(),
        "kind": version.projection.kind.name(),
        "decode": decode,
        "generation": version.projection.generation.map(|value| value.to_string()),
        "primaryBinding": version.projection.primary_binding.map(hex32),
        "secondaryBinding": version.projection.secondary_binding.map(hex32)
    })
}

fn commitment_query(query: &str) -> Result<RpcCommitment, &'static str> {
    if query.is_empty() || query == "commitment=finalized" {
        Ok(RpcCommitment::Finalized)
    } else if query == "commitment=processed" {
        Ok(RpcCommitment::Processed)
    } else {
        Err("query must be exactly commitment=finalized or commitment=processed")
    }
}

fn account_json(version: &IndexedAccountVersion, effective: RpcCommitment) -> Value {
    let decode_state = match version.projection.decode_state {
        DecodeState::Canonical => json!({"status": "canonical"}),
        DecodeState::RequiresContext(requirement) => {
            json!({"status": "requires-context", "requirement": requirement})
        }
    };
    json!({
        "address": version.account.address.to_string(),
        "owner": version.account.owner.to_string(),
        "releaseKey": version.account.provenance.release_key,
        "slot": version.account.provenance.slot.to_string(),
        "receiveSequence": version.account.provenance.receive_sequence.to_string(),
        "observedCommitment": version.account.provenance.commitment.name(),
        "effectiveCommitment": effective.name(),
        "finalityDisposition": if effective == RpcCommitment::Processed { "nonfinal-rollbackable" } else { "finalized-projection" },
        "authorityEligible": false,
        "lamports": version.account.lamports.to_string(),
        "rentEpoch": version.account.rent_epoch.to_string(),
        "dataBytes": version.account.data.len().to_string(),
        "dataSha256": hex32(version.data_sha256),
        "accountTag": version.account.data.first().copied().map(|value| value.to_string()),
        "accountVersion": version.account.data.get(1).copied().map(|value| value.to_string()),
        "family": version.projection.family.name(),
        "kind": version.projection.kind.name(),
        "decode": decode_state,
        "generation": version.projection.generation.map(|value| value.to_string()),
        "primaryBinding": version.projection.primary_binding.map(hex32),
        "secondaryBinding": version.projection.secondary_binding.map(hex32),
        "branch": branch_json(&version.branch)
    })
}

fn selection_json(selection: &KeeperActionSelection) -> Value {
    json!({
        "account": selection.account.to_string(),
        "releaseKey": selection.release_key,
        "action": selection.action,
        "accountSlot": selection.account_slot.to_string(),
        "observedCommitment": selection.observed_commitment.name(),
        "effectiveCommitment": selection.effective_commitment.name(),
        "branch": branch_json(&selection.branch),
        "dependencies": selection.dependencies.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "cursor": {
            "workflowId": hex32(selection.cursor.workflow_id),
            "lane": lane_name(selection.cursor.lane),
            "generation": selection.cursor.generation.to_string(),
            "phase": selection.cursor.position.phase.to_string(),
            "item": selection.cursor.position.item.to_string(),
            "observedStateSha256": hex32(selection.cursor.observed_state_sha256)
        }
    })
}

fn branch_json(branch: &IndexedBranch) -> Value {
    match branch {
        IndexedBranch::FinalizedScan => json!({"kind": "finalized-scan"}),
        IndexedBranch::Processed { blockhash } => {
            json!({"kind": "processed-fork", "blockhash": blockhash})
        }
    }
}

const fn lane_name(lane: WorkflowLane) -> &'static str {
    match lane {
        WorkflowLane::Creation => "creation",
        WorkflowLane::SourceCrank => "source-crank",
        WorkflowLane::Candidate => "candidate",
        WorkflowLane::KeeperReceipts => "keeper-receipts",
        WorkflowLane::RecoveryRetirement => "recovery-retirement",
    }
}

fn hex32(bytes: [u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in bytes {
        use core::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn response(status: u16, body: Value) -> OperatorJsonResponse {
    OperatorJsonResponse { status, body }
}

#[cfg(test)]
mod read_only_session_contract_tests {
    use super::*;

    fn optional_u8_digest(value: Option<u8>) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash_optional_u8(&mut hash, value);
        hash.finalize().into()
    }

    #[test]
    fn absent_codec_coordinate_cannot_alias_zero() {
        assert_ne!(optional_u8_digest(None), optional_u8_digest(Some(0)));
    }

    #[test]
    fn length_prefixed_session_text_has_unambiguous_field_boundaries() {
        let digest = |values: &[&str]| {
            let mut hash = Sha256::new();
            for value in values {
                hash_text(&mut hash, value);
            }
            let output: [u8; 32] = hash.finalize().into();
            output
        };
        assert_ne!(digest(&["ab", "c"]), digest(&["a", "bc"]));
    }
}
