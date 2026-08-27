//! The machine-readable campaign record, in the committed-walk style.
//!
//! One JSON document per run: every signed transaction with its signature,
//! slot, confirmation level and observed error; every reloaded account with
//! its byte length and SHA-256 digest; the watched-unchanged counts of every
//! asserted refusal; and the devnet-impossible boundary enumeration.  The
//! transcript is written on failure as well as success — a red run is
//! evidence too.

use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// The claim-vocabulary line every transcript leads with.
///
/// Only a run whose genesis hash matched public devnet may claim
/// PUBLIC-TESTNET; anything else (a loopback validator, an unknown cluster)
/// is branded a dry-run rehearsal so the two can never be conflated.
pub fn claim_line(profile: &str, network: &str) -> String {
    if network == "devnet" {
        format!(
            "PUBLIC-TESTNET paces evidence (devnet, profile {profile}): confirmed \
             public-cluster transactions against a deployed ELF. Distinct from local \
             SBF-EXECUTED evidence and from mainnet anything. The funded mock lifecycle is \
             devnet-impossible; see `boundaries` for the exact enumeration of steps replaced \
             by asserted refusals."
        )
    } else {
        format!(
            "DRY-RUN paces rehearsal ({network}, profile {profile}): confirmed transactions \
             against a locally deployed ELF. NOT public-testnet evidence, not local \
             SBF-EXECUTED gate evidence, and not mainnet anything. The funded mock lifecycle \
             is devnet-impossible; see `boundaries` for the exact enumeration of steps \
             replaced by asserted refusals."
        )
    }
}

#[derive(Serialize)]
pub struct ReloadRecord {
    pub role: String,
    pub address: String,
    pub len: usize,
    pub sha256: String,
}

#[derive(Serialize)]
pub struct StepRecord {
    pub ordinal: usize,
    pub name: String,
    /// "accept" | "refuse" | "funding"
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expect_code: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_error: Option<serde_json::Value>,
    /// Funding steps: "airdrop" or "payer-transfer" or "already-funded".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub reloads: Vec<ReloadRecord>,
    /// Refusal steps: how many watched accounts were byte-identical.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watched_unchanged: Option<usize>,
}

#[derive(Serialize)]
pub struct BoundaryRecord {
    pub local_step: String,
    pub status: String,
    pub reason: String,
    pub asserted_instead: String,
}

#[derive(Serialize)]
pub struct Transcript {
    pub claim: String,
    pub profile: String,
    pub url: String,
    pub genesis_hash: String,
    /// "devnet" | "loopback-or-other" (never mainnet: that aborts preflight).
    pub network: String,
    pub program_id: String,
    pub program_owner: String,
    pub start_slot: u64,
    pub degree: u8,
    pub payer: String,
    pub identities: BTreeMap<String, String>,
    pub addresses: BTreeMap<String, String>,
    pub steps: Vec<StepRecord>,
    pub boundaries: Vec<BoundaryRecord>,
    pub outcome: String,
}

impl Transcript {
    pub fn write(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json + "\n")?;
        Ok(())
    }
}

/// Lowercase hex of arbitrary bytes.
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Hex SHA-256 of one account's data bytes.
pub fn sha256_hex(data: &[u8]) -> String {
    hex(&solana_sha256_hasher::hashv(&[data]).to_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_the_known_empty_and_abc_digests() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn the_claim_line_separates_testnet_from_local_and_mainnet() {
        let claim = claim_line("default", "devnet");
        assert!(claim.contains("PUBLIC-TESTNET"));
        assert!(claim.contains("SBF-EXECUTED"));
        assert!(claim.contains("mainnet"));
        assert!(claim.contains("devnet-impossible"));
    }

    #[test]
    fn a_loopback_run_never_claims_public_testnet() {
        let claim = claim_line("mock", "loopback-or-other");
        assert!(claim.contains("DRY-RUN"));
        assert!(claim.contains("NOT public-testnet"));
        assert!(!claim.starts_with("PUBLIC-TESTNET"));
        assert!(claim.contains("devnet-impossible"));
    }

    #[test]
    fn a_transcript_serializes_with_stable_field_names() {
        let transcript = Transcript {
            claim: claim_line("mock", "devnet"),
            profile: "mock".into(),
            url: "https://api.devnet.solana.com".into(),
            genesis_hash: "test".into(),
            network: "devnet".into(),
            program_id: "prog".into(),
            program_owner: "loader".into(),
            start_slot: 5,
            degree: 2,
            payer: "payer".into(),
            identities: BTreeMap::new(),
            addresses: BTreeMap::new(),
            steps: vec![StepRecord {
                ordinal: 1,
                name: "init-source-spec-refused".into(),
                kind: "refuse".into(),
                expect_code: Some(0x007a),
                signature: Some("sig".into()),
                slot: Some(9),
                confirmation: Some("confirmed".into()),
                observed_error: Some(serde_json::json!({
                    "InstructionError": [1, {"Custom": 122}]
                })),
                method: None,
                reloads: vec![],
                watched_unchanged: Some(22),
            }],
            boundaries: vec![],
            outcome: "PASS".into(),
        };
        let json = serde_json::to_string(&transcript).expect("serializes");
        assert!(json.contains("\"expect_code\":122"));
        assert!(json.contains("\"watched_unchanged\":22"));
        assert!(json.contains("\"outcome\":\"PASS\""));
    }
}
