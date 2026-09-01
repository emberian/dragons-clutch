//! Read-only projection of the exact Direct children a terminal campaign owes.
//!
//! The Python supervisor is orchestration, not state authority. This command
//! reopens the signed Direct manifest, every durable mutation journal, and
//! finalized history, then emits the seller/buyer Positions and the exhaustive
//! maker replay set whose count is authenticated by the Direct root poststate.

use std::{
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use solana_sdk::pubkey::Pubkey;

use crate::{
    Error, Result,
    campaign::parse_campaign_terminal_evidence_with_expected_cluster_v1,
    cluster::{ClusterOriginV1, ExpectedClusterV1},
    direct_trade::{
        authenticate_devnet_terminal_evidence_v1, authenticate_owned_loopback_terminal_evidence_v1,
    },
    rpc::{Rpc, WritePolicyV1},
    terminal_lifecycle::authenticate_plan_source,
};

pub(crate) const COMMAND_V1: &str = "local-private-validator-direct-terminal-children-v1";
pub(crate) const COMMAND_DEVNET_V1: &str = "devnet-direct-terminal-children-v1";
const SCHEMA_V1: &str = "dclutch-owned-loopback-direct-terminal-children-v1";
const SCHEMA_DEVNET_V1: &str = "dclutch-devnet-direct-terminal-children-v1";

pub(crate) fn usage() -> &'static str {
    "dclutch-local-successor-bootstrap local-private-validator-direct-terminal-children-v1 --rpc-url http://127.0.0.1:PORT --plan ABSOLUTE_JSON --market-input ABSOLUTE_JSON --campaign-evidence ABSOLUTE_JSON --direct-evidence ABSOLUTE_JSON --output ABSOLUTE_NEW_JSON\n\
     dclutch-local-successor-bootstrap devnet-direct-terminal-children-v1 --rpc-url URL --i-mean-devnet GENESIS_HASH --plan ABSOLUTE_JSON --market-input ABSOLUTE_JSON --campaign-evidence ABSOLUTE_JSON --direct-evidence ABSOLUTE_JSON --output ABSOLUTE_NEW_JSON\n\
     \nRead-only terminal projection. Reauthenticates the exact signed Direct manifest, durable mutation journals, and finalized transaction history after payout, then emits the two authenticated Position children and every standing maker replay whose cardinality equals the authenticated Direct root count. Opens no key and submits nothing."
}

#[derive(Debug)]
struct ArgumentsV1 {
    origin: ClusterOriginV1,
    expected_cluster: ExpectedClusterV1,
    plan: PathBuf,
    market_input: PathBuf,
    campaign_evidence: PathBuf,
    direct_evidence: PathBuf,
    output: PathBuf,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PositionChildV1 {
    role: String,
    owner: String,
    position: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MakerReplayChildV1 {
    maker: String,
    replay: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReceiptV1 {
    schema: String,
    cluster: String,
    plan_sha256: String,
    market_input_sha256: String,
    campaign_evidence_sha256: String,
    direct_evidence_sha256: String,
    direct_semantic_evidence_sha256: String,
    market: String,
    claims_market: String,
    direct_root: String,
    capability_entry_index: u16,
    position_children: Vec<PositionChildV1>,
    open_maker_root_count: u64,
    maker_replay_children: Vec<MakerReplayChildV1>,
    state_sha256: String,
}

pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    run_for_cluster_v1(arguments, ExpectedClusterV1::OwnedLoopback)
}

pub(crate) fn run_devnet_v1(arguments: Vec<String>) -> Result<()> {
    run_for_cluster_v1(arguments, ExpectedClusterV1::Devnet)
}

fn run_for_cluster_v1(arguments: Vec<String>, expected_cluster: ExpectedClusterV1) -> Result<()> {
    let arguments = parse_for_cluster_v1(arguments, expected_cluster)?;
    let plan = read_exact(&arguments.plan, "successor plan")?;
    let market_input = read_exact(&arguments.market_input, "Market input")?;
    let campaign_source = read_exact(&arguments.campaign_evidence, "campaign evidence")?;
    let direct_source = read_exact(&arguments.direct_evidence, "Direct evidence")?;
    let campaign = parse_campaign_terminal_evidence_with_expected_cluster_v1(
        &campaign_source,
        expected_cluster,
    )?;
    authenticate_plan_source(&plan, &campaign.plan_sha256)?;
    let plan_sha256 = sha256(&plan);
    let market_input_sha256 = sha256(&market_input);
    if market_input_sha256 != campaign.market_sha256 {
        return Err(refusal("Market input changed from founding evidence"));
    }
    let campaign_key = |name: &str| -> Result<Pubkey> {
        campaign
            .accounts
            .get(name)
            .ok_or_else(|| refusal(format!("campaign evidence omitted {name}")))?
            .address
            .parse::<Pubkey>()
            .map_err(|error| Error::new(format!("campaign {name}: {error}")))
    };
    let market = campaign_key("founding_market")?;
    let claims_market = campaign_key("claims_aggregate")?;
    let mut rpc = Rpc::connect_cluster(&arguments.origin, WritePolicyV1::ReadsOnly)?;
    let terminal = match expected_cluster {
        ExpectedClusterV1::Devnet => authenticate_devnet_terminal_evidence_v1(
            &mut rpc,
            &arguments.direct_evidence,
            market,
            &plan_sha256,
            &market_input_sha256,
        )?,
        ExpectedClusterV1::OwnedLoopback => authenticate_owned_loopback_terminal_evidence_v1(
            &mut rpc,
            &arguments.direct_evidence,
            market,
            &plan_sha256,
            &market_input_sha256,
        )?,
    };
    if terminal.direct.market != market || terminal.claims_market != claims_market {
        return Err(refusal(
            "Direct terminal history and founding evidence named different roots",
        ));
    }
    if read_exact(&arguments.direct_evidence, "Direct evidence")? != direct_source {
        return Err(refusal(
            "Direct evidence changed while finalized history was authenticated",
        ));
    }

    let positions = [
        PositionChildV1 {
            role: "seller".into(),
            owner: terminal.direct.seller_owner.to_string(),
            position: terminal.direct.seller_position.to_string(),
        },
        PositionChildV1 {
            role: "buyer".into(),
            owner: terminal.direct.buyer_owner.to_string(),
            position: terminal.direct.buyer_position.to_string(),
        },
    ];
    let makers = terminal
        .maker_replays
        .iter()
        .map(|row| MakerReplayChildV1 {
            maker: row.maker.to_string(),
            replay: row.replay.to_string(),
        })
        .collect::<Vec<_>>();
    let mut receipt = ReceiptV1 {
        schema: match expected_cluster {
            ExpectedClusterV1::Devnet => SCHEMA_DEVNET_V1,
            ExpectedClusterV1::OwnedLoopback => SCHEMA_V1,
        }
        .into(),
        cluster: expected_cluster.evidence_label().into(),
        plan_sha256,
        market_input_sha256,
        campaign_evidence_sha256: sha256(&campaign_source),
        direct_evidence_sha256: sha256(&direct_source),
        direct_semantic_evidence_sha256: terminal.direct.evidence_sha256,
        market: market.to_string(),
        claims_market: claims_market.to_string(),
        direct_root: terminal.direct_root.to_string(),
        capability_entry_index: campaign.direct_selected_manifest_entry_index,
        position_children: positions.into(),
        open_maker_root_count: terminal.open_maker_root_count,
        maker_replay_children: makers,
        state_sha256: String::new(),
    };
    receipt.state_sha256 = receipt_digest(&receipt)?;
    write_or_authenticate(&arguments.output, &receipt)?;
    println!("{}", serde_json::to_string_pretty(&receipt)?);
    Ok(())
}

fn receipt_digest(receipt: &ReceiptV1) -> Result<String> {
    let mut material = receipt.clone();
    material.state_sha256.clear();
    Ok(sha256(&serde_json::to_vec(&material)?))
}

fn write_or_authenticate(path: &Path, receipt: &ReceiptV1) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(receipt)?;
    bytes.push(b'\n');
    if path.exists() {
        if read_exact(path, "Direct terminal children receipt")? != bytes {
            return Err(refusal("existing Direct terminal children receipt changed"));
        }
        return Ok(());
    }
    let parent = path
        .parent()
        .ok_or_else(|| refusal("Direct terminal children output omitted parent"))?;
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    OpenOptions::new().read(true).open(parent)?.sync_all()?;
    Ok(())
}

fn read_exact(path: &Path, label: &str) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| Error::new(format!("{label} {}: {error}", path.display())))?;
    if !path.is_absolute()
        || metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > 16 * 1024 * 1024
        || fs::canonicalize(path)? != path
    {
        return Err(refusal(format!(
            "{label} must be one canonical absolute regular file within 1..16777216 bytes"
        )));
    }
    fs::read(path).map_err(Into::into)
}

fn parse(arguments: Vec<String>) -> Result<ArgumentsV1> {
    parse_for_cluster_v1(arguments, ExpectedClusterV1::OwnedLoopback)
}

fn parse_for_cluster_v1(
    arguments: Vec<String>,
    expected_cluster: ExpectedClusterV1,
) -> Result<ArgumentsV1> {
    let mut values = std::collections::BTreeMap::new();
    let mut iterator = arguments.into_iter();
    while let Some(flag) = iterator.next() {
        if !matches!(
            flag.as_str(),
            "--rpc-url"
                | "--plan"
                | "--market-input"
                | "--campaign-evidence"
                | "--direct-evidence"
                | "--output"
                | "--i-mean-devnet"
        ) || values.contains_key(&flag)
        {
            return Err(refusal(format!("unknown or repeated argument {flag}")));
        }
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{flag} requires a value")))?;
        values.insert(flag, value);
    }
    let take = |name: &str| {
        values
            .get(name)
            .cloned()
            .ok_or_else(|| Error::new(format!("{name} is required\n{}", usage())))
    };
    let absolute = |name: &str| -> Result<PathBuf> {
        let path = PathBuf::from(take(name)?);
        if !path.is_absolute() {
            return Err(refusal(format!("{name} must be absolute")));
        }
        Ok(path)
    };
    let rpc_url = take("--rpc-url")?;
    let origin =
        ClusterOriginV1::parse(&rpc_url, values.get("--i-mean-devnet").map(String::as_str))?;
    expected_cluster.authenticate(&origin)?;
    Ok(ArgumentsV1 {
        origin,
        expected_cluster,
        plan: absolute("--plan")?,
        market_input: absolute("--market-input")?,
        campaign_evidence: absolute("--campaign-evidence")?,
        direct_evidence: absolute("--direct-evidence")?,
        output: absolute("--output")?,
    })
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn refusal(reason: impl AsRef<str>) -> Error {
    Error::new(format!("REFUSED: {}", reason.as_ref()))
}

#[cfg(test)]
mod receipt_digest_tests {
    use super::{MakerReplayChildV1, PositionChildV1, ReceiptV1, receipt_digest, sha256};

    #[test]
    fn receipt_digest_blanks_self_field_without_reordering_the_emitted_schema() {
        let receipt = ReceiptV1 {
            schema: "schema".into(),
            cluster: "owned-loopback".into(),
            plan_sha256: "11".repeat(32),
            market_input_sha256: "22".repeat(32),
            campaign_evidence_sha256: "33".repeat(32),
            direct_evidence_sha256: "44".repeat(32),
            direct_semantic_evidence_sha256: "55".repeat(32),
            market: "market".into(),
            claims_market: "claims".into(),
            direct_root: "root".into(),
            capability_entry_index: 7,
            position_children: vec![PositionChildV1 {
                role: "seller".into(),
                owner: "owner".into(),
                position: "position".into(),
            }],
            open_maker_root_count: 1,
            maker_replay_children: vec![MakerReplayChildV1 {
                maker: "owner".into(),
                replay: "replay".into(),
            }],
            state_sha256: "66".repeat(32),
        };
        let mut blank = receipt.clone();
        blank.state_sha256.clear();
        let ordered = serde_json::to_vec(&blank).expect("serialize ordered receipt");
        assert_eq!(receipt_digest(&receipt).expect("digest"), sha256(&ordered));
        assert!(
            std::str::from_utf8(&ordered)
                .expect("JSON")
                .starts_with("{\"schema\":\"schema\",\"cluster\":\"owned-loopback\"")
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{parse, parse_for_cluster_v1};
    use crate::cluster::{DEVNET_GENESIS_HASH, ExpectedClusterV1};

    #[test]
    fn projection_requires_every_exact_source_path() {
        let complete = [
            "--rpc-url",
            "http://127.0.0.1:8899",
            "--plan",
            "/tmp/plan.json",
            "--market-input",
            "/tmp/market.json",
            "--campaign-evidence",
            "/tmp/campaign.json",
            "--direct-evidence",
            "/tmp/direct.json",
            "--output",
            "/tmp/children.json",
        ];
        assert!(parse(complete.map(str::to_owned).to_vec()).is_ok());
        assert!(
            parse(
                complete[..10]
                    .iter()
                    .map(|value| (*value).to_owned())
                    .collect()
            )
            .is_err()
        );
    }

    #[test]
    fn public_projection_has_a_distinct_exact_devnet_origin() {
        let exact = [
            "--rpc-url",
            "https://api.devnet.solana.com:443/",
            "--i-mean-devnet",
            DEVNET_GENESIS_HASH,
            "--plan",
            "/tmp/plan.json",
            "--market-input",
            "/tmp/market.json",
            "--campaign-evidence",
            "/tmp/campaign.json",
            "--direct-evidence",
            "/tmp/direct.json",
            "--output",
            "/tmp/children.json",
        ];
        let parsed =
            parse_for_cluster_v1(exact.map(str::to_owned).to_vec(), ExpectedClusterV1::Devnet)
                .expect("exact devnet projection");
        assert_eq!(parsed.expected_cluster, ExpectedClusterV1::Devnet);

        let mut absent = exact.map(str::to_owned).to_vec();
        absent.drain(2..4);
        assert!(parse_for_cluster_v1(absent, ExpectedClusterV1::Devnet).is_err());
        assert!(
            parse_for_cluster_v1(
                exact.map(str::to_owned).to_vec(),
                ExpectedClusterV1::OwnedLoopback,
            )
            .is_err()
        );
    }
}
