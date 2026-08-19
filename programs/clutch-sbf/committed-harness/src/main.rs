//! Submit, confirm, and reload a local committed lifecycle plan.
//!
//! This runner is deliberately narrower than a general Solana client.  It
//! accepts only a loopback RPC URL, only legacy transactions emitted by the
//! repository's fixture generator, and only the explicitly supplied ephemeral
//! test signers (normally a fee payer and market actor, plus any holder identity
//! a step requires). It replaces the zero fixture
//! blockhash, signs the actual serialized message bytes, submits with
//! preflight disabled (so expected refusals are themselves recorded by the
//! local bank), waits for confirmation, and reloads every compared account.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::Deserialize;
use serde_json::{json, Value};
use solana_keypair::{read_keypair_file, Keypair, Signer};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
type BankSnapshot = BTreeMap<String, Option<Vec<u8>>>;

#[derive(Debug, Deserialize)]
struct Plan {
    program_id: String,
    payer: String,
    actor: String,
    holder: String,
    /// True while the validator must preload zeroed, program-owned market
    /// PDAs that an ordinary wallet cannot create.  The runner reports this
    /// provenance before it submits anything; it must never be confused with
    /// an end-to-end permissionless lifecycle.
    genesis_assisted: bool,
    #[serde(default)]
    precreated_program_accounts: Vec<String>,
    steps: Vec<Step>,
}

#[derive(Debug, Deserialize)]
struct Step {
    name: String,
    tx: String,
    kind: String,
    #[serde(default)]
    expect_code: Option<u64>,
    #[serde(default)]
    compare: Vec<Compare>,
    #[serde(default)]
    identical_to_pre: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Compare {
    role: String,
    address: String,
    expected: String,
}

fn usage() -> ! {
    eprintln!(
        "usage: clutch-sbf-committed-harness <loopback-url> <plan-dir> \
         <payer-keypair.json> <actor-keypair.json> [additional-test-keypair.json ...]"
    );
    std::process::exit(2);
}

fn require_loopback(url: &str) -> Result<()> {
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

fn rpc(url: &str, method: &str, params: &Value) -> Result<Value> {
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

fn base58_decode_32(text: &str) -> Result<[u8; 32]> {
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

fn sign_transaction(
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

fn custom_error_code(error: &Value) -> Option<u64> {
    error
        .get("InstructionError")?
        .as_array()?
        .get(1)?
        .get("Custom")?
        .as_u64()
}

fn computational_budget_exhausted(error: &Value) -> bool {
    error
        .get("InstructionError")
        .and_then(Value::as_array)
        .and_then(|parts| parts.get(1))
        .and_then(Value::as_str)
        == Some("ComputationalBudgetExceeded")
}

fn latest_blockhash(url: &str) -> Result<[u8; 32]> {
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

fn submit(url: &str, transaction: &[u8]) -> Result<String> {
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

fn await_confirmation(url: &str, signature: &str) -> Result<Value> {
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

fn account_bytes(url: &str, address: &str) -> Result<Option<Vec<u8>>> {
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

fn hex_decode(text: &str) -> Result<Vec<u8>> {
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

fn snapshot(url: &str, addresses: &BTreeSet<String>) -> Result<BankSnapshot> {
    addresses
        .iter()
        .map(|address| Ok((address.clone(), account_bytes(url, address)?)))
        .collect()
}

fn report_scope(plan: &Plan, watched: usize) {
    println!(
        "program={} steps={} watched={} genesis_assisted={}",
        plan.program_id,
        plan.steps.len(),
        watched,
        plan.genesis_assisted
    );
    if plan.genesis_assisted {
        println!(
            "NOT END TO END: {} program-owned account(s) were precreated by local validator genesis:",
            plan.precreated_program_accounts.len()
        );
        for account in &plan.precreated_program_accounts {
            println!("  precreated {account}");
        }
    }
}

fn watch_index(plan: &Plan) -> Result<(BTreeSet<String>, BTreeMap<String, String>)> {
    let mut watched = BTreeSet::new();
    let mut role_addresses = BTreeMap::new();
    for entry in plan.steps.iter().flat_map(|step| step.compare.iter()) {
        watched.insert(entry.address.clone());
        if let Some(previous) = role_addresses.insert(entry.role.clone(), entry.address.clone()) {
            if previous != entry.address {
                return Err(format!(
                    "role {} maps to both {} and {}",
                    entry.role, previous, entry.address
                )
                .into());
            }
        }
    }
    for step in &plan.steps {
        for role in &step.identical_to_pre {
            if !role_addresses.contains_key(role) {
                return Err(format!("{}: unknown identical-to-pre role {role}", step.name).into());
            }
        }
    }
    Ok((watched, role_addresses))
}

fn unchanged_snapshot(
    url: &str,
    step: &Step,
    role_addresses: &BTreeMap<String, String>,
) -> Result<(BTreeSet<String>, BankSnapshot)> {
    let addresses: BTreeSet<String> = step
        .identical_to_pre
        .iter()
        .map(|role| {
            role_addresses
                .get(role)
                .expect("roles were validated before transaction submission")
                .clone()
        })
        .collect();
    let state = snapshot(url, &addresses)?;
    Ok((addresses, state))
}

fn load_keypairs(plan: &Plan, paths: &[String]) -> Result<Vec<Keypair>> {
    let keypairs: Vec<Keypair> = paths
        .iter()
        .map(read_keypair_file)
        .collect::<std::result::Result<_, _>>()?;
    let mut public_keys = BTreeSet::new();
    for keypair in &keypairs {
        if !public_keys.insert(keypair.pubkey().to_string()) {
            return Err("the same ephemeral signer was supplied more than once".into());
        }
    }
    for (role, address) in [
        ("payer", &plan.payer),
        ("actor", &plan.actor),
        ("holder", &plan.holder),
    ] {
        if !public_keys.contains(address) {
            return Err(format!("no supplied keypair matches plan {role} {address}").into());
        }
    }
    Ok(keypairs)
}

fn verify_nonaccepting_step(
    url: &str,
    step: &Step,
    error: &Value,
    watched: &BTreeSet<String>,
    before: Option<&BankSnapshot>,
) -> Result<()> {
    match step.kind.as_str() {
        "refuse" => {
            let expected = step
                .expect_code
                .ok_or_else(|| format!("{}: refusal has no expect_code", step.name))?;
            let observed = custom_error_code(error);
            if observed != Some(expected) {
                return Err(format!(
                    "{}: expected Custom({expected:#06x}), got {error}",
                    step.name
                )
                .into());
            }
        }
        "exhausted" if computational_budget_exhausted(error) => {}
        "exhausted" => {
            return Err(format!(
                "{}: expected ComputationalBudgetExceeded, got {error}",
                step.name
            )
            .into());
        }
        other => return Err(format!("{}: unknown step kind {other}", step.name).into()),
    }
    let after = snapshot(url, watched)?;
    if before != Some(&after) {
        return Err(format!("{}: failed transaction changed watched state", step.name).into());
    }
    Ok(())
}

fn run(url: &str, plan_dir: &Path, keypair_paths: &[String]) -> Result<()> {
    require_loopback(url)?;

    let plan: Plan = serde_json::from_slice(&fs::read(plan_dir.join("committed.json"))?)?;
    let keypairs = load_keypairs(&plan, keypair_paths)?;
    if plan.genesis_assisted && plan.precreated_program_accounts.is_empty() {
        return Err("genesis-assisted plan does not enumerate its precreated accounts".into());
    }

    let (watched, role_addresses) = watch_index(&plan)?;
    let keypair_refs: Vec<&Keypair> = keypairs.iter().collect();
    report_scope(&plan, watched.len());

    for (ordinal, step) in plan.steps.iter().enumerate() {
        let unsigned = BASE64.decode(fs::read_to_string(plan_dir.join(&step.tx))?.trim())?;
        let before = if matches!(step.kind.as_str(), "refuse" | "exhausted") {
            Some(snapshot(url, &watched)?)
        } else {
            None
        };
        let (unchanged_addresses, unchanged_before) =
            unchanged_snapshot(url, step, &role_addresses)?;
        let signed = sign_transaction(&unsigned, latest_blockhash(url)?, &keypair_refs)?;
        let signature = submit(url, &signed)?;
        let status = await_confirmation(url, &signature)?;
        let error = status.get("err").cloned().unwrap_or(Value::Null);

        match step.kind.as_str() {
            "accept" if error.is_null() => {}
            "accept" => return Err(format!("{}: expected success, got {error}", step.name).into()),
            _ => verify_nonaccepting_step(url, step, &error, &watched, before.as_ref())?,
        }

        for entry in &step.compare {
            let observed = account_bytes(url, &entry.address)?
                .ok_or_else(|| format!("{} / {}: account is absent", step.name, entry.role))?;
            let expected = hex_decode(&fs::read_to_string(plan_dir.join(&entry.expected))?)?;
            if observed != expected {
                return Err(format!(
                    "{} / {}: committed bytes differ (observed {}, expected {})",
                    step.name,
                    entry.role,
                    observed.len(),
                    expected.len()
                )
                .into());
            }
        }
        let unchanged_after = snapshot(url, &unchanged_addresses)?;
        if unchanged_before != unchanged_after {
            return Err(format!(
                "{}: identical-to-pre role changed ({})",
                step.name,
                step.identical_to_pre.join(", ")
            )
            .into());
        }
        println!(
            "{:>2} {:<34} {:<7} confirmed={} reloads={} signature={}",
            ordinal + 1,
            step.name,
            step.kind,
            status
                .get("confirmationStatus")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            step.compare.len() + step.identical_to_pre.len(),
            signature
        );
    }

    println!("PASS: every signed transaction confirmed and every declared state reload matched");
    Ok(())
}

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let Some(url) = args.next() else { usage() };
    let Some(plan_dir) = args.next().map(PathBuf::from) else {
        usage()
    };
    let keypair_paths: Vec<String> = args.collect();
    if keypair_paths.len() < 2 {
        usage();
    }
    run(&url, &plan_dir, &keypair_paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_signer_fixture(payer: &Keypair, actor: &Keypair) -> Vec<u8> {
        let mut transaction = vec![2];
        transaction.extend_from_slice(&[0_u8; 128]);
        transaction.extend_from_slice(&[2, 1, 0]);
        transaction.push(2);
        transaction.extend_from_slice(&payer.pubkey().to_bytes());
        transaction.extend_from_slice(&actor.pubkey().to_bytes());
        transaction.extend_from_slice(&[0_u8; 32]);
        transaction.push(0);
        transaction
    }

    fn three_signer_fixture(signers: [&Keypair; 3]) -> Vec<u8> {
        let mut transaction = vec![3];
        transaction.extend_from_slice(&[0_u8; 192]);
        transaction.extend_from_slice(&[3, 0, 0]);
        transaction.push(3);
        for signer in signers {
            transaction.extend_from_slice(&signer.pubkey().to_bytes());
        }
        transaction.extend_from_slice(&[0_u8; 32]);
        transaction.push(0);
        transaction
    }

    #[test]
    fn only_exact_loopback_urls_are_admitted() {
        assert!(require_loopback("http://127.0.0.1:18899").is_ok());
        assert!(require_loopback("http://localhost:18899").is_ok());
        assert!(require_loopback("https://127.0.0.1:18899").is_err());
        assert!(require_loopback("http://127.0.0.1.example:18899").is_err());
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
    fn signing_replaces_the_blockhash_and_both_signature_slots() {
        let payer = Keypair::new();
        let actor = Keypair::new();
        let unsigned = two_signer_fixture(&payer, &actor);
        let signed = sign_transaction(&unsigned, [7_u8; 32], &[&payer, &actor])
            .expect("fixture transaction signs");
        let (signatures_start, _, blockhash_start, signers) =
            message_layout(&signed).expect("signed transaction parses");
        assert_eq!(
            signers,
            [payer.pubkey().to_bytes(), actor.pubkey().to_bytes()]
        );
        assert!(signed[signatures_start..signatures_start + 64]
            .iter()
            .any(|byte| *byte != 0));
        assert!(signed[signatures_start + 64..signatures_start + 128]
            .iter()
            .any(|byte| *byte != 0));
        assert_eq!(&signed[blockhash_start..blockhash_start + 32], &[7_u8; 32]);
    }

    #[test]
    fn signing_refuses_a_plan_with_a_missing_required_key() {
        let payer = Keypair::new();
        let actor = Keypair::new();
        let unsigned = two_signer_fixture(&actor, &payer);
        assert!(sign_transaction(&unsigned, [7_u8; 32], &[&payer]).is_err());
    }

    #[test]
    fn a_third_ephemeral_holder_can_sign_in_message_order() {
        let payer = Keypair::new();
        let actor = Keypair::new();
        let holder = Keypair::new();
        let unsigned = three_signer_fixture([&payer, &holder, &actor]);
        let signed = sign_transaction(&unsigned, [9_u8; 32], &[&actor, &payer, &holder])
            .expect("all message signers are found by public key");
        let (start, _, _, signers) = message_layout(&signed).expect("transaction parses");
        assert_eq!(
            signers,
            [
                payer.pubkey().to_bytes(),
                holder.pubkey().to_bytes(),
                actor.pubkey().to_bytes()
            ]
        );
        for index in 0..3 {
            assert!(signed[start + index * 64..start + (index + 1) * 64]
                .iter()
                .any(|byte| *byte != 0));
        }
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
