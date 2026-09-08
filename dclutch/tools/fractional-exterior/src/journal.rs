//! Durable journal with a byte-identical canonical section.
//!
//! A rerun of this exterior must produce the same journal. Two facts in a real
//! cluster run genuinely cannot: the transaction signature and the slot it
//! landed in. Recording those inside the canonical section would make
//! "byte-identical" unachievable and the discipline meaningless, so the journal
//! is split.
//!
//! `canonical.json` holds what the protocol did -- the exact instruction bytes
//! submitted, the exact account frame, whether it was accepted, any refusal
//! code, and the observed poststate. That file must be byte-identical across
//! runs, and `verify` recomputes its digest.
//!
//! `observed.jsonl` holds what the cluster happened to do this time --
//! signatures, slots, compute units. It is evidence, it is kept, and it is
//! deliberately outside the digest.

use std::{fs, path::Path};

use serde_json::{Value, json};

use crate::{
    Error, Result,
    stage::{EXPECTED_ACTIONS, ExpectedPoststate},
};

/// Canonical journal filename.
pub const CANONICAL: &str = "canonical.json";
/// Volatile observation filename.
pub const OBSERVED: &str = "observed.jsonl";
/// Finalized transaction evidence, including the chain-recovered instruction
/// bytes consumed by the route census.
pub const NATIVE_EVIDENCE: &str = "native-evidence.json";

/// One recorded action outcome.
#[derive(Clone, Debug)]
pub struct Entry {
    /// Stable action label.
    pub name: String,
    /// SHA-256 of the exact submitted instruction data.
    pub data_digest: String,
    /// SHA-256 over the ordered account frame.
    pub frame_digest: String,
    /// Whether the cluster accepted it.
    pub accepted: bool,
    /// Custom refusal code, when refused.
    pub refusal: Option<u32>,
    /// Observed protocol poststate.
    pub poststate: Value,
}

impl Entry {
    fn to_value(&self) -> Value {
        json!({
            "action": self.name,
            "instruction_data_sha256": self.data_digest,
            "account_frame_sha256": self.frame_digest,
            "accepted": self.accepted,
            "refusal": self.refusal,
            "poststate": self.poststate,
        })
    }
}

/// Write the canonical section. Pretty-printed with sorted keys by construction,
/// so the bytes are a function of the facts alone.
pub fn write_canonical(out: &Path, entries: &[Entry]) -> Result<String> {
    let value = json!({
        "schema": "dclutch/fractional-exterior/canonical/v1",
        "entries": entries.iter().map(Entry::to_value).collect::<Vec<_>>(),
    });
    let mut bytes = serde_json::to_vec_pretty(&value)?;
    bytes.push(b'\n');
    fs::write(out.join(CANONICAL), &bytes)?;
    Ok(digest(&bytes))
}

/// Append one volatile observation.
pub fn append_observed(out: &Path, value: &Value) -> Result<()> {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(out.join(OBSERVED))?;
    file.write_all(serde_json::to_string(value)?.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

/// Write the cluster-volatile finalized transaction evidence as one document.
///
/// This file is deliberately separate from [`CANONICAL`]: signatures and slots
/// change from run to run. Unlike [`OBSERVED`], it is a complete input to the
/// route census rather than a human progress log.
pub fn write_native_evidence(out: &Path, transactions: &[Value]) -> Result<String> {
    let value = json!({
        "schema": "dclutch/fractional-exterior/finalized-instructions/v1",
        "transactions": transactions,
    });
    verify_native_value(&value)?;
    let mut bytes = serde_json::to_vec_pretty(&value)?;
    bytes.push(b'\n');
    fs::write(out.join(NATIVE_EVIDENCE), &bytes)?;
    Ok(digest(&bytes))
}

/// Verify the durable finalized-instruction document without a live validator.
pub fn verify_native_evidence(out: &Path) -> Result<(usize, String)> {
    let path = out.join(NATIVE_EVIDENCE);
    let bytes = fs::read(&path).map_err(|error| {
        Error::new(format!(
            "no finalized instruction evidence at {}: {error}",
            path.display()
        ))
    })?;
    let value: Value = serde_json::from_slice(&bytes)?;
    let entries = verify_native_value(&value)?;
    let mut canonical = serde_json::to_vec_pretty(&value)?;
    canonical.push(b'\n');
    if canonical != bytes {
        return Err(Error::new("finalized instruction evidence is not canonical JSON").into());
    }
    Ok((entries, digest(&bytes)))
}

fn verify_native_value(value: &Value) -> Result<usize> {
    if value.get("schema").and_then(Value::as_str)
        != Some("dclutch/fractional-exterior/finalized-instructions/v1")
    {
        return Err(Error::new("unknown finalized instruction evidence schema").into());
    }
    let entries = value
        .get("transactions")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("finalized instruction evidence has no transactions"))?;
    if entries.len() != EXPECTED_ACTIONS.len() {
        return Err(Error::new(format!(
            "finalized instruction evidence has {} transactions; expected {}",
            entries.len(),
            EXPECTED_ACTIONS.len()
        ))
        .into());
    }
    for (entry, (expected_name, _)) in entries.iter().zip(EXPECTED_ACTIONS) {
        if entry.get("label").and_then(Value::as_str) != Some(expected_name) {
            return Err(Error::new(format!(
                "finalized instruction order refused: expected {expected_name}"
            ))
            .into());
        }
        if entry
            .get("signature")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
            || entry.get("slot").and_then(Value::as_u64).unwrap_or(0) == 0
            || entry.get("error").is_none_or(|error| !error.is_null())
            || entry
                .get("transaction_metadata_available")
                .and_then(Value::as_bool)
                != Some(true)
        {
            return Err(Error::new(format!(
                "{expected_name} is not a successful finalized transaction"
            ))
            .into());
        }
        let logs = entry
            .get("logs")
            .and_then(Value::as_array)
            .ok_or_else(|| Error::new(format!("{expected_name} omitted finalized logs")))?;
        if logs.is_empty() || !logs.iter().all(Value::is_string) {
            return Err(Error::new(format!("{expected_name} has malformed finalized logs")).into());
        }
        let instructions = entry
            .get("instructions")
            .and_then(Value::as_array)
            .ok_or_else(|| Error::new(format!("{expected_name} omitted finalized instructions")))?;
        if instructions.is_empty()
            || instructions.iter().any(|instruction| {
                instruction
                    .get("program_id")
                    .and_then(Value::as_str)
                    .is_none_or(str::is_empty)
                    || instruction
                        .get("data_hex")
                        .and_then(Value::as_str)
                        .is_none_or(|hex| {
                            hex.len() % 2 != 0
                                || !hex.bytes().all(|byte| {
                                    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
                                })
                        })
            })
        {
            return Err(Error::new(format!(
                "{expected_name} has malformed finalized instructions"
            ))
            .into());
        }
    }
    Ok(entries.len())
}

/// Re-read the canonical journal and check it is internally exact.
///
/// Returns the entry count. Refuses a journal whose entries are not all present
/// and well formed, so a truncated run cannot be mistaken for a clean one.
pub fn verify(out: &Path) -> Result<(usize, String)> {
    let path = out.join(CANONICAL);
    let bytes = fs::read(&path)
        .map_err(|error| Error::new(format!("no journal at {}: {error}", path.display())))?;
    let value: Value = serde_json::from_slice(&bytes)?;
    if value.get("schema").and_then(Value::as_str)
        != Some("dclutch/fractional-exterior/canonical/v1")
    {
        return Err(Error::new("journal is not the canonical v1 schema").into());
    }
    let entries = value
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("journal has no entries array"))?;
    if entries.len() != EXPECTED_ACTIONS.len() {
        return Err(Error::new(format!(
            "journal has {} entries; this campaign requires {}",
            entries.len(),
            EXPECTED_ACTIONS.len()
        ))
        .into());
    }
    for (entry, (expected_name, expected_poststate)) in entries.iter().zip(EXPECTED_ACTIONS) {
        for field in [
            "action",
            "instruction_data_sha256",
            "account_frame_sha256",
            "accepted",
            "poststate",
        ] {
            if entry.get(field).is_none() {
                return Err(Error::new(format!("journal entry is missing {field}")).into());
            }
        }
        if entry.get("action").and_then(Value::as_str) != Some(expected_name) {
            return Err(Error::new(format!(
                "journal action order refused: expected {expected_name}"
            ))
            .into());
        }
        if entry.get("accepted").and_then(Value::as_bool) != Some(true)
            || !entry.get("refusal").is_some_and(Value::is_null)
        {
            return Err(Error::new(format!("{expected_name} did not commit cleanly")).into());
        }
        if entry.get("poststate") != Some(&poststate_value(expected_poststate)) {
            return Err(Error::new(format!("{expected_name} poststate is not exact")).into());
        }
        for field in ["instruction_data_sha256", "account_frame_sha256"] {
            let digest = entry.get(field).and_then(Value::as_str).unwrap_or_default();
            if digest.len() != 64
                || !digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(Error::new(format!("{expected_name} has a malformed {field}")).into());
            }
        }
    }
    let mut canonical = serde_json::to_vec_pretty(&value)?;
    canonical.push(b'\n');
    if canonical != bytes {
        return Err(Error::new("journal bytes are not canonical JSON").into());
    }
    Ok((entries.len(), digest(&bytes)))
}

fn poststate_value(value: ExpectedPoststate) -> Value {
    json!({
        "shard_mint_supply": value.shard_mint_supply,
        "holder_token_amount": value.holder_token_amount,
        "sleeper_token_amount": value.sleeper_token_amount,
        "actor_native_claims": value.actor_native_claims,
        "reserve_native_claims": value.reserve_native_claims,
    })
}

/// Lowercase hex SHA-256, the digest spelling used throughout the tree.
pub fn digest(bytes: &[u8]) -> String {
    let value = solana_program::hash::hash(bytes).to_bytes();
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::{EXPECTED_ACTIONS, verify_native_value};
    use serde_json::json;

    fn accepted_native_evidence() -> serde_json::Value {
        json!({
            "schema": "dclutch/fractional-exterior/finalized-instructions/v1",
            "transactions": EXPECTED_ACTIONS.iter().enumerate().map(|(index, (label, _))| {
                json!({
                    "label": label,
                    "signature": format!("signature-{index}"),
                    "slot": index + 1,
                    "transaction_metadata_available": true,
                    "error": null,
                    "logs": ["Program log: accepted"],
                    "instructions": [{
                        "program_id": "Bswb3UPMzWLhs4WhNBSXzfULc5XnJr2LLoQvhXbBBkmC",
                        "data_hex": "4443465245513032",
                    }],
                })
            }).collect::<Vec<_>>(),
        })
    }

    #[test]
    fn finalized_evidence_requires_successful_ordered_native_transactions() {
        let accepted = accepted_native_evidence();
        assert_eq!(
            verify_native_value(&accepted).unwrap(),
            EXPECTED_ACTIONS.len()
        );

        let mut zero_slot = accepted.clone();
        zero_slot["transactions"][0]["slot"] = json!(0);
        assert!(verify_native_value(&zero_slot).is_err());

        let mut malformed_instruction = accepted.clone();
        malformed_instruction["transactions"][1]["instructions"][0]["data_hex"] = json!("0g");
        assert!(verify_native_value(&malformed_instruction).is_err());

        let mut wrong_order = accepted;
        wrong_order["transactions"][0]["label"] = json!(EXPECTED_ACTIONS[1].0);
        assert!(verify_native_value(&wrong_order).is_err());
    }
}
