//! Thread-safe bridge from the fork-aware account index into operatord HTTP.
//!
//! The bridge owns no transport and performs no acquisition itself. A bounded
//! RPC worker may update the shared index; HTTP readers receive only the
//! untrusted projection and deterministic unsigned-workflow cursors.

use crate::http::{JsonReadResponse, ReadApi};
use clutch_local_real_pyth::index_service::RpcIndexEngine;
use clutch_local_real_pyth::operatord::{OperatorJsonApi, ResumableKeeperSelector};
use serde_json::json;
use std::sync::{Arc, RwLock};

pub struct SharedIndexApi {
    engine: Arc<RwLock<RpcIndexEngine>>,
    selector: ResumableKeeperSelector,
    processed_enabled: bool,
}

impl SharedIndexApi {
    pub fn new(engine: Arc<RwLock<RpcIndexEngine>>, selector: ResumableKeeperSelector) -> Self {
        Self {
            engine,
            selector,
            processed_enabled: true,
        }
    }

    /// Construct a projection backed only by repeated finalized RPC scans.
    /// Processed queries fail closed until a transport owns all required
    /// program/block/slot/root subscriptions and admits them to the engine.
    pub fn finalized_only(
        engine: Arc<RwLock<RpcIndexEngine>>,
        selector: ResumableKeeperSelector,
    ) -> Self {
        Self {
            engine,
            selector,
            processed_enabled: false,
        }
    }

    /// Produce the GET-only route callback accepted by the loopback server.
    pub fn read_api(self) -> ReadApi {
        Arc::new(move |method, target| {
            if !target.starts_with("/v1/") {
                return None;
            }
            let Ok(engine) = self.engine.read() else {
                return Some(JsonReadResponse {
                    status: 500,
                    body: json!({"error": "operator index lock is unavailable"}),
                });
            };
            if method == "GET" && target == "/v1/acquisition" {
                let status = engine.status();
                return Some(JsonReadResponse {
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
                        "transportMode": if self.processed_enabled { "finalized-plus-processed-subscriptions" } else { "finalized-rpc-polling" },
                        "processedAvailable": self.processed_enabled
                    }),
                });
            }
            if !self.processed_enabled
                && target
                    .split_once('?')
                    .is_some_and(|(_, query)| query == "commitment=processed")
            {
                return Some(JsonReadResponse {
                    status: 409,
                    body: json!({
                        "error": "processed projection is disabled because this transport owns finalized scans only"
                    }),
                });
            }
            let reply = OperatorJsonApi::new(engine.index(), self.selector).handle(method, target);
            Some(JsonReadResponse {
                status: reply.status,
                body: reply.body,
            })
        })
    }
}
