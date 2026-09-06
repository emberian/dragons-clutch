//! The four stages the gap register used to state as walls: admission, fill,
//! redemption and retirement.
//!
//! # What this file is, and what it deliberately is not
//!
//! It is NOT a second implementation of any of them. Every act below is the
//! SHIPPED driver a host runs, called with the argument vector a host would
//! type, in this process, against the validator this campaign stood up:
//!
//! | stage | shipped command | entry called here |
//! |---|---|---|
//! | admission | `local-private-validator-user-position-admission-v1` | [`crate::user_position_admission::run_owned_loopback`] |
//! | fill | `local-private-validator-direct-trade-produce-v1` then `local-private-validator-direct-trade-v1` | [`crate::direct_trade_producer::run_owned_loopback`], [`crate::direct_trade::run_owned_loopback`] |
//! | fill (fee) | `local-private-validator-direct-fee-settlement-v1` | [`crate::direct_fee_settlement::run_owned_loopback_v1`] |
//! | redemption | `local-private-validator-wallet-terminal-payout-input-v1` then `local-private-validator-wallet-terminal-payout-v1` | [`crate::terminal_lifecycle::run_wallet_terminal_input_owned_loopback_v1`], [`crate::wallet_terminal_payout_exterior::run`] |
//! | retirement | `local-private-validator-refresh-evidence-v1`, `local-private-validator-terminal-sequence-v1`, `local-private-validator-aggregate-retirement-v1` | [`crate::evidence_refresh::run_owned_loopback`], [`crate::terminal_sequence::run_terminal_sequence_owned_loopback_v1`], [`crate::aggregate_retirement_exterior::run_owned_loopback`] |
//!
//! A tier that rebuilt any of those frames would be measuring a second author,
//! not the driver a host runs. That is the lesson `tools/gauntlet/ladder/`
//! wrote down when it called `recovery_crank::run_v1` with a vector instead of
//! constructing an 18-account frame, and it is the whole design of this file.
//!
//! # Why the stages are RESUMPTION LOOPS rather than calls
//!
//! Three of these drivers advance **exactly one durable action per
//! invocation** and are re-run until their completion file exists — that is
//! their crash-safety contract, not an inconvenience. `direct_trade` walks
//! replay-setup, token-setup, four lookup acts, the capability seal and the
//! Hot execution; `wallet_terminal_payout_exterior` walks four lookup acts and
//! the payout; `terminal_sequence` walks CoreBeginRetiring, DirectBeginRetiring,
//! ResolutionCloseFund, DirectCloseCapability, RetirementReplayHandoff and
//! AggregateRetirement; `aggregate_retirement_exterior` walks the four
//! checkpoint packets. So each stage here is a BOUNDED loop with a stated
//! ceiling, and a stage that hits its ceiling reports how far it got rather
//! than looping forever or claiming completion.
//!
//! # Findings, not stops
//!
//! A refused stage is recorded with the driver's own sentence and the journey
//! continues to the next one, exactly as the Pyth provider leg already does.
//! The transcript then says which walls a real chain put up, in the order it
//! put them up, which is worth more than a campaign that dies at the first.

use std::path::{Path, PathBuf};

use serde_json::Value;
use solana_sdk::{pubkey::Pubkey, signature::Signature};

use crate::model::TransactionEvidence;
use crate::rpc::Rpc;
use crate::stages::StageReportV1;
use crate::{Error, Result};

/// How many times a resumption loop may re-enter one driver.
///
/// Every driver below advances at most one durable action per invocation, so
/// this is a bound on ACTS and not on retries. The longest chain in the table
/// is `direct_trade`'s eight, and `terminal_sequence` can add four lookup acts
/// in front of its six protocol stages; twenty-four is comfortably above both
/// and small enough that a driver stuck on one action is REPORTED within a
/// minute rather than spun on.
const RESUMPTION_CEILING_V1: usize = 24;

/// What the spine did, in the shape the journey folds.
pub(crate) struct SpineV1 {
    pub(crate) stages: Vec<StageReportV1>,
    /// Every transaction a shipped driver landed, re-read from the chain with
    /// its finalized logs so the census can corroborate the program that ran.
    pub(crate) transactions: Vec<TransactionEvidence>,
    /// A stage that was supposed to execute and refused, with the driver's own
    /// sentence. The journey fails at the end on a non-empty list.
    pub(crate) refusals: Vec<String>,
    /// One machine-readable row per stage, for the transcript.
    pub(crate) reports: serde_json::Map<String, Value>,
}

impl SpineV1 {
    pub(crate) fn new() -> Self {
        Self {
            stages: Vec::new(),
            transactions: Vec::new(),
            refusals: Vec::new(),
            reports: serde_json::Map::new(),
        }
    }

    fn executed(&mut self, stage: &str, transactions: usize, compute: u64, note: String) {
        self.stages.push(StageReportV1 {
            stage: stage.into(),
            outcome: "executed".into(),
            transactions,
            compute_units: compute,
            note,
        });
    }

    fn refused(&mut self, stage: &str, error: &str, note: String) {
        self.refusals.push(format!("{stage} -- {error}"));
        self.stages.push(StageReportV1 {
            stage: stage.into(),
            outcome: "refused".into(),
            transactions: 0,
            compute_units: 0,
            note,
        });
    }
}

/// Everything a spine stage needs that the founding produced.
pub(crate) struct SpineContextV1<'a> {
    pub(crate) rpc_url: &'a str,
    /// The checked-mutable plan `local-mutable-prepare-v1` wrote.
    pub(crate) plan: &'a Path,
    /// `campaign --founding-only`'s own report. Every driver in the table
    /// authenticates this document, and three of them refuse a report whose
    /// `cluster` is not `loopback`, so there is no way to hand one of these
    /// commands a devnet founding by accident.
    pub(crate) campaign_report: &'a Path,
    /// The compiled `MarketRunInput` this campaign founded.
    pub(crate) market_input: &'a Path,
    pub(crate) market: Pubkey,
    /// This stage's own scratch root, under the run directory.
    pub(crate) work: &'a Path,
    /// The prepare stage's role key files, by role name.
    pub(crate) keypairs: &'a std::collections::BTreeMap<String, String>,
    /// The prepare stage's founding role key files, by role name.
    pub(crate) founding_keypairs: &'a std::collections::BTreeMap<String, String>,
}

impl SpineContextV1<'_> {
    fn key(&self, role: &str) -> Result<PathBuf> {
        self.founding_keypairs
            .get(role)
            .or_else(|| self.keypairs.get(role))
            .map(PathBuf::from)
            .ok_or_else(|| {
                Error::new(format!(
                    "the prepare report names no key file for role `{role}`; the spine signs with \
                     the substrate's own disposable roles and never mints one"
                ))
            })
    }

    fn dir(&self, name: &str) -> Result<PathBuf> {
        let path = self.work.join(name);
        std::fs::create_dir_all(&path)?;
        Ok(path)
    }
}

/// Re-read one signature the chain finalized, with its logs.
///
/// The drivers report a signature and a compute figure in their own documents,
/// and this does not take their word for either: `finalized_signed_packet`
/// re-derives the packet, verifies its signatures against the message, and
/// returns the chain's own `TransactionEvidence`. That is what makes these
/// transactions admissible to `census observe`, which cross-checks every
/// claimed route against the chain's `Program <address> invoke [n]` lines --
/// a driver's own JSON has no logs in it and could not be corroborated.
fn harvest(rpc: &mut Rpc, label: &str, signature: &str, into: &mut Vec<TransactionEvidence>) {
    let Ok(parsed) = signature.parse::<Signature>() else {
        return;
    };
    if into
        .iter()
        .any(|evidence| evidence.signature == signature)
    {
        return;
    }
    if let Ok(Some(finalized)) = rpc.finalized_signed_packet(label, parsed, false) {
        let mut evidence = finalized.evidence;
        evidence.label = label.to_owned();
        into.push(evidence);
    }
}

/// Every string under `key` anywhere in a driver's document, in document order.
///
/// The four drivers spell their landed signature in four places -- `signature`,
/// `landed.signature`, `journals[].signature`, `mutations[].signature` -- and
/// a harvester that named each path would be a fifth author of the same fact
/// and would go stale the first time a driver grew a journal. Walking for the
/// KEY instead is stable under that change, and every candidate is then
/// checked against the chain, so a string that is not a real signature is
/// dropped rather than believed.
fn signatures_in(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if key == "signature" || key == "expectedSignature" || key == "landedSignature" {
                    if let Some(text) = child.as_str() {
                        out.push(text.to_owned());
                    }
                }
                signatures_in(child, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                signatures_in(item, out);
            }
        }
        _ => {}
    }
}

fn read_json(path: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}

/// Harvest every signature a driver's document names, and total the compute the
/// chain reports for them.
fn harvest_document(
    rpc: &mut Rpc,
    label: &str,
    document: &Value,
    into: &mut Vec<TransactionEvidence>,
) -> (usize, u64) {
    let mut signatures = Vec::new();
    signatures_in(document, &mut signatures);
    let before = into.len();
    for signature in signatures {
        harvest(rpc, label, &signature, into);
    }
    let landed = &into[before..];
    (
        landed.len(),
        landed
            .iter()
            .map(|evidence| evidence.compute_units_consumed.unwrap_or(0))
            .sum(),
    )
}

/// Harvest every driver document under a directory (a journal directory).
fn harvest_dir(
    rpc: &mut Rpc,
    label: &str,
    dir: &Path,
    into: &mut Vec<TransactionEvidence>,
) -> (usize, u64) {
    let mut total = (0_usize, 0_u64);
    let Ok(entries) = std::fs::read_dir(dir) else {
        return total;
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            let nested = harvest_dir(rpc, label, &path, into);
            total = (total.0 + nested.0, total.1 + nested.1);
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let Ok(document) = read_json(&path) else {
            continue;
        };
        // ONE LABEL PER JOURNAL FILE, not one per stage. A stage's journal
        // directory holds transactions that invoke DIFFERENT programs -- the
        // four routing-table acts are the Address Lookup Table program's, the
        // seal and the Hot execution are Trading's -- and `census observe`
        // refuses an observation whose finalized logs do not show the bound
        // program invoked. A single stage-wide label could therefore never be
        // bound honestly; the journal's own file stem is the act's name and is
        // what a binding can be written against.
        let stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("journal");
        let scoped = format!("{label}: {stem}");
        let one = harvest_document(rpc, &scoped, &document, into);
        total = (total.0 + one.0, total.1 + one.1);
    }
    total
}

/// Drive one resumption loop to its completion file.
///
/// `completed` is asked BEFORE each invocation as well as after, so a stage
/// whose completion file already exists costs zero transactions and reports
/// `already complete` rather than re-entering a driver that would refuse.
fn resume_until<F, C>(mut invoke: F, mut completed: C) -> std::result::Result<usize, (usize, String)>
where
    F: FnMut(usize) -> Result<()>,
    C: FnMut() -> bool,
{
    for pass in 0..RESUMPTION_CEILING_V1 {
        if completed() {
            return Ok(pass);
        }
        if let Err(error) = invoke(pass) {
            return Err((pass, error.to_string()));
        }
    }
    if completed() {
        return Ok(RESUMPTION_CEILING_V1);
    }
    Err((
        RESUMPTION_CEILING_V1,
        format!(
            "the driver did not reach its completion file within {RESUMPTION_CEILING_V1} \
             invocations; each invocation advances at most one durable action, so this is a \
             stalled action rather than a slow one"
        ),
    ))
}

// ----------------------------------------------------------------- admission

/// One stranger this campaign admits a protocol Position to.
pub(crate) struct StrangerV1 {
    pub(crate) label: String,
    pub(crate) owner: Pubkey,
    pub(crate) keypair: PathBuf,
    /// The collateral leg, when this stranger is the one who will BUY. The
    /// admission command takes its four collateral flags together or not at
    /// all, so this is one option rather than four.
    pub(crate) collateral: Option<StrangerCollateralV1>,
    pub(crate) report: PathBuf,
}

pub(crate) struct StrangerCollateralV1 {
    pub(crate) source_owner: Pubkey,
    pub(crate) source_owner_keypair: PathBuf,
    pub(crate) source_account: Pubkey,
    pub(crate) quantity_atoms: u64,
}

/// Admit each stranger a protocol Position through the shipped command.
///
/// `trading/user_position_admission_v1::process_user_position_admission_v1#Admit`
/// is `unwired` in `tools/gauntlet/blocked.json` with the reason "driven today,
/// and invisible to the census for a wiring reason": a ProgramTest lifecycle
/// drives it and emits no evidence. This is the other half -- the same route,
/// on a validator, with the chain's own logs behind it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn admit_strangers(
    rpc: &mut Rpc,
    context: &SpineContextV1<'_>,
    spine: &mut SpineV1,
    strangers: &[StrangerV1],
    fee_payer: Pubkey,
    fee_payer_keypair: &Path,
    routing_tables: &[Pubkey],
) -> Result<()> {
    let stage = "post-open life: two strangers are admitted a protocol Position";
    let minimum_slot = rpc.finalized_slot()?.max(1);
    let mut landed = 0_usize;
    let mut compute = 0_u64;
    let mut rows = serde_json::Map::new();
    for stranger in strangers {
        let mut arguments = vec![
            "--rpc-url".to_owned(),
            context.rpc_url.to_owned(),
            "--plan".to_owned(),
            context.plan.display().to_string(),
            "--campaign-evidence".to_owned(),
            context.campaign_report.display().to_string(),
            "--position-owner".to_owned(),
            stranger.owner.to_string(),
            "--position-owner-keypair".to_owned(),
            stranger.keypair.display().to_string(),
            "--fee-payer".to_owned(),
            fee_payer.to_string(),
            "--fee-payer-keypair".to_owned(),
            fee_payer_keypair.display().to_string(),
            "--minimum-finalized-slot".to_owned(),
            minimum_slot.to_string(),
            "--output".to_owned(),
            stranger.report.display().to_string(),
            "--execute".to_owned(),
        ];
        if let Some(collateral) = &stranger.collateral {
            arguments.extend([
                "--collateral-source-owner".to_owned(),
                collateral.source_owner.to_string(),
                "--collateral-source-owner-keypair".to_owned(),
                collateral.source_owner_keypair.display().to_string(),
                "--collateral-source-account".to_owned(),
                collateral.source_account.to_string(),
                "--collateral-quantity-atoms".to_owned(),
                collateral.quantity_atoms.to_string(),
            ]);
        }
        if !routing_tables.is_empty() {
            // ONE value, comma separated: the command refuses a repeated flag
            // and refuses a repeated table inside the value.
            arguments.extend([
                "--routing-table".to_owned(),
                routing_tables
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            ]);
        }
        // The admission command's `--output` doubles as its crash journal, and
        // it resumes from a partial one rather than refusing, so a stranger
        // whose first pass died mid-dispatch is finished by the next pass.
        let label = format!("journey admission: {}", stranger.label);
        let outcome = resume_until(
            |_| crate::user_position_admission::run_owned_loopback(arguments.clone()),
            || {
                read_json(&stranger.report)
                    .ok()
                    .and_then(|document| {
                        document
                            .get("phase")
                            .and_then(Value::as_str)
                            .map(|phase| phase == "finalized")
                    })
                    .unwrap_or(false)
            },
        );
        match outcome {
            Ok(passes) => {
                let document = read_json(&stranger.report)?;
                let (count, units) = harvest_document(rpc, &label, &document, &mut spine.transactions);
                landed += count;
                compute += units;
                rows.insert(
                    stranger.label.clone(),
                    serde_json::json!({
                        "outcome": "executed",
                        "passes": passes,
                        "report": stranger.report.display().to_string(),
                        "landed": count,
                        "computeUnitsConsumed": units,
                        "collateral": stranger.collateral.as_ref().map(|value| serde_json::json!({
                            "sourceAccount": value.source_account.to_string(),
                            "quantityAtoms": value.quantity_atoms,
                        })),
                    }),
                );
            }
            Err((passes, error)) => {
                rows.insert(
                    stranger.label.clone(),
                    serde_json::json!({
                        "outcome": "refused",
                        "passes": passes,
                        "reason": error,
                    }),
                );
                spine.refused(
                    stage,
                    &error,
                    format!(
                        "The shipped `local-private-validator-user-position-admission-v1` refused \
                         while admitting {}: {error}. Everything it needs is on this chain -- the \
                         Market is Open, the campaign report is this founding's own, and the \
                         owner's key is the substrate's own disposable role.",
                        stranger.label
                    ),
                );
                spine
                    .reports
                    .insert("admission".into(), Value::Object(rows));
                return Ok(());
            }
        }
    }
    spine.executed(
        stage,
        landed,
        compute,
        format!(
            "`local-private-validator-user-position-admission-v1`, once per stranger, called in \
             this process with the argument vector a host would type. {} strangers, {landed} \
             finalized transactions re-read from the chain with their logs. The buyer's admission \
             carries the four collateral flags the command takes together or not at all, so the \
             delegated collateral the fill needs is placed by the same act that admits the \
             Position.",
            strangers.len()
        ),
    );
    spine
        .reports
        .insert("admission".into(), Value::Object(rows));
    Ok(())
}

// ---------------------------------------------------------------------- fill

/// Produce the Direct session and walk it to the Hot execution.
///
/// The producer takes a KEY DIRECTORY rather than key flags, and it requires
/// exactly three files in it: `core-upgrade-authority.json` (the payer),
/// `founding-founder.json` (the seller) and `participant.json` (the buyer).
/// This assembles that directory out of the prepare report's own role files by
/// copying, never by minting: a tier that generated a key here would be
/// trading between two identities the founding never admitted.
pub(crate) fn fill(
    rpc: &mut Rpc,
    context: &SpineContextV1<'_>,
    spine: &mut SpineV1,
    buyer_report: &Path,
) -> Result<()> {
    let stage = "trading: a Direct Hot fill between the founder and an admitted stranger";
    let key_dir = context.dir("fill-keys")?;
    for (role, name) in [
        ("core-upgrade-authority", "core-upgrade-authority.json"),
        ("founding-founder", "founding-founder.json"),
        ("participant", "participant.json"),
    ] {
        let destination = key_dir.join(name);
        if !destination.exists() {
            std::fs::copy(context.key(role)?, &destination)?;
        }
    }
    let output_dir = context.dir("fill")?;
    let session = output_dir.join("direct-trade-session.json");
    let finalized = output_dir.join("direct-trade-finalized.json");
    if !session.exists() {
        let produce = vec![
            "--rpc-url".to_owned(),
            context.rpc_url.to_owned(),
            "--plan".to_owned(),
            context.plan.display().to_string(),
            "--market-input".to_owned(),
            context.market_input.display().to_string(),
            "--campaign-report".to_owned(),
            context.campaign_report.display().to_string(),
            "--participant-report".to_owned(),
            buyer_report.display().to_string(),
            "--key-dir".to_owned(),
            key_dir.display().to_string(),
            "--output-dir".to_owned(),
            output_dir.display().to_string(),
        ];
        if let Err(error) = crate::direct_trade_producer::run_owned_loopback(produce) {
            spine.refused(
                stage,
                &error.to_string(),
                format!(
                    "The shipped `local-private-validator-direct-trade-produce-v1` refused before \
                     any key was opened: {error}. It reads the plan, the market input, this \
                     founding's campaign report and the buyer's finalized admission, and it sends \
                     nothing -- so this refusal is a statement about those four documents."
                ),
            );
            return Ok(());
        }
    }
    // One durable action per invocation: replay-setup, token-setup, the four
    // lookup acts, the capability seal, then the Hot execution. The driver
    // decides which is next from the journals it finds, which is why the loop
    // hands it the same vector every time.
    let execute = vec![
        "--rpc-url".to_owned(),
        context.rpc_url.to_owned(),
        "--session".to_owned(),
        session.display().to_string(),
        "--execute".to_owned(),
    ];
    let outcome = resume_until(
        |_| crate::direct_trade::run_owned_loopback(execute.clone()),
        || finalized.exists(),
    );
    let label = "journey fill: Direct Hot execution";
    let (landed, compute) = harvest_dir(rpc, label, &output_dir, &mut spine.transactions);
    match outcome {
        Ok(passes) => {
            let document = read_json(&finalized)?;
            spine.executed(
                stage,
                landed,
                compute,
                format!(
                    "`local-private-validator-direct-trade-v1 --execute`, {passes} invocations, \
                     one durable action each: replay setup, token setup, the four routing-table \
                     acts, the capability seal, then the Hot execution. {landed} finalized \
                     transactions re-read from the chain with their logs, {compute} compute units \
                     across them. Signature {}.",
                    document
                        .get("signature")
                        .and_then(Value::as_str)
                        .unwrap_or("unreported")
                ),
            );
            spine.reports.insert("fill".into(), document);
        }
        Err((passes, error)) => {
            spine.refused(
                stage,
                &error,
                format!(
                    "The shipped Direct trade driver refused at invocation {passes}: {error}. \
                     {landed} of its acts had already finalized, and their evidence is in the \
                     journal directory beside this transcript."
                ),
            );
            spine.reports.insert(
                "fill".into(),
                serde_json::json!({
                    "outcome": "refused",
                    "passes": passes,
                    "reason": error,
                    "landedBefore": landed,
                    "outputDir": output_dir.display().to_string(),
                }),
            );
        }
    }
    Ok(())
}

/// Settle the accrued fee, permissionlessly.
///
/// The fee a Direct fill charges is ACCRUED and not transferred: the maker
/// replay root carries `fee_owed`, and the settlement is a separate, unsigned-
/// by-the-maker act anyone may pay for. `docs/evidence/FIRST_LOCAL_DIRECT_FILL_2026_08_31.md`
/// recorded the zero in the fee destination for exactly this reason.
pub(crate) fn settle_fee(
    rpc: &mut Rpc,
    context: &SpineContextV1<'_>,
    spine: &mut SpineV1,
    debtor: Pubkey,
    fee_payer_keypair: &Path,
) -> Result<()> {
    let stage = "trading: the accrued Direct fee is settled permissionlessly";
    let public_manifest = context.work.join("fill").join("direct-trade-public.json");
    if !public_manifest.exists() {
        spine.stages.push(StageReportV1 {
            stage: stage.into(),
            outcome: "unreachable".into(),
            transactions: 0,
            compute_units: 0,
            note: "the fill produced no public manifest, so there is no accrued fee to settle and \
                   nothing here is a claim about the settlement route"
                .into(),
        });
        return Ok(());
    }
    let evidence = context.work.join("fee-settlement.json");
    let arguments = vec![
        "--rpc-url".to_owned(),
        context.rpc_url.to_owned(),
        "--public-manifest".to_owned(),
        public_manifest.display().to_string(),
        "--maker".to_owned(),
        debtor.to_string(),
        "--evidence".to_owned(),
        evidence.display().to_string(),
        "--fee-payer-keypair".to_owned(),
        fee_payer_keypair.display().to_string(),
        "--execute".to_owned(),
    ];
    match crate::direct_fee_settlement::run_owned_loopback_v1(arguments) {
        Ok(()) => {
            let document = read_json(&evidence)?;
            let (landed, compute) = harvest_document(
                rpc,
                "journey fill: Direct fee settlement",
                &document,
                &mut spine.transactions,
            );
            spine.executed(
                stage,
                landed,
                compute,
                "`local-private-validator-direct-fee-settlement-v1 --execute`. The driver reads \
                 `fee_owed` back off the maker replay after the send and refuses if it is not \
                 zero, which is the only thing that distinguishes a settled fee from a sent one."
                    .into(),
            );
            spine
                .reports
                .insert("fee-settlement".into(), document);
        }
        Err(error) => spine.refused(
            stage,
            &error.to_string(),
            format!("The shipped fee-settlement driver refused: {error}."),
        ),
    }
    Ok(())
}

// ---------------------------------------------------------------- redemption

/// Pay one holder out of the Hoard, into their own token account.
///
/// The gap register said this was behind the Hot gate. It is not, and has not
/// been since terminal settlement became a wallet-signed top-level Claims
/// route: the payout's owner signs for their own Position and the fee payer
/// signs for the packet, and no program signs a CallerAuthority PDA anywhere in
/// the frame. See the register's own corrected entry.
#[allow(clippy::too_many_arguments)]
pub(crate) fn redeem(
    rpc: &mut Rpc,
    context: &SpineContextV1<'_>,
    spine: &mut SpineV1,
    owner_role: &str,
    owner: Pubkey,
    recipient: Pubkey,
    claim_index: u32,
    fee_payer: Pubkey,
    fee_payer_keypair: &Path,
) -> Result<()> {
    let stage = "redemption: a holder redeems through wallet-signed terminal settlement";
    let input_arguments = vec![
        "--rpc-url".to_owned(),
        context.rpc_url.to_owned(),
        "--plan".to_owned(),
        context.plan.display().to_string(),
        "--evidence".to_owned(),
        context.campaign_report.display().to_string(),
        "--market".to_owned(),
        context.market.to_string(),
        "--owner".to_owned(),
        owner.to_string(),
        "--recipient".to_owned(),
        recipient.to_string(),
        "--claim-index".to_owned(),
        claim_index.to_string(),
    ];
    // The one entry in the whole spine that returns a typed document rather
    // than writing one, so this is the one place a file is written by this
    // tier rather than by a driver -- and it is written from the driver's own
    // value, not rebuilt.
    let input = match crate::terminal_lifecycle::produce_wallet_terminal_input_owned_loopback_v1(
        input_arguments,
    ) {
        Ok(input) => input,
        Err(error) => {
            spine.refused(
                stage,
                &error.to_string(),
                format!(
                    "`local-private-validator-wallet-terminal-payout-input-v1` refused: {error}. \
                     It opens no key, sends nothing, and makes exactly two finalized RPC rounds, \
                     so this refusal is about the Market's terminal state or its composition \
                     records and about nothing else."
                ),
            );
            return Ok(());
        }
    };
    let input_path = context.work.join("payout-input.json");
    std::fs::write(&input_path, serde_json::to_vec_pretty(&input)?)?;
    let journal_dir = context.dir("payout-journal")?;
    let evidence = context.work.join("payout-evidence.json");
    let arguments = vec![
        "--rpc-url".to_owned(),
        context.rpc_url.to_owned(),
        "--input".to_owned(),
        input_path.display().to_string(),
        "--fee-payer".to_owned(),
        fee_payer.to_string(),
        "--fee-payer-keypair".to_owned(),
        fee_payer_keypair.display().to_string(),
        "--owner-keypair".to_owned(),
        context.key(owner_role)?.display().to_string(),
        "--journal-dir".to_owned(),
        journal_dir.display().to_string(),
        "--evidence".to_owned(),
        evidence.display().to_string(),
        "--execute".to_owned(),
    ];
    let outcome = resume_until(
        |_| crate::wallet_terminal_payout_exterior::run(arguments.clone()),
        || evidence.exists(),
    );
    let label = "journey redemption: wallet terminal payout";
    let (landed, compute) = harvest_dir(rpc, label, &journal_dir, &mut spine.transactions);
    match outcome {
        Ok(passes) => {
            let document = read_json(&evidence)?;
            let (extra, more) = harvest_document(rpc, label, &document, &mut spine.transactions);
            spine.executed(
                stage,
                landed + extra,
                compute + more,
                format!(
                    "`local-private-validator-wallet-terminal-payout-v1 --execute`, {passes} \
                     invocations, one durable stage each: the four routing-table acts and then the \
                     payout. The recipient is the holder's OWN token account and the debit is the \
                     Hoard's."
                ),
            );
            spine.reports.insert("redemption".into(), document);
        }
        Err((passes, error)) => {
            spine.refused(
                stage,
                &error,
                format!(
                    "The shipped payout driver refused at invocation {passes}: {error}. {landed} \
                     of its stages had already finalized."
                ),
            );
            spine.reports.insert(
                "redemption".into(),
                serde_json::json!({
                    "outcome": "refused",
                    "passes": passes,
                    "reason": error,
                    "landedBefore": landed,
                }),
            );
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- retirement

/// Close the fund, begin retiring, and drive the four checkpointed packets to
/// **Retired**.
///
/// Three shipped commands in order, and the order is the protocol's:
///
/// 1. `local-private-validator-refresh-evidence-v1` re-reads the founding's own
///    accounts at the current slot and refuses a world in which any immutable
///    founding record moved. Its output is what the next two consume.
/// 2. `local-private-validator-terminal-sequence-v1` walks CoreBeginRetiring,
///    DirectBeginRetiring, ResolutionCloseFund, DirectCloseCapability,
///    RetirementReplayHandoff and AggregateRetirement, one per invocation,
///    creating its own exact-union routing table first.
/// 3. `local-private-validator-aggregate-retirement-v1` drives the four
///    checkpoint packets -- prepare, close-vault, close-replay, finish -- and
///    writes a conservation receipt that classifies every lamport they moved.
///
/// The four checkpoint packets have executed against real ELFs under
/// `solana-program-test` since `tools/gauntlet/retirement-checkpoint/` landed.
/// What they had never done is execute on a chain of any kind.
#[allow(clippy::too_many_arguments)]
pub(crate) fn retire(
    rpc: &mut Rpc,
    context: &SpineContextV1<'_>,
    spine: &mut SpineV1,
    source_receipt: Pubkey,
    fee_payer: Pubkey,
    fee_payer_keypair: &Path,
) -> Result<()> {
    let refresh = context.work.join("refresh.json");
    let refresh_stage = "retirement: the founding's evidence is refreshed against the live chain";
    if !refresh.exists() {
        let arguments = vec![
            "--rpc-url".to_owned(),
            context.rpc_url.to_owned(),
            "--plan".to_owned(),
            context.plan.display().to_string(),
            "--expected-plan-sha256".to_owned(),
            digest_of(context.plan)?,
            "--market-input".to_owned(),
            context.market_input.display().to_string(),
            "--expected-market-input-sha256".to_owned(),
            digest_of(context.market_input)?,
            "--campaign-report".to_owned(),
            context.campaign_report.display().to_string(),
            "--expected-campaign-report-sha256".to_owned(),
            digest_of(context.campaign_report)?,
            "--output".to_owned(),
            refresh.display().to_string(),
        ];
        if let Err(error) = crate::evidence_refresh::run_owned_loopback(arguments) {
            spine.refused(
                refresh_stage,
                &error.to_string(),
                format!(
                    "`local-private-validator-refresh-evidence-v1` refused: {error}. It cannot \
                     write to the chain by construction, so this is a statement about the three \
                     documents it pins and the accounts it re-read."
                ),
            );
            return Ok(());
        }
    }
    spine.stages.push(StageReportV1 {
        stage: refresh_stage.into(),
        outcome: "executed".into(),
        transactions: 0,
        compute_units: 0,
        note: "Read-only by construction: the driver connects with a reads-only write policy and \
               refuses any founding record that moved since the founding sealed it."
            .into(),
    });

    // ---- 2. the terminal sequence: CloseFund, BeginRetiring and the handoff
    let sequence_stage =
        "retirement: CloseFund, BeginRetiring and the retirement replay handoff";
    let journal_dir = context.dir("terminal-journal")?;
    let session = context.work.join("terminal-session.json");
    let completion = context.work.join("terminal-completion.json");
    let sequence = vec![
        "--rpc-url".to_owned(),
        context.rpc_url.to_owned(),
        "--plan".to_owned(),
        context.plan.display().to_string(),
        "--market-input".to_owned(),
        context.market_input.display().to_string(),
        "--evidence".to_owned(),
        context.campaign_report.display().to_string(),
        "--refreshed-evidence".to_owned(),
        refresh.display().to_string(),
        "--market".to_owned(),
        context.market.to_string(),
        "--fee-payer".to_owned(),
        fee_payer.to_string(),
        "--fee-payer-keypair".to_owned(),
        fee_payer_keypair.display().to_string(),
        "--session".to_owned(),
        session.display().to_string(),
        "--journal-dir".to_owned(),
        journal_dir.display().to_string(),
        "--completion".to_owned(),
        completion.display().to_string(),
        "--execute".to_owned(),
    ];
    let outcome = resume_until(
        |_| crate::terminal_sequence::run_terminal_sequence_owned_loopback_v1(sequence.clone()),
        || completion.exists(),
    );
    let label = "journey retirement: terminal sequence";
    let (landed, compute) = harvest_dir(rpc, label, &journal_dir, &mut spine.transactions);
    let lookup_table: Option<String>;
    match outcome {
        Ok(passes) => {
            let document = read_json(&completion)?;
            lookup_table = document
                .get("lookup_table")
                .or_else(|| document.get("lookupTable"))
                .and_then(Value::as_str)
                .map(str::to_owned);
            spine.executed(
                sequence_stage,
                landed,
                compute,
                format!(
                    "`local-private-validator-terminal-sequence-v1 --execute`, {passes} \
                     invocations, one durable stage each over the driver's own ordered six -- \
                     CoreBeginRetiring, DirectBeginRetiring, ResolutionCloseFund, \
                     DirectCloseCapability, RetirementReplayHandoff, AggregateRetirement -- with \
                     an exact-union routing table built by the same journal machinery in front."
                ),
            );
            spine
                .reports
                .insert("terminal-sequence".into(), document);
        }
        Err((passes, error)) => {
            spine.refused(
                sequence_stage,
                &error,
                format!(
                    "The shipped terminal-sequence driver refused at invocation {passes}: \
                     {error}. {landed} of its acts had already finalized."
                ),
            );
            spine.reports.insert(
                "terminal-sequence".into(),
                serde_json::json!({
                    "outcome": "refused",
                    "passes": passes,
                    "reason": error,
                    "landedBefore": landed,
                }),
            );
            return Ok(());
        }
    }

    // ---- 3. the four checkpointed packets
    let checkpoint_stage =
        "retirement: the four checkpointed aggregate-retirement packets, to Retired";
    let Some(table) = lookup_table else {
        spine.refused(
            checkpoint_stage,
            "the terminal completion named no lookup table",
            "`local-private-validator-aggregate-retirement-v1` requires `--lookup-table` and the \
             terminal sequence's completion is its one author here; a completion that names none \
             is a finding about that document rather than about the retirement."
                .into(),
        );
        return Ok(());
    };
    let retirement_journal = context.dir("retirement-journal")?;
    let campaign = context.work.join("retirement-campaign.json");
    let retirement_completion = context.work.join("retirement-completion.json");
    let arguments = vec![
        "--rpc-url".to_owned(),
        context.rpc_url.to_owned(),
        "--plan".to_owned(),
        context.plan.display().to_string(),
        "--evidence".to_owned(),
        context.campaign_report.display().to_string(),
        "--refreshed-evidence".to_owned(),
        refresh.display().to_string(),
        "--market".to_owned(),
        context.market.to_string(),
        "--source-receipt".to_owned(),
        source_receipt.to_string(),
        "--fee-payer".to_owned(),
        fee_payer.to_string(),
        "--fee-payer-keypair".to_owned(),
        fee_payer_keypair.display().to_string(),
        "--lookup-table".to_owned(),
        table,
        "--campaign".to_owned(),
        campaign.display().to_string(),
        "--journal-dir".to_owned(),
        retirement_journal.display().to_string(),
        "--completion".to_owned(),
        retirement_completion.display().to_string(),
        "--execute".to_owned(),
    ];
    let outcome = resume_until(
        |_| crate::aggregate_retirement_exterior::run_owned_loopback(arguments.clone()),
        || retirement_completion.exists(),
    );
    let label = "journey retirement: aggregate retirement checkpoint";
    let (landed, compute) = harvest_dir(rpc, label, &retirement_journal, &mut spine.transactions);
    match outcome {
        Ok(passes) => {
            let document = read_json(&retirement_completion)?;
            spine.executed(
                checkpoint_stage,
                landed,
                compute,
                format!(
                    "`local-private-validator-aggregate-retirement-v1 --execute`, {passes} \
                     invocations: prepare, close-vault, close-replay, finish. On a REFUNDING \
                     market the prepare packet also burns the failure column and closes the escrow \
                     pair. {landed} finalized transactions, {compute} compute units, and the \
                     driver's own conservation receipt classifies every lamport the four packets \
                     moved."
                ),
            );
            spine
                .reports
                .insert("aggregate-retirement".into(), document);
        }
        Err((passes, error)) => {
            spine.refused(
                checkpoint_stage,
                &error,
                format!(
                    "The shipped aggregate-retirement driver refused at invocation {passes}: \
                     {error}. {landed} of the four packets had already finalized."
                ),
            );
            spine.reports.insert(
                "aggregate-retirement".into(),
                serde_json::json!({
                    "outcome": "refused",
                    "passes": passes,
                    "reason": error,
                    "landedBefore": landed,
                }),
            );
        }
    }
    Ok(())
}

/// SHA-256 of a file, in the spelling the `--expected-*-sha256` flags take.
fn digest_of(path: &Path) -> Result<String> {
    use sha2::Digest;
    Ok(crate::plan::hex(&sha2::Sha256::digest(std::fs::read(path)?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The harvester walks for the KEY, so a driver that grows a journal array
    /// is harvested without this file learning its shape.
    #[test]
    fn every_signature_key_is_found_at_every_depth() {
        let document = serde_json::json!({
            "signature": "one",
            "landed": {"signature": "two"},
            "journals": [{"signature": "three"}, {"nested": {"signature": "four"}}],
            "notASignature": "five",
        });
        let mut found = Vec::new();
        signatures_in(&document, &mut found);
        // Order is the document's own object order, which serde_json sorts by
        // key, so the assertion is about the SET: the point is that no depth
        // and no array nesting hides a signature, not that the walk emits them
        // in the order a reader of the literal above would guess.
        found.sort();
        assert_eq!(found, vec!["four", "one", "three", "two"]);
    }

    /// A completion file that already exists costs zero invocations. This is
    /// the property that makes a resumed run cheap rather than a re-entry into
    /// a driver that would refuse its own finished journal.
    #[test]
    fn a_completed_loop_never_enters_the_driver() {
        let mut entered = 0;
        let passes = resume_until(
            |_| {
                entered += 1;
                Ok(())
            },
            || true,
        )
        .expect("a completed loop returns");
        assert_eq!((passes, entered), (0, 0));
    }

    /// A driver that never completes is REPORTED at the ceiling rather than
    /// spun on, and the report says how many acts it was given.
    #[test]
    fn a_stalled_loop_reports_its_ceiling() {
        let (passes, reason) = resume_until(|_| Ok(()), || false).expect_err("a stall reports");
        assert_eq!(passes, RESUMPTION_CEILING_V1);
        assert!(reason.contains("stalled action"), "{reason}");
    }
}
