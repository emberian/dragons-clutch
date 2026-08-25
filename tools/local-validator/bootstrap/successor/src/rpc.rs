use std::{net::IpAddr, thread, time::Duration};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use reqwest::{Url, blocking::Client, redirect::Policy};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use solana_sdk::{
    hash::Hash,
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
    transaction::Transaction,
};

use crate::{
    Error, Result,
    model::{AccountEvidence, TransactionEvidence},
    plan::{hex, pubkey},
};

const LOCAL_PROTOCOL_COMPUTE_UNIT_LIMIT: u32 = 1_400_000;

#[derive(Clone, Debug)]
pub(crate) struct RpcAccount {
    pub(crate) lamports: u64,
    pub(crate) owner: Pubkey,
    pub(crate) executable: bool,
    pub(crate) rent_epoch: u64,
    pub(crate) data: Vec<u8>,
}

pub(crate) struct Rpc {
    url: Url,
    client: Client,
    request_id: u64,
}

impl Rpc {
    pub(crate) fn connect(value: &str) -> Result<Self> {
        let url = validate_loopback_url(value)?;
        let client = Client::builder()
            .no_proxy()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|error| Error::new(format!("build RPC client: {error}")))?;
        let mut rpc = Self {
            url,
            client,
            request_id: 0,
        };
        let health = rpc.call("getHealth", &json!([]))?;
        if health != Value::String("ok".into()) {
            return Err(Error::new(format!("local RPC health refused: {health}")));
        }
        Ok(rpc)
    }

    pub(crate) fn url(&self) -> &str {
        self.url.as_str()
    }

    pub(crate) fn call(&mut self, method: &str, params: &Value) -> Result<Value> {
        self.request_id = self
            .request_id
            .checked_add(1)
            .ok_or_else(|| Error::new("RPC request ID overflow"))?;
        let response = self
            .client
            .post(self.url.clone())
            .json(&json!({
                "jsonrpc": "2.0",
                "id": self.request_id,
                "method": method,
                "params": params,
            }))
            .send()
            .map_err(|error| Error::new(format!("{method} transport: {error}")))?;
        if !response.status().is_success() {
            return Err(Error::new(format!(
                "{method} returned HTTP {}",
                response.status()
            )));
        }
        let body: Value = response
            .json()
            .map_err(|error| Error::new(format!("{method} JSON: {error}")))?;
        if let Some(error) = body.get("error") {
            return Err(Error::new(format!("{method} RPC error: {error}")));
        }
        body.get("result")
            .cloned()
            .ok_or_else(|| Error::new(format!("{method} response omitted result")))
    }

    pub(crate) fn account(&mut self, address: Pubkey) -> Result<Option<RpcAccount>> {
        let value = self.call(
            "getAccountInfo",
            &json!([address.to_string(), {"encoding":"base64","commitment":"finalized"}]),
        )?;
        let Some(account) = value.get("value") else {
            return Err(Error::new("getAccountInfo omitted value"));
        };
        if account.is_null() {
            return Ok(None);
        }
        let encoded = account
            .get("data")
            .and_then(Value::as_array)
            .and_then(|values| values.first())
            .and_then(Value::as_str)
            .ok_or_else(|| Error::new("getAccountInfo omitted base64 data"))?;
        let data = BASE64
            .decode(encoded)
            .map_err(|error| Error::new(format!("account base64: {error}")))?;
        Ok(Some(RpcAccount {
            lamports: u64_field(account, "lamports")?,
            owner: pubkey(string_field(account, "owner")?)?,
            executable: account
                .get("executable")
                .and_then(Value::as_bool)
                .ok_or_else(|| Error::new("account omitted executable"))?,
            rent_epoch: u64_field(account, "rentEpoch")?,
            data,
        }))
    }

    pub(crate) fn required_account(&mut self, address: Pubkey, label: &str) -> Result<RpcAccount> {
        self.account(address)?
            .ok_or_else(|| Error::new(format!("missing {label} account {address}")))
    }

    pub(crate) fn airdrop(
        &mut self,
        label: &str,
        address: Pubkey,
        lamports: u64,
    ) -> Result<TransactionEvidence> {
        let signature = self
            .call("requestAirdrop", &json!([address.to_string(), lamports]))?
            .as_str()
            .ok_or_else(|| Error::new("requestAirdrop result was not a signature"))?
            .parse::<Signature>()
            .map_err(|error| Error::new(format!("airdrop signature: {error}")))?;
        self.confirm_airdrop(label, signature)
    }

    pub(crate) fn send(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: &Keypair,
    ) -> Result<TransactionEvidence> {
        self.send_inner(label, instructions, payer, false)
    }

    pub(crate) fn send_expected_failure(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: &Keypair,
    ) -> Result<TransactionEvidence> {
        self.send_inner(label, instructions, payer, true)
    }

    fn send_inner(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: &Keypair,
        expect_failure: bool,
    ) -> Result<TransactionEvidence> {
        let blockhash = self.latest_blockhash()?;
        let mut bounded_instructions = Vec::with_capacity(instructions.len().saturating_add(1));
        let mut compute_limit_data = Vec::with_capacity(5);
        compute_limit_data.push(2);
        compute_limit_data.extend_from_slice(&LOCAL_PROTOCOL_COMPUTE_UNIT_LIMIT.to_le_bytes());
        bounded_instructions.push(Instruction {
            program_id: solana_sdk_ids::compute_budget::ID,
            accounts: Vec::new(),
            data: compute_limit_data,
        });
        bounded_instructions.extend_from_slice(instructions);
        let transaction = Transaction::new_signed_with_payer(
            &bounded_instructions,
            Some(&payer.pubkey()),
            &[payer],
            blockhash,
        );
        let encoded = BASE64.encode(
            bincode::serialize(&transaction)
                .map_err(|error| Error::new(format!("serialize transaction: {error}")))?,
        );
        let signature = self
            .call(
                "sendTransaction",
                &json!([encoded, {
                    "encoding":"base64",
                    "skipPreflight": expect_failure,
                    "preflightCommitment":"confirmed",
                    "maxRetries": 8
                }]),
            )?
            .as_str()
            .ok_or_else(|| Error::new("sendTransaction result was not a signature"))?
            .parse::<Signature>()
            .map_err(|error| Error::new(format!("transaction signature: {error}")))?;
        self.confirm(label, signature, expect_failure)
    }

    fn latest_blockhash(&mut self) -> Result<Hash> {
        let value = self.call("getLatestBlockhash", &json!([{"commitment":"finalized"}]))?;
        value
            .get("value")
            .and_then(|value| value.get("blockhash"))
            .and_then(Value::as_str)
            .ok_or_else(|| Error::new("getLatestBlockhash omitted blockhash"))?
            .parse::<Hash>()
            .map_err(|error| Error::new(format!("blockhash: {error}")))
    }

    fn confirm(
        &mut self,
        label: &str,
        signature: Signature,
        expect_failure: bool,
    ) -> Result<TransactionEvidence> {
        let mut status = None;
        for _ in 0..120 {
            let result = self.call(
                "getSignatureStatuses",
                &json!([[signature.to_string()], {"searchTransactionHistory":true}]),
            )?;
            status = result
                .get("value")
                .and_then(Value::as_array)
                .and_then(|values| values.first())
                .filter(|value| !value.is_null())
                .cloned();
            if status
                .as_ref()
                .and_then(|value| value.get("confirmationStatus"))
                == Some(&Value::String("finalized".into()))
            {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
        let status = status.ok_or_else(|| {
            Error::new(format!(
                "{label} {signature} did not reach a visible status"
            ))
        })?;
        let status_error = status.get("err").cloned().filter(|value| !value.is_null());
        if expect_failure != status_error.is_some() {
            return Err(Error::new(format!(
                "{label} status contradicted expectation: {}",
                status.get("err").unwrap_or(&Value::Null)
            )));
        }
        let mut transaction = None;
        for _ in 0..120 {
            let candidate = self.call(
                "getTransaction",
                &json!([signature.to_string(), {
                    "encoding":"json",
                    "commitment":"finalized",
                    "maxSupportedTransactionVersion":0
                }]),
            )?;
            if candidate.get("meta").is_some() {
                transaction = Some(candidate);
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
        let transaction = transaction.ok_or_else(|| {
            Error::new(format!(
                "{label} {signature} did not reach finalized transaction history"
            ))
        })?;
        let meta = transaction
            .get("meta")
            .ok_or_else(|| Error::new(format!("{label} transaction omitted meta")))?;
        let meta_error = meta.get("err").cloned().filter(|value| !value.is_null());
        if meta_error != status_error {
            return Err(Error::new(format!(
                "{label} status and transaction errors differ"
            )));
        }
        let slot = transaction
            .get("slot")
            .and_then(Value::as_u64)
            .ok_or_else(|| Error::new(format!("{label} transaction omitted slot")))?;
        let fee_lamports = u64_field(meta, "fee")?;
        let compute_units_consumed = meta.get("computeUnitsConsumed").and_then(Value::as_u64);
        let logs = meta
            .get("logMessages")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        Ok(TransactionEvidence {
            label: label.into(),
            signature: signature.to_string(),
            slot,
            transaction_metadata_available: true,
            fee_lamports: Some(fee_lamports),
            compute_units_consumed,
            error: meta_error,
            logs,
        })
    }

    fn confirm_airdrop(
        &mut self,
        label: &str,
        signature: Signature,
    ) -> Result<TransactionEvidence> {
        let mut status = None;
        for _ in 0..120 {
            let result = self.call(
                "getSignatureStatuses",
                &json!([[signature.to_string()], {"searchTransactionHistory":true}]),
            )?;
            status = result
                .get("value")
                .and_then(Value::as_array)
                .and_then(|values| values.first())
                .filter(|value| !value.is_null())
                .cloned();
            if status
                .as_ref()
                .and_then(|value| value.get("confirmationStatus"))
                == Some(&Value::String("finalized".into()))
            {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
        let status = status.ok_or_else(|| {
            Error::new(format!(
                "{label} {signature} did not reach a visible status"
            ))
        })?;
        if let Some(error) = status.get("err").filter(|value| !value.is_null()) {
            return Err(Error::new(format!("{label} airdrop failed: {error}")));
        }
        Ok(TransactionEvidence {
            label: label.into(),
            signature: signature.to_string(),
            slot: u64_field(&status, "slot")?,
            transaction_metadata_available: false,
            fee_lamports: None,
            compute_units_consumed: None,
            error: None,
            logs: Vec::new(),
        })
    }
}

pub(crate) fn account_evidence(address: Pubkey, account: &RpcAccount) -> AccountEvidence {
    let data_sha256 = Sha256::digest(&account.data);
    let mut exact = Sha256::new();
    exact.update(account.owner.as_ref());
    exact.update(account.lamports.to_le_bytes());
    exact.update([u8::from(account.executable)]);
    exact.update(account.rent_epoch.to_le_bytes());
    exact.update(
        u64::try_from(account.data.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    exact.update(&account.data);
    AccountEvidence {
        address: address.to_string(),
        owner: account.owner.to_string(),
        lamports: account.lamports,
        executable: account.executable,
        data_len: account.data.len(),
        data_sha256: hex(&data_sha256),
        account_sha256: hex(&exact.finalize()),
    }
}

pub(crate) fn validate_loopback_url(value: &str) -> Result<Url> {
    let url = Url::parse(value).map_err(|error| Error::new(format!("RPC URL: {error}")))?;
    if url.scheme() != "http"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
        || url.port().is_none()
    {
        return Err(Error::new(
            "RPC URL must be a credential-free explicit-port loopback HTTP origin",
        ));
    }
    let host = url
        .host_str()
        .ok_or_else(|| Error::new("RPC URL omitted host"))?;
    let normalized = host.trim_start_matches('[').trim_end_matches(']');
    if !normalized.eq_ignore_ascii_case("localhost")
        && !normalized
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
    {
        return Err(Error::new("RPC URL host is not loopback"));
    }
    Ok(url)
}

fn string_field<'a>(value: &'a Value, name: &str) -> Result<&'a str> {
    value
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new(format!("JSON omitted string {name}")))
}

fn u64_field(value: &Value, name: &str) -> Result<u64> {
    value
        .get(name)
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new(format!("JSON omitted u64 {name}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_explicit_loopback_origins_are_admitted() {
        for value in ["http://127.0.0.1:20890/", "http://[::1]:20890/"] {
            assert!(validate_loopback_url(value).is_ok(), "{value}");
        }
        for value in [
            "https://127.0.0.1:20890/",
            "http://127.0.0.1/",
            "http://127.0.0.1:20890/path",
            "http://example.com:20890/",
            "http://user@127.0.0.1:20890/",
        ] {
            assert!(validate_loopback_url(value).is_err(), "{value}");
        }
    }
}
