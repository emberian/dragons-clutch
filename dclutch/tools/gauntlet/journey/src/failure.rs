//! The failure walk: the window closes unobserved, the ladder exhausts, and
//! the failure selector is committed -- three shipped drivers called in this
//! process with the argument vectors a host would type.
//!
//! # What this walk is
//!
//! The honest walk (`provider.rs`) answers the market through the real Pyth
//! receiver. This one does not answer it at all. The market's primary window
//! closes with nobody reporting, a stranger cranks the funded ladder onto the
//! one alternative it paid for, that rung's own committed deadline closes with
//! nobody reporting either, the same crank exhausts the ladder, and the
//! stranger then walks the market to the Product's own pre-disclosed failure
//! selector and is paid the bounty the founding disclosed for doing so.
//! Everything after that -- Core admitting the failure certificate, every
//! ordinary holder refunded pro rata from the Hoard, the founder drawing only
//! their holdings, the escrow's column burned at closure -- is the honest
//! walk's own stages reading a different certificate kind, which is the whole
//! point: the refund is not a second protocol.
//!
//! # No second author
//!
//! `crate::recovery_crank::run_v1` builds the crank's 18-account frame and
//! `crate::deadline_failure::run_v1` the walk's 22, each from the relay
//! contract's own roles through `dclutch_operator`. A tier that rebuilt either
//! frame would be measuring a second author, not the driver a host runs. That
//! is `tools/gauntlet/ladder/`'s rule and it is this file's.
//!
//! # The clock is not warped, and the market is founded so it need not be
//!
//! A crank and a walk are admissible STRICTLY after their leg's deadline, read
//! against the cluster's own clock. On the local Pyth fixture the primary
//! leg's deadline is the captured publication instant plus `max_age`, and the
//! fixture's declared shelf life is one year -- so the honest lab default can
//! never reach a deadline inside a bounded run. The failure walk founds its
//! market with `terminal_max_age_seconds` short enough that the window has
//! ALREADY closed by the time the market is Open: a frozen publication's
//! staleness is a fact about the LAB, never about a market, and stating a
//! shorter shelf life is the honest alternative to moving a validator's clock.
//! What that shape cannot do is answer a rung -- one `max_age` governs both the
//! crank's admissibility and the publication's freshness -- which is exactly
//! why it is the FAILURE walk's shape and not the honest one's.

use std::path::Path;

use serde_json::{Value, json};
use solana_sdk::pubkey::Pubkey;

use crate::cluster::ExpectedClusterV1;
use crate::ledger::LamportClaimV1;
use crate::model::TransactionEvidence;
use crate::stages::StageReportV1;
use crate::{Error, Result};

/// The stage label, in one place: the transcript, the ledger boundary and a
/// witness read it.
pub(crate) const FAILURE_WALK_STAGE_V1: &str = "resolution: the window closes unobserved, the ladder exhausts, and the failure selector is committed";

/// The one rung the failure walk's market buys, in the SHIPPED flag's spelling.
///
/// The same rung the honest walk buys, so the two walks differ in exactly the
/// thing under test -- whether anybody answers -- and in nothing else.
pub(crate) const DEFAULT_FAILURE_RECOVERY_RUNGS_V1: &str = "2500:120";

/// The shelf life the failure walk's market declares for the captured
/// publication, in seconds.
///
/// Short on purpose: the primary leg's deadline is `window.end + max_age` and
/// `window.end` IS the captured publication instant, so a shelf life this
/// short puts the deadline in the past before the market is Open. That is the
/// lab stating a fact about its own fixture, not a knob on a market.
pub(crate) const FAILURE_WALK_MAX_AGE_SECONDS_V1: u32 = 120;

/// The whole budget one leg may spend waiting for its deadline.
pub(crate) const FAILURE_WALK_MAX_WAIT_SECONDS_V1: i64 = 600;

/// The certificate sequences the walk's three receipts occupy.
///
/// The same numbering the real-ELF walk in
/// `crates/dclutch-svm-harness/tests/resolution_core_v3_lifecycle.rs` and the
/// ladder tier use, so a reader comparing the loopback figures against the
/// program-test figures is comparing the same seats. The terminal itself is
/// sequence one, because the terminal admission that follows it and every
/// terminal payout after that read sequence one.
const FAILURE_SEQUENCE_V1: u64 = 1;
const ADVANCE_SEQUENCE_V1: u64 = 2;
const EXHAUST_SEQUENCE_V1: u64 = 3;

/// Everything the three drivers are given.
pub(crate) struct FailureWalkContextV1<'a> {
    pub(crate) rpc_url: &'a str,
    pub(crate) plan: &'a Path,
    pub(crate) campaign_report: &'a Path,
    pub(crate) market: Pubkey,
    pub(crate) work: &'a Path,
    /// The stranger who cranks and walks, and is PAID for both.
    pub(crate) worker: Pubkey,
    pub(crate) worker_keypair: &'a Path,
}

/// One leg's machine-readable row.
fn leg_report(label: &str, outcome: &str, note: Value) -> Value {
    json!({ "leg": label, "outcome": outcome, "detail": note })
}

fn base_arguments(context: &FailureWalkContextV1<'_>, sequence: u64) -> Vec<String> {
    vec![
        "--rpc-url".to_owned(),
        context.rpc_url.to_owned(),
        "--plan".to_owned(),
        context.plan.display().to_string(),
        "--evidence".to_owned(),
        context.campaign_report.display().to_string(),
        "--market".to_owned(),
        context.market.to_string(),
        "--terminal-sequence".to_owned(),
        sequence.to_string(),
        "--worker".to_owned(),
        context.worker.to_string(),
    ]
}

fn executing_arguments(
    context: &FailureWalkContextV1<'_>,
    sequence: u64,
    output: &str,
) -> Vec<String> {
    let mut arguments = base_arguments(context, sequence);
    arguments.extend([
        "--output".to_owned(),
        context.work.join(output).display().to_string(),
        "--wait".to_owned(),
        "--max-wait-seconds".to_owned(),
        FAILURE_WALK_MAX_WAIT_SECONDS_V1.to_string(),
        "--execute".to_owned(),
        "--worker-keypair".to_owned(),
        context.worker_keypair.display().to_string(),
    ]);
    arguments
}

/// One crank, through the shipped driver, as a leg of this walk.
///
/// A refusal carrying the driver's not-yet-due sentence is the HOSTILE
/// satisfied -- the conjunct the whole ladder rests on -- and is recorded as
/// such; on the failure walk's shape it should never fire, because the market
/// was founded with its deadlines already behind it, so a not-yet-due here is
/// a finding about the shape and stops the walk with that sentence.
fn crank(
    context: &FailureWalkContextV1<'_>,
    sequence: u64,
    label: &str,
    expected_arm: &str,
    transactions: &mut Vec<TransactionEvidence>,
    legs: &mut Vec<Value>,
) -> Result<u64> {
    let arguments = executing_arguments(context, sequence, &format!("crank-{sequence}.json"));
    let landed = crate::recovery_crank::run_v1(arguments, ExpectedClusterV1::OwnedLoopback)
        .map_err(|error| {
            let sentence = error.to_string();
            legs.push(leg_report(label, "refused", json!({ "reason": sentence })));
            if sentence.contains(crate::recovery_crank::TOO_EARLY_MARKER_V1)
                || sentence.contains(crate::recovery_crank::WAIT_CEILING_MARKER_V1)
            {
                Error::new(format!(
                    "{label}: the shipped crank driver reports the leg is not yet due -- {sentence}. \
                     On the failure walk the market is founded with terminal_max_age_seconds \
                     {FAILURE_WALK_MAX_AGE_SECONDS_V1}, which is supposed to put every deadline \
                     behind the founding; a leg still ahead of the chain clock means the shape \
                     did not reach the compiler"
                ))
            } else {
                Error::new(format!("{label}: the shipped crank driver refused: {sentence}"))
            }
        })?;
    if landed.arm != expected_arm {
        return Err(Error::new(format!(
            "{label}: the crank planned the {} arm and this walk expected {expected_arm}; the \
             Source stood on {:?} attempt {} entering {} of {} funded",
            landed.arm, landed.phase, landed.entering, landed.entering, landed.attempt_count
        )));
    }
    let evidence = landed.landed.ok_or_else(|| {
        Error::new(format!(
            "{label}: an executed crank reported no landed transaction"
        ))
    })?;
    let units = evidence.compute_units_consumed.unwrap_or(0);
    legs.push(leg_report(
        label,
        "executed",
        json!({
            "arm": landed.arm,
            "phaseBefore": format!("{:?}", landed.phase),
            "enteringAttempt": landed.entering,
            "fundedAttempts": landed.attempt_count,
            "dueUnixSeconds": landed.due_unix_seconds,
            "observedUnixSeconds": landed.observed_unix_seconds,
            "certificate": landed.certificate.to_string(),
            "frameAccounts": landed.frame_accounts,
            "signature": evidence.signature,
            "computeUnitsConsumed": evidence.compute_units_consumed,
        }),
    ));
    transactions.push(evidence);
    Ok(units)
}

/// What the walk left behind, for the stages after it.
pub(crate) struct FailureWalkOutcomeV1 {
    /// The `ResolutionFailure` seat the walk minted, which the terminal
    /// admission consumes and every terminal payout then reads.
    pub(crate) certificate: Pubkey,
    /// The Product's failure cell, read off the finalized result domain by the
    /// driver and never chosen here.
    pub(crate) failure_selector: u32,
    pub(crate) outcome_count: u32,
    /// The bounty the walk paid the stranger, read off the certificate.
    pub(crate) work_paid: u64,
}

/// Walk one Market from Open, unanswered, to `FailureCommitted`.
///
/// Three legs, each the shipped driver: advance (the primary window closed
/// unobserved), exhaust (the rung's own deadline closed unobserved), commit
/// (the failure selector). The report counts every transaction the three
/// landed and the compute units they consumed, and the lamport claim is
/// inapplicable by name -- the drivers pay the worker a bounty out of the
/// market's own Resolution funding ledger and write their own receipts, and L7
/// does not restate another author's arithmetic.
pub(crate) fn walk_to_failure(
    context: &FailureWalkContextV1<'_>,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<(StageReportV1, LamportClaimV1, Value, FailureWalkOutcomeV1)> {
    let mut legs: Vec<Value> = Vec::new();
    let before = transactions.len();
    let mut compute_units = 0_u64;

    // THE PRIMARY WINDOW CLOSED UNOBSERVED. A stranger advances the ladder onto
    // the one alternative the founding paid for.
    compute_units = compute_units.saturating_add(crank(
        context,
        ADVANCE_SEQUENCE_V1,
        "advance onto the funded alternative",
        "advance",
        transactions,
        &mut legs,
    )?);
    // THE RUNG'S OWN DEADLINE CLOSED UNOBSERVED TOO. The same transition ends
    // the ladder: there is no attempt after this one.
    compute_units = compute_units.saturating_add(crank(
        context,
        EXHAUST_SEQUENCE_V1,
        "exhaust the last funded rung",
        "exhaust",
        transactions,
        &mut legs,
    )?);

    // THE FAILURE SELECTOR. From Exhausted, the deadline walk commits the
    // Product's own failure cell and is paid the bounty the market disclosed.
    let walked = crate::deadline_failure::run_v1(
        executing_arguments(context, FAILURE_SEQUENCE_V1, "deadline-failure.json"),
        ExpectedClusterV1::OwnedLoopback,
    )
    .map_err(|error| {
        let sentence = error.to_string();
        legs.push(leg_report(
            "commit the failure selector",
            "refused",
            json!({ "reason": sentence }),
        ));
        Error::new(format!(
            "commit the failure selector: the shipped deadline-failure driver refused: {sentence}"
        ))
    })?;
    if walked.arm != "ladder-exhausted" {
        return Err(Error::new(format!(
            "commit the failure selector: the driver took the {} arm on a market that bought a \
             ladder; the exhausted arm is the one this walk exists to drive",
            walked.arm
        )));
    }
    let evidence = walked.landed.ok_or_else(|| {
        Error::new("commit the failure selector: an executed walk reported no landed transaction")
    })?;
    let work_paid = walked
        .work_paid
        .ok_or_else(|| Error::new("commit the failure selector: the walk reported no bounty"))?;
    compute_units = compute_units.saturating_add(evidence.compute_units_consumed.unwrap_or(0));
    legs.push(leg_report(
        "commit the failure selector",
        "executed",
        json!({
            "arm": walked.arm,
            "phaseBefore": format!("{:?}", walked.phase_before),
            "failureSelector": walked.failure_selector,
            "outcomeCount": walked.outcome_count,
            "dueUnixSeconds": walked.due_unix_seconds,
            "observedUnixSeconds": walked.observed_unix_seconds,
            "certificate": walked.certificate.to_string(),
            "frameAccounts": walked.frame_accounts,
            "workPaid": work_paid,
            "signature": evidence.signature,
            "computeUnitsConsumed": evidence.compute_units_consumed,
        }),
    ));
    transactions.push(evidence);

    let submitted = transactions.len().saturating_sub(before);
    let report = StageReportV1 {
        stage: FAILURE_WALK_STAGE_V1.into(),
        outcome: "executed".into(),
        transactions: submitted,
        compute_units,
        note: format!(
            "NOBODY ANSWERED, AND THE MARKET STILL ENDED. Three shipped drivers in this process: \
             `local-private-validator-advance-recovery-v1` twice -- the primary window closed \
             unobserved and a stranger advanced the ladder onto the funded alternative (18 \
             accounts, the relay contract's own frame), then the rung's committed deadline \
             closed unobserved and the same transition exhausted it -- and \
             `local-private-validator-commit-deadline-failure-v1` once, from Exhausted, \
             committing the Product's own failure cell {} of {} through the 22-account frame and \
             paying the stranger {work_paid} lamports out of the market's own prepaid \
             compartment. Every deadline was read off the market's published WindowSpecV1 and \
             RecoveryPolicyV2 against the cluster's own clock; no clock was warped, the market \
             was founded with its shelf life already spent. The certificate at {} is a \
             ResolutionFailure with no route and no provider evidence, which is the whole \
             content of the claim that nobody answered.",
            walked.failure_selector, walked.outcome_count, walked.certificate
        ),
    };
    let claim = LamportClaimV1::inapplicable(
        "the crank and walk drivers pay the worker a bounty out of the market's own Resolution \
         funding ledger and write their own receipts; L7 does not restate another author's \
         arithmetic",
    );
    let document = json!({
        "schema": "dclutch-journey-failure-walk-v1",
        "market": context.market.to_string(),
        "worker": context.worker.to_string(),
        "legs": legs,
        "certificate": walked.certificate.to_string(),
        "failureSelector": walked.failure_selector,
        "outcomeCount": walked.outcome_count,
        "workPaid": work_paid,
    });
    Ok((
        report,
        claim,
        document,
        FailureWalkOutcomeV1 {
            certificate: walked.certificate,
            failure_selector: walked.failure_selector,
            outcome_count: walked.outcome_count,
            work_paid,
        },
    ))
}
