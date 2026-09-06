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
    map.get(role).map(PathBuf::from).ok_or_else(|| {
        Error::new(format!(
            "the prepare report's {which} map omits role {role}"
        ))
    })
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
        // A LONG RUN NEEDS ITS HISTORY, and this tier was running without it.
        //
        // `solana-test-validator` keeps `--limit-ledger-size` shreds in root
        // slots and DEFAULTS TO 10,000 -- roughly seven hundred slots of a
        // journey, against a run that reaches nine thousand. Past that the
        // roots are purged in chunks, and `getTransaction` answers null for a
        // signature `getSignatureStatuses --searchTransactionHistory` still
        // calls finalized. Every driver here re-verifies its earlier stages
        // from history, so a purge between two stages strands the journal
        // permanently and no retry recovers it: hbox `20260906T140439Z` lost a
        // Direct fill to `finalized signature omitted finalized transaction
        // history` after its admission had landed, and JOURNEY-6 read the same
        // purge as a property of the substrate ("null ~750 slots back") and
        // derived the founding's routing table around it.
        //
        // `docs/evidence/DIRECT_FILL_WALLS_2026_08_31.md` named this in August
        // and `tools/local-validator/dclutch-successor-validator` has passed the
        // flag since; this launcher never did. Same knob, same spelling, same
        // default. Budget about 470 KB per slot, measured there -- a journey's
        // ledger lives under the run's own work directory and goes with it.
        .arg("--limit-ledger-size")
        .arg(std::env::var("DCLUTCH_LIMIT_LEDGER_SIZE").unwrap_or_else(|_| "100000000".to_owned()))
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
    //    campaign.
    //
    // THE SUCCESSION STAGE HAS A SECOND SIGNER, and this bring-up did not know
    // it. `administration_required_roles_v1` adds `campaign-payer` whenever the
    // Succession stage is Absent or Partial and inside `--through`, which on a
    // freshly prepared substrate is always; the required set is ALSO the
    // allowed set, so the payer is neither optional nor safe to pass at another
    // time. A bring-up that named only the retained authority therefore refused
    // "campaign omitted required keypair paths: campaign-payer" before sending
    // anything -- measured 2026-09-04 by the `ladder` tier, which links this
    // file rather than forking it and hit the refusal on its first run.
    //
    // The payer starts empty (the genesis mint is the retained authority), so
    // the driver funds it here, exactly as `found_market` below funds it for
    // the founding. The campaign never airdrops.
    let campaign_payer_path = report
        .campaign_founding_keypairs
        .get("campaign-payer")
        .map(PathBuf::from)
        .ok_or_else(|| Error::new("prepare report omits founding role campaign-payer"))?;
    {
        let mut funding = Rpc::connect(&rpc_url)?;
        let payer = load_keypair(&campaign_payer_path)?;
        funding.airdrop(
            "substrate: fund the campaign payer for the succession stage",
            payer.pubkey(),
            500_000_000_000,
        )?;
    }
    campaign::execute(CampaignArgsV1 {
        origin: ClusterOriginV1::parse(&rpc_url, None)?,
        mode: CampaignModeV1::Administration,
        // This vertical founds fresh substrates and never resumes one, so the
        // repair mode is off at both call sites. It is a MODE and not a
        // fallback (`campaign.rs:950-958`): an unmarked resume must not emit a
        // report that reads like a fresh founding.
        recover_finalized_founding: false,
        plan_path: plan_path.clone(),
        market_path: None,
        evidence_path: Some(request.work.join("administration-evidence.json")),
        // The vertical asks for no standalone lineage artifact: `main.rs` is the
        // only caller that wants one, and asking here would add a second owner
        // for infrastructure facts this substrate does not publish.
        infrastructure_lineage_path: None,
        founding_founder: None,
        substituted_founder: None,
        keypairs: BTreeMap::from([
            (
                "core-upgrade-authority".to_owned(),
                role_key_path(&report.keypairs, "core-upgrade-authority", "keypairs")?,
            ),
            ("campaign-payer".to_owned(), campaign_payer_path),
        ]),
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

/// The founding roles EVERY market signs with.
///
/// The two fixture roles the prepare report also derives -- `participant` and
/// `direct-buyer` -- are not here because they are not universal: they are
/// required exactly when the market being founded carries local participant
/// fixture liquidity, and `campaign --founding-only` refuses them on a market
/// that does not ("outside this mode") and refuses their ABSENCE on a market
/// that does ("local participant fixture liquidity requires
/// --keypair-participant"). So the role set is a function of the market, which
/// is what [`founding_roles_for`] makes it. The graduation market carries zero;
/// the loopback demo market carries 100,000,000 atoms.
const FOUNDING_ROLES_V1: &[&str] = &[
    "campaign-payer",
    "collateral-mint",
    "collateral-wallet",
    "founding-beneficiary",
    "founding-projection-witness",
    "founding-source-funder",
];

/// The two roles a market with fixture liquidity additionally signs with.
const FIXTURE_LIQUIDITY_ROLES_V1: &[&str] = &["participant", "direct-buyer"];

/// The roles this market's founding needs, read off the market itself.
fn founding_roles_for(market: &crate::model::MarketRunInput) -> Vec<&'static str> {
    let mut roles = FOUNDING_ROLES_V1.to_vec();
    if market.local_participant_fixture_liquidity_atoms != 0 {
        roles.extend_from_slice(FIXTURE_LIQUIDITY_ROLES_V1);
    }
    roles
}

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
    // The market this founding is about decides which roles sign it. Reading
    // the input rather than pinning a list is what lets one bring-up found the
    // liquidity-free graduation market AND a demo market that carries fixture
    // liquidity, instead of one of them refusing by the other's name.
    // Two spellings reach this function: a bare `MarketRunInput`, and the
    // graduation wrapper that carries one under `market`. Reading either is not
    // a second authority -- the campaign authenticates whichever it was given;
    // this only asks the input how much fixture liquidity it declares.
    let market_bytes = std::fs::read(market_path)?;
    let market_input: crate::model::MarketRunInput =
        match serde_json::from_slice::<crate::model::MarketRunInput>(&market_bytes) {
            Ok(input) => input,
            Err(_) => {
                let wrapper: serde_json::Value = serde_json::from_slice(&market_bytes)?;
                serde_json::from_value(wrapper.get("market").cloned().ok_or_else(|| {
                    Error::new(
                        "the market input is neither a MarketRunInput nor a wrapper carrying one \
                         under `market`",
                    )
                })?)?
            }
        };
    let roles = founding_roles_for(&market_input);
    let mut founding_keys: BTreeMap<String, PathBuf> = BTreeMap::new();
    for role in &roles {
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
        recover_finalized_founding: false,
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
/// The key the founding's collateral wallet is owned BY.
///
/// `found_market` runs `campaign --founding-only` under the `campaign-payer`
/// role, and the founding's own suffix-resume check is
/// `wallet.owner == payer.pubkey()` -- so the founder's collateral wallet
/// answers to this key and to no other. It is NOT `authority_keypair`, which
/// is the `core-upgrade-authority` role a campaign signs its administration
/// with, and a caller that wanted to move a founded market's collateral and
/// reached for the authority got Token-2022's `OwnerMismatch` from the chain.
pub(crate) fn campaign_payer_keypair(substrate: &CheckedSubstrateV1) -> Result<Keypair> {
    load_keypair(&role_key_path(
        &substrate.report.campaign_founding_keypairs,
        "campaign-payer",
        "campaign founding keypairs",
    )?)
}

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
