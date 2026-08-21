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

pub fn account_bytes(url: &str, address: &str) -> Result<Option<Vec<u8>>> {
    let result = rpc(
        url,
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
    Ok(Some(BASE64.decode(encoded)?))
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
}
