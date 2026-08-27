//! The operator's configuration file, hostile-decoded.
//!
//! Two properties matter more than the field list:
//!
//! - **Unknown fields are refused.** A typo'd key would otherwise fall back to
//!   a default, and a silently defaulted `allow_public_submission` or
//!   `inline_len` is exactly the kind of quiet wrong that this family exists to
//!   make impossible.
//! - **`account_set_id` is never written by hand.** It is derived from the
//!   ordered positions (§4.3) and printed so it can be pinned at founding. A
//!   config that could state it would be a second authority for which accounts
//!   may be attested.
//!
//! The daemon holds no market policy, no thresholds and no schedule beyond
//! "which account sets to observe, how often" (§4.11).  Nothing in this file
//! carries a price, a window, a staleness bound, or a comparison: those live in
//! the `decoding_rules_id` record and in `RelayedAdapterConfigV1`, and they are
//! applied by the on-devnet adapter.

use std::path::{Path, PathBuf};
use std::time::Duration;

use dclutch_relay_contract::MAX_RELAYED_INLINE_BYTES_V1;
use dclutch_relay_contract::release::AccountSetEntryV1;
use serde::Deserialize;

use crate::derive::derive_account_set_id;
use crate::error::{RelayerError, Result};
use crate::id32::{ID_BYTES, is_zero, parse_id32};
use crate::keys::{expand_tilde, require_safe_keypair_path};

/// Largest number of watched sets one process will carry.
const MAX_ACCOUNT_SETS: usize = 32;
/// Largest admitted-length list per position.
const MAX_ADMITTED_DATA_LENS: usize = 16;
/// Ceiling on one paged body read, in bytes.
const MAX_BODY_PAGE_BYTES: usize = 8 * 1024 * 1024;

fn default_poll_interval_seconds() -> u64 {
    30
}
fn default_body_page_bytes() -> usize {
    256 * 1024
}
fn default_request_timeout_seconds() -> u64 {
    30
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    output_dir: String,
    #[serde(default = "default_poll_interval_seconds")]
    poll_interval_seconds: u64,
    #[serde(default = "default_body_page_bytes")]
    body_page_bytes: usize,
    observed_cluster: RawObservedCluster,
    keys: RawKeys,
    submit: Option<RawSubmit>,
    #[serde(default)]
    account_sets: Vec<RawAccountSet>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawObservedCluster {
    rpc_endpoints: Vec<String>,
    expected_genesis_hash: String,
    #[serde(default = "default_request_timeout_seconds")]
    request_timeout_seconds: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawKeys {
    attestation_keypair_path: String,
    fee_payer_keypair_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSubmit {
    endpoint: String,
    #[serde(default)]
    allow_public_submission: bool,
    relay_program_id: String,
    market: String,
    generation: u64,
    relayer_key_set: String,
    relayer_key_set_staging_vacancy: String,
    compute_unit_limit: Option<u32>,
    compute_unit_price_micro_lamports: Option<u64>,
    address_lookup_table: Option<RawAddressLookupTable>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAddressLookupTable {
    key: String,
    addresses: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAccountSet {
    name: String,
    relay_family_id: String,
    decoding_rules_id: String,
    positions: Vec<RawPosition>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPosition {
    key: String,
    expected_owner: String,
    inline_len: u16,
    #[serde(default)]
    admitted_data_lens: Vec<u32>,
}

/// One founding-time pinned position of one ordered set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PositionConfig {
    /// The exact observed account address.
    pub key: [u8; ID_BYTES],
    /// The exact owning program the account must report.
    pub expected_owner: [u8; ID_BYTES],
    /// The exact inline prefix width the relayer carries.
    pub inline_len: u16,
    /// The admitted full `data_len` values, or empty for "any width that can
    /// carry the pinned inline prefix".
    ///
    /// §4.11's failure list names "a `data_len` outside the admitted set". The
    /// admitted set is a *decoding-rules* fact that the config echoes so the
    /// daemon can stop early; the on-devnet adapter checks it again from the
    /// pinned record, and that check is the authority.
    pub admitted_data_lens: Vec<u32>,
}

impl PositionConfig {
    /// Whether a full-width read is admitted at this position.
    pub fn admits_data_len(&self, data_len: u32) -> bool {
        if u32::from(self.inline_len) > data_len {
            return false;
        }
        if self.admitted_data_lens.is_empty() {
            return true;
        }
        self.admitted_data_lens.contains(&data_len)
    }
}

/// One watched ordered account set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountSetConfig {
    /// Operator-chosen name, used for artifact paths and diagnostics only.
    pub name: String,
    /// `ProviderReleaseV1.provider_family_id`.
    pub relay_family_id: [u8; ID_BYTES],
    /// `ProviderReleaseV1.decoding_rules_id`.
    pub decoding_rules_id: [u8; ID_BYTES],
    /// The ordered positions.
    pub positions: Vec<PositionConfig>,
    /// Derived, never configured.
    pub account_set_id: [u8; ID_BYTES],
}

impl AccountSetConfig {
    /// The wire-crate view of the ordered positions.
    pub fn entries(&self) -> Vec<AccountSetEntryV1> {
        self.positions
            .iter()
            .map(|position| AccountSetEntryV1 {
                key: position.key,
                expected_owner: position.expected_owner,
                inline_len: position.inline_len,
            })
            .collect()
    }

    /// The set cardinality as the wire carries it.
    pub fn set_count(&self) -> Result<u16> {
        u16::try_from(self.positions.len())
            .map_err(|_| RelayerError::config("account set has more positions than a u16"))
    }

    /// The widest pinned inline prefix in the set.
    ///
    /// One `getMultipleAccounts` call carries one `dataSlice` for the whole
    /// response, so the batch asks for this width and each position is then
    /// truncated to its own pinned `inline_len`.  Asking per position would
    /// mean one call per account, which §4.11 forbids.
    pub fn max_inline_len(&self) -> u16 {
        self.positions
            .iter()
            .map(|position| position.inline_len)
            .max()
            .unwrap_or(0)
    }
}

/// The execution-gated submit surface.
#[derive(Clone, Debug)]
pub struct SubmitConfig {
    /// Where transactions would go.
    pub endpoint: String,
    /// Refuses a non-local endpoint unless explicitly set.
    ///
    /// Defaults to false.  Setting it true is an assertion that a current
    /// authorization names devnet or mainnet submission; nothing in this
    /// service can grant that, and the daemon refuses a public host without it.
    pub allow_public_submission: bool,
    /// The dClutch relay program.
    pub relay_program_id: [u8; ID_BYTES],
    /// The owning Market root.
    pub market: [u8; ID_BYTES],
    /// The Market generation the record lives under.
    pub generation: u64,
    /// The raw immutable `RelayerKeySetV1` record.
    pub relayer_key_set: [u8; ID_BYTES],
    /// The finalized staging vacancy proving that record is immutable.
    pub relayer_key_set_staging_vacancy: [u8; ID_BYTES],
    /// Optional `SetComputeUnitLimit`.
    pub compute_unit_limit: Option<u32>,
    /// Optional `SetComputeUnitPrice`.
    pub compute_unit_price_micro_lamports: Option<u64>,
    /// Optional Market address lookup table.
    pub address_lookup_table: Option<AddressLookupTableConfig>,
}

/// One address lookup table the v0 message compiles against.
#[derive(Clone, Debug)]
pub struct AddressLookupTableConfig {
    /// The table account.
    pub key: [u8; ID_BYTES],
    /// The addresses the table holds, in table order.
    pub addresses: Vec<[u8; ID_BYTES]>,
}

/// The resolved, validated configuration.
#[derive(Clone, Debug)]
pub struct Config {
    /// Where this was loaded from.
    pub source_path: PathBuf,
    /// Root for dry-run artifacts, the publication log and the RPC read log.
    pub output_dir: PathBuf,
    /// Time between observation cycles.
    pub poll_interval: Duration,
    /// One paged body read's width.
    pub body_page_bytes: usize,
    /// Per-request HTTP timeout.
    pub request_timeout: Duration,
    /// Endpoints for the observed cluster; the first is primary and the rest
    /// are cross-checks.
    pub rpc_endpoints: Vec<String>,
    /// The genesis hash the observed cluster must report.
    pub expected_genesis_hash: [u8; ID_BYTES],
    /// The release-identity signing key.
    pub attestation_keypair_path: PathBuf,
    /// The hot, replaceable fee payer.
    pub fee_payer_keypair_path: Option<PathBuf>,
    /// Present only when a `[submit]` table was supplied.
    pub submit: Option<SubmitConfig>,
    /// The watched sets.
    pub account_sets: Vec<AccountSetConfig>,
}

impl Config {
    /// Load and validate a configuration file.
    ///
    /// `home` is the directory the keypair-path safety rules are measured
    /// against; production passes the real home, tests pass a fixture.
    pub fn load(path: &Path, home: Option<&Path>) -> Result<Self> {
        let text =
            std::fs::read_to_string(path).map_err(|source| RelayerError::io(path, source))?;
        Self::from_toml(&text, path, home)
    }

    /// Validate configuration text that has already been read.
    pub fn from_toml(text: &str, source_path: &Path, home: Option<&Path>) -> Result<Self> {
        let raw: RawConfig = toml::from_str(text).map_err(|source| RelayerError::ConfigSyntax {
            path: source_path.to_path_buf(),
            source,
        })?;

        if raw.observed_cluster.rpc_endpoints.is_empty() {
            return Err(RelayerError::config(
                "observed_cluster.rpc_endpoints must name at least one endpoint",
            ));
        }
        for endpoint in &raw.observed_cluster.rpc_endpoints {
            reqwest::Url::parse(endpoint).map_err(|error| {
                RelayerError::config(format!(
                    "observed_cluster.rpc_endpoints entry {endpoint:?} is not a URL: {error}"
                ))
            })?;
        }
        if raw.poll_interval_seconds == 0 {
            return Err(RelayerError::config(
                "poll_interval_seconds must be at least 1; a zero interval is an unbounded read \
                 loop against a public cluster",
            ));
        }
        if raw.body_page_bytes < MAX_RELAYED_INLINE_BYTES_V1
            || raw.body_page_bytes > MAX_BODY_PAGE_BYTES
        {
            return Err(RelayerError::config(format!(
                "body_page_bytes must be between {MAX_RELAYED_INLINE_BYTES_V1} and \
                 {MAX_BODY_PAGE_BYTES}; the lower bound is the release inline ceiling, because \
                 the first page must contain the whole pinned inline prefix so it can be checked \
                 against the batch read"
            )));
        }
        if raw.observed_cluster.request_timeout_seconds == 0 {
            return Err(RelayerError::config(
                "observed_cluster.request_timeout_seconds must be at least 1",
            ));
        }

        let expected_genesis_hash = parse_id32(
            "observed_cluster.expected_genesis_hash",
            &raw.observed_cluster.expected_genesis_hash,
        )?;
        if is_zero(&expected_genesis_hash) {
            return Err(RelayerError::config(
                "observed_cluster.expected_genesis_hash must not be all zero",
            ));
        }

        let attestation_keypair_path =
            expand_tilde(Path::new(&raw.keys.attestation_keypair_path), home);
        require_safe_keypair_path(&attestation_keypair_path, home)?;
        let fee_payer_keypair_path = match raw.keys.fee_payer_keypair_path.as_deref() {
            None => None,
            Some(text) => {
                let path = expand_tilde(Path::new(text), home);
                require_safe_keypair_path(&path, home)?;
                if path == attestation_keypair_path {
                    return Err(RelayerError::config(
                        "keys.fee_payer_keypair_path must differ from \
                         keys.attestation_keypair_path: the fee payer is hot and replaceable, the \
                         attestation key is the release identity (\u{a7}4.11)",
                    ));
                }
                Some(path)
            }
        };

        if raw.account_sets.is_empty() {
            return Err(RelayerError::config(
                "at least one [[account_sets]] entry is required",
            ));
        }
        if raw.account_sets.len() > MAX_ACCOUNT_SETS {
            return Err(RelayerError::config(format!(
                "at most {MAX_ACCOUNT_SETS} watched account sets are admitted"
            )));
        }

        let mut account_sets = Vec::with_capacity(raw.account_sets.len());
        for set in &raw.account_sets {
            account_sets.push(resolve_account_set(set, expected_genesis_hash)?);
        }
        for (index, set) in account_sets.iter().enumerate() {
            if account_sets
                .iter()
                .take(index)
                .any(|earlier| earlier.name == set.name)
            {
                return Err(RelayerError::config(format!(
                    "two account sets share the name {:?}; names index artifact directories and \
                     must be distinct",
                    set.name
                )));
            }
        }

        let submit = match raw.submit {
            None => None,
            Some(submit) => Some(resolve_submit(submit)?),
        };

        Ok(Self {
            source_path: source_path.to_path_buf(),
            output_dir: expand_tilde(Path::new(&raw.output_dir), home),
            poll_interval: Duration::from_secs(raw.poll_interval_seconds),
            body_page_bytes: raw.body_page_bytes,
            request_timeout: Duration::from_secs(raw.observed_cluster.request_timeout_seconds),
            rpc_endpoints: raw.observed_cluster.rpc_endpoints.clone(),
            expected_genesis_hash,
            attestation_keypair_path,
            fee_payer_keypair_path,
            submit,
            account_sets,
        })
    }

    /// The primary endpoint, which the observation is taken from.
    pub fn primary_endpoint(&self) -> &str {
        self.rpc_endpoints.first().map_or("", String::as_str)
    }

    /// The cross-check endpoints, if any.
    pub fn cross_check_endpoints(&self) -> &[String] {
        self.rpc_endpoints.get(1..).unwrap_or(&[])
    }
}

fn resolve_account_set(
    raw: &RawAccountSet,
    observed_cluster_id: [u8; ID_BYTES],
) -> Result<AccountSetConfig> {
    if raw.name.is_empty()
        || !raw
            .name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(RelayerError::config(format!(
            "account set name {:?} must be a non-empty ASCII [A-Za-z0-9_-] string: it becomes a \
             directory name",
            raw.name
        )));
    }

    let relay_family_id = parse_id32("account_sets.relay_family_id", &raw.relay_family_id)?;
    let decoding_rules_id = parse_id32("account_sets.decoding_rules_id", &raw.decoding_rules_id)?;
    if is_zero(&relay_family_id) || is_zero(&decoding_rules_id) {
        return Err(RelayerError::config(format!(
            "account set {:?}: relay_family_id and decoding_rules_id must not be all zero",
            raw.name
        )));
    }

    let mut positions = Vec::with_capacity(raw.positions.len());
    for (index, position) in raw.positions.iter().enumerate() {
        let key = parse_id32("account_sets.positions.key", &position.key)?;
        let expected_owner = parse_id32(
            "account_sets.positions.expected_owner",
            &position.expected_owner,
        )?;
        if is_zero(&key) || is_zero(&expected_owner) {
            return Err(RelayerError::config(format!(
                "account set {:?} position {index}: key and expected_owner must not be all zero",
                raw.name
            )));
        }
        if usize::from(position.inline_len) > MAX_RELAYED_INLINE_BYTES_V1 {
            return Err(RelayerError::config(format!(
                "account set {:?} position {index}: inline_len {} exceeds the release ceiling \
                 {MAX_RELAYED_INLINE_BYTES_V1}",
                raw.name, position.inline_len
            )));
        }
        if position.admitted_data_lens.len() > MAX_ADMITTED_DATA_LENS {
            return Err(RelayerError::config(format!(
                "account set {:?} position {index}: at most {MAX_ADMITTED_DATA_LENS} admitted \
                 data lengths",
                raw.name
            )));
        }
        for admitted in &position.admitted_data_lens {
            if *admitted < u32::from(position.inline_len) {
                return Err(RelayerError::config(format!(
                    "account set {:?} position {index}: admitted data length {admitted} is \
                     narrower than the pinned inline_len {}",
                    raw.name, position.inline_len
                )));
            }
        }
        if raw
            .positions
            .iter()
            .take(index)
            .any(|earlier| earlier.key == position.key)
        {
            return Err(RelayerError::config(format!(
                "account set {:?} position {index}: the same account appears twice; the ordered \
                 set must be alias-free",
                raw.name
            )));
        }
        positions.push(PositionConfig {
            key,
            expected_owner,
            inline_len: position.inline_len,
            admitted_data_lens: position.admitted_data_lens.clone(),
        });
    }

    let entries: Vec<AccountSetEntryV1> = positions
        .iter()
        .map(|position| AccountSetEntryV1 {
            key: position.key,
            expected_owner: position.expected_owner,
            inline_len: position.inline_len,
        })
        .collect();
    // Refuses an empty set and a set past MAX_RELAYED_ACCOUNTS_V1 by
    // construction, so those bounds are not restated here.
    let account_set_id = derive_account_set_id(observed_cluster_id, relay_family_id, &entries)?;

    Ok(AccountSetConfig {
        name: raw.name.clone(),
        relay_family_id,
        decoding_rules_id,
        positions,
        account_set_id,
    })
}

fn resolve_submit(raw: RawSubmit) -> Result<SubmitConfig> {
    reqwest::Url::parse(&raw.endpoint).map_err(|error| {
        RelayerError::config(format!(
            "submit.endpoint {:?} is not a URL: {error}",
            raw.endpoint
        ))
    })?;
    let address_lookup_table = match raw.address_lookup_table {
        None => None,
        Some(table) => {
            let key = parse_id32("submit.address_lookup_table.key", &table.key)?;
            let mut addresses = Vec::with_capacity(table.addresses.len());
            for address in &table.addresses {
                addresses.push(parse_id32(
                    "submit.address_lookup_table.addresses",
                    address,
                )?);
            }
            if addresses.is_empty() {
                return Err(RelayerError::config(
                    "submit.address_lookup_table.addresses must not be empty",
                ));
            }
            Some(AddressLookupTableConfig { key, addresses })
        }
    };
    Ok(SubmitConfig {
        endpoint: raw.endpoint,
        allow_public_submission: raw.allow_public_submission,
        relay_program_id: parse_id32("submit.relay_program_id", &raw.relay_program_id)?,
        market: parse_id32("submit.market", &raw.market)?,
        generation: raw.generation,
        relayer_key_set: parse_id32("submit.relayer_key_set", &raw.relayer_key_set)?,
        relayer_key_set_staging_vacancy: parse_id32(
            "submit.relayer_key_set_staging_vacancy",
            &raw.relayer_key_set_staging_vacancy,
        )?,
        compute_unit_limit: raw.compute_unit_limit,
        compute_unit_price_micro_lamports: raw.compute_unit_price_micro_lamports,
        address_lookup_table,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id32::base58;

    fn minimal(extra: &str) -> String {
        format!(
            r#"
output_dir = "./out"
poll_interval_seconds = 30

[observed_cluster]
rpc_endpoints = ["http://127.0.0.1:8899"]
expected_genesis_hash = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d"

[keys]
attestation_keypair_path = "./keys/attestation.json"

[[account_sets]]
name = "dbc-graduation"
relay_family_id = "{family}"
decoding_rules_id = "{rules}"

[[account_sets.positions]]
key = "11111111111111111111111111111112"
expected_owner = "BPFLoaderUpgradeab1e11111111111111111111111"
inline_len = 36

[[account_sets.positions]]
key = "SysvarC1ock11111111111111111111111111111111"
expected_owner = "Sysvar1111111111111111111111111111111111111"
inline_len = 40
admitted_data_lens = [40]
{extra}
"#,
            family = crate::id32::to_hex(&dclutch_relay_contract::RELAYED_FAMILY_RELEASE_ID_V1),
            rules = crate::id32::to_hex(
                &dclutch_relay_contract::RELAYED_DECODING_RULES_SCHEMA_RELEASE_ID_V1
            ),
            extra = extra
        )
    }

    fn load(text: &str) -> Result<Config> {
        Config::from_toml(text, Path::new("relayer.toml"), Some(Path::new("/home/t")))
    }

    #[test]
    fn a_minimal_config_resolves_and_derives_its_account_set_id() {
        let config = load(&minimal("")).expect("config");
        assert_eq!(config.account_sets.len(), 1);
        let set = config.account_sets.first().expect("set");
        assert_eq!(set.positions.len(), 2);
        assert_eq!(set.max_inline_len(), 40);
        assert!(!is_zero(&set.account_set_id));
        // Derivation, not configuration: the same positions under a different
        // observed cluster produce a different pin.
        let elsewhere = derive_account_set_id(
            dclutch_relay_contract::SOLANA_DEVNET_GENESIS_HASH_V1,
            set.relay_family_id,
            &set.entries(),
        )
        .expect("derive");
        assert_ne!(set.account_set_id, elsewhere);
        assert_eq!(base58(&config.expected_genesis_hash).len(), 44);
    }

    #[test]
    fn an_unknown_field_refuses_rather_than_defaulting() {
        let text = minimal("").replace("output_dir", "output_directory");
        assert!(load(&text).is_err());
        let text = format!("{}\nallow_public_submission = true\n", minimal(""));
        assert!(load(&text).is_err());
    }

    #[test]
    fn an_account_set_id_cannot_be_stated_by_hand() {
        let text = format!(
            "{}\n[[account_sets]]\nname = \"x\"\nrelay_family_id = \"{}\"\n\
             decoding_rules_id = \"{}\"\naccount_set_id = \"{}\"\n",
            minimal(""),
            crate::id32::to_hex(&dclutch_relay_contract::RELAYED_FAMILY_RELEASE_ID_V1),
            crate::id32::to_hex(&dclutch_relay_contract::RELAYED_FAMILY_RELEASE_ID_V1),
            crate::id32::to_hex(&[9u8; 32]),
        );
        assert!(
            load(&text).is_err(),
            "a hand-written account_set_id was accepted"
        );
    }

    #[test]
    fn a_wallet_store_keypair_path_refuses_at_config_load() {
        let text = minimal("").replace(
            "\"./keys/attestation.json\"",
            "\"~/.config/solana/id.json\"",
        );
        let error = load(&text).unwrap_err();
        assert!(
            matches!(error, RelayerError::UnsafeKeypairPath { .. }),
            "{error:?}"
        );
    }

    #[test]
    fn the_fee_payer_and_attestation_keys_must_be_distinct_files() {
        let text = minimal("").replace(
            "attestation_keypair_path = \"./keys/attestation.json\"",
            "attestation_keypair_path = \"./keys/attestation.json\"\n\
             fee_payer_keypair_path = \"./keys/attestation.json\"",
        );
        assert!(load(&text).is_err());
    }

    #[test]
    fn a_zero_poll_interval_refuses() {
        let text = minimal("").replace("poll_interval_seconds = 30", "poll_interval_seconds = 0");
        assert!(load(&text).is_err());
    }

    #[test]
    fn a_body_page_narrower_than_the_inline_ceiling_refuses() {
        let text = format!("{}\nbody_page_bytes = 64\n", minimal(""));
        assert!(load(&text).is_err());
    }

    #[test]
    fn an_inline_width_above_the_release_ceiling_refuses() {
        let text = minimal("").replace("inline_len = 36", "inline_len = 449");
        assert!(load(&text).is_err());
    }

    #[test]
    fn an_aliased_position_refuses() {
        let text = minimal("").replace(
            "key = \"11111111111111111111111111111112\"",
            "key = \"SysvarC1ock11111111111111111111111111111111\"",
        );
        assert!(load(&text).is_err());
    }

    #[test]
    fn an_admitted_length_narrower_than_the_inline_prefix_refuses() {
        let text = minimal("").replace("admitted_data_lens = [40]", "admitted_data_lens = [39]");
        assert!(load(&text).is_err());
    }

    #[test]
    fn admitted_data_lengths_gate_the_observed_width() {
        let position = PositionConfig {
            key: [1; 32],
            expected_owner: [2; 32],
            inline_len: 416,
            admitted_data_lens: vec![416],
        };
        assert!(position.admits_data_len(416));
        assert!(!position.admits_data_len(415));
        assert!(!position.admits_data_len(417));

        let open = PositionConfig {
            admitted_data_lens: Vec::new(),
            ..position
        };
        assert!(open.admits_data_len(416));
        assert!(open.admits_data_len(2_300_000));
        assert!(!open.admits_data_len(4));
    }

    #[test]
    fn allow_public_submission_defaults_to_false() {
        let text = format!(
            "{}\n[submit]\nendpoint = \"http://127.0.0.1:8899\"\n\
             relay_program_id = \"11111111111111111111111111111112\"\n\
             market = \"11111111111111111111111111111113\"\ngeneration = 1\n\
             relayer_key_set = \"11111111111111111111111111111114\"\n\
             relayer_key_set_staging_vacancy = \"11111111111111111111111111111115\"\n",
            minimal("")
        );
        let config = load(&text).expect("config");
        let submit = config.submit.expect("submit");
        assert!(!submit.allow_public_submission);
    }

    #[test]
    fn a_config_with_no_account_sets_refuses() {
        let text = r#"
output_dir = "./out"

[observed_cluster]
rpc_endpoints = ["http://127.0.0.1:8899"]
expected_genesis_hash = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d"

[keys]
attestation_keypair_path = "./keys/attestation.json"
"#;
        assert!(load(text).is_err());
    }
}
