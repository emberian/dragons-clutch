//! Live, read-only supervision of the local real-Pyth joined lifecycle.
//!
//! This mode does not load a retained transcript and it does not make the
//! browser a signer. It starts the repository's clean-HEAD, loopback-only
//! `joined-multiboundary-v1` producer as a child and admits only its opt-in,
//! versioned JSON events. The daemon owns one private session directory and
//! its child validator/key lifecycle. After terminal state is independently
//! rediscovered, the daemon rebuilds one typed unsigned transaction from the
//! child's public identities. It neither reads the ephemeral private files nor
//! fetches a blockhash, signs, submits, or exports that wire.

use crate::{http, integer, rpc, toolchain, Bus};
use clutch_local_real_pyth::session_builder::{LocalTradingBuilder, SignerRole, PLAN_SCHEMA};
use clutch_solana_layout::{account_len, registry, SupplyLedgerAccount};
use serde_json::{json, Map, Value};
use solana_address::Address;
use std::collections::BTreeSet;
use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const CLAIM: &str = "NON-PRODUCTION / SYNTHETIC OBSERVATION / LOCAL VALIDATOR ONLY / NO VALUE";
const MODE: &str = "non-production-synthetic-source-v2-live";
const CAMPAIGN_MODE: &str = "joined-multiboundary-v1";
const TRANSCRIPT_SCHEMA: &str =
    "dragons-clutch/operator/local-real-pyth-multiboundary-joined-lifecycle/v1";
const MANIFEST_SCHEMA: &str = "dragons-clutch/operator/live-real-pyth-manifest/v1";
const RESULT_SCHEMA: &str = "dragons-clutch/operator/live-real-pyth-result/v1";
const RUN_SCHEMA: &str = "dragons-clutch/operator/live-real-pyth-run/v1";
const CHAIN_SCHEMA: &str = "dragons-clutch/operator/live-real-pyth-chain-discovery/v1";
const BUILDER_CONSTRUCTION_SCHEMA: &str =
    "dragons-clutch/operator/local-real-builder-construction/v2";
const EVENT_PREFIX: &str = "CLUTCH_OPERATOR_EVENT ";
const PROFILE: &str = "NON-PRODUCTION-non-production-real-pyth-lab";
const MAX_OUTPUT_LINE_BYTES: usize = 16 * 1024;
const ROLLBACK_SNAPSHOT_DOMAIN: &str = "dragons-clutch/local-real-pyth/rollback-snapshot/v1";
const ROLLBACK_SNAPSHOT_ENCODING: &str = "domain || target_count:u64-le || repeated(key:32 || present:u8 || if-present(lamports:u64-le || owner:32 || executable:u8 || data_len:u64-le || data))";
const MULTIBOUNDARY_PASS: &str = "PASS: signed PriceGrid/policy artifacts -> CreateMarket -> two-owner funded general book -> freeze -> best valid submitted candidate verification/selection -> entitlement/settlement -> real router/receiver source window -> Resolve(1) -> two-owner redemption/withdrawal";

pub struct Options {
    pub port: u16,
    pub rpc_port: u16,
    pub faucet_port: u16,
    pub gossip_port: u16,
    pub dynamic_port_range: String,
    pub work: Option<PathBuf>,
    pub statics: PathBuf,
    pub exit_when_done: bool,
}

struct LocalSessionOwner {
    root: PathBuf,
    campaign_work: PathBuf,
    control: PathBuf,
    session_id: String,
}

impl LocalSessionOwner {
    fn create(requested_base: Option<&PathBuf>) -> Result<Self> {
        let base = requested_base.cloned().unwrap_or_else(std::env::temp_dir);
        fs::create_dir_all(&base)?;
        let base_metadata = fs::symlink_metadata(&base)?;
        if base_metadata.file_type().is_symlink() || !base_metadata.is_dir() {
            return Err("local session work base must be a non-symlink directory".into());
        }
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let session_id = format!("{}-{stamp}", std::process::id());
        let root = base.join(format!("clutch-pyth-live-session.{session_id}"));
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700).create(&root)?;
        let campaign_work = root.join("campaign");
        let control = root.join("control");
        fs::DirBuilder::new().mode(0o700).create(&campaign_work)?;
        fs::DirBuilder::new().mode(0o700).create(&control)?;
        let marker = root.join("owner-v1");
        fs::write(&marker, b"dragons-clutch/operator/local-session-owner/v1\n")?;
        fs::set_permissions(&marker, fs::Permissions::from_mode(0o600))?;
        Ok(Self {
            root,
            campaign_work,
            control,
            session_id,
        })
    }

    fn configure(&self, command: &mut Command) {
        command
            .env("CLUTCH_LOCAL_REAL_PYTH_CALLER_OWNS_WORK", "1")
            .env(
                "CLUTCH_LOCAL_REAL_PYTH_CALLER_WORK_DIR",
                &self.campaign_work,
            )
            .env("CLUTCH_LOCAL_REAL_PYTH_CONTROL_DIR", &self.control);
    }

    fn request_stop(&self) -> Result<()> {
        fs::write(self.control.join("stop"), b"stop\n")?;
        Ok(())
    }

    fn retain_public_restart(&self, descriptor: &Value) -> Result<()> {
        let path = self.root.join("public-restart.json");
        fs::write(&path, serde_json::to_vec_pretty(descriptor)?)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        Ok(())
    }

    fn load_public_identity(&self, role: &str, filename: &str) -> Result<Address> {
        let path = self.campaign_work.join(filename);
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(format!("owned local public identity {role} has unsafe metadata").into());
        }
        let text = fs::read_to_string(&path)?;
        let canonical = text.trim();
        let bytes = rpc::base58_decode_32(canonical)?;
        if clutch_sbf_harness::base58_of(&bytes) != canonical {
            return Err(format!("owned local public identity {role} is not canonical").into());
        }
        Ok(Address::new_from_array(bytes))
    }

    fn signer_event(&self) -> Result<Value> {
        let payer = self.load_public_identity("payer", "payer.pubkey")?;
        let second_owner = self.load_public_identity("second_owner", "second-owner.pubkey")?;
        if payer == second_owner {
            return Err("owned local public identities alias".into());
        }
        let actors = [("payer", payer), ("second_owner", second_owner)]
            .into_iter()
            .map(|(role, public_key)| {
                json!({
                    "role": role,
                    "public_key": public_key.to_string(),
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({
            "type": "live-local-session-owner",
            "schema": "dragons-clutch/operator/local-session-owner/v1",
            "session_id": self.session_id,
            "lifecycle": "daemon-owned child, validator, work directory, and ephemeral signer roster",
            "actors": actors,
            "private_paths_exported": false,
            "private_key_material_exported": false,
            "browser_signing": false,
            "daemon_signing_seam": "disabled; Operator reads only child-emitted public identities and constructs an unsigned blockhash-free plan",
        }))
    }

    fn builder_construction_event(&self, result: &Value) -> Result<Value> {
        let payer = self.load_public_identity("payer", "payer.pubkey")?;
        let second_owner = self.load_public_identity("second_owner", "second-owner.pubkey")?;
        let records = field_array(result, "archive_records", "live result")?;
        let first = records
            .first()
            .ok_or("live result has no first source record")?;
        let last = records
            .last()
            .ok_or("live result has no last source record")?;
        let start_bucket =
            u64::try_from(canonical_unsigned(first, "bucket", "first source record")?)?;
        let end_bucket_exclusive = u64::try_from(
            canonical_unsigned(last, "bucket", "last source record")?
                .checked_add(1)
                .ok_or("live source window end overflows")?,
        )?;
        let builder =
            LocalTradingBuilder::campaign(payer, second_owner, start_bucket, end_bucket_exclusive)?;
        let admitted_archive = string(
            result
                .get("source_archive")
                .ok_or("live result has no source archive")?,
            "key",
            "source archive",
        )?;
        if builder.source_archive_address().to_string() != admitted_archive {
            return Err("typed local builder does not derive the admitted SourceArchive".into());
        }
        let plan = builder.freeze_epoch()?;
        if plan.schema != PLAN_SCHEMA || plan.required_signers != [SignerRole::Payer] {
            return Err("typed local builder emitted an unexpected construction contract".into());
        }
        Ok(json!({
            "type": "live-local-builder-construction",
            "schema": BUILDER_CONSTRUCTION_SCHEMA,
            "session_id": self.session_id,
            "boundary": "CONSTRUCTION ONLY / NO BLOCKHASH / NOT SIGNED / NOT SUBMITTED",
            "plan_schema": plan.schema,
            "family": plan.family,
            "source_archive": admitted_archive,
            "market": builder.market_address().to_string(),
            "source_window": {
                "start_bucket": start_bucket.to_string(),
                "end_bucket_exclusive": end_bucket_exclusive.to_string(),
            },
            "required_signers": ["payer"],
            "unsigned_transaction_sha256": body_sha256(&plan.unsigned_transaction),
            "unsigned_transaction_bytes": plan.unsigned_transaction.len().to_string(),
            "recent_blockhash_present": false,
            "signed": false,
            "submitted": false,
            "submission_signature": Value::Null,
            "transaction_bytes_exported": false,
            "private_key_material_exported": false,
            "browser_signing": false,
            "transaction_admission": "not inferred; this terminal-state plan proves construction continuity only",
        }))
    }
}

impl Drop for LocalSessionOwner {
    fn drop(&mut self) {
        let safe_name = self
            .root
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("clutch-pyth-live-session."));
        if safe_name {
            let _ignored = fs::remove_dir_all(&self.root);
        }
    }
}

struct ChildGuard(Child);

impl ChildGuard {
    fn child_mut(&mut self) -> &mut Child {
        &mut self.0
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            // The supervised runner owns the validator process while the
            // daemon owns its private session root. Give the runner's
            // EXIT/TERM trap time to stop the validator before the enclosing
            // session owner removes only that marked root.
            let pid = self.0.id().to_string();
            let _ignored = Command::new("kill").args(["-TERM", &pid]).status();
            for _ in 0..50 {
                if self.0.try_wait().ok().flatten().is_some() {
                    return;
                }
                thread::sleep(Duration::from_millis(100));
            }
            let _ignored = self.0.kill();
        }
        let _ignored = self.0.wait();
    }
}

#[derive(Debug)]
enum ReaderMessage {
    Line {
        stream: &'static str,
        line: String,
    },
    Error {
        stream: &'static str,
        detail: String,
    },
}

fn read_lines<R: std::io::Read + Send + 'static>(
    reader: R,
    stream: &'static str,
    sender: mpsc::Sender<ReaderMessage>,
) {
    for line in BufReader::new(reader).lines() {
        let message = match line {
            Ok(line) => ReaderMessage::Line { stream, line },
            Err(error) => ReaderMessage::Error {
                stream,
                detail: error.to_string(),
            },
        };
        if sender.send(message).is_err() {
            return;
        }
    }
}

fn exact_keys(value: &Map<String, Value>, expected: &[&str], role: &str) -> Result<()> {
    let actual = value.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(format!("{role} has unknown or missing fields").into());
    }
    Ok(())
}

fn object<'a>(value: &'a Value, role: &str) -> Result<&'a Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| format!("{role} is not a JSON object").into())
}

fn field_object<'a>(value: &'a Value, field: &str, role: &str) -> Result<&'a Map<String, Value>> {
    value
        .get(field)
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{role}.{field} is not an object").into())
}

fn field_array<'a>(value: &'a Value, field: &str, role: &str) -> Result<&'a Vec<Value>> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{role}.{field} is not an array").into())
}

fn string<'a>(value: &'a Value, field: &str, role: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| format!("{role}.{field} is not a nonempty string").into())
}

fn boolean(value: &Value, field: &str, role: &str) -> Result<bool> {
    value
        .get(field)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("{role}.{field} is not a boolean").into())
}

fn canonical_unsigned(value: &Value, field: &str, role: &str) -> Result<u128> {
    let text = string(value, field, role)?;
    if !text.bytes().all(|byte| byte.is_ascii_digit()) || (text.len() > 1 && text.starts_with('0'))
    {
        return Err(format!("{role}.{field} is not a canonical unsigned decimal string").into());
    }
    text.parse::<u128>()
        .map_err(|_| format!("{role}.{field} is outside u128").into())
}

fn lowercase_hex(value: &Value, field: &str, bytes: usize, role: &str) -> Result<()> {
    let text = string(value, field, role)?;
    if text.len() != bytes * 2
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(format!("{role}.{field} is not lowercase {bytes}-byte hex").into());
    }
    Ok(())
}

fn canonical_address(value: &Value, field: &str, role: &str) -> Result<[u8; 32]> {
    let address = string(value, field, role)?;
    let bytes = rpc::base58_decode_32(address)?;
    if clutch_sbf_harness::base58_of(&bytes) != address {
        return Err(format!("{role}.{field} is not canonical Solana base58").into());
    }
    Ok(bytes)
}

fn base58_signature_shape(value: &Value, field: &str, role: &str) -> Result<()> {
    const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    let signature = string(value, field, role)?;
    if !(64..=88).contains(&signature.len())
        || !signature.bytes().all(|byte| ALPHABET.contains(&byte))
    {
        return Err(
            format!("{role}.{field} is not a Solana signature-shaped base58 string").into(),
        );
    }
    Ok(())
}

fn no_json_numbers(value: &Value) -> bool {
    match value {
        Value::Number(_) => false,
        Value::Array(values) => values.iter().all(no_json_numbers),
        Value::Object(values) => values.values().all(no_json_numbers),
        _ => true,
    }
}

fn digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

/// Admit only source-authored, structurally bounded progress. Arbitrary child
/// stdout and all stderr stay process-local: cargo can print filesystem paths,
/// and a future tool must not accidentally turn a key-shaped line into browser
/// data. Structured manifest/result events are validated separately.
fn admitted_progress_line(stream: &str, line: &str) -> bool {
    if stream != "stdout" {
        return false;
    }
    if matches!(
        line,
        CLAIM
            | "campaign_mode=joined-multiboundary-v1"
            | "pinned loopback validator runtime: PASS"
            | "== seed exact locked dependency source offline =="
            | "== build standalone signed-RPC driver =="
            | "== build unmistakably test-only Clutch ELF =="
            | "== probe the same warped validator Clock before proof generation =="
            | "== start explicitly selected validator =="
            | "== real provider / joined Clutch campaign =="
            | "local_session_owner=ready"
            | "local_session_owner=stopping"
    ) || line == MULTIBOUNDARY_PASS
    {
        return true;
    }
    if let Some(clock) = line.strip_prefix("campaign_clock_settled=") {
        return digits(clock);
    }
    if let Some(count) = line
        .strip_prefix("prepared ")
        .and_then(|rest| rest.strip_suffix(" exact genesis accounts"))
    {
        return digits(count);
    }
    if let Some(rest) = line.strip_prefix("waiting reason=") {
        let Some((reason, counters)) = rest.split_once(" slot=") else {
            return false;
        };
        if !matches!(
            reason,
            "general epoch freeze" | "best-valid-submitted-candidate selection"
        ) {
            return false;
        }
        let Some((slot, target)) = counters.split_once(" target=") else {
            return false;
        };
        return digits(slot) && digits(target);
    }
    let Some(rest) = line.strip_prefix("step=") else {
        return false;
    };
    let Some((label, rest)) = rest.split_once(" slot=") else {
        return false;
    };
    let Some((slot, rest)) = rest.split_once(" cu=") else {
        return false;
    };
    let Some((compute, error)) = rest.split_once(" error=") else {
        return false;
    };
    !label.is_empty()
        && label
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && digits(slot)
        && digits(compute)
        && matches!(error, "null" | r#"{"InstructionError":[2,{"Custom":122}]}"#)
}

fn validate_common(value: &Value, schema: &str, role: &str) -> Result<()> {
    if !no_json_numbers(value) {
        return Err(format!("{role} contains a JSON number instead of exact decimal text").into());
    }
    if string(value, "schema", role)? != schema
        || string(value, "claim", role)? != CLAIM
        || string(value, "campaign_mode", role)? != CAMPAIGN_MODE
        || string(value, "transcript_schema", role)? != TRANSCRIPT_SCHEMA
        || boolean(value, "retained_transcript", role)?
    {
        return Err(format!("{role} identity or truth boundary differs").into());
    }
    Ok(())
}

fn validate_manifest(value: &Value) -> Result<()> {
    let role = "live real-Pyth manifest";
    exact_keys(
        object(value, role)?,
        &[
            "type",
            "schema",
            "claim",
            "campaign_mode",
            "transcript_schema",
            "retained_transcript",
            "repository_head",
            "program_id",
            "clutch_elf_sha256",
            "validator_binary_sha256",
            "source_profile_snapshot_sha256",
            "boundary_count",
            "provider",
            "genesis_prerequisite_roles",
        ],
        role,
    )?;
    validate_common(value, MANIFEST_SCHEMA, role)?;
    if string(value, "type", role)? != "live-real-pyth-manifest"
        || canonical_unsigned(value, "boundary_count", role)? != 2
    {
        return Err("live real-Pyth manifest type or boundary count differs".into());
    }
    lowercase_hex(value, "repository_head", 20, role)?;
    lowercase_hex(value, "clutch_elf_sha256", 32, role)?;
    lowercase_hex(value, "validator_binary_sha256", 32, role)?;
    lowercase_hex(value, "source_profile_snapshot_sha256", 32, role)?;
    let program = string(value, "program_id", role)?;
    let bytes = rpc::base58_decode_32(program)?;
    if clutch_sbf_harness::base58_of(&bytes) != program {
        return Err("live real-Pyth manifest program id is not canonical base58".into());
    }
    let providers = field_array(value, "provider", role)?;
    if providers.len() != 4 {
        return Err("live real-Pyth manifest does not carry four provider loader rows".into());
    }
    let expected = [
        ("receiver-program", true),
        ("receiver-programdata", false),
        ("router-program", true),
        ("router-programdata", false),
    ];
    for (row, (expected_role, expected_executable)) in providers.iter().zip(expected) {
        let row_role = format!("provider {expected_role}");
        exact_keys(
            object(row, &row_role)?,
            &[
                "role",
                "address",
                "complete_account_body_sha256",
                "executable",
            ],
            &row_role,
        )?;
        if string(row, "role", &row_role)? != expected_role
            || boolean(row, "executable", &row_role)? != expected_executable
        {
            return Err(format!("{row_role} identity differs").into());
        }
        lowercase_hex(row, "complete_account_body_sha256", 32, &row_role)?;
        let address = string(row, "address", &row_role)?;
        let bytes = rpc::base58_decode_32(address)?;
        if clutch_sbf_harness::base58_of(&bytes) != address {
            return Err(format!("{row_role} address is not canonical base58").into());
        }
    }
    let roles = field_array(value, "genesis_prerequisite_roles", role)?;
    if roles.is_empty()
        || roles
            .iter()
            .any(|entry| entry.as_str().is_none_or(str::is_empty))
    {
        return Err("live real-Pyth manifest has malformed genesis roles".into());
    }
    Ok(())
}

fn validate_rollback(
    value: &Value,
    role: &str,
    expected_kind: &str,
    expected_step: &str,
) -> Result<()> {
    exact_keys(
        object(value, role)?,
        &[
            "ok",
            "attempt_kind",
            "attempt_identity",
            "ephemeral_update_account",
            "ephemeral_update_absent_after_refusal",
            "refusal_step_label",
            "refusal_signature",
            "instruction_error",
            "snapshot_encoding",
            "snapshot_domain",
            "watched_accounts",
            "before_snapshot_sha256",
            "after_snapshot_sha256",
            "snapshots_equal",
        ],
        role,
    )?;
    if !boolean(value, "ok", role)?
        || !boolean(value, "ephemeral_update_absent_after_refusal", role)?
        || !boolean(value, "snapshots_equal", role)?
        || string(value, "attempt_kind", role)? != expected_kind
        || string(value, "refusal_step_label", role)? != expected_step
        || string(value, "snapshot_domain", role)? != ROLLBACK_SNAPSHOT_DOMAIN
        || string(value, "snapshot_encoding", role)? != ROLLBACK_SNAPSHOT_ENCODING
    {
        return Err(format!("{role} identity or closure differs").into());
    }
    let ephemeral = canonical_address(value, "ephemeral_update_account", role)?;
    if ephemeral.iter().all(|byte| *byte == 0) {
        return Err(format!("{role} ephemeral update account is zero").into());
    }
    base58_signature_shape(value, "refusal_signature", role)?;
    lowercase_hex(value, "before_snapshot_sha256", 32, role)?;
    lowercase_hex(value, "after_snapshot_sha256", 32, role)?;
    if string(value, "before_snapshot_sha256", role)?
        != string(value, "after_snapshot_sha256", role)?
    {
        return Err(format!("{role} full-state hashes differ").into());
    }

    let instruction = field_object(value, "instruction_error", role)?;
    let instruction = Value::Object(instruction.clone());
    exact_keys(
        object(&instruction, "rollback instruction error")?,
        &["instruction_index", "custom_code", "custom_code_hex"],
        "rollback instruction error",
    )?;
    if canonical_unsigned(&instruction, "instruction_index", role)? != 2
        || canonical_unsigned(&instruction, "custom_code", role)? != 122
        || string(&instruction, "custom_code_hex", role)? != "0x7a"
    {
        return Err(format!("{role} refusal differs").into());
    }

    let watched = field_array(value, "watched_accounts", role)?;
    if watched.len() != 2 {
        return Err(format!("{role} does not bind two rollback targets").into());
    }
    let mut watched_addresses = Vec::with_capacity(2);
    for (index, (row, expected_role)) in watched
        .iter()
        .zip(["source_archive", "receiver_treasury"])
        .enumerate()
    {
        let row_role = format!("{role} watched account {index}");
        exact_keys(object(row, &row_role)?, &["role", "address"], &row_role)?;
        if string(row, "role", &row_role)? != expected_role {
            return Err(format!("{row_role} role differs").into());
        }
        let address = canonical_address(row, "address", &row_role)?;
        if address.iter().all(|byte| *byte == 0) {
            return Err(format!("{row_role} address is zero").into());
        }
        watched_addresses.push(address);
    }
    if watched_addresses[0] == watched_addresses[1] {
        return Err(format!("{role} aliases its watched accounts").into());
    }

    let identity = field_object(value, "attempt_identity", role)?;
    let identity = Value::Object(identity.clone());
    match expected_kind {
        "wrong_config" => {
            exact_keys(
                object(&identity, "wrong-config identity")?,
                &["attempted_config_account", "registered_config_account"],
                "wrong-config identity",
            )?;
            let attempted = canonical_address(
                &identity,
                "attempted_config_account",
                "wrong-config identity",
            )?;
            let registered = canonical_address(
                &identity,
                "registered_config_account",
                "wrong-config identity",
            )?;
            if attempted == registered {
                return Err("wrong-config rollback substitutes the registered config".into());
            }
        }
        "wrong_feed" => {
            exact_keys(
                object(&identity, "wrong-feed identity")?,
                &[
                    "attempted_provider_feed_id",
                    "registered_provider_feed_id",
                    "verified_vaa_account",
                ],
                "wrong-feed identity",
            )?;
            lowercase_hex(
                &identity,
                "attempted_provider_feed_id",
                32,
                "wrong-feed identity",
            )?;
            lowercase_hex(
                &identity,
                "registered_provider_feed_id",
                32,
                "wrong-feed identity",
            )?;
            if string(
                &identity,
                "attempted_provider_feed_id",
                "wrong-feed identity",
            )? == string(
                &identity,
                "registered_provider_feed_id",
                "wrong-feed identity",
            )? {
                return Err("wrong-feed rollback substitutes the registered feed".into());
            }
            canonical_address(&identity, "verified_vaa_account", "wrong-feed identity")?;
        }
        "out_of_order_boundary" => {
            exact_keys(
                object(&identity, "out-of-order identity")?,
                &[
                    "attempted_boundary_index",
                    "expected_next_boundary_index",
                    "attempted_publish_time",
                    "expected_next_publish_time",
                ],
                "out-of-order identity",
            )?;
            if canonical_unsigned(
                &identity,
                "attempted_boundary_index",
                "out-of-order identity",
            )? != 1
                || canonical_unsigned(
                    &identity,
                    "expected_next_boundary_index",
                    "out-of-order identity",
                )? != 0
            {
                return Err("out-of-order rollback boundary relation differs".into());
            }
            let attempted =
                canonical_unsigned(&identity, "attempted_publish_time", "out-of-order identity")?;
            let expected = canonical_unsigned(
                &identity,
                "expected_next_publish_time",
                "out-of-order identity",
            )?;
            if expected.checked_add(60) != Some(attempted) {
                return Err("out-of-order rollback publish-time relation differs".into());
            }
        }
        _ => return Err("unsupported rollback kind".into()),
    }
    Ok(())
}

fn validate_liabilities(terminal: &Value) -> Result<()> {
    let liabilities = field_object(terminal, "liabilities", "terminal lifecycle")?;
    let liabilities = Value::Object(liabilities.clone());
    exact_keys(
        object(&liabilities, "terminal liabilities")?,
        &["all_zero", "supply_ledger", "outcome_mints"],
        "terminal liabilities",
    )?;
    if !boolean(&liabilities, "all_zero", "terminal liabilities")? {
        return Err("terminal liabilities are not all zero".into());
    }
    let ledger = field_object(&liabilities, "supply_ledger", "terminal liabilities")?;
    let ledger = Value::Object(ledger.clone());
    exact_keys(
        object(&ledger, "terminal SupplyLedger")?,
        &[
            "address",
            "outcome_count",
            "internal_supply",
            "external_supply",
            "aggregate_supply",
        ],
        "terminal SupplyLedger",
    )?;
    let ledger_address = canonical_address(&ledger, "address", "terminal SupplyLedger")?;
    if ledger_address.iter().all(|byte| *byte == 0)
        || canonical_unsigned(&ledger, "outcome_count", "terminal SupplyLedger")? != 4
    {
        return Err("terminal SupplyLedger identity differs".into());
    }
    for field in ["internal_supply", "external_supply", "aggregate_supply"] {
        let values = field_array(&ledger, field, "terminal SupplyLedger")?;
        if values.len() != 4 || values.iter().any(|value| value.as_str() != Some("0")) {
            return Err(format!("terminal SupplyLedger.{field} is not exactly zero").into());
        }
    }
    let mints = field_array(&liabilities, "outcome_mints", "terminal liabilities")?;
    if mints.len() != 4 {
        return Err("terminal liabilities do not bind four outcome mints".into());
    }
    let mut addresses = BTreeSet::new();
    for (index, mint) in mints.iter().enumerate() {
        let role = format!("terminal outcome mint {index}");
        exact_keys(
            object(mint, &role)?,
            &["outcome_index", "address", "supply"],
            &role,
        )?;
        if canonical_unsigned(mint, "outcome_index", &role)? != index as u128
            || canonical_unsigned(mint, "supply", &role)? != 0
        {
            return Err(format!("{role} index or supply differs").into());
        }
        let address = canonical_address(mint, "address", &role)?;
        if address.iter().all(|byte| *byte == 0) || !addresses.insert(address) {
            return Err(format!("{role} address is zero or aliased").into());
        }
    }
    Ok(())
}

fn validate_result(value: &Value) -> Result<()> {
    let role = "live real-Pyth result";
    exact_keys(
        object(value, role)?,
        &[
            "type",
            "schema",
            "claim",
            "campaign_mode",
            "transcript_schema",
            "retained_transcript",
            "genesis_hash",
            "boundary_count",
            "step_count",
            "sealed",
            "resolved_payout",
            "archive_records",
            "source_archive",
            "wrong_config_rollback",
            "wrong_feed_rollback",
            "out_of_order_boundary_rollback",
            "trade_status",
            "collateral_atoms",
            "terminal",
        ],
        role,
    )?;
    validate_common(value, RESULT_SCHEMA, role)?;
    if string(value, "type", role)? != "live-real-pyth-result"
        || canonical_unsigned(value, "boundary_count", role)? != 2
        || canonical_unsigned(value, "step_count", role)? != 56
        || canonical_unsigned(value, "resolved_payout", role)? != 1
        || canonical_unsigned(value, "collateral_atoms", role)? != 128
        || !boolean(value, "sealed", role)?
        || string(value, "trade_status", role)? != "settled"
    {
        return Err("live real-Pyth result terminal summary differs".into());
    }
    let records = field_array(value, "archive_records", role)?;
    if records.len() != 2 {
        return Err("live real-Pyth result does not carry two archive records".into());
    }
    let mut previous_bucket = None;
    let mut previous_publish_time = None;
    for (index, record) in records.iter().enumerate() {
        let record_role = format!("archive record {index}");
        exact_keys(
            object(record, &record_role)?,
            &[
                "index",
                "bucket",
                "lower",
                "upper",
                "sequence",
                "write_slot",
                "publish_time",
            ],
            &record_role,
        )?;
        if canonical_unsigned(record, "index", &record_role)? != index as u128 {
            return Err(format!("{record_role} index differs").into());
        }
        let bucket = canonical_unsigned(record, "bucket", &record_role)?;
        let lower = canonical_unsigned(record, "lower", &record_role)?;
        let upper = canonical_unsigned(record, "upper", &record_role)?;
        let sequence = canonical_unsigned(record, "sequence", &record_role)?;
        let publish_time = canonical_unsigned(record, "publish_time", &record_role)?;
        canonical_unsigned(record, "write_slot", &record_role)?;
        if lower > upper || sequence != publish_time {
            return Err(format!("{record_role} interval or sequence differs").into());
        }
        if let Some(previous) = previous_bucket {
            if bucket != previous + 1 {
                return Err("live real-Pyth archive buckets are not consecutive".into());
            }
        }
        if let Some(previous) = previous_publish_time {
            if publish_time != previous + 60 {
                return Err("live real-Pyth publish times are not 60 seconds apart".into());
            }
        }
        previous_bucket = Some(bucket);
        previous_publish_time = Some(publish_time);
    }

    let archive = field_object(value, "source_archive", role)?;
    exact_keys(
        archive,
        &[
            "key",
            "owner",
            "executable",
            "data_len",
            "body_sha256",
            "page_commitment",
            "feed_id",
            "window_id",
            "record_count",
        ],
        "source archive",
    )?;
    if boolean(
        &Value::Object(archive.clone()),
        "executable",
        "source archive",
    )? || canonical_unsigned(
        &Value::Object(archive.clone()),
        "data_len",
        "source archive",
    )? != 2_560
        || canonical_unsigned(
            &Value::Object(archive.clone()),
            "record_count",
            "source archive",
        )? != 2
    {
        return Err("live real-Pyth source archive envelope differs".into());
    }
    for field in ["body_sha256", "page_commitment", "feed_id", "window_id"] {
        lowercase_hex(&Value::Object(archive.clone()), field, 32, "source archive")?;
    }
    for field in ["key", "owner"] {
        let address = archive
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("source archive.{field} is absent"))?;
        let bytes = rpc::base58_decode_32(address)?;
        if clutch_sbf_harness::base58_of(&bytes) != address {
            return Err(format!("source archive.{field} is not canonical base58").into());
        }
    }

    validate_rollback(
        value
            .get("wrong_config_rollback")
            .ok_or("live result has no wrong-config rollback")?,
        "wrong-config rollback",
        "wrong_config",
        "wrong-config-post-update-plus-append-rollback",
    )?;
    validate_rollback(
        value
            .get("wrong_feed_rollback")
            .ok_or("live result has no wrong-feed rollback")?,
        "wrong-feed rollback",
        "wrong_feed",
        "wrong-feed-post-update-plus-append-rollback",
    )?;
    validate_rollback(
        value
            .get("out_of_order_boundary_rollback")
            .ok_or("live result has no out-of-order rollback")?,
        "out-of-order rollback",
        "out_of_order_boundary",
        "out-of-order-boundary-post-update-plus-append-rollback",
    )?;

    let terminal = field_object(value, "terminal", role)?;
    let terminal_value = Value::Object(terminal.clone());
    for (field, expected) in [
        ("buyer_position_cash_atoms", 0),
        ("seller_position_cash_atoms", 0),
        ("hoard_collateral_atoms", 0),
        ("hoard_token_atoms", 0),
        ("buyer_token_atoms", 76),
        ("seller_token_atoms", 52),
    ] {
        if canonical_unsigned(&terminal_value, field, "terminal lifecycle")? != expected {
            return Err(format!("terminal lifecycle.{field} differs").into());
        }
    }
    for field in [
        "buyer_position_internal",
        "seller_position_internal",
        "supply_internal",
    ] {
        let values = terminal
            .get(field)
            .and_then(Value::as_array)
            .ok_or_else(|| format!("terminal lifecycle.{field} is not an array"))?;
        if values.len() != 4 || values.iter().any(|value| value.as_str() != Some("0")) {
            return Err(format!("terminal lifecycle.{field} is not zero").into());
        }
    }
    validate_liabilities(&terminal_value)?;
    Ok(())
}

fn parse_operator_event(line: &str) -> Result<Option<Value>> {
    let Some(payload) = line.strip_prefix(EVENT_PREFIX) else {
        return Ok(None);
    };
    let event: Value = serde_json::from_str(payload)
        .map_err(|error| format!("malformed live Operator event: {error}"))?;
    match event.get("type").and_then(Value::as_str) {
        Some("live-real-pyth-manifest") => validate_manifest(&event)?,
        Some("live-real-pyth-result") => validate_result(&event)?,
        other => return Err(format!("unknown live Operator event type {other:?}").into()),
    }
    Ok(Some(event))
}

fn body_sha256(data: &[u8]) -> String {
    clutch_sbf_harness::hex_encode(solana_sha256_hasher::hash(data).to_bytes().as_slice())
}

fn required_envelope<'a>(
    snapshot: &'a rpc::GraphSnapshotV2,
    address: &str,
    role: &str,
) -> Result<&'a rpc::AccountEnvelope> {
    snapshot
        .accounts
        .get(address)
        .ok_or_else(|| format!("chain discovery omitted {role} {address}"))?
        .as_ref()
        .ok_or_else(|| format!("chain discovery found {role} {address} absent").into())
}

fn discovered_account(
    role: &str,
    address: &str,
    envelope: &rpc::AccountEnvelope,
    account_schema: &str,
) -> Value {
    json!({
        "role": role,
        "address": address,
        "address_source": "admitted-live-result",
        "owner": envelope.owner,
        "executable": envelope.executable,
        "lamports": envelope.lamports.to_string(),
        "data_len": envelope.data.len().to_string(),
        "body_sha256": body_sha256(&envelope.data),
        "account_schema": account_schema,
    })
}

/// Independently rediscover the terminal source/liability roots from the live
/// bank before the campaign child is allowed to exit and remove its ledger.
/// Addresses come from the already-admitted result; owner, executable, bytes,
/// and one shared RPC context come from a root-bracketed chain read.
fn discover_terminal_chain(
    result: &Value,
    manifest: &Value,
    options: &Options,
    owner: &LocalSessionOwner,
) -> Result<Value> {
    let archive = result
        .get("source_archive")
        .ok_or("live result has no source archive")?;
    let archive_address = string(archive, "key", "source archive")?;
    let expected_program = string(manifest, "program_id", "live manifest")?;
    if string(archive, "owner", "source archive")? != expected_program {
        return Err("source archive result owner differs from the admitted program".into());
    }
    let terminal = result
        .get("terminal")
        .ok_or("live result has no terminal lifecycle")?;
    let liabilities = terminal
        .get("liabilities")
        .ok_or("terminal lifecycle has no liabilities")?;
    let ledger = liabilities
        .get("supply_ledger")
        .ok_or("terminal liabilities have no SupplyLedger")?;
    let ledger_address = string(ledger, "address", "terminal SupplyLedger")?.to_string();
    let mints = field_array(liabilities, "outcome_mints", "terminal liabilities")?;
    let mint_addresses = mints
        .iter()
        .enumerate()
        .map(|(index, mint)| {
            Ok(string(mint, "address", &format!("terminal outcome mint {index}"))?.to_string())
        })
        .collect::<Result<Vec<_>>>()?;
    let mut children = Vec::with_capacity(5);
    children.push(ledger_address.clone());
    children.extend(mint_addresses.iter().cloned());
    let url = format!("http://127.0.0.1:{}", options.rpc_port);
    let snapshot = rpc::graph_snapshot_v2(&url, archive_address, &children)?;

    let archive_envelope = required_envelope(&snapshot, archive_address, "SourceArchive")?;
    if archive_envelope.owner != expected_program
        || archive_envelope.executable
        || archive_envelope.data.len()
            != clutch_sbf::source_archive_v2::SOURCE_ARCHIVE_ACCOUNT_V2_BYTES
        || archive_envelope.data.first().copied() != Some(registry::SOURCE_ARCHIVE_V2_ACCOUNT_TAG)
        || archive_envelope.data.get(1).copied()
            != Some(registry::SOURCE_ARCHIVE_V2_ACCOUNT_VERSION)
        || body_sha256(&archive_envelope.data) != string(archive, "body_sha256", "source archive")?
    {
        return Err("chain-discovered SourceArchive envelope or bytes differ".into());
    }

    let ledger_envelope = required_envelope(&snapshot, &ledger_address, "SupplyLedger")?;
    if ledger_envelope.owner != expected_program
        || ledger_envelope.executable
        || ledger_envelope.data.len() != account_len::SUPPLY_LEDGER
    {
        return Err("chain-discovered SupplyLedger envelope differs".into());
    }
    let decoded_ledger = SupplyLedgerAccount::decode(&ledger_envelope.data)
        .map_err(|error| format!("chain-discovered SupplyLedger does not decode: {error:?}"))?;
    decoded_ledger
        .validate()
        .map_err(|error| format!("chain-discovered SupplyLedger is invalid: {error:?}"))?;
    if decoded_ledger.outcome_count != 4
        || decoded_ledger.internal_supply[..4]
            .iter()
            .chain(&decoded_ledger.external_supply[..4])
            .any(|amount| *amount != 0)
    {
        return Err("chain-discovered SupplyLedger retains a liability".into());
    }

    let token_program =
        clutch_sbf_harness::base58_of(&clutch_solana_layout::collateral::TOKEN_2022_PROGRAM);
    let mut accounts = vec![
        discovered_account(
            "source_archive",
            archive_address,
            archive_envelope,
            "source-archive-v2/exact-2560",
        ),
        discovered_account(
            "supply_ledger",
            &ledger_address,
            ledger_envelope,
            "supply-ledger/v2-exact",
        ),
    ];
    for (index, address) in mint_addresses.iter().enumerate() {
        let role = format!("outcome_mint.{index}");
        let envelope = required_envelope(&snapshot, address, &role)?;
        if envelope.owner != token_program || envelope.executable || envelope.data.len() != 82 {
            return Err(format!("chain-discovered {role} envelope differs").into());
        }
        let observation = clutch_sbf::token::observe_mint(&envelope.data).map_err(|error| {
            format!("chain-discovered {role} is not a canonical mint: {error:?}")
        })?;
        if observation.supply != 0 || observation.decimals != 0 || observation.extensions != 0 {
            return Err(
                format!("chain-discovered {role} retains supply or a widened shape").into(),
            );
        }
        accounts.push(discovered_account(
            &role,
            address,
            envelope,
            "token-2022-base-mint/exact-82",
        ));
    }

    Ok(json!({
        "type": "live-real-pyth-chain-discovery",
        "schema": CHAIN_SCHEMA,
        "mode": MODE,
        "authority": "loopback RPC graph-root-bracketed same-context account envelopes",
        "context_slot": snapshot.context_slot.to_string(),
        "attempts": snapshot.attempts.to_string(),
        "root_role": "source_archive",
        "root_address": archive_address,
        "program_id": expected_program,
        "token_program": token_program,
        "accounts": accounts,
        "restart_descriptor": {
            "schema": "dragons-clutch/operator/local-session-restart-descriptor/v1",
            "session_id": owner.session_id,
            "genesis_hash": string(result, "genesis_hash", "live result")?,
            "repository_head": string(manifest, "repository_head", "live manifest")?,
            "rpc_url": url,
            "program_id": expected_program,
            "source_archive": archive_address,
            "supply_ledger": ledger_address,
            "outcome_mints": mint_addresses,
            "public_only": true,
            "signer_material": "not exported",
            "restart_capability": "read-only rediscovery while the daemon-owned child is live; local signer continuity is owner-scoped but transaction admission is not yet exposed",
        }
    }))
}

fn run_event(options: &Options, phase: &str) -> Value {
    json!({
        "type": "live-real-pyth-run",
        "schema": RUN_SCHEMA,
        "mode": MODE,
        "phase": phase,
        "campaign_mode": CAMPAIGN_MODE,
        "retained_transcript": false,
        "rpc_url": format!("http://127.0.0.1:{}", options.rpc_port),
        "websocket_url": format!("ws://127.0.0.1:{}", options.rpc_port + 1),
        "faucet": format!("127.0.0.1:{}", options.faucet_port),
        "gossip": format!("127.0.0.1:{}", options.gossip_port),
        "dynamic_port_range": options.dynamic_port_range,
        "authority": "read-only live child telemetry; no retained transcript; no browser key material",
    })
}

fn identity(manifest: &Value, options: &Options, scope: &str, genesis_hash: Option<&str>) -> Value {
    json!({
        "type": "identity",
        "schema": RUN_SCHEMA,
        "mode": MODE,
        "integer_transport": integer::TRANSPORT,
        "source_profile": PROFILE,
        "elf_sha256": manifest["clutch_elf_sha256"],
        "program_id": manifest["program_id"],
        "repository_head": manifest["repository_head"],
        "validator_binary_sha256": manifest["validator_binary_sha256"],
        "source_profile_snapshot_sha256": manifest["source_profile_snapshot_sha256"],
        "rpc_url": format!("http://127.0.0.1:{}", options.rpc_port),
        "genesis_hash": genesis_hash,
        "genesis_assisted": true,
        "precreated": manifest["genesis_prerequisite_roles"],
        "evidence_scope": scope,
        "promotion": "unpromoted",
        "network": "LOCAL; OPERATOR HTTP AND VALIDATOR SERVICES LOOPBACK ONLY",
        "observation": "SYNTHETIC OBSERVATION",
        "retained_transcript": false,
        "value": "no value",
    })
}

fn publish_line(
    bus: &Bus,
    message: ReaderMessage,
    sequence: &mut u64,
    manifest: &mut Option<Value>,
    result: &mut Option<Value>,
    options: &Options,
    owner: &LocalSessionOwner,
) -> Result<()> {
    match message {
        ReaderMessage::Error { stream, detail } => {
            return Err(format!("reading live child {stream}: {detail}").into());
        }
        ReaderMessage::Line { stream, line } => {
            if line.len() > MAX_OUTPUT_LINE_BYTES {
                return Err(format!("live child {stream} emitted an oversized line").into());
            }
            if stream == "stdout" {
                if let Some(event) = parse_operator_event(&line)? {
                    match event.get("type").and_then(Value::as_str) {
                        Some("live-real-pyth-manifest") => {
                            if manifest.replace(event.clone()).is_some() {
                                return Err("live child emitted a second manifest event".into());
                            }
                            bus.publish(&event);
                            bus.publish(&owner.signer_event()?);
                            bus.publish(&identity(&event, options, "IN_FLIGHT", None));
                            bus.publish(&run_event(options, "running"));
                        }
                        Some("live-real-pyth-result") => {
                            let discovery = discover_terminal_chain(
                                &event,
                                manifest.as_ref().ok_or(
                                    "live child emitted a result before its admitted manifest",
                                )?,
                                options,
                                owner,
                            )?;
                            let builder_construction = owner.builder_construction_event(&event)?;
                            if result.replace(event.clone()).is_some() {
                                return Err("live child emitted a second result event".into());
                            }
                            owner.retain_public_restart(
                                discovery
                                    .get("restart_descriptor")
                                    .ok_or("chain discovery has no restart descriptor")?,
                            )?;
                            bus.publish(&discovery);
                            bus.publish(&builder_construction);
                            bus.publish(&event);
                            bus.publish(&run_event(options, "session-ready"));
                            if options.exit_when_done {
                                owner.request_stop()?;
                            }
                        }
                        _ => unreachable!("parse_operator_event admitted the event"),
                    }
                    return Ok(());
                }
            }
            if admitted_progress_line(stream, &line) {
                *sequence = sequence
                    .checked_add(1)
                    .ok_or("live output sequence overflow")?;
                bus.publish(&json!({
                    "type": "live-output",
                    "schema": RUN_SCHEMA,
                    "sequence": sequence.to_string(),
                    "stream": "stdout",
                    "text": line,
                }));
            }
        }
    }
    Ok(())
}

fn supervise(options: &Options, bus: &Bus) -> Result<(Value, Value, ExitStatus)> {
    let owner = LocalSessionOwner::create(options.work.as_ref())?;
    let script =
        crate::repo_path("programs/clutch-sbf/scripts/run_local_multiboundary_pyth_lifecycle.sh");
    let mut command = Command::new(&script);
    owner.configure(&mut command);
    command
        .current_dir(crate::repo_path("."))
        .env("CLUTCH_LOCAL_REAL_PYTH_OPERATOR_EVENTS", "1")
        .env("CLUTCH_LOCAL_REAL_PYTH_KEEP_WORK", "0")
        .env(
            "CLUTCH_LOCAL_REAL_PYTH_RPC_PORT",
            options.rpc_port.to_string(),
        )
        .env(
            "CLUTCH_LOCAL_REAL_PYTH_FAUCET_PORT",
            options.faucet_port.to_string(),
        )
        .env(
            "CLUTCH_LOCAL_REAL_PYTH_GOSSIP_PORT",
            options.gossip_port.to_string(),
        )
        .env(
            "CLUTCH_LOCAL_REAL_PYTH_DYNAMIC_PORT_RANGE",
            &options.dynamic_port_range,
        )
        .env_remove("CLUTCH_LOCAL_REAL_PYTH_TRANSCRIPT_DIR")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = ChildGuard(command.spawn()?);
    let stdout = child
        .child_mut()
        .stdout
        .take()
        .ok_or("live child stdout was not piped")?;
    let stderr = child
        .child_mut()
        .stderr
        .take()
        .ok_or("live child stderr was not piped")?;
    let (sender, receiver) = mpsc::channel();
    let stdout_sender = sender.clone();
    let stdout_thread = thread::spawn(move || read_lines(stdout, "stdout", stdout_sender));
    let stderr_thread = thread::spawn(move || read_lines(stderr, "stderr", sender));

    let mut manifest = None;
    let mut result = None;
    let mut sequence = 0_u64;
    let status = loop {
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(message) => publish_line(
                bus,
                message,
                &mut sequence,
                &mut manifest,
                &mut result,
                options,
                &owner,
            )?,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if let Some(status) = child.child_mut().try_wait()? {
                    break status;
                }
            }
        }
        if let Some(status) = child.child_mut().try_wait()? {
            break status;
        }
    };
    stdout_thread
        .join()
        .map_err(|_| "live stdout reader panicked")?;
    stderr_thread
        .join()
        .map_err(|_| "live stderr reader panicked")?;
    while let Ok(message) = receiver.try_recv() {
        publish_line(
            bus,
            message,
            &mut sequence,
            &mut manifest,
            &mut result,
            options,
            &owner,
        )?;
    }
    Ok((
        manifest.ok_or("live child exited without an admitted manifest event")?,
        result.ok_or("live child exited without an admitted result event")?,
        status,
    ))
}

pub fn serve(options: Options) -> Result<()> {
    toolchain::validate_validator_network(
        Some(options.port),
        options.rpc_port,
        options.faucet_port,
        options.gossip_port,
        &options.dynamic_port_range,
    )?;
    let url = format!("http://127.0.0.1:{}", options.rpc_port);
    rpc::require_loopback(&url)?;
    toolchain::refuse_occupied_port(&url)?;

    let bus = Bus::new();
    let action: http::Action = Arc::new(|request: &Value| {
        if request.get("action").and_then(Value::as_str) == Some("ping") {
            json!({
                "ok": true,
                "mode": MODE,
                "authority": "read-only live child telemetry; the browser cannot start or alter the campaign",
            })
        } else {
            json!({
                "ok": false,
                "detail": "the live real-Pyth surface has no campaign-authoring action",
            })
        }
    });
    let server = http::Server::bind(
        options.port,
        Arc::clone(&bus),
        options.statics.clone(),
        action,
    )?;
    let port = server.port()?;
    thread::spawn(move || server.serve_forever());
    bus.publish(&run_event(&options, "starting"));
    println!("Operator Bench (live local-real Pyth): http://127.0.0.1:{port}/");

    match supervise(&options, &bus) {
        Ok((manifest, result, status)) if status.success() => {
            bus.publish(&identity(
                &manifest,
                &options,
                "SBF_EXECUTED",
                result.get("genesis_hash").and_then(Value::as_str),
            ));
            bus.publish(&run_event(&options, "passed"));
            bus.publish(&json!({
                "type": "done",
                "verdict": "PASS",
                "scope": "SBF_EXECUTED",
                "promotion": "unpromoted",
                "mode": MODE,
                "retained_transcript": false,
            }));
        }
        Ok((_manifest, _result, status)) => {
            let detail = format!("live real-Pyth child exited with {status}");
            bus.publish(&run_event(&options, "failed"));
            bus.publish(&json!({"type": "fault", "text": detail}));
            return Err("live real-Pyth child refused or failed".into());
        }
        Err(error) => {
            bus.publish(&run_event(&options, "failed"));
            bus.publish(&json!({"type": "fault", "text": error.to_string()}));
            return Err(error);
        }
    }

    if options.exit_when_done {
        thread::sleep(Duration::from_secs(2));
        return Ok(());
    }
    loop {
        thread::sleep(Duration::from_secs(3600));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> Value {
        json!({
            "type": "live-real-pyth-manifest",
            "schema": MANIFEST_SCHEMA,
            "claim": CLAIM,
            "campaign_mode": CAMPAIGN_MODE,
            "transcript_schema": TRANSCRIPT_SCHEMA,
            "retained_transcript": false,
            "repository_head": "a".repeat(40),
            "program_id": "p2YiDXJNN89JVt4BZmZo6TJQBfCNfHTgJZ8Y5F6LnMZ",
            "clutch_elf_sha256": "b".repeat(64),
            "validator_binary_sha256": "c".repeat(64),
            "source_profile_snapshot_sha256": "d".repeat(64),
            "boundary_count": "2",
            "provider": [
                {"role":"receiver-program","address":"rec2HHDDnjLfj4kE7VyEtFA1HPGQLK33259532cRyHp","complete_account_body_sha256":"1".repeat(64),"executable":true},
                {"role":"receiver-programdata","address":"3UV7w2yTaqVcUAbWm1KUXdcE1Ziw8CfyyCpZvhKFkPfX","complete_account_body_sha256":"2".repeat(64),"executable":false},
                {"role":"router-program","address":"HDw2E7P8X1SkCyjvoGsfBGAVUutKcj874bXjHrpVYrVL","complete_account_body_sha256":"3".repeat(64),"executable":true},
                {"role":"router-programdata","address":"9hLWdeVhSG9ufuQFA5d6zUoZ6qXoMRWrS8i4HGFHnR1x","complete_account_body_sha256":"4".repeat(64),"executable":false}
            ],
            "genesis_prerequisite_roles": ["clutch-program", "receiver-program"],
        })
    }

    fn test_address(byte: u8) -> String {
        clutch_sbf_harness::base58_of(&[byte; 32])
    }

    fn rollback(kind: &str, label: &str, identity: Value, ephemeral: u8) -> Value {
        let snapshot = "e".repeat(64);
        json!({
            "ok":true,
            "attempt_kind":kind,
            "attempt_identity":identity,
            "ephemeral_update_account":test_address(ephemeral),
            "ephemeral_update_absent_after_refusal":true,
            "refusal_step_label":label,
            "refusal_signature":"3".repeat(88),
            "instruction_error":{"instruction_index":"2","custom_code":"122","custom_code_hex":"0x7a"},
            "snapshot_encoding":ROLLBACK_SNAPSHOT_ENCODING,
            "snapshot_domain":ROLLBACK_SNAPSHOT_DOMAIN,
            "watched_accounts":[
                {"role":"source_archive","address":test_address(40)},
                {"role":"receiver_treasury","address":test_address(41)}
            ],
            "before_snapshot_sha256":snapshot,
            "after_snapshot_sha256":snapshot,
            "snapshots_equal":true
        })
    }

    fn result() -> Value {
        json!({
            "type": "live-real-pyth-result",
            "schema": RESULT_SCHEMA,
            "claim": CLAIM,
            "campaign_mode": CAMPAIGN_MODE,
            "transcript_schema": TRANSCRIPT_SCHEMA,
            "retained_transcript": false,
            "genesis_hash": "Hi2mifydJYhedaYqjufcfiwBarA3RravnmGZEb8Pdupr",
            "boundary_count": "2",
            "step_count": "56",
            "sealed": true,
            "resolved_payout": "1",
            "archive_records": [
                {"index":"0","bucket":"29792340","lower":"99980929","upper":"100019071","sequence":"1787540400","write_slot":"460336348","publish_time":"1787540400"},
                {"index":"1","bucket":"29792341","lower":"99980929","upper":"100019071","sequence":"1787540460","write_slot":"460336349","publish_time":"1787540460"}
            ],
            "source_archive": {
                "key":"3R9qSxN4uBLeubEyUvLGmTGkLQTPAXyP5Dk72H4Ybx9z",
                "owner":"p2YiDXJNN89JVt4BZmZo6TJQBfCNfHTgJZ8Y5F6LnMZ",
                "executable":false,
                "data_len":"2560",
                "body_sha256":"5".repeat(64),
                "page_commitment":"6".repeat(64),
                "feed_id":"7".repeat(64),
                "window_id":"8".repeat(64),
                "record_count":"2"
            },
            "wrong_config_rollback": rollback(
                "wrong_config",
                "wrong-config-post-update-plus-append-rollback",
                json!({
                    "attempted_config_account":test_address(42),
                    "registered_config_account":test_address(43)
                }),
                44,
            ),
            "wrong_feed_rollback": rollback(
                "wrong_feed",
                "wrong-feed-post-update-plus-append-rollback",
                json!({
                    "attempted_provider_feed_id":"9".repeat(64),
                    "registered_provider_feed_id":"a".repeat(64),
                    "verified_vaa_account":test_address(45)
                }),
                46,
            ),
            "out_of_order_boundary_rollback": rollback(
                "out_of_order_boundary",
                "out-of-order-boundary-post-update-plus-append-rollback",
                json!({
                    "attempted_boundary_index":"1",
                    "expected_next_boundary_index":"0",
                    "attempted_publish_time":"1787540460",
                    "expected_next_publish_time":"1787540400"
                }),
                47,
            ),
            "trade_status":"settled",
            "collateral_atoms":"128",
            "terminal": {
                "buyer_position_cash_atoms":"0",
                "buyer_position_internal":["0","0","0","0"],
                "seller_position_cash_atoms":"0",
                "seller_position_internal":["0","0","0","0"],
                "supply_internal":["0","0","0","0"],
                "hoard_collateral_atoms":"0",
                "hoard_token_atoms":"0",
                "buyer_token_atoms":"76",
                "seller_token_atoms":"52",
                "liabilities": {
                    "all_zero":true,
                    "supply_ledger":{
                        "address":test_address(48),
                        "outcome_count":"4",
                        "internal_supply":["0","0","0","0"],
                        "external_supply":["0","0","0","0"],
                        "aggregate_supply":["0","0","0","0"]
                    },
                    "outcome_mints":[0_u8,1,2,3].into_iter().map(|index| json!({
                        "outcome_index":index.to_string(),
                        "address":test_address(50 + index),
                        "supply":"0"
                    })).collect::<Vec<_>>()
                }
            }
        })
    }

    #[test]
    fn admits_only_exact_nonretained_live_events() {
        let manifest = manifest();
        validate_manifest(&manifest).unwrap();
        let result = result();
        validate_result(&result).unwrap();
        assert_eq!(parse_operator_event("ordinary output").unwrap(), None);
        let line = format!("{EVENT_PREFIX}{manifest}");
        assert_eq!(parse_operator_event(&line).unwrap(), Some(manifest));
    }

    #[test]
    fn refuses_unsafe_numbers_substitution_and_incomplete_rollback() {
        let mut unsafe_manifest = manifest();
        unsafe_manifest["boundary_count"] = json!(2);
        assert!(validate_manifest(&unsafe_manifest).is_err());

        let mut bad_result = result();
        bad_result["out_of_order_boundary_rollback"]["snapshots_equal"] = json!(false);
        assert!(validate_result(&bad_result).is_err());

        let mut short = result();
        short["archive_records"].as_array_mut().unwrap().pop();
        assert!(validate_result(&short).is_err());
    }

    #[test]
    fn live_run_has_no_transcript_or_authoring_surface() {
        let options = Options {
            port: 9130,
            rpc_port: 9137,
            faucet_port: 9139,
            gossip_port: 9200,
            dynamic_port_range: "9201-9250".to_string(),
            work: None,
            statics: PathBuf::from("unused"),
            exit_when_done: true,
        };
        let event = run_event(&options, "starting");
        assert_eq!(event["retained_transcript"], false);
        assert_eq!(
            event["authority"],
            "read-only live child telemetry; no retained transcript; no browser key material"
        );
        assert_eq!(event["rpc_url"], "http://127.0.0.1:9137");
    }

    #[test]
    fn browser_progress_refuses_paths_stderr_and_key_shaped_arbitrary_output() {
        for admitted in [
            "pinned loopback validator runtime: PASS",
            "campaign_mode=joined-multiboundary-v1",
            "campaign_clock_settled=1787540520",
            "prepared 29 exact genesis accounts",
            "local_session_owner=ready",
            "local_session_owner=stopping",
            "waiting reason=best-valid-submitted-candidate selection slot=40 target=50",
            "step=router-initialize slot=10 cu=34183 error=null",
            "step=wrong-feed-post-update-plus-append-rollback slot=11 cu=81604 error={\"InstructionError\":[2,{\"Custom\":122}]}",
        ] {
            assert!(admitted_progress_line("stdout", admitted), "{admitted}");
        }
        for refused in [
            "/private/tmp/campaign/payer.json",
            "binary: /private/cache/solana-test-validator",
            "ephemeral secret key bytes: [1,2,3]",
            "step=../../payer slot=10 cu=1 error=null",
            "step=ok slot=10 cu=1 error={\"InstructionError\":[2,{\"Custom\":121}]}",
            "{\"result\":\"retained campaign body\"}",
        ] {
            assert!(!admitted_progress_line("stdout", refused), "{refused}");
        }
        assert!(!admitted_progress_line(
            "stderr",
            "step=router-initialize slot=10 cu=34183 error=null"
        ));
    }
}
