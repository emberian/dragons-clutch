//! The **published** publication log, in sealed segments.
//!
//! `publog.rs` owns the daemon's local append-only file: one line per signed
//! message, never rewritten.  That file is the source of truth and this module
//! does not change it.  What this module owns is the *served* shape of the same
//! history, and the reason it exists is that a flat file is a shape a reader
//! learns and then cannot be taken away from them.
//!
//! The published surface therefore has three properties, in this order of
//! importance:
//!
//! 1. **Nothing published ever changes.**  A segment is sealed by being renamed
//!    to a number it keeps forever, and no byte in it is touched again.  The one
//!    file that changes is the active segment, and even it only grows.
//! 2. **Any prefix verifies without refetching the rest.**  Segment *n+1*'s
//!    first line is a header carrying segment *n*'s SHA-256, so the segments
//!    form a hash chain.  A single value — the chain head — commits to every
//!    sealed byte in order, and it is published in `LATEST.json`, so a reader
//!    who has already checked history up to segment *k* needs one small request
//!    to learn whether anything before *k* was disturbed.
//! 3. **Liveness costs one small request.**  `LATEST.json` stays a few hundred
//!    bytes no matter how large the history grows.
//!
//! The daemon-side consequence matters too: the old push read the entire log
//! into memory and wrote the entire log back out, every cycle.  At the armed
//! cadence that reaches hundreds of megabytes inside a unit with
//! `MemoryMax=256M`.  Publishing here is O(new bytes): the tail is read from an
//! offset the served directory itself records, and the work per cycle is bounded
//! by [`MAX_PUBLISH_TAIL_BYTES`] no matter how far behind the publisher is.
//!
//! # The migration, and why the old digest claim stays true
//!
//! The first deployment published a single flat `publication_log.jsonl` and
//! `LATEST.json` claimed its length and SHA-256.  The claim attached to that
//! digest was always a **prefix** claim — "the new log still begins with the
//! bytes it already read" — never an equality claim.  So:
//!
//! * While segment 1 is the active segment the flat file is maintained
//!   byte-for-byte alongside it, and it *is* the whole log, exactly as before.
//! * When segment 1 seals, the flat file is left holding those same bytes at the
//!   same offsets forever, and receives exactly one more line: a continuation
//!   record naming the segment it became and the index that continues it.
//!
//! Every byte ever served at that path is still there, at the same offset, with
//! the same value; the prefix claim holds; and a reader who only knows the old
//! name learns where the log went **in band**, in the format they were already
//! parsing, rather than by silently reading a file that stopped growing.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::derive::sha256;
use crate::error::{RelayerError, Result};
use crate::id32::{ID_BYTES, to_hex};
use crate::publog::wall_unix_seconds;

/// The daemon's local append-only log, under `output_dir`.
pub const LOCAL_LOG_FILE: &str = "publication_log.jsonl";

/// The active segment in the served directory.
///
/// This is the **only** path under a served directory whose contents change
/// meaning over time: at a seal it is renamed away and a new, nearly empty file
/// takes the name.  A reader that wants to poll it must read `LATEST.json`
/// first and check `current_segment`; `README.txt` says so in the served
/// directory itself.
pub const CURRENT_SEGMENT_FILE: &str = "publication_log.current.jsonl";

/// The flat log's historical name, kept for readers of the first deployment.
pub const LEGACY_FLAT_LOG_FILE: &str = "publication_log.jsonl";

/// The segment index, rewritten only when a segment seals.
pub const INDEX_FILE: &str = "segments.json";

/// The liveness file: small, and small forever.
pub const LATEST_FILE: &str = "LATEST.json";

/// The verifier's instructions, served next to the data they describe.
pub const README_FILE: &str = "README.txt";

/// The size at which the active segment is sealed.
///
/// **4 MiB, chosen against the measured line size.**  A published attestation
/// line is 1,361 bytes and a seal line is 1,052; a segment therefore holds
/// roughly 3,400 records.  Three things fixed the number:
///
/// * **The index is the file every cold reader must fetch in full**, and it is
///   the only published thing that grows without bound.  At the armed cadence
///   (five records a window, a window every few minutes) 4 MiB segments seal
///   about every two days, so `segments.json` gains ~300 bytes every two days —
///   tens of kilobytes a year, which stays a small request for the life of the
///   deployment.  A 1 MiB threshold would quadruple that for no reader benefit,
///   because a warm reader never fetches whole segments anyway.
/// * **A segment is the unit you must fetch whole to check one link of the
///   chain.**  4 MiB is about a second on an ordinary link and a few
///   milliseconds to hash, and it fits inside the unit's `MemoryMax=256M` with
///   three orders of magnitude to spare, so no part of publishing or verifying
///   needs streaming machinery.
/// * **It is a power of two**, so "did this cross the threshold?" is exact
///   arithmetic and the number is quotable in the served README.
///
/// At the current disarmed cadence (two records every fifteen minutes) this
/// seals about every seventeen days, which is why the seal path is exercised by
/// `--segment-bytes` in a rehearsal directory rather than by waiting.
pub const DEFAULT_SEGMENT_BYTES: u64 = 4 * 1024 * 1024;

/// The most local-log tail one publish will move.
///
/// Publishing is bounded work.  A publisher that has fallen a long way behind —
/// a served directory restored from a backup, a rehearsal replaying a large log
/// — catches up over several cycles instead of reading an unbounded amount into
/// a 256 MB unit in one go.
pub const MAX_PUBLISH_TAIL_BYTES: u64 = 64 * 1024 * 1024;

/// The domain the segment chain folds from.
///
/// Domain-separated so a chain value can never be confused with any other
/// SHA-256 this daemon publishes — an account tail digest, a set digest, a
/// segment's own digest.
pub const CHAIN_DOMAIN: &[u8] = b"dclutch/relayer/publication-log/segment-chain/v1";

/// The schema every line the daemon signs carries.
pub const PUBLICATION_SCHEMA: &str = "dclutch.relayer.publication.v1";

/// The schema of the first line of every segment after the first.
pub const SEGMENT_HEADER_SCHEMA: &str = "dclutch.relayer.segment-header.v1";

/// The schema of the single line appended to the flat log when it is retired.
pub const CONTINUATION_SCHEMA: &str = "dclutch.relayer.publication-log-continued.v1";

/// The schema of [`INDEX_FILE`].
pub const INDEX_SCHEMA: &str = "dclutch.relayer.publication-log-index.v1";

/// The schema of [`LATEST_FILE`].
pub const LATEST_SCHEMA: &str = "dclutch.relayer.publication-push.v2";

/// The name a sealed segment carries forever.
///
/// Zero-padded so a directory listing and a lexicographic sort agree with the
/// chain order for the first 99,999 segments — which at 4 MiB apiece is four
/// hundred gigabytes of history.
pub fn segment_file_name(segment: u32) -> String {
    format!("publication_log.{segment:05}.jsonl")
}

/// The chain's starting value: `sha256(CHAIN_DOMAIN)`.
pub fn chain_genesis() -> [u8; ID_BYTES] {
    sha256(CHAIN_DOMAIN)
}

/// Fold one sealed segment's digest into the chain.
///
/// `chain(n) = sha256(chain(n-1) || sha256(segment_n_bytes))`, where the segment
/// digest covers the file exactly as served, header line included — so a reader
/// hashes what it downloaded and compares, with no parsing in between.
pub fn chain_fold(previous: &[u8; ID_BYTES], segment_digest: &[u8; ID_BYTES]) -> [u8; ID_BYTES] {
    let mut preimage = [0u8; ID_BYTES * 2];
    let (head, tail) = preimage.split_at_mut(ID_BYTES);
    head.copy_from_slice(previous);
    tail.copy_from_slice(segment_digest);
    sha256(&preimage)
}

/// One sealed segment, as the index records it.
///
/// Written once, when the segment seals, and never rewritten.  Publishing
/// refuses to alter an entry that already exists, which is the executable form
/// of "sealed means sealed".
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SegmentEntry {
    /// One-based segment number.
    pub segment: u32,
    /// The file name, which is derived from `segment` and never changes.
    pub name: String,
    /// SHA-256 over the whole file as served, header line included.
    pub sha256_hex: String,
    /// The file's length in bytes, header line included.
    pub bytes: u64,
    /// Bytes occupied by the leading header line; zero for segment 1.
    pub header_bytes: u64,
    /// Bytes of publication records: `bytes - header_bytes`.
    pub record_bytes: u64,
    /// How many publication records the segment carries.
    pub records: u64,
    /// The observed mainnet slot of the segment's first record.
    pub first_slot: u64,
    /// The observed mainnet slot of the segment's last record.
    pub last_slot: u64,
    /// The chain value after folding this segment in.
    pub chain_sha256_hex: String,
    /// When the segment was sealed, by the publisher's wall clock.
    pub sealed_at_wall_unix_seconds: u64,
}

/// The whole index: every sealed segment, in order, plus the chain head.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SegmentIndex {
    /// Always [`INDEX_SCHEMA`].
    pub schema: String,
    /// The domain string the chain folds from, so the fold is reproducible from
    /// this file alone.
    pub chain_domain: String,
    /// `sha256(chain_domain)`, the chain's value before any segment.
    pub chain_genesis_sha256_hex: String,
    /// The seal threshold in force when this index was last written.  Recorded
    /// for the reader's benefit; changing it never invalidates anything sealed.
    pub segment_bytes_target: u64,
    /// The chain value after every sealed segment.
    pub chain_head_sha256_hex: String,
    /// Every sealed segment, ascending, contiguous from 1.
    pub segments: Vec<SegmentEntry>,
    /// When the index was last written.
    pub updated_wall_unix_seconds: u64,
}

impl SegmentIndex {
    /// An index with no sealed segments.
    pub fn empty(segment_bytes_target: u64) -> Self {
        Self {
            schema: INDEX_SCHEMA.to_owned(),
            chain_domain: String::from_utf8_lossy(CHAIN_DOMAIN).into_owned(),
            chain_genesis_sha256_hex: to_hex(&chain_genesis()),
            segment_bytes_target,
            chain_head_sha256_hex: to_hex(&chain_genesis()),
            segments: Vec::new(),
            updated_wall_unix_seconds: wall_unix_seconds(),
        }
    }

    /// How many segments are sealed.
    pub fn sealed(&self) -> u32 {
        u32::try_from(self.segments.len()).unwrap_or(u32::MAX)
    }

    /// Total bytes of publication records across every sealed segment.
    ///
    /// This is the offset into the local log at which the active segment starts,
    /// which is what makes an incremental publish possible at all.
    pub fn sealed_record_bytes(&self) -> u64 {
        self.segments
            .iter()
            .map(|entry| entry.record_bytes)
            .fold(0u64, u64::saturating_add)
    }

    /// Total publication records across every sealed segment.
    pub fn sealed_records(&self) -> u64 {
        self.segments
            .iter()
            .map(|entry| entry.records)
            .fold(0u64, u64::saturating_add)
    }

    /// Recompute the chain from the recorded per-segment digests.
    ///
    /// Catches an index whose head was edited without the segments, and an index
    /// whose segments were reordered.
    pub fn recompute_chain(&self) -> Result<[u8; ID_BYTES]> {
        let mut chain = chain_genesis();
        for (position, entry) in self.segments.iter().enumerate() {
            let expected = u32::try_from(position)
                .ok()
                .and_then(|index| index.checked_add(1))
                .ok_or_else(|| index_refusal("more segments than a u32 can number"))?;
            if entry.segment != expected {
                return Err(index_refusal(format!(
                    "segment numbering is not contiguous: entry {position} says segment {}, \
                     expected {expected}",
                    entry.segment
                )));
            }
            if entry.name != segment_file_name(entry.segment) {
                return Err(index_refusal(format!(
                    "segment {} is named {:?}, but a segment's name is derived from its number \
                     and must be {:?}",
                    entry.segment,
                    entry.name,
                    segment_file_name(entry.segment)
                )));
            }
            let digest = parse_digest(&entry.sha256_hex, "segment sha256")?;
            chain = chain_fold(&chain, &digest);
            if to_hex(&chain) != entry.chain_sha256_hex {
                return Err(index_refusal(format!(
                    "segment {}'s recorded chain value does not follow from the segments before \
                     it: the index says {}, folding gives {}",
                    entry.segment,
                    entry.chain_sha256_hex,
                    to_hex(&chain)
                )));
            }
        }
        if to_hex(&chain) != self.chain_head_sha256_hex {
            return Err(index_refusal(format!(
                "the index's chain head {} does not follow from its segments (folding gives {})",
                self.chain_head_sha256_hex,
                to_hex(&chain)
            )));
        }
        Ok(chain)
    }
}

/// What one publish did, for the operator's line of output.
#[derive(Clone, Debug)]
pub struct PublishOutcome {
    /// Records appended to the served log this run.
    pub records_appended: u64,
    /// Segments sealed this run, by number.
    pub sealed_this_run: Vec<u32>,
    /// The active segment's number after this run.
    pub current_segment: u32,
    /// The active segment's length in bytes after this run.
    pub current_bytes: u64,
    /// Total publication records now published.
    pub total_records: u64,
    /// Total publication-record bytes now published.
    pub total_record_bytes: u64,
    /// The chain head over every sealed segment.
    pub chain_head_sha256_hex: String,
    /// Local-log bytes not yet published because the per-run cap was reached.
    pub deferred_bytes: u64,
    /// Whether the flat log was retired to a continuation record this run.
    pub retired_flat_log: bool,
}

/// Facts a publication record must carry for the index to describe a segment.
struct RecordFacts {
    observed_slot: u64,
}

/// Records buffered for one append, so a seal never lands mid-batch.
#[derive(Default)]
struct PendingBatch {
    bytes: Vec<u8>,
    records: u64,
    first_slot: Option<u64>,
    last_slot: Option<u64>,
}

impl PendingBatch {
    fn push(&mut self, line: &[u8], observed_slot: u64) {
        self.bytes.extend_from_slice(line);
        self.records = self.records.saturating_add(1);
        if self.first_slot.is_none() {
            self.first_slot = Some(observed_slot);
        }
        self.last_slot = Some(observed_slot);
    }
}

fn index_refusal(reason: impl Into<String>) -> RelayerError {
    RelayerError::config(reason.into())
}

fn parse_digest(hex_text: &str, what: &str) -> Result<[u8; ID_BYTES]> {
    let bytes = hex::decode(hex_text)
        .map_err(|_| index_refusal(format!("{what} {hex_text:?} is not hex")))?;
    <[u8; ID_BYTES]>::try_from(bytes.as_slice())
        .map_err(|_| index_refusal(format!("{what} {hex_text:?} is not 32 bytes")))
}

fn read_file(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|source| RelayerError::io(path, source))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut staging = path.as_os_str().to_owned();
    staging.push(".tmp");
    let staging = PathBuf::from(staging);
    std::fs::write(&staging, bytes).map_err(|source| RelayerError::io(&staging, source))?;
    std::fs::rename(&staging, path).map_err(|source| RelayerError::io(path, source))
}

fn append_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|source| RelayerError::io(path, source))?;
    file.write_all(bytes)
        .map_err(|source| RelayerError::io(path, source))?;
    file.flush()
        .map_err(|source| RelayerError::io(path, source))
}

fn file_len(path: &Path) -> Result<u64> {
    std::fs::metadata(path)
        .map(|meta| meta.len())
        .map_err(|source| RelayerError::io(path, source))
}

/// Read `len` bytes from `offset`, refusing a short read.
fn read_range(path: &Path, offset: u64, len: u64) -> Result<Vec<u8>> {
    let len = usize::try_from(len).map_err(|_| {
        index_refusal(format!(
            "{} is longer than this machine can address",
            path.display()
        ))
    })?;
    let mut file = std::fs::File::open(path).map_err(|source| RelayerError::io(path, source))?;
    file.seek(SeekFrom::Start(offset))
        .map_err(|source| RelayerError::io(path, source))?;
    let mut buffer = vec![0u8; len];
    file.read_exact(&mut buffer)
        .map_err(|source| RelayerError::io(path, source))?;
    Ok(buffer)
}

/// Split a byte run into complete newline-terminated lines, plus any tail that
/// has no newline yet.
///
/// **A partial final line is held back on purpose.**  `publog` opens the log
/// `append` for each line and flushes, so a crash mid-write can leave a
/// truncated last line; publishing it would put bytes into an immutable segment
/// that the daemon is about to finish writing differently.  Holding it back
/// costs one cycle of latency in a case that should never happen, and the
/// alternative costs the one property the segment is for.
fn complete_lines(bytes: &[u8]) -> (Vec<&[u8]>, usize) {
    let mut lines = Vec::new();
    let mut start = 0usize;
    for (offset, byte) in bytes.iter().enumerate() {
        if *byte == b'\n'
            && let Some(end) = offset.checked_add(1)
            && let Some(line) = bytes.get(start..end)
        {
            lines.push(line);
            start = end;
        }
    }
    (lines, bytes.len().saturating_sub(start))
}

/// Read the facts the index needs off one publication record.
fn record_facts(line: &[u8], where_from: &str) -> Result<RecordFacts> {
    let value: serde_json::Value = serde_json::from_slice(line).map_err(|source| {
        index_refusal(format!(
            "a line in {where_from} is not JSON ({source}); the publication log is written by this \
             daemon and a line it cannot read means something else wrote to it"
        ))
    })?;
    let schema = value.get("schema").and_then(serde_json::Value::as_str);
    if schema != Some(PUBLICATION_SCHEMA) {
        return Err(index_refusal(format!(
            "a line in {where_from} carries schema {schema:?}, not {PUBLICATION_SCHEMA:?}"
        )));
    }
    let observed_slot = value
        .get("observed_slot")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            index_refusal(format!(
                "a line in {where_from} has no numeric observed_slot; the slot is what makes an \
                 attestation checkable against the cluster"
            ))
        })?;
    Ok(RecordFacts { observed_slot })
}

/// The state of the active segment on disk.
struct ActiveSegment {
    number: u32,
    path: PathBuf,
    /// Bytes of the leading header line; zero for segment 1.
    header_bytes: u64,
    /// Bytes of publication records in the file.
    record_bytes: u64,
    records: u64,
    first_slot: Option<u64>,
    last_slot: Option<u64>,
}

impl ActiveSegment {
    fn bytes(&self) -> u64 {
        self.header_bytes.saturating_add(self.record_bytes)
    }
}

/// A served publication-log directory, opened for publishing or verification.
pub struct PublishedLog {
    dir: PathBuf,
    index: SegmentIndex,
    current: ActiveSegment,
    /// Present while the historical flat name is still being maintained.
    flat_log_live: bool,
    /// Set by the seal that retired the flat name, read once by `publish`.
    retired_flat_log: bool,
}

impl PublishedLog {
    /// Open a served directory, creating it if it does not exist.
    ///
    /// Recovers the two states a crash mid-seal can leave behind, because the
    /// alternative is an operator repairing a directory whose whole value is
    /// that nobody edited it by hand:
    ///
    /// * the active segment was renamed but the index was not yet replaced — the
    ///   orphaned sealed file is adopted into the index, recomputed from its own
    ///   bytes;
    /// * the index was replaced but the new active segment was not yet created —
    ///   it is created, with the header the index implies.
    pub fn open(dir: &Path, segment_bytes_target: u64) -> Result<Self> {
        std::fs::create_dir_all(dir).map_err(|source| RelayerError::io(dir, source))?;

        let index_path = dir.join(INDEX_FILE);
        let mut index = if index_path.exists() {
            let text = read_file(&index_path)?;
            let index: SegmentIndex = serde_json::from_slice(&text).map_err(|source| {
                index_refusal(format!(
                    "{} is not a segment index: {source}",
                    index_path.display()
                ))
            })?;
            if index.schema != INDEX_SCHEMA {
                return Err(index_refusal(format!(
                    "{} carries schema {:?}, not {INDEX_SCHEMA:?}",
                    index_path.display(),
                    index.schema
                )));
            }
            index.recompute_chain()?;
            index
        } else {
            SegmentIndex::empty(segment_bytes_target)
        };

        let current_path = dir.join(CURRENT_SEGMENT_FILE);

        // Crash recovery, case one: the seal renamed but the index write did not
        // land.  The sealed file is the authority; recompute its entry.
        if !current_path.exists() {
            let orphan_number = index.sealed().saturating_add(1);
            let orphan_path = dir.join(segment_file_name(orphan_number));
            if orphan_path.exists() {
                let entry = derive_entry(&orphan_path, orphan_number, &index)?;
                index.segments.push(entry);
                index.chain_head_sha256_hex = index
                    .segments
                    .last()
                    .map(|entry| entry.chain_sha256_hex.clone())
                    .unwrap_or_else(|| to_hex(&chain_genesis()));
                index.updated_wall_unix_seconds = wall_unix_seconds();
                write_index(dir, &index)?;
            }
        }

        // A reader is told to fetch the index, so the index must be THERE from
        // the first publish rather than 404 until the first seal — an instruction
        // that returns "not found" for the first fortnight is an instruction that
        // teaches readers to skip it.  Written once, here, and thereafter only
        // when a segment seals: an index whose bytes changed every cycle would
        // make every reader revalidate it for nothing.
        if !dir.join(INDEX_FILE).exists() {
            write_index(dir, &index)?;
        }

        let flat_path = dir.join(LEGACY_FLAT_LOG_FILE);
        let mut flat_log_live = false;

        let current = if current_path.exists() {
            let segment_number = index.sealed().saturating_add(1);
            let scanned = scan_segment(&current_path, segment_number, &index)?;
            flat_log_live = segment_number == 1 && flat_path.exists();
            ActiveSegment {
                number: segment_number,
                path: current_path,
                header_bytes: scanned.header_bytes,
                record_bytes: scanned.record_bytes,
                records: scanned.records,
                first_slot: scanned.first_slot,
                last_slot: scanned.last_slot,
            }
        } else if index.sealed() == 0 && flat_path.exists() {
            // MIGRATION.  The first deployment served one flat file; those bytes
            // are already published, so they become segment 1 exactly as they
            // are — not copied into a new numbering with a header prepended,
            // which would change bytes a reader may already have hashed.
            let flat_bytes = read_file(&flat_path)?;
            if flat_is_retired(&flat_bytes) {
                return Err(index_refusal(format!(
                    "{} already carries a continuation record, so it was retired by an earlier \
                     publish — but there is no segment index beside it. Something removed \
                     {INDEX_FILE}; restore it rather than republishing over this directory",
                    flat_path.display()
                )));
            }
            std::fs::copy(&flat_path, &current_path)
                .map_err(|source| RelayerError::io(&current_path, source))?;
            let scanned = scan_segment(&current_path, 1, &index)?;
            flat_log_live = true;
            ActiveSegment {
                number: 1,
                path: current_path,
                header_bytes: scanned.header_bytes,
                record_bytes: scanned.record_bytes,
                records: scanned.records,
                first_slot: scanned.first_slot,
                last_slot: scanned.last_slot,
            }
        } else {
            // Crash recovery, case two — and also the ordinary fresh directory.
            let segment_number = index.sealed().saturating_add(1);
            let header = segment_header_line(&index, segment_number)?;
            let header_bytes = u64::try_from(header.len()).unwrap_or(u64::MAX);
            write_atomic(&current_path, &header)?;
            ActiveSegment {
                number: segment_number,
                path: current_path,
                header_bytes,
                record_bytes: 0,
                records: 0,
                first_slot: None,
                last_slot: None,
            }
        };

        Ok(Self {
            dir: dir.to_path_buf(),
            index,
            current,
            flat_log_live,
            retired_flat_log: false,
        })
    }

    /// Publish everything the local log has that the served directory does not.
    ///
    /// The prefix check that the flat push made by comparing whole files is made
    /// here against the active segment only, and that is not a weakening: the
    /// bytes behind it are pinned by digests taken at seal time and folded into
    /// a chain that the served directory publishes and any reader can recompute.
    /// `verify-log --against` performs the whole-file comparison on demand.
    pub fn publish(
        &mut self,
        local_log: &Path,
        segment_bytes_target: u64,
    ) -> Result<PublishOutcome> {
        let local_len = file_len(local_log)?;
        let sealed_record_bytes = self.index.sealed_record_bytes();
        let published_record_bytes = sealed_record_bytes.saturating_add(self.current.record_bytes);

        if local_len < published_record_bytes {
            return Err(index_refusal(format!(
                "{} is {local_len} bytes but {published_record_bytes} bytes have already been \
                 published from it; a published history is append-only, and a local log shorter \
                 than what was published means one of the two was rewritten. Refusing to publish \
                 — resolve which history is real first",
                local_log.display()
            )));
        }

        // The exact, bounded prefix check: the active segment's records must be
        // byte-identical to the local log's bytes at the same offsets.
        if self.current.record_bytes > 0 {
            let local_slice =
                read_range(local_log, sealed_record_bytes, self.current.record_bytes)?;
            let published = read_file(&self.current.path)?;
            let header = usize::try_from(self.current.header_bytes).unwrap_or(usize::MAX);
            let published_records = published.get(header..).unwrap_or(&[]);
            if published_records != local_slice.as_slice() {
                return Err(index_refusal(format!(
                    "{} is not a byte-copy of the local log at offset {sealed_record_bytes}; a \
                     published history is append-only, and a divergent copy means one of the two \
                     was rewritten. Refusing to overwrite the public copy — resolve which history \
                     is real first",
                    self.current.path.display()
                )));
            }
        }

        // The flat name, while it is still live, must agree byte for byte too.
        if self.flat_log_live {
            let flat_path = self.dir.join(LEGACY_FLAT_LOG_FILE);
            let flat = read_file(&flat_path)?;
            let published = read_file(&self.current.path)?;
            if flat != published {
                return Err(index_refusal(format!(
                    "{} and {} have diverged; while segment 1 is active they are the same bytes \
                     under two names. Refusing to publish",
                    flat_path.display(),
                    self.current.path.display()
                )));
            }
        }

        let available = local_len.saturating_sub(published_record_bytes);
        let take = available.min(MAX_PUBLISH_TAIL_BYTES);
        let tail = if take > 0 {
            read_range(local_log, published_record_bytes, take)?
        } else {
            Vec::new()
        };
        let (lines, partial) = complete_lines(&tail);
        let consumed: u64 = lines
            .iter()
            .map(|line| u64::try_from(line.len()).unwrap_or(u64::MAX))
            .fold(0u64, u64::saturating_add);
        let deferred = available.saturating_sub(consumed);

        let mut sealed_this_run = Vec::new();
        let mut records_appended = 0u64;
        let mut pending = PendingBatch::default();

        for line in &lines {
            let facts = record_facts(line, "the local publication log")?;
            let line_len = u64::try_from(line.len()).unwrap_or(u64::MAX);
            let would_be = self
                .current
                .bytes()
                .saturating_add(u64::try_from(pending.bytes.len()).unwrap_or(u64::MAX))
                .saturating_add(line_len);

            // A segment seals when the NEXT record would take it past the
            // threshold, never merely because it reached the threshold: an
            // active segment that stops growing is not a segment that needs
            // closing.  The consequence is the invariant a reader can rely on —
            // no sealed segment exceeds the threshold, unless one record does.
            let has_records = self.current.records.saturating_add(pending.records) > 0;
            if has_records && would_be > segment_bytes_target {
                self.flush_pending(&mut pending)?;
                let sealed = self.seal_current(segment_bytes_target)?;
                sealed_this_run.push(sealed);
            }

            pending.push(line, facts.observed_slot);
            records_appended = records_appended.saturating_add(1);
        }
        self.flush_pending(&mut pending)?;

        if partial > 0 {
            eprintln!(
                "note: the last {partial} bytes of {} are not a complete line yet and were held \
                 back; a segment only ever receives whole records",
                local_log.display()
            );
        }

        let retired_flat_log = std::mem::take(&mut self.retired_flat_log);

        self.write_latest(segment_bytes_target)?;
        self.write_readme()?;

        Ok(PublishOutcome {
            records_appended,
            sealed_this_run,
            current_segment: self.current.number,
            current_bytes: self.current.bytes(),
            total_records: self
                .index
                .sealed_records()
                .saturating_add(self.current.records),
            total_record_bytes: self
                .index
                .sealed_record_bytes()
                .saturating_add(self.current.record_bytes),
            chain_head_sha256_hex: self.index.chain_head_sha256_hex.clone(),
            deferred_bytes: deferred,
            retired_flat_log,
        })
    }

    /// Append the buffered records to the active segment, and to the flat name
    /// while it is still live.
    fn flush_pending(&mut self, pending: &mut PendingBatch) -> Result<()> {
        if pending.bytes.is_empty() {
            return Ok(());
        }
        append_bytes(&self.current.path, &pending.bytes)?;
        if self.flat_log_live {
            append_bytes(&self.dir.join(LEGACY_FLAT_LOG_FILE), &pending.bytes)?;
        }
        self.current.record_bytes = self
            .current
            .record_bytes
            .saturating_add(u64::try_from(pending.bytes.len()).unwrap_or(u64::MAX));
        self.current.records = self.current.records.saturating_add(pending.records);
        if self.current.first_slot.is_none() {
            self.current.first_slot = pending.first_slot;
        }
        if pending.last_slot.is_some() {
            self.current.last_slot = pending.last_slot;
        }
        *pending = PendingBatch::default();
        Ok(())
    }

    /// Seal the active segment and open the next one.
    ///
    /// Order matters and is chosen so every crash point is recoverable by
    /// [`PublishedLog::open`]: compute the entry, stage the index, rename the
    /// segment (this is the moment it becomes immutable), commit the index,
    /// create the successor.
    fn seal_current(&mut self, segment_bytes_target: u64) -> Result<u32> {
        let number = self.current.number;
        let sealed_name = segment_file_name(number);
        let sealed_path = self.dir.join(&sealed_name);
        if sealed_path.exists() {
            return Err(index_refusal(format!(
                "{} already exists; a sealed segment is immutable and is never written twice",
                sealed_path.display()
            )));
        }

        let bytes = read_file(&self.current.path)?;
        let digest = sha256(&bytes);
        let previous_chain = parse_digest(&self.index.chain_head_sha256_hex, "chain head")?;
        let chain = chain_fold(&previous_chain, &digest);
        let first_slot = self.current.first_slot.ok_or_else(|| {
            index_refusal("refusing to seal a segment that carries no publication record")
        })?;
        let last_slot = self.current.last_slot.unwrap_or(first_slot);

        let entry = SegmentEntry {
            segment: number,
            name: sealed_name,
            sha256_hex: to_hex(&digest),
            bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            header_bytes: self.current.header_bytes,
            record_bytes: self.current.record_bytes,
            records: self.current.records,
            first_slot,
            last_slot,
            chain_sha256_hex: to_hex(&chain),
            sealed_at_wall_unix_seconds: wall_unix_seconds(),
        };

        // "Sealed means sealed", as an executable check rather than a comment:
        // the entries already on disk must survive this write untouched.
        let before = self.index.segments.clone();
        self.index.segments.push(entry);
        if self.index.segments.get(..before.len()) != Some(before.as_slice()) {
            return Err(index_refusal(
                "the segment index changed an entry that was already sealed; refusing to write it",
            ));
        }
        self.index.chain_head_sha256_hex = to_hex(&chain);
        self.index.segment_bytes_target = segment_bytes_target;
        self.index.updated_wall_unix_seconds = wall_unix_seconds();

        std::fs::rename(&self.current.path, &sealed_path)
            .map_err(|source| RelayerError::io(&sealed_path, source))?;

        // The flat name is retired HERE, in the same breath as the rename that
        // sealed the bytes it mirrors — not after the publish loop, which would
        // let the records destined for segment 2 land in it first and leave it
        // holding a segment boundary in the middle of a file.
        if self.retire_flat_log_if_live()? {
            self.retired_flat_log = true;
        }

        write_index(&self.dir, &self.index)?;

        let next = number.checked_add(1).ok_or_else(|| {
            index_refusal("the segment numbering reached u32::MAX; this log needs a new home")
        })?;
        let header = segment_header_line(&self.index, next)?;
        let header_bytes = u64::try_from(header.len()).unwrap_or(u64::MAX);
        write_atomic(&self.current.path, &header)?;
        self.current.number = next;
        self.current.header_bytes = header_bytes;
        self.current.record_bytes = 0;
        self.current.records = 0;
        self.current.first_slot = None;
        self.current.last_slot = None;
        Ok(number)
    }

    /// Retire the historical flat name with one final, self-describing line.
    ///
    /// Called only at the first seal.  Every byte ever served at that path stays
    /// where it was; the file gains a record, in the format its readers were
    /// already parsing, that names the segment it became and the index that
    /// continues it.  A reader who never learned about segments therefore finds
    /// out from the file itself, rather than by quietly reading history that
    /// stopped growing.
    fn retire_flat_log_if_live(&mut self) -> Result<bool> {
        if !self.flat_log_live {
            return Ok(false);
        }
        let flat_path = self.dir.join(LEGACY_FLAT_LOG_FILE);
        let sealed_path = self.dir.join(segment_file_name(1));
        let flat = read_file(&flat_path)?;
        let sealed = read_file(&sealed_path)?;
        if flat != sealed {
            return Err(index_refusal(format!(
                "{} was expected to hold exactly the bytes of {} at the moment segment 1 sealed, \
                 and does not. Refusing to retire it",
                flat_path.display(),
                sealed_path.display()
            )));
        }
        let digest = sha256(&sealed);
        let line = serde_json::json!({
            "schema": CONTINUATION_SCHEMA,
            "note": "This file is complete and will never grow again. It is segment 1 of a \
                     segmented publication log; every line above this one is unchanged. The log \
                     continues in the files named by segments.json.",
            "sealed_as": segment_file_name(1),
            "sealed_bytes": sealed.len(),
            "sealed_sha256_hex": to_hex(&digest),
            "index_file": INDEX_FILE,
            "liveness_file": LATEST_FILE,
            "current_segment_file": CURRENT_SEGMENT_FILE,
            "instructions_file": README_FILE,
            "retired_at_wall_unix_seconds": wall_unix_seconds(),
        });
        let mut text = serde_json::to_string(&line)
            .map_err(|source| RelayerError::Serialization(source.to_string()))?;
        text.push('\n');
        append_bytes(&flat_path, text.as_bytes())?;
        self.flat_log_live = false;
        Ok(true)
    }

    /// Write the liveness file.
    fn write_latest(&self, segment_bytes_target: u64) -> Result<()> {
        let current_bytes = read_file(&self.current.path)?;
        let flat_path = self.dir.join(LEGACY_FLAT_LOG_FILE);
        let legacy = if flat_path.exists() {
            let flat = read_file(&flat_path)?;
            let (state, prefix_bytes, prefix_digest) = if self.flat_log_live {
                let digest = sha256(&flat);
                ("live", flat.len(), to_hex(&digest))
            } else {
                let sealed =
                    self.index.segments.first().ok_or_else(|| {
                        index_refusal("a retired flat log with no sealed segment 1")
                    })?;
                ("sealed", 0usize, sealed.sha256_hex.clone())
            };
            let published_prefix_bytes = if self.flat_log_live {
                u64::try_from(prefix_bytes).unwrap_or(u64::MAX)
            } else {
                self.index
                    .segments
                    .first()
                    .map(|entry| entry.bytes)
                    .unwrap_or(0)
            };
            Some(serde_json::json!({
                "flat_log_file": LEGACY_FLAT_LOG_FILE,
                "state": state,
                "sealed_as": segment_file_name(1),
                // The exact prefix a reader of the first deployment may have
                // hashed: it is still the first bytes of that file, at the same
                // offsets, and it always will be.
                "published_prefix_bytes": published_prefix_bytes,
                "published_prefix_sha256_hex": prefix_digest,
                "bytes": flat.len(),
            }))
        } else {
            None
        };

        let mut latest = serde_json::json!({
            "schema": LATEST_SCHEMA,
            "index_file": INDEX_FILE,
            "instructions_file": README_FILE,
            "current_segment": self.current.number,
            "current_segment_file": CURRENT_SEGMENT_FILE,
            "current_segment_sealed_name": segment_file_name(self.current.number),
            "current_bytes": current_bytes.len(),
            "current_records": self.current.records,
            "current_sha256_hex": to_hex(&sha256(&current_bytes)),
            "segments": self.index.sealed(),
            "chain_head_sha256_hex": self.index.chain_head_sha256_hex,
            "segment_bytes_target": segment_bytes_target,
            // `lines` and `byte_len` keep the exact meaning they had in the flat
            // deployment — total signed messages published, total bytes of those
            // records — so a reader's liveness check is unchanged by all of this.
            "lines": self.index.sealed_records().saturating_add(self.current.records),
            "byte_len": self
                .index
                .sealed_record_bytes()
                .saturating_add(self.current.record_bytes),
            "first_slot": self
                .index
                .segments
                .first()
                .map(|entry| entry.first_slot)
                .or(self.current.first_slot),
            "last_slot": self.current.last_slot.or_else(|| {
                self.index.segments.last().map(|entry| entry.last_slot)
            }),
            "updated_wall_unix_seconds": wall_unix_seconds(),
        });
        if let Some(legacy) = legacy
            && let Some(object) = latest.as_object_mut()
        {
            object.insert("legacy".to_owned(), legacy);
        }
        let mut text = serde_json::to_string_pretty(&latest)
            .map_err(|source| RelayerError::Serialization(source.to_string()))?;
        text.push('\n');
        write_atomic(&self.dir.join(LATEST_FILE), text.as_bytes())
    }

    /// Write the reader's instructions, if they are not already there.
    ///
    /// Content is constant, so this is a no-op after the first publish and the
    /// file's timestamp does not churn.
    fn write_readme(&self) -> Result<()> {
        let path = self.dir.join(README_FILE);
        let wanted = served_readme();
        if path.exists()
            && let Ok(existing) = std::fs::read(&path)
            && existing == wanted.as_bytes()
        {
            return Ok(());
        }
        write_atomic(&path, wanted.as_bytes())
    }

    /// The index, as it now stands.
    pub fn index(&self) -> &SegmentIndex {
        &self.index
    }
}

fn write_index(dir: &Path, index: &SegmentIndex) -> Result<()> {
    let mut text = serde_json::to_string_pretty(index)
        .map_err(|source| RelayerError::Serialization(source.to_string()))?;
    text.push('\n');
    write_atomic(&dir.join(INDEX_FILE), text.as_bytes())
}

/// Build the header line that opens segment `number`.
///
/// Segment 1 has no header, in every deployment: the first bytes of the log are
/// publication records and nothing else, which is what let the first
/// deployment's already-served file become segment 1 unchanged.  The rule a
/// reader is told is exactly that — *a segment carries a header if and only if
/// its number is greater than one.*
fn segment_header_line(index: &SegmentIndex, number: u32) -> Result<Vec<u8>> {
    if number <= 1 {
        return Ok(Vec::new());
    }
    let previous = index.segments.last().ok_or_else(|| {
        index_refusal(format!(
            "segment {number} needs a predecessor and the index has none"
        ))
    })?;
    let header = serde_json::json!({
        "schema": SEGMENT_HEADER_SCHEMA,
        "segment": number,
        "segment_file": segment_file_name(number),
        "prev_segment": previous.segment,
        "prev_segment_file": previous.name,
        "prev_sha256_hex": previous.sha256_hex,
        "prev_bytes": previous.bytes,
        "prev_records": previous.records,
        // The slot span of the segment this header seals.  `first_slot` of the
        // segment being OPENED cannot be known when its header is written — no
        // record exists yet — so what is carried is the span of the closed one,
        // named for what it is, plus the slot the whole chain begins at.
        "prev_first_slot": previous.first_slot,
        "prev_last_slot": previous.last_slot,
        "chain_first_slot": index.segments.first().map(|entry| entry.first_slot),
        "chain_head_sha256_hex": previous.chain_sha256_hex,
        "sealed_at_wall_unix_seconds": previous.sealed_at_wall_unix_seconds,
        "index_file": INDEX_FILE,
    });
    let mut text = serde_json::to_string(&header)
        .map_err(|source| RelayerError::Serialization(source.to_string()))?;
    text.push('\n');
    Ok(text.into_bytes())
}

/// What scanning a segment file establishes about it.
struct ScannedSegment {
    header_bytes: u64,
    record_bytes: u64,
    records: u64,
    first_slot: Option<u64>,
    last_slot: Option<u64>,
    digest: [u8; ID_BYTES],
    bytes: u64,
}

/// Read a segment file and check its shape against the index.
fn scan_segment(path: &Path, number: u32, index: &SegmentIndex) -> Result<ScannedSegment> {
    let bytes = read_file(path)?;
    let digest = sha256(&bytes);
    let (lines, partial) = complete_lines(&bytes);
    if partial > 0 {
        return Err(index_refusal(format!(
            "{} ends in {partial} bytes that are not a complete line; a published segment only \
             ever contains whole records",
            path.display()
        )));
    }

    let mut header_bytes = 0u64;
    let mut record_lines = lines.as_slice();
    if number > 1 {
        let first = lines.first().ok_or_else(|| {
            index_refusal(format!(
                "{} is empty but segment {number} must open with a header",
                path.display()
            ))
        })?;
        let value: serde_json::Value = serde_json::from_slice(first).map_err(|source| {
            index_refusal(format!(
                "{}'s first line is not JSON: {source}",
                path.display()
            ))
        })?;
        if value.get("schema").and_then(serde_json::Value::as_str) != Some(SEGMENT_HEADER_SCHEMA) {
            return Err(index_refusal(format!(
                "{} must open with a {SEGMENT_HEADER_SCHEMA} record and does not",
                path.display()
            )));
        }
        if value.get("segment").and_then(serde_json::Value::as_u64) != Some(u64::from(number)) {
            return Err(index_refusal(format!(
                "{}'s header names a different segment than its position in the chain",
                path.display()
            )));
        }
        if let Some(previous) = index.segments.last() {
            let claimed = value
                .get("prev_sha256_hex")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            if claimed != previous.sha256_hex {
                return Err(index_refusal(format!(
                    "{}'s header claims its predecessor hashes to {claimed}, the index says {}",
                    path.display(),
                    previous.sha256_hex
                )));
            }
        }
        header_bytes = u64::try_from(first.len()).unwrap_or(u64::MAX);
        record_lines = lines.get(1..).unwrap_or(&[]);
    }

    let mut records = 0u64;
    let mut record_bytes = 0u64;
    let mut first_slot = None;
    let mut last_slot = None;
    for line in record_lines {
        let facts = record_facts(line, &path.display().to_string())?;
        if let Some(previous) = last_slot
            && facts.observed_slot < previous
        {
            return Err(index_refusal(format!(
                "{} goes backwards in observed slot ({previous} then {}); the log is written in \
                 observation order",
                path.display(),
                facts.observed_slot
            )));
        }
        if first_slot.is_none() {
            first_slot = Some(facts.observed_slot);
        }
        last_slot = Some(facts.observed_slot);
        records = records.saturating_add(1);
        record_bytes = record_bytes.saturating_add(u64::try_from(line.len()).unwrap_or(u64::MAX));
    }

    Ok(ScannedSegment {
        header_bytes,
        record_bytes,
        records,
        first_slot,
        last_slot,
        digest,
        bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
    })
}

/// Recompute an index entry from a sealed file's own bytes.
fn derive_entry(path: &Path, number: u32, index: &SegmentIndex) -> Result<SegmentEntry> {
    let scanned = scan_segment(path, number, index)?;
    let previous_chain = parse_digest(&index.chain_head_sha256_hex, "chain head")?;
    let chain = chain_fold(&previous_chain, &scanned.digest);
    let first_slot = scanned.first_slot.ok_or_else(|| {
        index_refusal(format!("{} carries no publication record", path.display()))
    })?;
    Ok(SegmentEntry {
        segment: number,
        name: segment_file_name(number),
        sha256_hex: to_hex(&scanned.digest),
        bytes: scanned.bytes,
        header_bytes: scanned.header_bytes,
        record_bytes: scanned.record_bytes,
        records: scanned.records,
        first_slot,
        last_slot: scanned.last_slot.unwrap_or(first_slot),
        chain_sha256_hex: to_hex(&chain),
        sealed_at_wall_unix_seconds: std::fs::metadata(path)
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(|when| when.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or_else(wall_unix_seconds, |elapsed| elapsed.as_secs()),
    })
}

/// Whether a flat log has already received its continuation record.
fn flat_is_retired(bytes: &[u8]) -> bool {
    let (lines, _) = complete_lines(bytes);
    lines.last().is_some_and(|line| {
        serde_json::from_slice::<serde_json::Value>(line)
            .ok()
            .and_then(|value| {
                value
                    .get("schema")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            })
            .as_deref()
            == Some(CONTINUATION_SCHEMA)
    })
}

/// One verified served directory.
#[derive(Clone, Debug)]
pub struct VerifyReport {
    /// Human-readable lines, in the order they were established.
    pub checks: Vec<String>,
    /// Sealed segments.
    pub sealed_segments: u32,
    /// Publication records across the whole log.
    pub records: u64,
    /// Bytes of publication records across the whole log.
    pub record_bytes: u64,
    /// The chain head over every sealed segment.
    pub chain_head_sha256_hex: String,
}

/// Verify a served directory end to end, using nothing but its own contents.
///
/// This is the executable form of the instructions the directory serves, and it
/// is deliberately offline: no config, no network, no key.  Anyone can run it on
/// a directory they downloaded.
///
/// With `against`, it additionally performs the whole-history comparison the
/// incremental publish does not: every published record byte, in order, must be
/// a byte-prefix of the named local log.
pub fn verify_directory(dir: &Path, against: Option<&Path>) -> Result<VerifyReport> {
    let mut checks = Vec::new();

    let index_path = dir.join(INDEX_FILE);
    let index: SegmentIndex = if index_path.exists() {
        let bytes = read_file(&index_path)?;
        serde_json::from_slice(&bytes).map_err(|source| {
            index_refusal(format!(
                "{} is not a segment index: {source}",
                index_path.display()
            ))
        })?
    } else {
        checks.push(format!(
            "{INDEX_FILE}: absent — nothing has sealed yet, so the whole log is the active segment"
        ));
        SegmentIndex::empty(DEFAULT_SEGMENT_BYTES)
    };
    if index.chain_genesis_sha256_hex != to_hex(&chain_genesis()) {
        return Err(index_refusal(format!(
            "the index's chain genesis {} is not sha256 of the published domain {}",
            index.chain_genesis_sha256_hex,
            String::from_utf8_lossy(CHAIN_DOMAIN)
        )));
    }
    let chain_head = index.recompute_chain()?;
    if !index.segments.is_empty() {
        checks.push(format!(
            "{INDEX_FILE}: {} sealed segment(s), numbering contiguous, chain folds to {}",
            index.segments.len(),
            to_hex(&chain_head)
        ));
    }

    // Every sealed segment: bytes hash to what the index recorded, the header
    // names the predecessor the index names, records are in slot order.
    let mut record_bytes = 0u64;
    let mut records = 0u64;
    let mut walked = SegmentIndex::empty(index.segment_bytes_target);
    for entry in &index.segments {
        let path = dir.join(&entry.name);
        let scanned = scan_segment(&path, entry.segment, &walked)?;
        if to_hex(&scanned.digest) != entry.sha256_hex {
            return Err(index_refusal(format!(
                "{} hashes to {}, the index says {}",
                path.display(),
                to_hex(&scanned.digest),
                entry.sha256_hex
            )));
        }
        if scanned.bytes != entry.bytes
            || scanned.record_bytes != entry.record_bytes
            || scanned.records != entry.records
            || scanned.first_slot != Some(entry.first_slot)
            || scanned.last_slot != Some(entry.last_slot)
        {
            return Err(index_refusal(format!(
                "{} does not match the index's description of it (bytes/records/slot span)",
                path.display()
            )));
        }
        record_bytes = record_bytes.saturating_add(entry.record_bytes);
        records = records.saturating_add(entry.records);
        walked.segments.push(entry.clone());
        walked.chain_head_sha256_hex = entry.chain_sha256_hex.clone();
        checks.push(format!(
            "{}: {} bytes, {} record(s), slots {}..{}, sha256 {}",
            entry.name,
            entry.bytes,
            entry.records,
            entry.first_slot,
            entry.last_slot,
            entry.sha256_hex
        ));
    }

    // The active segment.
    let current_number = index.sealed().saturating_add(1);
    let current_path = dir.join(CURRENT_SEGMENT_FILE);
    let current = if current_path.exists() {
        let scanned = scan_segment(&current_path, current_number, &walked)?;
        record_bytes = record_bytes.saturating_add(scanned.record_bytes);
        records = records.saturating_add(scanned.records);
        checks.push(format!(
            "{CURRENT_SEGMENT_FILE}: segment {current_number}, {} bytes, {} record(s), sha256 {}",
            scanned.bytes,
            scanned.records,
            to_hex(&scanned.digest)
        ));
        Some(scanned)
    } else {
        checks.push(format!("{CURRENT_SEGMENT_FILE}: absent"));
        None
    };

    // The liveness file must describe exactly what is on disk.
    let latest_path = dir.join(LATEST_FILE);
    if latest_path.exists() {
        let bytes = read_file(&latest_path)?;
        let latest: serde_json::Value = serde_json::from_slice(&bytes).map_err(|source| {
            index_refusal(format!("{} is not JSON: {source}", latest_path.display()))
        })?;
        let claimed_lines = latest.get("lines").and_then(serde_json::Value::as_u64);
        let claimed_bytes = latest.get("byte_len").and_then(serde_json::Value::as_u64);
        let claimed_head = latest
            .get("chain_head_sha256_hex")
            .and_then(serde_json::Value::as_str);
        if claimed_lines != Some(records) || claimed_bytes != Some(record_bytes) {
            return Err(index_refusal(format!(
                "{} claims {claimed_lines:?} records in {claimed_bytes:?} bytes; the files hold \
                 {records} in {record_bytes}",
                latest_path.display()
            )));
        }
        if claimed_head != Some(index.chain_head_sha256_hex.as_str()) {
            return Err(index_refusal(format!(
                "{} claims chain head {claimed_head:?}; the index's is {}",
                latest_path.display(),
                index.chain_head_sha256_hex
            )));
        }
        if let Some(scanned) = &current {
            let claimed_current = latest
                .get("current_sha256_hex")
                .and_then(serde_json::Value::as_str);
            if claimed_current != Some(to_hex(&scanned.digest).as_str()) {
                return Err(index_refusal(format!(
                    "{} claims the active segment hashes to {claimed_current:?}; it hashes to {}",
                    latest_path.display(),
                    to_hex(&scanned.digest)
                )));
            }
        }
        checks.push(format!(
            "{LATEST_FILE}: agrees with the files — {records} record(s), {record_bytes} bytes, \
             chain head {}",
            index.chain_head_sha256_hex
        ));
    }

    // The historical flat name, if it is still there.
    let flat_path = dir.join(LEGACY_FLAT_LOG_FILE);
    if flat_path.exists() {
        let flat = read_file(&flat_path)?;
        if flat_is_retired(&flat) {
            let sealed = index.segments.first().ok_or_else(|| {
                index_refusal("the flat log is retired but no segment 1 is indexed")
            })?;
            let prefix_len = usize::try_from(sealed.bytes).unwrap_or(usize::MAX);
            let prefix = flat.get(..prefix_len).ok_or_else(|| {
                index_refusal(format!(
                    "{} is shorter than the segment it was retired into",
                    flat_path.display()
                ))
            })?;
            if to_hex(&sha256(prefix)) != sealed.sha256_hex {
                return Err(index_refusal(format!(
                    "{}'s first {prefix_len} bytes are no longer the bytes of {}",
                    flat_path.display(),
                    sealed.name
                )));
            }
            checks.push(format!(
                "{LEGACY_FLAT_LOG_FILE}: retired — its first {prefix_len} bytes are still exactly \
                 {}, and one continuation record names where the log went",
                sealed.name
            ));
        } else {
            let current_bytes = if current.is_some() {
                read_file(&current_path)?
            } else {
                Vec::new()
            };
            if flat != current_bytes {
                return Err(index_refusal(format!(
                    "{} is live but is not byte-identical to {CURRENT_SEGMENT_FILE}",
                    flat_path.display()
                )));
            }
            checks.push(format!(
                "{LEGACY_FLAT_LOG_FILE}: live — byte-identical to the active segment, so it is \
                 still the whole log"
            ));
        }
    }

    // The whole-history comparison, on request.
    if let Some(local) = against {
        let mut offset = 0u64;
        for entry in &index.segments {
            let path = dir.join(&entry.name);
            let bytes = read_file(&path)?;
            let header = usize::try_from(entry.header_bytes).unwrap_or(usize::MAX);
            let published = bytes.get(header..).unwrap_or(&[]);
            let local_slice = read_range(local, offset, entry.record_bytes)?;
            if published != local_slice.as_slice() {
                return Err(index_refusal(format!(
                    "{} is not the local log's bytes at offset {offset}",
                    path.display()
                )));
            }
            offset = offset.saturating_add(entry.record_bytes);
        }
        if let Some(scanned) = &current
            && scanned.record_bytes > 0
        {
            let bytes = read_file(&current_path)?;
            let header = usize::try_from(scanned.header_bytes).unwrap_or(usize::MAX);
            let published = bytes.get(header..).unwrap_or(&[]);
            let local_slice = read_range(local, offset, scanned.record_bytes)?;
            if published != local_slice.as_slice() {
                return Err(index_refusal(format!(
                    "{} is not the local log's bytes at offset {offset}",
                    current_path.display()
                )));
            }
            offset = offset.saturating_add(scanned.record_bytes);
        }
        let local_len = file_len(local)?;
        if local_len < offset {
            return Err(index_refusal(format!(
                "{} is shorter than what has been published from it",
                local.display()
            )));
        }
        checks.push(format!(
            "against {}: every published byte is the local log's byte at the same offset \
             ({offset} of {local_len})",
            local.display()
        ));
    }

    Ok(VerifyReport {
        checks,
        sealed_segments: index.sealed(),
        records,
        record_bytes,
        chain_head_sha256_hex: index.chain_head_sha256_hex,
    })
}

/// The instructions served next to the log.
///
/// Written for someone who found the URL and has no other context: no section
/// numbers, no internal names, no vocabulary from this repository.  Three
/// questions, three answers, each one runnable.
fn served_readme() -> String {
    format!(
        "\
dClutch relay — published observation log
=========================================

Everything in this directory is signed observation history. Each record is one
message this operator signed: the exact signed bytes in hex, the signer, the
signature, and the Solana mainnet slot the observation was taken at. The point
of publishing it is that you do not have to trust it — every record names a slot
and an account, so you can check it against mainnet yourself.

History here is append-only. Nothing that has been published is ever edited or
removed.


The files
---------

  {LATEST}
      Small, and small forever. Read this first.

  {INDEX}
      One entry per finished file: its name, its SHA-256, its byte length, how
      many records it holds and the slots it spans. Entries are added, never
      changed.

  publication_log.NNNNN.jsonl
      A finished file. Once it has this name its bytes never change again.

  {CURRENT}
      The file being written right now. It only grows — until it is finished,
      at which point it is renamed to the next NNNNN and a new, nearly empty
      {CURRENT} takes its place. This is the one name here whose contents change
      meaning over time, so read {LATEST} before you read it (see \"Only the new
      data\" below).

  {README}
      This file.

Every file is JSON Lines: one JSON object per line, newline-terminated.


1. Is the operator alive? — one request
---------------------------------------

    curl -s https://<host>/{LATEST}

    {{
      \"current_segment\": 3,
      \"segments\": 2,
      \"chain_head_sha256_hex\": \"...\",
      \"lines\": 4127,
      \"byte_len\": 5063118,
      \"updated_wall_unix_seconds\": 1787875188
    }}

  updated_wall_unix_seconds   the operator ran. It is refreshed every cycle even
                              when there was nothing to add, so a stale value
                              means the machine or the schedule is down.

  lines                       the operator actually signed something. It counts
                              every record ever published and only goes up. A
                              fresh timestamp with a flat line count means the
                              operator is running and deliberately not attesting
                              — which is a real signal, not a fault to restart
                              through.

  chain_head_sha256_hex       history was not rewritten. One value that commits
                              to every finished file, in order (see 3).

That is the whole liveness check, and its cost does not grow with the log.


2. Only the new data — one small request per poll
-------------------------------------------------

Remember two things between polls: the current segment number, and how many
bytes of it you have read.

    # what you remembered last time
    seg=3 ; have=180224

    read -r now bytes <<<\"$(curl -s https://<host>/{LATEST} \\
        | python3 -c 'import json,sys; d=json.load(sys.stdin); \\
                      print(d[\"current_segment\"], d[\"current_bytes\"])')\"

    if [ \"$now\" = \"$seg\" ]; then
        # same file, just longer: ask only for the bytes past what you have
        curl -s -r \"${{have}}-\" https://<host>/{CURRENT}
    else
        # it was finished while you were away. Finish your copy of the old file
        # under its permanent name, then start on the new one from byte 0.
        curl -s -r \"${{have}}-\" \"https://<host>/publication_log.$(printf %05d $seg).jsonl\"
        curl -s https://<host>/{CURRENT}
    fi

Range requests are supported. Never assume {CURRENT} is the same file it was
last time you looked — that is what the segment number is for.


3. Verify the whole chain
-------------------------

Each finished file after the first begins with one header line naming the file
before it and that file's SHA-256. So the finished files form a chain, and one
value stands for all of them:

    chain(0) = sha256(\"{DOMAIN}\")
    chain(n) = sha256( chain(n-1) || sha256(bytes of finished file n) )

chain(n) for the last finished file is chain_head_sha256_hex in {LATEST} and in
{INDEX}. To check the whole history:

    curl -sO https://<host>/{INDEX}
    python3 - <<'EOF'
    import hashlib, json, urllib.request
    HOST = \"https://<host>\"
    idx = json.load(open(\"{INDEX}\"))
    chain = hashlib.sha256(idx[\"chain_domain\"].encode()).digest()
    for e in idx[\"segments\"]:
        body = urllib.request.urlopen(f\"{{HOST}}/{{e['name']}}\").read()
        d = hashlib.sha256(body).hexdigest()
        assert d == e[\"sha256_hex\"], (e[\"name\"], d)
        chain = hashlib.sha256(chain + bytes.fromhex(d)).digest()
        assert chain.hex() == e[\"chain_sha256_hex\"], e[\"name\"]
    assert chain.hex() == idx[\"chain_head_sha256_hex\"]
    print(\"chain verified:\", chain.hex())
    EOF

Because the chain runs forward from the first file, checking a prefix is enough
to pin that prefix: if you verified up to file 7 yesterday and file 7's chain
value in {INDEX} is unchanged today, nothing at or before file 7 was touched,
and you never have to download those files again.

A finished file's header line is a record like any other; a reader that only
wants signed observations should skip lines whose \"schema\" is not
\"{PUBLICATION}\".


Checking a record against mainnet
---------------------------------

Each observation record carries \"message_hex\" (the exact bytes that were
signed), \"signature_hex\", \"signer_pubkey_base58\" and \"observed_slot\". Verify
the Ed25519 signature over those exact bytes, then fetch the same account at the
same slot from any Solana mainnet RPC and compare. A signature that verifies
over bytes that do not match the chain is a falsifiable claim, publicly on
record — which is the entire reason this directory exists.


A note on the file named publication_log.jsonl
----------------------------------------------

The first deployment published a single flat file under that name. It is still
here and every byte in it is still at the same offset it was always at. When it
was finished it was kept as the first file of the chain, and one final record
was added to it saying so and naming this index. If you have that file, its
bytes are still good; the log simply continues elsewhere.
",
        LATEST = LATEST_FILE,
        INDEX = INDEX_FILE,
        CURRENT = CURRENT_SEGMENT_FILE,
        README = README_FILE,
        DOMAIN = String::from_utf8_lossy(CHAIN_DOMAIN),
        PUBLICATION = PUBLICATION_SCHEMA,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(slot: u64, filler: usize) -> String {
        let value = serde_json::json!({
            "schema": PUBLICATION_SCHEMA,
            "kind": "attestation",
            "observed_slot": slot,
            "message_hex": "ab".repeat(filler),
        });
        format!("{value}\n")
    }

    fn write_local(path: &Path, slots: impl IntoIterator<Item = u64>) {
        let mut text = String::new();
        for slot in slots {
            text.push_str(&record(slot, 8));
        }
        std::fs::write(path, text).expect("write local");
    }

    fn append_local(path: &Path, slots: impl IntoIterator<Item = u64>) {
        let mut text = String::new();
        for slot in slots {
            text.push_str(&record(slot, 8));
        }
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(path)
            .expect("open local");
        file.write_all(text.as_bytes()).expect("append local");
    }

    #[test]
    fn a_fresh_directory_seals_at_the_threshold_and_chains_the_segments() {
        let home = tempfile::tempdir().expect("tempdir");
        let local = home.path().join(LOCAL_LOG_FILE);
        let served = home.path().join("public");
        write_local(&local, 100..112);

        // One record is ~120 bytes here; 300 bytes therefore holds two.
        let mut log = PublishedLog::open(&served, 300).expect("open");
        let outcome = log.publish(&local, 300).expect("publish");
        assert_eq!(outcome.records_appended, 12);
        assert!(outcome.sealed_this_run.len() >= 4);

        let report = verify_directory(&served, Some(&local)).expect("verify");
        assert_eq!(report.records, 12);
        assert_eq!(report.sealed_segments, outcome.current_segment - 1);

        // Segment 1 has no header; every later one opens with one naming its
        // predecessor's digest.
        let first = std::fs::read_to_string(served.join(segment_file_name(1))).expect("seg 1");
        assert!(first.starts_with('{'));
        let value: serde_json::Value =
            serde_json::from_str(first.lines().next().expect("line")).expect("json");
        assert_eq!(value["schema"], PUBLICATION_SCHEMA);

        let second = std::fs::read_to_string(served.join(segment_file_name(2))).expect("seg 2");
        let header: serde_json::Value =
            serde_json::from_str(second.lines().next().expect("line")).expect("json");
        assert_eq!(header["schema"], SEGMENT_HEADER_SCHEMA);
        assert_eq!(header["prev_segment"], 1);
        let digest = sha256(&std::fs::read(served.join(segment_file_name(1))).expect("bytes"));
        assert_eq!(header["prev_sha256_hex"], to_hex(&digest));
    }

    #[test]
    fn sealed_segments_are_byte_identical_across_later_publishes() {
        let home = tempfile::tempdir().expect("tempdir");
        let local = home.path().join(LOCAL_LOG_FILE);
        let served = home.path().join("public");
        write_local(&local, 100..108);

        let mut log = PublishedLog::open(&served, 300).expect("open");
        log.publish(&local, 300).expect("publish");
        let sealed: Vec<(PathBuf, Vec<u8>)> = std::fs::read_dir(&served)
            .expect("dir")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.starts_with("publication_log.0") && name.ends_with(".jsonl")
                    })
            })
            .map(|path| {
                let bytes = std::fs::read(&path).expect("read");
                (path, bytes)
            })
            .collect();
        assert!(!sealed.is_empty());

        append_local(&local, 108..120);
        let mut log = PublishedLog::open(&served, 300).expect("reopen");
        log.publish(&local, 300).expect("publish again");

        for (path, bytes) in &sealed {
            let now = std::fs::read(path).expect("read again");
            assert_eq!(&now, bytes, "{} was rewritten", path.display());
        }
        verify_directory(&served, Some(&local)).expect("verify");
    }

    #[test]
    fn a_rewritten_local_history_is_refused_rather_than_published() {
        let home = tempfile::tempdir().expect("tempdir");
        let local = home.path().join(LOCAL_LOG_FILE);
        let served = home.path().join("public");
        write_local(&local, 100..104);

        let mut log = PublishedLog::open(&served, 1 << 20).expect("open");
        log.publish(&local, 1 << 20).expect("publish");

        // Rewrite the first record in place, keeping the length identical.
        let mut bytes = std::fs::read(&local).expect("read");
        if let Some(slot) = bytes.get_mut(0) {
            *slot = b'{';
        }
        let text = String::from_utf8_lossy(&bytes)
            .replace("\"observed_slot\":100", "\"observed_slot\":999");
        std::fs::write(&local, text).expect("rewrite");

        let mut log = PublishedLog::open(&served, 1 << 20).expect("reopen");
        let refusal = log.publish(&local, 1 << 20).expect_err("must refuse");
        assert!(
            refusal.to_string().contains("append-only"),
            "unexpected refusal: {refusal}"
        );
    }

    #[test]
    fn a_tampered_sealed_segment_fails_verification() {
        let home = tempfile::tempdir().expect("tempdir");
        let local = home.path().join(LOCAL_LOG_FILE);
        let served = home.path().join("public");
        write_local(&local, 100..108);
        let mut log = PublishedLog::open(&served, 300).expect("open");
        log.publish(&local, 300).expect("publish");
        verify_directory(&served, None).expect("verify clean");

        let victim = served.join(segment_file_name(1));
        let text = std::fs::read_to_string(&victim).expect("read");
        std::fs::write(
            &victim,
            text.replace("\"observed_slot\":100", "\"observed_slot\":101"),
        )
        .expect("tamper");
        let refusal = verify_directory(&served, None).expect_err("must refuse");
        assert!(
            refusal.to_string().contains("hashes to"),
            "unexpected refusal: {refusal}"
        );
    }

    #[test]
    fn a_truncated_final_line_is_held_back_until_it_is_complete() {
        let home = tempfile::tempdir().expect("tempdir");
        let local = home.path().join(LOCAL_LOG_FILE);
        let served = home.path().join("public");
        write_local(&local, 100..103);
        std::fs::OpenOptions::new()
            .append(true)
            .open(&local)
            .expect("open")
            .write_all(b"{\"schema\":\"dclutch.relayer.publ")
            .expect("partial");

        let mut log = PublishedLog::open(&served, 1 << 20).expect("open");
        let outcome = log.publish(&local, 1 << 20).expect("publish");
        assert_eq!(outcome.records_appended, 3);
        let published = std::fs::read(served.join(CURRENT_SEGMENT_FILE)).expect("read");
        assert!(published.ends_with(b"\n"));
        assert_eq!(published.iter().filter(|byte| **byte == b'\n').count(), 3);
    }

    #[test]
    fn the_index_a_reader_is_told_to_fetch_exists_before_anything_has_sealed() {
        let home = tempfile::tempdir().expect("tempdir");
        let local = home.path().join(LOCAL_LOG_FILE);
        let served = home.path().join("public");
        write_local(&local, 100..102);
        let mut log = PublishedLog::open(&served, 1 << 20).expect("open");
        let outcome = log.publish(&local, 1 << 20).expect("publish");
        assert!(outcome.sealed_this_run.is_empty());

        let index: SegmentIndex =
            serde_json::from_slice(&std::fs::read(served.join(INDEX_FILE)).expect("index present"))
                .expect("json");
        assert!(index.segments.is_empty());
        assert_eq!(index.chain_head_sha256_hex, to_hex(&chain_genesis()));
        assert!(served.join(README_FILE).exists());
    }

    #[test]
    fn the_flat_log_stays_byte_identical_and_is_retired_with_one_record() {
        let home = tempfile::tempdir().expect("tempdir");
        let local = home.path().join(LOCAL_LOG_FILE);
        let served = home.path().join("public");
        std::fs::create_dir_all(&served).expect("mkdir");

        // The first deployment's state: a flat log, already served.
        write_local(&local, 100..104);
        let original = std::fs::read(&local).expect("read");
        std::fs::write(served.join(LEGACY_FLAT_LOG_FILE), &original).expect("seed flat");
        let original_digest = sha256(&original);

        // First publish under segments: the flat name is still the whole log.
        append_local(&local, 104..106);
        let mut log = PublishedLog::open(&served, 1 << 20).expect("open");
        log.publish(&local, 1 << 20).expect("publish");
        let flat = std::fs::read(served.join(LEGACY_FLAT_LOG_FILE)).expect("flat");
        assert_eq!(
            flat.get(..original.len()),
            Some(original.as_slice()),
            "the already-published bytes moved"
        );
        assert_eq!(
            flat,
            std::fs::read(served.join(CURRENT_SEGMENT_FILE)).expect("current")
        );
        verify_directory(&served, Some(&local)).expect("verify pre-seal");

        // Now force a seal: the flat name freezes with the sealed bytes plus one
        // continuation record, and every earlier byte is where it was.
        append_local(&local, 106..140);
        let mut log = PublishedLog::open(&served, 900).expect("reopen");
        let outcome = log.publish(&local, 900).expect("publish");
        assert!(outcome.retired_flat_log);

        let flat = std::fs::read(served.join(LEGACY_FLAT_LOG_FILE)).expect("flat");
        assert_eq!(flat.get(..original.len()), Some(original.as_slice()));
        assert_eq!(
            to_hex(&sha256(original.as_slice())),
            to_hex(&original_digest)
        );

        let sealed = std::fs::read(served.join(segment_file_name(1))).expect("sealed");
        assert_eq!(flat.get(..sealed.len()), Some(sealed.as_slice()));
        let last = String::from_utf8_lossy(&flat)
            .lines()
            .last()
            .unwrap_or_default()
            .to_owned();
        let value: serde_json::Value = serde_json::from_str(&last).expect("json");
        assert_eq!(value["schema"], CONTINUATION_SCHEMA);
        assert_eq!(value["sealed_sha256_hex"], to_hex(&sha256(&sealed)));

        // A second publish must not touch the retired file again.
        let frozen = std::fs::read(served.join(LEGACY_FLAT_LOG_FILE)).expect("flat");
        append_local(&local, 140..146);
        let mut log = PublishedLog::open(&served, 900).expect("reopen");
        log.publish(&local, 900).expect("publish");
        assert_eq!(
            std::fs::read(served.join(LEGACY_FLAT_LOG_FILE)).expect("flat"),
            frozen
        );
        verify_directory(&served, Some(&local)).expect("verify post-seal");
    }

    #[test]
    fn a_seal_interrupted_before_the_index_landed_is_recovered_on_reopen() {
        let home = tempfile::tempdir().expect("tempdir");
        let local = home.path().join(LOCAL_LOG_FILE);
        let served = home.path().join("public");
        write_local(&local, 100..112);
        let mut log = PublishedLog::open(&served, 300).expect("open");
        log.publish(&local, 300).expect("publish");

        // Simulate the crash window: the rename happened, the index write did
        // not.  Roll the index back one entry and delete the active segment.
        let index_path = served.join(INDEX_FILE);
        let mut index: SegmentIndex =
            serde_json::from_slice(&std::fs::read(&index_path).expect("read")).expect("json");
        index.segments.pop();
        index.chain_head_sha256_hex = index
            .segments
            .last()
            .map(|entry| entry.chain_sha256_hex.clone())
            .unwrap_or_else(|| to_hex(&chain_genesis()));
        std::fs::write(
            &index_path,
            serde_json::to_string_pretty(&index).expect("ser"),
        )
        .expect("write");
        std::fs::remove_file(served.join(CURRENT_SEGMENT_FILE)).expect("remove current");

        let mut log = PublishedLog::open(&served, 300).expect("reopen");
        log.publish(&local, 300).expect("publish");
        verify_directory(&served, Some(&local)).expect("verify after recovery");
    }

    #[test]
    fn the_chain_is_a_domain_separated_fold_a_reader_can_reproduce() {
        let home = tempfile::tempdir().expect("tempdir");
        let local = home.path().join(LOCAL_LOG_FILE);
        let served = home.path().join("public");
        write_local(&local, 100..112);
        let mut log = PublishedLog::open(&served, 300).expect("open");
        log.publish(&local, 300).expect("publish");

        let index: SegmentIndex =
            serde_json::from_slice(&std::fs::read(served.join(INDEX_FILE)).expect("read"))
                .expect("json");
        let mut chain = sha256(index.chain_domain.as_bytes());
        assert_eq!(to_hex(&chain), index.chain_genesis_sha256_hex);
        for entry in &index.segments {
            let bytes = std::fs::read(served.join(&entry.name)).expect("segment");
            assert_eq!(to_hex(&sha256(&bytes)), entry.sha256_hex);
            chain = chain_fold(&chain, &sha256(&bytes));
            assert_eq!(to_hex(&chain), entry.chain_sha256_hex);
        }
        assert_eq!(to_hex(&chain), index.chain_head_sha256_hex);
    }
}
