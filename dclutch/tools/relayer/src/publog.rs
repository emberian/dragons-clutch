//! Append-only logs: the publication log, and the RPC read log.
//!
//! **Publication is a requirement, not a nicety** (§4.11).  For every message
//! it signs, the daemon must publish the exact message bytes and the mainnet
//! slot to a public location.  This is the *entire* mitigation for "the relayer
//! can lie" (§4.9): an attestation nobody can check against mainnet is a trust
//! assumption; one that is published is a falsifiable claim.  A relayer profile
//! without publication should not be released.
//!
//! What is implemented here is the local half — an append-only JSONL file whose
//! every line carries the exact signed bytes, the signer, the signature and the
//! observed slot.  **Pushing that file to a public location is not implemented
//! and is a separately authorized act.** Until it is done, this daemon does not
//! satisfy §4.11's publication requirement, and saying otherwise would be the
//! claim the requirement exists to prevent.
//!
//! The RPC read log exists for a different reason: `AGENTS.md` requires public
//! RPC reads to be explicit and bounded, so every call this service makes is
//! recorded with its method, its endpoint host and its outcome.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{RelayerError, Result};
use crate::id32::{ID_BYTES, base58, to_hex};

/// Wall-clock seconds since the Unix epoch, or zero if the clock is before it.
///
/// The daemon's own wall clock is never a fact it signs — the only time that
/// enters a signed message is the attested foreign `Clock` sysvar's bytes
/// (§4.7).  This value is metadata on the log line, so a coarse floor at zero
/// is correct rather than lossy.
pub fn wall_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

/// A file that is only ever appended to.
#[derive(Clone, Debug)]
pub struct AppendOnlyLog {
    path: PathBuf,
}

impl AppendOnlyLog {
    /// Open (creating the parent directory if needed) an append-only log.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(|source| RelayerError::io(parent, source))?;
        }
        Ok(Self { path })
    }

    /// The file this log writes to.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one JSON value as a single line.
    ///
    /// Opened `append` every time rather than held open: a crash mid-run leaves
    /// a truncated *last line* at worst, never a rewritten earlier one, and an
    /// operator can tail or rotate the file without coordinating with a running
    /// process.
    pub fn append(&self, value: &serde_json::Value) -> Result<()> {
        let mut line = serde_json::to_string(value)
            .map_err(|source| RelayerError::Serialization(source.to_string()))?;
        line.push('\n');
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|source| RelayerError::io(&self.path, source))?;
        file.write_all(line.as_bytes())
            .map_err(|source| RelayerError::io(&self.path, source))?;
        file.flush()
            .map_err(|source| RelayerError::io(&self.path, source))
    }
}

/// Which signed message kind a publication line carries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageKind {
    /// `RelayedMainnetAccountAttestationV1`, one signer, one account.
    Attestation,
    /// `RelayedObservationSetSealV1`, one signer, one completed set.
    Seal,
}

impl MessageKind {
    /// The stable string this kind is written as.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Attestation => "attestation",
            Self::Seal => "seal",
        }
    }
}

/// The publication log.
#[derive(Clone, Debug)]
pub struct PublicationLog {
    log: AppendOnlyLog,
}

impl PublicationLog {
    /// Open the publication log under an output directory.
    pub fn open(output_dir: &Path) -> Result<Self> {
        Ok(Self {
            log: AppendOnlyLog::open(output_dir.join("publication_log.jsonl"))?,
        })
    }

    /// The file this log writes to.
    pub fn path(&self) -> &Path {
        self.log.path()
    }

    /// Record one signed message, exactly as signed.
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &self,
        kind: MessageKind,
        account_set_name: &str,
        account_set_id: &[u8; ID_BYTES],
        observed_slot: u64,
        set_index: Option<u16>,
        message: &[u8],
        signer: &[u8; ID_BYTES],
        signature: &[u8; 64],
    ) -> Result<()> {
        self.log.append(&serde_json::json!({
            "schema": "dclutch.relayer.publication.v1",
            "kind": kind.as_str(),
            "account_set_name": account_set_name,
            "account_set_id_hex": to_hex(account_set_id),
            "account_set_id_base58": base58(account_set_id),
            "observed_slot": observed_slot,
            "set_index": set_index,
            "message_len": message.len(),
            "message_hex": to_hex(message),
            "signer_pubkey_base58": base58(signer),
            "signature_hex": to_hex(signature),
            "signature_base58": bs58::encode(signature).into_string(),
            "wall_unix_seconds": wall_unix_seconds(),
        }))
    }
}

/// The RPC read log.
#[derive(Clone, Debug)]
pub struct RpcReadLog {
    log: AppendOnlyLog,
}

impl RpcReadLog {
    /// Open the read log under an output directory.
    pub fn open(output_dir: &Path) -> Result<Self> {
        Ok(Self {
            log: AppendOnlyLog::open(output_dir.join("rpc_reads.jsonl"))?,
        })
    }

    /// The file this log writes to.
    pub fn path(&self) -> &Path {
        self.log.path()
    }

    /// Record one call.  `detail` summarizes the parameters without carrying
    /// the endpoint URL, which may hold an API key.
    pub fn record(
        &self,
        method: &str,
        endpoint_host: &str,
        detail: serde_json::Value,
        outcome: &str,
    ) {
        // A read log that can fail the read it is logging would be worse than
        // no read log; a failure here is reported and the call proceeds.
        let line = serde_json::json!({
            "schema": "dclutch.relayer.rpc-read.v1",
            "method": method,
            "endpoint_host": endpoint_host,
            "detail": detail,
            "outcome": outcome,
            "wall_unix_seconds": wall_unix_seconds(),
        });
        if let Err(error) = self.log.append(&line) {
            eprintln!("warning: could not write the rpc read log: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_publication_log_appends_one_line_per_message_and_never_rewrites() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log = PublicationLog::open(dir.path()).expect("open");
        for index in 0..3u16 {
            log.record(
                MessageKind::Attestation,
                "dbc",
                &[7u8; ID_BYTES],
                423_941_138,
                Some(index),
                &[0xab, 0xcd],
                &[3u8; ID_BYTES],
                &[9u8; 64],
            )
            .expect("record");
        }
        log.record(
            MessageKind::Seal,
            "dbc",
            &[7u8; ID_BYTES],
            423_941_138,
            None,
            &[0x01],
            &[3u8; ID_BYTES],
            &[9u8; 64],
        )
        .expect("record");

        let text = std::fs::read_to_string(log.path()).expect("read");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 4);
        let first: serde_json::Value = serde_json::from_str(lines[0]).expect("json");
        assert_eq!(first["kind"], "attestation");
        assert_eq!(first["message_hex"], "abcd");
        assert_eq!(first["observed_slot"], 423_941_138u64);
        let last: serde_json::Value = serde_json::from_str(lines[3]).expect("json");
        assert_eq!(last["kind"], "seal");
        assert!(last["set_index"].is_null());
    }
}
