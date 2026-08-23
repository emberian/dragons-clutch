//! Read-only presentation of a retained local-real Pyth campaign.
//!
//! The campaign runner owns validation, signing, RPC submission, and capture.
//! This module reads only the three explicitly public transcript files that the
//! runner retains. It never reads the runner's temporary directory, keys, a
//! wallet, RPC, or a provider API, and it cannot build or submit a transaction.

use crate::bus::Bus;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const CLAIM: &str = "NON-PRODUCTION / SYNTHETIC OBSERVATION / LOCAL VALIDATOR ONLY / NO VALUE";
const SCHEMA: &str = "dragons-clutch/operator/local-real-pyth-transcript/v1";
const PROFILE: &str = "NON-PRODUCTION-non-production-real-pyth-lab";
const PROVIDER_ROLES: [(&str, bool); 4] = [
    ("receiver-program", true),
    ("receiver-programdata", false),
    ("router-program", true),
    ("router-programdata", false),
];
const STEP_LABELS: [&str; 13] = [
    "router-initialize",
    "router-init-and-write-encoded-vaa",
    "router-write-and-verify-encoded-vaa",
    "receiver-initialize",
    "correct-init-source-spec-v2",
    "correct-init-source-archive-v2",
    "wrong-feed-init-source-spec-v2",
    "wrong-feed-init-source-archive-v2",
    "wrong-config-post-update-plus-append-rollback",
    "wrong-feed-post-update-plus-append-rollback",
    "real-post-update-plus-clutch-append-atomic",
    "seal-source-archive-v2",
    "categorical-resolve-cell-1",
];

pub struct Options {
    pub port: u16,
    pub transcript: PathBuf,
    pub statics: PathBuf,
    pub exit_when_done: bool,
}

struct View {
    identity: Value,
    campaign: Value,
}

fn read_json(path: &Path) -> Result<Value> {
    let bytes = fs::read(path)?;
    let value: Value = serde_json::from_slice(&bytes)?;
    if !value.is_object() {
        return Err(format!("{} is not a JSON object", path.display()).into());
    }
    Ok(value)
}

fn object<'a>(value: &'a Value, field: &str) -> Result<&'a Map<String, Value>> {
    value
        .get(field)
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{field} is absent or is not an object").into())
}

fn array<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{field} is absent or is not an array").into())
}

fn string<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| format!("{field} is absent or is not a nonempty string").into())
}

fn boolean(value: &Value, field: &str) -> Result<bool> {
    value
        .get(field)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("{field} is absent or is not a boolean").into())
}

fn unsigned(value: &Value, field: &str) -> Result<u64> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{field} is absent or is not an unsigned integer").into())
}

fn signed(value: &Value, field: &str) -> Result<i64> {
    value
        .get(field)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("{field} is absent or is not an integer").into())
}

fn exact_claim(value: &Value, role: &str) -> Result<()> {
    if string(value, "claim")? != CLAIM {
        return Err(format!("{role} does not carry the exact campaign truth label").into());
    }
    Ok(())
}

fn lowercase_hex(text: &str, bytes: usize, role: &str) -> Result<String> {
    if text.len() != bytes * 2
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(format!("{role} is not lowercase {}-byte hex", bytes).into());
    }
    Ok(text.to_string())
}

fn decimal(value: u64) -> String {
    value.to_string()
}

fn signed_decimal(value: i64) -> String {
    value.to_string()
}

fn canonical_unsigned_decimal<'a>(value: &'a Map<String, Value>, field: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|text| {
            !text.is_empty()
                && text.bytes().all(|byte| byte.is_ascii_digit())
                && (*text == "0" || !text.starts_with('0'))
        })
        .ok_or_else(|| format!("{field} is not a canonical unsigned decimal string").into())
}

fn loopback_endpoint(value: &Value, field: &str) -> Result<String> {
    let text = string(value, field)?;
    let address: SocketAddr = text
        .parse()
        .map_err(|_| format!("{field} is not a socket address"))?;
    if !address.ip().is_loopback() {
        return Err(format!("{field} is not loopback-bound").into());
    }
    Ok(text.to_string())
}

fn feed_id(result: &Value) -> Result<String> {
    let bytes = array(result, "provider_feed_id")?;
    if bytes.len() != 32 {
        return Err("provider_feed_id must contain exactly 32 bytes".into());
    }
    let mut out = String::with_capacity(64);
    for byte in bytes {
        let value = byte
            .as_u64()
            .filter(|value| *value <= u64::from(u8::MAX))
            .ok_or("provider_feed_id contains a non-byte value")?;
        out.push_str(&format!("{value:02x}"));
    }
    Ok(out)
}

fn providers(manifest: &Value) -> Result<Vec<Value>> {
    let rows = array(manifest, "provider")?;
    if rows.len() != PROVIDER_ROLES.len() {
        return Err(format!(
            "expected {} provider identity rows, saw {}",
            PROVIDER_ROLES.len(),
            rows.len()
        )
        .into());
    }
    let mut addresses = BTreeSet::new();
    let mut out = Vec::new();
    for (row, (expected_role, expected_executable)) in rows.iter().zip(PROVIDER_ROLES) {
        let role = string(row, "role")?;
        if role != expected_role {
            return Err(
                format!("provider role {role:?} is not expected role {expected_role:?}").into(),
            );
        }
        let address = string(row, "address")?;
        if !addresses.insert(address.to_string()) {
            return Err(format!("duplicate provider address {address}").into());
        }
        let executable = boolean(row, "executable")?;
        if executable != expected_executable {
            return Err(format!("provider {role} executable bit differs").into());
        }
        out.push(json!({
            "role": role,
            "address": address,
            "complete_account_body_sha256": lowercase_hex(
                string(row, "complete_account_body_sha256")?, 32, "provider body hash"
            )?,
            "executable": executable,
        }));
    }
    Ok(out)
}

fn source_admission_refusal(error: &Value) -> bool {
    error
        .get("InstructionError")
        .and_then(Value::as_array)
        .is_some_and(|parts| {
            parts.first().and_then(Value::as_u64) == Some(2)
                && parts
                    .get(1)
                    .and_then(|detail| detail.get("Custom"))
                    .and_then(Value::as_u64)
                    == Some(0x007a)
        })
}

fn steps(result: &Value) -> Result<Vec<Value>> {
    let rows = array(result, "steps")?;
    if rows.len() != STEP_LABELS.len() {
        return Err(format!(
            "expected {} campaign steps, saw {}",
            STEP_LABELS.len(),
            rows.len()
        )
        .into());
    }
    let mut signatures = BTreeSet::new();
    let mut out = Vec::new();
    for (index, (row, expected_label)) in rows.iter().zip(STEP_LABELS).enumerate() {
        let label = string(row, "label")?;
        if label != expected_label {
            return Err(format!(
                "campaign step {} is {label:?}, expected {expected_label:?}",
                index + 1
            )
            .into());
        }
        let error = row.get("error").ok_or("campaign step has no error field")?;
        let refused = !error.is_null();
        let expected_refusal = matches!(index, 8 | 9);
        if refused != expected_refusal {
            return Err(format!(
                "campaign step {label} has an unexpected acceptance/refusal state"
            )
            .into());
        }
        if expected_refusal && !source_admission_refusal(error) {
            return Err(format!(
                "campaign step {label} is not the exact instruction-2 SourceAdmissionFailed refusal"
            )
            .into());
        }
        let order = array(row, "program_order")?;
        if order.is_empty() || !order.iter().all(Value::is_string) {
            return Err(format!("campaign step {label} has malformed program_order").into());
        }
        let signature = string(row, "signature")?;
        if !signatures.insert(signature.to_string()) {
            return Err(format!("campaign step {label} repeats signature {signature}").into());
        }
        out.push(json!({
            "ordinal": decimal(u64::try_from(index + 1)?),
            "label": label,
            "state": if refused { "refused-as-expected" } else { "accepted" },
            "signature": signature,
            "slot": decimal(unsigned(row, "slot")?),
            "compute_units_consumed": decimal(unsigned(row, "compute_units_consumed")?),
            "fee_lamports": decimal(unsigned(row, "fee_lamports")?),
            "signed_wire_sha256": lowercase_hex(
                string(row, "signed_wire_sha256")?, 32, "signed wire hash"
            )?,
            "program_order": order,
            "error": error,
        }));
    }
    Ok(out)
}

fn signature_for<'a>(steps: &'a [Value], label: &str) -> Result<&'a str> {
    steps
        .iter()
        .find(|step| step.get("label").and_then(Value::as_str) == Some(label))
        .and_then(|step| step.get("signature"))
        .and_then(Value::as_str)
        .ok_or_else(|| format!("no signature retained for {label}").into())
}

fn build_view(manifest: &Value, result: &Value, probe: &Value) -> Result<View> {
    exact_claim(manifest, "campaign.json")?;
    exact_claim(result, "result.json")?;
    exact_claim(probe, "probe-evidence.json")?;
    if string(manifest, "network")? != "127.0.0.1 loopback only"
        || string(manifest, "value")? != "none"
        || string(result, "network")? != "loopback validator only"
    {
        return Err("campaign value/network boundary differs".into());
    }
    if !string(manifest, "observation")?.contains("synthetic") {
        return Err("campaign observation is not explicitly synthetic".into());
    }
    if !boolean(result, "wrong_config_rollback")?
        || !boolean(result, "wrong_feed_rollback")?
        || !boolean(result, "sealed")?
        || unsigned(result, "resolved_payout")? != 1
    {
        return Err("campaign terminal and rollback assertions are not all closed".into());
    }
    if string(probe, "selected_validator_sha256")? != string(manifest, "validator_binary_sha256")? {
        return Err("listener probe and campaign validator identities differ".into());
    }

    let steps = steps(result)?;
    for (field, label) in [
        (
            "joined_post_append_signature",
            "real-post-update-plus-clutch-append-atomic",
        ),
        ("seal_signature", "seal-source-archive-v2"),
        ("resolve_signature", "categorical-resolve-cell-1"),
    ] {
        if string(result, field)? != signature_for(&steps, label)? {
            return Err(format!("{field} does not match the ordered step transcript").into());
        }
    }

    let interval = object(result, "interval")?;
    let lower = canonical_unsigned_decimal(interval, "lower")?;
    let upper = canonical_unsigned_decimal(interval, "upper")?;
    if (lower.len(), lower) > (upper.len(), upper) {
        return Err("source interval lower exceeds upper".into());
    }

    let source_profile = object(manifest, "source_profile_snapshot")?;
    let validator_provenance = object(manifest, "validator_build_provenance")?;
    let identity = json!({
        "type": "identity",
        "mode": "pyth-local",
        "source_profile": PROFILE,
        "elf_sha256": lowercase_hex(string(manifest, "clutch_elf_sha256")?, 32, "Clutch ELF hash")?,
        "program_id": string(manifest, "program_id")?,
        "repository_head": lowercase_hex(
            string(manifest, "dragons_clutch_repository_head")?, 20, "repository HEAD"
        )?,
        "evidence_scope": "SBF_EXECUTED",
        "promotion": "unpromoted",
        "network": "LOCAL VALIDATOR ONLY",
        "value": "no value",
        "observation": "SYNTHETIC OBSERVATION",
        "retained_transcript": true,
    });
    let campaign = json!({
        "type": "pyth-campaign",
        "schema": SCHEMA,
        "claim": CLAIM,
        "retained_transcript": true,
        "identity": {
            "upstream_pyth_crosschain_commit": lowercase_hex(
                string(manifest, "upstream_pyth_crosschain_commit")?, 20, "upstream commit"
            )?,
            "repository_head": string(manifest, "dragons_clutch_repository_head")?,
            "clutch_elf_sha256": string(manifest, "clutch_elf_sha256")?,
            "validator_binary_sha256": lowercase_hex(
                string(manifest, "validator_binary_sha256")?, 32, "validator hash"
            )?,
            "validator_build_record_sha256": lowercase_hex(
                validator_provenance.get("selected_build_record_sha256")
                    .and_then(Value::as_str).ok_or("selected validator build record hash is absent")?,
                32, "validator build record hash"
            )?,
            "source_profile_snapshot_sha256": lowercase_hex(
                source_profile.get("sha256").and_then(Value::as_str)
                    .ok_or("source profile snapshot hash is absent")?,
                32, "source profile snapshot hash"
            )?,
            "vaa_sha256": lowercase_hex(string(manifest, "vaa_sha256")?, 32, "VAA hash")?,
            "post_update_data_sha256": lowercase_hex(
                string(manifest, "post_update_data_sha256")?, 32, "PostUpdate data hash"
            )?,
            "genesis_hash": string(result, "genesis_hash")?,
            "warp_slot": decimal(unsigned(manifest, "warp_slot")?),
        },
        "provider": providers(manifest)?,
        "listener_evidence": {
            "rpc": loopback_endpoint(probe, "rpc")?,
            "websocket": loopback_endpoint(probe, "websocket")?,
            "faucet": loopback_endpoint(probe, "faucet")?,
            "gossip": loopback_endpoint(probe, "gossip")?,
            "configured_dynamic_port_range": string(probe, "configured_dynamic_port_range")?,
            "scope": string(probe, "scope")?,
            "probe_before_sha256": lowercase_hex(
                string(probe, "probe_before_sha256")?, 32, "pre-campaign listener probe hash"
            )?,
            "probe_after_sha256": lowercase_hex(
                string(probe, "probe_after_sha256")?, 32, "post-campaign listener probe hash"
            )?,
        },
        "source": {
            "provider_feed_id_hex": feed_id(result)?,
            "price": signed_decimal(signed(result, "price")?),
            "confidence": decimal(unsigned(result, "confidence")?),
            "exponent": signed_decimal(signed(result, "exponent")?),
            "publish_time": signed_decimal(signed(result, "publish_time")?),
            "interval_lower": lower,
            "interval_upper": upper,
            "verified_vaa_account": string(result, "verified_vaa_account")?,
            "update_account": string(result, "update_account")?,
        },
        "rollbacks": [
            {
                "label": "wrong Config",
                "ok": true,
                "scope": "receiver-created update absent; source archive and treasury byte-identical",
            },
            {
                "label": "wrong feed",
                "ok": true,
                "scope": "receiver-created update absent; wrong-feed archive and treasury byte-identical",
            },
        ],
        "outcome": {
            "sealed": boolean(result, "sealed")?,
            "resolved_payout": decimal(unsigned(result, "resolved_payout")?),
            "joined_post_append_signature": string(result, "joined_post_append_signature")?,
            "seal_signature": string(result, "seal_signature")?,
            "resolve_signature": string(result, "resolve_signature")?,
        },
        "steps": steps,
    });
    Ok(View { identity, campaign })
}

fn load(transcript: &Path) -> Result<View> {
    build_view(
        &read_json(&transcript.join("campaign.json"))?,
        &read_json(&transcript.join("result.json"))?,
        &read_json(&transcript.join("probe-evidence.json"))?,
    )
}

pub fn serve(options: Options) -> Result<()> {
    let view = load(&options.transcript)?;
    let bus = Bus::new();
    let action: crate::http::Action = Arc::new(|request: &Value| {
        if request.get("action").and_then(Value::as_str) == Some("ping") {
            json!({"ok": true, "mode": "pyth-local", "authority": "read-only transcript"})
        } else {
            json!({"ok": false, "detail": "the retained Pyth campaign surface is read-only"})
        }
    });
    let server =
        crate::http::Server::bind(options.port, Arc::clone(&bus), options.statics, action)?;
    let port = server.port()?;
    thread::spawn(move || server.serve_forever());
    bus.publish(&view.identity);
    bus.publish(&view.campaign);
    bus.publish(&json!({
        "type": "done",
        "verdict": "PASS",
        "scope": "SBF_EXECUTED",
        "promotion": "unpromoted",
        "mode": "pyth-local",
    }));
    println!("Operator Bench (retained local-real Pyth): http://127.0.0.1:{port}/");
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
    use super::{build_view, CLAIM, STEP_LABELS};
    use serde_json::{json, Value};

    fn hash(byte: char) -> String {
        std::iter::repeat(byte).take(64).collect()
    }

    fn fixtures() -> (Value, Value, Value) {
        let providers = [
            ("receiver-program", true),
            ("receiver-programdata", false),
            ("router-program", true),
            ("router-programdata", false),
        ]
        .into_iter()
        .map(|(role, executable)| {
            json!({
                "role": role, "address": format!("address-{role}"),
                "complete_account_body_sha256": hash('a'), "executable": executable,
            })
        })
        .collect::<Vec<_>>();
        let steps = STEP_LABELS
            .iter()
            .enumerate()
            .map(|(index, label)| {
                json!({
                    "label": label,
                    "signature": format!("signature-{index}"),
                    "slot": 460_336_312_u64 + u64::try_from(index).unwrap(),
                    "compute_units_consumed": 100_u64 + u64::try_from(index).unwrap(),
                    "fee_lamports": 5000,
                    "signed_wire_sha256": hash('b'),
                    "program_order": ["ComputeBudget111", "Program111"],
                    "error": if matches!(index, 8 | 9) {
                        json!({"InstructionError": [2, {"Custom": 122}]})
                    } else {
                        Value::Null
                    },
                })
            })
            .collect::<Vec<_>>();
        let manifest = json!({
            "claim": CLAIM,
            "network": "127.0.0.1 loopback only",
            "observation": "synthetic deterministic local guardian quorum; not devnet price evidence",
            "value": "none",
            "upstream_pyth_crosschain_commit": std::iter::repeat('c').take(40).collect::<String>(),
            "dragons_clutch_repository_head": std::iter::repeat('d').take(40).collect::<String>(),
            "source_profile_snapshot": {"sha256": hash('e')},
            "validator_build_provenance": {"selected_build_record_sha256": hash('f')},
            "warp_slot": 460_336_312,
            "program_id": "Clutch111",
            "clutch_elf_sha256": hash('1'),
            "validator_binary_sha256": hash('2'),
            "vaa_sha256": hash('3'),
            "post_update_data_sha256": hash('4'),
            "provider": providers,
        });
        let result = json!({
            "claim": CLAIM,
            "network": "loopback validator only",
            "genesis_hash": "genesis111",
            "publish_time": 1_787_431_680_i64,
            "provider_feed_id": vec![42; 32],
            "price": 100_000_000_i64,
            "confidence": 6_357,
            "exponent": -8,
            "interval": {"lower": "99980929", "upper": "100019071"},
            "verified_vaa_account": "vaa111",
            "update_account": "update111",
            "joined_post_append_signature": "signature-10",
            "seal_signature": "signature-11",
            "resolve_signature": "signature-12",
            "wrong_config_rollback": true,
            "wrong_feed_rollback": true,
            "sealed": true,
            "resolved_payout": 1,
            "steps": steps,
        });
        let probe = json!({
            "claim": CLAIM,
            "rpc": "127.0.0.1:1", "websocket": "127.0.0.1:2",
            "faucet": "127.0.0.1:3", "gossip": "127.0.0.1:4",
            "configured_dynamic_port_range": "5-9",
            "scope": "all observed sockets were loopback-bound",
            "selected_validator_sha256": hash('2'),
            "probe_before_sha256": hash('5'), "probe_after_sha256": hash('6'),
        });
        (manifest, result, probe)
    }

    #[test]
    fn public_transcripts_become_exact_decimal_display_events() {
        let (manifest, result, probe) = fixtures();
        let view = build_view(&manifest, &result, &probe).unwrap();
        assert_eq!(view.identity["mode"], "pyth-local");
        assert_eq!(view.campaign["schema"], super::SCHEMA);
        assert_eq!(view.campaign["source"]["interval_lower"], "99980929");
        assert_eq!(view.campaign["steps"][0]["slot"], "460336312");
        assert_eq!(view.campaign["steps"][8]["state"], "refused-as-expected");
    }

    #[test]
    fn missing_rollback_closure_is_refused() {
        let (manifest, mut result, probe) = fixtures();
        result["wrong_feed_rollback"] = json!(false);
        assert!(build_view(&manifest, &result, &probe).is_err());
    }

    #[test]
    fn reordered_signed_steps_are_refused() {
        let (manifest, mut result, probe) = fixtures();
        result["steps"].as_array_mut().unwrap().swap(0, 1);
        assert!(build_view(&manifest, &result, &probe).is_err());
    }

    #[test]
    fn wrong_refusal_code_is_refused() {
        let (manifest, mut result, probe) = fixtures();
        result["steps"][8]["error"]["InstructionError"][1]["Custom"] = json!(123);
        assert!(build_view(&manifest, &result, &probe).is_err());
    }

    #[test]
    fn off_loopback_listener_evidence_is_refused() {
        let (manifest, result, mut probe) = fixtures();
        probe["rpc"] = json!("192.0.2.1:18537");
        assert!(build_view(&manifest, &result, &probe).is_err());
    }

    #[test]
    fn noncanonical_interval_decimal_is_refused() {
        let (manifest, mut result, probe) = fixtures();
        result["interval"]["lower"] = json!("099980929");
        assert!(build_view(&manifest, &result, &probe).is_err());
    }
}
