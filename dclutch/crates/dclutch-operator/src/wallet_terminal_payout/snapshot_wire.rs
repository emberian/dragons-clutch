//! The JSON wire a caller hands a compiled derivation its observations on.
//!
//! ONE DECODER, TWO STAGES. Stage two (`dclutch-wallet-terminal-payout-wasm`)
//! reads one round of thirty-six accounts; stage one
//! (`dclutch-wallet-terminal-input-wasm`) reads two rounds of its own. The
//! bytes-to-[`FinalizedSnapshotV1`] mapping is the same in both, including the
//! cross-check that catches the one corruption a snapshot can suffer that still
//! decodes cleanly and still authenticates — against the wrong account. A
//! second copy of that check is exactly the drift these extractions exist to
//! prevent, so the boundaries name their own format and share this.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Deserialize;
use solana_program::pubkey::Pubkey;

use crate::wallet_terminal_payout::{
    Error, ObservedAccountValueV1, Result, pubkey, wire::FinalizedSnapshotV1,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AccountWireV1 {
    /// The address this observation is OF. Redundant with the `keys` list on
    /// purpose: checking the two against each other catches a transport that
    /// reordered or mispaired them.
    key: String,
    owner: String,
    lamports: String,
    executable: bool,
    data_base64: String,
}

/// One finalized observation of every address a derivation asked for.
///
/// `deny_unknown_fields` is the load-bearing half: a snapshot carrying a
/// coordinate the boundary does not forward must fail loudly rather than be
/// planned around.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SnapshotWireV1 {
    format: String,
    slot: String,
    unix_timestamp: String,
    /// Absent entries are carried as vacant; the derivation decides which of
    /// the frame may be empty, and the wire does not.
    accounts: Vec<Option<AccountWireV1>>,
    /// The addresses, in the exact order the derivation asked for them.
    keys: Vec<String>,
}

fn key(value: &str, field: &str) -> Result<Pubkey> {
    pubkey(value).map_err(|_| Error::new(format!("{field} is not a base58 public key")))
}

/// Decode one observed snapshot, and refuse everything that is not exactly one.
///
/// The caller names the format it accepts, so two boundaries over the same
/// derivation cannot be handed each other's artifact.
pub fn parse_observed_snapshot_v1(
    snapshot_json: &str,
    expected_format: &str,
) -> Result<FinalizedSnapshotV1> {
    let wire: SnapshotWireV1 = serde_json::from_str(snapshot_json).map_err(|error| {
        Error::new(format!(
            "payout snapshot is not the exact accepted JSON: {error}"
        ))
    })?;
    if wire.format != expected_format {
        return Err(Error::new(format!(
            "payout snapshot format must be {expected_format}"
        )));
    }
    if wire.keys.len() != wire.accounts.len() {
        return Err(Error::new(format!(
            "payout snapshot has {} keys and {} observations",
            wire.keys.len(),
            wire.accounts.len()
        )));
    }
    let keys = wire
        .keys
        .iter()
        .map(|value| key(value, "snapshot address"))
        .collect::<Result<Vec<_>>>()?;
    let values = wire
        .accounts
        .iter()
        .zip(keys.iter())
        .map(|(entry, expected)| match entry {
            None => Ok(None),
            Some(account) => {
                if key(&account.key, "observed address")? != *expected {
                    return Err(Error::new(format!(
                        "payout snapshot pairs an observation of {} with the slot for {expected}",
                        account.key
                    )));
                }
                Ok(Some(ObservedAccountValueV1 {
                    owner: key(&account.owner, "observed owner")?,
                    lamports: account
                        .lamports
                        .parse()
                        .map_err(|_| Error::new("observed lamports is not a u64"))?,
                    executable: account.executable,
                    data: STANDARD
                        .decode(&account.data_base64)
                        .map_err(|_| Error::new("observed data is not canonical base64"))?,
                }))
            }
        })
        .collect::<Result<Vec<_>>>()?;

    FinalizedSnapshotV1::from_observed(
        wire.slot
            .parse()
            .map_err(|_| Error::new("snapshot slot is not a u64"))?,
        wire.unix_timestamp
            .parse()
            .map_err(|_| Error::new("snapshot unix timestamp is not an i64"))?,
        &keys,
        values,
    )
}
