//! Program-address derivation, out of process and memoized.
//!
//! Off-chain derivation needs the `curve25519` backend, whose proc-macro
//! dependency has no archive in this host's offline crate cache, which is why
//! `clutch_sbf::seeds::find` is `unimplemented!()` off-chain and why the
//! differential harness derives with the pinned `solana` CLI instead.  The
//! keeper does the same, from the **same** `SEED_*` byte constants, so there
//! is still exactly one source of truth for the seed bytes; every derivation
//! is memoized because a keeper re-derives the same handful of addresses on
//! every poll.

use clutch_sbf::seeds;
use clutch_solana_layout::Hash32;
use std::{
    collections::HashMap,
    process::Command,
};

/// One derived program address.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pda {
    /// Base58 address.
    pub address: String,
    /// Raw address bytes.
    pub bytes: [u8; 32],
    /// Canonical bump.
    pub bump: u8,
}

/// A memoizing deriver bound to one program id.
#[derive(Debug)]
pub struct Deriver {
    program_id: String,
    solana_bin: String,
    cache: HashMap<Vec<Vec<u8>>, Pda>,
}

impl Deriver {
    /// Bind to a program id, using `$SOLANA_BIN` or the `solana` on `PATH`.
    #[must_use]
    pub fn new(program_id: &str) -> Self {
        Self {
            program_id: program_id.to_string(),
            solana_bin: std::env::var("SOLANA_BIN").unwrap_or_else(|_| "solana".to_string()),
            cache: HashMap::new(),
        }
    }

    /// Derive one seed tuple.
    ///
    /// # Errors
    /// Returns an error when the CLI is missing or refuses the derivation.
    pub fn find(&mut self, seeds: &[&[u8]]) -> Result<Pda, String> {
        let key: Vec<Vec<u8>> = seeds.iter().map(|seed| seed.to_vec()).collect();
        if let Some(hit) = self.cache.get(&key) {
            return Ok(hit.clone());
        }
        let mut args = vec![
            "find-program-derived-address".to_string(),
            self.program_id.clone(),
        ];
        for seed in seeds {
            args.push(format!("hex:{}", hex(seed)));
        }
        args.push("--output".to_string());
        args.push("json-compact".to_string());
        let output = Command::new(&self.solana_bin)
            .args(&args)
            .output()
            .map_err(|error| format!("could not run {}: {error}", self.solana_bin))?;
        if !output.status.success() {
            return Err(format!(
                "solana find-program-derived-address failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let text = String::from_utf8(output.stdout)
            .map_err(|error| format!("CLI output is not utf8: {error}"))?;
        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|error| format!("CLI output is not json: {error}"))?;
        let address = value
            .get("address")
            .and_then(serde_json::Value::as_str)
            .ok_or("CLI output has no address")?
            .to_string();
        let bump = value
            .get("bumpSeed")
            .and_then(serde_json::Value::as_u64)
            .ok_or("CLI output has no bumpSeed")?;
        let pda = Pda {
            bytes: crate::wire::base58_decode_32(&address)?,
            address,
            bump: u8::try_from(bump).map_err(|_| "bump out of range".to_string())?,
        };
        self.cache.insert(key, pda.clone());
        Ok(pda)
    }

    /// `SEED_EPOCH / market / epoch_index`.
    ///
    /// # Errors
    /// Propagates a derivation failure.
    pub fn epoch(&mut self, market: Hash32, epoch_index: u64) -> Result<Pda, String> {
        self.find(&[seeds::SEED_EPOCH, &market.bytes(), &epoch_index.to_le_bytes()])
    }

    /// `SEED_EPOCH_WINDOW / market / epoch_index`.
    ///
    /// # Errors
    /// Propagates a derivation failure.
    pub fn window(&mut self, market: Hash32, epoch_index: u64) -> Result<Pda, String> {
        self.find(&[
            seeds::SEED_EPOCH_WINDOW,
            &market.bytes(),
            &epoch_index.to_le_bytes(),
        ])
    }

    /// `SEED_PAGE / epoch / page_index`.
    ///
    /// # Errors
    /// Propagates a derivation failure.
    pub fn page(&mut self, epoch: Hash32, page_index: u16) -> Result<Pda, String> {
        self.find(&[seeds::SEED_PAGE, &epoch.bytes(), &page_index.to_le_bytes()])
    }

    /// `SEED_CANDIDATE / epoch / candidate`.
    ///
    /// # Errors
    /// Propagates a derivation failure.
    pub fn candidate(&mut self, epoch: Hash32, candidate: Hash32) -> Result<Pda, String> {
        self.find(&[seeds::SEED_CANDIDATE, &epoch.bytes(), &candidate.bytes()])
    }

    /// `SEED_CANDIDATE_FEED / epoch / candidate`.
    ///
    /// # Errors
    /// Propagates a derivation failure.
    pub fn candidate_feed(&mut self, epoch: Hash32, candidate: Hash32) -> Result<Pda, String> {
        self.find(&[
            seeds::SEED_CANDIDATE_FEED,
            &epoch.bytes(),
            &candidate.bytes(),
        ])
    }

    /// `SEED_CLEAR_WORK / epoch / candidate`.
    ///
    /// # Errors
    /// Propagates a derivation failure.
    pub fn clear_work(&mut self, epoch: Hash32, candidate: Hash32) -> Result<Pda, String> {
        self.find(&[seeds::SEED_CLEAR_WORK, &epoch.bytes(), &candidate.bytes()])
    }

    /// `SEED_POT / epoch`.
    ///
    /// # Errors
    /// Propagates a derivation failure.
    pub fn pot(&mut self, epoch: Hash32) -> Result<Pda, String> {
        self.find(&[seeds::SEED_POT, &epoch.bytes()])
    }

    /// `SEED_RECEIPT / epoch / candidate / slice_index`.
    ///
    /// # Errors
    /// Propagates a derivation failure.
    pub fn receipt(
        &mut self,
        epoch: Hash32,
        candidate: Hash32,
        slice_index: u16,
    ) -> Result<Pda, String> {
        self.find(&[
            seeds::SEED_RECEIPT,
            &epoch.bytes(),
            &candidate.bytes(),
            &slice_index.to_le_bytes(),
        ])
    }

    /// `SEED_RESERVATION / canonical_reservation_id(..)`.
    ///
    /// # Errors
    /// Propagates a derivation failure.
    pub fn reservation(&mut self, reservation_id: Hash32) -> Result<Pda, String> {
        self.find(&[seeds::SEED_RESERVATION, &reservation_id.bytes()])
    }

    /// `SEED_POSITION / market / owner`.
    ///
    /// # Errors
    /// Propagates a derivation failure.
    pub fn position(&mut self, market: Hash32, owner: Hash32) -> Result<Pda, String> {
        self.find(&[seeds::SEED_POSITION, &market.bytes(), &owner.bytes()])
    }

    /// `SEED_GENERAL_FUNDING / target-address-bytes`.
    ///
    /// The funding ledger is keyed by the funded account's own address, so one
    /// machinery account has exactly one recorded funding.
    ///
    /// # Errors
    /// Propagates a derivation failure.
    pub fn funding_ledger(&mut self, target: &[u8; 32]) -> Result<Pda, String> {
        self.find(&[seeds::SEED_GENERAL_FUNDING, target])
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(out, "{byte:02x}").expect("writing hex into a String cannot fail");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_is_lower_case_and_fixed_width() {
        assert_eq!(hex(&[0x00, 0x0f, 0xff]), "000fff");
    }

    #[test]
    fn the_seed_prefixes_come_from_the_program_crate() {
        // Not a copy: a change to the program's seed schema must move these.
        assert_eq!(seeds::SEED_EPOCH, b"dragons-clutch:epoch:v1");
        assert_eq!(seeds::SEED_GENERAL_FUNDING, b"dc:gen-funding:v1");
    }
}
