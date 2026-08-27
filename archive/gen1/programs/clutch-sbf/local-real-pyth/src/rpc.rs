//! Minimal loopback-only JSON-RPC client for the local laboratory validator.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde_json::{json, Value};
use std::{
    process::Command,
    thread,
    time::{Duration, Instant},
};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountView {
    pub lamports: u64,
    pub owner: String,
    pub executable: bool,
    pub data: Vec<u8>,
}

pub fn require_loopback(url: &str) -> Result<()> {
    let Some(port) = url.strip_prefix("http://127.0.0.1:") else {
        return Err(format!("refusing non-loopback RPC URL: {url}").into());
    };
    if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("loopback RPC URL has an invalid port: {url}").into());
    }
    let number: u16 = port
        .parse()
        .map_err(|_| format!("loopback RPC URL has an out-of-range port: {url}"))?;
    if number == 0 || number.to_string() != port {
        return Err(format!("loopback RPC URL port is not canonical: {url}").into());
    }
    Ok(())
}

pub struct Rpc {
    url: String,
}

impl Rpc {
    pub fn new(url: &str) -> Result<Self> {
        require_loopback(url)?;
        Ok(Self {
            url: url.to_string(),
        })
    }

    fn call(&self, method: &str, params: &Value) -> Result<Value> {
        let body = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
        let output = Command::new("curl")
            .args([
                "-q",
                "-fsS",
                "--max-time",
                "60",
                "--noproxy",
                "*",
                "--proxy",
                "",
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
            return Err(format!(
                "curl failed for {method}: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }
        let response: Value = serde_json::from_slice(&output.stdout)?;
        if response.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
            || response.get("id").and_then(Value::as_u64) != Some(1)
        {
            return Err(format!("RPC response for {method} has a malformed envelope").into());
        }
        let has_result = response.get("result").is_some();
        let has_error = response.get("error").is_some_and(|error| !error.is_null());
        if has_result == has_error {
            return Err(format!(
                "RPC response for {method} must carry exactly one of result or non-null error"
            )
            .into());
        }
        if let Some(error) = response.get("error").filter(|error| !error.is_null()) {
            return Err(format!("RPC error for {method}: {error}").into());
        }
        response
            .get("result")
            .cloned()
            .ok_or_else(|| format!("RPC response for {method} has no result").into())
    }

    pub fn genesis_hash(&self) -> Result<String> {
        self.call("getGenesisHash", &json!([]))?
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| "getGenesisHash returned no hash".into())
    }

    pub fn slot(&self) -> Result<u64> {
        self.call("getSlot", &json!([{"commitment": "confirmed"}]))?
            .as_u64()
            .ok_or_else(|| "getSlot returned no slot".into())
    }

    pub fn latest_blockhash(&self) -> Result<String> {
        self.call("getLatestBlockhash", &json!([{"commitment": "confirmed"}]))?
            .get("value")
            .and_then(|value| value.get("blockhash"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| "getLatestBlockhash returned no blockhash".into())
    }

    pub fn blockhash_valid(&self, blockhash: &str) -> Result<bool> {
        self.call(
            "isBlockhashValid",
            &json!([blockhash, {"commitment": "confirmed"}]),
        )?
        .get("value")
        .and_then(Value::as_bool)
        .ok_or_else(|| "isBlockhashValid returned no verdict".into())
    }

    pub fn minimum_rent(&self, data_len: usize) -> Result<u64> {
        self.call("getMinimumBalanceForRentExemption", &json!([data_len]))?
            .as_u64()
            .ok_or_else(|| "rent RPC returned no value".into())
    }

    pub fn account(&self, address: &str) -> Result<Option<AccountView>> {
        let result = self.call(
            "getAccountInfo",
            &json!([address, {"encoding": "base64", "commitment": "confirmed"}]),
        )?;
        let value = result
            .as_object()
            .and_then(|result| result.get("value"))
            .ok_or_else(|| format!("account {address} response has no explicit value field"))?;
        if value.is_null() {
            return Ok(None);
        }
        let parts = value
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("account {address} returned no data tuple"))?;
        if parts.len() != 2 {
            return Err(format!(
                "account {address} returned a {}-element data tuple, expected exactly 2",
                parts.len()
            )
            .into());
        }
        let encoded = parts
            .first()
            .and_then(Value::as_str)
            .ok_or_else(|| format!("account {address} returned no base64 body"))?;
        let encoding = parts
            .get(1)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("account {address} returned no data encoding tag"))?;
        if encoding != "base64" {
            return Err(
                format!("account {address} returned encoding {encoding}, not base64").into(),
            );
        }
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
                .ok_or_else(|| format!("account {address} returned no executable bit"))?,
            data: BASE64.decode(encoded)?,
        }))
    }

    fn send_transaction(&self, wire: &[u8], expected_signature: &str) -> Result<String> {
        let signature = self
            .call(
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
            .ok_or("sendTransaction returned no signature")?;
        if signature != expected_signature {
            return Err(format!(
                "sendTransaction returned {signature}, expected locally signed {expected_signature}"
            )
            .into());
        }
        Ok(signature)
    }

    fn signature_status(&self, signature: &str) -> Result<Option<Value>> {
        let result = self.call(
            "getSignatureStatuses",
            &json!([[signature], {"searchTransactionHistory": true}]),
        )?;
        let values = result
            .get("value")
            .and_then(Value::as_array)
            .ok_or("getSignatureStatuses returned no value array")?;
        if values.len() != 1 {
            return Err(format!("getSignatureStatuses returned {} entries", values.len()).into());
        }
        Ok(values.first().filter(|status| !status.is_null()).cloned())
    }

    pub fn transaction(&self, signature: &str) -> Result<Value> {
        for _attempt in 0..20 {
            let result = self.call(
                "getTransaction",
                &json!([signature, {
                    "encoding": "json",
                    "commitment": "confirmed",
                    "maxSupportedTransactionVersion": 0
                }]),
            )?;
            if !result.is_null() {
                return Ok(result);
            }
            thread::sleep(Duration::from_millis(250));
        }
        Err(format!("confirmed transaction {signature} is unavailable").into())
    }

    /// Submit fixed signed bytes until confirmed. `None` means the blockhash
    /// expired without the signature ever being observed, so the caller may
    /// safely rebuild and re-sign once.
    pub fn submit_and_confirm(
        &self,
        wire: &[u8],
        blockhash: &str,
        expected_signature: &str,
    ) -> Result<Option<(String, Value)>> {
        let signature = self.send_transaction(wire, expected_signature)?;
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
            } else if !self.blockhash_valid(blockhash)?
                && self.signature_status(&signature)?.is_none()
            {
                return Ok(None);
            }
            if last_resubmit.elapsed() > Duration::from_secs(10) {
                let _ignored = self.send_transaction(wire, expected_signature);
                last_resubmit = Instant::now();
            }
            thread::sleep(Duration::from_millis(500));
        }
        Err(format!("transaction {signature} did not confirm before timeout").into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_loopback_http_is_admitted() {
        assert!(require_loopback("http://127.0.0.1:18537").is_ok());
        assert!(require_loopback("http://localhost:8899").is_err());
        assert!(require_loopback("https://127.0.0.1:8899").is_err());
        assert!(require_loopback("https://api.devnet.solana.com").is_err());
        assert!(require_loopback("http://127.0.0.1.example:8899").is_err());
        assert!(require_loopback("http://127.0.0.1:0").is_err());
        assert!(require_loopback("http://127.0.0.1:018537").is_err());
        assert!(require_loopback("http://127.0.0.1:65536").is_err());
    }
}
