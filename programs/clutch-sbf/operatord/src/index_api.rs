//! Thread-safe bridge from the fork-aware account index into operatord HTTP.
//!
//! Processed reads are dynamically admitted only while one complete WebSocket
//! subscription generation is live. Every such read remains a rollbackable,
//! untrusted projection and is never eligible to authorize a workflow.

use crate::http::{JsonReadResponse, ReadApi};
use clutch_local_real_pyth::index_service::{
    ProcessedReconnectRollback, RpcIndexEngine, RpcIndexEngineEvent,
};
use clutch_local_real_pyth::operatord::{OperatorJsonApi, ResumableKeeperSelector};
use clutch_local_real_pyth::rpc_index::ObservedSlotUpdateKind;
use serde_json::{json, Value};
use std::sync::{Arc, RwLock};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessedTransportSnapshot {
    pub phase: String,
    pub available: bool,
    pub connection_generation: u64,
    pub rollback_epoch: u64,
    pub reconnect_attempt: u64,
    pub next_backoff_milliseconds: u64,
    pub reconnect_indexed_versions_withdrawn: u64,
    pub reconnect_buffered_accounts_withdrawn: u64,
    pub dead_slot_rollbacks: u64,
    pub dead_slot_indexed_versions_withdrawn: u64,
    pub dead_slot_buffered_accounts_withdrawn: u64,
    pub last_rollback_slot: Option<u64>,
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
                connection_generation: 0,
                rollback_epoch: 0,
                reconnect_attempt: 0,
                next_backoff_milliseconds: 0,
                reconnect_indexed_versions_withdrawn: 0,
                reconnect_buffered_accounts_withdrawn: 0,
                dead_slot_rollbacks: 0,
                dead_slot_indexed_versions_withdrawn: 0,
                dead_slot_buffered_accounts_withdrawn: 0,
                last_rollback_slot: None,
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
        self.snapshot.next_backoff_milliseconds = 0;
        Ok(())
    }

    pub fn mark_registering(&mut self) {
        self.snapshot.phase = "registering-and-buffering".to_string();
        self.snapshot.available = false;
    }

    pub fn mark_replaying(&mut self) {
        self.snapshot.phase = "replaying-after-finalized-scan".to_string();
        self.snapshot.available = false;
    }

    pub fn mark_live(&mut self) {
        self.snapshot.phase = "live-nonfinal".to_string();
        self.snapshot.available = true;
        self.snapshot.reconnect_attempt = 0;
        self.snapshot.next_backoff_milliseconds = 0;
        self.snapshot.last_error = None;
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
                "branchSelection": "highest frozen descendant of the observed finalized root; deterministic receive-sequence and blockhash tie-break",
                "deadSlot": "withdraw buffered rows and exclude every indexed descendant of the dead slot",
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
                "families": release.families.iter().map(|family| family.name()).collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema": "dragons-clutch/operator-rpc-transport-binding/v1",
        "verificationDisposition": "last-complete-untrusted-http-release-bracket",
        "authorityEligible": false,
        "clusterName": plan.cluster.cluster_name,
        "genesisHash": plan.cluster.genesis_hash,
        "clusterKey": plan.cluster.key(),
        "rpcHttpUrl": plan.cluster.rpc_http_url,
        "rpcWebsocketUrl": plan.cluster.rpc_websocket_url,
        "releases": releases
    })
}

fn processed_json(state: &ProcessedTransportSnapshot) -> Value {
    json!({
        "phase": state.phase,
        "available": state.available,
        "connectionGeneration": state.connection_generation.to_string(),
        "rollbackEpoch": state.rollback_epoch.to_string(),
        "reconnectAttempt": state.reconnect_attempt.to_string(),
        "nextBackoffMilliseconds": state.next_backoff_milliseconds.to_string(),
        "reconnectIndexedVersionsWithdrawn": state.reconnect_indexed_versions_withdrawn.to_string(),
        "reconnectBufferedAccountsWithdrawn": state.reconnect_buffered_accounts_withdrawn.to_string(),
        "deadSlotRollbacks": state.dead_slot_rollbacks.to_string(),
        "deadSlotIndexedVersionsWithdrawn": state.dead_slot_indexed_versions_withdrawn.to_string(),
        "deadSlotBufferedAccountsWithdrawn": state.dead_slot_buffered_accounts_withdrawn.to_string(),
        "lastRollbackSlot": state.last_rollback_slot.map(|slot| slot.to_string()),
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
        state.mark_live();
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
}
