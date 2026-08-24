//! Read-only JSON projection for a transport-owned operator daemon.
//!
//! This module deliberately has no socket, RPC, wallet, signer, transaction
//! submission, or mutation endpoint. A host may route HTTP requests here only
//! after it has populated [`CanonicalAccountIndex`] from the bounded acquisition
//! plans in `rpc_index`.

use crate::account_index::{
    CanonicalAccountIndex, CanonicalAccountKind, DecodeState, IndexedAccountVersion, IndexedBranch,
};
use crate::action_material::{
    source_action_from_selection, source_role_label_v2, source_selection_action,
    structured_action_from_selection, structured_selection_action,
    CanonicalActionMaterialV1,
};
use crate::rpc_index::{
    public_rpc_endpoint_binding, CanonicalIntentCoordinate, CanonicalIntentVariantV1,
    IndexedProgramRelease, RpcCommitment,
};
use crate::workflow_graph::{ResumableWorkflowCursor, WorkflowLane, WorkflowPosition};
use crate::transaction_builder::{
    IntegerUnit, ProtocolFlow, RuntimeAdmission, TransactionMessageVersionV1,
};
use clutch_solana_layout::registry::{
    GeneralV2Action, RecurringSeriesAction, RecoveryAction, SourceSeriesAction,
    GENERAL_V2_FAMILY_TAG, GENERAL_V2_FAMILY_VERSION, RECOVERY_FAMILY_TAG,
    RECOVERY_FAMILY_VERSION, SOURCE_SERIES_FAMILY_TAG, SOURCE_SERIES_FAMILY_VERSION,
    STRUCTURED_CLAIM_FAMILY_TAG, STRUCTURED_CLAIM_FAMILY_VERSION,
};
use clutch_solana_layout::source_series::account_contract_v2;
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
            if source_dependency(accounts, driver, candidate) {
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

fn source_dependency(
    accounts: &[&IndexedAccountVersion],
    driver: &IndexedAccountVersion,
    candidate: &IndexedAccountVersion,
) -> bool {
    if !matches!(
        driver.projection.kind,
        CanonicalAccountKind::SourceHead | CanonicalAccountKind::SourceOpenRawPage
    ) {
        return false;
    }
    if candidate.projection.kind == CanonicalAccountKind::SourceLineage
        && candidate.projection.secondary_binding == Some(driver.account.address.to_bytes())
    {
        return true;
    }
    let Some(source_spec_id) = driver.projection.primary_binding else {
        return false;
    };
    let Some(release) = accounts.iter().copied().find(|version| {
        version.projection.kind == CanonicalAccountKind::SourceRelease
            && version.projection.primary_binding == Some(source_spec_id)
    }) else {
        return false;
    };
    let Ok(manifest) = clutch_source_plane_v3_runtime::SourceReleaseManifestV2::decode(
        &release.account.data,
    ) else {
        return false;
    };
    match candidate.projection.kind {
        CanonicalAccountKind::SourceRelease => candidate.account.address == release.account.address,
        CanonicalAccountKind::SourceWorkSchedule => {
            candidate.projection.secondary_binding
                == Some(manifest.base.source_work_schedule_id.bytes())
        }
        CanonicalAccountKind::LivenessPolicy => {
            candidate.projection.primary_binding == Some(manifest.base.liveness_policy_id.bytes())
        }
        CanonicalAccountKind::LivenessCompartment => {
            candidate.account.address.to_bytes()
                == manifest.base.source_compartment_account.bytes()
                && candidate.projection.primary_binding
                    == Some(manifest.base.liveness_policy_id.bytes())
        }
        CanonicalAccountKind::SourceLineage => {
            candidate.projection.primary_binding == Some(source_spec_id)
        }
        _ => false,
    }
}

fn dependency_digest(
    commitment: RpcCommitment,
    dependencies: &[&IndexedAccountVersion],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"dragons-clutch/operator-index-observation/v3-semantic-finalized-state");
    hash.update([match commitment {
        RpcCommitment::Processed => 1,
        RpcCommitment::Finalized => 2,
    }]);
    for dependency in dependencies {
        hash.update(dependency.account.address.to_bytes());
        hash.update(dependency.account.owner.to_bytes());
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
        if commitment == RpcCommitment::Processed {
            hash.update(dependency.account.provenance.slot.to_le_bytes());
            hash.update(dependency.account.provenance.receive_sequence.to_le_bytes());
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
    action_materials: &'index [CanonicalActionMaterialV1],
}

impl<'index> OperatorJsonApi<'index> {
    #[must_use]
    pub const fn new(
        index: &'index CanonicalAccountIndex,
        selector: ResumableKeeperSelector,
    ) -> Self {
        Self {
            index,
            selector,
            action_materials: &[],
        }
    }

    /// Bind opaque server-constructed action material to this read-only
    /// projection. The API still has no signing or submission method.
    #[must_use]
    pub const fn with_action_materials(
        index: &'index CanonicalAccountIndex,
        selector: ResumableKeeperSelector,
        action_materials: &'index [CanonicalActionMaterialV1],
    ) -> Self {
        Self {
            index,
            selector,
            action_materials,
        }
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
            "/v1/actions" if query.is_empty() => self.actions(),
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
                    "enabledIntentVariants": release.enabled_intent_variants.iter().map(|variant| json!({
                        "familyTag": variant.coordinate().family_tag.to_string(),
                        "familyVersion": variant.coordinate().family_version.to_string(),
                        "localAction": variant.coordinate().local_action.to_string(),
                        "payloadDiscriminator": variant.payload_discriminator().to_string(),
                        "name": variant.name()
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
                    "decoderSet": crate::account_index::CANONICAL_ACCOUNT_DECODER_SET,
                    "enabledIntents": release.enabled_intents.iter().map(|intent| json!({
                        "familyTag": intent.family_tag.to_string(),
                        "familyVersion": intent.family_version.to_string(),
                        "localAction": intent.local_action.to_string()
                    })).collect::<Vec<_>>(),
                    "enabledIntentVariants": release.enabled_intent_variants.iter().map(|variant| json!({
                        "familyTag": variant.coordinate().family_tag.to_string(),
                        "familyVersion": variant.coordinate().family_version.to_string(),
                        "localAction": variant.coordinate().local_action.to_string(),
                        "payloadDiscriminator": variant.payload_discriminator().to_string(),
                        "name": variant.name()
                    })).collect::<Vec<_>>()
                },
                "canonicalAccounts": accounts.iter().map(|version| session_account_json(version)).collect::<Vec<_>>(),
                "restart": {
                    "semantics": "reload every named account through its canonical codec and reauthenticate all joins before using a cursor",
                    "identitySource": "finalized onchain account bodies plus immutable checked release and RPC bindings",
                    "accountCount": accounts.len().to_string(),
                    "cursorCount": cursors.len().to_string(),
                    "cursors": cursors.iter().map(session_selection_json).collect::<Vec<_>>()
                }
            }),
        )
    }

    /// Project release-authenticated action verdicts. An enabled release tuple
    /// and an onchain-derived scheduling cursor are both necessary, but still
    /// insufficient, for callability: exact semantic-owner bytes, every
    /// account-role identity, creation-target prestate, and signer identities
    /// must also be present in one server-constructed transaction draft.
    fn actions(&self) -> OperatorJsonResponse {
        let plan = self.index.acquisition_plan();
        let [release] = plan.releases.as_slice() else {
            return response(
                409,
                json!({
                    "schema": "dragons-clutch/operator-action-capability-unavailable/v1",
                    "status": "unavailable",
                    "reason": "action projection requires exactly one checked release"
                }),
            );
        };
        let cursors = match self.selector.select(self.index, RpcCommitment::Finalized) {
            Ok(cursors) => cursors,
            Err(error) => {
                return response(
                    409,
                    json!({
                        "schema": "dragons-clutch/operator-action-capability-unavailable/v1",
                        "status": "unavailable",
                        "reason": error.to_string()
                    }),
                );
            }
        };
        let accounts = self.index.current_accounts(RpcCommitment::Finalized);
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
        let mut verdicts = release
            .enabled_intents
            .iter()
            .map(|coordinate| {
                action_verdict_json(release, *coordinate, &cursors, self.action_materials)
            })
            .collect::<Vec<_>>();
        verdicts.extend(release.enabled_intent_variants.iter().map(|variant| {
            action_variant_verdict_json(release, *variant, self.action_materials)
        }));
        response(
            200,
            json!({
                "schema": "dragons-clutch/operator-action-capability-set/v1",
                "status": "ready",
                "sessionId": hex32(session_id),
                "commitment": "finalized",
                "releaseKey": release.key(),
                "capabilityProfileId": hex32(release.capability_profile_id),
                "projectionAuthority": "untrusted-release-and-canonical-codec-projection",
                "signing": false,
                "submission": false,
                "freshness": {
                    "recentBlockhash": "absent-by-contract",
                    "feePayer": "must-be-explicit-in-server-constructed-draft",
                    "validBeforeSlot": "must-be-derived-from-a-fresh-clock-observation",
                    "beforeSigning": "reacquire every named account and reject any changed session, cursor, role, balance, owner, executable bit, or data digest",
                    "afterSubmission": "discard the draft and reacquire /v1/session plus /v1/actions; never advance from an expected poststate"
                },
                "actions": verdicts
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
                        "releaseKey": absence.release_key(),
                        "finalizedAbsenceSlot": absence.slot().to_string(),
                        "receiveSequence": absence.receive_sequence().to_string()
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

fn action_verdict_json(
    release: &IndexedProgramRelease,
    coordinate: CanonicalIntentCoordinate,
    cursors: &[KeeperActionSelection],
    action_materials: &[CanonicalActionMaterialV1],
) -> Value {
    let cursor = cursors
        .iter()
        .find(|cursor| action_coordinate(cursor.action) == Some(coordinate));
    let (family, action, semantic_builder) = coordinate_description(coordinate);
    let unresolved_roles = source_action(coordinate)
        .map(|action| {
            let contract = account_contract_v2(action);
            (0..contract.len())
                .filter_map(|index| contract.meta(index).map(|role| (index, role)))
                .map(|(index, role)| {
                    json!({
                        "index": index.to_string(),
                        "role": source_role_label_v2(role.role),
                        "writable": role.writable,
                        "signer": role.signer,
                        "address": null,
                        "identityDisposition": "unresolved-until-semantic-owner-construction"
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let matching_materials = cursor
        .map(|selection| {
            action_materials
                .iter()
                .filter(|material| {
                    material.matches(release, coordinate, selection)
                        || (coordinate.family_tag == STRUCTURED_CLAIM_FAMILY_TAG
                            && coordinate.family_version == STRUCTURED_CLAIM_FAMILY_VERSION
                            && material.matches_structured_driver_release(
                                release,
                                coordinate,
                                selection,
                            ))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let material = match matching_materials.as_slice() {
        [material] => Some(*material),
        _ => None,
    };
    if let (Some(selection), Some(material)) = (cursor, material) {
        return callable_action_verdict_json(
            release,
            coordinate,
            selection,
            material,
            family,
            action,
            semantic_builder,
        );
    }
    let state_reason = if matching_materials.len() > 1 {
        "multiple canonical materials claim the same release/cursor coordinate"
    } else if semantic_builder.is_none() {
        "no reviewed semantic-owner transaction constructor is registered for this release-enabled coordinate"
    } else if cursor.is_none() {
        "no finalized canonical account body presently selects this action"
    } else {
        "semantic-owner transaction material and complete role reacquisition are not yet present"
    };
    json!({
        "coordinate": {
            "familyTag": coordinate.family_tag.to_string(),
            "familyVersion": coordinate.family_version.to_string(),
            "localAction": coordinate.local_action.to_string(),
            "family": family,
            "action": action
        },
        "releaseAdmission": {
            "enabled": true,
            "releaseKey": release.key(),
            "capabilityProfileId": hex32(release.capability_profile_id)
        },
        "stateSelection": cursor.map(selection_json),
        "semanticOwnerConstructor": semantic_builder,
        "accountRoles": unresolved_roles,
        "callable": false,
        "verdict": "unavailable",
        "reason": state_reason,
        "transactionDraft": null,
        "signerRequirements": [],
        "freshnessDisposition": "no draft; no blockhash, signing, or submission is permitted"
    })
}

fn action_variant_verdict_json(
    release: &IndexedProgramRelease,
    variant: CanonicalIntentVariantV1,
    action_materials: &[CanonicalActionMaterialV1],
) -> Value {
    let coordinate = variant.coordinate();
    let matching_materials = action_materials
        .iter()
        .filter(|material| material.matches_variant(release, variant))
        .collect::<Vec<_>>();
    let material = match matching_materials.as_slice() {
        [material] => Some(*material),
        _ => None,
    };
    if let Some(material) = material {
        return callable_action_variant_verdict_json(release, variant, material);
    }
    json!({
        "coordinate": {
            "familyTag": coordinate.family_tag.to_string(),
            "familyVersion": coordinate.family_version.to_string(),
            "localAction": coordinate.local_action.to_string(),
            "family": "dealer",
            "action": "retire"
        },
        "payloadVariant": {
            "discriminator": variant.payload_discriminator().to_string(),
            "name": variant.name()
        },
        "releaseAdmission": {
            "enabled": true,
            "scope": "payload-discriminator-only",
            "coarseCoordinateEnabled": false,
            "releaseKey": release.key(),
            "capabilityProfileId": hex32(release.capability_profile_id)
        },
        "stateSelection": null,
        "semanticOwnerConstructor": "chain-derived-dealer-terminal-v1",
        "accountRoles": [],
        "callable": false,
        "verdict": "unavailable",
        "reason": if matching_materials.len() > 1 {
            "multiple canonical materials claim the same payload-scoped Dealer terminal variant"
        } else {
            "no complete finalized hostile-authenticated Dealer terminal account frame is presently available"
        },
        "transactionDraft": null,
        "signerRequirements": [],
        "freshnessDisposition": "no draft; no blockhash, signing, or submission is permitted"
    })
}

fn callable_action_variant_verdict_json(
    release: &IndexedProgramRelease,
    variant: CanonicalIntentVariantV1,
    material: &CanonicalActionMaterialV1,
) -> Value {
    let coordinate = variant.coordinate();
    let transaction = material.unsigned_transaction();
    let roles = material
        .account_roles()
        .iter()
        .enumerate()
        .map(|(index, role)| json!({
            "index": index.to_string(),
            "role": role.label(),
            "writable": role.writable(),
            "signer": role.signer(),
            "address": role.address().to_string(),
            "identityDisposition": "semantic-owner-derived-and-bound-to-draft"
        }))
        .collect::<Vec<_>>();
    let freshness = material.freshness();
    json!({
        "coordinate": {
            "familyTag": coordinate.family_tag.to_string(),
            "familyVersion": coordinate.family_version.to_string(),
            "localAction": coordinate.local_action.to_string(),
            "family": "dealer",
            "action": "retire"
        },
        "payloadVariant": {
            "discriminator": variant.payload_discriminator().to_string(),
            "name": variant.name()
        },
        "releaseAdmission": {
            "enabled": true,
            "scope": "payload-discriminator-only",
            "coarseCoordinateEnabled": false,
            "releaseKey": release.key(),
            "capabilityProfileId": hex32(release.capability_profile_id)
        },
        "stateSelection": null,
        "semanticOwnerConstructor": "chain-derived-dealer-terminal-v1",
        "accountRoles": roles,
        "callable": true,
        "verdict": "callable-unsigned-draft",
        "reason": "checked payload-scoped release admission and one finalized hostile-authenticated Dealer terminal frame agree",
        "transactionDraft": {
            "schema": crate::action_material::CANONICAL_ACTION_MATERIAL_SCHEMA_V1,
            "draftId": hex32(material.draft_id()),
            "constructionSchema": transaction.schema,
            "driverAccount": material.driver_account().to_string(),
            "driverAccountSlot": material.driver_account_slot().to_string(),
            "authorityStateSha256": hex32(material.authority_state_sha256()),
            "releaseManifestSha256": hex32(material.release_manifest_sha256()),
            "capabilityProfileId": hex32(material.capability_profile_id()),
            "feePayer": material.fee_payer().to_string(),
            "messageVersion": match transaction.message_version {
                TransactionMessageVersionV1::Legacy => "legacy",
                TransactionMessageVersionV1::V0 => "v0",
            },
            "serializedTransactionHex": hex_bytes(&transaction.serialized_transaction),
            "recentBlockhashPresent": transaction.has_recent_blockhash,
            "signed": transaction.signed,
            "submitted": transaction.submitted
        },
        "signerRequirements": transaction.required_signers.iter().map(|signer| json!({
            "address": signer.to_string(),
            "signaturePresent": false,
            "keyAccess": false
        })).collect::<Vec<_>>(),
        "freshness": {
            "observedSlot": freshness.observed_slot.to_string(),
            "validBeforeSlot": freshness.valid_before_slot.to_string(),
            "maximumValiditySlots": freshness.maximum_validity_slots.to_string(),
            "recentBlockhash": "absent-by-contract"
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn callable_action_verdict_json(
    release: &IndexedProgramRelease,
    coordinate: CanonicalIntentCoordinate,
    selection: &KeeperActionSelection,
    material: &CanonicalActionMaterialV1,
    family: &'static str,
    action: &'static str,
    semantic_builder: Option<&'static str>,
) -> Value {
    let transaction = material.unsigned_transaction();
    let roles = material
        .account_roles()
        .iter()
        .enumerate()
        .map(|(index, role)| {
            json!({
                "index": index.to_string(),
                "role": role.label(),
                "writable": role.writable(),
                "signer": role.signer(),
                "address": role.address().to_string(),
                "identityDisposition": "semantic-owner-derived-and-bound-to-draft"
            })
        })
        .collect::<Vec<_>>();
    let signer_requirements = transaction
        .required_signers
        .iter()
        .map(|signer| {
            let mut semantic_roles = material
                .account_roles()
                .iter()
                .filter(|role| role.signer() && role.address() == *signer)
                .map(|role| role.label())
                .collect::<Vec<_>>();
            if *signer == material.fee_payer() {
                semantic_roles.push("transaction-fee-payer");
            }
            json!({
                "address": signer.to_string(),
                "semanticRoles": semantic_roles,
                "signaturePresent": false,
                "keyAccess": false
            })
        })
        .collect::<Vec<_>>();
    let freshness = material.freshness();
    let release_admission = if family == "structured-claim" {
        json!({
            "enabled": true,
            "scope": "structured-composite-wrapper-execution-base-driver-v1",
            "releaseKey": release.key(),
            "executionReleaseKey": material.release_key(),
            "driverReleaseKey": material.driver_release_key(),
            "executionReleaseManifestSha256": hex32(material.release_manifest_sha256()),
            "capabilityProfileId": hex32(material.capability_profile_id())
        })
    } else {
        json!({
            "enabled": true,
            "scope": "single-release-execution-and-driver-v1",
            "releaseKey": release.key(),
            "executionReleaseKey": material.release_key(),
            "driverReleaseKey": material.driver_release_key(),
            "executionReleaseManifestSha256": hex32(material.release_manifest_sha256()),
            "capabilityProfileId": hex32(release.capability_profile_id)
        })
    };
    json!({
        "coordinate": {
            "familyTag": coordinate.family_tag.to_string(),
            "familyVersion": coordinate.family_version.to_string(),
            "localAction": coordinate.local_action.to_string(),
            "family": family,
            "action": action
        },
        "releaseAdmission": release_admission,
        "stateSelection": selection_json(selection),
        "semanticOwnerConstructor": semantic_builder,
        "accountRoles": roles,
        "callable": true,
        "verdict": "callable-unsigned-draft",
        "reason": "checked release, finalized semantic state, exact account roles, and one canonical blockhash-free transaction draft agree",
        "transactionDraft": {
            "schema": crate::action_material::CANONICAL_ACTION_MATERIAL_SCHEMA_V1,
            "draftId": hex32(material.draft_id()),
            "constructionSchema": transaction.schema,
            "driverAccount": material.driver_account().to_string(),
            "driverAccountSlot": material.driver_account_slot().to_string(),
            "driverReleaseKey": material.driver_release_key(),
            "executionReleaseKey": material.release_key(),
            "authorityStateSha256": hex32(material.authority_state_sha256()),
            "releaseManifestSha256": hex32(material.release_manifest_sha256()),
            "capabilityProfileId": hex32(material.capability_profile_id()),
            "feePayer": material.fee_payer().to_string(),
            "messageVersion": match transaction.message_version {
                TransactionMessageVersionV1::Legacy => "legacy",
                TransactionMessageVersionV1::V0 => "v0",
            },
            "addressLookupTables": transaction.address_lookup_tables.iter().map(|lookup| json!({
                "account": lookup.account.to_string(),
                "observedSlot": lookup.observed_slot.to_string(),
                "stateSha256": hex32(lookup.state_sha256),
                "writableAddresses": lookup.writable_addresses.to_string(),
                "readonlyAddresses": lookup.readonly_addresses.to_string()
            })).collect::<Vec<_>>(),
            "recentBlockhash": null,
            "hasRecentBlockhash": transaction.has_recent_blockhash,
            "signed": transaction.signed,
            "submitted": transaction.submitted,
            "serializedTransactionHex": hex_bytes(&transaction.serialized_transaction),
            "serializedBytes": transaction.serialized_transaction.len().to_string(),
            "actions": transaction.actions.iter().cloned().collect::<Vec<_>>(),
            "flows": transaction.flows.iter().map(|flow| protocol_flow_name(*flow)).collect::<Vec<_>>(),
            "semanticOwners": transaction.semantic_owners.iter().map(|owner| json!({
                "package": owner.package.as_str(),
                "schema": owner.schema.as_str(),
                "releaseSha256": hex32(owner.release_sha256)
            })).collect::<Vec<_>>(),
            "registryBindings": transaction.registry_bindings.iter().map(|binding| binding.map(|binding| json!({
                "familyTag": binding.family.tag().to_string(),
                "familyVersion": binding.family.version().to_string(),
                "localAction": binding.local_action.to_string(),
                "allocationStatus": allocation_status_name(binding.family_status),
                "centralAction": binding.central_action.map(|action| action.local_tag().to_string())
            }))).collect::<Vec<_>>(),
            "runtimeAdmissions": transaction.runtime_admissions.iter().map(|admission| runtime_admission_name(*admission)).collect::<Vec<_>>(),
            "exactEquations": transaction.exact_equations.iter().map(|equation| json!({
                "name": equation.name.as_str(),
                "unit": integer_unit_json(equation.unit),
                "left": equation.left.to_string(),
                "right": equation.right.to_string()
            })).collect::<Vec<_>>(),
            "reloadAuthoritativeAccounts": material.reload_authoritative_accounts()
        },
        "signerRequirements": signer_requirements,
        "freshnessDisposition": {
            "observedSlot": freshness.observed_slot.to_string(),
            "validBeforeSlot": freshness.valid_before_slot.to_string(),
            "maximumValiditySlots": freshness.maximum_validity_slots.to_string(),
            "recentBlockhash": "absent; a launcher must reacquire state before adding one",
            "feePayer": "fixed by the semantic payer role; no key was read",
            "beforeSigning": "reacquire the complete named prestate and current slot; discard on any identity, balance, owner, executable-bit, data-digest, cursor, session, or release change",
            "afterSubmission": "discard this draft regardless of outcome; reacquire /v1/session and /v1/actions and decode the authoritative poststate"
        }
    })
}

fn action_coordinate(action: &str) -> Option<CanonicalIntentCoordinate> {
    if let Some(action) = source_action_from_selection(action) {
        return Some(CanonicalIntentCoordinate {
            family_tag: SOURCE_SERIES_FAMILY_TAG,
            family_version: SOURCE_SERIES_FAMILY_VERSION,
            local_action: action.tag(),
        });
    }
    if let Some(action) = structured_action_from_selection(action) {
        return Some(CanonicalIntentCoordinate {
            family_tag: STRUCTURED_CLAIM_FAMILY_TAG,
            family_version: STRUCTURED_CLAIM_FAMILY_VERSION,
            local_action: action.tag(),
        });
    }
    let (family_tag, family_version, local_action) = match action {
        "advance-series-occurrence" => (
            SOURCE_SERIES_FAMILY_TAG,
            SOURCE_SERIES_FAMILY_VERSION,
            RecurringSeriesAction::AdvanceOccurrence.tag(),
        ),
        "close-series-funding" => (
            SOURCE_SERIES_FAMILY_TAG,
            SOURCE_SERIES_FAMILY_VERSION,
            RecurringSeriesAction::CloseFunding.tag(),
        ),
        "close-position" | "close-position-replay" => (
            GENERAL_V2_FAMILY_TAG,
            GENERAL_V2_FAMILY_VERSION,
            GeneralV2Action::ClosePosition.tag(),
        ),
        "advance-failure-recovery" => (
            RECOVERY_FAMILY_TAG,
            RECOVERY_FAMILY_VERSION,
            RecoveryAction::AcceptRecoveryWork.tag(),
        ),
        "advance-failure-interval-consensus" => (
            RECOVERY_FAMILY_TAG,
            RECOVERY_FAMILY_VERSION,
            RecoveryAction::AdvanceIntervalConsensus.tag(),
        ),
        _ => return None,
    };
    Some(CanonicalIntentCoordinate {
        family_tag,
        family_version,
        local_action,
    })
}

fn source_action(coordinate: CanonicalIntentCoordinate) -> Option<SourceSeriesAction> {
    if coordinate.family_tag == SOURCE_SERIES_FAMILY_TAG
        && coordinate.family_version == SOURCE_SERIES_FAMILY_VERSION
    {
        SourceSeriesAction::from_tag(coordinate.local_action)
    } else {
        None
    }
}

fn coordinate_description(
    coordinate: CanonicalIntentCoordinate,
) -> (&'static str, &'static str, Option<&'static str>) {
    if let Some(action) = source_action(coordinate) {
        let name = source_selection_action(action);
        let builder = matches!(
            action,
            SourceSeriesAction::InitializeHead
                | SourceSeriesAction::OpenRawPage
                | SourceSeriesAction::IngestBoundaryBatch
        )
        .then_some("clutch-source-plane-v3-adapter/intent-preimage-v3");
        return ("source", name, builder);
    }
    if coordinate.family_tag == SOURCE_SERIES_FAMILY_TAG
        && coordinate.family_version == SOURCE_SERIES_FAMILY_VERSION
    {
        let action = match RecurringSeriesAction::from_tag(coordinate.local_action) {
            Some(RecurringSeriesAction::RegisterSeries) => "register-series",
            Some(RecurringSeriesAction::ActivateFunding) => "activate-series-funding",
            Some(RecurringSeriesAction::AdvanceOccurrence) => "advance-series-occurrence",
            Some(RecurringSeriesAction::LapseOccurrence) => "lapse-series-occurrence",
            Some(RecurringSeriesAction::ObserveDonation) => "observe-series-donation",
            Some(RecurringSeriesAction::CloseFunding) => "close-series-funding",
            None => "unknown-source-series-action",
        };
        return ("series", action, None);
    }
    if coordinate.family_tag == GENERAL_V2_FAMILY_TAG
        && coordinate.family_version == GENERAL_V2_FAMILY_VERSION
    {
        return ("general", "general-v2-action", None);
    }
    if coordinate.family_tag == STRUCTURED_CLAIM_FAMILY_TAG
        && coordinate.family_version == STRUCTURED_CLAIM_FAMILY_VERSION
    {
        let action = clutch_structured_claim_runtime_contract::StructuredClaimActionV1::from_tag(
            coordinate.local_action,
        )
        .ok();
        return (
            "structured-claim",
            action
                .map(structured_selection_action)
                .unwrap_or("unknown-structured-action"),
            action.map(|_| "clutch-structured-claim-adapter/current-account-contract-v1"),
        );
    }
    if coordinate.family_tag == RECOVERY_FAMILY_TAG
        && coordinate.family_version == RECOVERY_FAMILY_VERSION
    {
        return ("recovery", "recovery-action", None);
    }
    ("unknown", "unknown-action", None)
}

const fn protocol_flow_name(flow: ProtocolFlow) -> &'static str {
    match flow {
        ProtocolFlow::CollateralCustodyV3 => "collateral-custody-v3",
        ProtocolFlow::MarketEpochCreation => "market-epoch-creation",
        ProtocolFlow::SourcePlaneV3 => "source-plane-v3",
        ProtocolFlow::GeneralV2Candidate => "general-v2-candidate",
        ProtocolFlow::GeneralV2Settlement => "general-v2-settlement",
        ProtocolFlow::GeneralV2Fees => "general-v2-fees",
        ProtocolFlow::DirectMarketV1 => "direct-market-v1",
        ProtocolFlow::DirectEggSettlement => "direct-egg-settlement",
        ProtocolFlow::Liveness => "liveness",
        ProtocolFlow::ProductSeries => "product-series",
        ProtocolFlow::StructuredClaim => "structured-claim",
        ProtocolFlow::DealerFacilityTerminal => "dealer-facility-terminal",
        ProtocolFlow::KeeperSettlement => "keeper-settlement",
        ProtocolFlow::RecoveryRetirement => "recovery-retirement",
    }
}

const fn runtime_admission_name(admission: RuntimeAdmission) -> &'static str {
    match admission {
        RuntimeAdmission::ReservedDisabled => "reserved-disabled",
        RuntimeAdmission::ReleaseBoundEnabled => "release-bound-enabled",
        RuntimeAdmission::PayloadVariantReleaseBoundEnabled => {
            "payload-variant-release-bound-enabled"
        }
    }
}

const fn allocation_status_name(
    status: clutch_solana_layout::registry::AllocationStatus,
) -> &'static str {
    match status {
        clutch_solana_layout::registry::AllocationStatus::Frozen => "frozen",
        clutch_solana_layout::registry::AllocationStatus::ReservedDisabled => {
            "reserved-disabled"
        }
        clutch_solana_layout::registry::AllocationStatus::NonProductionLab => {
            "non-production-lab"
        }
        clutch_solana_layout::registry::AllocationStatus::Withdrawn => "withdrawn",
    }
}

fn integer_unit_json(unit: IntegerUnit) -> Value {
    match unit {
        IntegerUnit::Lamports => json!({"kind": "lamports"}),
        IntegerUnit::CollateralAtoms { mint } => {
            json!({"kind": "collateral-atoms", "mint": mint.to_string()})
        }
        IntegerUnit::PriceUnits { scale } => {
            json!({"kind": "price-units", "scale": scale.to_string()})
        }
        IntegerUnit::EggAtoms { market, outcome } => json!({
            "kind": "egg-atoms",
            "market": hex32(market),
            "outcome": outcome.to_string()
        }),
        IntegerUnit::FeeAtoms { mint } => {
            json!({"kind": "fee-atoms", "mint": mint.to_string()})
        }
        IntegerUnit::WrapperAtoms { mint } => {
            json!({"kind": "wrapper-atoms", "mint": mint.to_string()})
        }
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
    hash.update(
        u64::try_from(release.enabled_intents.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for coordinate in &release.enabled_intents {
        hash.update([
            coordinate.family_tag,
            coordinate.family_version,
            coordinate.local_action,
        ]);
    }
    hash.update(
        u64::try_from(release.enabled_intent_variants.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for variant in &release.enabled_intent_variants {
        let coordinate = variant.coordinate();
        hash.update([
            coordinate.family_tag,
            coordinate.family_version,
            coordinate.local_action,
            variant.payload_discriminator(),
        ]);
    }
    hash.update(workflow_id);
    hash.update(u64::try_from(accounts.len()).unwrap_or(u64::MAX).to_le_bytes());
    for version in accounts {
        hash.update(version.account.address.to_bytes());
        hash.update(version.account.owner.to_bytes());
        hash_text(&mut hash, &version.account.provenance.release_key);
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

fn session_selection_json(selection: &KeeperActionSelection) -> Value {
    json!({
        "account": selection.account.to_string(),
        "releaseKey": selection.release_key,
        "action": selection.action,
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
        WorkflowLane::StructuredLifecycle => "structured-lifecycle",
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

fn hex_bytes(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
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

    #[test]
    fn scheduling_names_cannot_promote_an_unrelated_release_coordinate() {
        assert_eq!(
            action_coordinate("open-raw-page"),
            Some(CanonicalIntentCoordinate {
                family_tag: SOURCE_SERIES_FAMILY_TAG,
                family_version: SOURCE_SERIES_FAMILY_VERSION,
                local_action: SourceSeriesAction::OpenRawPage.tag(),
            })
        );
        assert_eq!(action_coordinate("caller-says-enabled"), None);
    }

    #[test]
    fn source_account_roles_are_projected_from_the_layout_owner() {
        let contract = account_contract_v2(SourceSeriesAction::OpenRawPage);
        assert_eq!(contract.len(), 19);
        assert_eq!(
            source_role_label_v2(contract.meta(0).unwrap().role),
            "source-release"
        );
        assert_eq!(
            source_role_label_v2(contract.meta(15).unwrap().role),
            "keeper"
        );
        assert!(contract.meta(15).unwrap().signer);
        assert!(contract.meta(16).unwrap().signer);
        assert_eq!(
            source_role_label_v2(contract.meta(18).unwrap().role),
            "rent-sysvar"
        );
    }
}
