//! Throttled JSON-RPC client over a `curl` subprocess.
//!
//! Deliberately the same transport shape as `committed-harness`: no HTTP
//! crate enters the graph, every request is one `curl` invocation, and the
//! caller decides what a response means.  On top of that this client adds the
//! two things a public cluster demands and a loopback validator does not:
//! a minimum interval between requests (devnet RPC is rate limited) and
//! bounded retries with backoff for transport-level failures.

use serde_json::{json, Value};
use std::{
    process::Command,
    thread,
    time::{Duration, Instant},
};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// The mainnet-beta genesis hash.  Observing it is an immediate abort: this
/// driver is devnet/loopback evidence tooling and must never touch mainnet.
pub const MAINNET_GENESIS: &str = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d";
/// The public devnet genesis hash, used only to label the transcript.
pub const DEVNET_GENESIS: &str = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG";

/// Admit only loopback HTTP or an HTTPS devnet endpoint.
///
/// Fail-closed on purpose: anything mentioning mainnet is refused outright,
/// non-loopback URLs must be HTTPS and name devnet in their host, and the
/// loopback form must match the committed harness's exact shape.
pub fn admit_url(url: &str) -> Result<()> {
    if url.to_ascii_lowercase().contains("mainnet") {
        return Err(format!("refusing mainnet-adjacent RPC URL: {url}").into());
    }
    if let Some(port) = url
        .strip_prefix("http://127.0.0.1:")
        .or_else(|| url.strip_prefix("http://localhost:"))
    {
        if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(format!("loopback RPC URL has an invalid port: {url}").into());
        }
        return Ok(());
    }
    let Some(rest) = url.strip_prefix("https://") else {
        return Err(format!(
            "refusing non-loopback non-HTTPS RPC URL: {url}"
        )
        .into());
    };
    let host = rest.split(['/', ':']).next().unwrap_or_default();
    if host.contains("devnet") {
        Ok(())
    } else {
        Err(format!("refusing RPC host that does not name devnet: {host}").into())
    }
}

/// One reloaded account, at confirmed commitment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountView {
    pub lamports: u64,
    pub owner: String,
    pub executable: bool,
    pub data: Vec<u8>,
}

pub struct Rpc {
    url: String,
    min_interval: Duration,
    last_call: Option<Instant>,
}

impl Rpc {
    pub fn new(url: &str, throttle_ms: u64) -> Self {
        Self {
            url: url.to_string(),
            min_interval: Duration::from_millis(throttle_ms),
            last_call: None,
        }
    }

    fn throttle(&mut self) {
        if let Some(last) = self.last_call {
            let remaining = self.min_interval.saturating_sub(last.elapsed());
            if !remaining.is_zero() {
                thread::sleep(remaining);
            }
        }
        self.last_call = Some(Instant::now());
    }

    /// One JSON-RPC call.  Transport failures (curl exit, malformed JSON) are
    /// retried with backoff; an in-protocol `error` object is returned to the
    /// caller immediately because retrying a semantic refusal is noise.
    pub fn call(&mut self, method: &str, params: &Value) -> Result<Value> {
        let body = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
        let mut last_error: Option<String> = None;
        for attempt in 0..4_u32 {
            if attempt > 0 {
                thread::sleep(Duration::from_millis(1_500 * u64::from(attempt)));
            }
            self.throttle();
            let output = Command::new("curl")
                .args([
                    "-fsS",
                    "--max-time",
                    "60",
                    "-H",
                    "Content-Type: application/json",
                    "-X",
                    "POST",
                    "--data-binary",
                    &body.to_string(),
                    &self.url,
                ])
                .output()?;
            if !output.status.success() {
                last_error = Some(format!(
                    "curl failed for {method}: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
                continue;
            }
            let response: Value = match serde_json::from_slice(&output.stdout) {
                Ok(response) => response,
                Err(error) => {
                    last_error = Some(format!("{method} returned malformed JSON: {error}"));
                    continue;
                }
            };
            if let Some(error) = response.get("error") {
                return Err(format!("RPC error for {method}: {error}").into());
            }
            return response
                .get("result")
                .cloned()
                .ok_or_else(|| format!("RPC response for {method} has no result").into());
        }
        Err(last_error
            .unwrap_or_else(|| format!("{method} failed with no recorded error"))
            .into())
    }

    pub fn genesis_hash(&mut self) -> Result<String> {
        self.call("getGenesisHash", &json!([]))?
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| "getGenesisHash returned no hash".into())
    }

    pub fn slot(&mut self) -> Result<u64> {
        self.call("getSlot", &json!([{"commitment": "confirmed"}]))?
            .as_u64()
            .ok_or_else(|| "getSlot returned no slot".into())
    }

    pub fn latest_blockhash(&mut self) -> Result<String> {
        self.call(
            "getLatestBlockhash",
            &json!([{"commitment": "confirmed"}]),
        )?
        .get("value")
        .and_then(|value| value.get("blockhash"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "getLatestBlockhash returned no blockhash".into())
    }

    pub fn is_blockhash_valid(&mut self, blockhash: &str) -> Result<bool> {
        self.call(
            "isBlockhashValid",
            &json!([blockhash, {"commitment": "confirmed"}]),
        )?
        .get("value")
        .and_then(Value::as_bool)
        .ok_or_else(|| "isBlockhashValid returned no verdict".into())
    }

    pub fn minimum_rent(&mut self, data_len: usize) -> Result<u64> {
        self.call("getMinimumBalanceForRentExemption", &json!([data_len]))?
            .as_u64()
            .ok_or_else(|| "getMinimumBalanceForRentExemption returned no value".into())
    }

    pub fn balance(&mut self, address: &str) -> Result<u64> {
        self.call(
            "getBalance",
            &json!([address, {"commitment": "confirmed"}]),
        )?
        .get("value")
        .and_then(Value::as_u64)
        .ok_or_else(|| "getBalance returned no value".into())
    }

    /// Reload one account at confirmed commitment; `None` when absent.
    /// `slice_empty` fetches metadata only (used for the executable program
    /// account, whose ELF bytes would otherwise ride along).
    pub fn account(&mut self, address: &str, slice_empty: bool) -> Result<Option<AccountView>> {
        use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
        let mut config = json!({"encoding": "base64", "commitment": "confirmed"});
        if slice_empty {
            config["dataSlice"] = json!({"offset": 0, "length": 0});
        }
        let result = self.call("getAccountInfo", &json!([address, config]))?;
        let Some(value) = result.get("value").filter(|value| !value.is_null()) else {
            return Ok(None);
        };
        let encoded = value
            .get("data")
            .and_then(Value::as_array)
            .and_then(|parts| parts.first())
            .and_then(Value::as_str)
            .ok_or_else(|| format!("account {address} returned no base64 data"))?;
        Ok(Some(AccountView {
            lamports: value
                .get("lamports")
                .and_then(Value::as_u64)
                .ok_or_else(|| format!("account {address} returned no lamports"))?,
            owner: value
                .get("owner")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("account {address} returned no owner"))?
                .to_string(),
            executable: value
                .get("executable")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            data: BASE64.decode(encoded)?,
        }))
    }

    pub fn request_airdrop(&mut self, address: &str, lamports: u64) -> Result<String> {
        self.call("requestAirdrop", &json!([address, lamports]))?
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| "requestAirdrop returned no signature".into())
    }

    /// Submit signed wire bytes.  Preflight stays disabled so an expected
    /// refusal is recorded by the cluster itself rather than simulated away.
    pub fn send_transaction(&mut self, wire: &[u8]) -> Result<String> {
        use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
        self.call(
            "sendTransaction",
            &json!([BASE64.encode(wire), {
                "encoding": "base64",
                "skipPreflight": true,
                "preflightCommitment": "confirmed",
                "maxRetries": 0
            }]),
        )?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| "sendTransaction returned no signature".into())
    }

    pub fn signature_status(&mut self, signature: &str) -> Result<Option<Value>> {
        let result = self.call(
            "getSignatureStatuses",
            &json!([[signature], {"searchTransactionHistory": true}]),
        )?;
        Ok(result
            .get("value")
            .and_then(Value::as_array)
            .and_then(|values| values.first())
            .filter(|status| !status.is_null())
            .cloned())
    }

    /// Submit and confirm one signed transaction whose recent blockhash is
    /// `blockhash`.  The same wire bytes are resubmitted periodically (safe:
    /// same signature) until the status reaches confirmed, the blockhash
    /// expires, or the deadline passes.  `Ok(None)` means the caller should
    /// re-sign against a fresh blockhash and try again.
    pub fn submit_and_confirm(
        &mut self,
        wire: &[u8],
        blockhash: &str,
    ) -> Result<Option<(String, Value)>> {
        let signature = self.send_transaction(wire)?;
        let deadline = Instant::now() + Duration::from_secs(90);
        let mut last_resubmit = Instant::now();
        while Instant::now() < deadline {
            if let Some(status) = self.signature_status(&signature)? {
                let confirmation = status
                    .get("confirmationStatus")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if confirmation == "confirmed" || confirmation == "finalized" {
                    return Ok(Some((signature, status)));
                }
            } else if !self.is_blockhash_valid(blockhash)? {
                /* The blockhash is dead and the cluster has never seen the
                 * signature: the transaction can no longer land, so a
                 * re-signed retry is safe from double execution. */
                if self.signature_status(&signature)?.is_none() {
                    return Ok(None);
                }
            }
            if last_resubmit.elapsed() > Duration::from_secs(10) {
                let _ = self.send_transaction(wire);
                last_resubmit = Instant::now();
            }
            thread::sleep(Duration::from_millis(1_200));
        }
        Err(format!("transaction {signature} did not confirm before the deadline").into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devnet_and_loopback_urls_are_admitted() {
        assert!(admit_url("https://api.devnet.solana.com").is_ok());
        assert!(admit_url("https://devnet.helius-rpc.com/?api-key=x").is_ok());
        assert!(admit_url("http://127.0.0.1:18939").is_ok());
        assert!(admit_url("http://localhost:8899").is_ok());
    }

    #[test]
    fn mainnet_and_ambiguous_urls_are_refused() {
        assert!(admit_url("https://api.mainnet-beta.solana.com").is_err());
        assert!(admit_url("https://api.MAINNET-beta.solana.com").is_err());
        assert!(admit_url("https://api.testnet.solana.com").is_err());
        assert!(admit_url("http://api.devnet.solana.com").is_err());
        assert!(admit_url("https://example.com/devnet").is_err());
        assert!(admit_url("http://127.0.0.1:").is_err());
        assert!(admit_url("http://127.0.0.1:80x").is_err());
    }

    #[test]
    fn the_genesis_guard_constants_are_distinct() {
        assert_ne!(MAINNET_GENESIS, DEVNET_GENESIS);
    }
}
