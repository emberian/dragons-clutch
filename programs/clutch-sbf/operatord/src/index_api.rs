//! Thread-safe bridge from the fork-aware account index into operatord HTTP.
//!
//! Processed reads are dynamically admitted only while one complete WebSocket
//! subscription generation is live. Every such read remains a rollbackable,
//! untrusted projection and is never eligible to authorize a workflow.

use crate::http::{JsonReadResponse, ReadApi};
use clutch_local_real_pyth::account_index::CANONICAL_ACCOUNT_DECODER_SET;
use clutch_local_real_pyth::index_service::{
    ProcessedReconnectRollback, RpcIndexEngine, RpcIndexEngineEvent,
};
use clutch_local_real_pyth::operatord::{OperatorJsonApi, ResumableKeeperSelector};
use clutch_local_real_pyth::rpc_index::{
    public_rpc_endpoint_binding, ObservedSlotUpdateKind, RpcAccountRemovalKind,
};
use serde_json::{json, Value};
use std::sync::{Arc, RwLock};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessedTransportSnapshot {
    pub phase: String,
    pub available: bool,
    pub websocket_genesis_matched: bool,
    pub connection_generation: u64,
    pub rollback_epoch: u64,
    pub reconnect_attempt: u64,
    pub next_backoff_milliseconds: u64,
    pub reconnect_indexed_versions_withdrawn: u64,
    pub reconnect_buffered_accounts_withdrawn: u64,
    pub dead_slot_rollbacks: u64,
    pub dead_slot_indexed_versions_withdrawn: u64,
    pub dead_slot_buffered_accounts_withdrawn: u64,
    pub account_removal_events: u64,
    pub closed_account_removals: u64,
    pub owner_changed_account_removals: u64,
    pub account_projections_withdrawn: u64,
    pub last_rollback_slot: Option<u64>,
    pub last_removed_account: Option<String>,
    pub last_removal_observed_owner: Option<String>,
    pub last_removal_kind: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug)]
pub struct ProcessedTransportState {
    snapshot: ProcessedTransportSnapshot,
}

impl Default for ProcessedTransportState {
    fn default() -> Self {
        Self {
            snapshot: ProcessedTransportSnapshot {
                phase: "not-started".to_string(),
                available: false,
                websocket_genesis_matched: false,
                connection_generation: 0,
                rollback_epoch: 0,
                reconnect_attempt: 0,
                next_backoff_milliseconds: 0,
                reconnect_indexed_versions_withdrawn: 0,
                reconnect_buffered_accounts_withdrawn: 0,
                dead_slot_rollbacks: 0,
                dead_slot_indexed_versions_withdrawn: 0,
                dead_slot_buffered_accounts_withdrawn: 0,
                account_removal_events: 0,
                closed_account_removals: 0,
                owner_changed_account_removals: 0,
                account_projections_withdrawn: 0,
                last_rollback_slot: None,
                last_removed_account: None,
                last_removal_observed_owner: None,
                last_removal_kind: None,
                last_error: None,
            },
        }
    }
}

impl ProcessedTransportState {
    #[must_use]
    pub fn snapshot(&self) -> ProcessedTransportSnapshot {
        self.snapshot.clone()
    }

    pub fn withdraw_generation(
        &mut self,
        rollback: ProcessedReconnectRollback,
    ) -> Result<(), &'static str> {
        self.snapshot.rollback_epoch = self
            .snapshot
            .rollback_epoch
            .checked_add(1)
            .ok_or("processed rollback epoch exhausted")?;
        self.snapshot.reconnect_indexed_versions_withdrawn = self
            .snapshot
            .reconnect_indexed_versions_withdrawn
            .checked_add(
                u64::try_from(rollback.indexed_versions_removed)
                    .map_err(|_| "processed rollback count exceeds u64")?,
            )
            .ok_or("processed rollback count exhausted")?;
        self.snapshot.reconnect_buffered_accounts_withdrawn = self
            .snapshot
            .reconnect_buffered_accounts_withdrawn
            .checked_add(
                u64::try_from(rollback.buffered_accounts_removed)
                    .map_err(|_| "processed buffered rollback count exceeds u64")?,
            )
            .ok_or("processed buffered rollback count exhausted")?;
        self.snapshot.phase = "withdrawn".to_string();
        self.snapshot.available = false;
        self.snapshot.websocket_genesis_matched = false;
        self.snapshot.next_backoff_milliseconds = 0;
        Ok(())
    }

    pub fn begin_generation(&mut self) -> Result<(), &'static str> {
        self.snapshot.connection_generation = self
            .snapshot
            .connection_generation
            .checked_add(1)
            .ok_or("processed connection generation exhausted")?;
        self.snapshot.phase = "connecting".to_string();
        self.snapshot.available = false;
        self.snapshot.websocket_genesis_matched = false;
        self.snapshot.next_backoff_milliseconds = 0;
        Ok(())
    }

    pub fn mark_registering(&mut self) -> Result<(), &'static str> {
        if !self.snapshot.websocket_genesis_matched {
            return Err("processed subscriptions require a matched WebSocket genesis challenge");
        }
        self.snapshot.phase = "registering-and-buffering".to_string();
        self.snapshot.available = false;
        Ok(())
    }

    pub fn mark_authenticating_genesis(&mut self) {
        self.snapshot.phase = "authenticating-websocket-genesis".to_string();
        self.snapshot.available = false;
        self.snapshot.websocket_genesis_matched = false;
    }

    pub fn mark_genesis_matched(&mut self) -> Result<(), &'static str> {
        if self.snapshot.phase != "authenticating-websocket-genesis" {
            return Err("WebSocket genesis match arrived outside its challenge phase");
        }
        self.snapshot.websocket_genesis_matched = true;
        Ok(())
    }

    pub fn mark_replaying(&mut self) {
        self.snapshot.phase = "replaying-after-finalized-scan".to_string();
        self.snapshot.available = false;
    }

    pub fn mark_live(&mut self) -> Result<(), &'static str> {
        if !self.snapshot.websocket_genesis_matched {
            return Err("processed generation cannot become live before genesis matches");
        }
        self.snapshot.phase = "live-nonfinal".to_string();
        self.snapshot.available = true;
        self.snapshot.reconnect_attempt = 0;
        self.snapshot.next_backoff_milliseconds = 0;
        self.snapshot.last_error = None;
        Ok(())
    }

    pub fn mark_withdrawing(&mut self, error: &str) {
        self.snapshot.phase = "withdrawing-failed-generation".to_string();
        self.snapshot.available = false;
        self.snapshot.last_error = Some(error.chars().take(512).collect());
    }

    pub fn mark_backoff(
        &mut self,
        error: &str,
        backoff_milliseconds: u64,
    ) -> Result<(), &'static str> {
        self.snapshot.phase = "backoff-withdrawn".to_string();
        self.snapshot.available = false;
        self.snapshot.reconnect_attempt = self
            .snapshot
            .reconnect_attempt
            .checked_add(1)
            .ok_or("processed reconnect attempt exhausted")?;
        self.snapshot.next_backoff_milliseconds = backoff_milliseconds;
        self.snapshot.last_error = Some(error.chars().take(512).collect());
        Ok(())
    }

    pub fn admit_events(&mut self, events: &[RpcIndexEngineEvent]) -> Result<(), &'static str> {
        for event in events {
            match event {
                RpcIndexEngineEvent::SlotUpdated {
                    slot,
                    kind: ObservedSlotUpdateKind::Dead,
                } => {
                    self.snapshot.rollback_epoch = self
                        .snapshot
                        .rollback_epoch
                        .checked_add(1)
                        .ok_or("processed rollback epoch exhausted")?;
                    self.snapshot.dead_slot_rollbacks = self
                        .snapshot
                        .dead_slot_rollbacks
                        .checked_add(1)
                        .ok_or("processed dead-slot rollback count exhausted")?;
                    self.snapshot.last_rollback_slot = Some(*slot);
                }
                RpcIndexEngineEvent::IndexedAccountsRolledBack { account_count, .. } => {
                    self.snapshot.dead_slot_indexed_versions_withdrawn = self
                        .snapshot
                        .dead_slot_indexed_versions_withdrawn
                        .checked_add(
                            u64::try_from(*account_count)
                                .map_err(|_| "dead-slot indexed rollback exceeds u64")?,
                        )
                        .ok_or("dead-slot indexed rollback count exhausted")?;
                }
                RpcIndexEngineEvent::BufferedAccountsDropped { account_count, .. } => {
                    self.snapshot.dead_slot_buffered_accounts_withdrawn = self
                        .snapshot
                        .dead_slot_buffered_accounts_withdrawn
                        .checked_add(
                            u64::try_from(*account_count)
                                .map_err(|_| "dead-slot buffered rollback exceeds u64")?,
                        )
                        .ok_or("dead-slot buffered rollback count exhausted")?;
                }
                RpcIndexEngineEvent::AccountRemoved {
                    address,
                    observed_owner,
                    slot,
                    kind,
                    projection_withdrawn,
                } => {
                    self.snapshot.account_removal_events = self
                        .snapshot
                        .account_removal_events
                        .checked_add(1)
                        .ok_or("processed account removal count exhausted")?;
                    match kind {
                        RpcAccountRemovalKind::Closed => {
                            self.snapshot.closed_account_removals = self
                                .snapshot
                                .closed_account_removals
                                .checked_add(1)
                                .ok_or("processed closure count exhausted")?;
                        }
                        RpcAccountRemovalKind::OwnerChanged => {
                            self.snapshot.owner_changed_account_removals = self
                                .snapshot
                                .owner_changed_account_removals
                                .checked_add(1)
                                .ok_or("processed owner-change count exhausted")?;
                        }
                    }
                    if *projection_withdrawn {
                        self.snapshot.rollback_epoch = self
                            .snapshot
                            .rollback_epoch
                            .checked_add(1)
                            .ok_or("processed rollback epoch exhausted")?;
                        self.snapshot.account_projections_withdrawn = self
                            .snapshot
                            .account_projections_withdrawn
                            .checked_add(1)
                            .ok_or("processed projection withdrawal count exhausted")?;
                        self.snapshot.last_rollback_slot = Some(*slot);
                    }
                    self.snapshot.last_removed_account = Some(address.to_string());
                    self.snapshot.last_removal_observed_owner = Some(observed_owner.to_string());
                    self.snapshot.last_removal_kind = Some(kind.name().to_string());
                }
                _ => {}
            }
        }
        Ok(())
    }
}

pub type SharedProcessedTransport = Arc<RwLock<ProcessedTransportState>>;

pub struct SharedIndexApi {
    engine: Arc<RwLock<RpcIndexEngine>>,
    selector: ResumableKeeperSelector,
    processed: SharedProcessedTransport,
}

impl SharedIndexApi {
    pub fn processed(
        engine: Arc<RwLock<RpcIndexEngine>>,
        selector: ResumableKeeperSelector,
        processed: SharedProcessedTransport,
    ) -> Self {
        Self {
            engine,
            selector,
            processed,
        }
    }

    pub fn read_api(self) -> ReadApi {
        Arc::new(move |method, target| {
            if !target.starts_with("/v1/") {
                return None;
            }
            let before = processed_snapshot(&self.processed);
            let Ok(engine) = self.engine.read() else {
                return Some(JsonReadResponse {
                    status: 500,
                    body: json!({"error": "operator index lock is unavailable"}),
                });
            };
            if method == "GET" && target == "/v1/acquisition" {
                return Some(acquisition_response(&engine, before));
            }
            let processed_query = target
                .split_once('?')
                .is_some_and(|(_, query)| query == "commitment=processed");
            let fork_surface = method == "GET" && target == "/v1/forks";
            if fork_surface && !before.as_ref().is_some_and(|state| state.available) {
                return Some(JsonReadResponse {
                    status: 200,
                    body: json!({
                        "finalizedRoot": null,
                        "authorityEligible": false,
                        "processedTopology": true,
                        "processedAvailable": false,
                        "disabledReason": "processed fork topology is withdrawn with its WebSocket generation",
                        "frozenSlots": [],
                        "deadSlots": [],
                        "nodes": []
                    }),
                });
            }
            if processed_query && !before.as_ref().is_some_and(|state| state.available) {
                return Some(processed_unavailable(before.as_ref()));
            }
            if processed_query && target.starts_with("/v1/keeper/next?") {
                return Some(JsonReadResponse {
                    status: 200,
                    body: json!({
                        "effectiveCommitment": "processed",
                        "authorityEligible": false,
                        "disabledReason": "processed observations are rollbackable untrusted projections and never authorize keeper workflow construction",
                        "actions": []
                    }),
                });
            }
            let reply = OperatorJsonApi::new(engine.index(), self.selector).handle(method, target);
            let mut body = reply.body;
            if method == "GET" && matches!(target, "/v1/health" | "/v1/releases") {
                if let Some(object) = body.as_object_mut() {
                    object.insert(
                        "transportBinding".to_string(),
                        transport_binding(engine.index().acquisition_plan()),
                    );
                }
            }
            drop(engine);
            if processed_query || fork_surface {
                let after = processed_snapshot(&self.processed);
                if after != before || !after.as_ref().is_some_and(|state| state.available) {
                    return Some(processed_unavailable(after.as_ref()));
                }
            }
            Some(JsonReadResponse {
                status: reply.status,
                body,
            })
        })
    }
}

fn processed_snapshot(processed: &SharedProcessedTransport) -> Option<ProcessedTransportSnapshot> {
    processed.read().ok().map(|state| state.snapshot())
}

fn processed_unavailable(state: Option<&ProcessedTransportSnapshot>) -> JsonReadResponse {
    JsonReadResponse {
        status: 409,
        body: json!({
            "error": "processed projection is withdrawn until one complete WebSocket subscription generation and its release-bracketed finalized scan replay succeed",
            "processedTransport": state.map(processed_json)
        }),
    }
}

fn acquisition_response(
    engine: &RpcIndexEngine,
    processed: Option<ProcessedTransportSnapshot>,
) -> JsonReadResponse {
    let status = engine.status();
    let plan = engine.index().acquisition_plan();
    let available = processed.as_ref().is_some_and(|state| state.available)
        && processed
            .as_ref()
            .is_some_and(|state| state.websocket_genesis_matched)
        && status.remaining_subscription_registrations == 0
        && status.active_subscriptions == plan.releases.len().saturating_add(3);
    JsonReadResponse {
        status: 200,
        body: json!({
            "bootstrapComplete": status.bootstrap_complete,
            "remainingScans": status.remaining_scans.to_string(),
            "remainingSubscriptionRegistrations": status.remaining_subscription_registrations.to_string(),
            "activeSubscriptions": status.active_subscriptions.to_string(),
            "pendingAccounts": status.pending_accounts.to_string(),
            "pendingAccountBytes": status.pending_account_bytes.to_string(),
            "pendingRoot": status.pending_root.map(|slot| slot.to_string()),
            "nextReceiveSequence": status.next_receive_sequence.to_string(),
            "authority": "untrusted read model",
            "authorityEligible": false,
            "transportMode": if processed.is_some() { "finalized-plus-processed-websocket" } else { "finalized-rpc-polling" },
            "processedAvailable": available,
            "processedSemantics": {
                "finality": "nonfinal-rollbackable",
                "websocketGenesis": "the exact WebSocket connection must answer getGenesisHash with the selected genesis before any subscription request is sent",
                "branchSelection": "highest frozen descendant of the observed finalized root; deterministic receive-sequence and blockhash tie-break",
                "deadSlot": "withdraw buffered rows and exclude every indexed descendant of the dead slot",
                "accountRemoval": "a well-formed non-executable owner change or exact zero-lamport empty closure records a fork-bound release-specific removal; ambiguous or malformed changes withdraw the generation",
                "reconnect": "withdraw every processed version, fork node, pending row, root, and server-assigned subscription ID; rebuild after a new finalized scan and ordered notification replay",
                "authorityEligibility": false
            },
            "processedTransport": processed.as_ref().map(processed_json),
            "transportBinding": transport_binding(plan)
        }),
    }
}

fn transport_binding(plan: &clutch_local_real_pyth::rpc_index::RpcIndexPlan) -> Value {
    let releases = plan
        .releases
        .iter()
        .map(|release| {
            json!({
                "releaseKey": release.key(),
                "programId": release.program_id.to_string(),
                "programData": release.program_data.to_string(),
                "deploymentSlot": release.deployment_slot.to_string(),
                "elfSha256": hex32(release.elf_sha256),
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
        .collect::<Vec<_>>();
    let http = public_rpc_endpoint_binding(&plan.cluster.rpc_http_url);
    let websocket = public_rpc_endpoint_binding(&plan.cluster.rpc_websocket_url);
    json!({
        "schema": "dragons-clutch/operator-rpc-transport-binding/v3",
        "verificationDisposition": "last-complete-untrusted-http-release-bracket",
        "authorityEligible": false,
        "clusterName": plan.cluster.cluster_name,
        "genesisHash": plan.cluster.genesis_hash,
        "clusterKey": plan.cluster.key(),
        "decoderSet": CANONICAL_ACCOUNT_DECODER_SET,
        "rpcHttpEndpoint": {
            "redacted": http.redacted,
            "bindingSha256": hex32(http.binding_sha256)
        },
        "rpcWebsocketEndpoint": {
            "redacted": websocket.redacted,
            "bindingSha256": hex32(websocket.binding_sha256)
        },
        "releases": releases
    })
}

fn processed_json(state: &ProcessedTransportSnapshot) -> Value {
    json!({
        "phase": state.phase,
        "available": state.available,
        "websocketGenesisMatched": state.websocket_genesis_matched,
        "connectionGeneration": state.connection_generation.to_string(),
        "rollbackEpoch": state.rollback_epoch.to_string(),
        "reconnectAttempt": state.reconnect_attempt.to_string(),
        "nextBackoffMilliseconds": state.next_backoff_milliseconds.to_string(),
        "reconnectIndexedVersionsWithdrawn": state.reconnect_indexed_versions_withdrawn.to_string(),
        "reconnectBufferedAccountsWithdrawn": state.reconnect_buffered_accounts_withdrawn.to_string(),
        "deadSlotRollbacks": state.dead_slot_rollbacks.to_string(),
        "deadSlotIndexedVersionsWithdrawn": state.dead_slot_indexed_versions_withdrawn.to_string(),
        "deadSlotBufferedAccountsWithdrawn": state.dead_slot_buffered_accounts_withdrawn.to_string(),
        "accountRemovalEvents": state.account_removal_events.to_string(),
        "closedAccountRemovals": state.closed_account_removals.to_string(),
        "ownerChangedAccountRemovals": state.owner_changed_account_removals.to_string(),
        "accountProjectionsWithdrawn": state.account_projections_withdrawn.to_string(),
        "lastRollbackSlot": state.last_rollback_slot.map(|slot| slot.to_string()),
        "lastRemovedAccount": state.last_removed_account,
        "lastRemovalObservedOwner": state.last_removal_observed_owner,
        "lastRemovalKind": state.last_removal_kind,
        "lastError": state.last_error
    })
}

fn hex32(bytes: [u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dead_slot_is_an_explicit_rollback_epoch() {
        let mut state = ProcessedTransportState::default();
        state.begin_generation().unwrap();
        state.mark_authenticating_genesis();
        state.mark_genesis_matched().unwrap();
        state.mark_registering().unwrap();
        state.mark_live().unwrap();
        let before = state.snapshot().rollback_epoch;
        state
            .admit_events(&[
                RpcIndexEngineEvent::SlotUpdated {
                    slot: 41,
                    kind: ObservedSlotUpdateKind::Dead,
                },
                RpcIndexEngineEvent::IndexedAccountsRolledBack {
                    slot: 41,
                    account_count: 3,
                },
                RpcIndexEngineEvent::BufferedAccountsDropped {
                    slot: 42,
                    account_count: 2,
                },
            ])
            .unwrap();
        let snapshot = state.snapshot();
        assert_eq!(snapshot.rollback_epoch, before + 1);
        assert_eq!(snapshot.last_rollback_slot, Some(41));
        assert_eq!(snapshot.dead_slot_rollbacks, 1);
        assert_eq!(snapshot.dead_slot_indexed_versions_withdrawn, 3);
        assert_eq!(snapshot.dead_slot_buffered_accounts_withdrawn, 2);
        assert!(snapshot.available);
    }

    #[test]
    fn safe_account_removal_advances_projection_rollback_epoch() {
        let mut state = ProcessedTransportState::default();
        state.begin_generation().unwrap();
        state.mark_authenticating_genesis();
        state.mark_genesis_matched().unwrap();
        state.mark_registering().unwrap();
        state.mark_live().unwrap();
        let address = solana_address::Address::new_from_array([0x31; 32]);
        let observed_owner = solana_address::Address::new_from_array([0x32; 32]);
        state
            .admit_events(&[RpcIndexEngineEvent::AccountRemoved {
                address,
                observed_owner,
                slot: 52,
                kind: RpcAccountRemovalKind::OwnerChanged,
                projection_withdrawn: true,
            }])
            .unwrap();
        let snapshot = state.snapshot();
        assert_eq!(snapshot.rollback_epoch, 1);
        assert_eq!(snapshot.account_removal_events, 1);
        assert_eq!(snapshot.owner_changed_account_removals, 1);
        assert_eq!(snapshot.account_projections_withdrawn, 1);
        assert_eq!(snapshot.last_removed_account, Some(address.to_string()));
        assert_eq!(
            snapshot.last_removal_observed_owner,
            Some(observed_owner.to_string())
        );
        assert_eq!(snapshot.last_removal_kind.as_deref(), Some("owner-changed"));
        assert!(snapshot.available);
    }
}
