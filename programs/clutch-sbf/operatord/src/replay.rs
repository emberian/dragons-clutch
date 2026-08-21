//! The replay falsifier: one builder, proved by byte comparison.
//!
//! The Operator Bench's central claim is that the browser cannot originate a
//! transaction — every byte that reaches the bank is built by
//! `clutch_sbf_harness`.  That claim is only worth something if it is
//! checkable, so this rebuilds every transaction of a plan through the
//! daemon's own call into those builders and byte-diffs the result against
//! the files the harness emitted.
//!
//! It is **not** a proof about the wire format, and it is not translation
//! validation.  It is a byte comparison between two callers of one function,
//! plus a corruption that must go red.  Described at that resolution and no
//! higher.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Emit a general-clearing plan through the library's own emitter.
///
/// Key-free by default: with no `CLUTCH_COMMITTED_*` variable set, every
/// fixture identity is a System-program PDA of a literal seed, so the plan is
/// reproducible and carries no key material at all.
pub fn emit(out_dir: &Path) -> Result<()> {
    for sub in ["accounts", "expected", "tx"] {
        fs::create_dir_all(out_dir.join(sub))?;
    }
    let fixture = clutch_sbf_harness::build_fixture();
    clutch_sbf_harness::emit_general_committed_plan(out_dir, &fixture);
    Ok(())
}

fn collect(dir: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in fs::read_dir(&current)? {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let name = path
                    .strip_prefix(dir)?
                    .to_str()
                    .ok_or("plan file name is not utf-8")?
                    .to_string();
                out.insert(name, fs::read(&path)?);
            }
        }
    }
    Ok(out)
}

/// What one replay found.
pub struct Verdict {
    pub compared: usize,
    pub transactions: usize,
    pub differing: Vec<String>,
    pub missing: Vec<String>,
    pub corruption_detected: bool,
}

impl Verdict {
    pub fn green(&self) -> bool {
        self.differing.is_empty() && self.missing.is_empty() && self.corruption_detected
    }
}

/// Rebuild `plan_dir` through the builders and byte-diff, then require that a
/// single corrupted byte is caught.
pub fn replay(plan_dir: &Path, scratch: &Path) -> Result<Verdict> {
    let emitted = collect(plan_dir)?;
    fs::create_dir_all(scratch)?;
    emit(scratch)?;
    let rebuilt = collect(scratch)?;

    let mut differing = Vec::new();
    let mut missing = Vec::new();
    let mut transactions = 0;
    for (name, bytes) in &emitted {
        match rebuilt.get(name) {
            None => missing.push(name.clone()),
            Some(theirs) => {
                if name.starts_with("tx/") {
                    transactions += 1;
                }
                if theirs != bytes {
                    differing.push(name.clone());
                }
            }
        }
    }
    for name in rebuilt.keys() {
        if !emitted.contains_key(name) {
            missing.push(format!("only in rebuild: {name}"));
        }
    }

    // The comparator must be able to fail.  Flip one byte of one rebuilt
    // transaction and require the comparison to notice.
    let corruption_detected = rebuilt
        .iter()
        .find(|(name, bytes)| name.starts_with("tx/") && !bytes.is_empty())
        .is_some_and(|(name, bytes)| {
            let mut corrupted = bytes.clone();
            corrupted[0] = corrupted[0].wrapping_add(1);
            emitted.get(name) != Some(&corrupted)
        });

    Ok(Verdict {
        compared: emitted.len(),
        transactions,
        differing,
        missing,
        corruption_detected,
    })
}

/// A scratch directory beside the plan being replayed.
pub fn scratch_beside(plan_dir: &Path) -> PathBuf {
    let stamp = std::process::id();
    plan_dir
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("operator-replay-{stamp}"))
}
