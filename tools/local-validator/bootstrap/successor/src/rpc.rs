use std::{net::IpAddr, thread, time::Duration};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_versioned_message_operator::{Finality, Observation, ObservedAccount};
use reqwest::{Url, blocking::Client, redirect::Policy};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use solana_sdk::{
    hash::Hash,
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
    transaction::{Transaction, VersionedTransaction},
};

use crate::{
    Error, Result,
    model::{AccountEvidence, TransactionEvidence},
    plan::{hex, pubkey},
};

const LOCAL_PROTOCOL_COMPUTE_UNIT_LIMIT: u32 = 1_400_000;

/// Why this campaign does NOT request a larger heap frame.
///
/// `DCLTPCB1` exhausts the program heap entering its third stage. Requesting a
/// 256 KiB heap frame was tried and **measured to change nothing**: the
/// transaction carried the instruction, the runtime accepted it, and the route
/// failed at the same point.
///
/// The reason is that `RequestHeapFrame` raises the heap region the runtime
/// grants, while the stock allocator never asks how big it is:
/// `solana-program-entrypoint` builds its `BumpAllocator` with
/// `len: HEAP_LENGTH`, and `HEAP_LENGTH` is the compile-time constant
/// `32 * 1024` (`solana-program-entrypoint-3.1.1/src/lib.rs:39,226`). Every
/// dClutch program uses that entrypoint, so no transaction-level declaration
/// can move the bound.
///
/// This is therefore a **program-side** bound, and it is recorded against
/// `projected_custody_bootstrap_v1` rather than papered over with an
/// instruction that only looks like a fix.

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

    pub(crate) fn finalized_accounts(
        &mut self,
        addresses: &[Pubkey],
        minimum_slot: u64,
    ) -> Result<(u64, Vec<Option<RpcAccount>>)> {
        if addresses.is_empty() || addresses.len() > 100 {
            return Err(Error::new(
                "getMultipleAccounts requires one through 100 exact addresses",
            ));
        }
        let value = self.call(
            "getMultipleAccounts",
            &json!([addresses.iter().map(ToString::to_string).collect::<Vec<_>>(), {
                "encoding":"base64",
                "commitment":"finalized",
                "minContextSlot":minimum_slot
            }]),
        )?;
        let slot = value
            .get("context")
            .and_then(|context| context.get("slot"))
            .and_then(Value::as_u64)
            .ok_or_else(|| Error::new("getMultipleAccounts omitted context slot"))?;
        if slot < minimum_slot {
            return Err(Error::new(
                "getMultipleAccounts returned a snapshot before the required transaction",
            ));
        }
        let values = value
            .get("value")
            .and_then(Value::as_array)
            .ok_or_else(|| Error::new("getMultipleAccounts omitted values"))?;
        if values.len() != addresses.len() {
            return Err(Error::new(
                "getMultipleAccounts response width differed from request",
            ));
        }
        let accounts = values
            .iter()
            .map(parse_optional_account)
            .collect::<Result<Vec<_>>>()?;
        Ok((slot, accounts))
    }

    /// Reacquire one finalized account as an exact routing observation.
    ///
    /// Address lookup tables are transaction routing data, never protocol
    /// authority. The observation is still finalized and slot-pinned so the
    /// shared compiler can refuse a table extended in the observed slot.
    pub(crate) fn finalized_observed_accounts(
        &mut self,
        addresses: &[Pubkey],
        minimum_slot: u64,
    ) -> Result<(Observation, Vec<ObservedAccount>)> {
        let (slot, accounts) = self.finalized_accounts(addresses, minimum_slot)?;
        let observation = Observation {
            slot,
            unix_timestamp: self.block_time(slot)?,
            finality: Finality::Finalized,
        };
        let mut observed = Vec::with_capacity(addresses.len());
        for (key, account) in addresses.iter().copied().zip(accounts) {
            let account = account
                .ok_or_else(|| Error::new(format!("finalized observation missing {key}")))?;
            observed.push(ObservedAccount {
                observation,
                key,
                owner: account.owner,
                lamports: account.lamports,
                executable: account.executable,
                data: account.data,
            });
        }
        Ok((observation, observed))
    }

    pub(crate) fn block_time(&mut self, slot: u64) -> Result<i64> {
        self.call("getBlockTime", &json!([slot]))?
            .as_i64()
            .ok_or_else(|| Error::new("getBlockTime result was not an integer"))
    }

    pub(crate) fn finalized_slot(&mut self) -> Result<u64> {
        self.call("getSlot", &json!([{"commitment":"finalized"}]))?
            .as_u64()
            .ok_or_else(|| Error::new("getSlot result was not a u64"))
    }

    pub(crate) fn minimum_balance(&mut self, data_len: usize) -> Result<u64> {
        self.call(
            "getMinimumBalanceForRentExemption",
            &json!([data_len, {"commitment":"finalized"}]),
        )?
        .as_u64()
        .ok_or_else(|| Error::new("rent minimum result was not a u64"))
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

    pub(crate) fn send_with_signers(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: &Keypair,
        additional_signers: &[&Keypair],
    ) -> Result<TransactionEvidence> {
        self.send_inner_with_signers(label, instructions, payer, additional_signers, false)
    }

    pub(crate) fn send_expected_failure(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: &Keypair,
    ) -> Result<TransactionEvidence> {
        self.send_inner(label, instructions, payer, true)
    }

    /// Submit one packet-safe v0 transaction routed through finalized tables.
    ///
    /// The canonical Found and generic-founding frames exceed the 1,232-byte
    /// legacy packet with their account keys inline; the shared versioned
    /// message operator owns table admission and packet geometry.
    pub(crate) fn send_v0(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: &Keypair,
        observation: Observation,
        tables: &[ObservedAccount],
    ) -> Result<TransactionEvidence> {
        self.send_v0_inner(label, instructions, payer, &[], observation, tables, false)
    }

    pub(crate) fn send_v0_expected_failure(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: &Keypair,
        observation: Observation,
        tables: &[ObservedAccount],
    ) -> Result<TransactionEvidence> {
        self.send_v0_inner(label, instructions, payer, &[], observation, tables, true)
    }

    /// Submit one routed v0 transaction expected to refuse, carrying the exact
    /// signatures its frame requires.
    ///
    /// A hostile case must differ from the honest one in exactly the coordinate
    /// under test. If it also drops a signature the frame needs, the transaction
    /// never reaches the chain and the refusal proves nothing.
    pub(crate) fn send_v0_expected_failure_with_signers(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: &Keypair,
        additional_signers: &[&Keypair],
        observation: Observation,
        tables: &[ObservedAccount],
    ) -> Result<TransactionEvidence> {
        self.send_v0_inner(
            label,
            instructions,
            payer,
            additional_signers,
            observation,
            tables,
            true,
        )
    }

    /// Submit one routed v0 transaction carrying additional exact signers.
    ///
    /// A routed frame can still require a signature that is not the fee
    /// payer's: the projected-Custody bootstrap needs the principal supplier to
    /// sign while remaining non-writable, which the fee payer cannot do.
    pub(crate) fn send_v0_with_signers(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: &Keypair,
        additional_signers: &[&Keypair],
        observation: Observation,
        tables: &[ObservedAccount],
    ) -> Result<TransactionEvidence> {
        self.send_v0_inner(
            label,
            instructions,
            payer,
            additional_signers,
            observation,
            tables,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn send_v0_inner(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: &Keypair,
        additional_signers: &[&Keypair],
        observation: Observation,
        tables: &[ObservedAccount],
        expect_failure: bool,
    ) -> Result<TransactionEvidence> {
        let blockhash = self.latest_blockhash()?;
        let bounded = bounded_instructions(instructions);
        let plan = dclutch_versioned_message_operator::compile_v0_message(
            payer.pubkey(),
            &bounded,
            solana_hash::Hash::new_from_array(blockhash.to_bytes()),
            observation,
            tables,
        )
        .map_err(|error| Error::new(format!("{label}: v0 message compilation: {error:?}")))?;
        let mut signers: Vec<&dyn Signer> = Vec::with_capacity(additional_signers.len() + 1);
        signers.push(payer);
        signers.extend(
            additional_signers
                .iter()
                .map(|signer| *signer as &dyn Signer),
        );
        let transaction = VersionedTransaction::try_new(plan.message, &signers)
            .map_err(|error| Error::new(format!("{label}: sign v0 transaction: {error}")))?;
        let signature = self.submit(label, &transaction, expect_failure)?;
        self.confirm(label, signature, expect_failure)
    }

    fn submit<T: serde::Serialize>(
        &mut self,
        label: &str,
        transaction: &T,
        expect_failure: bool,
    ) -> Result<Signature> {
        let encoded = BASE64.encode(
            bincode::serialize(transaction)
                .map_err(|error| Error::new(format!("serialize transaction: {error}")))?,
        );
        self.call(
            "sendTransaction",
            &json!([encoded, {
                "encoding":"base64",
                "skipPreflight": expect_failure,
                "preflightCommitment":"confirmed",
                "maxRetries": 8
            }]),
        )
        .map_err(|error| Error::new(format!("{label}: {error}")))?
        .as_str()
        .ok_or_else(|| Error::new("sendTransaction result was not a signature"))?
        .parse::<Signature>()
        .map_err(|error| Error::new(format!("transaction signature: {error}")))
    }

    fn send_inner(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: &Keypair,
        expect_failure: bool,
    ) -> Result<TransactionEvidence> {
        self.send_inner_with_signers(label, instructions, payer, &[], expect_failure)
    }

    fn send_inner_with_signers(
        &mut self,
        label: &str,
        instructions: &[Instruction],
        payer: &Keypair,
        additional_signers: &[&Keypair],
        expect_failure: bool,
    ) -> Result<TransactionEvidence> {
        if additional_signers
            .iter()
            .any(|signer| signer.pubkey() == payer.pubkey())
        {
            return Err(Error::new("transaction signer list duplicated its payer"));
        }
        for (index, signer) in additional_signers.iter().enumerate() {
            if additional_signers
                .iter()
                .skip(index.saturating_add(1))
                .any(|other| other.pubkey() == signer.pubkey())
            {
                return Err(Error::new("transaction signer list contained duplicates"));
            }
        }
        let blockhash = self.latest_blockhash()?;
        let bounded_instructions = bounded_instructions(instructions);
        let mut signers: Vec<&dyn Signer> = Vec::with_capacity(additional_signers.len() + 1);
        signers.push(payer);
        signers.extend(
            additional_signers
                .iter()
                .map(|signer| *signer as &dyn Signer),
        );
        let transaction = Transaction::new_signed_with_payer(
            &bounded_instructions,
            Some(&payer.pubkey()),
            &signers,
            blockhash,
        );
        let signature = self.submit(label, &transaction, expect_failure)?;
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
        for _ in 0..600 {
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
        for _ in 0..600 {
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
        eprintln!(
            "campaign transaction: slot={slot} fee={fee_lamports} compute_units={} {label}",
            compute_units_consumed
                .map(|units| units.to_string())
                .unwrap_or_else(|| "unavailable".into())
        );
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

fn bounded_instructions(instructions: &[Instruction]) -> Vec<Instruction> {
    let mut bounded = Vec::with_capacity(instructions.len().saturating_add(1));
    let mut compute_limit_data = Vec::with_capacity(5);
    compute_limit_data.push(2);
    compute_limit_data.extend_from_slice(&LOCAL_PROTOCOL_COMPUTE_UNIT_LIMIT.to_le_bytes());
    bounded.push(Instruction {
        program_id: solana_sdk_ids::compute_budget::ID,
        accounts: Vec::new(),
        data: compute_limit_data,
    });
    bounded.extend_from_slice(instructions);
    bounded
}

fn parse_optional_account(value: &Value) -> Result<Option<RpcAccount>> {
    if value.is_null() {
        return Ok(None);
    }
    let encoded = value
        .get("data")
        .and_then(Value::as_array)
        .and_then(|values| values.first())
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("account omitted base64 data"))?;
    let data = BASE64
        .decode(encoded)
        .map_err(|error| Error::new(format!("account base64: {error}")))?;
    Ok(Some(RpcAccount {
        lamports: u64_field(value, "lamports")?,
        owner: pubkey(string_field(value, "owner")?)?,
        executable: value
            .get("executable")
            .and_then(Value::as_bool)
            .ok_or_else(|| Error::new("account omitted executable"))?,
        rent_epoch: u64_field(value, "rentEpoch")?,
        data,
    }))
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
