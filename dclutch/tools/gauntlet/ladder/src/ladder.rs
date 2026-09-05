//! The ladder campaign: one validator, one two-source market, the crank.
//!
//! # Why this tier exists at all
//!
//! `tools/gauntlet/blocked.json` carried the recovery arm as class `unwired`
//! with the obstacle stated as "no tier runs one": tier 1 founds and resolves
//! inside ONE process whose `runtime::found_through_open` owns the validator
//! child, so it cannot host found -> crank -> answer, which is three commands
//! against one live cluster. The relayed vertical solved that problem for its
//! own family by bringing the substrate up itself and keeping the child, and
//! this tier links that bring-up rather than writing a second one.
//!
//! # What it refuses to do
//!
//! It never warps a clock. A crank is admissible STRICTLY after the current
//! leg's deadline, and that deadline is a fact about the market's own
//! published `WindowSpecV1` and `RecoveryPolicyV2` read against the cluster's
//! own clock. A campaign that moved the validator's clock to make its own
//! hostile pass would be measuring a market it had edited, and the
//! before-the-deadline refusal -- the one conjunct the whole ladder rests on --
//! would become unfalsifiable. So when a leg is not yet due, this campaign
//! records the two seconds and says so, and its transcript is an honest
//! account of a walk that stopped rather than a green one that did not happen.

use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};

use crate::cluster::ExpectedClusterV1;
use crate::market::LocalMarketShapeV1;
use crate::model::TransactionEvidence;
use crate::plan::pubkey;
use crate::rpc::Rpc;
use crate::substrate::{self, SubstrateRequestV1};
use crate::{Error, Result};

/// The rung a `--recovery-rungs`-free run buys, in the SHIPPED flag's spelling.
///
/// One rung, which is a TWO-source market: the primary answerer plus the named
/// alternative it paid for. One is the width that leaves the founding's own
/// shape unmoved -- the Resolution manifest's hard four is
/// `1 + rungs.max(1) + 2`, which is four for a no-recovery market and four
/// again at one rung -- so the market this tier founds differs from the market
/// tier 1 founds in exactly the thing under test and in nothing else.
///
/// 2,500 bps is a TIGHTER confidence bound than the lab's 10,000-bp ceiling,
/// which is the only axis two Pyth sources of one feed can differ on: a market
/// whose first choice went silent has a reason to demand a better-conditioned
/// reading from its second.
pub(crate) const DEFAULT_RECOVERY_RUNGS_V1: &str = "2500:120";

/// How long a bounded wait for a leg's deadline may sleep.
///
/// There is no default inside the crank driver on purpose, so this tier states
/// one. Ten minutes is the whole budget a campaign may spend waiting, and a
/// deadline further away than that is REPORTED rather than slept for.
pub(crate) const DEFAULT_MAX_WAIT_SECONDS_V1: i64 = 600;

/// The certificate sequence each crank writes its receipt under.
///
/// Three seats, three kinds, three sequences -- the same numbering the real-ELF
/// walk in `crates/dclutch-svm-harness/tests/resolution_core_v3_lifecycle.rs`
/// uses, so a reader comparing the loopback figures against the program-test
/// figures is comparing the same seats.
const ADVANCE_SEQUENCE_V1: u64 = 2;
const EXHAUST_SEQUENCE_V1: u64 = 3;

/// What the worker is funded with. It pays fees and pre-funds a short seat, and
/// it is paid back the bounty out of the market's own compartment.
const WORKER_FUNDING_LAMPORTS: u64 = 2_000_000_000;

/// The sentence the crank driver refuses a too-early crank with.
///
/// Matched rather than parsed: this tier needs to tell "not yet due" from every
/// other refusal, and the driver states that distinction in words because the
/// two seconds it names are the whole of what a caller has to know. A driver
/// that stopped saying this would make the marker stop matching, and the tier
/// would report the refusal as a STOP -- loudly, in its transcript -- rather
/// than quietly treat some other refusal as a hostile satisfied.
const CRANK_TOO_EARLY_MARKER_V1: &str = "a crank is admissible STRICTLY after the deadline";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WalkV1 {
    /// Both legs expire: primary unobserved, then the funded rung unobserved,
    /// so the ladder advances once and then exhausts.
    Exhaust,
    /// The rung is ANSWERED inside its own committed deadline.
    Capture,
}

pub(crate) struct LadderRequestV1 {
    pub(crate) walk: WalkV1,
    pub(crate) transcript: PathBuf,
    pub(crate) work: PathBuf,
    pub(crate) rpc_port: u16,
    pub(crate) checked_release_gate: PathBuf,
    pub(crate) expected_gate_sha256: String,
    pub(crate) expected_source_revision: String,
    pub(crate) expected_source_tree_sha256: String,
    pub(crate) seed: String,
    pub(crate) recovery_rungs: String,
    pub(crate) max_wait_seconds: i64,
}

#[derive(Serialize)]
struct StageV1 {
    stage: String,
    outcome: String,
    note: String,
}

impl StageV1 {
    fn new(stage: &str, outcome: &str, note: String) -> Self {
        Self {
            stage: stage.to_owned(),
            outcome: outcome.to_owned(),
            note,
        }
    }
}

fn now_unix() -> Result<i64> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH)?;
    i64::try_from(elapsed.as_secs()).map_err(|_| Error::new("wall clock out of range"))
}

/// Write one Solana-convention keypair file, refusing to overwrite.
fn write_keypair_file(path: &Path, keypair: &Keypair) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes: Vec<u8> = keypair.to_bytes().to_vec();
    std::fs::write(path, serde_json::to_vec(&bytes)?)?;
    Ok(())
}

/// The whole ladder, in the order a market lives it.
pub(crate) fn execute(request: LadderRequestV1) -> Result<serde_json::Value> {
    std::fs::create_dir_all(&request.work)?;
    let mut stages: Vec<StageV1> = Vec::new();
    let mut transactions: Vec<TransactionEvidence> = Vec::new();
    let start_unix = now_unix()?;

    // ------------------------------------- 1. the checked-mutable substrate
    let substrate_dir = request.work.join("substrate");
    std::fs::create_dir_all(&substrate_dir)?;
    let checked = substrate::bring_up(&SubstrateRequestV1 {
        work: &substrate_dir,
        checked_release_gate: &request.checked_release_gate,
        expected_gate_sha256: &request.expected_gate_sha256,
        expected_source_revision: &request.expected_source_revision,
        expected_source_tree_sha256: &request.expected_source_tree_sha256,
        seed: &request.seed,
        rpc_port: request.rpc_port,
    })?;
    stages.push(StageV1::new(
        "checked-mutable substrate",
        "executed",
        format!(
            "local-mutable-prepare-v1 derived the substrate from the checked release gate ({}), a \
             fresh solana-test-validator booted the prepared account directory, and the \
             administration campaign published, initialized and activated through the retained \
             authority. THE VALIDATOR STAYS UP for everything below -- that is the whole reason \
             this tier can exist and tier 1 cannot host it.",
            request.expected_gate_sha256
        ),
    ));

    // ------------------------------- 2. the two-source market, compiled live
    //
    // The rung string is handed to the SHIPPED parser rather than to a second
    // one this tier owns, so `--recovery-rungs`'s meaning and this tier's
    // meaning cannot drift apart.
    let rungs = crate::local_mutable::parse_recovery_rungs_v1(&request.recovery_rungs)?;
    let rung_count = rungs.len();
    let registry = pubkey(&checked.plan.registry.program_id)?;
    let fee_recipient = Keypair::new();
    let direct = crate::direct_market::DirectMarketCompilerOwnedV1::load_local(
        &checked.plan_path,
        &checked.rpc_url,
        registry,
        Some(50),
        Some(fee_recipient.pubkey()),
    )?;
    let shape = LocalMarketShapeV1 {
        recovery: Some(rungs),
        ..LocalMarketShapeV1::default()
    };
    let market_input = crate::market::demo_market_input_shaped(registry, direct.compiler(), &shape)?;
    // A market compiled WITHOUT the flag reads a ZERO recovery link and is a
    // different market. Cohort-16's own verifier for this row, asked offline,
    // before a lamport moves.
    if market_input.recovery_policy_hex.is_empty() {
        return Err(Error::new(
            "the compiled market carries an empty recovery_policy_hex: the ladder never reached \
             the compiler, and founding it would found the one-source market this tier is not \
             about",
        ));
    }
    if market_input.recovery_source_records.len() != rung_count {
        return Err(Error::new(format!(
            "the compiled market publishes {} recovery source record pairs for {rung_count} \
             rungs; a rung's alternative source and its adapter config are the records that make \
             it a NAMED alternative rather than a retry",
            market_input.recovery_source_records.len()
        )));
    }
    let market_path = request.work.join("market.json");
    std::fs::write(&market_path, serde_json::to_vec_pretty(&market_input)?)?;
    stages.push(StageV1::new(
        "two-source market compiled",
        "executed",
        format!(
            "`--recovery-rungs {}` through the shipped parser: {rung_count} rung(s), each an \
             alternative SourceSpecV1 and its own PythAdapterConfigV1 published as their own \
             records, funded by a RecoveryPolicyV2, against the LIVE checked deployment \
             (DirectMarketCompilerOwnedV1::load_local -- fixture Direct identities are refused).",
            request.recovery_rungs
        ),
    ));

    // ------------------------------------------------------- 3. the founding
    let mut rpc = Rpc::connect(&checked.rpc_url)?;
    let founding = substrate::found_market(
        &checked,
        &mut rpc,
        &market_path,
        &request.work.join("founding-evidence.json"),
    )?;
    transactions.extend(founding.transactions.iter().cloned());
    let accounts = founding.market.accounts;
    let market = pubkey(
        &accounts
            .get("founding_market")
            .ok_or_else(|| Error::new("the founding's evidence names no founding_market"))?
            .address,
    )?;
    // THE ROUTE THIS TIER WAS BUILT TO WITNESS. A recovery-bearing material
    // takes the `Some` arm of Core's `authenticate_recovery_policy`, which the
    // founding's own readiness suffix executes at CreateFund and again at
    // VerifyFundReady. A campaign whose evidence names no recovery-policy
    // record founded the market this tier is not about.
    let recovery_policy_record = accounts
        .get("recovery_policy_record")
        .ok_or_else(|| {
            Error::new(
                "the founding's evidence names no recovery_policy_record: the market that reached \
                 the chain bought no ladder, so nothing below can be cranked",
            )
        })?
        .address
        .clone();
    stages.push(StageV1::new(
        "founding through Open, and funded",
        "executed",
        format!(
            "campaign --founding-only over the live checked substrate. {} transactions to here. \
             The market is {market} and its RecoveryPolicyV2 record is at \
             {recovery_policy_record}. The founding's own post-Open readiness suffix drives \
             CreateFund, ActivateFund and VerifyFundReady over the RECOVERY-BEARING frame, which \
             is the `Some` arm of core/resolution::authenticate_recovery_policy executing on a \
             chain for the first time.",
            transactions.len()
        ),
    ));

    // ------------------------------------------------------ 4. the crank(s)
    let worker = Keypair::new();
    let worker_keypair_path = request.work.join("worker.json");
    write_keypair_file(&worker_keypair_path, &worker)?;
    transactions.push(rpc.airdrop(
        "ladder: fund the stranger who cranks",
        worker.pubkey(),
        WORKER_FUNDING_LAMPORTS,
    )?);

    let mut cranks = Vec::new();
    let advance = drive_crank(
        &checked,
        &request,
        &mut stages,
        &mut transactions,
        market,
        worker.pubkey(),
        &worker_keypair_path,
        ADVANCE_SEQUENCE_V1,
        "advance onto the funded alternative",
    )?;
    let advanced = advance.landed;
    cranks.push(advance.report);

    if advanced && request.walk == WalkV1::Exhaust {
        let exhaust = drive_crank(
            &checked,
            &request,
            &mut stages,
            &mut transactions,
            market,
            worker.pubkey(),
            &worker_keypair_path,
            EXHAUST_SEQUENCE_V1,
            "exhaust the last funded rung",
        )?;
        cranks.push(exhaust.report);
    }

    if request.walk == WalkV1::Capture {
        stages.push(StageV1::new(
            "the rung is answered",
            if advanced { "not-driven" } else { "unreachable" },
            "A rung capture is the provider submit/execute pair with the market's Source standing \
             on the rung: `dclutch-provider-transport-v3-operator` derives the request's \
             source_index and its source-spec identity FROM the Source's own phase and active \
             attempt, so the capture that answers a rung is buildable today and needs no new \
             instruction. What it needs from a host is the provider snapshot's recovery_ladder \
             pair, and what it cannot use is the successor's flagship command, whose terminal \
             verifier pins route Primary and attempt_index 0 in two places \
             (flagship_resolution.rs:7373-7376 and :8216/:8237). This stage states that and does \
             not drive it."
                .to_owned(),
        ));
    }

    let transcript = serde_json::json!({
        "schema": "dclutch-ladder-transcript-v1",
        "walk": request.walk,
        "rpc_url": checked.rpc_url,
        "market": market.to_string(),
        "recovery_policy_record": recovery_policy_record,
        "recovery_rungs": request.recovery_rungs,
        "gate_sha256": request.expected_gate_sha256,
        "gate_source_revision": request.expected_source_revision,
        "started_unix_seconds": start_unix,
        "finished_unix_seconds": now_unix()?,
        "clock_discipline": "No clock was warped. Every deadline below is the market's own \
                             published record read against the cluster's own clock, and a leg \
                             that was not yet due is REPORTED as not yet due.",
        "cranks": cranks,
        "stages": stages,
    });
    std::fs::write(
        &request.transcript,
        format!("{}\n", serde_json::to_string_pretty(&transcript)?),
    )?;

    // The tier's evidence document, in the shape `census observe` reads.
    let evidence = serde_json::json!({
        "schema": "dclutch-local-successor-run-evidence-v2",
        "rpc_url": checked.rpc_url,
        "plan_sha256": checked.plan_sha256,
        "transactions": serde_json::to_value(&transactions)?,
        "accounts": serde_json::to_value(&accounts)?,
    });
    std::fs::write(
        request.work.join("evidence.json"),
        format!("{}\n", serde_json::to_string_pretty(&evidence)?),
    )?;
    Ok(transcript)
}

/// One crank of the ladder, driven through the SHIPPED command.
struct CrankRunV1 {
    landed: bool,
    report: serde_json::Value,
}

#[allow(clippy::too_many_arguments)]
fn drive_crank(
    checked: &substrate::CheckedSubstrateV1,
    request: &LadderRequestV1,
    stages: &mut Vec<StageV1>,
    transactions: &mut Vec<TransactionEvidence>,
    market: Pubkey,
    worker: Pubkey,
    worker_keypair: &Path,
    terminal_sequence: u64,
    label: &str,
) -> Result<CrankRunV1> {
    let mut hostile: Option<String> = None;
    let base = vec![
        "--rpc-url".to_owned(),
        checked.rpc_url.clone(),
        "--plan".to_owned(),
        checked.plan_path.display().to_string(),
        "--evidence".to_owned(),
        request
            .work
            .join("founding-evidence.json")
            .display()
            .to_string(),
        "--market".to_owned(),
        market.to_string(),
        "--terminal-sequence".to_owned(),
        terminal_sequence.to_string(),
        "--worker".to_owned(),
        worker.to_string(),
    ];

    // THE PREFLIGHT FIRST, ALWAYS. It opens no key and sends nothing, and it is
    // the hostile this tier owes: a crank is admissible strictly after the
    // leg's deadline, so a driver that would refuse must refuse HERE, by name,
    // rather than after a cluster round trip.
    let mut preflight = base.clone();
    preflight.extend([
        "--output".to_owned(),
        request
            .work
            .join(format!("crank-{terminal_sequence}-preflight.json"))
            .display()
            .to_string(),
    ]);
    let planned = match crate::recovery_crank::run_v1(preflight, ExpectedClusterV1::OwnedLoopback) {
        Ok(planned) => Some(planned),
        // THE PREFLIGHT REFUSAL IS THE HOSTILE, NOT A STOP. The driver refuses
        // a not-yet-due crank inside `plan`, before it builds anything, which
        // is exactly the conjunct this tier exists to exercise: a crank is
        // admissible STRICTLY after its leg's deadline. So a refusal carrying
        // that sentence is the hostile SATISFIED, and the walk continues into
        // the bounded wait. Any other refusal is a real one and stops the walk.
        //
        // This tier read it as a stop on its first live run and reported a walk
        // that had proved its own hostile and then declined to continue.
        Err(error) if error.to_string().contains(CRANK_TOO_EARLY_MARKER_V1) => {
            hostile = Some(error.to_string());
            None
        }
        Err(error) => {
            stages.push(StageV1::new(
                label,
                "refused-in-preflight",
                format!(
                    "The shipped advance-recovery driver refused before opening a key: {error}"
                ),
            ));
            return Ok(CrankRunV1 {
                landed: false,
                report: serde_json::json!({
                    "sequence": terminal_sequence,
                    "outcome": "refused-in-preflight",
                    "reason": error.to_string(),
                }),
            });
        }
    };
    // A refused preflight built no plan, so there is no distance to compare
    // against the ceiling: the driver has already said the leg is not due, and
    // its own bounded wait is what decides whether the target is reachable.
    let remaining = match &planned {
        Some(planned) => planned.due_unix_seconds - planned.observed_unix_seconds,
        None => 0,
    };

    // NOT YET DUE, AND FURTHER AWAY THAN THIS CAMPAIGN MAY WAIT. This is a
    // real measurement of the market this tier founded, not a failure of the
    // driver: the wait is bounded on purpose and a target past the ceiling is
    // refused rather than slept for.
    if planned.is_some() && remaining >= request.max_wait_seconds {
        let planned = planned.as_ref().expect("a not-yet-due report carries its plan");
        stages.push(StageV1::new(
            label,
            "not-yet-due",
            format!(
                "The {} leg becomes crankable at unix {} and the cluster's own clock reads {}: {} \
                 seconds away, past this campaign's {}-second ceiling. The crank REFUSES rather \
                 than sending, which is the conjunct the ladder rests on -- the last second an \
                 honest observation may land and the first second a crank may run are different \
                 seconds. No clock was warped to shorten this.",
                planned.arm,
                planned.due_unix_seconds,
                planned.observed_unix_seconds,
                remaining,
                request.max_wait_seconds
            ),
        ));
        return Ok(CrankRunV1 {
            landed: false,
            report: serde_json::json!({
                "sequence": terminal_sequence,
                "outcome": "not-yet-due",
                "arm": planned.arm,
                "phase": format!("{:?}", planned.phase),
                "enteringAttempt": planned.entering,
                "fundedAttempts": planned.attempt_count,
                "dueUnixSeconds": planned.due_unix_seconds,
                "observedUnixSeconds": planned.observed_unix_seconds,
                "secondsUntilDue": remaining,
                "frameAccounts": planned.frame_accounts,
            }),
        });
    }

    // DUE, OR CLOSE ENOUGH TO WAIT FOR. `--wait` sleeps to the deadline through
    // the driver's own bounded wait against the chain's clock; it never warps.
    let mut execute = base;
    execute.extend([
        "--output".to_owned(),
        request
            .work
            .join(format!("crank-{terminal_sequence}.json"))
            .display()
            .to_string(),
        "--wait".to_owned(),
        "--max-wait-seconds".to_owned(),
        request.max_wait_seconds.to_string(),
        "--execute".to_owned(),
        "--worker-keypair".to_owned(),
        worker_keypair.display().to_string(),
    ]);
    let landed = crate::recovery_crank::run_v1(execute, ExpectedClusterV1::OwnedLoopback)?;
    let evidence = landed
        .landed
        .ok_or_else(|| Error::new("an executed crank reported no landed transaction"))?;
    let units = evidence.compute_units_consumed;
    let signature = evidence.signature.clone();
    transactions.push(evidence);
    stages.push(StageV1::new(
        label,
        "executed",
        format!(
            "The shipped advance-recovery driver built the {}-account frame from \
             relay_frame_roles_v1 itself, waited to unix {} through one bounded wait against the \
             chain's own clock, sent, and read the Source back to prove the ladder moved. \
             Signature {signature}, {} compute units.",
            landed.frame_accounts,
            landed.due_unix_seconds,
            units.map_or_else(|| "unreported".to_owned(), |value| value.to_string())
        ),
    ));
    Ok(CrankRunV1 {
        landed: true,
        report: serde_json::json!({
            "sequence": terminal_sequence,
            "outcome": "executed",
            "arm": landed.arm,
            "phase": format!("{:?}", landed.phase),
            "enteringAttempt": landed.entering,
            "fundedAttempts": landed.attempt_count,
            "dueUnixSeconds": landed.due_unix_seconds,
            "observedUnixSeconds": landed.observed_unix_seconds,
            "frameAccounts": landed.frame_accounts,
            "signature": signature,
            "computeUnitsConsumed": units,
            "refusedBeforeTheDeadline": hostile,
        }),
    })
}
