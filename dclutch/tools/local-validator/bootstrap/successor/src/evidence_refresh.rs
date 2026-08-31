//! The post-activation evidence refresh: the founded world, legitimately advanced.
//!
//! A completed founding campaign seals evidence describing the world it
//! founded. Two consumers then pin that evidence against finalized chain state
//! — `flagship_resolution`'s twelve-label producer pin and the terminal
//! sequence's Direct lifecycle labels — and both demand byte-equality between a
//! recorded row and a live re-read.
//!
//! For eleven of the twelve that relation is correct forever: they are finalized
//! content-addressed Registry records, and a record whose bytes moved is a
//! different record. For `founding_market` it is correct only at the founding
//! instant. Core commits the Market's outstanding-capability count on
//! activation (`programs/dclutch-core-sbf/src/capability.rs`), so the pin, read
//! literally, says *this market must never have had a capability activated* —
//! which `retire_v1`'s own `outstanding_capabilities != 0` gate proves was never
//! the intent, since the protocol expects that counter to rise and fall over a
//! market's life.
//!
//! # What this module is, and what it deliberately is not
//!
//! It is **not** a relaxation. The relation stays byte-equality against live
//! finalized state, enforced by the same `authenticate_campaign_account` with
//! no change at all. What widens is the set of documents permitted to supply a
//! row: the founding generation, plus a refreshed generation *chained to it*.
//!
//! The chain link is carried by the records that cannot change.
//! [`IMMUTABLE_FOUNDING_RECORD_LABELS_V1`] must appear in the refresh
//! byte-identical to their founding rows, or the refresh is refused. That is
//! the anti-forgery spine, and it costs nothing in strictness: those eleven are
//! already pinned against chain at resolution, so a refresh that cannot
//! reproduce them describes a world the consumer would have refused anyway.
//!
//! Every byte a refresh carries is therefore either re-checked against live
//! chain by the consumer, pinned byte-identical to the founding evidence here,
//! or an audit scalar bounded by the observation. There is no field an attacker
//! can set that is *believed* rather than re-derived. The refresh adds a
//! document; it adds no trust (O-016).
//!
//! `docs/design/EVIDENCE_REFRESH_V1.md` is the design; §3 there records the
//! two-roots naming hazard this module resolves.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use solana_sdk::pubkey::Pubkey;

use crate::{
    Error, Result, campaign,
    cluster::{ClusterOriginV1, DEVNET_GENESIS_HASH, ExpectedClusterV1},
    direct_capability_activation::direct_execution_root_v1,
    direct_trade_producer::resolved_record_v1,
    model::{AccountEvidence, MarketRunInput, SuccessorPlan},
    plan::pubkey,
    rpc::{Rpc, WritePolicyV1, account_evidence, parse_json_without_duplicate_keys_v1},
};

pub(crate) const EVIDENCE_REFRESH_SCHEMA_V1: &str = "dclutch-successor-evidence-refresh-v1";

pub(crate) const REFRESH_EVIDENCE_COMMAND_V1: &str = "devnet-refresh-evidence-v1";

pub(crate) const LOCAL_REFRESH_EVIDENCE_COMMAND_V1: &str =
    "local-private-validator-refresh-evidence-v1";

const MAX_REFRESH_BYTES_V1: usize = 1024 * 1024;

/// The finalized content-addressed records among the producer's twelve.
///
/// A refresh MUST carry each of these byte-identical to its founding row. They
/// are the lineage: an author who cannot reproduce them is not describing a
/// later view of this founded world.
///
/// `founding_market` is deliberately absent — it is the one live mutable
/// account in the producer's set, and advancing it is the whole point.
pub(crate) const IMMUTABLE_FOUNDING_RECORD_LABELS_V1: [&str; 11] = [
    "capability_manifest_record",
    "portfolio_record",
    "product_record",
    "provider_release_record",
    "pyth_adapter_config_record",
    "resolution_funding_ledger",
    "result_domain_record",
    "source_material_record",
    "source_spec_record",
    "statistic_spec_record",
    "window_spec_record",
];

/// The live accounts a market's own lawful life moves after founding.
///
/// This list is deliberately about a CLASS, not about one cause. Activation is
/// how the wall was first hit — Core commits the Market's outstanding-capability
/// count, and carries the Trading ledger's parked rent quote into the root it
/// creates — but it is not special. Admission moves the Claims trio; a fill and
/// a fee settlement moved `founding_market` twice more on the substrate where
/// this was first measured, with no activation involved. Every one of these is
/// an account the protocol is *designed* to advance between founding and
/// resolution, and whose row some consumer pins.
///
/// Being listed here buys permission to *differ from the founding evidence*. It
/// never buys permission to differ from the chain: each row is still pinned
/// byte-exact against the live finalized account by the consumer that reads it.
pub(crate) const ADVANCEABLE_FOUNDING_LABELS_V1: [&str; 5] = [
    // Core state: phase, readiness, outstanding capabilities.
    "founding_market",
    // Activation carries the parked rent quote out of it.
    "direct_trading_funding_ledger",
    // The Claims trio, moved by admission. `claims_admission` is pinned
    // byte-exact in its own right (`flagship_resolution::admitted_campaign_resolver`),
    // which is the second, independent way this wall is reached.
    "claims_admission",
    "claims_aggregate",
    "founder_position",
];

/// The label under which the refresh publishes the Direct **execution** root.
///
/// The founding checkpoint scalar of the same name is the founding-permit
/// address, at which no account can ever exist. This one is the address
/// activation creates and the terminal sequence means.
pub(crate) const DIRECT_EXECUTION_ROOT_LABEL_V1: &str = "direct_capability_root";

fn refusal(code: &str, reason: impl AsRef<str>) -> Error {
    Error::new(format!(
        "REFUSED evidence refresh [{code}]: {}",
        reason.as_ref()
    ))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EvidenceRefreshV1 {
    pub(crate) schema: String,
    pub(crate) cluster: String,
    pub(crate) mode: String,
    pub(crate) plan_sha256: String,
    /// SHA-256 of the founding campaign report bytes this refresh advances.
    pub(crate) founding_evidence_sha256: String,
    pub(crate) genesis_hash: String,
    /// The finalized slot every row below was read at.
    pub(crate) as_of_slot: u64,
    pub(crate) market: String,
    /// The address activation created and every terminal consumer means.
    pub(crate) direct_execution_capability_root: String,
    /// The founding-permit namespace address, reported for the record. No
    /// account can exist here; nothing derives authority from it.
    pub(crate) founding_permit_capability_root: Option<String>,
    pub(crate) accounts: BTreeMap<String, AccountEvidence>,
}

/// Merge an admitted refresh over founding rows, or refuse.
///
/// The returned map is the **effective** evidence: founding rows, overridden and
/// extended by the refresh's. Every existing consumer check then runs against
/// it unchanged — which is the point. This function widens *where a row may come
/// from*; it weakens nothing about what is then demanded of that row.
pub(crate) fn effective_accounts_v1(
    refresh: &EvidenceRefreshV1,
    founding_bytes: &[u8],
    founding_accounts: &BTreeMap<String, AccountEvidence>,
    plan_sha256: &str,
    expected_cluster: ExpectedClusterV1,
    observation_slot: u64,
) -> Result<BTreeMap<String, AccountEvidence>> {
    // R1 - envelope binding. The refresh must name this schema, this cluster,
    // this plan, and the exact founding bytes loaded in this run.
    let expected_label = match expected_cluster {
        ExpectedClusterV1::Devnet => "devnet",
        ExpectedClusterV1::OwnedLoopback => "loopback",
    };
    if refresh.schema != EVIDENCE_REFRESH_SCHEMA_V1
        || refresh.cluster != expected_label
        || refresh.mode != "refresh"
    {
        return Err(refusal(
            "refresh/envelope",
            "refreshed evidence is not a refresh envelope for this cluster",
        ));
    }
    if refresh.plan_sha256 != plan_sha256 {
        return Err(refusal(
            "refresh/plan-digest",
            "refreshed evidence was produced against another plan",
        ));
    }
    let founding_digest = hex_digest_v1(founding_bytes);
    if refresh.founding_evidence_sha256 != founding_digest {
        return Err(refusal(
            "refresh/lineage",
            "refreshed evidence is not chained to this founding campaign",
        ));
    }

    // R5 - as-of slot. A refresh cannot have observed the future.
    if refresh.as_of_slot > observation_slot {
        return Err(refusal(
            "refresh/as-of-slot",
            format!(
                "refreshed evidence as-of slot {} is ahead of the finalized observation {}",
                refresh.as_of_slot, observation_slot
            ),
        ));
    }

    // R2 - the immutable eleven do not move. This is the spine: an author who
    // cannot reproduce the founding's content-addressed records byte-for-byte
    // is not describing this founded world at a later slot.
    for label in IMMUTABLE_FOUNDING_RECORD_LABELS_V1 {
        let Some(founding_row) = founding_accounts.get(label) else {
            // Founding never carried it; there is nothing to pin, and the
            // consumer's own omission refusal remains the authority.
            continue;
        };
        let refreshed_row = refresh.accounts.get(label).ok_or_else(|| {
            refusal(
                "refresh/immutable-omitted",
                format!("refreshed evidence altered immutable founding record {label}"),
            )
        })?;
        if refreshed_row != founding_row {
            return Err(refusal(
                "refresh/immutable-altered",
                format!("refreshed evidence altered immutable founding record {label}"),
            ));
        }
    }

    // R3 - the market coordinate does not move. Only its state may.
    if let Some(founding_market) = founding_accounts.get("founding_market") {
        let refreshed_market = refresh.accounts.get("founding_market").ok_or_else(|| {
            refusal(
                "refresh/market-omitted",
                "refreshed evidence omitted founding_market",
            )
        })?;
        if refreshed_market.address != founding_market.address {
            return Err(refusal(
                "refresh/market-substituted",
                "refreshed evidence substituted the founding Market address",
            ));
        }
    }
    if refresh.market
        != refresh
            .accounts
            .get("founding_market")
            .map(|row| row.address.clone())
            .unwrap_or_default()
    {
        return Err(refusal(
            "refresh/market-scalar",
            "refreshed evidence market scalar disagrees with its own founding_market row",
        ));
    }
    // The root row is present exactly when activation has run. A refresh taken
    // of a market advanced only by admission has no root to append, and saying
    // so is the honest answer — not a reason to refuse the refresh. When the row
    // IS present it must agree with the scalar.
    if let Some(root_row) = refresh.accounts.get(DIRECT_EXECUTION_ROOT_LABEL_V1)
        && root_row.address != refresh.direct_execution_capability_root
    {
        return Err(refusal(
            "refresh/root-scalar",
            "refreshed evidence execution-root scalar disagrees with its own root row",
        ));
    }

    // Nothing outside the pinned eleven, the advanceable set, and the execution
    // root has any business in a refresh. An author that smuggles other labels
    // is overriding rows this design never examined.
    for label in refresh.accounts.keys() {
        if IMMUTABLE_FOUNDING_RECORD_LABELS_V1.contains(&label.as_str())
            || ADVANCEABLE_FOUNDING_LABELS_V1.contains(&label.as_str())
            || label == DIRECT_EXECUTION_ROOT_LABEL_V1
        {
            continue;
        }
        return Err(refusal(
            "refresh/unadmitted-label",
            format!("refreshed evidence carries unadmitted label {label}"),
        ));
    }

    let mut effective = founding_accounts.clone();
    for (label, row) in &refresh.accounts {
        effective.insert(label.clone(), row.clone());
    }
    Ok(effective)
}

/// Parse a refresh document, bounded and duplicate-key-refusing.
pub(crate) fn parse_refresh_v1(source: &[u8]) -> Result<EvidenceRefreshV1> {
    if source.is_empty() || source.len() > MAX_REFRESH_BYTES_V1 {
        return Err(refusal(
            "refresh/size",
            "refreshed evidence is outside the 1..1048576 byte bound",
        ));
    }
    let value: Value = parse_json_without_duplicate_keys_v1(source)?;
    serde_json::from_value(value)
        .map_err(|error| refusal("refresh/shape", format!("refreshed evidence: {error}")))
}

pub(crate) fn hex_digest_v1(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

// ------------------------------------------------------------------ emitter

#[derive(Debug)]
struct ArgumentsV1 {
    rpc_url: String,
    acknowledgment: Option<String>,
    plan: PathBuf,
    expected_plan_sha256: String,
    market_input: PathBuf,
    expected_market_input_sha256: String,
    campaign_report: PathBuf,
    expected_campaign_report_sha256: String,
    output: PathBuf,
}

fn usage_for(expected: ExpectedClusterV1) -> String {
    let command = match expected {
        ExpectedClusterV1::Devnet => REFRESH_EVIDENCE_COMMAND_V1,
        ExpectedClusterV1::OwnedLoopback => LOCAL_REFRESH_EVIDENCE_COMMAND_V1,
    };
    let acknowledgment = match expected {
        ExpectedClusterV1::Devnet => " --i-mean-devnet <devnet genesis hash>",
        ExpectedClusterV1::OwnedLoopback => "",
    };
    format!(
        "{command} --rpc-url <url>{acknowledgment} --plan <path> --expected-plan-sha256 <hex> \
         --market-input <path> --expected-market-input-sha256 <hex> --campaign-report <path> \
         --expected-campaign-report-sha256 <hex> --output <path>"
    )
}

fn parse_arguments(arguments: Vec<String>, expected: ExpectedClusterV1) -> Result<ArgumentsV1> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut plan = None;
    let mut expected_plan = None;
    let mut market_input = None;
    let mut expected_market = None;
    let mut campaign_report = None;
    let mut expected_campaign = None;
    let mut output = None;
    let mut iterator = arguments.into_iter();
    while let Some(flag) = iterator.next() {
        let value = iterator.next().ok_or_else(|| {
            refusal(
                "input/missing-value",
                format!("{flag}; usage: {}", usage_for(expected)),
            )
        })?;
        let slot = match flag.as_str() {
            "--rpc-url" => &mut rpc_url,
            "--i-mean-devnet" => &mut acknowledgment,
            "--plan" => &mut plan,
            "--expected-plan-sha256" => &mut expected_plan,
            "--market-input" => &mut market_input,
            "--expected-market-input-sha256" => &mut expected_market,
            "--campaign-report" => &mut campaign_report,
            "--expected-campaign-report-sha256" => &mut expected_campaign,
            "--output" => &mut output,
            other => return Err(refusal("input/unknown-flag", other)),
        };
        if slot.replace(value).is_some() {
            return Err(refusal("input/repeated-flag", flag));
        }
    }
    match expected {
        ExpectedClusterV1::Devnet => {
            if acknowledgment.as_deref() != Some(DEVNET_GENESIS_HASH) {
                return Err(refusal(
                    "input/devnet-acknowledgment",
                    format!("--i-mean-devnet must be exactly {DEVNET_GENESIS_HASH}"),
                ));
            }
        }
        ExpectedClusterV1::OwnedLoopback => {
            if acknowledgment.is_some() {
                return Err(refusal(
                    "input/loopback-acknowledgment",
                    format!(
                        "{LOCAL_REFRESH_EVIDENCE_COMMAND_V1} runs against an owned loopback \
                         validator, which needs no acknowledgment; \
                         {REFRESH_EVIDENCE_COMMAND_V1} is the devnet endpoint"
                    ),
                ));
            }
        }
    }
    let required = |value: Option<String>, name: &str| {
        value.ok_or_else(|| {
            refusal(
                "input/missing-flag",
                format!("{name}; usage: {}", usage_for(expected)),
            )
        })
    };
    Ok(ArgumentsV1 {
        rpc_url: required(rpc_url, "--rpc-url")?,
        acknowledgment,
        plan: PathBuf::from(required(plan, "--plan")?),
        expected_plan_sha256: required(expected_plan, "--expected-plan-sha256")?,
        market_input: PathBuf::from(required(market_input, "--market-input")?),
        expected_market_input_sha256: required(expected_market, "--expected-market-input-sha256")?,
        campaign_report: PathBuf::from(required(campaign_report, "--campaign-report")?),
        expected_campaign_report_sha256: required(
            expected_campaign,
            "--expected-campaign-report-sha256",
        )?,
        output: PathBuf::from(required(output, "--output")?),
    })
}

fn pinned(path: &Path, expected: &str, label: &str) -> Result<Vec<u8>> {
    let bytes = std::fs::read(path)
        .map_err(|error| refusal("input/read", format!("{label} {}: {error}", path.display())))?;
    let actual = hex_digest_v1(&bytes);
    if actual != expected {
        return Err(refusal(
            "input/digest",
            format!("{label} digest is {actual}, expected {expected}"),
        ));
    }
    Ok(bytes)
}

pub(crate) fn run_devnet(arguments: Vec<String>) -> Result<()> {
    run(arguments, ExpectedClusterV1::Devnet)
}

pub(crate) fn run_owned_loopback(arguments: Vec<String>) -> Result<()> {
    run(arguments, ExpectedClusterV1::OwnedLoopback)
}

/// Emit a refresh from finalized chain state. Reads only; it never writes.
fn run(arguments: Vec<String>, expected: ExpectedClusterV1) -> Result<()> {
    let arguments = parse_arguments(arguments, expected)?;
    let plan_bytes = pinned(&arguments.plan, &arguments.expected_plan_sha256, "plan")?;
    let market_bytes = pinned(
        &arguments.market_input,
        &arguments.expected_market_input_sha256,
        "market input",
    )?;
    let campaign_bytes = pinned(
        &arguments.campaign_report,
        &arguments.expected_campaign_report_sha256,
        "campaign report",
    )?;
    if arguments.output.exists() {
        return Err(refusal(
            "output/exists",
            format!("refusing to overwrite {}", arguments.output.display()),
        ));
    }
    let plan: SuccessorPlan = serde_json::from_slice(&plan_bytes)
        .map_err(|error| refusal("input/plan", format!("successor plan: {error}")))?;
    let market_input: MarketRunInput = serde_json::from_slice(&market_bytes)
        .map_err(|error| refusal("input/market", format!("market input: {error}")))?;
    let evidence = campaign::parse_campaign_terminal_evidence_with_expected_cluster_v1(
        &campaign_bytes,
        expected,
    )?;

    let origin = ClusterOriginV1::parse(&arguments.rpc_url, arguments.acknowledgment.as_deref())?;
    expected.authenticate(&origin)?;
    // ReadsOnly is enforced by the method allowlist at the single call site
    // every request passes through: a refresh CANNOT write, rather than
    // intending not to.
    let mut rpc = Rpc::connect_cluster(&origin, WritePolicyV1::ReadsOnly)?;
    let genesis_hash = rpc
        .call("getGenesisHash", &json!([]))?
        .as_str()
        .ok_or_else(|| refusal("chain/genesis", "getGenesisHash result was not a string"))?
        .to_owned();

    let market = evidence
        .accounts
        .get("founding_market")
        .map(|row| pubkey(&row.address))
        .transpose()?
        .ok_or_else(|| {
            refusal(
                "refresh/campaign-market",
                "campaign omitted founding_market",
            )
        })?;
    let entry_index = evidence.direct_selected_manifest_entry_index;
    let manifest_pair = resolved_record_v1(&plan, &market_input, &evidence, "capability_manifest_record")?;

    // Every coordinate below is re-derived from the pinned plan and finalized
    // chain state. Nothing is taken from a flag, and the founding checkpoint's
    // permit scalar is carried for the record only.
    let market_account = rpc.required_account(market, "founding Market")?;
    let market_state = dclutch_market_core_codec::CoreState::decode(&market_account.data)
        .map_err(|error| refusal("chain/market", format!("Core Market state: {error:?}")))?;
    if market_account.owner != pubkey(&plan.core.program_id)? {
        return Err(refusal(
            "chain/market-owner",
            "founding Market is not Core-owned",
        ));
    }
    let manifest_body = {
        let account = rpc.required_account(pubkey(&manifest_pair.raw)?, "capability manifest")?;
        if hex_digest_v1(&account.data) != manifest_pair.content_sha256 {
            return Err(refusal(
                "chain/manifest-content",
                "manifest record bytes differ from their sealed digest",
            ));
        }
        account.data
    };
    if market_state.identity.capability_manifest.to_bytes()
        != <[u8; 32]>::from(Sha256::digest(&manifest_body))
    {
        return Err(refusal(
            "chain/manifest-identity",
            "market identity selects another capability manifest",
        ));
    }
    let derived = direct_execution_root_v1(
        pubkey(&plan.trading.program_id)?,
        market_state.identity.selected_release_set,
        market,
        market_state.identity.generation,
        entry_index,
        &manifest_body,
    )?;
    let root = derived.root;

    let ledger = evidence
        .accounts
        .get("direct_trading_funding_ledger")
        .map(|row| pubkey(&row.address))
        .transpose()?
        .ok_or_else(|| {
            refusal(
                "refresh/campaign-ledger",
                "campaign omitted direct_trading_funding_ledger",
            )
        })?;

    let as_of_slot = rpc.finalized_slot()?;
    let mut accounts = BTreeMap::new();
    // The advanceable set, re-read at the refresh slot. Which of them actually
    // moved is not this command's business: it reads them all and reports what
    // the chain says, so the refresh covers the whole class of lawful
    // post-founding advancement rather than one cause of it.
    accounts.insert(
        "founding_market".into(),
        account_evidence(market, &market_account),
    );
    let ledger_account = rpc.required_account(ledger, "direct_trading_funding_ledger")?;
    accounts.insert(
        "direct_trading_funding_ledger".into(),
        account_evidence(ledger, &ledger_account),
    );
    for label in ["claims_admission", "claims_aggregate", "founder_position"] {
        let Some(founding_row) = evidence.accounts.get(label) else {
            continue;
        };
        let address = pubkey(&founding_row.address)?;
        let account = rpc.required_account(address, label)?;
        accounts.insert(label.into(), account_evidence(address, &account));
    }
    // The execution root. Its absence is not a failed read: it is the statement
    // that activation has not run, and a refresh of a market advanced only by
    // admission has no root to append. Emitting it half-present would trip the
    // terminal sequence's all-or-none pairing, which is exactly right.
    if let Some(root_account) = rpc.account(root)? {
        accounts.insert(
            DIRECT_EXECUTION_ROOT_LABEL_V1.into(),
            account_evidence(root, &root_account),
        );
    }
    // The immutable eleven, re-read and required to still be what founding
    // sealed. Emitting a differing row would only be refused downstream; saying
    // so here names the record that moved.
    for label in IMMUTABLE_FOUNDING_RECORD_LABELS_V1 {
        let Some(founding_row) = evidence.accounts.get(label) else {
            continue;
        };
        let address = pubkey(&founding_row.address)?;
        let account = rpc.required_account(address, label)?;
        let row = account_evidence(address, &account);
        if row.owner != founding_row.owner
            || row.lamports != founding_row.lamports
            || row.executable != founding_row.executable
            || row.data_len != founding_row.data_len
            || row.data_sha256 != founding_row.data_sha256
            || row.account_sha256 != founding_row.account_sha256
        {
            return Err(refusal(
                "refresh/immutable-moved",
                format!(
                    "immutable founding record {label} differs on chain from the founding \
                     campaign's sealed row; this world is not that world"
                ),
            ));
        }
        accounts.insert(label.into(), row);
    }

    let refresh = EvidenceRefreshV1 {
        schema: EVIDENCE_REFRESH_SCHEMA_V1.into(),
        cluster: match expected {
            ExpectedClusterV1::Devnet => "devnet".into(),
            ExpectedClusterV1::OwnedLoopback => "loopback".into(),
        },
        mode: "refresh".into(),
        plan_sha256: arguments.expected_plan_sha256.clone(),
        founding_evidence_sha256: arguments.expected_campaign_report_sha256.clone(),
        genesis_hash,
        as_of_slot,
        market: market.to_string(),
        direct_execution_capability_root: root.to_string(),
        founding_permit_capability_root: evidence.checkpoint_direct_capability_root.clone(),
        accounts,
    };
    let rendered = format!("{}\n", serde_json::to_string_pretty(&refresh)?);
    std::fs::write(&arguments.output, &rendered)?;
    println!("{rendered}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(address: &str, data_sha: &str) -> AccountEvidence {
        AccountEvidence {
            address: address.into(),
            owner: "11111111111111111111111111111111".into(),
            lamports: 1_000,
            executable: false,
            data_len: 8,
            data_sha256: data_sha.into(),
            account_sha256: "cc".repeat(32),
        }
    }

    const MARKET: &str = "So11111111111111111111111111111111111111112";
    const ROOT: &str = "SysvarC1ock11111111111111111111111111111111";

    fn founding_bytes() -> Vec<u8> {
        b"{\"founding\":\"campaign\"}".to_vec()
    }

    fn founding_map() -> BTreeMap<String, AccountEvidence> {
        let mut map = BTreeMap::new();
        map.insert("founding_market".to_string(), row(MARKET, &"aa".repeat(32)));
        map.insert(
            "direct_trading_funding_ledger".to_string(),
            row("SysvarRent111111111111111111111111111111111", &"bb".repeat(32)),
        );
        for label in ["claims_admission", "claims_aggregate", "founder_position"] {
            map.insert(label.to_string(), row(MARKET, &"33".repeat(32)));
        }
        for (index, label) in IMMUTABLE_FOUNDING_RECORD_LABELS_V1.into_iter().enumerate() {
            map.insert(
                label.to_string(),
                row(MARKET, &format!("{:02x}", index).repeat(32)),
            );
        }
        map
    }

    fn admissible_refresh() -> EvidenceRefreshV1 {
        let founding = founding_map();
        let mut accounts = BTreeMap::new();
        // The market advanced: a different data digest, the same address.
        accounts.insert("founding_market".to_string(), row(MARKET, &"dd".repeat(32)));
        accounts.insert(
            "direct_trading_funding_ledger".to_string(),
            row("SysvarRent111111111111111111111111111111111", &"ee".repeat(32)),
        );
        accounts.insert(
            DIRECT_EXECUTION_ROOT_LABEL_V1.to_string(),
            row(ROOT, &"ff".repeat(32)),
        );
        for label in IMMUTABLE_FOUNDING_RECORD_LABELS_V1 {
            accounts.insert(
                label.to_string(),
                founding.get(label).expect("founding row").clone(),
            );
        }
        EvidenceRefreshV1 {
            schema: EVIDENCE_REFRESH_SCHEMA_V1.into(),
            cluster: "loopback".into(),
            mode: "refresh".into(),
            plan_sha256: "aa".repeat(32),
            founding_evidence_sha256: hex_digest_v1(&founding_bytes()),
            genesis_hash: "GenesisHash11111111111111111111111111111111".into(),
            as_of_slot: 500,
            market: MARKET.into(),
            direct_execution_capability_root: ROOT.into(),
            founding_permit_capability_root: None,
            accounts,
        }
    }

    fn admit(refresh: &EvidenceRefreshV1) -> Result<BTreeMap<String, AccountEvidence>> {
        effective_accounts_v1(
            refresh,
            &founding_bytes(),
            &founding_map(),
            &"aa".repeat(32),
            ExpectedClusterV1::OwnedLoopback,
            600,
        )
    }

    /// The green direction: a well-formed refresh advances the market and
    /// appends the root, and leaves every immutable pin exactly where it was.
    #[test]
    fn admissible_refresh_advances_the_market_and_appends_the_root() {
        let effective = admit(&admissible_refresh()).expect("admissible refresh");
        assert_eq!(
            effective
                .get("founding_market")
                .expect("market")
                .data_sha256,
            "dd".repeat(32),
            "the market row must be the refreshed one"
        );
        assert_eq!(
            effective
                .get(DIRECT_EXECUTION_ROOT_LABEL_V1)
                .expect("root")
                .address,
            ROOT,
            "the execution root must be appended"
        );
        let founding = founding_map();
        for label in IMMUTABLE_FOUNDING_RECORD_LABELS_V1 {
            assert_eq!(
                effective.get(label),
                founding.get(label),
                "immutable pin {label} moved"
            );
        }
    }

    #[test]
    fn a_refresh_that_alters_an_immutable_record_is_refused() {
        for label in IMMUTABLE_FOUNDING_RECORD_LABELS_V1 {
            let mut refresh = admissible_refresh();
            refresh
                .accounts
                .get_mut(label)
                .expect("immutable row")
                .data_sha256 = "99".repeat(32);
            let error = admit(&refresh).expect_err("altered immutable record");
            assert!(
                error.to_string().contains("altered immutable founding record"),
                "{label}: {error}"
            );
        }
    }

    #[test]
    fn a_refresh_that_omits_an_immutable_record_is_refused() {
        for label in IMMUTABLE_FOUNDING_RECORD_LABELS_V1 {
            let mut refresh = admissible_refresh();
            refresh.accounts.remove(label);
            let error = admit(&refresh).expect_err("omitted immutable record");
            assert!(
                error.to_string().contains("altered immutable founding record"),
                "{label}: {error}"
            );
        }
    }

    #[test]
    fn a_refresh_naming_another_market_is_refused() {
        let mut refresh = admissible_refresh();
        let substituted = "SysvarS1otHashes111111111111111111111111111";
        refresh.market = substituted.into();
        refresh
            .accounts
            .get_mut("founding_market")
            .expect("market row")
            .address = substituted.into();
        let error = admit(&refresh).expect_err("substituted market");
        assert!(
            error
                .to_string()
                .contains("substituted the founding Market address"),
            "{error}"
        );
    }

    #[test]
    fn a_refresh_chained_to_another_founding_is_refused() {
        let mut refresh = admissible_refresh();
        refresh.founding_evidence_sha256 = "12".repeat(32);
        let error = admit(&refresh).expect_err("foreign lineage");
        assert!(
            error
                .to_string()
                .contains("not chained to this founding campaign"),
            "{error}"
        );
    }

    #[test]
    fn a_refresh_from_the_future_is_refused() {
        let mut refresh = admissible_refresh();
        refresh.as_of_slot = 601;
        let error = admit(&refresh).expect_err("future observation");
        assert!(
            error
                .to_string()
                .contains("ahead of the finalized observation"),
            "{error}"
        );
    }

    #[test]
    fn a_refresh_for_another_plan_or_cluster_is_refused() {
        let mut wrong_plan = admissible_refresh();
        wrong_plan.plan_sha256 = "bb".repeat(32);
        assert!(admit(&wrong_plan).is_err(), "another plan was admitted");
        let mut wrong_cluster = admissible_refresh();
        wrong_cluster.cluster = "devnet".into();
        assert!(admit(&wrong_cluster).is_err(), "another cluster was admitted");
        let mut wrong_mode = admissible_refresh();
        wrong_mode.mode = "preflight".into();
        assert!(admit(&wrong_mode).is_err(), "a preflight was admitted");
    }

    /// The scalars are not decoration: a document whose own header disagrees
    /// with its own rows is incoherent and is refused before any merge.
    #[test]
    fn a_refresh_whose_scalars_disagree_with_its_rows_is_refused() {
        let mut refresh = admissible_refresh();
        refresh.direct_execution_capability_root =
            "SysvarS1otHashes111111111111111111111111111".into();
        let error = admit(&refresh).expect_err("root scalar disagreement");
        assert!(error.to_string().contains("execution-root scalar"), "{error}");
    }

    /// A refresh may advance only what this design examined. Overriding some
    /// other label would be a row nobody reasoned about.
    #[test]
    fn a_refresh_carrying_an_unadmitted_label_is_refused() {
        let mut refresh = admissible_refresh();
        refresh.accounts.insert(
            "founding_hoard_vault".to_string(),
            row(MARKET, &"77".repeat(32)),
        );
        let error = admit(&refresh).expect_err("unadmitted label");
        assert!(error.to_string().contains("unadmitted label"), "{error}");
    }

    /// The general class, not one cause. A market advanced only by ADMISSION —
    /// no activation, so no execution root to append — refreshes just as well
    /// as one advanced by activation. The wall was reached from both
    /// directions, and the bridge has to carry both.
    #[test]
    fn a_market_advanced_only_by_admission_refreshes_without_a_root() {
        let mut refresh = admissible_refresh();
        refresh.accounts.remove(DIRECT_EXECUTION_ROOT_LABEL_V1);
        // Admission moved the Claims trio; the Market itself moved too.
        for label in ["claims_admission", "claims_aggregate", "founder_position"] {
            refresh
                .accounts
                .insert(label.to_string(), row(MARKET, &"44".repeat(32)));
        }
        let effective = admit(&refresh).expect("admission-only refresh");
        assert_eq!(
            effective
                .get("claims_admission")
                .expect("admission")
                .data_sha256,
            "44".repeat(32),
            "the byte-pinned claims_admission row must be the refreshed one"
        );
        assert!(
            !effective.contains_key(DIRECT_EXECUTION_ROOT_LABEL_V1),
            "a market that never activated must not acquire a root row"
        );
    }

    /// Every advanceable label is genuinely advanceable: none of them may be
    /// silently frozen back to its founding value by this admission path.
    #[test]
    fn each_advanceable_label_may_differ_from_founding() {
        for label in ADVANCEABLE_FOUNDING_LABELS_V1 {
            let mut refresh = admissible_refresh();
            let address = refresh
                .accounts
                .get(label)
                .map(|existing| existing.address.clone())
                .unwrap_or_else(|| MARKET.to_string());
            refresh
                .accounts
                .insert(label.to_string(), row(&address, &"55".repeat(32)));
            let effective = admit(&refresh)
                .unwrap_or_else(|error| panic!("advanceable label {label} refused: {error}"));
            assert_eq!(
                effective.get(label).expect("row").data_sha256,
                "55".repeat(32),
                "{label} did not advance"
            );
        }
    }

    #[test]
    fn the_parser_refuses_an_empty_or_oversized_document() {
        assert!(parse_refresh_v1(b"").is_err(), "empty document admitted");
        let oversized = vec![b' '; MAX_REFRESH_BYTES_V1 + 1];
        assert!(
            parse_refresh_v1(&oversized).is_err(),
            "oversized document admitted"
        );
    }

    #[test]
    fn the_two_roots_have_distinct_names_in_the_schema() {
        let refresh = admissible_refresh();
        let rendered = serde_json::to_string(&refresh).expect("render");
        assert!(
            rendered.contains("direct_execution_capability_root")
                && rendered.contains("founding_permit_capability_root"),
            "the schema must never spell the two roots the same way: {rendered}"
        );
    }

    #[test]
    fn the_loopback_entry_refuses_a_devnet_acknowledgment() {
        let error = parse_arguments(
            vec![
                "--rpc-url".into(),
                "http://127.0.0.1:8899/".into(),
                "--i-mean-devnet".into(),
                DEVNET_GENESIS_HASH.into(),
            ],
            ExpectedClusterV1::OwnedLoopback,
        )
        .expect_err("loopback acknowledgment");
        assert!(error.to_string().contains("needs no acknowledgment"), "{error}");
    }
}
