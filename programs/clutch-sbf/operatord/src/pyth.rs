//! Read-only presentation of a retained local-real Pyth campaign.
//!
//! The campaign runner owns validation, signing, RPC submission, and capture.
//! This module reads only the three explicitly public transcript files that the
//! runner retains. It never reads the runner's temporary directory, keys, a
//! wallet, RPC, or a provider API, and it cannot build or submit a transaction.

use crate::bus::Bus;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::fmt::Write;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const CLAIM: &str = "NON-PRODUCTION / SYNTHETIC OBSERVATION / LOCAL VALIDATOR ONLY / NO VALUE";
const SOURCE_SCHEMA: &str = "dragons-clutch/operator/local-real-pyth-transcript/v1";
const JOINED_V2_SCHEMA: &str = "dragons-clutch/operator/local-real-pyth-joined-lifecycle/v2";
const JOINED_V3_SCHEMA: &str = "dragons-clutch/operator/local-real-pyth-joined-lifecycle/v3";
const SOURCE_ONLY_MODE: &str = "source-only-v1";
const JOINED_LIFECYCLE_MODE: &str = "joined-user-lifecycle-v1";
const PROFILE: &str = "NON-PRODUCTION-non-production-real-pyth-lab";
const PROVIDER_ROLES: [(&str, bool); 4] = [
    ("receiver-program", true),
    ("receiver-programdata", false),
    ("router-program", true),
    ("router-programdata", false),
];
const SOURCE_STEP_LABELS: [&str; 13] = [
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
const JOINED_V2_STEP_LABELS: [&str; 21] = [
    "router-initialize",
    "router-init-and-write-encoded-vaa",
    "router-write-and-verify-encoded-vaa",
    "receiver-initialize",
    "correct-init-source-spec-v2",
    "correct-init-source-archive-v2",
    "wrong-feed-init-source-spec-v2",
    "wrong-feed-init-source-archive-v2",
    "joined-create-market",
    "joined-endow-collateral",
    "joined-split-complete-sets",
    "wrong-config-post-update-plus-append-rollback",
    "wrong-feed-post-update-plus-append-rollback",
    "real-post-update-plus-clutch-append-atomic",
    "seal-source-archive-v2",
    "categorical-resolve-cell-1",
    "joined-redeem-internal-outcome-0",
    "joined-redeem-internal-outcome-1",
    "joined-redeem-internal-outcome-2",
    "joined-redeem-internal-outcome-3",
    "joined-withdraw-redeemed-collateral",
];
const JOINED_V3_STEP_LABELS: [&str; 52] = [
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
    "joined-fund-second-owner-account-creation",
    "joined-price-grid-artifact-begin",
    "joined-price-grid-artifact-write-0",
    "joined-price-grid-artifact-write-1",
    "joined-price-grid-artifact-write-2",
    "joined-price-grid-artifact-write-3",
    "joined-price-grid-artifact-seal",
    "joined-create-market",
    "joined-endow-buyer-collateral",
    "joined-endow-seller-collateral",
    "joined-seller-split-complete-sets",
    "joined-general-policy-artifact-begin",
    "joined-general-policy-artifact-write-0",
    "joined-general-policy-artifact-seal",
    "joined-general-init-epoch",
    "joined-general-init-order-page",
    "joined-general-place-funded-buy",
    "joined-general-place-funded-sell",
    "joined-general-freeze-epoch",
    "joined-general-submit-candidate",
    "joined-general-write-candidate-fills",
    "joined-general-write-candidate-slices",
    "joined-general-seal-candidate",
    "joined-general-create-clear-work",
    "joined-general-verify-pass-one",
    "joined-general-verify-slices",
    "joined-general-verify-pass-two",
    "joined-general-complete-clear-work",
    "joined-general-finalize-selection",
    "joined-general-freeze-entitlement",
    "joined-general-entitle-direct-slice",
    "joined-general-settle-direct-slice",
    "categorical-resolve-cell-1",
    "joined-buyer-redeem-winning-eggs",
    "joined-seller-redeem-outcome-0",
    "joined-seller-redeem-outcome-1",
    "joined-seller-redeem-outcome-2",
    "joined-seller-redeem-outcome-3",
    "joined-buyer-withdraw-redeemed-collateral",
    "joined-seller-withdraw-redeemed-collateral",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JoinedSchema {
    V2,
    V3,
}

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
        return Err(format!("{role} is not lowercase {bytes}-byte hex").into());
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

fn exact_keys(value: &Map<String, Value>, expected: &[&str], role: &str) -> Result<()> {
    let actual = value.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(format!("{role} has unknown or missing fields").into());
    }
    Ok(())
}

fn fixed_hash(value: &Value, role: &str) -> Result<String> {
    let bytes = value
        .as_array()
        .ok_or_else(|| format!("{role} is not a byte array"))?;
    if bytes.len() != 32 {
        return Err(format!("{role} must contain exactly 32 bytes").into());
    }
    let mut out = String::with_capacity(64);
    for (index, byte) in bytes.iter().enumerate() {
        let byte = byte
            .as_u64()
            .ok_or_else(|| format!("{role}[{index}] is not a byte"))?;
        let byte = u8::try_from(byte).map_err(|_| format!("{role}[{index}] is not a byte"))?;
        write!(&mut out, "{byte:02x}")?;
    }
    Ok(out)
}

fn exact_unsigned_vector(value: &Value, expected: &[u64], role: &str) -> Result<Vec<Value>> {
    let values = value
        .as_array()
        .ok_or_else(|| format!("{role} is not an array"))?;
    if values.len() != expected.len() {
        return Err(format!("{role} must contain exactly {} values", expected.len()).into());
    }
    values
        .iter()
        .zip(expected)
        .enumerate()
        .map(|(index, (value, expected))| {
            let actual = value
                .as_u64()
                .ok_or_else(|| format!("{role}[{index}] is not an unsigned integer"))?;
            if actual != *expected {
                return Err(format!("{role}[{index}] is {actual}, expected {expected}").into());
            }
            Ok(json!(decimal(actual)))
        })
        .collect()
}

fn exact_strings(value: &Value, expected: &[&str], role: &str) -> Result<Vec<Value>> {
    let values = value
        .as_array()
        .ok_or_else(|| format!("{role} is not an array"))?;
    if values.len() != expected.len() {
        return Err(format!("{role} must contain exactly {} values", expected.len()).into());
    }
    values
        .iter()
        .zip(expected)
        .enumerate()
        .map(|(index, (value, expected))| {
            let actual = value
                .as_str()
                .filter(|text| !text.is_empty())
                .ok_or_else(|| format!("{role}[{index}] is not a nonempty string"))?;
            if actual != *expected {
                return Err(format!("{role}[{index}] differs from its bound identity").into());
            }
            Ok(json!(actual))
        })
        .collect()
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
            .ok_or("provider_feed_id contains a non-byte value")?;
        let value =
            u8::try_from(value).map_err(|_| "provider_feed_id contains a non-byte value")?;
        write!(&mut out, "{value:02x}")?;
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

fn campaign_mode(manifest: &Value, result: &Value) -> Result<&'static str> {
    let manifest_mode = manifest.get("campaign_mode");
    let result_mode = result.get("campaign_mode");
    match (manifest_mode, result_mode) {
        (None, None) => Ok(SOURCE_ONLY_MODE),
        (Some(left), Some(right)) if left == right => match left.as_str() {
            Some(SOURCE_ONLY_MODE) => Ok(SOURCE_ONLY_MODE),
            Some(JOINED_LIFECYCLE_MODE) => Ok(JOINED_LIFECYCLE_MODE),
            _ => Err("campaign_mode is not recognized".into()),
        },
        _ => Err("manifest/result campaign_mode differs or is absent on one side".into()),
    }
}

fn steps(result: &Value, labels: &[&str]) -> Result<Vec<Value>> {
    let rows = array(result, "steps")?;
    if rows.len() != labels.len() {
        return Err(format!(
            "expected {} campaign steps, saw {}",
            labels.len(),
            rows.len()
        )
        .into());
    }
    let mut signatures = BTreeSet::new();
    let mut out = Vec::new();
    for (index, (row, expected_label)) in rows.iter().zip(labels).enumerate() {
        let label = string(row, "label")?;
        if label != *expected_label {
            return Err(format!(
                "campaign step {} is {label:?}, expected {expected_label:?}",
                index + 1
            )
            .into());
        }
        let error = row.get("error").ok_or("campaign step has no error field")?;
        let refused = !error.is_null();
        let expected_refusal = matches!(
            label,
            "wrong-config-post-update-plus-append-rollback"
                | "wrong-feed-post-update-plus-append-rollback"
        );
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

fn signature_sequence(
    value: &Value,
    field: &str,
    steps: &[Value],
    labels: &[&str],
) -> Result<Vec<Value>> {
    let signatures = array(value, field)?;
    if signatures.len() != labels.len() {
        return Err(format!("{field} must contain exactly {} signed steps", labels.len()).into());
    }
    signatures
        .iter()
        .zip(labels)
        .enumerate()
        .map(|(index, (signature, label))| {
            let signature = signature
                .as_str()
                .filter(|text| !text.is_empty())
                .ok_or_else(|| format!("{field}[{index}] is not a nonempty signature"))?;
            if signature != signature_for(steps, label)? {
                return Err(format!("{field}[{index}] differs from signed step {label}").into());
            }
            Ok(json!(signature))
        })
        .collect()
}

fn all_exact_decimal(values: &[Value], expected: &str, role: &str) -> Result<Vec<Value>> {
    if values.len() != 4 {
        return Err(format!("{role} must contain exactly four outcomes").into());
    }
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let text = value
                .as_str()
                .ok_or_else(|| format!("{role}[{index}] is not a decimal string"))?;
            if text != expected {
                return Err(format!("{role}[{index}] is {text}, expected {expected}").into());
            }
            Ok(json!(text))
        })
        .collect()
}

#[allow(clippy::too_many_lines)]
fn joined_lifecycle_v2(manifest: &Value, result: &Value, steps: &[Value]) -> Result<Value> {
    let correct = object(manifest, "correct")?;
    if correct
        .get("market_genesis_assisted")
        .and_then(Value::as_bool)
        .ok_or("correct.market_genesis_assisted is absent or not boolean")?
    {
        return Err("joined market is marked genesis-assisted".into());
    }
    let lifecycle_value = result
        .get("lifecycle")
        .ok_or("result.lifecycle is absent")?;
    let lifecycle = lifecycle_value
        .as_object()
        .ok_or("result.lifecycle is not an object")?;
    if lifecycle
        .get("market_genesis_assisted")
        .and_then(Value::as_bool)
        .ok_or("lifecycle.market_genesis_assisted is absent or not boolean")?
    {
        return Err("joined lifecycle is marked genesis-assisted".into());
    }
    let market = string(lifecycle_value, "market")?;
    if correct.get("market").and_then(Value::as_str) != Some(market) {
        return Err("joined lifecycle market differs from the prepared manifest".into());
    }
    let user_token = string(lifecycle_value, "user_collateral_token")?;
    if correct.get("user_collateral_token").and_then(Value::as_str) != Some(user_token) {
        return Err("joined lifecycle user token differs from the prepared manifest".into());
    }
    if canonical_unsigned_decimal(lifecycle, "collateral_atoms")? != "64" {
        return Err("joined collateral quantity is not the exact 64 atoms".into());
    }

    for (field, label) in [
        ("create_market_signature", "joined-create-market"),
        ("endow_signature", "joined-endow-collateral"),
        ("split_signature", "joined-split-complete-sets"),
        ("withdraw_signature", "joined-withdraw-redeemed-collateral"),
    ] {
        if string(lifecycle_value, field)? != signature_for(steps, label)? {
            return Err(format!("lifecycle {field} differs from the signed step").into());
        }
    }

    let redeem = array(lifecycle_value, "redeem_internal")?;
    if redeem.len() != 4 {
        return Err("joined lifecycle must retain four RedeemInternal rows".into());
    }
    let mut projected_redeem = Vec::with_capacity(4);
    for (outcome, row) in redeem.iter().enumerate() {
        if unsigned(row, "outcome")? != u64::try_from(outcome)? {
            return Err(format!("RedeemInternal row {outcome} has the wrong outcome").into());
        }
        let quantity = canonical_unsigned_decimal(
            row.as_object()
                .ok_or("RedeemInternal row is not an object")?,
            "quantity",
        )?;
        let payout = canonical_unsigned_decimal(
            row.as_object()
                .ok_or("RedeemInternal row is not an object")?,
            "payout_atoms",
        )?;
        let expected_payout = if outcome == 1 { "64" } else { "0" };
        if quantity != "64" || payout != expected_payout {
            return Err(format!(
                "RedeemInternal row {outcome} has the wrong exact quantity/payout"
            )
            .into());
        }
        let label = format!("joined-redeem-internal-outcome-{outcome}");
        let signature = string(row, "signature")?;
        if signature != signature_for(steps, &label)? {
            return Err(
                format!("RedeemInternal row {outcome} differs from the signed step").into(),
            );
        }
        projected_redeem.push(json!({
            "outcome": decimal(u64::try_from(outcome)?),
            "quantity": quantity,
            "payout_atoms": payout,
            "signature": signature,
        }));
    }

    let terminal = lifecycle
        .get("terminal")
        .and_then(Value::as_object)
        .ok_or("lifecycle.terminal is absent or is not an object")?;
    for field in [
        "position_cash_atoms",
        "hoard_collateral_atoms",
        "hoard_token_atoms",
    ] {
        if canonical_unsigned_decimal(terminal, field)? != "0" {
            return Err(format!("terminal {field} is not zero").into());
        }
    }
    if canonical_unsigned_decimal(terminal, "user_token_atoms")? != "64" {
        return Err("terminal user token balance is not 64".into());
    }
    let position_internal = all_exact_decimal(
        terminal
            .get("position_internal")
            .and_then(Value::as_array)
            .ok_or("terminal.position_internal is absent or is not an array")?,
        "0",
        "terminal.position_internal",
    )?;
    let supply_internal = all_exact_decimal(
        terminal
            .get("supply_internal")
            .and_then(Value::as_array)
            .ok_or("terminal.supply_internal is absent or is not an array")?,
        "0",
        "terminal.supply_internal",
    )?;

    let trade = lifecycle
        .get("trade")
        .and_then(Value::as_object)
        .ok_or("lifecycle.trade is absent or is not an object")?;
    if trade.get("status").and_then(Value::as_str) != Some("blocked")
        || trade.get("reason_code").and_then(Value::as_str)
            != Some("missing-sealed-price-grid-and-epoch-plane")
    {
        return Err("joined trading blocker is absent or differs".into());
    }
    let trade_detail = trade
        .get("detail")
        .and_then(Value::as_str)
        .filter(|detail| !detail.is_empty())
        .ok_or("joined trading blocker detail is absent")?;

    Ok(json!({
        "market_genesis_assisted": false,
        "market": market,
        "ephemeral_user": string(lifecycle_value, "ephemeral_user")?,
        "user_collateral_token": user_token,
        "collateral_atoms": "64",
        "create_market_signature": string(lifecycle_value, "create_market_signature")?,
        "endow_signature": string(lifecycle_value, "endow_signature")?,
        "split_signature": string(lifecycle_value, "split_signature")?,
        "redeem_internal": projected_redeem,
        "withdraw_signature": string(lifecycle_value, "withdraw_signature")?,
        "terminal": {
            "position_cash_atoms": "0",
            "position_internal": position_internal,
            "supply_internal": supply_internal,
            "hoard_collateral_atoms": "0",
            "hoard_token_atoms": "0",
            "user_token_atoms": "64",
        },
        "trade": {
            "status": "blocked",
            "reason_code": "missing-sealed-price-grid-and-epoch-plane",
            "detail": trade_detail,
        },
    }))
}

#[allow(clippy::too_many_lines)]
fn joined_lifecycle_v3(manifest: &Value, result: &Value, steps: &[Value]) -> Result<Value> {
    let correct = object(manifest, "correct")?;
    if boolean(&Value::Object(correct.clone()), "market_genesis_assisted")? {
        return Err("joined-v3 market is marked genesis-assisted".into());
    }
    let payer = string(manifest, "payer")?;
    let second_owner = correct
        .get("second_owner")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .ok_or("correct.second_owner is absent")?;
    let buyer_token = correct
        .get("user_collateral_token")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .ok_or("correct.user_collateral_token is absent")?;
    let seller_token = correct
        .get("second_owner_collateral_token")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .ok_or("correct.second_owner_collateral_token is absent")?;

    let lifecycle_value = result
        .get("lifecycle")
        .ok_or("result.lifecycle is absent")?;
    let lifecycle = lifecycle_value
        .as_object()
        .ok_or("result.lifecycle is not an object")?;
    exact_keys(
        lifecycle,
        &[
            "market_genesis_assisted",
            "market",
            "ephemeral_users",
            "user_collateral_tokens",
            "collateral_atoms",
            "create_market_signature",
            "buyer_endow_signature",
            "seller_endow_signature",
            "split_signature",
            "redeem_internal",
            "buyer_withdraw_signature",
            "seller_withdraw_signature",
            "terminal",
            "trade",
        ],
        "joined-v3 lifecycle",
    )?;
    if boolean(lifecycle_value, "market_genesis_assisted")? {
        return Err("joined-v3 lifecycle is marked genesis-assisted".into());
    }
    let market = string(lifecycle_value, "market")?;
    if correct.get("market").and_then(Value::as_str) != Some(market) {
        return Err("joined-v3 lifecycle market differs from the prepared manifest".into());
    }
    let users = exact_strings(
        lifecycle
            .get("ephemeral_users")
            .ok_or("ephemeral_users is absent")?,
        &[payer, second_owner],
        "ephemeral_users",
    )?;
    let tokens = exact_strings(
        lifecycle
            .get("user_collateral_tokens")
            .ok_or("user_collateral_tokens is absent")?,
        &[buyer_token, seller_token],
        "user_collateral_tokens",
    )?;
    if canonical_unsigned_decimal(lifecycle, "collateral_atoms")? != "128" {
        return Err("joined-v3 collateral quantity is not the exact 128 atoms".into());
    }

    for (field, label) in [
        ("create_market_signature", "joined-create-market"),
        ("buyer_endow_signature", "joined-endow-buyer-collateral"),
        ("seller_endow_signature", "joined-endow-seller-collateral"),
        ("split_signature", "joined-seller-split-complete-sets"),
        (
            "buyer_withdraw_signature",
            "joined-buyer-withdraw-redeemed-collateral",
        ),
        (
            "seller_withdraw_signature",
            "joined-seller-withdraw-redeemed-collateral",
        ),
    ] {
        if string(lifecycle_value, field)? != signature_for(steps, label)? {
            return Err(format!("joined-v3 lifecycle {field} differs from {label}").into());
        }
    }

    let expected_redeem = [
        (payer, 1_u64, "16", "16", "joined-buyer-redeem-winning-eggs"),
        (second_owner, 0, "64", "0", "joined-seller-redeem-outcome-0"),
        (
            second_owner,
            1,
            "48",
            "48",
            "joined-seller-redeem-outcome-1",
        ),
        (second_owner, 2, "64", "0", "joined-seller-redeem-outcome-2"),
        (second_owner, 3, "64", "0", "joined-seller-redeem-outcome-3"),
    ];
    let redeem = array(lifecycle_value, "redeem_internal")?;
    if redeem.len() != expected_redeem.len() {
        return Err("joined-v3 lifecycle must retain exactly five redemption rows".into());
    }
    let mut projected_redeem = Vec::with_capacity(expected_redeem.len());
    for (index, (row, (owner, outcome, quantity, payout, label))) in
        redeem.iter().zip(expected_redeem).enumerate()
    {
        let record = row
            .as_object()
            .ok_or_else(|| format!("redemption row {index} is not an object"))?;
        exact_keys(
            record,
            &["owner", "outcome", "quantity", "payout_atoms", "signature"],
            &format!("redemption row {index}"),
        )?;
        if string(row, "owner")? != owner
            || unsigned(row, "outcome")? != outcome
            || canonical_unsigned_decimal(record, "quantity")? != quantity
            || canonical_unsigned_decimal(record, "payout_atoms")? != payout
            || string(row, "signature")? != signature_for(steps, label)?
        {
            return Err(format!("redemption row {index} differs from exact signed {label}").into());
        }
        projected_redeem.push(json!({
            "owner": owner,
            "outcome": decimal(outcome),
            "quantity": quantity,
            "payout_atoms": payout,
            "signature": string(row, "signature")?,
        }));
    }

    let terminal_value = lifecycle
        .get("terminal")
        .ok_or("lifecycle.terminal is absent")?;
    let terminal = terminal_value
        .as_object()
        .ok_or("lifecycle.terminal is not an object")?;
    exact_keys(
        terminal,
        &[
            "buyer_position_cash_atoms",
            "buyer_position_internal",
            "seller_position_cash_atoms",
            "seller_position_internal",
            "supply_internal",
            "hoard_collateral_atoms",
            "hoard_token_atoms",
            "buyer_token_atoms",
            "seller_token_atoms",
        ],
        "joined-v3 terminal",
    )?;
    for (field, expected) in [
        ("buyer_position_cash_atoms", "0"),
        ("seller_position_cash_atoms", "0"),
        ("hoard_collateral_atoms", "0"),
        ("hoard_token_atoms", "0"),
        ("buyer_token_atoms", "76"),
        ("seller_token_atoms", "52"),
    ] {
        if canonical_unsigned_decimal(terminal, field)? != expected {
            return Err(format!("joined-v3 terminal {field} is not {expected}").into());
        }
    }
    let zero_outcomes = ["0", "0", "0", "0"];
    let buyer_terminal = exact_strings(
        terminal
            .get("buyer_position_internal")
            .ok_or("buyer_position_internal is absent")?,
        &zero_outcomes,
        "buyer_position_internal",
    )?;
    let seller_terminal = exact_strings(
        terminal
            .get("seller_position_internal")
            .ok_or("seller_position_internal is absent")?,
        &zero_outcomes,
        "seller_position_internal",
    )?;
    let supply_terminal = exact_strings(
        terminal
            .get("supply_internal")
            .ok_or("supply_internal is absent")?,
        &zero_outcomes,
        "supply_internal",
    )?;

    let trade_value = lifecycle.get("trade").ok_or("lifecycle.trade is absent")?;
    let trade = trade_value
        .as_object()
        .ok_or("lifecycle.trade is not an object")?;
    exact_keys(
        trade,
        &[
            "status",
            "grid_genesis_assisted",
            "epoch_genesis_assisted",
            "order_genesis_assisted",
            "candidate_genesis_assisted",
            "price_grid",
            "price_grid_digest",
            "grid_upload_signatures",
            "policy_upload_signatures",
            "second_owner_account_creation_funding",
            "epoch",
            "epoch_id",
            "init_epoch_signature",
            "freeze_epoch_signature",
            "owners",
            "orders",
            "candidate",
            "prices",
            "fills",
            "witness_slices",
            "submit_signature",
            "complete_verification_signature",
            "selection_signature",
            "freeze_entitlement_signature",
            "entitle_signature",
            "settlement_signature",
            "post_settlement",
        ],
        "joined-v3 trade",
    )?;
    if string(trade_value, "status")? != "settled" {
        return Err("joined-v3 trade is not exactly settled".into());
    }
    for field in [
        "grid_genesis_assisted",
        "epoch_genesis_assisted",
        "order_genesis_assisted",
        "candidate_genesis_assisted",
    ] {
        if boolean(trade_value, field)? {
            return Err(format!("joined-v3 {field} is true").into());
        }
    }

    let grid_signatures = signature_sequence(
        trade_value,
        "grid_upload_signatures",
        steps,
        &[
            "joined-price-grid-artifact-begin",
            "joined-price-grid-artifact-write-0",
            "joined-price-grid-artifact-write-1",
            "joined-price-grid-artifact-write-2",
            "joined-price-grid-artifact-write-3",
            "joined-price-grid-artifact-seal",
        ],
    )?;
    let policy_signatures = signature_sequence(
        trade_value,
        "policy_upload_signatures",
        steps,
        &[
            "joined-general-policy-artifact-begin",
            "joined-general-policy-artifact-write-0",
            "joined-general-policy-artifact-seal",
        ],
    )?;

    let funding_value = trade
        .get("second_owner_account_creation_funding")
        .ok_or("second-owner funding is absent")?;
    let funding = funding_value
        .as_object()
        .ok_or("second-owner funding is not an object")?;
    exact_keys(
        funding,
        &["lamports", "signature", "genesis_assisted"],
        "second-owner funding",
    )?;
    let funding_lamports = canonical_unsigned_decimal(funding, "lamports")?;
    if funding_lamports == "0" || boolean(funding_value, "genesis_assisted")? {
        return Err("second-owner funding is zero or genesis-assisted".into());
    }
    if string(funding_value, "signature")?
        != signature_for(steps, "joined-fund-second-owner-account-creation")?
    {
        return Err("second-owner funding signature differs from its signed step".into());
    }

    let trade_owners = exact_strings(
        trade.get("owners").ok_or("trade owners are absent")?,
        &[payer, second_owner],
        "trade owners",
    )?;
    let orders_value = trade.get("orders").ok_or("trade orders are absent")?;
    let orders = orders_value
        .as_object()
        .ok_or("trade orders is not an object")?;
    exact_keys(
        orders,
        &["buyer", "seller", "buyer_signature", "seller_signature"],
        "trade orders",
    )?;
    for (role, side, limit) in [("buyer", "buy", "7500"), ("seller", "sell", "2500")] {
        let order_record = orders.get(role).ok_or("trade order is absent")?;
        let order = order_record
            .as_object()
            .ok_or("trade order is not an object")?;
        exact_keys(
            order,
            &["outcome", "side", "quantity", "limit"],
            &format!("{role} order"),
        )?;
        if unsigned(order_record, "outcome")? != 1
            || string(order_record, "side")? != side
            || canonical_unsigned_decimal(order, "quantity")? != "16"
            || canonical_unsigned_decimal(order, "limit")? != limit
        {
            return Err(format!("{role} order differs from the exact joined book").into());
        }
    }
    for (field, label) in [
        ("buyer_signature", "joined-general-place-funded-buy"),
        ("seller_signature", "joined-general-place-funded-sell"),
    ] {
        if string(orders_value, field)? != signature_for(steps, label)? {
            return Err(format!("orders.{field} differs from {label}").into());
        }
    }

    for (field, label) in [
        ("init_epoch_signature", "joined-general-init-epoch"),
        ("freeze_epoch_signature", "joined-general-freeze-epoch"),
        ("submit_signature", "joined-general-submit-candidate"),
        (
            "complete_verification_signature",
            "joined-general-complete-clear-work",
        ),
        ("selection_signature", "joined-general-finalize-selection"),
        (
            "freeze_entitlement_signature",
            "joined-general-freeze-entitlement",
        ),
        ("entitle_signature", "joined-general-entitle-direct-slice"),
        ("settlement_signature", "joined-general-settle-direct-slice"),
    ] {
        if string(trade_value, field)? != signature_for(steps, label)? {
            return Err(format!("trade.{field} differs from {label}").into());
        }
    }

    let expected_prices = [
        2_500, 2_500, 2_500, 2_500, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    let prices = exact_unsigned_vector(
        trade.get("prices").ok_or("candidate prices are absent")?,
        &expected_prices,
        "candidate prices",
    )?;
    let fills = exact_unsigned_vector(
        trade.get("fills").ok_or("candidate fills are absent")?,
        &[16, 16],
        "candidate fills",
    )?;
    if unsigned(trade_value, "witness_slices")? != 1 {
        return Err("joined-v3 witness slice count is not exactly one".into());
    }

    let post_value = trade
        .get("post_settlement")
        .ok_or("post_settlement is absent")?;
    let post = post_value
        .as_object()
        .ok_or("post_settlement is not an object")?;
    exact_keys(
        post,
        &[
            "buyer_cash",
            "buyer_internal",
            "seller_cash",
            "seller_internal",
            "locked_collateral",
            "pooled_custody",
        ],
        "post_settlement",
    )?;
    for (field, expected) in [
        ("buyer_cash", "60"),
        ("seller_cash", "4"),
        ("locked_collateral", "64"),
        ("pooled_custody", "128"),
    ] {
        if canonical_unsigned_decimal(post, field)? != expected {
            return Err(format!("post_settlement.{field} is not {expected}").into());
        }
    }
    let buyer_post = exact_strings(
        post.get("buyer_internal")
            .ok_or("post-settlement buyer_internal is absent")?,
        &["0", "16", "0", "0"],
        "post-settlement buyer_internal",
    )?;
    let seller_post = exact_strings(
        post.get("seller_internal")
            .ok_or("post-settlement seller_internal is absent")?,
        &["64", "48", "64", "64"],
        "post-settlement seller_internal",
    )?;

    let price_grid = string(trade_value, "price_grid")?;
    let epoch = string(trade_value, "epoch")?;
    let unique_addresses = [
        market,
        payer,
        second_owner,
        buyer_token,
        seller_token,
        price_grid,
        epoch,
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    if unique_addresses.len() != 7 {
        return Err("joined-v3 projected identities alias".into());
    }

    let projected_terminal = json!({
        "buyer_position_cash_atoms": "0",
        "buyer_position_internal": buyer_terminal,
        "seller_position_cash_atoms": "0",
        "seller_position_internal": seller_terminal,
        "supply_internal": supply_terminal,
        "hoard_collateral_atoms": "0",
        "hoard_token_atoms": "0",
        "buyer_token_atoms": "76",
        "seller_token_atoms": "52",
    });
    let projected_funding = json!({
        "lamports": funding_lamports,
        "signature": string(funding_value, "signature")?,
        "genesis_assisted": false,
    });
    let projected_orders = json!({
        "buyer": {"outcome": "1", "side": "buy", "quantity": "16", "limit": "7500"},
        "seller": {"outcome": "1", "side": "sell", "quantity": "16", "limit": "2500"},
        "buyer_signature": string(orders_value, "buyer_signature")?,
        "seller_signature": string(orders_value, "seller_signature")?,
    });
    let projected_post = json!({
        "buyer_cash": "60",
        "buyer_internal": buyer_post,
        "seller_cash": "4",
        "seller_internal": seller_post,
        "locked_collateral": "64",
        "pooled_custody": "128",
    });
    let projected_trade = json!({
        "status": "settled",
        "grid_genesis_assisted": false,
        "epoch_genesis_assisted": false,
        "order_genesis_assisted": false,
        "candidate_genesis_assisted": false,
        "price_grid": price_grid,
        "price_grid_digest": fixed_hash(
            trade.get("price_grid_digest").ok_or("price_grid_digest is absent")?,
            "price_grid_digest"
        )?,
        "grid_upload_signatures": grid_signatures,
        "policy_upload_signatures": policy_signatures,
        "second_owner_account_creation_funding": projected_funding,
        "epoch": epoch,
        "epoch_id": fixed_hash(
            trade.get("epoch_id").ok_or("epoch_id is absent")?, "epoch_id"
        )?,
        "init_epoch_signature": string(trade_value, "init_epoch_signature")?,
        "freeze_epoch_signature": string(trade_value, "freeze_epoch_signature")?,
        "owners": trade_owners,
        "orders": projected_orders,
        "candidate": fixed_hash(
            trade.get("candidate").ok_or("candidate is absent")?, "candidate"
        )?,
        "prices": prices,
        "fills": fills,
        "witness_slices": "1",
        "submit_signature": string(trade_value, "submit_signature")?,
        "complete_verification_signature": string(
            trade_value, "complete_verification_signature"
        )?,
        "selection_signature": string(trade_value, "selection_signature")?,
        "freeze_entitlement_signature": string(
            trade_value, "freeze_entitlement_signature"
        )?,
        "entitle_signature": string(trade_value, "entitle_signature")?,
        "settlement_signature": string(trade_value, "settlement_signature")?,
        "post_settlement": projected_post,
    });
    Ok(json!({
        "market_genesis_assisted": false,
        "market": market,
        "ephemeral_users": users,
        "user_collateral_tokens": tokens,
        "collateral_atoms": "128",
        "create_market_signature": string(lifecycle_value, "create_market_signature")?,
        "buyer_endow_signature": string(lifecycle_value, "buyer_endow_signature")?,
        "seller_endow_signature": string(lifecycle_value, "seller_endow_signature")?,
        "split_signature": string(lifecycle_value, "split_signature")?,
        "redeem_internal": projected_redeem,
        "buyer_withdraw_signature": string(lifecycle_value, "buyer_withdraw_signature")?,
        "seller_withdraw_signature": string(lifecycle_value, "seller_withdraw_signature")?,
        "terminal": projected_terminal,
        "trade": projected_trade,
    }))
}

#[allow(clippy::too_many_lines)]
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

    let campaign_mode = campaign_mode(manifest, result)?;
    let joined_schema = if campaign_mode == JOINED_LIFECYCLE_MODE {
        match array(result, "steps")?.len() {
            count if count == JOINED_V2_STEP_LABELS.len() => Some(JoinedSchema::V2),
            count if count == JOINED_V3_STEP_LABELS.len() => Some(JoinedSchema::V3),
            count => {
                return Err(format!(
                    "joined transcript has {count} steps, expected exact v2 or v3 count"
                )
                .into())
            }
        }
    } else {
        None
    };
    let (schema, expected_labels): (&str, &[&str]) = match joined_schema {
        Some(JoinedSchema::V2) => (JOINED_V2_SCHEMA, &JOINED_V2_STEP_LABELS),
        Some(JoinedSchema::V3) => (JOINED_V3_SCHEMA, &JOINED_V3_STEP_LABELS),
        None => (SOURCE_SCHEMA, &SOURCE_STEP_LABELS),
    };
    let steps = steps(result, expected_labels)?;
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

    let lifecycle = match joined_schema {
        Some(JoinedSchema::V2) => joined_lifecycle_v2(manifest, result, &steps)?,
        Some(JoinedSchema::V3) => joined_lifecycle_v3(manifest, result, &steps)?,
        None => {
            if result
                .get("lifecycle")
                .is_some_and(|value| !value.is_null())
            {
                return Err(
                    "source-only transcript unexpectedly carries a lifecycle projection".into(),
                );
            }
            Value::Null
        }
    };

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
        "campaign_mode": campaign_mode,
    });
    let campaign = json!({
        "type": "pyth-campaign",
        "schema": schema,
        "claim": CLAIM,
        "campaign_mode": campaign_mode,
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
        "lifecycle": lifecycle,
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
    use super::{
        build_view, CLAIM, JOINED_LIFECYCLE_MODE, JOINED_V2_SCHEMA, JOINED_V2_STEP_LABELS,
        JOINED_V3_SCHEMA, JOINED_V3_STEP_LABELS, SOURCE_SCHEMA, SOURCE_STEP_LABELS,
    };
    use serde_json::{json, Value};

    fn hash(byte: char) -> String {
        std::iter::repeat_n(byte, 64).collect()
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
        let steps = SOURCE_STEP_LABELS
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
            "upstream_pyth_crosschain_commit": "c".repeat(40),
            "dragons_clutch_repository_head": "d".repeat(40),
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

    fn joined_fixtures() -> (Value, Value, Value) {
        let (mut manifest, mut result, probe) = fixtures();
        let steps = JOINED_V2_STEP_LABELS
            .iter()
            .enumerate()
            .map(|(index, label)| {
                let refused = label.contains("-rollback");
                json!({
                    "label": label,
                    "signature": format!("joined-signature-{index}"),
                    "slot": 460_336_312_u64 + u64::try_from(index).unwrap(),
                    "compute_units_consumed": 200_u64 + u64::try_from(index).unwrap(),
                    "fee_lamports": 5000,
                    "signed_wire_sha256": hash('b'),
                    "program_order": ["ComputeBudget111", "Program111"],
                    "error": if refused {
                        json!({"InstructionError": [2, {"Custom": 122}]})
                    } else {
                        Value::Null
                    },
                })
            })
            .collect::<Vec<_>>();
        manifest["campaign_mode"] = json!(JOINED_LIFECYCLE_MODE);
        manifest["correct"] = json!({
            "market": "market111",
            "market_genesis_assisted": false,
            "user_collateral_token": "user-token111",
        });
        result["campaign_mode"] = json!(JOINED_LIFECYCLE_MODE);
        result["joined_post_append_signature"] = json!("joined-signature-13");
        result["seal_signature"] = json!("joined-signature-14");
        result["resolve_signature"] = json!("joined-signature-15");
        result["lifecycle"] = json!({
            "market_genesis_assisted": false,
            "market": "market111",
            "ephemeral_user": "ephemeral-user111",
            "user_collateral_token": "user-token111",
            "collateral_atoms": "64",
            "create_market_signature": "joined-signature-8",
            "endow_signature": "joined-signature-9",
            "split_signature": "joined-signature-10",
            "redeem_internal": [
                {"outcome": 0, "quantity": "64", "payout_atoms": "0", "signature": "joined-signature-16"},
                {"outcome": 1, "quantity": "64", "payout_atoms": "64", "signature": "joined-signature-17"},
                {"outcome": 2, "quantity": "64", "payout_atoms": "0", "signature": "joined-signature-18"},
                {"outcome": 3, "quantity": "64", "payout_atoms": "0", "signature": "joined-signature-19"},
            ],
            "withdraw_signature": "joined-signature-20",
            "terminal": {
                "position_cash_atoms": "0",
                "position_internal": ["0", "0", "0", "0"],
                "supply_internal": ["0", "0", "0", "0"],
                "hoard_collateral_atoms": "0",
                "hoard_token_atoms": "0",
                "user_token_atoms": "64",
            },
            "trade": {
                "status": "blocked",
                "reason_code": "missing-sealed-price-grid-and-epoch-plane",
                "detail": "InitEpoch requires the immutable Terms' exact sealed PriceGrid; no mock is substituted.",
            },
        });
        result["steps"] = json!(steps);
        (manifest, result, probe)
    }

    #[allow(clippy::too_many_lines)]
    fn joined_v3_fixtures() -> (Value, Value, Value) {
        let (mut manifest, mut result, probe) = fixtures();
        let signature = |index: usize| format!("joined-v3-signature-{index}");
        let steps = JOINED_V3_STEP_LABELS
            .iter()
            .enumerate()
            .map(|(index, label)| {
                let refused = label.contains("-rollback");
                json!({
                    "label": label,
                    "signature": signature(index),
                    "slot": 460_336_312_u64 + u64::try_from(index).unwrap(),
                    "compute_units_consumed": 300_u64 + u64::try_from(index).unwrap(),
                    "fee_lamports": 5000,
                    "signed_wire_sha256": hash('b'),
                    "program_order": ["ComputeBudget111", "Program111"],
                    "error": if refused {
                        json!({"InstructionError": [2, {"Custom": 122}]})
                    } else {
                        Value::Null
                    },
                })
            })
            .collect::<Vec<_>>();
        manifest["campaign_mode"] = json!(JOINED_LIFECYCLE_MODE);
        manifest["payer"] = json!("buyer111");
        manifest["correct"] = json!({
            "market": "market111",
            "market_genesis_assisted": false,
            "user_collateral_token": "buyer-token111",
            "second_owner": "seller111",
            "second_owner_collateral_token": "seller-token111",
        });
        result["campaign_mode"] = json!(JOINED_LIFECYCLE_MODE);
        result["joined_post_append_signature"] = json!(signature(10));
        result["seal_signature"] = json!(signature(11));
        result["resolve_signature"] = json!(signature(44));
        let terminal = json!({
            "buyer_position_cash_atoms": "0",
            "buyer_position_internal": ["0", "0", "0", "0"],
            "seller_position_cash_atoms": "0",
            "seller_position_internal": ["0", "0", "0", "0"],
            "supply_internal": ["0", "0", "0", "0"],
            "hoard_collateral_atoms": "0",
            "hoard_token_atoms": "0",
            "buyer_token_atoms": "76",
            "seller_token_atoms": "52",
        });
        let second_owner_funding = json!({
            "lamports": "1234560",
            "signature": signature(12),
            "genesis_assisted": false,
        });
        let orders = json!({
            "buyer": {"outcome": 1, "side": "buy", "quantity": "16", "limit": "7500"},
            "seller": {"outcome": 1, "side": "sell", "quantity": "16", "limit": "2500"},
            "buyer_signature": signature(28),
            "seller_signature": signature(29),
        });
        let post_settlement = json!({
            "buyer_cash": "60",
            "buyer_internal": ["0", "16", "0", "0"],
            "seller_cash": "4",
            "seller_internal": ["64", "48", "64", "64"],
            "locked_collateral": "64",
            "pooled_custody": "128",
        });
        let trade = json!({
            "status": "settled",
            "grid_genesis_assisted": false,
            "epoch_genesis_assisted": false,
            "order_genesis_assisted": false,
            "candidate_genesis_assisted": false,
            "price_grid": "price-grid111",
            "price_grid_digest": vec![7_u8; 32],
            "grid_upload_signatures": (13..=18).map(&signature).collect::<Vec<_>>(),
            "policy_upload_signatures": (23..=25).map(&signature).collect::<Vec<_>>(),
            "second_owner_account_creation_funding": second_owner_funding,
            "epoch": "epoch111",
            "epoch_id": vec![8_u8; 32],
            "init_epoch_signature": signature(26),
            "freeze_epoch_signature": signature(30),
            "owners": ["buyer111", "seller111"],
            "orders": orders,
            "candidate": vec![9_u8; 32],
            "prices": [2500, 2500, 2500, 2500, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            "fills": [16, 16],
            "witness_slices": 1,
            "submit_signature": signature(31),
            "complete_verification_signature": signature(39),
            "selection_signature": signature(40),
            "freeze_entitlement_signature": signature(41),
            "entitle_signature": signature(42),
            "settlement_signature": signature(43),
            "post_settlement": post_settlement,
        });
        result["lifecycle"] = json!({
            "market_genesis_assisted": false,
            "market": "market111",
            "ephemeral_users": ["buyer111", "seller111"],
            "user_collateral_tokens": ["buyer-token111", "seller-token111"],
            "collateral_atoms": "128",
            "create_market_signature": signature(19),
            "buyer_endow_signature": signature(20),
            "seller_endow_signature": signature(21),
            "split_signature": signature(22),
            "redeem_internal": [
                {"owner": "buyer111", "outcome": 1, "quantity": "16", "payout_atoms": "16", "signature": signature(45)},
                {"owner": "seller111", "outcome": 0, "quantity": "64", "payout_atoms": "0", "signature": signature(46)},
                {"owner": "seller111", "outcome": 1, "quantity": "48", "payout_atoms": "48", "signature": signature(47)},
                {"owner": "seller111", "outcome": 2, "quantity": "64", "payout_atoms": "0", "signature": signature(48)},
                {"owner": "seller111", "outcome": 3, "quantity": "64", "payout_atoms": "0", "signature": signature(49)},
            ],
            "buyer_withdraw_signature": signature(50),
            "seller_withdraw_signature": signature(51),
            "terminal": terminal,
            "trade": trade,
        });
        result["steps"] = json!(steps);
        (manifest, result, probe)
    }

    #[test]
    fn public_transcripts_become_exact_decimal_display_events() {
        let (manifest, result, probe) = fixtures();
        let view = build_view(&manifest, &result, &probe).unwrap();
        assert_eq!(view.identity["mode"], "pyth-local");
        assert_eq!(view.campaign["schema"], SOURCE_SCHEMA);
        assert_eq!(view.campaign["source"]["interval_lower"], "99980929");
        assert_eq!(view.campaign["steps"][0]["slot"], "460336312");
        assert_eq!(view.campaign["steps"][8]["state"], "refused-as-expected");
    }

    #[test]
    fn joined_transcript_projects_signed_lifecycle_and_blocker() {
        let (manifest, result, probe) = joined_fixtures();
        let view = build_view(&manifest, &result, &probe).unwrap();
        assert_eq!(view.campaign["schema"], JOINED_V2_SCHEMA);
        assert_eq!(view.campaign["campaign_mode"], JOINED_LIFECYCLE_MODE);
        assert_eq!(view.campaign["steps"].as_array().unwrap().len(), 21);
        assert_eq!(view.campaign["lifecycle"]["collateral_atoms"], "64");
        assert_eq!(
            view.campaign["lifecycle"]["trade"]["reason_code"],
            "missing-sealed-price-grid-and-epoch-plane"
        );
    }

    #[test]
    fn joined_v3_projects_exact_settled_trade_and_two_owner_terminal_state() {
        let (manifest, result, probe) = joined_v3_fixtures();
        let view = build_view(&manifest, &result, &probe).unwrap();
        assert_eq!(view.campaign["schema"], JOINED_V3_SCHEMA);
        assert_eq!(view.campaign["steps"].as_array().unwrap().len(), 52);
        assert_eq!(view.campaign["lifecycle"]["trade"]["status"], "settled");
        assert_eq!(
            view.campaign["lifecycle"]["trade"]["grid_upload_signatures"]
                .as_array()
                .unwrap()
                .len(),
            6
        );
        assert_eq!(
            view.campaign["lifecycle"]["terminal"]["buyer_token_atoms"],
            "76"
        );
        assert_eq!(
            view.campaign["lifecycle"]["terminal"]["seller_token_atoms"],
            "52"
        );
    }

    #[test]
    fn joined_v3_refuses_a_substituted_candidate_or_terminal_atom() {
        let (manifest, mut result, probe) = joined_v3_fixtures();
        result["lifecycle"]["trade"]["prices"][1] = json!(2501);
        assert!(build_view(&manifest, &result, &probe).is_err());

        let (manifest, mut result, probe) = joined_v3_fixtures();
        result["lifecycle"]["terminal"]["seller_token_atoms"] = json!("51");
        assert!(build_view(&manifest, &result, &probe).is_err());
    }

    #[test]
    fn joined_v3_refuses_unknown_fields_and_signed_step_reordering() {
        let (manifest, mut result, probe) = joined_v3_fixtures();
        result["lifecycle"]["trade"]["mock_source"] = json!(true);
        assert!(build_view(&manifest, &result, &probe).is_err());

        let (manifest, mut result, probe) = joined_v3_fixtures();
        result["steps"].as_array_mut().unwrap().swap(39, 40);
        assert!(build_view(&manifest, &result, &probe).is_err());
    }

    #[test]
    fn joined_transcript_refuses_false_terminal_conservation() {
        let (manifest, mut result, probe) = joined_fixtures();
        result["lifecycle"]["terminal"]["user_token_atoms"] = json!("63");
        assert!(build_view(&manifest, &result, &probe).is_err());
    }

    #[test]
    fn joined_transcript_refuses_substituted_trade_status() {
        let (manifest, mut result, probe) = joined_fixtures();
        result["lifecycle"]["trade"]["status"] = json!("mocked");
        assert!(build_view(&manifest, &result, &probe).is_err());
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
