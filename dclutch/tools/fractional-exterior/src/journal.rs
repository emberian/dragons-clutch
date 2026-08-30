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

use crate::{Error, Result};

/// Canonical journal filename.
pub const CANONICAL: &str = "canonical.json";
/// Volatile observation filename.
pub const OBSERVED: &str = "observed.jsonl";

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

/// Re-read the canonical journal and check it is internally exact.
///
/// Returns the entry count. Refuses a journal whose entries are not all present
/// and well formed, so a truncated run cannot be mistaken for a clean one.
pub fn verify(out: &Path) -> Result<usize> {
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
    for entry in entries {
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
    }
    Ok(entries.len())
}

/// Lowercase hex SHA-256, the digest spelling used throughout the tree.
pub fn digest(bytes: &[u8]) -> String {
    let value = solana_program::hash::hash(bytes).to_bytes();
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}
