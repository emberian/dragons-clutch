//! The snapshot file: devnet-grade durability, and no more than that.
//!
//! WHAT THIS IS FOR. A restart should not empty the board. That is the whole
//! ambition. The snapshot is written after every accepted post and read once at
//! startup, which survives an ordinary restart and a deploy.
//!
//! WHAT IT IS NOT. It is not a database and this is not durability. The write
//! is atomic against a crash mid-write (temporary file on the same filesystem,
//! then rename), but posts accepted between a write and a power loss are gone,
//! there is no replication, and two boards pointed at one file will overwrite
//! each other. That is stated in the README as a limit rather than smoothed
//! over, because the losses are bounded and harmless: losing this file loses
//! AVAILABILITY of some offers and nothing else. No key, no custody, and no
//! authority lives here — every offer is a bearer-signed artifact its maker
//! still holds and can post again.
//!
//! THE FILE IS NOT TRUSTED. Every row is re-admitted through the shared ticket
//! reader on load, signature included, so a hand-edited snapshot cannot inject
//! an offer that reader would refuse. A relay that believed its own disk would
//! be a relay that could be made to forge by anyone who could write to it.

use std::{
    fs,
    io::Write as _,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::board::BoardStateV1;

/// The `schema` every snapshot declares, and the only one the reader accepts.
pub const SNAPSHOT_SCHEMA_V1: &str = "dclutch/ticket-board-snapshot/v1";

/// One stored offer, as the file carries it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SnapshotOfferV1 {
    /// SHA-256 of `text`, lowercase hex. Recorded for a human reading the file;
    /// the loader recomputes it and does not take this on faith.
    pub digest: String,
    /// The exact ticket bytes, verbatim.
    pub text: String,
    /// The slot the poster asserted, if any.
    pub posted_at_slot: Option<u64>,
}

/// The file's whole shape.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SnapshotV1 {
    /// Always [`SNAPSHOT_SCHEMA_V1`].
    pub schema: String,
    /// Offers in arrival order.
    pub offers: Vec<SnapshotOfferV1>,
}

/// What loading one snapshot amounted to.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SnapshotLoadV1 {
    /// Offers restored into the board.
    pub restored: usize,
    /// Rows the ticket reader refused, with its sentence for each.
    ///
    /// A refused row is reported and skipped rather than fatal: one bad line
    /// must not stop a board from serving the offers that are fine.
    pub refused: Vec<String>,
}

/// Read a snapshot into a board, re-validating every row.
///
/// A missing file is not an error — it is the first run.
pub fn load_snapshot_v1(path: &Path, board: &mut BoardStateV1) -> Result<SnapshotLoadV1, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(SnapshotLoadV1::default());
        }
        Err(error) => {
            return Err(format!(
                "could not read the snapshot at {}: {error}",
                path.display()
            ));
        }
    };
    let snapshot: SnapshotV1 = serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "the snapshot at {} is not one {SNAPSHOT_SCHEMA_V1}: {error}",
            path.display()
        )
    })?;
    if snapshot.schema != SNAPSHOT_SCHEMA_V1 {
        return Err(format!(
            "the snapshot at {} declares schema {} and this board reads only {SNAPSHOT_SCHEMA_V1}",
            path.display(),
            snapshot.schema
        ));
    }

    let mut load = SnapshotLoadV1::default();
    for offer in snapshot.offers {
        match board.restore_v1(offer.text.as_bytes(), offer.posted_at_slot) {
            Ok(_) => load.restored = load.restored.saturating_add(1),
            Err(refusal) => {
                load.refused
                    .push(format!("{}: {}", refusal.name(), refusal.sentence()))
            }
        }
    }
    Ok(load)
}

/// Write the board to its snapshot, atomically.
///
/// Emit to a temporary file on the SAME filesystem, flush and sync it, then
/// rename over the canonical path. A failed write leaves the last accepted
/// snapshot byte-for-byte intact, which is the only behaviour that makes the
/// file worth reading at startup.
pub fn write_snapshot_v1(path: &Path, board: &BoardStateV1) -> Result<(), String> {
    let snapshot = SnapshotV1 {
        schema: SNAPSHOT_SCHEMA_V1.into(),
        offers: board
            .entries_in_arrival_order()
            .into_iter()
            .map(|entry| SnapshotOfferV1 {
                digest: entry.digest.clone(),
                text: entry.text.clone(),
                posted_at_slot: entry.posted_at_slot,
            })
            .collect(),
    };
    let text = serde_json::to_string_pretty(&snapshot)
        .map_err(|error| format!("could not encode the snapshot: {error}"))?;

    let temporary = temporary_path_v1(path);
    // `create` and not `create_new`: a temporary left by a killed process must
    // not wedge every later write.
    let mut file = fs::File::create(&temporary)
        .map_err(|error| format!("could not open {}: {error}", temporary.display()))?;
    file.write_all(text.as_bytes())
        .and_then(|()| file.write_all(b"\n"))
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("could not write {}: {error}", temporary.display()))?;
    drop(file);
    fs::rename(&temporary, path).map_err(|error| {
        format!(
            "could not replace {} with {}: {error}",
            path.display(),
            temporary.display()
        )
    })
}

/// The temporary path a snapshot is staged through: a sibling, so the rename is
/// on one filesystem and therefore atomic.
fn temporary_path_v1(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".writing");
    path.with_file_name(name)
}
