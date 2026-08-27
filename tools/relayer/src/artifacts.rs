//! Dry-run artifacts.
//!
//! One directory per set per cycle, holding the raw RPC response, every
//! encoded message, every signature, and a manifest that ties them together.
//! This is the input to a `ProgramTest` campaign, so the format is deliberately
//! boring: flat files with fixed names, a JSON manifest that spells every
//! identifier in both hex and base58, and no format the reader has to already
//! know.
//!
//! ```text
//! <output_dir>/artifacts/<set-name>/slot-<observed_slot>/
//!     manifest.json                     everything below, tied together
//!     account_set_id.hex                the derived pin, one line
//!     observed_slot.txt                 the finalized slot, one line
//!     rpc_get_multiple_accounts.json    the verbatim primary RPC response
//!     rpc_cross_check.<host>.json       verbatim secondary responses, if any
//!     attestation.<index>.bin           exact signed message bytes
//!     attestation.<index>.sig           64 raw signature bytes
//!     seal.bin                          exact 156-byte seal message
//!     seal.sig                          64 raw signature bytes
//! ```
//!
//! A cycle that lands on a slot already written is *not* silently rewritten.
//! Two different observations of one set at one slot is equivocation, and the
//! artifact tree is the place it would first be visible, so a differing rewrite
//! is a refusal and an identical one is a no-op.

use std::path::{Path, PathBuf};

use crate::error::{RelayerError, Result};
use crate::id32::{base58, to_hex};
use crate::observe::{ObservationCycle, TailDigestSource};
use crate::publog::wall_unix_seconds;

/// Writes dry-run artifact directories under one root.
#[derive(Clone, Debug)]
pub struct ArtifactWriter {
    root: PathBuf,
}

impl ArtifactWriter {
    /// Write artifacts under `<output_dir>/artifacts`.
    pub fn new(output_dir: &Path) -> Self {
        Self {
            root: output_dir.join("artifacts"),
        }
    }

    /// The directory one cycle's artifacts land in.
    pub fn cycle_dir(&self, cycle: &ObservationCycle) -> PathBuf {
        self.root
            .join(&cycle.set_name)
            .join(format!("slot-{}", cycle.observed_slot))
    }

    /// Write one cycle, returning the directory it landed in.
    pub fn write_cycle(&self, cycle: &ObservationCycle) -> Result<PathBuf> {
        let dir = self.cycle_dir(cycle);
        let manifest_path = dir.join("manifest.json");
        let manifest = build_manifest(cycle);

        if manifest_path.exists() {
            let existing = std::fs::read_to_string(&manifest_path)
                .map_err(|source| RelayerError::io(&manifest_path, source))?;
            let existing: serde_json::Value = serde_json::from_str(&existing)
                .map_err(|source| RelayerError::Serialization(source.to_string()))?;
            let same_digest = existing.get("set_digest_hex") == manifest.get("set_digest_hex");
            if same_digest {
                return Ok(dir);
            }
            return Err(RelayerError::ObservationRefused {
                set: cycle.set_name.clone(),
                reason: format!(
                    "slot {} already has an artifact with a different set_digest. Two different \
                     observations of one set at one slot is equivocation; refusing to overwrite \
                     the first",
                    cycle.observed_slot
                ),
            });
        }

        std::fs::create_dir_all(&dir).map_err(|source| RelayerError::io(&dir, source))?;
        write_bytes(
            &dir.join("account_set_id.hex"),
            to_hex(&cycle.account_set_id).as_bytes(),
        )?;
        write_bytes(
            &dir.join("observed_slot.txt"),
            cycle.observed_slot.to_string().as_bytes(),
        )?;
        write_json(
            &dir.join("rpc_get_multiple_accounts.json"),
            &cycle.raw_batch,
        )?;
        for (host, raw) in &cycle.cross_check_raw {
            write_json(
                &dir.join(format!("rpc_cross_check.{}.json", sanitize(host))),
                raw,
            )?;
        }
        for position in &cycle.positions {
            write_bytes(
                &dir.join(format!("attestation.{}.bin", position.set_index)),
                &position.message_bytes,
            )?;
            write_bytes(
                &dir.join(format!("attestation.{}.sig", position.set_index)),
                &position.signature,
            )?;
        }
        write_bytes(&dir.join("seal.bin"), &cycle.seal_bytes)?;
        write_bytes(&dir.join("seal.sig"), &cycle.seal_signature)?;
        write_json(&manifest_path, &manifest)?;
        Ok(dir)
    }
}

fn sanitize(host: &str) -> String {
    host.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    std::fs::write(path, bytes).map_err(|source| RelayerError::io(path, source))
}

fn write_json(path: &Path, value: &serde_json::Value) -> Result<()> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|source| RelayerError::Serialization(source.to_string()))?;
    write_bytes(path, text.as_bytes())
}

/// Build one cycle's manifest.
pub fn build_manifest(cycle: &ObservationCycle) -> serde_json::Value {
    let positions: Vec<serde_json::Value> = cycle
        .positions
        .iter()
        .map(|position| {
            let (pages, tail_bytes, cached_slot) = match position.tail_digest_source {
                TailDigestSource::FullyInline => (0u32, 0u64, None),
                TailDigestSource::Paged { pages, bytes } => (pages, bytes, None),
                TailDigestSource::Cached { deployment_slot } => (0, 0, Some(deployment_slot)),
            };
            serde_json::json!({
                "set_index": position.set_index,
                "key_base58": base58(&position.key),
                "key_hex": to_hex(&position.key),
                "observed_owner_base58": base58(&position.owner),
                "lamports": position.lamports,
                "data_len": position.data_len,
                "inline_len": position.inline.len(),
                "executable": position.executable,
                "inline_hex": to_hex(&position.inline),
                "tail_digest_hex": to_hex(&position.tail_digest),
                "tail_digest_source": position.tail_digest_source.as_str(),
                "tail_pages_read": pages,
                "tail_bytes_hashed": tail_bytes,
                "tail_cache_deployment_slot": cached_slot,
                "body_len": position.body_bytes.len(),
                "body_hex": to_hex(&position.body_bytes),
                "message_file": format!("attestation.{}.bin", position.set_index),
                "message_len": position.message_bytes.len(),
                "message_sha256_hex": to_hex(&crate::derive::sha256(&position.message_bytes)),
                "signature_file": format!("attestation.{}.sig", position.set_index),
                "signature_hex": to_hex(&position.signature),
            })
        })
        .collect();

    serde_json::json!({
        "artifact_schema": "dclutch.relayer.dry-run.v1",
        "produced_by": "dclutch-relayer",
        "wall_unix_seconds": wall_unix_seconds(),
        "set_name": cycle.set_name,
        "observed_cluster_id_hex": to_hex(&cycle.observed_cluster_id),
        "observed_cluster_id_base58": base58(&cycle.observed_cluster_id),
        "relay_family_id_hex": to_hex(&cycle.relay_family_id),
        "decoding_rules_id_hex": to_hex(&cycle.decoding_rules_id),
        "account_set_id_hex": to_hex(&cycle.account_set_id),
        "account_set_id_base58": base58(&cycle.account_set_id),
        "observed_slot": cycle.observed_slot,
        "set_count": cycle.set_count,
        "set_digest_hex": to_hex(&cycle.set_digest),
        "attestation_signer_pubkey_base58": base58(&cycle.signer),
        "attestation_signer_pubkey_hex": to_hex(&cycle.signer),
        "positions": positions,
        "seal": {
            "message_file": "seal.bin",
            "message_len": cycle.seal_bytes.len(),
            "message_hex": to_hex(&cycle.seal_bytes),
            "signature_file": "seal.sig",
            "signature_hex": to_hex(&cycle.seal_signature),
        },
        "rpc": {
            "primary_endpoint_host": cycle.primary_endpoint_host,
            "raw_response_file": "rpc_get_multiple_accounts.json",
            "cross_check_endpoint_hosts": cycle
                .cross_check_raw
                .iter()
                .map(|(host, _)| host.clone())
                .collect::<Vec<String>>(),
            "paged_body_reads": cycle.paged_reads,
        },
        "notes": {
            "endpoint_urls_are_not_recorded":
                "only the host is written, because a provider URL commonly carries an API key",
            "publication":
                "these bytes are also appended to publication_log.jsonl; pushing that log to a \
                 public location is a separate, separately authorized act",
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id32::ID_BYTES;
    use crate::observe::ObservedPosition;

    fn cycle(set_digest: [u8; ID_BYTES]) -> ObservationCycle {
        ObservationCycle {
            set_name: "dbc".to_owned(),
            account_set_id: [0x5a; ID_BYTES],
            observed_cluster_id: dclutch_relay_contract::SOLANA_MAINNET_GENESIS_HASH_V1,
            relay_family_id: dclutch_relay_contract::RELAYED_FAMILY_RELEASE_ID_V1,
            decoding_rules_id: [0x11; ID_BYTES],
            observed_slot: 423_941_138,
            set_count: 1,
            set_digest,
            positions: vec![ObservedPosition {
                set_index: 0,
                key: [1; ID_BYTES],
                owner: [2; ID_BYTES],
                lamports: 5,
                data_len: 4,
                inline: vec![1, 2, 3, 4],
                executable: false,
                tail_digest: dclutch_relay_contract::SHA256_EMPTY_DIGEST,
                tail_digest_source: TailDigestSource::FullyInline,
                body_bytes: vec![0xAA; 116],
                message_bytes: vec![0xBB; 272],
                signature: [0xCC; 64],
            }],
            seal_bytes: [0xDD; dclutch_relay_contract::RELAYED_SEAL_BYTES],
            seal_signature: [0xEE; 64],
            signer: [0x99; ID_BYTES],
            raw_batch: serde_json::json!({"context": {"slot": 423_941_138}, "value": []}),
            cross_check_raw: Vec::new(),
            primary_endpoint_host: "127.0.0.1".to_owned(),
            paged_reads: 0,
        }
    }

    #[test]
    fn a_cycle_writes_a_self_describing_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let writer = ArtifactWriter::new(dir.path());
        let written = writer.write_cycle(&cycle([7; ID_BYTES])).expect("write");

        for name in [
            "manifest.json",
            "account_set_id.hex",
            "observed_slot.txt",
            "rpc_get_multiple_accounts.json",
            "attestation.0.bin",
            "attestation.0.sig",
            "seal.bin",
            "seal.sig",
        ] {
            assert!(written.join(name).exists(), "{name} is missing");
        }
        assert_eq!(
            std::fs::read(written.join("attestation.0.sig")).expect("sig"),
            vec![0xCC; 64]
        );
        assert_eq!(
            std::fs::read(written.join("seal.bin")).expect("seal").len(),
            dclutch_relay_contract::RELAYED_SEAL_BYTES
        );
        assert_eq!(
            std::fs::read_to_string(written.join("observed_slot.txt")).expect("slot"),
            "423941138"
        );

        let manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(written.join("manifest.json")).expect("manifest"),
        )
        .expect("json");
        assert_eq!(manifest["artifact_schema"], "dclutch.relayer.dry-run.v1");
        assert_eq!(manifest["observed_slot"], 423_941_138u64);
        assert_eq!(manifest["set_count"], 1);
        assert_eq!(
            manifest["positions"][0]["tail_digest_source"],
            "fully-inline"
        );
        assert_eq!(manifest["positions"][0]["message_len"], 272);
    }

    #[test]
    fn rewriting_a_slot_with_the_same_observation_is_a_no_op() {
        let dir = tempfile::tempdir().expect("tempdir");
        let writer = ArtifactWriter::new(dir.path());
        let first = writer.write_cycle(&cycle([7; ID_BYTES])).expect("write");
        let again = writer.write_cycle(&cycle([7; ID_BYTES])).expect("rewrite");
        assert_eq!(first, again);
    }

    #[test]
    fn rewriting_a_slot_with_a_different_observation_is_refused_as_equivocation() {
        let dir = tempfile::tempdir().expect("tempdir");
        let writer = ArtifactWriter::new(dir.path());
        writer.write_cycle(&cycle([7; ID_BYTES])).expect("write");
        let error = writer
            .write_cycle(&cycle([8; ID_BYTES]))
            .expect_err("equivocation must refuse");
        assert!(
            matches!(error, RelayerError::ObservationRefused { .. }),
            "{error:?}"
        );
    }
}
