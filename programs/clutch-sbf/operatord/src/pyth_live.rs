//! Live, read-only supervision of the local real-Pyth joined lifecycle.
//!
//! This mode does not load a retained transcript and it does not make the
//! browser a signer. It starts the repository's clean-HEAD, loopback-only
//! `joined-multiboundary-v1` producer as a child and admits only its opt-in,
//! versioned JSON events. The child still owns temporary keys, validator RPC,
//! transaction construction, exact provider binaries, and cleanup.

use crate::{http, integer, rpc, toolchain, Bus};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const CLAIM: &str = "NON-PRODUCTION / SYNTHETIC OBSERVATION / LOCAL VALIDATOR ONLY / NO VALUE";
const MODE: &str = "pyth-live";
const CAMPAIGN_MODE: &str = "joined-multiboundary-v1";
const TRANSCRIPT_SCHEMA: &str =
    "dragons-clutch/operator/local-real-pyth-multiboundary-joined-lifecycle/v1";
const MANIFEST_SCHEMA: &str = "dragons-clutch/operator/live-real-pyth-manifest/v1";
const RESULT_SCHEMA: &str = "dragons-clutch/operator/live-real-pyth-result/v1";
const RUN_SCHEMA: &str = "dragons-clutch/operator/live-real-pyth-run/v1";
const EVENT_PREFIX: &str = "CLUTCH_OPERATOR_EVENT ";
const PROFILE: &str = "NON-PRODUCTION-non-production-real-pyth-lab";
const MAX_OUTPUT_LINE_BYTES: usize = 16 * 1024;

pub struct Options {
    pub port: u16,
    pub rpc_port: u16,
    pub faucet_port: u16,
    pub gossip_port: u16,
    pub dynamic_port_range: String,
    pub statics: PathBuf,
    pub exit_when_done: bool,
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
            // The supervised runner owns a validator and ephemeral keys. Give
            // its EXIT/TERM trap time to stop the validator and remove its
            // private directory before falling back to an uncatchable kill.
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

fn no_json_numbers(value: &Value) -> bool {
    match value {
        Value::Number(_) => false,
        Value::Array(values) => values.iter().all(no_json_numbers),
        Value::Object(values) => values.values().all(no_json_numbers),
        _ => true,
    }
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

    let rollback = field_object(value, "out_of_order_boundary_rollback", role)?;
    let rollback_value = Value::Object(rollback.clone());
    if !boolean(&rollback_value, "ok", "rollback")?
        || !boolean(
            &rollback_value,
            "skipped_update_absent_after_refusal",
            "rollback",
        )?
        || !boolean(&rollback_value, "snapshots_equal", "rollback")?
    {
        return Err("live real-Pyth rollback closure differs".into());
    }
    let before = string(&rollback_value, "before_snapshot_sha256", "rollback")?;
    let after = string(&rollback_value, "after_snapshot_sha256", "rollback")?;
    lowercase_hex(&rollback_value, "before_snapshot_sha256", 32, "rollback")?;
    lowercase_hex(&rollback_value, "after_snapshot_sha256", 32, "rollback")?;
    if before != after {
        return Err("live real-Pyth rollback snapshots differ".into());
    }
    let refusal = rollback
        .get("instruction_error")
        .and_then(Value::as_object)
        .ok_or("live real-Pyth rollback has no instruction error")?;
    let refusal_value = Value::Object(refusal.clone());
    if canonical_unsigned(&refusal_value, "instruction_index", "rollback refusal")? != 2
        || canonical_unsigned(&refusal_value, "custom_code", "rollback refusal")? != 122
        || string(&refusal_value, "custom_code_hex", "rollback refusal")? != "0x7a"
    {
        return Err("live real-Pyth rollback refusal differs".into());
    }

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
                            bus.publish(&identity(&event, options, "IN_FLIGHT", None));
                            bus.publish(&run_event(options, "running"));
                        }
                        Some("live-real-pyth-result") => {
                            if result.replace(event.clone()).is_some() {
                                return Err("live child emitted a second result event".into());
                            }
                            bus.publish(&event);
                            bus.publish(&run_event(options, "validating-exit"));
                        }
                        _ => unreachable!("parse_operator_event admitted the event"),
                    }
                    return Ok(());
                }
            }
            *sequence = sequence
                .checked_add(1)
                .ok_or("live output sequence overflow")?;
            bus.publish(&json!({
                "type": "live-output",
                "schema": RUN_SCHEMA,
                "sequence": sequence.to_string(),
                "stream": stream,
                "text": line,
            }));
        }
    }
    Ok(())
}

fn supervise(options: &Options, bus: &Bus) -> Result<(Value, Value, ExitStatus)> {
    let script =
        crate::repo_path("programs/clutch-sbf/scripts/run_local_multiboundary_pyth_lifecycle.sh");
    let mut command = Command::new(&script);
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

    fn result() -> Value {
        let snapshot = "e".repeat(64);
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
            "out_of_order_boundary_rollback": {
                "ok":true,
                "skipped_boundary_index":"1",
                "skipped_update_account":"8opHzTAnfzRpPEx21XtnrVTX28YQuCpAjcn1PczScKh",
                "skipped_update_absent_after_refusal":true,
                "refusal_signature":"signature",
                "instruction_error":{"instruction_index":"2","custom_code":"122","custom_code_hex":"0x7a"},
                "snapshot_encoding":"encoding",
                "snapshot_domain":"domain",
                "watched_accounts":[],
                "before_snapshot_sha256":snapshot,
                "after_snapshot_sha256":snapshot,
                "snapshots_equal":true
            },
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
                "seller_token_atoms":"52"
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
}
