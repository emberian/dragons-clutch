#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Census evidence emitter for `solana-program-test` fast-lane campaigns.
//!
//! `tools/gauntlet/census` cross-checks every claimed route against the chain's
//! own `Program <address> invoke [n]` log lines, so a campaign that cannot
//! surface finalized logs cannot feed it. Until now the only producer was the
//! local-validator bootstrap, and every real-ELF ProgramTest campaign in the
//! tree was invisible to the census however many routes it drove.
//!
//! This crate is the missing half. A campaign calls [`record`] once per
//! submitted transaction; each call writes one self-contained JSON object into
//! the directory named by `DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR`. A campaign's run
//! script then folds that directory into the single `{"transactions": [...]}`
//! document `census observe --evidence` consumes.
//!
//! One file per transaction rather than one shared file, because `cargo test`
//! runs `#[tokio::test]` cases on many threads and an interleaved append is a
//! corrupt document. Each file is named by the transaction's signature, which
//! is also the census's dedup key, so a campaign re-run overwrites rather than
//! double-counting.
//!
//! # What the census actually reads
//!
//! Only these fields, and it is strict about two of them:
//!
//! - `label` — required; the key a `bindings.json` entry matches on.
//! - `signature` — the dedup key. Empty or duplicated signatures collapse
//!   distinct transactions into one observation, so [`record`] refuses an empty
//!   one rather than emitting evidence that silently under-counts.
//! - `slot`, `logs`, `compute_units_consumed` — recorded as observed.
//! - `error` — `null` means the transaction succeeded, which is the ONLY thing
//!   that makes a route EXECUTED. Any non-null value is a refusal.
//!
//! # What this is not
//!
//! ProgramTest is not a validator. It does not submit a packet, so it cannot
//! catch a frame that overruns the legacy packet maximum; its Loader V3
//! ProgramData accounts are constructed by the campaign rather than deployed;
//! and its compute ceiling is a builder setting rather than a submitted budget.
//! `tools/gauntlet/TIERS.md` states the bar a fast lane must meet and requires
//! each tier to say which conditions it satisfies. A route whose only
//! observation came from a fast lane is recorded under that campaign's name so
//! the report shows where the evidence came from.

use std::{
    env,
    ffi::OsString,
    fs,
    io::Error as IoError,
    path::{Path, PathBuf},
};

/// Environment variable naming the directory each transaction record lands in.
pub const EVIDENCE_DIR_ENV: &str = "DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR";

/// Why one transaction could not be recorded.
#[derive(Debug)]
pub enum EvidenceError {
    /// The signature was empty, and an empty signature is not a dedup key.
    EmptySignature,
    /// The label was empty; the census refuses an unlabelled transaction.
    EmptyLabel,
    /// The evidence directory could not be created or written.
    Io(IoError),
}

impl core::fmt::Display for EvidenceError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptySignature => {
                formatter.write_str("campaign transaction has no signature to key an observation on")
            }
            Self::EmptyLabel => formatter.write_str("campaign transaction has no label"),
            Self::Io(error) => write!(formatter, "evidence directory: {error}"),
        }
    }
}

impl std::error::Error for EvidenceError {}

/// One submitted transaction, as the census will read it.
pub struct TransactionEvidence<'a> {
    /// Stable binding key. Must match exactly one `bindings.json` pattern.
    pub label: &'a str,
    /// The transaction's own signature; the census dedup key.
    pub signature: &'a str,
    /// The slot the campaign observed the transaction at.
    pub slot: u64,
    /// `None` on success. `Some` carries the refusal, rendered by the caller.
    pub error: Option<&'a str>,
    /// The runtime's own log messages, verbatim and in order.
    pub logs: &'a [String],
    /// Compute units the runtime reported consuming.
    pub compute_units_consumed: Option<u64>,
    /// The transaction's serialised extent, when the campaign measured it.
    ///
    /// ProgramTest submits no packet, so it cannot enforce Solana's 1,232-byte
    /// maximum and a frame that exceeds it survives a fast lane untouched --
    /// Found31 was exactly that defect and it survived every fixture test. A
    /// campaign that wants the second fast-lane condition has to MEASURE
    /// against the limit rather than ask the runtime to enforce it, and record
    /// what it measured so a witness can check it. `None` says the campaign did
    /// not measure, which is honest; it is not a claim that the frame fits.
    pub wire_bytes: Option<usize>,
}

/// Where evidence is being written, if a campaign asked for any.
///
/// Returns `None` when [`EVIDENCE_DIR_ENV`] is unset, which is the ordinary
/// case: the campaign is running as a plain test and nobody wants a census
/// document out of it. A campaign should treat that as "do not record", never
/// as an error.
#[must_use]
pub fn evidence_directory() -> Option<PathBuf> {
    match env::var_os(EVIDENCE_DIR_ENV) {
        Some(value) if value != OsString::new() => Some(PathBuf::from(value)),
        _ => None,
    }
}

/// Record one transaction, if a campaign asked for evidence.
///
/// A no-op returning `Ok(())` when [`EVIDENCE_DIR_ENV`] is unset, so a campaign
/// can call this unconditionally and stay an ordinary test.
///
/// # Errors
///
/// Refuses an empty label or signature, and propagates any filesystem failure.
/// A campaign should let these fail the test: evidence that cannot be written
/// is not evidence that can be quietly skipped.
pub fn record(evidence: &TransactionEvidence<'_>) -> Result<(), EvidenceError> {
    let Some(directory) = evidence_directory() else {
        return Ok(());
    };
    record_into(&directory, evidence)
}

/// Record one transaction into a named directory.
///
/// [`record`] is this with the directory read from the environment. This form
/// exists so the guards below can be tested for real: `std::env::set_var` is
/// `unsafe` under edition 2024 and this crate forbids `unsafe`, so a test that
/// went through the environment could only assert a tautology.
///
/// # Errors
///
/// Refuses an empty label or signature, and propagates any filesystem failure.
pub fn record_into(
    directory: &Path,
    evidence: &TransactionEvidence<'_>,
) -> Result<(), EvidenceError> {
    if evidence.label.is_empty() {
        return Err(EvidenceError::EmptyLabel);
    }
    if evidence.signature.is_empty() {
        return Err(EvidenceError::EmptySignature);
    }
    fs::create_dir_all(directory).map_err(EvidenceError::Io)?;
    let path = directory.join(format!("{}.json", sanitize(evidence.signature)));
    fs::write(&path, render(evidence)).map_err(EvidenceError::Io)
}

/// Fold a directory of per-transaction records into one census document.
///
/// This is the same operation a run script performs with `jq`; it exists so a
/// campaign can produce the finished document without depending on `jq` being
/// installed, and so the exact shape has one owner in Rust.
///
/// # Errors
///
/// Propagates any filesystem failure reading the directory.
pub fn fold(directory: &Path) -> Result<String, EvidenceError> {
    let mut entries = fs::read_dir(directory)
        .map_err(EvidenceError::Io)?
        .filter_map(|entry| entry.ok().map(|found| found.path()))
        .filter(|path| path.extension().is_some_and(|extension| extension == "json"))
        .collect::<Vec<_>>();
    entries.sort();
    let mut document = String::from("{\n  \"schema\": \"dclutch-program-test-evidence-v1\",\n  \"transactions\": [\n");
    for (index, path) in entries.iter().enumerate() {
        let body = fs::read_to_string(path).map_err(EvidenceError::Io)?;
        document.push_str(body.trim_end());
        if index.saturating_add(1) != entries.len() {
            document.push(',');
        }
        document.push('\n');
    }
    document.push_str("  ]\n}\n");
    Ok(document)
}

/// A signature is base58 and therefore already filename-safe; be sure anyway.
fn sanitize(signature: &str) -> String {
    signature
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn render(evidence: &TransactionEvidence<'_>) -> String {
    let mut out = String::from("    {\n");
    out.push_str(&format!("      \"label\": {},\n", json_string(evidence.label)));
    out.push_str(&format!(
        "      \"signature\": {},\n",
        json_string(evidence.signature)
    ));
    out.push_str(&format!("      \"slot\": {},\n", evidence.slot));
    match evidence.error {
        None => out.push_str("      \"error\": null,\n"),
        Some(error) => out.push_str(&format!("      \"error\": {},\n", json_string(error))),
    }
    match evidence.compute_units_consumed {
        None => out.push_str("      \"compute_units_consumed\": null,\n"),
        Some(units) => out.push_str(&format!("      \"compute_units_consumed\": {units},\n")),
    }
    match evidence.wire_bytes {
        None => out.push_str("      \"wire_bytes\": null,\n"),
        Some(bytes) => out.push_str(&format!("      \"wire_bytes\": {bytes},\n")),
    }
    out.push_str("      \"logs\": [");
    for (index, line) in evidence.logs.iter().enumerate() {
        if index == 0 {
            out.push('\n');
        }
        out.push_str("        ");
        out.push_str(&json_string(line));
        if index.saturating_add(1) != evidence.logs.len() {
            out.push(',');
        }
        out.push('\n');
    }
    if evidence.logs.is_empty() {
        out.push_str("]\n");
    } else {
        out.push_str("      ]\n");
    }
    out.push_str("    }");
    out
}

/// Render one JSON string literal.
///
/// Log lines are runtime output and carry quotes, backslashes, and occasional
/// control bytes; an emitter that only wrapped them in quotes would produce a
/// document the census cannot parse, on exactly the campaigns most worth
/// recording.
fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len().saturating_add(2));
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            control if control < ' ' || control == '\u{7f}' => {
                out.push_str(&format!("\\u{:04x}", u32::from(control)));
            }
            ordinary => out.push(ordinary),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests;
