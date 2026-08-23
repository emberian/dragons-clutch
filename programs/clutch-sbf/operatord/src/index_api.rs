//! Thread-safe bridge from the fork-aware account index into operatord HTTP.
//!
//! The bridge owns no transport and performs no acquisition itself. A bounded
//! RPC worker may update the shared index; HTTP readers receive only the
//! untrusted projection and deterministic unsigned-workflow cursors.

use crate::http::{JsonReadResponse, ReadApi};
use clutch_local_real_pyth::account_index::CanonicalAccountIndex;
use clutch_local_real_pyth::operatord::{OperatorJsonApi, ResumableKeeperSelector};
use serde_json::json;
use std::sync::{Arc, RwLock};

pub struct SharedIndexApi {
    index: Arc<RwLock<CanonicalAccountIndex>>,
    selector: ResumableKeeperSelector,
}

impl SharedIndexApi {
    pub fn new(
        index: Arc<RwLock<CanonicalAccountIndex>>,
        selector: ResumableKeeperSelector,
    ) -> Self {
        Self { index, selector }
    }

    /// Produce the GET-only route callback accepted by the loopback server.
    pub fn read_api(self) -> ReadApi {
        Arc::new(move |method, target| {
            if !target.starts_with("/v1/") {
                return None;
            }
            let Ok(index) = self.index.read() else {
                return Some(JsonReadResponse {
                    status: 500,
                    body: json!({"error": "operator index lock is unavailable"}),
                });
            };
            let reply = OperatorJsonApi::new(&index, self.selector).handle(method, target);
            Some(JsonReadResponse {
                status: reply.status,
                body: reply.body,
            })
        })
    }
}
