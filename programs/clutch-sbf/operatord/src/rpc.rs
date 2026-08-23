//! Loopback JSON-RPC, signing, submission, confirmation, and reload.
//!
//! Every function here is **vendored verbatim** from
//! `../committed-harness/src/main.rs`, which is a binary crate in its own
//! workspace and therefore not callable.  Keeping the text identical is what
//! makes a diff between the two meaningful: the sealed lane's runner and this
//! daemon must agree about how a plan transaction is signed and submitted, and
//! the way to see a disagreement is `diff`.
//!
//! What is deliberately *not* vendored is transaction construction.  That has
//! exactly one implementation, `clutch_sbf_harness`, and the replay falsifier
//! proves this daemon uses it.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde_json::{json, Value};
use solana_keypair::{Keypair, Signer};
use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub type BankSnapshot = BTreeMap<String, Option<Vec<u8>>>;

/// One complete account value returned by Solana JSON-RPC.
///
/// Data without these fields cannot support an authority-checked daemon
/// observation: in particular, owner and executable separate protocol state
/// from arbitrary bytes at an attacker-chosen address. This is still an RPC
/// projection, not browser-side authentication. The response context slot
/// lives on the enclosing read because one `getMultipleAccounts` batch shares
/// one slot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountEnvelope {
    pub data: Vec<u8>,
    pub owner: String,
    pub executable: bool,
    pub lamports: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountRead {
    pub context_slot: u64,
    pub account: Option<AccountEnvelope>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchRead {
    pub context_slot: u64,
    pub accounts: BTreeMap<String, Option<AccountEnvelope>>,
}

/// A same-context batch bracketed by an unchanged root account.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphSnapshotV2 {
    pub context_slot: u64,
    pub attempts: usize,
    pub root: String,
    pub accounts: BTreeMap<String, Option<AccountEnvelope>>,
}

const SNAPSHOT_V2_MAX_ATTEMPTS: usize = 3;
const RPC_MULTIPLE_ACCOUNT_LIMIT: usize = 100;

/// Admit only an exact loopback RPC URL.
pub fn require_loopback(url: &str) -> Result<()> {
    let accepted = url
        .strip_prefix("http://127.0.0.1:")
        .or_else(|| url.strip_prefix("http://localhost:"));
    let Some(port) = accepted else {
        return Err(format!("refusing non-loopback RPC URL: {url}").into());
    };
    if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("loopback RPC URL has an invalid port: {url}").into());
    }
    Ok(())
}

pub fn rpc(url: &str, method: &str, params: &Value) -> Result<Value> {
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
            url,
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
    if let Some(error) = response.get("error") {
        return Err(format!("RPC error for {method}: {error}").into());
    }
    response
        .get("result")
        .cloned()
        .ok_or_else(|| format!("RPC response for {method} has no result").into())
}

fn decode_short_vec(bytes: &[u8], offset: &mut usize) -> Result<usize> {
    let mut value = 0_usize;
    let mut shift = 0_u32;
    loop {
        let byte = *bytes
            .get(*offset)
            .ok_or("truncated short-vec while parsing transaction")?;
        *offset += 1;
        value |= usize::from(byte & 0x7f)
            .checked_shl(shift)
            .ok_or("short-vec value overflow")?;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
        shift += 7;
        if shift > 21 {
            return Err("short-vec is wider than a Solana packet permits".into());
        }
    }
}

pub fn base58_decode_32(text: &str) -> Result<[u8; 32]> {
    const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    let mut out = [0_u8; 32];
    for character in text.bytes() {
        let digit = ALPHABET
            .iter()
            .position(|candidate| *candidate == character)
            .ok_or_else(|| format!("invalid base58 character in blockhash: {character}"))?;
        let mut carry = digit;
        for byte in out.iter_mut().rev() {
            let wide = usize::from(*byte) * 58 + carry;
            *byte = u8::try_from(wide & 0xff)?;
            carry = wide >> 8;
        }
        if carry != 0 {
            return Err("base58 blockhash is wider than 32 bytes".into());
        }
    }
    Ok(out)
}

fn message_layout(transaction: &[u8]) -> Result<(usize, usize, usize, Vec<[u8; 32]>)> {
    let mut cursor = 0;
    let signature_count = decode_short_vec(transaction, &mut cursor)?;
    let signatures_start = cursor;
    let signatures_bytes = signature_count
        .checked_mul(64)
        .ok_or("signature-vector length overflow")?;
    cursor = cursor
        .checked_add(signatures_bytes)
        .ok_or("transaction offset overflow")?;
    let message_start = cursor;

    let required = usize::from(
        *transaction
            .get(cursor)
            .ok_or("transaction has no message header")?,
    );
    if required != signature_count {
        return Err(format!(
            "signature count {signature_count} disagrees with message header {required}"
        )
        .into());
    }
    cursor += 3;
    let key_count = decode_short_vec(transaction, &mut cursor)?;
    let keys_start = cursor;
    let keys_end = keys_start
        .checked_add(
            key_count
                .checked_mul(32)
                .ok_or("key-vector length overflow")?,
        )
        .ok_or("transaction offset overflow")?;
    if transaction.len() < keys_end + 32 {
        return Err("transaction is truncated before its recent blockhash".into());
    }
    let mut signers = Vec::with_capacity(required);
    for index in 0..required {
        let start = keys_start + index * 32;
        signers.push(transaction[start..start + 32].try_into()?);
    }
    Ok((signatures_start, message_start, keys_end, signers))
}

/// Replace the zero fixture blockhash and sign every required message slot.
pub fn sign_transaction(
    unsigned: &[u8],
    blockhash: [u8; 32],
    keypairs: &[&Keypair],
) -> Result<Vec<u8>> {
    let (signatures_start, message_start, blockhash_start, signer_keys) = message_layout(unsigned)?;
    let mut signed = unsigned.to_vec();
    signed[blockhash_start..blockhash_start + 32].copy_from_slice(&blockhash);
    let (signature_prefix, message) = signed.split_at_mut(message_start);
    for (index, signer_key) in signer_keys.iter().enumerate() {
        let keypair = keypairs
            .iter()
            .find(|candidate| candidate.pubkey().to_bytes() == *signer_key)
            .ok_or_else(|| format!("no ephemeral keypair for required signer {signer_key:?}"))?;
        let signature = keypair.sign_message(message);
        let start = signatures_start + index * 64;
        signature_prefix[start..start + 64].copy_from_slice(signature.as_ref());
    }
    Ok(signed)
}

pub fn custom_error_code(error: &Value) -> Option<u64> {
    error
        .get("InstructionError")?
        .as_array()?
        .get(1)?
        .get("Custom")?
        .as_u64()
}

pub fn computational_budget_exhausted(error: &Value) -> bool {
    error
        .get("InstructionError")
        .and_then(Value::as_array)
        .and_then(|parts| parts.get(1))
        .and_then(Value::as_str)
        == Some("ComputationalBudgetExceeded")
}

/// The bank's current confirmed slot.
pub fn current_slot(url: &str) -> Result<u64> {
    rpc(url, "getSlot", &json!([{"commitment": "confirmed"}]))?
        .as_u64()
        .ok_or_else(|| "getSlot returned no slot".into())
}

/// Block until the bank reaches `target`, reporting progress to `tick`.
///
/// The deadline transitions of the general plane are clock-gated by the
/// program, so this waits on the real validator's clock rather than warping
/// it.  `tick` is how the browser gets a live countdown instead of a freeze.
pub fn wait_for_slot(
    url: &str,
    target: u64,
    reason: &str,
    tick: &mut dyn FnMut(u64, u64, &str),
) -> Result<()> {
    let mut now = current_slot(url)?;
    if now >= target {
        return Ok(());
    }
    while now < target {
        tick(now, target, reason);
        thread::sleep(Duration::from_millis(250));
        now = current_slot(url)?;
    }
    tick(now, target, reason);
    Ok(())
}

/// Compute units the bank charged one committed transaction, when reported.
pub fn compute_units(url: &str, signature: &str) -> Option<u64> {
    let result = rpc(
        url,
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

pub fn latest_blockhash(url: &str) -> Result<[u8; 32]> {
    let result = rpc(
        url,
        "getLatestBlockhash",
        &json!([{"commitment": "confirmed"}]),
    )?;
    let text = result
        .get("value")
        .and_then(|value| value.get("blockhash"))
        .and_then(Value::as_str)
        .ok_or("getLatestBlockhash returned no blockhash")?;
    base58_decode_32(text)
}

pub fn submit(url: &str, transaction: &[u8]) -> Result<String> {
    let encoded = BASE64.encode(transaction);
    let result = rpc(
        url,
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
        .ok_or_else(|| "sendTransaction returned no signature".into())
}

pub fn await_confirmation(url: &str, signature: &str) -> Result<Value> {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        let result = rpc(
            url,
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
                return Ok(status.clone());
            }
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(format!("transaction {signature} did not confirm within 30 seconds").into())
}

fn context_slot(result: &Value, method: &str) -> Result<u64> {
    result
        .get("context")
        .and_then(|context| context.get("slot"))
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{method} returned no unsigned context slot").into())
}

fn account_envelope(value: &Value, address: &str) -> Result<AccountEnvelope> {
    let parts = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("account {address} returned no encoded data array"))?;
    if parts.get(1).and_then(Value::as_str) != Some("base64") || parts.len() != 2 {
        return Err(format!("account {address} did not return exact base64 encoding").into());
    }
    let encoded = parts
        .first()
        .and_then(Value::as_str)
        .ok_or_else(|| format!("account {address} returned no base64 payload"))?;
    let owner = value
        .get("owner")
        .and_then(Value::as_str)
        .filter(|owner| !owner.is_empty())
        .ok_or_else(|| format!("account {address} returned no owner"))?;
    let executable = value
        .get("executable")
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("account {address} returned no executable bit"))?;
    let lamports = value
        .get("lamports")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("account {address} returned no unsigned lamport balance"))?;
    Ok(AccountEnvelope {
        data: BASE64.decode(encoded)?,
        owner: owner.to_string(),
        executable,
        lamports,
    })
}

fn parse_account_read(result: &Value, address: &str, method: &str) -> Result<AccountRead> {
    let context_slot = context_slot(result, method)?;
    let account = result
        .get("value")
        .ok_or_else(|| format!("{method} returned no value field"))?;
    let account = if account.is_null() {
        None
    } else {
        Some(account_envelope(account, address)?)
    };
    Ok(AccountRead {
        context_slot,
        account,
    })
}

fn rpc_account_config(min_context_slot: Option<u64>) -> Value {
    let mut config = json!({"encoding": "base64", "commitment": "confirmed"});
    if let Some(slot) = min_context_slot {
        config["minContextSlot"] = json!(slot);
    }
    config
}

pub fn account(url: &str, address: &str, min_context_slot: Option<u64>) -> Result<AccountRead> {
    let result = rpc(
        url,
        "getAccountInfo",
        &json!([address, rpc_account_config(min_context_slot)]),
    )?;
    parse_account_read(&result, address, "getAccountInfo")
}

pub fn multiple_accounts(
    url: &str,
    addresses: &[String],
    min_context_slot: Option<u64>,
) -> Result<BatchRead> {
    if addresses.is_empty() || addresses.len() > RPC_MULTIPLE_ACCOUNT_LIMIT {
        return Err(format!(
            "getMultipleAccounts requires 1..={RPC_MULTIPLE_ACCOUNT_LIMIT} addresses"
        )
        .into());
    }
    let result = rpc(
        url,
        "getMultipleAccounts",
        &json!([addresses, rpc_account_config(min_context_slot)]),
    )?;
    parse_batch_read(&result, addresses)
}

fn parse_batch_read(result: &Value, addresses: &[String]) -> Result<BatchRead> {
    let context_slot = context_slot(result, "getMultipleAccounts")?;
    let values = result
        .get("value")
        .and_then(Value::as_array)
        .ok_or("getMultipleAccounts returned no value array")?;
    if values.len() != addresses.len() {
        return Err(format!(
            "getMultipleAccounts returned {} values for {} addresses",
            values.len(),
            addresses.len()
        )
        .into());
    }
    let accounts = addresses
        .iter()
        .zip(values)
        .map(|(address, value)| {
            let envelope = if value.is_null() {
                None
            } else {
                Some(account_envelope(value, address)?)
            };
            Ok((address.clone(), envelope))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    Ok(BatchRead {
        context_slot,
        accounts,
    })
}

fn graph_snapshot_with<One, Many>(
    root: &str,
    children: &[String],
    max_attempts: usize,
    mut read_one: One,
    mut read_many: Many,
) -> Result<GraphSnapshotV2>
where
    One: FnMut(&str, Option<u64>) -> Result<AccountRead>,
    Many: FnMut(&[String], Option<u64>) -> Result<BatchRead>,
{
    if max_attempts == 0 {
        return Err("graph snapshot requires at least one attempt".into());
    }
    let mut addresses = Vec::with_capacity(children.len() + 1);
    addresses.push(root.to_string());
    let mut distinct = BTreeSet::from([root.to_string()]);
    for child in children {
        if !distinct.insert(child.clone()) {
            return Err(format!("graph snapshot contains duplicate address {child}").into());
        }
        addresses.push(child.clone());
    }

    let mut last = String::new();
    for attempt in 1..=max_attempts {
        let before = read_one(root, None)?;
        if before.account.is_none() {
            return Err(format!("graph snapshot root {root} is absent").into());
        }
        let batch = read_many(&addresses, Some(before.context_slot))?;
        if batch.context_slot < before.context_slot {
            return Err("getMultipleAccounts violated minContextSlot".into());
        }
        let after = read_one(root, Some(batch.context_slot))?;
        if after.context_slot < batch.context_slot {
            return Err("root re-read violated minContextSlot".into());
        }
        let in_batch = batch
            .accounts
            .get(root)
            .ok_or("getMultipleAccounts omitted the graph root")?;
        if &before.account == in_batch && &after.account == in_batch {
            return Ok(GraphSnapshotV2 {
                context_slot: batch.context_slot,
                attempts: attempt,
                root: root.to_string(),
                accounts: batch.accounts,
            });
        }
        last = format!(
            "root changed while bracketing batch (before slot {}, batch slot {}, after slot {})",
            before.context_slot, batch.context_slot, after.context_slot
        );
    }
    Err(
        format!("graph snapshot remained inconsistent after {max_attempts} attempts: {last}")
            .into(),
    )
}

/// Read one fail-closed V2 snapshot: root, one same-context batch, then root.
///
/// `getMultipleAccounts` supplies the common context slot. The two root reads
/// use `minContextSlot` and must carry the exact same complete envelope as the
/// root inside that batch. A moving root is retried a fixed number of times;
/// malformed responses, absent roots, duplicate addresses, and exhaustion are
/// errors rather than partial state. Child consistency comes from the one
/// batch context; the bracket proves only that this root envelope did not move
/// around the batch, not that unrelated graph state was immutable.
pub fn graph_snapshot_v2(url: &str, root: &str, children: &[String]) -> Result<GraphSnapshotV2> {
    graph_snapshot_with(
        root,
        children,
        SNAPSHOT_V2_MAX_ATTEMPTS,
        |address, floor| account(url, address, floor),
        |addresses, floor| multiple_accounts(url, addresses, floor),
    )
}

pub fn account_bytes(url: &str, address: &str) -> Result<Option<Vec<u8>>> {
    Ok(account(url, address, None)?.account.map(|entry| entry.data))
}

pub fn snapshot(url: &str, addresses: &BTreeSet<String>) -> Result<BankSnapshot> {
    addresses
        .iter()
        .map(|address| Ok((address.clone(), account_bytes(url, address)?)))
        .collect()
}

pub fn hex_decode(text: &str) -> Result<Vec<u8>> {
    let clean = text.trim();
    if !clean.len().is_multiple_of(2) {
        return Err("hex expectation has odd length".into());
    }
    clean
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair)?;
            Ok(u8::from_str_radix(pair, 16)?)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope(marker: u8) -> AccountEnvelope {
        AccountEnvelope {
            data: vec![marker],
            owner: format!("owner-{marker}"),
            executable: false,
            lamports: u64::from(marker),
        }
    }

    fn read(slot: u64, marker: u8) -> AccountRead {
        AccountRead {
            context_slot: slot,
            account: Some(envelope(marker)),
        }
    }

    fn batch(slot: u64, root: &str, marker: u8) -> BatchRead {
        BatchRead {
            context_slot: slot,
            accounts: BTreeMap::from([(root.to_string(), Some(envelope(marker)))]),
        }
    }

    #[test]
    fn only_exact_loopback_urls_are_admitted() {
        assert!(require_loopback("http://127.0.0.1:9137").is_ok());
        assert!(require_loopback("http://localhost:9137").is_ok());
        assert!(require_loopback("https://127.0.0.1:9137").is_err());
        assert!(require_loopback("http://127.0.0.1.example:9137").is_err());
        assert!(require_loopback("http://api.mainnet-beta.solana.com:80").is_err());
    }

    #[test]
    fn the_zero_blockhash_has_the_canonical_base58_spelling() {
        assert_eq!(
            base58_decode_32("11111111111111111111111111111111").expect("zero hash decodes"),
            [0_u8; 32]
        );
    }

    #[test]
    fn signing_replaces_the_blockhash_and_every_signature_slot() {
        let payer = Keypair::new();
        let actor = Keypair::new();
        let mut unsigned = vec![2];
        unsigned.extend_from_slice(&[0_u8; 128]);
        unsigned.extend_from_slice(&[2, 1, 0]);
        unsigned.push(2);
        unsigned.extend_from_slice(&payer.pubkey().to_bytes());
        unsigned.extend_from_slice(&actor.pubkey().to_bytes());
        unsigned.extend_from_slice(&[0_u8; 32]);
        unsigned.push(0);
        let signed =
            sign_transaction(&unsigned, [7_u8; 32], &[&payer, &actor]).expect("fixture signs");
        let (start, _, blockhash_start, signers) =
            message_layout(&signed).expect("signed transaction parses");
        assert_eq!(
            signers,
            [payer.pubkey().to_bytes(), actor.pubkey().to_bytes()]
        );
        assert!(signed[start..start + 64].iter().any(|byte| *byte != 0));
        assert!(signed[start + 64..start + 128]
            .iter()
            .any(|byte| *byte != 0));
        assert_eq!(&signed[blockhash_start..blockhash_start + 32], &[7_u8; 32]);
    }

    #[test]
    fn signing_refuses_a_plan_with_a_missing_required_key() {
        let payer = Keypair::new();
        let actor = Keypair::new();
        let mut unsigned = vec![2];
        unsigned.extend_from_slice(&[0_u8; 128]);
        unsigned.extend_from_slice(&[2, 1, 0]);
        unsigned.push(2);
        unsigned.extend_from_slice(&actor.pubkey().to_bytes());
        unsigned.extend_from_slice(&payer.pubkey().to_bytes());
        unsigned.extend_from_slice(&[0_u8; 32]);
        unsigned.push(0);
        assert!(sign_transaction(&unsigned, [7_u8; 32], &[&payer]).is_err());
    }

    #[test]
    fn custom_error_extraction_is_structural() {
        assert_eq!(
            custom_error_code(&json!({"InstructionError": [1, {"Custom": 23}]})),
            Some(23)
        );
        assert_eq!(
            custom_error_code(&json!({"InstructionError": [1, "Invalid"]})),
            None
        );
    }

    #[test]
    fn compute_exhaustion_extraction_is_exact() {
        assert!(computational_budget_exhausted(
            &json!({"InstructionError": [2, "ComputationalBudgetExceeded"]})
        ));
        assert!(!computational_budget_exhausted(
            &json!({"InstructionError": [2, "ProgramFailedToComplete"]})
        ));
    }

    #[test]
    fn account_parser_retains_the_complete_envelope_and_context() {
        let result = json!({
            "context": {"slot": 42},
            "value": {
                "data": [BASE64.encode([1_u8, 2, 3]), "base64"],
                "owner": "owner-address",
                "executable": false,
                "lamports": 9,
                "rentEpoch": 0,
                "space": 3,
            }
        });
        let parsed = parse_account_read(&result, "account-address", "fixture")
            .expect("complete account response parses");
        assert_eq!(parsed.context_slot, 42);
        assert_eq!(
            parsed.account,
            Some(AccountEnvelope {
                data: vec![1, 2, 3],
                owner: "owner-address".to_string(),
                executable: false,
                lamports: 9,
            })
        );
    }

    #[test]
    fn account_parser_refuses_missing_authority_fields_and_wrong_encoding() {
        let base = json!({
            "context": {"slot": 42},
            "value": {
                "data": [BASE64.encode([1_u8]), "base64"],
                "owner": "owner-address",
                "executable": false,
                "lamports": 9,
            }
        });
        for field in ["owner", "executable", "lamports"] {
            let mut malformed = base.clone();
            malformed["value"]
                .as_object_mut()
                .expect("fixture object")
                .remove(field);
            assert!(parse_account_read(&malformed, "account-address", "fixture").is_err());
        }
        let mut wrong_encoding = base;
        wrong_encoding["value"]["data"][1] = json!("base64+zstd");
        assert!(parse_account_read(&wrong_encoding, "account-address", "fixture").is_err());
    }

    #[test]
    fn multiple_account_parser_binds_every_value_to_one_context_and_exact_address_count() {
        let addresses = vec!["first".to_string(), "second".to_string()];
        let result = json!({
            "context": {"slot": 77},
            "value": [
                {
                    "data": [BASE64.encode([1_u8]), "base64"],
                    "owner": "owner-1",
                    "executable": false,
                    "lamports": 1,
                },
                Value::Null,
            ]
        });
        let parsed = parse_batch_read(&result, &addresses).expect("exact batch parses");
        assert_eq!(parsed.context_slot, 77);
        assert_eq!(
            parsed.accounts["first"],
            Some(AccountEnvelope {
                data: vec![1],
                owner: "owner-1".to_string(),
                executable: false,
                lamports: 1,
            })
        );
        assert_eq!(parsed.accounts["second"], None);

        let mut short = result;
        short["value"].as_array_mut().expect("fixture array").pop();
        assert!(parse_batch_read(&short, &addresses).is_err());
    }

    #[test]
    fn graph_snapshot_retries_a_moving_root_then_returns_one_batch_slot() {
        let root = "root";
        let mut singles = vec![read(1, 1), read(3, 2), read(4, 2), read(6, 2)].into_iter();
        let mut batches = vec![batch(2, root, 2), batch(5, root, 2)].into_iter();
        let snapshot = graph_snapshot_with(
            root,
            &[],
            3,
            |_, _| Ok(singles.next().expect("fixture single read")),
            |_, _| Ok(batches.next().expect("fixture batch read")),
        )
        .expect("second stable bracket is admitted");
        assert_eq!(snapshot.attempts, 2);
        assert_eq!(snapshot.context_slot, 5);
        assert_eq!(snapshot.accounts[root], Some(envelope(2)));
    }

    #[test]
    fn graph_snapshot_is_bounded_and_fails_closed() {
        let root = "root";
        let mut singles = vec![read(1, 1), read(3, 3), read(4, 4), read(6, 6)].into_iter();
        let mut batches = vec![batch(2, root, 2), batch(5, root, 5)].into_iter();
        let error = graph_snapshot_with(
            root,
            &[],
            2,
            |_, _| Ok(singles.next().expect("fixture single read")),
            |_, _| Ok(batches.next().expect("fixture batch read")),
        )
        .expect_err("both moving-root attempts refuse")
        .to_string();
        assert!(error.contains("after 2 attempts"));
        assert!(graph_snapshot_with(
            root,
            &[root.to_string()],
            1,
            |_, _| Ok(read(1, 1)),
            |_, _| Ok(batch(1, root, 1)),
        )
        .is_err());
    }
}
