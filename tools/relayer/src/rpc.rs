//! A minimal, explicit JSON-RPC client for one Solana cluster.
//!
//! This is deliberately a JSON-RPC client rather than a typed SDK client, and
//! the reason is a correctness one rather than a dependency-weight one:
//!
//! - Under a `dataSlice`, the account's **full** width is only reported in the
//!   response's `space` field.  A typed client decodes into an `Account` and
//!   drops it, which would leave the daemon unable to attest `data_len` for any
//!   account it does not carry in full — including every `ProgramData`.
//! - §4.11's dry-run artifact is *the raw JSON RPC response*.  Re-serializing a
//!   decoded struct would be a different document.
//!
//! Every call is bounded, logged and takes `commitment: finalized`.

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};

use crate::error::{RelayerError, Result};
use crate::id32::{ID_BYTES, base58, parse_id32};
use crate::publog::RpcReadLog;

/// One account as a finalized read reported it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedAccount {
    /// Lamport balance.
    pub lamports: u64,
    /// Owning program.
    pub owner: [u8; ID_BYTES],
    /// Whether the runtime reports it executable.
    pub executable: bool,
    /// The bytes the requested `dataSlice` returned.
    pub data: Vec<u8>,
    /// The account's complete on-chain data length, from `space`.
    pub data_len: u64,
}

/// One `getMultipleAccounts` response.
#[derive(Clone, Debug)]
pub struct BatchRead {
    /// The single `context.slot` covering the whole response.
    ///
    /// This is the reason the batch call is mandatory: one slot for the entire
    /// ordered set, rather than a per-account slot that could differ.
    pub slot: u64,
    /// One entry per requested key, `None` where the account does not exist.
    pub accounts: Vec<Option<ObservedAccount>>,
    /// The exact JSON the endpoint returned, kept verbatim for the artifact.
    pub raw: Value,
}

/// One paged `getAccountInfo` read.
#[derive(Clone, Debug)]
pub struct PageRead {
    /// The slot this page was read at.
    pub slot: u64,
    /// The account, or `None` if it vanished between calls.
    pub account: Option<ObservedAccount>,
}

/// A JSON-RPC client bound to one endpoint.
#[derive(Clone)]
pub struct RpcClient {
    url: String,
    host: String,
    http: reqwest::Client,
    read_log: Option<RpcReadLog>,
}

impl RpcClient {
    /// Build a client for one endpoint URL.
    pub fn new(url: &str, timeout: Duration, read_log: Option<RpcReadLog>) -> Result<Self> {
        let parsed = reqwest::Url::parse(url)
            .map_err(|error| RelayerError::config(format!("{url:?} is not a URL: {error}")))?;
        let host = parsed.host_str().unwrap_or("unknown-host").to_owned();
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|source| RelayerError::Transport {
                endpoint: host.clone(),
                source,
            })?;
        Ok(Self {
            url: url.to_owned(),
            host,
            http,
            read_log,
        })
    }

    /// Attach a read log after construction.
    pub fn with_read_log(mut self, log: RpcReadLog) -> Self {
        self.read_log = Some(log);
        self
    }

    /// Open a read log under `output_dir` and attach it.
    pub fn logging_to(self, output_dir: &Path) -> Result<Self> {
        let log = RpcReadLog::open(output_dir)?;
        Ok(self.with_read_log(log))
    }

    /// The endpoint host, used in diagnostics and artifacts.
    ///
    /// The full URL is deliberately never written into an artifact: a
    /// provider URL commonly carries an API key.
    pub fn host(&self) -> &str {
        &self.host
    }

    async fn call(&self, method: &str, params: Value, detail: Value) -> Result<Value> {
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
        let response = self
            .http
            .post(&self.url)
            .json(&body)
            .send()
            .await
            .map_err(|source| RelayerError::Transport {
                endpoint: self.host.clone(),
                source,
            });
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                self.log(method, detail, "transport-error");
                return Err(error);
            }
        };
        let parsed: Value = match response.json().await {
            Ok(parsed) => parsed,
            Err(source) => {
                self.log(method, detail, "malformed-body");
                return Err(RelayerError::Transport {
                    endpoint: self.host.clone(),
                    source,
                });
            }
        };
        if let Some(error) = parsed.get("error") {
            let code = error.get("code").and_then(Value::as_i64).unwrap_or(0);
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("(no message)")
                .to_owned();
            self.log(method, detail, "rpc-error");
            return Err(RelayerError::RpcError {
                endpoint: self.host.clone(),
                method: method.to_owned(),
                code,
                message,
            });
        }
        self.log(method, detail, "ok");
        parsed
            .get("result")
            .cloned()
            .ok_or_else(|| self.malformed(method, "response carried no result"))
    }

    fn log(&self, method: &str, detail: Value, outcome: &str) {
        if let Some(log) = &self.read_log {
            log.record(method, &self.host, detail, outcome);
        }
    }

    fn malformed(&self, method: &str, reason: &str) -> RelayerError {
        RelayerError::MalformedRpcResponse {
            endpoint: self.host.clone(),
            method: method.to_owned(),
            reason: reason.to_owned(),
        }
    }

    /// Read the cluster's genesis hash.
    pub async fn get_genesis_hash(&self) -> Result<[u8; ID_BYTES]> {
        let result = self.call("getGenesisHash", json!([]), json!({})).await?;
        let text = result
            .as_str()
            .ok_or_else(|| self.malformed("getGenesisHash", "result was not a string"))?;
        parse_id32("getGenesisHash", text)
    }

    /// Require the cluster to be the one the config pinned.
    ///
    /// A devnet `Program` account can be byte-identical to its mainnet twin
    /// (§4.6); the genesis hash is the *only* thing that distinguishes them.
    /// Getting this wrong would mean signing an attestation that names mainnet
    /// over bytes read somewhere else, so it is checked once at startup and the
    /// daemon refuses to run on mismatch rather than degrading.
    pub async fn require_expected_genesis(&self, expected: [u8; ID_BYTES]) -> Result<()> {
        let observed = self.get_genesis_hash().await?;
        require_expected_genesis(&self.host, expected, observed)
    }

    /// One `getMultipleAccounts` covering an entire ordered set.
    ///
    /// **Per-account `getAccountInfo` is forbidden for an observation** (§4.11).
    /// The batch call returns a single `context.slot` for the whole response;
    /// separate calls would return separate slots, and a mixed-slot account set
    /// is the observation bug this family most needs to not have.  It is the
    /// RPC-side analogue of the operator invariant
    /// `dclutch-provider-transport-v3-operator::require_same_finalized_observation`.
    ///
    /// One `dataSlice` covers the whole call, so `slice_len` is the widest
    /// pinned `inline_len` in the set and each position is truncated to its own
    /// pinned width afterwards.
    pub async fn get_multiple_accounts(
        &self,
        keys: &[[u8; ID_BYTES]],
        slice_len: u16,
        min_context_slot: Option<u64>,
    ) -> Result<BatchRead> {
        let addresses: Vec<String> = keys.iter().map(base58).collect();
        let mut config = json!({
            "encoding": "base64",
            "commitment": "finalized",
            "dataSlice": { "offset": 0, "length": slice_len },
        });
        if let Some(slot) = min_context_slot
            && let Some(map) = config.as_object_mut()
        {
            map.insert("minContextSlot".to_owned(), json!(slot));
        }
        let detail = json!({ "accounts": addresses.len(), "data_slice_len": slice_len });
        let result = self
            .call("getMultipleAccounts", json!([addresses, config]), detail)
            .await?;

        let slot = context_slot(&result).ok_or_else(|| {
            self.malformed("getMultipleAccounts", "response carried no context.slot")
        })?;
        let values = result
            .get("value")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                self.malformed("getMultipleAccounts", "response carried no value array")
            })?;
        if values.len() != keys.len() {
            return Err(self.malformed(
                "getMultipleAccounts",
                "the response holds a different number of accounts than were requested",
            ));
        }
        let mut accounts = Vec::with_capacity(values.len());
        for value in values {
            accounts.push(self.decode_account("getMultipleAccounts", value)?);
        }
        Ok(BatchRead {
            slot,
            accounts,
            raw: result,
        })
    }

    /// One page of one account's body.
    ///
    /// This is a `getAccountInfo`, and it is not an observation: it carries no
    /// slot into any signed message.  It exists only to compute a tail digest
    /// over bytes the observation already committed to by width, and the caller
    /// re-checks the pinned inline prefix against the batch read so a body that
    /// moved between calls is refused rather than folded in.
    pub async fn get_account_page(
        &self,
        key: &[u8; ID_BYTES],
        offset: u64,
        length: u64,
        min_context_slot: Option<u64>,
    ) -> Result<PageRead> {
        let mut config = json!({
            "encoding": "base64",
            "commitment": "finalized",
            "dataSlice": { "offset": offset, "length": length },
        });
        if let Some(slot) = min_context_slot
            && let Some(map) = config.as_object_mut()
        {
            map.insert("minContextSlot".to_owned(), json!(slot));
        }
        let detail = json!({ "offset": offset, "length": length });
        let result = self
            .call("getAccountInfo", json!([base58(key), config]), detail)
            .await?;
        let slot = context_slot(&result)
            .ok_or_else(|| self.malformed("getAccountInfo", "response carried no context.slot"))?;
        let value = result
            .get("value")
            .ok_or_else(|| self.malformed("getAccountInfo", "response carried no value"))?;
        Ok(PageRead {
            slot,
            account: self.decode_account("getAccountInfo", value)?,
        })
    }

    /// The latest blockhash on the cluster this client points at.
    pub async fn get_latest_blockhash(&self) -> Result<(String, u64)> {
        let result = self
            .call(
                "getLatestBlockhash",
                json!([{ "commitment": "finalized" }]),
                json!({}),
            )
            .await?;
        let value = result
            .get("value")
            .ok_or_else(|| self.malformed("getLatestBlockhash", "response carried no value"))?;
        let blockhash = value
            .get("blockhash")
            .and_then(Value::as_str)
            .ok_or_else(|| self.malformed("getLatestBlockhash", "no blockhash"))?
            .to_owned();
        let last_valid = value
            .get("lastValidBlockHeight")
            .and_then(Value::as_u64)
            .ok_or_else(|| self.malformed("getLatestBlockhash", "no lastValidBlockHeight"))?;
        Ok((blockhash, last_valid))
    }

    /// Submit a serialized transaction.
    ///
    /// Reaching this function at all requires passing
    /// [`crate::submit::require_local_or_authorized`], which refuses a non-local
    /// host unless the operator set `allow_public_submission = true` under an
    /// authorization that names the act.
    pub async fn send_transaction(&self, wire_base64: &str) -> Result<String> {
        let result = self
            .call(
                "sendTransaction",
                json!([wire_base64, { "encoding": "base64", "preflightCommitment": "finalized" }]),
                json!({ "bytes_base64_len": wire_base64.len() }),
            )
            .await?;
        result
            .as_str()
            .map(ToOwned::to_owned)
            .ok_or_else(|| self.malformed("sendTransaction", "result was not a signature string"))
    }

    fn decode_account(&self, method: &str, value: &Value) -> Result<Option<ObservedAccount>> {
        if value.is_null() {
            return Ok(None);
        }
        let lamports = value
            .get("lamports")
            .and_then(Value::as_u64)
            .ok_or_else(|| self.malformed(method, "account carried no lamports"))?;
        let owner_text = value
            .get("owner")
            .and_then(Value::as_str)
            .ok_or_else(|| self.malformed(method, "account carried no owner"))?;
        let owner = parse_id32("owner", owner_text)?;
        let executable = value
            .get("executable")
            .and_then(Value::as_bool)
            .ok_or_else(|| self.malformed(method, "account carried no executable flag"))?;
        // `space` is the account's complete width, computed before the
        // `dataSlice` is applied.  Without it a sliced read cannot state
        // `data_len`, and a guessed `data_len` would be an interpretation.
        let data_len = value.get("space").and_then(Value::as_u64).ok_or_else(|| {
            self.malformed(
                method,
                "account carried no `space` field, so its complete data_len is unknown under a \
                     dataSlice; this endpoint cannot serve a relayed observation",
            )
        })?;
        let encoded = value
            .get("data")
            .and_then(Value::as_array)
            .and_then(|parts| parts.first())
            .and_then(Value::as_str)
            .ok_or_else(|| self.malformed(method, "account data was not [base64, \"base64\"]"))?;
        let encoding = value
            .get("data")
            .and_then(Value::as_array)
            .and_then(|parts| parts.get(1))
            .and_then(Value::as_str)
            .unwrap_or("");
        if encoding != "base64" {
            return Err(self.malformed(method, "account data was not base64 encoded"));
        }
        let data = base64_decode(encoded)
            .ok_or_else(|| self.malformed(method, "account data was not valid base64"))?;
        Ok(Some(ObservedAccount {
            lamports,
            owner,
            executable,
            data,
            data_len,
        }))
    }
}

fn context_slot(result: &Value) -> Option<u64> {
    result.get("context")?.get("slot")?.as_u64()
}

fn base64_decode(text: &str) -> Option<Vec<u8>> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.decode(text).ok()
}

/// Encode transaction wire bytes the way `sendTransaction` expects.
pub fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// The genesis check, factored out so it is testable without a network.
pub fn require_expected_genesis(
    endpoint_host: &str,
    expected: [u8; ID_BYTES],
    observed: [u8; ID_BYTES],
) -> Result<()> {
    if expected != observed {
        return Err(RelayerError::GenesisMismatch {
            endpoint: endpoint_host.to_owned(),
            expected: base58(&expected),
            observed: base58(&observed),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cluster_that_is_not_the_pinned_cluster_refuses() {
        let error = require_expected_genesis(
            "api.devnet.solana.com",
            dclutch_relay_contract::SOLANA_MAINNET_GENESIS_HASH_V1,
            dclutch_relay_contract::SOLANA_DEVNET_GENESIS_HASH_V1,
        )
        .unwrap_err();
        match error {
            RelayerError::GenesisMismatch {
                endpoint,
                expected,
                observed,
            } => {
                assert_eq!(endpoint, "api.devnet.solana.com");
                assert_eq!(expected, "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d");
                assert_eq!(observed, "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG");
            }
            other => panic!("expected a genesis mismatch, got {other:?}"),
        }
    }

    #[test]
    fn the_pinned_cluster_is_admitted() {
        require_expected_genesis(
            "localhost",
            dclutch_relay_contract::SOLANA_MAINNET_GENESIS_HASH_V1,
            dclutch_relay_contract::SOLANA_MAINNET_GENESIS_HASH_V1,
        )
        .expect("the pinned cluster is the observed cluster");
    }

    fn client() -> RpcClient {
        RpcClient::new("http://127.0.0.1:8899", Duration::from_secs(1), None).expect("client")
    }

    #[test]
    fn an_account_without_a_space_field_refuses_rather_than_guessing_data_len() {
        let value = serde_json::json!({
            "lamports": 1,
            "owner": "11111111111111111111111111111112",
            "executable": false,
            "data": ["", "base64"],
        });
        let error = client()
            .decode_account("getMultipleAccounts", &value)
            .unwrap_err();
        assert!(
            matches!(error, RelayerError::MalformedRpcResponse { .. }),
            "{error:?}"
        );
    }

    #[test]
    fn a_jsonparsed_account_refuses_because_it_is_not_raw_bytes() {
        let value = serde_json::json!({
            "lamports": 1,
            "owner": "11111111111111111111111111111112",
            "executable": false,
            "space": 40,
            "data": [{"parsed": {}}, "jsonParsed"],
        });
        assert!(
            client()
                .decode_account("getMultipleAccounts", &value)
                .is_err()
        );
    }

    #[test]
    fn a_well_formed_account_decodes_to_its_bytes_and_full_width() {
        let value = serde_json::json!({
            "lamports": 1_000_000,
            "owner": "BPFLoaderUpgradeab1e11111111111111111111111",
            "executable": true,
            "space": 2_300_000,
            "data": ["AwAAAA==", "base64"],
        });
        let account = client()
            .decode_account("getMultipleAccounts", &value)
            .expect("decodes")
            .expect("present");
        assert_eq!(account.lamports, 1_000_000);
        assert_eq!(account.owner, crate::chain::LOADER_V3_PROGRAM_ID);
        assert!(account.executable);
        assert_eq!(account.data, vec![3, 0, 0, 0]);
        assert_eq!(account.data_len, 2_300_000);
    }

    #[test]
    fn a_missing_account_decodes_to_none_rather_than_an_empty_one() {
        assert_eq!(
            client()
                .decode_account("getMultipleAccounts", &serde_json::Value::Null)
                .expect("decodes"),
            None
        );
    }

    #[test]
    fn base64_round_trips_the_transaction_wire() {
        let bytes = [0u8, 1, 2, 250, 251, 252];
        let text = base64_encode(&bytes);
        assert_eq!(base64_decode(&text), Some(bytes.to_vec()));
    }
}
