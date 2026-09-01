//! Supervisor-owned crash seams for the complete private lifecycle.
//!
//! These hooks are inert unless the owned-loopback runner supplies the exact
//! five-variable arm.  At an armed seam the exterior writes and fsyncs one
//! receipt describing the already-durable journal, then parks.  The exterior
//! never kills itself: the supervisor must observe the receipt and deliver an
//! actual `SIGKILL`, which is the process fact authenticated by the V2 chaos
//! session.

use std::{
    env, fs,
    fs::OpenOptions,
    io::Write as _,
    os::unix::fs::OpenOptionsExt as _,
    path::{Path, PathBuf},
    str::FromStr as _,
    thread,
    time::{Duration, Instant},
};

use serde::Serialize;
use sha2::{Digest as _, Sha256};
use solana_sdk::signature::Signature;

use crate::{Error, Result};

const CASE_ENV: &str = "DCLUTCH_CHAOS_FAULT_CASE_V1";
const MUTATION_ENV: &str = "DCLUTCH_CHAOS_FAULT_MUTATION_V1";
const BOUNDARY_ENV: &str = "DCLUTCH_CHAOS_FAULT_BOUNDARY_V1";
const JOURNAL_ENV: &str = "DCLUTCH_CHAOS_FAULT_JOURNAL_V1";
const RECEIPT_ENV: &str = "DCLUTCH_CHAOS_FAULT_RECEIPT_V1";
const RECEIPT_SCHEMA_V1: &str = "dclutch-owned-loopback-chaos-fault-boundary-v1";
const PARK_LIMIT: Duration = Duration::from_secs(300);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum BoundaryV1 {
    DispatchingBeforeSend,
    LandedBeforeFinalizationFsync,
}

/// Whether the supervisor armed this exact mutation/boundary.
///
/// Callers use this only to widen the otherwise tiny post-send window long
/// enough to observe finalized history before parking. The receipt-producing
/// [`park_if_armed_v1`] still revalidates the complete arm and owned-loopback
/// journal.
pub(crate) fn is_armed_for_v1(mutation: &str, boundary: BoundaryV1) -> Result<bool> {
    Ok(read_arm_v1()?
        .is_some_and(|arm| arm.mutation == mutation && arm.boundary == boundary.label()))
}

impl BoundaryV1 {
    fn label(self) -> &'static str {
        match self {
            Self::DispatchingBeforeSend => "dispatching-before-send",
            Self::LandedBeforeFinalizationFsync => "landed-before-finalization-fsync",
        }
    }

    fn send_count(self) -> u64 {
        match self {
            Self::DispatchingBeforeSend => 0,
            Self::LandedBeforeFinalizationFsync => 1,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ArmV1 {
    case_id: String,
    mutation: String,
    boundary: String,
    journal: PathBuf,
    receipt: PathBuf,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FaultReceiptV1<'a> {
    schema: &'static str,
    status: &'static str,
    case_id: &'a str,
    target_mutation: &'a str,
    boundary: &'a str,
    process_id: u32,
    durable_phase: &'static str,
    journal_path: &'a str,
    journal_before_kill_sha256: String,
    intent_sha256: &'a str,
    packet_sha256: &'a str,
    signature: &'a str,
    send_count_before_kill: u64,
}

/// Park at one exact already-durable boundary when the supervisor armed it.
///
/// `journal_path` is the semantic owner's live journal. `intent_sha256`,
/// `packet_sha256`, and `signature` must all come from that authenticated
/// journal rather than from environment input.
pub(crate) fn park_if_armed_v1(
    cluster: &str,
    mutation: &str,
    boundary: BoundaryV1,
    journal_path: &Path,
    intent_sha256: &str,
    packet_sha256: &str,
    signature: &str,
) -> Result<()> {
    let Some(arm) = read_arm_v1()? else {
        return Ok(());
    };
    if arm.mutation != mutation {
        return Ok(());
    }
    if cluster != "owned-loopback" {
        return Err(Error::new(
            "chaos fault injection is admitted only by an owned-loopback journal",
        ));
    }
    if arm.boundary != boundary.label() {
        return Err(Error::new(format!(
            "chaos target {mutation} reached {} while armed for {}",
            boundary.label(),
            arm.boundary
        )));
    }
    exact_lower_hex(intent_sha256, "chaos intent SHA-256")?;
    exact_lower_hex(packet_sha256, "chaos packet SHA-256")?;
    Signature::from_str(signature)
        .map_err(|error| Error::new(format!("chaos target signature: {error}")))?;

    let canonical_journal = canonical_regular(journal_path, "chaos target journal")?;
    let armed_journal = canonical_regular(&arm.journal, "armed chaos target journal")?;
    if canonical_journal != armed_journal {
        return Err(Error::new(
            "chaos arm named another journal than the target mutation",
        ));
    }
    let journal_bytes = fs::read(&canonical_journal)?;
    if journal_bytes.is_empty() || journal_bytes.len() > 16 * 1024 * 1024 {
        return Err(Error::new(
            "chaos target journal is empty or exceeds 16 MiB",
        ));
    }
    let journal_path_text = canonical_journal
        .to_str()
        .ok_or_else(|| Error::new("chaos target journal path is not UTF-8"))?;
    let receipt = FaultReceiptV1 {
        schema: RECEIPT_SCHEMA_V1,
        status: "armed",
        case_id: &arm.case_id,
        target_mutation: mutation,
        boundary: boundary.label(),
        process_id: std::process::id(),
        durable_phase: "dispatching",
        journal_path: journal_path_text,
        journal_before_kill_sha256: hex(&Sha256::digest(&journal_bytes)),
        intent_sha256,
        packet_sha256,
        signature,
        send_count_before_kill: boundary.send_count(),
    };
    write_receipt_new_v1(&arm.receipt, &receipt)?;

    let deadline = Instant::now() + PARK_LIMIT;
    while Instant::now() < deadline {
        thread::park_timeout(Duration::from_millis(100));
    }
    Err(Error::new(
        "chaos supervisor did not deliver SIGKILL within 300 seconds",
    ))
}

fn read_arm_v1() -> Result<Option<ArmV1>> {
    let values = [
        env::var_os(CASE_ENV),
        env::var_os(MUTATION_ENV),
        env::var_os(BOUNDARY_ENV),
        env::var_os(JOURNAL_ENV),
        env::var_os(RECEIPT_ENV),
    ];
    if values.iter().all(Option::is_none) {
        return Ok(None);
    }
    if values.iter().any(Option::is_none) {
        return Err(Error::new(
            "chaos fault arm is partial; all five V1 variables are required",
        ));
    }
    let text = |index: usize, label: &str| -> Result<String> {
        values[index]
            .as_ref()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty() && value.is_ascii())
            .map(str::to_owned)
            .ok_or_else(|| Error::new(format!("{label} is not nonempty ASCII")))
    };
    let case_id = text(0, "chaos case ID")?;
    let mutation = text(1, "chaos mutation")?;
    let boundary = text(2, "chaos boundary")?;
    if !matches!(
        boundary.as_str(),
        "dispatching-before-send" | "landed-before-finalization-fsync"
    ) {
        return Err(Error::new("chaos boundary is not one admitted seam"));
    }
    Ok(Some(ArmV1 {
        case_id,
        mutation,
        boundary,
        journal: PathBuf::from(text(3, "chaos journal path")?),
        receipt: PathBuf::from(text(4, "chaos receipt path")?),
    }))
}

fn write_receipt_new_v1(path: &Path, receipt: &FaultReceiptV1<'_>) -> Result<()> {
    if !path.is_absolute() || path.exists() {
        return Err(Error::new(
            "chaos fault receipt must be one absent absolute path",
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("chaos fault receipt has no parent"))?;
    let metadata = fs::symlink_metadata(parent)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || fs::canonicalize(parent)? != parent
    {
        return Err(Error::new(
            "chaos fault receipt parent is not a canonical directory",
        ));
    }
    let bytes = serde_json::to_vec(receipt)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    OpenOptions::new().read(true).open(parent)?.sync_all()?;
    Ok(())
}

fn canonical_regular(path: &Path, label: &str) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} must be absolute")));
    }
    let metadata = fs::symlink_metadata(path)?;
    let canonical = fs::canonicalize(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || canonical != path {
        return Err(Error::new(format!(
            "{label} must be one canonical regular file"
        )));
    }
    Ok(canonical)
}

fn exact_lower_hex(value: &str, label: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(Error::new(format!(
            "{label} is not canonical lowercase SHA-256"
        )));
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundary_names_and_send_counts_are_the_exact_two_seams() {
        assert_eq!(
            BoundaryV1::DispatchingBeforeSend.label(),
            "dispatching-before-send"
        );
        assert_eq!(BoundaryV1::DispatchingBeforeSend.send_count(), 0);
        assert_eq!(
            BoundaryV1::LandedBeforeFinalizationFsync.label(),
            "landed-before-finalization-fsync"
        );
        assert_eq!(BoundaryV1::LandedBeforeFinalizationFsync.send_count(), 1);
    }
}
