//! The checked-mutable loopback substrate, brought up in the order the park
//! banner prescribes: prepare-mutable -> spawn -> authenticate -> campaign
//! through activation. Only then does a market compiler exist to call.
//!
//! Everything here mirrors the executed private-validator-lifecycle driver
//! (tools/release/private-validator-lifecycle/run.py) rather than the tier-1
//! supervisor: `found_through_open`'s plan validator and the checked-mutable
//! plan are unsatisfiable together (five-stage revoke-true vs four-stage
//! revoke-false), and the tier-1 launcher hard-refuses
//! `release_recognition_requires_revoke == false` — so the substrate spawns
//! `solana-test-validator` over the prepared account directory directly, with
//! NO `--upgradeable-program` (which would replace the prepared tag-0
//! authority with Agave's default), exactly as run.py:1292 documents.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use sha2::Digest;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};

use crate::campaign::{self, CampaignArgsV1, CampaignModeV1, StageV1};
use crate::cluster::ClusterOriginV1;
use crate::local_mutable;
use crate::market::MarketExecutionEvidence;
use crate::model::{SuccessorPlan, TransactionEvidence};
use crate::plan::pubkey;
use crate::rpc::Rpc;
use crate::{Error, Result};

/// The prepared deployment slots are 1..=7; the validator must have finalized
/// past them before a checked plan snapshot is coherent.
const CHECKED_SLOT_FLOOR: u64 = 8;

const VALIDATOR_READY: Duration = Duration::from_secs(90);

/// Everything the checked-mutable bring-up needs from the caller.
pub(crate) struct SubstrateRequestV1<'a> {
    pub(crate) work: &'a Path,
    pub(crate) checked_release_gate: &'a Path,
    pub(crate) expected_gate_sha256: &'a str,
    pub(crate) expected_source_revision: &'a str,
    pub(crate) expected_source_tree_sha256: &'a str,
    pub(crate) seed: &'a str,
    pub(crate) rpc_port: u16,
}

/// A live validator carrying the checked-mutable substrate, plus the plan and
/// key material the campaigns need. Dropping this kills the validator.
pub(crate) struct CheckedSubstrateV1 {
    pub(crate) validator: ValidatorGuardV1,
    pub(crate) rpc_url: String,
    pub(crate) plan_path: PathBuf,
    pub(crate) plan: SuccessorPlan,
    pub(crate) plan_sha256: String,
    pub(crate) report: local_mutable::LocalMutablePrepareReportV1,
}

/// Owns the spawned validator's lifetime.
pub(crate) struct ValidatorGuardV1 {
    child: Child,
}

impl Drop for ValidatorGuardV1 {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// Load one Solana-convention keypair file.
pub(crate) fn load_keypair(path: &Path) -> Result<Keypair> {
    let bytes: Vec<u8> = serde_json::from_slice(&std::fs::read(path)?)?;
    Keypair::try_from(bytes.as_slice())
        .map_err(|error| Error::new(format!("keypair {}: {error}", path.display())))
}

fn role_key_path(
    map: &BTreeMap<String, String>,
    role: &str,
    which: &'static str,
) -> Result<PathBuf> {
    map.get(role)
        .map(PathBuf::from)
        .ok_or_else(|| Error::new(format!("the prepare report's {which} map omits role {role}")))
}

/// prepare-mutable -> spawn -> wait -> authenticate -> campaign activation.
pub(crate) fn bring_up(request: &SubstrateRequestV1<'_>) -> Result<CheckedSubstrateV1> {
    // 1. Prepare: derive keys and the exact genesis account fixtures from the
    //    checked release gate. In-process; the typed report comes back.
    let prepare_work = request.work.join("prepare");
    let plan_path = request.work.join("plan.json");
    let report = local_mutable::prepare_local_mutable_v1(vec![
        "--work".into(),
        prepare_work.display().to_string(),
        "--output".into(),
        plan_path.display().to_string(),
        "--checked-release-gate".into(),
        request.checked_release_gate.display().to_string(),
        "--expected-checked-release-gate-sha256".into(),
        request.expected_gate_sha256.to_owned(),
        "--expected-source-revision".into(),
        request.expected_source_revision.to_owned(),
        "--expected-source-tree-sha256".into(),
        request.expected_source_tree_sha256.to_owned(),
        "--seed".into(),
        request.seed.to_owned(),
    ])?;
    let plan_bytes = std::fs::read(&plan_path)?;
    let plan_sha256 = crate::plan::hex(&sha2::Sha256::digest(&plan_bytes));
    let plan: SuccessorPlan = serde_json::from_slice(&plan_bytes)?;
    local_mutable::authenticate_checked_local_mutable_plan_v1(&plan)?;

    // 2. Spawn the validator over the prepared account directory. The mint is
    //    the retained-authority key, exactly as the lifecycle driver does it,
    //    so the campaign bankroll exists at genesis; the faucet serves the
    //    airdrops the walks use.
    let mint = load_keypair(&role_key_path(
        &report.keypairs,
        "core-upgrade-authority",
        "keypairs",
    )?)?;
    let ledger = request.work.join("ledger");
    let log = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(request.work.join("validator.log"))?;
    let log_err = log.try_clone()?;
    let port = request.rpc_port;
    let child = Command::new("solana-test-validator")
        .arg("--config")
        .arg("/dev/null")
        .arg("--ledger")
        .arg(&ledger)
        .arg("--account-dir")
        .arg(&report.account_dir)
        .arg("--mint")
        .arg(mint.pubkey().to_string())
        .arg("--ticks-per-slot")
        .arg("16")
        .arg("--bind-address")
        .arg("127.0.0.1")
        .arg("--rpc-port")
        .arg(port.to_string())
        .arg("--faucet-port")
        .arg((port + 2).to_string())
        .arg("--gossip-port")
        .arg((port + 3).to_string())
        .arg("--dynamic-port-range")
        .arg(format!("{}-{}", port + 10, port + 41))
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err))
        .spawn()
        .map_err(|error| Error::new(format!("spawn solana-test-validator: {error}")))?;
    let mut validator = ValidatorGuardV1 { child };
    let rpc_url = format!("http://127.0.0.1:{port}/");

    // 3. Wait for health AND for finality past the prepared deployment slots,
    //    refusing early if the child dies.
    let deadline = Instant::now() + VALIDATOR_READY;
    loop {
        if let Some(status) = validator
            .child
            .try_wait()
            .map_err(|error| Error::new(format!("poll validator: {error}")))?
        {
            return Err(Error::new(format!(
                "solana-test-validator exited during bring-up: {status}; see {}",
                request.work.join("validator.log").display()
            )));
        }
        if let Ok(mut probe) = Rpc::connect(&rpc_url)
            && let Ok(slot) = probe.finalized_slot()
            && slot >= CHECKED_SLOT_FLOOR
        {
            break;
        }
        if Instant::now() >= deadline {
            return Err(Error::new(format!(
                "validator at {rpc_url} did not reach finalized slot {CHECKED_SLOT_FLOOR} within \
                 90 seconds"
            )));
        }
        std::thread::sleep(Duration::from_millis(250));
    }

    // 4. Administration: publish -> initialize -> activate, through the real
    //    campaign, signed only by the retained authority.
    campaign::execute(CampaignArgsV1 {
        origin: ClusterOriginV1::parse(&rpc_url, None)?,
        mode: CampaignModeV1::Administration,
        plan_path: plan_path.clone(),
        market_path: None,
        evidence_path: Some(request.work.join("administration-evidence.json")),
        // The vertical asks for no standalone lineage artifact: `main.rs` is the
        // only caller that wants one, and asking here would add a second owner
        // for infrastructure facts this substrate does not publish.
        infrastructure_lineage_path: None,
        founding_founder: None,
        substituted_founder: None,
        keypairs: BTreeMap::from([(
            "core-upgrade-authority".to_owned(),
            role_key_path(&report.keypairs, "core-upgrade-authority", "keypairs")?,
        )]),
        execute: true,
        through: StageV1::Activation,
    })?;

    Ok(CheckedSubstrateV1 {
        validator,
        rpc_url,
        plan_path,
        plan,
        plan_sha256,
        report,
    })
}

/// The founding roles a fixture-liquidity-free market signs with. The
/// graduation market carries zero fixture liquidity, so the two fixture roles
/// (participant, direct-buyer) the prepare report also derives are refused by
/// `campaign --founding-only` as outside this mode.
const FOUNDING_ROLES_V1: &[&str] = &[
    "campaign-payer",
    "collateral-mint",
    "collateral-wallet",
    "founding-beneficiary",
    "founding-projection-witness",
    "founding-source-funder",
];

/// What the founding campaign left behind, lifted into the session's shape.
pub(crate) struct FoundingYieldV1 {
    pub(crate) transactions: Vec<TransactionEvidence>,
    pub(crate) market: MarketExecutionEvidence,
}

/// Fund the founding keys, run `campaign --founding-only`, lift the evidence.
pub(crate) fn found_market(
    substrate: &CheckedSubstrateV1,
    rpc: &mut Rpc,
    market_path: &Path,
    evidence_path: &Path,
) -> Result<FoundingYieldV1> {
    let report = &substrate.report;
    // The campaign never airdrops; the driver funds. ONLY the campaign payer
    // is funded — the other five founding roles are protocol-created and MUST
    // be vacant, or the founding reads a pre-funded system account at the
    // collateral-mint address as a started-and-unresumable founding (the
    // lifecycle driver pins exactly this: PROTOCOL_CREATED_KEY_ROLES stay
    // vacant, only campaign-payer gets the bankroll). The campaign still needs
    // every role's key FILE to sign, so all six paths are handed through; the
    // two fixture roles the prepare report also derives are excluded, since
    // this fixture-liquidity-free founding refuses them as outside its mode.
    let mut founding_keys: BTreeMap<String, PathBuf> = BTreeMap::new();
    for role in FOUNDING_ROLES_V1 {
        let path = report
            .campaign_founding_keypairs
            .get(*role)
            .ok_or_else(|| Error::new(format!("prepare report omits founding role {role}")))?;
        if *role == "campaign-payer" {
            let keypair = load_keypair(Path::new(path))?;
            rpc.airdrop(
                "relayed vertical: fund the campaign payer",
                keypair.pubkey(),
                500_000_000_000,
            )?;
        }
        founding_keys.insert((*role).to_owned(), PathBuf::from(path));
    }
    let founder = report
        .campaign_public_identities
        .get("founding-founder")
        .ok_or_else(|| Error::new("prepare report names no founding-founder identity"))?;
    let substituted = report
        .campaign_public_identities
        .get("substituted-founder")
        .ok_or_else(|| Error::new("prepare report names no substituted-founder identity"))?;
    campaign::execute(CampaignArgsV1 {
        origin: ClusterOriginV1::parse(&substrate.rpc_url, None)?,
        mode: CampaignModeV1::FoundingOnly,
        plan_path: substrate.plan_path.clone(),
        market_path: Some(market_path.to_path_buf()),
        evidence_path: Some(evidence_path.to_path_buf()),
        // Founding refuses the flag outright (`campaign.rs:3479-3487`).
        infrastructure_lineage_path: None,
        founding_founder: Some(pubkey(founder)?),
        substituted_founder: Some(pubkey(substituted)?),
        keypairs: founding_keys,
        execute: true,
        through: StageV1::Founding,
    })?;

    let evidence: serde_json::Value = serde_json::from_slice(&std::fs::read(evidence_path)?)?;
    let execution = evidence
        .get("execution")
        .ok_or_else(|| Error::new("founding evidence carries no execution section"))?;
    let transactions: Vec<TransactionEvidence> = serde_json::from_value(
        execution
            .get("transactions")
            .cloned()
            .ok_or_else(|| Error::new("founding evidence carries no transactions"))?,
    )?;
    let market: MarketExecutionEvidence = serde_json::from_value(
        execution
            .get("market")
            .cloned()
            .ok_or_else(|| Error::new("founding evidence carries no market execution"))?,
    )?;
    Ok(FoundingYieldV1 {
        transactions,
        market,
    })
}

/// The retained-authority keypair: the substrate's bankroll and the session's
/// signing authority, exactly the role `found_through_open` used.
pub(crate) fn authority_keypair(substrate: &CheckedSubstrateV1) -> Result<Keypair> {
    load_keypair(&role_key_path(
        &substrate.report.keypairs,
        "core-upgrade-authority",
        "keypairs",
    )?)
}

/// A well-known Pubkey helper for report identities.
pub(crate) fn report_pubkey(value: &str, label: &str) -> Result<Pubkey> {
    pubkey(value).map_err(|_| Error::new(format!("{label} is not a canonical address: {value}")))
}
