//! Loopback-only JSON-RPC, submission, and confirmation.
//!
//! Deliberately narrower than a general Solana client, for the same reason
//! `clutch-sbf-committed-harness` is: this binary signs with real Ed25519 keys
//! and must never be pointed at a cluster.  [`Rpc::new`] refuses any URL that
//! is not `http://127.0.0.1:<port>` or `http://localhost:<port>` by
//! construction, before a keypair is ever read.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde_json::{json, Value};
use std::{
    process::Command,
    thread,
    time::{Duration, Instant},
};

/// A loopback RPC endpoint.
#[derive(Clone, Debug)]
pub struct Rpc {
    url: String,
}

/// One confirmed transaction's outcome.
#[derive(Clone, Debug)]
pub struct Confirmation {
    /// Transaction signature, base58.
    pub signature: String,
    /// Slot the bank committed it in.
    pub slot: u64,
    /// Compute units the bank charged, when the RPC reports them.
    pub compute_units: Option<u64>,
    /// `Custom(code)` of the instruction error, when the transaction failed
    /// with one.
    pub custom_code: Option<u64>,
    /// The raw error value; `Value::Null` on success.
    pub error: Value,
}

impl Confirmation {
    /// Whether the bank accepted the transaction.
    #[must_use]
    pub fn accepted(&self) -> bool {
        self.error.is_null()
    }
}

impl Rpc {
    /// Bind to a loopback URL.
    ///
    /// # Errors
    /// Returns an error for any non-loopback URL or an invalid port.
    pub fn new(url: &str) -> Result<Self, String> {
        let accepted = url
            .strip_prefix("http://127.0.0.1:")
            .or_else(|| url.strip_prefix("http://localhost:"));
        let Some(port) = accepted else {
            return Err(format!("refusing non-loopback RPC URL: {url}"));
        };
        if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(format!("loopback RPC URL has an invalid port: {url}"));
        }
        Ok(Self {
            url: url.to_string(),
        })
    }

    /// Issue one JSON-RPC call.
    ///
    /// # Errors
    /// Returns an error when `curl` fails, the body does not parse, or the
    /// response carries a JSON-RPC `error`.
    pub fn call(&self, method: &str, params: &Value) -> Result<Value, String> {
        let body = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
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
            .output()
            .map_err(|error| format!("curl could not run for {method}: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "curl failed for {method}: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let response: Value = serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("{method} response did not parse: {error}"))?;
        if let Some(error) = response.get("error") {
            return Err(format!("RPC error for {method}: {error}"));
        }
        response
            .get("result")
            .cloned()
            .ok_or_else(|| format!("RPC response for {method} has no result"))
    }

    /// The bank's current confirmed slot.
    ///
    /// # Errors
    /// Returns an error when the RPC call fails.
    pub fn slot(&self) -> Result<u64, String> {
        self.call("getSlot", &json!([{"commitment": "confirmed"}]))?
            .as_u64()
            .ok_or_else(|| "getSlot returned no slot".to_string())
    }

    /// Whether the endpoint is answering health checks.
    #[must_use]
    pub fn healthy(&self) -> bool {
        self.call("getHealth", &json!([]))
            .is_ok_and(|value| value.as_str() == Some("ok"))
    }

    /// The latest confirmed blockhash.
    ///
    /// # Errors
    /// Returns an error when the RPC call fails or the hash does not decode.
    pub fn blockhash(&self) -> Result<[u8; 32], String> {
        let result = self.call(
            "getLatestBlockhash",
            &json!([{"commitment": "confirmed"}]),
        )?;
        let text = result
            .get("value")
            .and_then(|value| value.get("blockhash"))
            .and_then(Value::as_str)
            .ok_or("getLatestBlockhash returned no blockhash")?;
        crate::wire::base58_decode_32(text)
    }

    /// One account's exact committed bytes, or `None` when it is absent.
    ///
    /// # Errors
    /// Returns an error when the RPC call fails or the encoding is unexpected.
    pub fn account(&self, address: &str) -> Result<Option<Vec<u8>>, String> {
        let result = self.call(
            "getAccountInfo",
            &json!([address, {"encoding": "base64", "commitment": "confirmed"}]),
        )?;
        let Some(value) = result.get("value").filter(|value| !value.is_null()) else {
            return Ok(None);
        };
        let encoded = value
            .get("data")
            .and_then(Value::as_array)
            .and_then(|parts| parts.first())
            .and_then(Value::as_str)
            .ok_or_else(|| format!("account {address} returned no base64 data"))?;
        BASE64
            .decode(encoded)
            .map(Some)
            .map_err(|error| format!("account {address} data did not decode: {error}"))
    }

    /// Every program-owned account of an exact length whose bytes at
    /// `offset` equal `needle`.
    ///
    /// This is how the keeper stays restart-safe past a page close: a
    /// reservation archive outlives the page that indexed it, so after the
    /// page is gone the only way to enumerate the archives is to ask the bank
    /// for them.  A keeper that cached the list instead would be unable to
    /// finish a walk it did not start.
    ///
    /// # Errors
    /// Returns an error when the RPC call fails or a row does not decode.
    pub fn program_accounts(
        &self,
        program: &str,
        data_size: usize,
        offset: usize,
        needle: &[u8; 32],
    ) -> Result<Vec<(String, Vec<u8>)>, String> {
        let result = self.call(
            "getProgramAccounts",
            &json!([program, {
                "encoding": "base64",
                "commitment": "confirmed",
                "filters": [
                    {"dataSize": data_size},
                    {"memcmp": {"offset": offset, "bytes": crate::wire::base58(needle)}}
                ]
            }]),
        )?;
        let rows = result
            .as_array()
            .ok_or("getProgramAccounts returned no array")?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let address = row
                .get("pubkey")
                .and_then(Value::as_str)
                .ok_or("a program account row has no pubkey")?
                .to_string();
            let encoded = row
                .get("account")
                .and_then(|account| account.get("data"))
                .and_then(Value::as_array)
                .and_then(|parts| parts.first())
                .and_then(Value::as_str)
                .ok_or("a program account row has no base64 data")?;
            let bytes = BASE64
                .decode(encoded)
                .map_err(|error| format!("{address} data did not decode: {error}"))?;
            out.push((address, bytes));
        }
        out.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(out)
    }

    /// One account's lamport balance; zero when absent.
    ///
    /// # Errors
    /// Returns an error when the RPC call fails.
    pub fn lamports(&self, address: &str) -> Result<u64, String> {
        let result = self.call(
            "getAccountInfo",
            &json!([address, {"encoding": "base64", "commitment": "confirmed"}]),
        )?;
        Ok(result
            .get("value")
            .filter(|value| !value.is_null())
            .and_then(|value| value.get("lamports"))
            .and_then(Value::as_u64)
            .unwrap_or(0))
    }

    /// Airdrop lamports to an address and wait for the confirmation.
    ///
    /// # Errors
    /// Returns an error when the faucet call or its confirmation fails.
    pub fn airdrop(&self, address: &str, lamports: u64) -> Result<(), String> {
        let signature = self
            .call("requestAirdrop", &json!([address, lamports]))?
            .as_str()
            .map(str::to_string)
            .ok_or("requestAirdrop returned no signature")?;
        self.await_confirmation(&signature)?;
        Ok(())
    }

    /// Submit a signed transaction with preflight disabled.
    ///
    /// Preflight is off so that an expected refusal is itself recorded by the
    /// local bank rather than being simulated away — the keeper's benign
    /// already-done codes must come from a committed transaction.
    ///
    /// # Errors
    /// Returns an error when the RPC refuses the submission, which is exactly
    /// what an over-budget packet earns.
    pub fn submit(&self, transaction: &[u8]) -> Result<String, String> {
        let encoded = BASE64.encode(transaction);
        let result = self.call(
            "sendTransaction",
            &json!([encoded, {
                "encoding": "base64",
                "skipPreflight": true,
                "preflightCommitment": "processed",
                "maxRetries": 0
            }]),
        )?;
        result
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| "sendTransaction returned no signature".to_string())
    }

    /// Block until one signature reaches `confirmed`.
    ///
    /// # Errors
    /// Returns an error when the signature does not confirm within 45 seconds.
    pub fn await_confirmation(&self, signature: &str) -> Result<Confirmation, String> {
        let deadline = Instant::now() + Duration::from_secs(45);
        while Instant::now() < deadline {
            let result = self.call(
                "getSignatureStatuses",
                &json!([[signature], {"searchTransactionHistory": true}]),
            )?;
            if let Some(status) = result
                .get("value")
                .and_then(Value::as_array)
                .and_then(|values| values.first())
                .filter(|status| !status.is_null())
            {
                let confirmation = status
                    .get("confirmationStatus")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if confirmation == "confirmed" || confirmation == "finalized" {
                    let error = status.get("err").cloned().unwrap_or(Value::Null);
                    let slot = status
                        .get("slot")
                        .and_then(Value::as_u64)
                        .ok_or("confirmation carries no slot")?;
                    return Ok(Confirmation {
                        compute_units: self.compute_units(signature),
                        custom_code: custom_error_code(&error),
                        signature: signature.to_string(),
                        slot,
                        error,
                    });
                }
            }
            thread::sleep(Duration::from_millis(100));
        }
        Err(format!(
            "transaction {signature} did not confirm within 45 seconds"
        ))
    }

    /// Submit, confirm, and report in one step.
    ///
    /// # Errors
    /// Returns an error when submission or confirmation fails.
    pub fn submit_and_confirm(&self, transaction: &[u8]) -> Result<Confirmation, String> {
        let signature = self.submit(transaction)?;
        self.await_confirmation(&signature)
    }

    /// The program's own log lines for one committed transaction.
    ///
    /// A bare `ProgramFailedToComplete` says nothing a caller can act on; the
    /// program's messages say which check refused and at what cost, so a
    /// keeper that stops carries them into its error.
    #[must_use]
    pub fn logs(&self, signature: &str) -> Vec<String> {
        let Ok(result) = self.call(
            "getTransaction",
            &json!([signature, {
                "encoding": "base64",
                "commitment": "confirmed",
                "maxSupportedTransactionVersion": 0
            }]),
        ) else {
            return Vec::new();
        };
        result
            .get("meta")
            .and_then(|meta| meta.get("logMessages"))
            .and_then(Value::as_array)
            .map(|lines| {
                lines
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn compute_units(&self, signature: &str) -> Option<u64> {
        let result = self
            .call(
                "getTransaction",
                &json!([signature, {
                    "encoding": "base64",
                    "commitment": "confirmed",
                    "maxSupportedTransactionVersion": 0
                }]),
            )
            .ok()?;
        result.get("meta")?.get("computeUnitsConsumed")?.as_u64()
    }
}

/// Extract the `Custom(code)` of an instruction error.
#[must_use]
pub fn custom_error_code(error: &Value) -> Option<u64> {
    error
        .get("InstructionError")?
        .as_array()?
        .get(1)?
        .get("Custom")?
        .as_u64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_exact_loopback_urls_are_admitted() {
        assert!(Rpc::new("http://127.0.0.1:9001").is_ok());
        assert!(Rpc::new("http://localhost:9001").is_ok());
        assert!(Rpc::new("https://127.0.0.1:9001").is_err());
        assert!(Rpc::new("http://127.0.0.1.example:9001").is_err());
        assert!(Rpc::new("http://api.mainnet-beta.solana.com:80").is_err());
        assert!(Rpc::new("http://127.0.0.1:").is_err());
    }

    #[test]
    fn custom_error_extraction_is_structural() {
        assert_eq!(
            custom_error_code(&json!({"InstructionError": [1, {"Custom": 0x40}]})),
            Some(0x40)
        );
        assert_eq!(
            custom_error_code(&json!({"InstructionError": [1, "InvalidAccountData"]})),
            None
        );
        assert_eq!(custom_error_code(&Value::Null), None);
    }
}
