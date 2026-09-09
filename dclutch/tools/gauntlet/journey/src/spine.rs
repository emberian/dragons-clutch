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
//! the payout; `terminal_sequence` walks the five pre-checkpoint mutations of
//! [`dclutch_market_retirement_v1_operator::terminal_stage_order_v1::TerminalStageV1::PRECHECKPOINT`];
//! `aggregate_retirement_exterior` walks the four
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

use std::{
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use serde_json::Value;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
};

use dclutch_market_retirement_v1_operator::terminal_stage_order_v1::TerminalStageV1;

use crate::model::TransactionEvidence;
use crate::rpc::Rpc;
use crate::stages::StageReportV1;
use crate::{Error, Result};

const UPKEEP_FOUND_DEPOSIT_LAMPORTS_V1: u64 = 17_001;

/// Observed sufficient on the accepted held Direct Resolution run. This is a
/// local campaign signer balance floor, not a protocol fee quote or funding
/// requirement. Exact protocol-account rent is always queried separately.
const LOCAL_RESOLUTION_SIGNER_BALANCE_FLOOR_V1: u64 = 100_000_000;

/// The terminal sequence's five pre-checkpoint mutations, in driver order.
///
/// READ, NEVER RESTATED. This file used to print the six by hand, in three
/// places, and printed the wrong order after PROGRAMS-18A reversed the pair a
/// devnet market had already been retired against. The order has one author --
/// `dclutch_market_retirement_v1_operator::terminal_stage_order_v1` -- and this
/// tier calls the driver that reads its pre-checkpoint prefix.
fn terminal_precheckpoint_order_v1() -> String {
    TerminalStageV1::PRECHECKPOINT
        .iter()
        .map(|stage| stage.kebab())
        .collect::<Vec<_>>()
        .join(", ")
}

/// How many times a resumption loop may re-enter one driver.
///
/// Every driver below advances at most one durable action per invocation, so
/// this is a bound on ACTS and not on retries. The longest chain in the table
/// is `direct_trade`'s eight, and `terminal_sequence` can add four lookup acts
/// in front of its five protocol stages; twenty-four is comfortably above both
/// and small enough that a driver stuck on one action is REPORTED within a
/// minute rather than spun on.
const RESUMPTION_CEILING_V1: usize = 24;

/// How long a durable driver may remain byte-for-byte `Submitted` while the
/// validator advances the exact signature to finalized.
///
/// This is a measured-profile bound: the accepted local-validator campaigns
/// use the same 90-second persisted-finalization window. A timeout reports the
/// retained journal and signature so a later invocation can resume it; lifting
/// the bound means replacing local wall-clock polling with an RPC-reported
/// block-height/finality horizon shared by every durable host driver.
const DURABLE_PROGRESS_WAIT_V1: Duration = Duration::from_secs(90);
const DURABLE_PROGRESS_POLL_V1: Duration = Duration::from_millis(250);

/// Re-enter one durable action while it is waiting for finality.
///
/// A byte-identical successful return is not a new action: `Submitted` is a
/// poll-only phase. Keeping those polls inside one resumption pass preserves
/// [`RESUMPTION_CEILING_V1`] as a bound on durable acts instead of accidentally
/// turning it into a bound on RPC speed.
fn await_durable_progress_v1<F, P>(
    mut invoke: F,
    mut progressed: P,
    label: &str,
    wait: Duration,
    poll: Duration,
) -> Result<()>
where
    F: FnMut() -> Result<()>,
    P: FnMut() -> Result<bool>,
{
    let deadline = Instant::now() + wait;
    loop {
        invoke()?;
        if progressed()? {
            return Ok(());
        }
        let now = Instant::now();
        if now >= deadline {
            return Err(Error::new(format!(
                "{label} stayed pending without durable journal progress for {} seconds",
                wait.as_secs()
            )));
        }
        thread::sleep(poll.min(deadline.saturating_duration_since(now)));
    }
}

/// What role an account a spine stage created plays in the conservation laws.
pub(crate) enum ApertureRoleV1 {
    /// A collateral-Mint token account: it joins L1's partition.
    Collateral,
    /// A Claims Position: it joins L3's supply sum.
    Position,
}

/// One account a spine stage brought into existence, read out of that stage's
/// own report.
///
/// THE APERTURE IS DERIVED, NEVER LISTED. A conservation law is only as total
/// as the set of accounts it names, and none of these accounts can be named
/// before the act that creates them -- which is why the founding's aperture,
/// discovered from the founding's own evidence, was complete and then was not.
/// hbox `20260906T131304Z` is what the gap costs: the first admission that ever
/// landed on a validator was met with `VIOLATED L1: tracked 1000000002 atoms
/// across 9 accounts != Mint supply 1100000002; 100000000 atoms are in accounts
/// this ledger does not name`, a true sentence about the ledger rather than
/// about the chain. The devnet spine has read these back out of the landed
/// admission reports since cohort-12 met the same wall
/// (`tools/cohort/build-sim-config.py`); this is that read, for a stage that
/// runs in-process.
pub(crate) struct ApertureEntryV1 {
    pub(crate) label: String,
    pub(crate) address: Pubkey,
    pub(crate) role: ApertureRoleV1,
}

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
    /// Accounts these stages created, for the conservation ledger to name
    /// before the census that follows them.
    pub(crate) aperture: Vec<ApertureEntryV1>,
    /// Collateral atoms the terminal payouts DECLARED they moved out of the
    /// Hoard, summed off each payout driver's own evidence.
    ///
    /// L2 is "the Hoard moves only by what the stage DECLARED", and the
    /// redemption boundary declared zero while the payouts drained the Hoard --
    /// which the law caught the first time this tier ever paid a holder (hbox
    /// `20260906T174856Z`, `the Hoard moved -500000001 atoms ... and the stage
    /// declared 0`). The declaration is the DRIVER's `payout` field, not a
    /// number this tier derives: restating the payout arithmetic here and then
    /// calling the agreement evidence is the second-author mistake the whole
    /// spine exists to avoid.
    pub(crate) paid_atoms: u128,
}

impl SpineV1 {
    pub(crate) fn new() -> Self {
        Self {
            stages: Vec::new(),
            transactions: Vec::new(),
            refusals: Vec::new(),
            reports: serde_json::Map::new(),
            aperture: Vec::new(),
            paid_atoms: 0,
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

    /// Name an account a stage created, from a key inside that stage's own
    /// report. A document that does not carry the key, or carries something
    /// that is not an address, adds nothing: the ledger's own L1 is what
    /// reports the resulting gap, and it reports it as a number.
    fn admit_account(
        &mut self,
        label: String,
        document: &Value,
        pointer: &str,
        role: ApertureRoleV1,
    ) {
        if let Some(address) = document
            .pointer(pointer)
            .and_then(Value::as_str)
            .and_then(|text| text.parse::<Pubkey>().ok())
        {
            self.aperture.push(ApertureEntryV1 {
                label,
                address,
                role,
            });
        }
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

/// Authenticated inputs for the existing flagship Resolution owner.
pub(crate) struct HeldResolutionContextV1<'a> {
    pub(crate) rpc_url: &'a str,
    pub(crate) plan: &'a Path,
    pub(crate) plan_sha256: &'a str,
    pub(crate) campaign_report: &'a Path,
    pub(crate) market_input: &'a Path,
    pub(crate) market_sha256: &'a str,
    pub(crate) market: Pubkey,
    pub(crate) pyth_facts: &'a Path,
    pub(crate) submitter_keypair: &'a Path,
    pub(crate) resolver_keypair: &'a Path,
    pub(crate) fee_payer_keypair: &'a Path,
    pub(crate) update_keypair: &'a Path,
    pub(crate) work: &'a Path,
    pub(crate) max_wait_seconds: i64,
    pub(crate) terminal_resume: bool,
}

pub(crate) struct HeldResolutionEvidenceV1 {
    pub(crate) report: Value,
    pub(crate) transactions: Vec<TransactionEvidence>,
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

fn write_resolution_journal_v1(path: &Path, document: &Value) -> Result<()> {
    let temporary = path.with_extension(format!("json.tmp-{}", std::process::id()));
    std::fs::write(&temporary, serde_json::to_vec_pretty(document)?)?;
    std::fs::rename(&temporary, path)?;
    Ok(())
}

fn append_resolution_setup_v1(path: &Path, event: Value) -> Result<()> {
    let mut document = if path.exists() {
        read_json(path)?
    } else {
        serde_json::json!({
            "schema": "dclutch-held-resolution-setup-journal-v1",
            "events": [],
        })
    };
    if document["schema"] != "dclutch-held-resolution-setup-journal-v1" {
        return Err(Error::new("held Resolution setup journal schema changed"));
    }
    document["events"]
        .as_array_mut()
        .ok_or_else(|| Error::new("held Resolution setup journal omitted events"))?
        .push(event);
    write_resolution_journal_v1(path, &document)
}

fn verified_prior_transaction_v1(
    rpc: &mut Rpc,
    expected: TransactionEvidence,
) -> Result<TransactionEvidence> {
    let signature = expected
        .signature
        .parse::<Signature>()
        .map_err(|error| Error::new(format!("held Resolution journal signature: {error}")))?;
    let mut actual = rpc
        .finalized_signed_packet(&expected.label, signature, false)?
        .ok_or_else(|| Error::new("held Resolution journal transaction disappeared from chain"))?
        .evidence;
    actual.label = expected.label.clone();
    if actual.signature != expected.signature
        || actual.slot != expected.slot
        || actual.fee_lamports != expected.fee_lamports
        || actual.compute_units_consumed != expected.compute_units_consumed
        || actual.error != expected.error
    {
        return Err(Error::new(
            "held Resolution setup journal differs from finalized transaction history",
        ));
    }
    Ok(actual)
}

fn resolution_setup_transactions_v1(
    rpc: &mut Rpc,
    path: &Path,
) -> Result<Vec<TransactionEvidence>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let document = read_json(path)?;
    if document["schema"] != "dclutch-held-resolution-setup-journal-v1" {
        return Err(Error::new("held Resolution setup journal schema changed"));
    }
    resolution_setup_transaction_values_v1(&document)?
        .into_iter()
        .map(|transaction| {
            let expected: TransactionEvidence = serde_json::from_value(transaction)?;
            verified_prior_transaction_v1(rpc, expected)
        })
        .collect()
}

fn resolution_setup_transaction_values_v1(document: &Value) -> Result<Vec<Value>> {
    let transactions = document["events"]
        .as_array()
        .ok_or_else(|| Error::new("held Resolution setup journal omitted events"))?
        .iter()
        .filter_map(|event| event.get("transaction").filter(|value| !value.is_null()))
        .cloned()
        .collect();
    Ok(transactions)
}

fn receipt_is_present_v1(checkpoint_path: &Path, stage: &str) -> Result<bool> {
    if !checkpoint_path.exists() {
        return Ok(false);
    }
    let checkpoint = read_json(checkpoint_path)?;
    if stage == "complete" {
        return Ok(checkpoint["verifiedTerminal"] == true);
    }
    let receipt_stage = match stage {
        "submit" => "submit",
        "execute" => "resolution-provider-execute-v1",
        "accept" => "core-terminal-accept-v1",
        "reclaim" => "reclaim",
        _ => return Err(Error::new(format!("unknown held Resolution stage {stage}"))),
    };
    Ok(checkpoint["receipts"].as_array().is_some_and(|receipts| {
        receipts
            .iter()
            .any(|receipt| receipt["stage"] == receipt_stage)
    }))
}

fn parse_resolution_projection_v1(
    input_path: &Path,
    producer_path: &Path,
    market: Pubkey,
) -> Result<crate::flagship_resolution::OwnedLoopbackContinuationInputV1> {
    let bytes = if input_path.exists() {
        std::fs::read(input_path)?
    } else {
        let checkpoint = read_json(producer_path)?;
        serde_json::to_vec(
            checkpoint
                .get("plannedInput")
                .ok_or_else(|| Error::new("flagship producer checkpoint omitted plannedInput"))?,
        )?
    };
    crate::flagship_resolution::parse_owned_loopback_continuation_input_v1(&bytes, market)
}

fn require_resolution_signers_v1(
    projection: crate::flagship_resolution::OwnedLoopbackContinuationInputV1,
    submitter: &Keypair,
    resolver: &Keypair,
    payer: &Keypair,
    update: &Keypair,
) -> Result<()> {
    if projection.submitter != submitter.pubkey()
        || projection.resolver != resolver.pubkey()
        || projection.payer != payer.pubkey()
        || projection.update != update.pubkey()
    {
        return Err(Error::new(
            "the four named Resolution keypairs do not match the flagship producer's native typed input",
        ));
    }
    Ok(())
}

fn prepay_resolution_account_v1(
    rpc: &mut Rpc,
    setup_path: &Path,
    label: &str,
    stage: &str,
    destination: Pubkey,
    bytes: usize,
    campaign_payer: &Keypair,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<Value> {
    let rent = rpc.minimum_balance(bytes)?;
    let before = rpc.account(destination)?;
    if before.as_ref().is_some_and(|account| {
        account.owner != solana_sdk_ids::system_program::ID
            || account.executable
            || !account.data.is_empty()
    }) {
        // A crash may leave the protocol-owned account finalized before its
        // receipt reached disk. The durable executor below is the authority
        // that reconciles or refuses those bytes; a setup transfer must not
        // touch it.
        return Ok(serde_json::json!({
            "label": label,
            "stage": stage,
            "address": destination.to_string(),
            "rentLamports": rent,
            "status": "already-created; durable executor must reconcile",
        }));
    }
    let before_lamports = before.as_ref().map_or(0, |account| account.lamports);
    if before_lamports > rent {
        return Err(Error::new(format!(
            "{label} prepay exceeds the current exact rent minimum"
        )));
    }
    let missing = rent - before_lamports;
    let transaction = if missing == 0 {
        None
    } else {
        let evidence = rpc.send(
            &format!("journey: prepay exact {label} rent before durable {stage}"),
            &[solana_system_interface::instruction::transfer(
                &campaign_payer.pubkey(),
                &destination,
                missing,
            )],
            campaign_payer,
        )?;
        transactions.push(evidence.clone());
        Some(evidence)
    };
    let after_lamports = rpc
        .account(destination)?
        .as_ref()
        .map_or(0, |account| account.lamports);
    if after_lamports != rent {
        return Err(Error::new(format!(
            "{label} prepay left {after_lamports} lamports, expected exact rent {rent}"
        )));
    }
    let event = serde_json::json!({
        "kind": "exact-protocol-rent-prepay",
        "label": label,
        "stage": stage,
        "address": destination.to_string(),
        "dataBytes": bytes,
        "beforeLamports": before_lamports,
        "transferredLamports": missing,
        "afterLamports": after_lamports,
        "transaction": transaction,
    });
    append_resolution_setup_v1(setup_path, event.clone())?;
    Ok(event)
}

/// Drive the existing flagship producer/table/executor through its durable
/// Complete checkpoint. This function constructs no protocol instruction: it
/// calls the exact same owner entry points a host command invokes.
pub(crate) fn resolve_held_market_v1(
    rpc: &mut Rpc,
    context: HeldResolutionContextV1<'_>,
) -> Result<HeldResolutionEvidenceV1> {
    use sha2::Digest as _;

    std::fs::create_dir_all(context.work)?;
    let refresh_path = context.work.join("refreshed-evidence.json");
    let producer_path = context.work.join("producer.json");
    let table_path = context.work.join("tables.json");
    let input_path = context.work.join("input.json");
    let checkpoint_path = context.work.join("checkpoint.json");
    let setup_path = context.work.join("setup-journal.json");

    // A Terminal resume is admitted only after the owner parses the exact
    // native input and durable checkpoint against live finalized state.
    // `--through accept` is deliberately behind the classified Reclaim stage,
    // so this cannot plan Reclaim before the immutable deadline or pin a
    // prepay prebalance. It cannot send without `--execute`.
    if context.terminal_resume {
        crate::flagship_resolution::run_owned_loopback(vec![
            "--rpc-url".into(),
            context.rpc_url.into(),
            "--input".into(),
            input_path.display().to_string(),
            "--checkpoint".into(),
            checkpoint_path.display().to_string(),
            "--through".into(),
            "accept".into(),
        ])?;
    }

    let submitter = crate::substrate::load_keypair(context.submitter_keypair)?;
    let resolver = crate::substrate::load_keypair(context.resolver_keypair)?;
    let payer = crate::substrate::load_keypair(context.fee_payer_keypair)?;
    let update = crate::substrate::load_keypair(context.update_keypair)?;
    let mut transactions = resolution_setup_transactions_v1(rpc, &setup_path)?;

    let producer_arguments = || {
        vec![
            "--rpc-url".into(),
            context.rpc_url.into(),
            "--produce-input".into(),
            "--plan".into(),
            context.plan.display().to_string(),
            "--campaign-evidence".into(),
            context.campaign_report.display().to_string(),
            "--refreshed-evidence".into(),
            refresh_path.display().to_string(),
            "--pyth-facts".into(),
            context.pyth_facts.display().to_string(),
            "--producer-checkpoint".into(),
            producer_path.display().to_string(),
            "--output".into(),
            input_path.display().to_string(),
            "--payer".into(),
            payer.pubkey().to_string(),
        ]
    };

    if !input_path.exists() {
        if !refresh_path.exists() {
            crate::evidence_refresh::run_owned_loopback(vec![
                "--rpc-url".into(),
                context.rpc_url.into(),
                "--plan".into(),
                context.plan.display().to_string(),
                "--expected-plan-sha256".into(),
                context.plan_sha256.into(),
                "--market-input".into(),
                context.market_input.display().to_string(),
                "--expected-market-input-sha256".into(),
                context.market_sha256.into(),
                "--campaign-report".into(),
                context.campaign_report.display().to_string(),
                "--expected-campaign-report-sha256".into(),
                crate::plan::hex(&sha2::Sha256::digest(std::fs::read(
                    context.campaign_report,
                )?)),
                "--output".into(),
                refresh_path.display().to_string(),
            ])?;
        }
        crate::flagship_resolution::run_owned_loopback(producer_arguments())?;
        let projection =
            parse_resolution_projection_v1(&input_path, &producer_path, context.market)?;
        require_resolution_signers_v1(projection, &submitter, &resolver, &payer, &update)?;

        if !table_path.exists() {
            let resolver_before = rpc
                .account(resolver.pubkey())?
                .map_or(0, |row| row.lamports);
            let payer_before = rpc.account(payer.pubkey())?.map_or(0, |row| row.lamports);
            let resolver_missing =
                LOCAL_RESOLUTION_SIGNER_BALANCE_FLOOR_V1.saturating_sub(resolver_before);
            let payer_missing =
                LOCAL_RESOLUTION_SIGNER_BALANCE_FLOOR_V1.saturating_sub(payer_before);
            let mut instructions = Vec::new();
            if resolver_missing > 0 {
                instructions.push(solana_system_interface::instruction::transfer(
                    &submitter.pubkey(),
                    &resolver.pubkey(),
                    resolver_missing,
                ));
            }
            if payer_missing > 0 {
                instructions.push(solana_system_interface::instruction::transfer(
                    &submitter.pubkey(),
                    &payer.pubkey(),
                    payer_missing,
                ));
            }
            let transaction = if instructions.is_empty() {
                None
            } else {
                let evidence = rpc.send(
                    "journey: capitalize Resolution signing wallets to the measured local balance floor",
                    &instructions,
                    &submitter,
                )?;
                transactions.push(evidence.clone());
                Some(evidence)
            };
            let resolver_after = rpc
                .account(resolver.pubkey())?
                .map_or(0, |row| row.lamports);
            let payer_after = rpc.account(payer.pubkey())?.map_or(0, |row| row.lamports);
            if resolver_after < LOCAL_RESOLUTION_SIGNER_BALANCE_FLOOR_V1
                || payer_after < LOCAL_RESOLUTION_SIGNER_BALANCE_FLOOR_V1
            {
                return Err(Error::new(
                    "local Resolution signer capitalization did not reach its measured balance floor",
                ));
            }
            append_resolution_setup_v1(
                &setup_path,
                serde_json::json!({
                    "kind": "local-campaign-execution-funding",
                    "notProtocolRequirement": true,
                    "balanceFloorLamports": LOCAL_RESOLUTION_SIGNER_BALANCE_FLOOR_V1,
                    "source": submitter.pubkey().to_string(),
                    "resolver": resolver.pubkey().to_string(),
                    "resolverBeforeLamports": resolver_before,
                    "resolverTransferredLamports": resolver_missing,
                    "resolverAfterLamports": resolver_after,
                    "feePayer": payer.pubkey().to_string(),
                    "feePayerBeforeLamports": payer_before,
                    "feePayerTransferredLamports": payer_missing,
                    "feePayerAfterLamports": payer_after,
                    "transaction": transaction,
                }),
            )?;
        }

        for _ in 0..RESUMPTION_CEILING_V1 {
            if input_path.exists() {
                break;
            }
            let before = std::fs::read(&table_path).ok();
            await_durable_progress_v1(
                || {
                    crate::flagship_resolution::run_owned_loopback(vec![
                        "--rpc-url".into(),
                        context.rpc_url.into(),
                        "--provision-tables".into(),
                        "--producer-checkpoint".into(),
                        producer_path.display().to_string(),
                        "--table-journal".into(),
                        table_path.display().to_string(),
                        "--authority-keypair".into(),
                        context.resolver_keypair.display().to_string(),
                        "--execute".into(),
                    ])
                },
                || Ok(input_path.exists() || before != std::fs::read(&table_path).ok()),
                "flagship table provisioner",
                DURABLE_PROGRESS_WAIT_V1,
                DURABLE_PROGRESS_POLL_V1,
            )?;
            crate::flagship_resolution::run_owned_loopback(producer_arguments())?;
        }
        if !input_path.exists() {
            return Err(Error::new(
                "flagship table provisioner reached the bounded resumption ceiling before input",
            ));
        }
    }

    let projection = parse_resolution_projection_v1(&input_path, &producer_path, context.market)?;
    require_resolution_signers_v1(projection, &submitter, &resolver, &payer, &update)?;
    let mut setup_reports = Vec::new();
    let signer_arguments = || {
        vec![
            "--submitter-keypair".into(),
            context.submitter_keypair.display().to_string(),
            "--resolver-keypair".into(),
            context.resolver_keypair.display().to_string(),
            "--payer-keypair".into(),
            context.fee_payer_keypair.display().to_string(),
            "--update-keypair".into(),
            context.update_keypair.display().to_string(),
        ]
    };
    for stage in ["submit", "execute", "accept", "reclaim", "complete"] {
        if receipt_is_present_v1(&checkpoint_path, stage)? {
            continue;
        }
        if stage == "submit" {
            setup_reports.push(prepay_resolution_account_v1(
                rpc,
                &setup_path,
                "provider lifecycle",
                stage,
                projection.lifecycle,
                dclutch_source::resolution::PROVIDER_UPDATE_LIFECYCLE_BYTES_V4,
                &submitter,
                &mut transactions,
            )?);
        } else if stage == "execute" {
            setup_reports.push(prepay_resolution_account_v1(
                rpc,
                &setup_path,
                "terminal certificate",
                stage,
                projection.certificate,
                dclutch_source::resolution::RESOLUTION_CERTIFICATE_BYTES_V2,
                &submitter,
                &mut transactions,
            )?);
        }
        if stage == "reclaim" {
            crate::sponsored_schedule::wait_until_unix_seconds_v1(
                rpc,
                projection.reclaim_after_unix_seconds,
                context.max_wait_seconds,
            )?;
        }
        for _ in 0..RESUMPTION_CEILING_V1 {
            let before = std::fs::read(&checkpoint_path).ok();
            let mut arguments = vec![
                "--rpc-url".into(),
                context.rpc_url.into(),
                "--input".into(),
                input_path.display().to_string(),
                "--checkpoint".into(),
                checkpoint_path.display().to_string(),
                "--through".into(),
                stage.into(),
            ];
            arguments.extend(signer_arguments());
            arguments.push("--execute".into());
            await_durable_progress_v1(
                || crate::flagship_resolution::run_owned_loopback(arguments.clone()),
                || {
                    Ok(receipt_is_present_v1(&checkpoint_path, stage)?
                        || before != std::fs::read(&checkpoint_path).ok())
                },
                &format!("flagship durable {stage}"),
                DURABLE_PROGRESS_WAIT_V1,
                DURABLE_PROGRESS_POLL_V1,
            )?;
            if receipt_is_present_v1(&checkpoint_path, stage)? {
                break;
            }
        }
        if !receipt_is_present_v1(&checkpoint_path, stage)? {
            return Err(Error::new(format!(
                "flagship durable {stage} reached the bounded resumption ceiling"
            )));
        }
    }

    let accepted_checkpoint_bytes = std::fs::read(&checkpoint_path)?;
    crate::flagship_resolution::run_owned_loopback(vec![
        "--rpc-url".into(),
        context.rpc_url.into(),
        "--input".into(),
        input_path.display().to_string(),
        "--checkpoint".into(),
        checkpoint_path.display().to_string(),
        "--through".into(),
        "complete".into(),
    ])?;
    if std::fs::read(&checkpoint_path)? != accepted_checkpoint_bytes {
        return Err(Error::new(
            "read-only flagship Complete restart changed its accepted checkpoint",
        ));
    }
    let table_journal = read_json(&table_path)?;
    let checkpoint: Value = serde_json::from_slice(&accepted_checkpoint_bytes)?;
    harvest_required_resolution_receipts_v1(rpc, [&table_journal, &checkpoint], &mut transactions)?;
    let report = serde_json::json!({
        "schema": "dclutch-held-journey-resolution-continuation-v1",
        "market": context.market.to_string(),
        "input": read_json(&input_path)?,
        "producer": read_json(&producer_path)?,
        "tableJournal": table_journal,
        "checkpoint": checkpoint,
        "setupJournal": read_json(&setup_path)?,
        "setupThisInvocation": setup_reports,
        "restartCompleteUnchanged": true,
        "transactionCount": transactions.len(),
        "computeUnitsByTransaction": transactions.iter().map(|transaction| serde_json::json!({
            "label": transaction.label,
            "signature": transaction.signature,
            "computeUnitsConsumed": transaction.compute_units_consumed,
        })).collect::<Vec<_>>(),
    });
    Ok(HeldResolutionEvidenceV1 {
        report,
        transactions,
    })
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
    if into.iter().any(|evidence| evidence.signature == signature) {
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
                // ANY `*Signature` KEY, not three named ones. The Direct
                // capability activation reports its own act under
                // `activationSignature`, so run 8 harvested the four routing
                // table transactions and MISSED the 505,381 CU activation
                // itself -- the stage reported 4 tx and 33,015 CU for a stage
                // whose whole subject cost fifteen times that. A harvest that
                // enumerates field names rots every time a driver names its
                // signature differently, and it rots SILENTLY, as an
                // understatement rather than an error.
                if key == "signature" || key.ends_with("Signature") {
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

fn harvest_required_resolution_receipts_v1<'a>(
    rpc: &mut Rpc,
    documents: impl IntoIterator<Item = &'a Value>,
    into: &mut Vec<TransactionEvidence>,
) -> Result<()> {
    let mut receipts = Vec::new();
    for document in documents {
        if let Some(rows) = document.get("receipts").and_then(Value::as_array) {
            receipts.extend(rows.iter());
        }
        if let Some(finalized) = document.get("finalized")
            && !finalized.is_null()
        {
            receipts.push(finalized);
        }
    }
    for receipt in receipts {
        let signature_text = receipt["signature"]
            .as_str()
            .ok_or_else(|| Error::new("held Resolution receipt omitted signature"))?;
        if into
            .iter()
            .any(|transaction| transaction.signature == signature_text)
        {
            continue;
        }
        let signature = signature_text
            .parse::<Signature>()
            .map_err(|error| Error::new(format!("held Resolution receipt signature: {error}")))?;
        let mut evidence = rpc
            .finalized_signed_packet(
                "journey: flagship Resolution finalized receipt",
                signature,
                false,
            )?
            .ok_or_else(|| Error::new("held Resolution receipt disappeared from chain"))?
            .evidence;
        evidence.label = format!(
            "journey: flagship Resolution {}",
            receipt["stage"].as_str().unwrap_or("table action")
        );
        if evidence.slot
            != receipt["slot"]
                .as_u64()
                .ok_or_else(|| Error::new("held Resolution receipt omitted slot"))?
            || evidence.fee_lamports != receipt["feeLamports"].as_u64()
            || evidence.compute_units_consumed != receipt["computeUnitsConsumed"].as_u64()
            || evidence.error.is_some()
        {
            return Err(Error::new(
                "held Resolution receipt differs from finalized transaction history",
            ));
        }
        into.push(evidence);
    }
    Ok(())
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
fn resume_until<F, C>(
    mut invoke: F,
    mut completed: C,
) -> std::result::Result<usize, (usize, String)>
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
                let (count, units) =
                    harvest_document(rpc, &label, &document, &mut spine.transactions);
                landed += count;
                compute += units;
                // The two accounts this admission created, named from the
                // report it just wrote. The Position always; the delegated
                // collateral account only for the stranger who carries the
                // collateral leg.
                spine.admit_account(
                    format!("{}_position", stranger.label),
                    &document,
                    "/intent/position",
                    ApertureRoleV1::Position,
                );
                spine.admit_account(
                    format!("{}_delegated_collateral", stranger.label),
                    &document,
                    "/collateral/intent/participantTokenAccount",
                    ApertureRoleV1::Collateral,
                );
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

// ------------------------------------------------------ Direct activation

/// Create the Direct capability root, which nothing else in this tree creates.
///
/// THE JOURNEY NEVER RAN THIS, and two of its walls are that absence.
/// `direct_capability_activation` is the only author of the Direct execution
/// root -- Core's `ActivateCapability` CPIs Trading's `process_activation` and
/// only this frame reaches it -- so on a validator where it has not run:
///
///   * `direct_trade_producer` derives the root, finds nothing at it and
///     refuses, which is the wall `SIMULATOR_POPULATION_DRIVEN_2026_08_30`
///     recorded as twenty-one refused fills and read as a width problem; and
///   * `evidence_refresh` emits `direct_capability_root` only where the account
///     EXISTS while the founding emits `direct_trading_funding_ledger`
///     unconditionally, so the refreshed evidence carries exactly half of the
///     Direct first-use pair and `require_direct_first_use_evidence_v1` refuses
///     the whole terminal sequence -- "it carries direct_trading_funding_ledger
///     and omits direct_capability_root", which is the true sentence about a
///     market that was never activated.
///
/// The devnet cohorts have run this immediately after the founding since the
/// row existed (`tools/cohort/steps.tsv`, `activate-direct`, between
/// `found-direct` and `arm-relay`) and with the same key: the campaign payer.
/// It is idempotent by design -- a live Trading-owned root at the derived
/// coordinate reports `already-active` and exits cleanly -- so a resumed run
/// converges instead of double-submitting.
pub(crate) fn activate_direct_capability(
    rpc: &mut Rpc,
    context: &SpineContextV1<'_>,
    spine: &mut SpineV1,
    payer: Pubkey,
    payer_keypair: &Path,
) -> Result<()> {
    let stage = "trading: the Direct capability root is activated";
    let report = context.dir("activation")?.join("direct-activation.json");
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
        "--payer".to_owned(),
        payer.to_string(),
        "--payer-keypair".to_owned(),
        payer_keypair.display().to_string(),
        "--output".to_owned(),
        report.display().to_string(),
        "--execute".to_owned(),
    ];
    match crate::direct_capability_activation::run_owned_loopback(arguments) {
        Ok(()) => {
            let document = read_json(&report)?;
            let (landed, compute) = harvest_document(
                rpc,
                "journey: Direct capability activation",
                &document,
                &mut spine.transactions,
            );
            let verdict = document
                .get("verdict")
                .and_then(Value::as_str)
                .unwrap_or("unstated")
                .to_owned();
            spine.executed(
                stage,
                landed,
                compute,
                format!(
                    "`local-private-validator-direct-capability-activation-v1 --execute`, verdict                      {verdict}. One Core-signed permissionless transaction writes                      `CapabilityRootHeaderV1 || DirectRootStateV1` at the derived root and moves                      the funding ledger's parked rent quote into it. The fill's root check and                      the terminal sequence's Direct first-use pair both read what this creates."
                ),
            );
            spine.reports.insert("direct-activation".into(), document);
        }
        Err(error) => spine.refused(
            stage,
            &error.to_string(),
            format!(
                "The shipped Direct capability activation refused: {error}. Nothing downstream of                  it can be read as a statement about Direct trading or about retirement evidence:                  the execution root does not exist and this tree has no other author for it."
            ),
        ),
    }
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
pub(crate) fn fill_key_directory(context: &SpineContextV1<'_>) -> Result<PathBuf> {
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
    Ok(key_dir)
}

pub(crate) fn fill(
    rpc: &mut Rpc,
    context: &SpineContextV1<'_>,
    spine: &mut SpineV1,
    buyer_report: &Path,
) -> Result<()> {
    let stage = "trading: a Direct Hot fill between the founder and an admitted stranger";
    let key_dir = fill_key_directory(context)?;
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
    // THE FILL'S TWO COLLATERAL DESTINATIONS, named before it runs. Its
    // token-setup act creates the seller's Direct token account and the fee
    // recipient's, and both hold collateral atoms the moment the Hot execution
    // lands -- so a ledger that did not name them would report the buyer's
    // debit as atoms nobody holds. They are read out of the producer's own
    // public manifest, and naming them BEFORE the execute loop means a fill
    // that refuses part way still leaves the census total over what it made.
    let public_manifest = output_dir.join("direct-trade-public.json");
    if public_manifest.exists() {
        let document = read_json(&public_manifest)?;
        spine.admit_account(
            "direct_seller_token".into(),
            &document,
            "/tokenSetup/sellerToken",
            ApertureRoleV1::Collateral,
        );
        spine.admit_account(
            "direct_fee_token".into(),
            &document,
            "/tokenSetup/feeToken",
            ApertureRoleV1::Collateral,
        );
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
    public_manifest: &Path,
    debtor: Pubkey,
    fee_payer_keypair: &Path,
) -> Result<()> {
    let stage = "trading: the accrued Direct fee is settled permissionlessly";
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
            spine.reports.insert("fee-settlement".into(), document);
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
/// Create the Market's Claims-role Custody replay, which terminal payout decodes
/// and never creates.
///
/// # The producer gap this closes for the journey
///
/// `programs/dclutch-claims-sbf/src/custody_replay_v1.rs` is a dedicated
/// first-use creation route -- only the Claims program can produce a Claims-role
/// caller authority, so only it can create the Claims-role replay -- and
/// `terminal_settlement_v3` deliberately does NOT create it, because creation is
/// never a side effect of a payout. The founding creates a TRADING-role replay
/// (`founding_normal_custody_replay`, `CallerRoleV1::Trading`); the Claims-role
/// one at the same market, release set and custody context is a different PDA
/// and nothing here had ever asked for it.
///
/// So the redemption refused twice, on two chains, with a true sentence about an
/// account that did not exist: `wallet payout snapshot is missing Claims Custody
/// replay 4U7Sq…` (hbox `20260906T152908Z`) and `… 9cp1MV3…`
/// (`20260906T155320Z`). The devnet spine has driven this act since cohort 14 --
/// `31-admit-terminal` runs `devnet-claims-custody-replay-v1` before the
/// admission -- and this is its loopback arm, the same shipped command.
///
/// It takes no custody context: the Claims aggregate is the sole persisted owner
/// of the Market's Custody namespace (decision 0008 §1) and the driver reads
/// release set, Realm, generation and context off it.
fn create_claims_custody_replay(
    rpc: &mut Rpc,
    context: &SpineContextV1<'_>,
    spine: &mut SpineV1,
    fee_payer: Pubkey,
    fee_payer_keypair: &Path,
) {
    let stage = "redemption: the Market's Claims-role Custody replay is created for the first time";
    let Ok(output) = context
        .dir("claims-custody-replay")
        .map(|dir| dir.join("claims-custody-replay.json"))
    else {
        return;
    };
    let arguments = vec![
        "--rpc-url".to_owned(),
        context.rpc_url.to_owned(),
        "--plan".to_owned(),
        context.plan.display().to_string(),
        "--evidence".to_owned(),
        context.campaign_report.display().to_string(),
        "--market".to_owned(),
        context.market.to_string(),
        "--fee-payer".to_owned(),
        fee_payer.to_string(),
        "--fee-payer-keypair".to_owned(),
        fee_payer_keypair.display().to_string(),
        "--output".to_owned(),
        output.display().to_string(),
        "--execute".to_owned(),
    ];
    let outcome = crate::claims_custody_replay::run_owned_loopback_v1(arguments);
    let label = "journey redemption: Claims-role Custody replay creation";
    let (landed, compute) = match read_json(&output) {
        Ok(document) => {
            let result = harvest_document(rpc, label, &document, &mut spine.transactions);
            spine
                .reports
                .insert("claims-custody-replay".into(), document);
            result
        }
        Err(_) => (0, 0),
    };
    match outcome {
        Ok(()) => spine.executed(
            stage,
            landed,
            compute,
            "`local-private-validator-claims-custody-replay-v1 --execute`: the only caller of the \
             DCLCCR01 route outside a program test. Fifteen accounts, a 48-byte wire carrying a \
             Market coordinate and nothing else; the release set, Realm, generation and custody \
             context come off the Claims aggregate, the rent refund off Core's own \
             `rent_beneficiary`, and the driver reads the replay back to prove Custody created it \
             at revision 1."
                .into(),
        ),
        Err(error) => spine.refused(
            stage,
            &error.to_string(),
            format!(
                "`local-private-validator-claims-custody-replay-v1` refused: {error}. Every \
                 economic fact it uses is read off the Claims aggregate, so a refusal here is a \
                 statement about that account and the Market's Custody namespace."
            ),
        ),
    }
}

pub(crate) fn redeem(
    rpc: &mut Rpc,
    context: &SpineContextV1<'_>,
    spine: &mut SpineV1,
    holder: &str,
    owner_role: &str,
    owner: Pubkey,
    recipient: Pubkey,
    claim_index: u32,
    fee_payer: Pubkey,
    fee_payer_keypair: &Path,
) -> Result<()> {
    // ONE STAGE PER HOLDER, and every path this stage writes carries the
    // holder's name. `BeginRetiring is blocked: Claims supply at index 0 is
    // 166666667; produce and execute wallet terminal payouts first` (hbox
    // `20260906T155320Z`) is the protocol saying the retirement is gated on
    // EVERY holder of the winning outcome being paid, not one of them -- so a
    // single fixed `payout-input.json` was a shape that could only ever pay the
    // first.
    let stage =
        &format!("redemption: {holder} redeems through wallet-signed terminal settlement") as &str;
    // FIRST USE, ONCE PER MARKET. The payout decodes the Claims-role Custody
    // replay and never creates it, so the creation runs in front of the first
    // payout and nowhere else. The driver is idempotent by refusal rather than
    // by guess: a replay that already exists refuses by name and the refusal is
    // recorded as this stage's own finding.
    if !context.work.join("claims-custody-replay").exists() {
        create_claims_custody_replay(rpc, context, spine, fee_payer, fee_payer_keypair);
    }
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
            // A HOLDER WITH NO CLAIM AT AN INDEX IS NOT A WALL. The redemption
            // walks every claim index because the retirement is gated on the
            // aggregate's whole liability record, and a Position that holds
            // nothing at index N makes the producer say so exactly: *payout
            // quantity must be within 1..=0 atoms at claim index N*. That is an
            // empty ledger row, not a refused act, and recording it as a
            // refusal would fail the run for the arithmetic being right.
            let sentence = error.to_string();
            if sentence.contains("must be within 1..=0 atoms") {
                spine.stages.push(StageReportV1 {
                    stage: stage.into(),
                    outcome: "not-driven".into(),
                    transactions: 0,
                    compute_units: 0,
                    note: format!(
                        "This holder's Position carries no claim at index {claim_index}, and the \
                         producer said so before a key was opened: {sentence}. Nothing to \
                         discharge, so nothing was sent."
                    ),
                });
                return Ok(());
            }
            spine.refused(
                stage,
                &sentence,
                format!(
                    "`local-private-validator-wallet-terminal-payout-input-v1` refused: \
                     {sentence}. It opens no key, sends nothing, and makes exactly two finalized \
                     RPC rounds, so this refusal is about the Market's terminal state or its \
                     composition records and about nothing else."
                ),
            );
            return Ok(());
        }
    };
    let input_path = context.work.join(format!("payout-input-{holder}.json"));
    std::fs::write(&input_path, serde_json::to_vec_pretty(&input)?)?;
    let journal_dir = context.dir(&format!("payout-journal-{holder}"))?;
    let evidence = context.work.join(format!("payout-evidence-{holder}.json"));
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
    let label = &format!("journey redemption: wallet terminal payout ({holder})") as &str;
    let (landed, compute) = harvest_dir(rpc, label, &journal_dir, &mut spine.transactions);
    match outcome {
        Ok(passes) => {
            let document = read_json(&evidence)?;
            spine.paid_atoms = spine
                .paid_atoms
                .checked_add(reported_payout_atoms(&document)?)
                .ok_or_else(|| Error::new("the campaign payout total overflowed"))?;
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
            spine
                .reports
                .insert(format!("redemption-{holder}"), document);
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
                format!("redemption-{holder}"),
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

/// Close one emptied seller or buyer Position through the shipped wallet
/// exterior, using the finalized Direct history as its source authority.
#[allow(clippy::too_many_arguments)]
pub(crate) fn close_direct_position(
    rpc: &mut Rpc,
    context: &SpineContextV1<'_>,
    spine: &mut SpineV1,
    holder: &str,
    owner_role: &str,
    owner: Pubkey,
    direct_finalized: &Path,
    fee_payer: Pubkey,
    fee_payer_keypair: &Path,
) -> Result<()> {
    close_position(
        rpc,
        context,
        spine,
        holder,
        "direct-terminal",
        owner,
        context.key(owner_role)?,
        vec![
            "--direct-evidence".to_owned(),
            direct_finalized.display().to_string(),
            "--plan".to_owned(),
            context.plan.display().to_string(),
            "--market-input".to_owned(),
            context.market_input.display().to_string(),
            "--campaign-evidence".to_owned(),
            context.campaign_report.display().to_string(),
            "--position-owner".to_owned(),
            owner.to_string(),
        ],
        fee_payer,
        fee_payer_keypair,
    )
}

/// Close one empty admission-only Position through the same shipped exterior.
/// The admission report remains the authority for its owner and coordinate;
/// no Direct manifest is allowed to speak for a party that never traded.
#[allow(clippy::too_many_arguments)]
pub(crate) fn close_participant_position(
    rpc: &mut Rpc,
    context: &SpineContextV1<'_>,
    spine: &mut SpineV1,
    holder: &str,
    owner: Pubkey,
    owner_keypair: &Path,
    participant_evidence: &Path,
    fee_payer: Pubkey,
    fee_payer_keypair: &Path,
) -> Result<()> {
    close_position(
        rpc,
        context,
        spine,
        holder,
        "participant",
        owner,
        owner_keypair.to_path_buf(),
        vec![
            "--participant-evidence".to_owned(),
            participant_evidence.display().to_string(),
        ],
        fee_payer,
        fee_payer_keypair,
    )
}

#[allow(clippy::too_many_arguments)]
fn close_position(
    rpc: &mut Rpc,
    context: &SpineContextV1<'_>,
    spine: &mut SpineV1,
    holder: &str,
    expected_source: &str,
    owner: Pubkey,
    owner_keypair: PathBuf,
    mut source_arguments: Vec<String>,
    fee_payer: Pubkey,
    fee_payer_keypair: &Path,
) -> Result<()> {
    let stage =
        format!("redemption: {holder}'s empty protocol Position and admission record are closed");
    let evidence = context.work.join(format!("position-close-{holder}.json"));
    if !evidence.exists() {
        let mut arguments = vec!["--rpc-url".to_owned(), context.rpc_url.to_owned()];
        arguments.append(&mut source_arguments);
        arguments.extend([
            "--fee-payer".to_owned(),
            fee_payer.to_string(),
            "--position-owner-keypair".to_owned(),
            owner_keypair.display().to_string(),
            "--fee-payer-keypair".to_owned(),
            fee_payer_keypair.display().to_string(),
            "--evidence".to_owned(),
            evidence.display().to_string(),
            "--execute".to_owned(),
        ]);
        if let Err(error) = crate::user_position_close::run(arguments) {
            spine.refused(
                &stage,
                &error.to_string(),
                format!(
                    "`local-private-validator-user-position-close-v1 --execute` refused for \
                     {holder}: {error}. The shipped exterior authenticates either finalized \
                     Direct terminal history or the admission-only participant history before \
                     it opens the owner's key."
                ),
            );
            return Ok(());
        }
    }

    let document = read_json(&evidence)?;
    let owner_text = owner.to_string();
    let market_text = context.market.to_string();
    let exact = document.get("schema").and_then(Value::as_str)
        == Some("dclutch-user-position-close-evidence-v1")
        && document.get("cluster").and_then(Value::as_str) == Some("owned-loopback")
        && document.get("phase").and_then(Value::as_str) == Some("finalized")
        && document.get("authorizedMutation").and_then(Value::as_bool) == Some(true)
        && document.pointer("/plan/sourceKind").and_then(Value::as_str) == Some(expected_source)
        && document.pointer("/plan/market").and_then(Value::as_str) == Some(market_text.as_str())
        && document.pointer("/plan/owner").and_then(Value::as_str) == Some(owner_text.as_str())
        && document
            .pointer("/finalized/positionClosed")
            .and_then(Value::as_bool)
            == Some(true)
        && document
            .pointer("/finalized/admissionClosed")
            .and_then(Value::as_bool)
            == Some(true);
    let position = document
        .pointer("/plan/position")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<Pubkey>().ok());
    let admission = document
        .pointer("/plan/admission")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<Pubkey>().ok());
    let (Some(position), Some(admission)) = (position, admission) else {
        spine.refused(
            &stage,
            "the Position-close report omitted canonical Position/admission addresses",
            "A completed close must name both accounts whose finalized absence it proves.".into(),
        );
        return Ok(());
    };
    let (landed, compute) = harvest_document(
        rpc,
        &format!("journey redemption: Position close ({holder})"),
        &document,
        &mut spine.transactions,
    );
    if !exact
        || landed != 1
        || rpc.account(position)?.is_some()
        || rpc.account(admission)?.is_some()
    {
        spine.refused(
            &stage,
            "the Position-close report did not bind one finalized close and two absent accounts",
            "A saved report is resumed only after its signature re-reads from finalized history, \
             its source/Market/owner fields match this holder, and both closed accounts are \
             absent on the same validator."
                .into(),
        );
        return Ok(());
    }
    spine.executed(
        &stage,
        landed,
        compute,
        format!(
            "`local-private-validator-user-position-close-v1 --execute` closed {holder}'s empty \
             Position and admission atomically. The finalized receipt credits their complete \
             live lamport balances to the Market RentCredit; the owner signs read-only."
        ),
    );
    spine
        .reports
        .insert(format!("position-close-{holder}"), document);
    Ok(())
}

// ---------------------------------------------------------------- retirement

/// Capture the immutable founding rows retirement will need before redemption
/// closes the user admission accounts that the refresh authenticates.
pub(crate) fn prepare_retirement_refresh_v1(context: &SpineContextV1<'_>) -> Result<PathBuf> {
    let refresh = context.work.join("refresh.json");
    if !refresh.exists() {
        crate::evidence_refresh::run_owned_loopback(vec![
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
        ])?;
    }
    Ok(refresh)
}

/// Close the fund, begin retiring, and drive the four checkpointed packets to
/// **Retired**.
///
/// Three shipped commands in order, and the order is the protocol's:
///
/// 1. `local-private-validator-refresh-evidence-v1` re-reads the founding's own
///    accounts at the current slot and refuses a world in which any immutable
///    founding record moved. Its output is what the next two consume.
/// 2. `local-private-validator-terminal-sequence-v1` walks
///    [`TerminalStageV1::ORDERED`], one stage per invocation, creating its own
///    exact-union routing table first.
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
    direct_public: &Path,
    direct_finalized: &Path,
    source_receipt: Pubkey,
    fee_payer: Pubkey,
    fee_payer_keypair: &Path,
) -> Result<()> {
    let refresh = prepare_retirement_refresh_v1(context)?;
    let refresh_stage = "retirement: the founding's evidence is refreshed against the live chain";
    spine.stages.push(StageReportV1 {
        stage: refresh_stage.into(),
        outcome: "executed".into(),
        transactions: 0,
        compute_units: 0,
        note: "Read-only by construction: the driver connects with a reads-only write policy and \
               refuses any founding record that moved since the founding sealed it."
            .into(),
    });

    // ---- 2. the terminal sequence, in the one admissible order
    //
    // The label used to name three of the six and name them out of order
    // ("CloseFund, BeginRetiring and the retirement replay handoff"), which is
    // the pre-PROGRAMS-18A ordering written into a string. The stage's contents
    // are `TerminalStageV1::ORDERED` and the note renders them; the label says
    // what the stage IS, and nothing here restates the order by hand.
    let sequence_stage =
        "retirement: the shipped driver walks the five pre-checkpoint terminal stages";
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
    let mut outcome = resume_until(
        |_| crate::terminal_sequence::run_terminal_sequence_owned_loopback_v1(sequence.clone()),
        || completion.exists(),
    );
    // THE ZERO-COUNT GATE BETWEEN STAGES TWO AND THREE, and why it is not
    // inside the sequence driver.
    //
    // `DirectCloseCapability` takes `outstanding_capabilities` to zero, and it
    // cannot run while the Direct root still holds an open maker root.
    // `direct_close_maker_v1` is the ONLY route in the protocol that ever
    // decrements `open_maker_root_count` -- its own module comment says so --
    // and it runs inside `Retiring`, because `consume_nonce_v2` refuses every
    // non-Open phase. So a market that was FILLED reaches the third of the six
    // and stops there until a separate shipped command runs: the devnet spine's
    // `34-close-maker`, `local-private-validator-direct-close-maker-v1` here.
    //
    // It is a separate command rather than a seventh stage because it is a
    // different program's route (`DCLTDMC1`, Trading), it is per maker replay
    // rather than per market, and `TerminalStageV1::ORDERED` is the order of
    // the six PROTOCOL MUTATIONS of the retirement -- a market that never
    // traded needs none of this and its sequence is complete without it. So the
    // campaign runs the loop, and only if the loop stops does it close the
    // replay and resume: the chain decides whether this act is needed, not a
    // flag here.
    // A terminal sequence can only ask for the maker-root decrement at its
    // DirectCloseCapability gate. Do not manufacture a close attempt after an
    // earlier terminal refusal: the driver named that earlier barrier, and a
    // maker close would merely add a second, unrelated refusal.
    if !completion.exists() && terminal_needs_maker_replay_close(&outcome) {
        let parameters_ready =
            found_protocol_parameters(rpc, context, spine, fee_payer, fee_payer_keypair);
        let upkeep_ready = parameters_ready
            && found_upkeep_vault(rpc, context, spine, fee_payer, fee_payer_keypair);
        // ONE CLOSE PER MAKER. A Direct fill opens a maker replay on BOTH
        // sides -- the manifest names `/seller/maker` and `/buyer/maker` and
        // the root counts both -- and `require_closable` demands
        // `open_maker_root_count == 0`, so closing one left the capability
        // close refusing `Successor(MakerRootCountInvariant)` with the count at
        // one (hbox `20260906T172418Z`, after the first close executed at
        // 101,252 CU).
        if upkeep_ready {
            for side in ["seller", "buyer"] {
                close_direct_maker_replay(
                    rpc,
                    context,
                    spine,
                    direct_public,
                    direct_finalized,
                    side,
                    fee_payer_keypair,
                );
            }
            outcome = resume_until(
                |_| {
                    crate::terminal_sequence::run_terminal_sequence_owned_loopback_v1(
                        sequence.clone(),
                    )
                },
                || completion.exists(),
            );
        }
    }
    let label = "journey retirement: terminal sequence";
    let (landed, compute) = harvest_dir(rpc, label, &journal_dir, &mut spine.transactions);
    match outcome {
        Ok(passes) => {
            let document = read_json(&completion)?;
            spine.executed(
                sequence_stage,
                landed,
                compute,
                format!(
                    "`local-private-validator-terminal-sequence-v1 --execute`, {passes} \
                     invocations, one durable stage each over the pre-checkpoint order -- {} -- \
                     with an exact-union routing table built by the same journal machinery in \
                     front. Aggregate retirement now belongs only to its four-packet campaign.",
                    terminal_precheckpoint_order_v1()
                ),
            );
            spine.reports.insert("terminal-sequence".into(), document);
        }
        Err((passes, error)) => {
            spine.refused(
                sequence_stage,
                &error,
                format!(
                    "The shipped terminal-sequence driver refused at invocation {passes}: \
                     {error}. {landed} of its acts had already finalized, over the \
                     pre-checkpoint order \
                     {}. Nothing past this stage is driven, and no retirement ledger is written \
                     for a sequence that did not complete.",
                    terminal_precheckpoint_order_v1()
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

    // ---- 3. the checkpoint campaign's own immutable routing table
    let table_stage = "retirement: publish the aggregate checkpoint routing table";
    let table_report = context.work.join("retirement-lookup-table.json");
    if !table_report.exists() {
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
            "--output".to_owned(),
            table_report.display().to_string(),
            "--execute".to_owned(),
        ];
        if let Err(error) =
            crate::aggregate_retirement_exterior::run_owned_loopback_lookup_table(arguments)
        {
            spine.refused(
                table_stage,
                &error.to_string(),
                "The aggregate checkpoint derives its own exact address union, publishes and \
                 freezes that table, then compiles all four packets over its read-back bytes."
                    .into(),
            );
            return Ok(());
        }
    }
    let table_document = read_json(&table_report)?;
    let market_text = context.market.to_string();
    let receipt_text = source_receipt.to_string();
    let payer_text = fee_payer.to_string();
    let table = table_document
        .get("lookupTable")
        .and_then(Value::as_str)
        .filter(|_| {
            table_document.get("executed").and_then(Value::as_bool) == Some(true)
                && table_document.get("market").and_then(Value::as_str)
                    == Some(market_text.as_str())
                && table_document.get("sourceReceipt").and_then(Value::as_str)
                    == Some(receipt_text.as_str())
                && table_document.get("payer").and_then(Value::as_str) == Some(payer_text.as_str())
        })
        .map(str::to_owned);
    let Some(table) = table else {
        spine.refused(
            table_stage,
            "the aggregate routing-table report did not bind the current market, receipt and payer",
            "A resumed journey accepts the prior table report only when its inputs match this \
             retirement. The four-packet campaign re-reads the table, proves its frozen \
             authority, exact address union and data digest before signing."
                .into(),
        );
        return Ok(());
    };
    let table_label = "journey retirement: aggregate checkpoint routing table";
    let (table_landed, table_compute) =
        harvest_document(rpc, table_label, &table_document, &mut spine.transactions);
    if table_landed == 0 {
        spine.refused(
            table_stage,
            "the aggregate routing-table report yielded no finalized transaction",
            "A table report is accepted only when at least one of its publication signatures \
             re-reads as a finalized signed packet from this validator."
                .into(),
        );
        return Ok(());
    }
    spine.executed(
        table_stage,
        table_landed,
        table_compute,
        "The canonical aggregate-retirement producer derived the four packets' exact address \
         union, published it under the retirement payer's authority, froze it, read it back, and \
         reproduced every packet's fixed wire width. A restart reuses this report and the \
         campaign re-authenticates the live frozen table before signing."
            .into(),
    );
    spine
        .reports
        .insert("aggregate-retirement-lookup-table".into(), table_document);

    // ---- 4. the four checkpointed packets
    let checkpoint_stage =
        "retirement: the four checkpointed aggregate-retirement packets, to Retired";
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

/// Whether the terminal driver stopped at the one gate that requires the
/// separate maker-replay close producer.
///
/// A refused terminal invocation is evidence about the current ordered stage.
/// Only the exact maker-root invariant permits the journey to write a separate
/// producer transaction before it resumes the terminal sequence.
fn terminal_needs_maker_replay_close(
    outcome: &std::result::Result<usize, (usize, String)>,
) -> bool {
    matches!(outcome, Err((_, error)) if error.contains("MakerRootCountInvariant"))
}

/// Found Custody's governed protocol parameters before planning a maker close.
///
/// `DirectCloseMaker` decodes this record while it derives its fee split. An
/// Upkeep vault alone cannot stand in for it: its header has a different
/// semantic owner and the close planner rightly refuses the vacant parameters
/// PDA as `ProtocolParameters(InvalidHeader)`.
fn found_protocol_parameters(
    rpc: &mut Rpc,
    context: &SpineContextV1<'_>,
    spine: &mut SpineV1,
    fee_payer: Pubkey,
    fee_payer_keypair: &Path,
) -> bool {
    let stage = "retirement: Custody ProtocolParameters are founded before Direct maker closure";
    let report = context.work.join("parameters-found.json");
    let custody = match read_json(context.plan)
        .ok()
        .and_then(|plan| {
            plan.pointer("/custody/program_id")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .and_then(|text| text.parse::<Pubkey>().ok())
    {
        Some(custody) => custody,
        None => {
            spine.refused(
                stage,
                "the checked plan omitted Custody's program_id",
                "`parameters-found` takes Custody's deployed program address from the checked \
                 plan; no caller-provided substitute is accepted."
                    .into(),
            );
            return false;
        }
    };
    let document = if report.exists() {
        match read_json(&report) {
            Ok(document) => document,
            Err(error) => {
                spine.refused(
                    stage,
                    &error.to_string(),
                    "The prior parameters producer report was unreadable, so the journey will \
                     not rerun a Found route whose existing record it has not authenticated."
                        .into(),
                );
                return false;
            }
        }
    } else {
        let arguments = vec![
            "--rpc-url".to_owned(),
            context.rpc_url.to_owned(),
            "--custody".to_owned(),
            custody.to_string(),
            "--payer".to_owned(),
            fee_payer.to_string(),
            "--payer-keypair".to_owned(),
            fee_payer_keypair.display().to_string(),
            "--execute".to_owned(),
        ];
        match crate::economics_successor::run_value(
            crate::economics_successor::RouteV1::Parameters,
            crate::economics_successor::ClusterV1::OwnedLoopback,
            arguments,
        ) {
            Ok(document) => {
                let bytes = match serde_json::to_vec_pretty(&document) {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        spine.refused(
                            stage,
                            &error.to_string(),
                            "The finalized parameters producer report could not be encoded for \
                             durable journey evidence."
                                .into(),
                        );
                        return false;
                    }
                };
                if let Err(error) = std::fs::write(&report, bytes) {
                    spine.refused(
                        stage,
                        &error.to_string(),
                        "The actual parameters Found finalized, but the journey could not \
                         persist its producer report; it will not continue to a maker close \
                         without that evidence."
                            .into(),
                    );
                    return false;
                }
                document
            }
            Err(error) => {
                spine.refused(
                    stage,
                    &error.to_string(),
                    format!(
                        "The shipped `parameters-found --execute` producer refused: {error}. \
                         The maker close is not planned because its governed parameters \
                         precondition has no authenticated producer evidence."
                    ),
                );
                return false;
            }
        }
    };
    if !is_exact_parameters_found_report(&document, custody) {
        spine.refused(
            stage,
            "the parameters producer report did not prove its canonical genesis poststate",
            "The journey requires the Custody-owned parameters record, an inactive proposal, \
             and zero Hoard movement before a Direct close can read its governed split."
                .into(),
        );
        return false;
    }
    let label = "journey retirement: Custody ProtocolParameters Found";
    let (landed, compute) = harvest_document(rpc, label, &document, &mut spine.transactions);
    if landed != 1 {
        spine.refused(
            stage,
            "the parameters producer report did not yield one finalized transaction",
            "Parameters Found is one producer act; the maker close is not planned unless its \
             exact signature re-reads from the chain."
                .into(),
        );
        return false;
    }
    spine.executed(
        stage,
        landed,
        compute,
        "`parameters-found --execute` founded Custody's canonical governed parameters record. \
         Direct maker close reads this exact record for its economic split; it is distinct from \
         the Upkeep vault that receives any Donation remainder."
            .into(),
    );
    spine.reports.insert("parameters-found".into(), document);
    true
}

fn is_exact_parameters_found_report(document: &Value, custody: Pubkey) -> bool {
    document.get("command").and_then(Value::as_str) == Some("parameters-found")
        && document.get("custody").and_then(Value::as_str) == Some(custody.to_string().as_str())
        && document
            .pointer("/poststate/recordOwner")
            .and_then(Value::as_str)
            == Some(custody.to_string().as_str())
        && document.pointer("/poststate/pendingProposal") == Some(&Value::Bool(false))
        && document.pointer("/poststate/hoardPrincipalMoved") == Some(&Value::from(0))
}

/// Found Custody's canonical Upkeep vault and receipt one nonzero Deposit.
///
/// `DirectCloseMaker` writes a protocol-owned Donation remainder after it has
/// authenticated this vault. The journey cannot seed that state: the Custody
/// `upkeep-found` producer owns both the Found instruction and the wallet's
/// voluntary Deposit receipt, and this stage records its two finalized
/// signatures before the maker-close driver is allowed to plan.
fn found_upkeep_vault(
    rpc: &mut Rpc,
    context: &SpineContextV1<'_>,
    spine: &mut SpineV1,
    fee_payer: Pubkey,
    fee_payer_keypair: &Path,
) -> bool {
    let stage = "retirement: Custody Upkeep is founded before Direct maker closure";
    let report = context.work.join("upkeep-found.json");
    let custody = match read_json(context.plan)
        .ok()
        .and_then(|plan| {
            plan.pointer("/custody/program_id")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .and_then(|text| text.parse::<Pubkey>().ok())
    {
        Some(custody) => custody,
        None => {
            spine.refused(
                stage,
                "the checked plan omitted Custody's program_id",
                "`upkeep-found` takes Custody's deployed program address from the checked plan; \
                 no caller-provided substitute is accepted."
                    .into(),
            );
            return false;
        }
    };
    let document = if report.exists() {
        match read_json(&report) {
            Ok(document) => document,
            Err(error) => {
                spine.refused(
                    stage,
                    &error.to_string(),
                    "The prior upkeep producer report was unreadable, so the journey will not \
                     rerun a Found route whose existing vault it has not authenticated."
                        .into(),
                );
                return false;
            }
        }
    } else {
        let arguments = vec![
            "--rpc-url".to_owned(),
            context.rpc_url.to_owned(),
            "--custody".to_owned(),
            custody.to_string(),
            "--payer".to_owned(),
            fee_payer.to_string(),
            "--amount".to_owned(),
            UPKEEP_FOUND_DEPOSIT_LAMPORTS_V1.to_string(),
            "--payer-keypair".to_owned(),
            fee_payer_keypair.display().to_string(),
            "--execute".to_owned(),
        ];
        match crate::economics_successor::run_value(
            crate::economics_successor::RouteV1::Upkeep,
            crate::economics_successor::ClusterV1::OwnedLoopback,
            arguments,
        ) {
            Ok(document) => {
                let bytes = match serde_json::to_vec_pretty(&document) {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        spine.refused(
                            stage,
                            &error.to_string(),
                            "The finalized Upkeep producer report could not be encoded for its \
                             durable journey evidence."
                                .into(),
                        );
                        return false;
                    }
                };
                if let Err(error) = std::fs::write(&report, bytes) {
                    spine.refused(
                        stage,
                        &error.to_string(),
                        "The actual Upkeep Found and Deposit Credit finalized, but the journey \
                         could not persist their producer report; it will not continue to a \
                         maker close without that evidence."
                            .into(),
                    );
                    return false;
                }
                document
            }
            Err(error) => {
                spine.refused(
                    stage,
                    &error.to_string(),
                    format!(
                        "The shipped `upkeep-found --execute` producer refused: {error}. The \
                         maker close is not planned because its Upkeep precondition has no \
                         authenticated producer evidence."
                    ),
                );
                return false;
            }
        }
    };
    if !is_exact_upkeep_found_report(&document, custody) {
        spine.refused(
            stage,
            "the Upkeep producer report did not prove its exact Found and Deposit poststate",
            "The journey requires the producer's custody identity, nonzero Deposit amount, and \
             zero unreceipted remainder before a Direct close can create its own Donation."
                .into(),
        );
        return false;
    }
    let label = "journey retirement: Custody Upkeep Found";
    let (landed, compute) = harvest_document(rpc, label, &document, &mut spine.transactions);
    if landed != 2 {
        spine.refused(
            stage,
            "the Upkeep producer report did not yield two finalized transactions",
            "Found and the nonzero Deposit are distinct producer acts; the maker close is not \
             planned unless both exact signatures re-read from the chain."
                .into(),
        );
        return false;
    }
    spine.executed(
        stage,
        landed,
        compute,
        "`upkeep-found --execute` founded Custody's canonical Upkeep vault then receipted one \
         nonzero voluntary Deposit. It does not impersonate the Direct close-maker Donation CPI; \
         that remainder remains the close route's own economic fact."
            .into(),
    );
    spine.reports.insert("upkeep-found".into(), document);
    true
}

/// The exact economic fact the journey requires from the Upkeep producer.
///
/// The producer may create the vault only once, so a resumed journey must be
/// able to authenticate its durable report without re-running Found. A Deposit
/// is a wallet act; a Donation is a Direct close-maker CPI act and cannot stand
/// in for it.
fn is_exact_upkeep_found_report(document: &Value, custody: Pubkey) -> bool {
    let expected_amount = Value::from(UPKEEP_FOUND_DEPOSIT_LAMPORTS_V1);
    document.get("command").and_then(Value::as_str) == Some("upkeep-found")
        && document.get("custody").and_then(Value::as_str) == Some(custody.to_string().as_str())
        && document.pointer("/poststate/creditAmount") == Some(&expected_amount)
        && document
            .pointer("/poststate/creditClass")
            .and_then(Value::as_str)
            == Some("upkeep:deposit")
        && document.pointer("/poststate/unreceiptedLamports") == Some(&Value::from(0))
}

/// Close the one Direct maker replay the fill opened, so the capability close
/// can reach its zero-count gate.
///
/// `local-private-validator-direct-close-maker-v1`, the shipped command. The
/// ONLY coordinate this passes is the maker, `/replaySetup/maker` out of the
/// fill's own public manifest; the driver finds that maker's unique replay in
/// the authenticated Direct history itself. The first version of this stage
/// also passed `--maker-replay`, using the manifest's `/replaySetup/custodyReplay`
/// -- which is the CLAIMS CUSTODY replay and not the Direct maker child -- and
/// the driver refused it by name (`--maker-replay differs from the
/// authenticated Direct maker child`, hbox `20260906T152908Z`). The flag exists
/// for an operator who has a reason to pin one; a campaign that has no such
/// reason is a second author with worse information.
///
/// A refusal is a FINDING and never a stop -- the driver refuses BY NAME on the
/// two states a real market can be in (`CloseMakerFeeOutstanding` when the
/// Direct fee is unsettled, live intents when the replay is not drained), and
/// both are worth reading beside the sequence that stopped. There is nothing to
/// close on a market that never traded, and that refusal is equally a finding.
fn close_direct_maker_replay(
    rpc: &mut Rpc,
    context: &SpineContextV1<'_>,
    spine: &mut SpineV1,
    public: &Path,
    finalized: &Path,
    side: &str,
    fee_payer_keypair: &Path,
) {
    let stage = &format!(
        "retirement: the {side}'s Direct maker replay is closed so the capability close can reach \
         its zero-count gate"
    ) as &str;
    if !public.exists() || !finalized.exists() {
        spine.stages.push(StageReportV1 {
            stage: stage.into(),
            outcome: "not-driven".into(),
            transactions: 0,
            compute_units: 0,
            note: "The fill stage left no finalized Direct evidence, so this market opened no \
                   maker replay and there is no decrement to drive. Whatever stopped the terminal \
                   sequence is not the zero-count gate."
                .into(),
        });
        return;
    }
    let Ok(document) = read_json(&public) else {
        return;
    };
    let field = |pointer: &str| -> Option<Pubkey> {
        document
            .pointer(pointer)
            .and_then(Value::as_str)
            .and_then(|text| text.parse::<Pubkey>().ok())
    };
    let Some(maker) = field(&format!("/{side}/maker")) else {
        spine.refused(
            stage,
            "the fill's public manifest names no maker for this side",
            format!(
                "`/{side}/maker` is the producer's own record of whose replay it opened; a \
                 manifest without it is a finding about that document."
            ),
        );
        return;
    };
    let Ok(evidence) = context
        .dir("close-maker")
        .map(|dir| dir.join(format!("close-maker-{side}.json")))
    else {
        return;
    };
    let arguments = vec![
        "--rpc-url".to_owned(),
        context.rpc_url.to_owned(),
        "--plan".to_owned(),
        context.plan.display().to_string(),
        "--market-input".to_owned(),
        context.market_input.display().to_string(),
        "--campaign-evidence".to_owned(),
        context.campaign_report.display().to_string(),
        "--direct-evidence".to_owned(),
        finalized.display().to_string(),
        "--market".to_owned(),
        context.market.to_string(),
        "--maker".to_owned(),
        maker.to_string(),
        "--evidence".to_owned(),
        evidence.display().to_string(),
        "--fee-payer-keypair".to_owned(),
        fee_payer_keypair.display().to_string(),
        "--execute".to_owned(),
    ];
    let outcome = crate::direct_close_maker::run_owned_loopback_v1(arguments);
    let label = &format!("journey retirement: Direct maker replay close ({side})") as &str;
    let (landed, compute) = match read_json(&evidence) {
        Ok(document) => {
            let result = harvest_document(rpc, label, &document, &mut spine.transactions);
            spine
                .reports
                .insert(format!("close-maker-{side}"), document);
            result
        }
        Err(_) => (0, 0),
    };
    match outcome {
        Ok(()) => spine.executed(
            stage,
            landed,
            compute,
            format!(
                "`local-private-validator-direct-close-maker-v1 --execute` closed maker {maker}'s \
                 replay. It is the only route that decrements \
                 `open_maker_root_count`, it runs inside Retiring, and the count it took down is \
                 what `DirectCloseCapability` gates on. Permissionless: no party to the market \
                 signed it."
            ),
        ),
        Err(error) => spine.refused(
            stage,
            &error.to_string(),
            format!(
                "The shipped maker close refused: {error}. It refuses at PLAN time, before a key \
                 is opened, so this is a statement about the replay's own authenticated bytes -- \
                 an unsettled Direct fee or a live intent -- and not about a spent signature."
            ),
        ),
    }
}

/// SHA-256 of a file, in the spelling the `--expected-*-sha256` flags take.
fn digest_of(path: &Path) -> Result<String> {
    use sha2::Digest;
    Ok(crate::plan::hex(&sha2::Sha256::digest(std::fs::read(
        path,
    )?)))
}

// The shipped exterior emits exact quantities as decimal strings so a JSON
// consumer never rounds them through a floating-point number.
fn reported_payout_atoms(document: &Value) -> Result<u128> {
    let text = document
        .get("payout")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("the payout evidence requires a decimal-string `payout`"))?;
    let amount = text
        .parse::<u128>()
        .map_err(|_| Error::new("the payout evidence contains an invalid atom quantity"))?;
    if amount.to_string() != text {
        return Err(Error::new(
            "the payout evidence contains a noncanonical atom quantity",
        ));
    }
    Ok(amount)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maker_close_producer_is_only_admitted_at_its_exact_terminal_gate() {
        assert!(terminal_needs_maker_replay_close(&Err((
            3,
            "Successor(MakerRootCountInvariant)".into(),
        ))));
        assert!(!terminal_needs_maker_replay_close(&Err((
            3,
            "Successor(InvalidUpkeepVault)".into(),
        ))));
        assert!(!terminal_needs_maker_replay_close(&Ok(6)));
    }

    #[test]
    fn parameters_found_report_requires_custody_owned_genesis_poststate() {
        let custody = Pubkey::new_unique();
        let valid = serde_json::json!({
            "command": "parameters-found",
            "custody": custody.to_string(),
            "poststate": {
                "recordOwner": custody.to_string(),
                "pendingProposal": false,
                "hoardPrincipalMoved": 0,
            },
        });
        assert!(is_exact_parameters_found_report(&valid, custody));

        let proposal = serde_json::json!({
            "command": "parameters-found",
            "custody": custody.to_string(),
            "poststate": {
                "recordOwner": custody.to_string(),
                "pendingProposal": true,
                "hoardPrincipalMoved": 0,
            },
        });
        assert!(!is_exact_parameters_found_report(&proposal, custody));

        let foreign_owner = serde_json::json!({
            "command": "parameters-found",
            "custody": custody.to_string(),
            "poststate": {
                "recordOwner": Pubkey::new_unique().to_string(),
                "pendingProposal": false,
                "hoardPrincipalMoved": 0,
            },
        });
        assert!(!is_exact_parameters_found_report(&foreign_owner, custody));
    }

    #[test]
    fn upkeep_found_report_requires_wallet_deposit_and_zero_remainder() {
        let custody = Pubkey::new_unique();
        let valid = serde_json::json!({
            "command": "upkeep-found",
            "custody": custody.to_string(),
            "poststate": {
                "creditAmount": UPKEEP_FOUND_DEPOSIT_LAMPORTS_V1,
                "creditClass": "upkeep:deposit",
                "unreceiptedLamports": 0,
            },
        });
        assert!(is_exact_upkeep_found_report(&valid, custody));

        let donation = serde_json::json!({
            "command": "upkeep-found",
            "custody": custody.to_string(),
            "poststate": {
                "creditAmount": UPKEEP_FOUND_DEPOSIT_LAMPORTS_V1,
                "creditClass": "Donation",
                "unreceiptedLamports": 0,
            },
        });
        assert!(!is_exact_upkeep_found_report(&donation, custody));

        let unreceipted = serde_json::json!({
            "command": "upkeep-found",
            "custody": custody.to_string(),
            "poststate": {
                "creditAmount": UPKEEP_FOUND_DEPOSIT_LAMPORTS_V1,
                "creditClass": "upkeep:deposit",
                "unreceiptedLamports": 1,
            },
        });
        assert!(!is_exact_upkeep_found_report(&unreceipted, custody));
    }

    #[test]
    fn payout_evidence_preserves_exact_decimal_atoms() {
        assert_eq!(
            reported_payout_atoms(&serde_json::json!({"payout":"9007199254740993"})).unwrap(),
            9_007_199_254_740_993
        );
        assert_eq!(
            reported_payout_atoms(&serde_json::json!({"payout":"0"})).unwrap(),
            0
        );
        for value in [serde_json::json!({}), serde_json::json!({"payout":1})] {
            assert_eq!(
                reported_payout_atoms(&value).unwrap_err().to_string(),
                "the payout evidence requires a decimal-string `payout`"
            );
        }
        assert_eq!(
            reported_payout_atoms(&serde_json::json!({"payout":"-1"}))
                .unwrap_err()
                .to_string(),
            "the payout evidence contains an invalid atom quantity"
        );
        assert_eq!(
            reported_payout_atoms(&serde_json::json!({"payout":"01"}))
                .unwrap_err()
                .to_string(),
            "the payout evidence contains a noncanonical atom quantity"
        );
    }

    /// The aperture is a READ, and a read that finds nothing adds nothing.
    ///
    /// The three cases are the three a landed report can present: the buyer,
    /// who carries the collateral leg and creates two accounts; the stranger,
    /// whose report has no `collateral` object at all and must contribute one
    /// entry rather than a placeholder; and a value that is present and is not
    /// an address, which must not enter a conservation law as a default key.
    /// L1 is what reports the resulting gap, in atoms, and this keeps a bad
    /// read from becoming a wrong balance instead of a visible one.
    #[test]
    fn the_aperture_names_only_what_a_report_actually_carries() {
        let position = Pubkey::new_unique();
        let token = Pubkey::new_unique();
        let mut spine = SpineV1::new();
        let buyer = serde_json::json!({
            "intent": {"position": position.to_string()},
            "collateral": {"intent": {"participantTokenAccount": token.to_string()}},
        });
        spine.admit_account(
            "buyer_position".into(),
            &buyer,
            "/intent/position",
            ApertureRoleV1::Position,
        );
        spine.admit_account(
            "buyer_delegated_collateral".into(),
            &buyer,
            "/collateral/intent/participantTokenAccount",
            ApertureRoleV1::Collateral,
        );
        let stranger = serde_json::json!({"intent": {"position": "not an address"}});
        spine.admit_account(
            "stranger_position".into(),
            &stranger,
            "/intent/position",
            ApertureRoleV1::Position,
        );
        spine.admit_account(
            "stranger_delegated_collateral".into(),
            &stranger,
            "/collateral/intent/participantTokenAccount",
            ApertureRoleV1::Collateral,
        );
        let named: Vec<(&str, Pubkey)> = spine
            .aperture
            .iter()
            .map(|entry| (entry.label.as_str(), entry.address))
            .collect();
        assert_eq!(
            named,
            vec![
                ("buyer_position", position),
                ("buyer_delegated_collateral", token)
            ]
        );
    }

    /// The harvester walks for the KEY, so a driver that grows a journal array
    /// is harvested without this file learning its shape.
    ///
    /// The rule is `signature` or any `*Signature`, and it was three named
    /// keys until run 8. The Direct capability activation reports its own act
    /// under `activationSignature`, so the harvest took the four routing-table
    /// transactions and MISSED the 505,381 CU activation: the stage reported
    /// 33,015 CU for a stage whose whole subject cost fifteen times that, and
    /// nothing failed. This test's old negative case was `notASignature`, which
    /// was chosen against an exact-match rule and is signature-shaped under a
    /// suffix one; the negative that carries the meaning is a key that does not
    /// name a signature at all.
    #[test]
    fn every_signature_key_is_found_at_every_depth() {
        let document = serde_json::json!({
            "signature": "one",
            "landed": {"signature": "two"},
            "journals": [{"signature": "three"}, {"nested": {"signature": "four"}}],
            "activationSignature": "five",
            "note": "not a signature",
            "slot": 91,
        });
        let mut found = Vec::new();
        signatures_in(&document, &mut found);
        // Order is the document's own object order, which serde_json sorts by
        // key, so the assertion is about the SET: the point is that no depth
        // and no array nesting hides a signature, not that the walk emits them
        // in the order a reader of the literal above would guess.
        found.sort();
        assert_eq!(found, vec!["five", "four", "one", "three", "two"]);
    }

    #[test]
    fn a_noop_resolution_setup_event_resumes_without_a_fake_transaction() {
        let document = serde_json::json!({
            "schema": "dclutch-held-resolution-setup-journal-v1",
            "events": [
                {"kind": "local-campaign-execution-funding", "transaction": null},
                {"kind": "exact-protocol-rent-prepay", "transaction": {
                    "signature": "an actual transaction-shaped row"
                }},
            ],
        });
        assert_eq!(
            resolution_setup_transaction_values_v1(&document)
                .expect("null means the setup needed no transaction"),
            vec![serde_json::json!({"signature": "an actual transaction-shaped row"})]
        );
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

    /// Polling a durable `Submitted` signature is one act regardless of how
    /// many byte-identical successful driver returns precede finality.
    #[test]
    fn pending_finality_polls_do_not_consume_the_durable_act_ceiling() {
        let invocations = std::cell::Cell::new(0_usize);
        await_durable_progress_v1(
            || {
                invocations.set(invocations.get() + 1);
                Ok(())
            },
            || Ok(invocations.get() == 3),
            "test durable action",
            Duration::from_secs(1),
            Duration::ZERO,
        )
        .expect("the third poll observes finality");
        assert_eq!(invocations.get(), 3);
        assert!(invocations.get() < RESUMPTION_CEILING_V1);
    }
}
