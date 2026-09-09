//! The exhausted terminal as a driven act: one funded deadline walk.
//!
//! # What this drives, and what already existed
//!
//! `RelayActionV1::CommitDeadlineFailure` has been a program route since the
//! relayed vertical (`programs/dclutch-resolution-proof-sbf/src/relay_transport_v1.rs`,
//! `process_commit_deadline_failure`), and `plan_deadline_failure_v1` has
//! always had two ways in and one way out: a market with no ladder arrives on
//! `Primary` and the walk spends its own deadline to reach `Exhausted`; a
//! market whose funded ladder walked every leg it sold arrives already
//! `Exhausted`. What no host ever drove is the second arm on a chain -- the
//! ladder's `Exhausted` receipt was the last thing RECOVERY-4 landed, and the
//! terminal past it was scoped out. This driver is that act, and it drives
//! both arms through one builder so the honest no-ladder walk and the
//! exhausted-ladder walk are one command with one evidence shape.
//!
//! # Why this is a frame builder and a bounded wait rather than a decision
//!
//! The instruction names only the Market generation and the terminal sequence.
//! Which arm the walk takes is the Source's own phase; the outcome it commits
//! is the Product's own failure selector out of the finalized result domain;
//! the bounty it pays is the capability manifest's quote. So there is nothing
//! here for a driver to decide, and the one thing it computes -- whether the
//! walk is admissible yet -- is `dclutch_operator::deadline_failure_v1`'s,
//! reproduced off chain so a walk that would refuse `Transition` refuses here
//! by name, before a lamport moves. `--wait` turns that refusal into one
//! bounded sleep against the chain's own clock; it never warps.
//!
//! # What it predicts, and what happens when it predicts wrong
//!
//! The seat is the `ResolutionFailure` kind at the terminal sequence. That is
//! not a prediction: the walk writes no other kind. What the driver does
//! predict is the SELECTOR -- the Product's failure cell -- and after the send
//! it reads the certificate back and refuses its own success unless the kind,
//! the selector and a nonzero `work_paid` are all what the chain wrote.

use std::path::{Path, PathBuf};

use serde_json::json;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};
use solana_sdk_ids::sysvar;
use solana_system_interface::instruction::transfer;

use dclutch_market::capability_manifest::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1;
use dclutch_operator::deadline_failure_v1::{
    DeadlineFailureArmV1, DeadlineFailureCoordinatesV1, DeadlineFailureDecisionV1, FinalizedPairV1,
    build_commit_deadline_failure_v1, deadline_failure_decision_v1,
};
use dclutch_product::ResultDomainV2;
use dclutch_product::admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_source::resolution::{
    RESOLUTION_CERTIFICATE_BYTES_V2, ResolutionCertificateKindV2, ResolutionCertificateV2,
};
use dclutch_source::{
    SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3, SOURCE_RESOLUTION_STATE_BYTES_V2,
    SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2, SourceMaterialV3, SourceResolutionPhaseV1,
    SourceResolutionStateV2, WINDOW_SPEC_SCHEMA_ID_V1, WindowSpecV1,
};

use crate::campaign::{
    parse_campaign_terminal_evidence_with_expected_cluster_v1, read_keypair_file,
};
use crate::cluster::{ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG, ExpectedClusterV1};
use crate::model::{SuccessorPlan, TransactionEvidence};
use crate::plan::pubkey;
use crate::rpc::{Rpc, WritePolicyV1};
use crate::sponsored_schedule::wait_until_unix_seconds_v1;
use crate::terminal_lifecycle::routed_record;
use crate::wallet_terminal::RecordPairV1;
use crate::{Error, Result};

/// The owned-loopback command name.
pub(crate) const COMMAND_V1: &str = "local-private-validator-commit-deadline-failure-v1";

/// The public arm.
///
/// Nothing about the instruction differs between the two clusters -- the walk
/// is permissionless on both and reads the same state. The cluster decides
/// which origin is admitted, which campaign report is consumable, and which
/// label the evidence carries.
pub(crate) const COMMAND_DEVNET_V1: &str = "devnet-commit-deadline-failure-v1";

const fn command(expected: ExpectedClusterV1) -> &'static str {
    match expected {
        ExpectedClusterV1::Devnet => COMMAND_DEVNET_V1,
        ExpectedClusterV1::OwnedLoopback => COMMAND_V1,
    }
}

/// The label the transaction carries. A census binding may not have two
/// owners, and the two arms are two facts about a market -- different
/// prestates, one certificate kind -- so the label names the arm.
fn label_for(arm: DeadlineFailureArmV1) -> String {
    format!("Deadline failure walk: {}", arm.label())
}

pub(crate) fn devnet_usage() -> &'static str {
    "dclutch-local-successor-bootstrap devnet-commit-deadline-failure-v1 --rpc-url URL --i-mean-devnet DEVNET_GENESIS --plan ABSOLUTE_JSON --evidence ABSOLUTE_JSON --market PUBKEY --terminal-sequence U64 --worker PUBKEY --output ABSOLUTE_JSON [--wait --max-wait-seconds I64] [--execute --worker-keypair ABSOLUTE_JSON]\n\
     \nThe public arm of the same permissionless deadline walk. It consumes an executed devnet campaign report, refuses every non-devnet origin, and writes its evidence under the devnet label."
}

pub(crate) fn usage() -> &'static str {
    "dclutch-local-successor-bootstrap local-private-validator-commit-deadline-failure-v1 --rpc-url http://127.0.0.1:PORT --plan ABSOLUTE_JSON --evidence ABSOLUTE_JSON --market PUBKEY --terminal-sequence U64 --worker PUBKEY --output ABSOLUTE_JSON [--wait --max-wait-seconds I64] [--execute --worker-keypair ABSOLUTE_JSON]\n\
     \nOne funded deadline walk: the market's window closed unobserved and a stranger commits the Product's own failure selector, paid the bounty the market disclosed at founding. Which arm -- a no-ladder market spending its primary deadline, or a walked ladder standing Exhausted -- is read off the Source's own phase, and the selector off the finalized result domain, so nothing economic is passed in: the driver assembles the 22-account frame the relay contract declares, derives the ResolutionFailure seat, and refuses by name while the primary deadline has not passed or while a funded rung still stands. --wait sleeps to the deadline through one bounded wait against the chain's own clock and refuses a target further away than --max-wait-seconds; it never warps. Preflight opens no key. Execute preserves the single-source same-transaction prepay; an Ensemble spends only its pre-liability Source reserve. The driver then reads the Source and certificate back to prove the failure selector was committed."
}

/// Parsed command line.
#[derive(Debug)]
struct ArgumentsV1 {
    origin: ClusterOriginV1,
    plan: PathBuf,
    evidence: PathBuf,
    market: Pubkey,
    terminal_sequence: u64,
    /// The worker's ADDRESS, required even on a preflight: position zero of
    /// the frame and the account the walk pays.
    worker: Pubkey,
    worker_keypair: Option<PathBuf>,
    output: PathBuf,
    wait: bool,
    max_wait_seconds: Option<i64>,
    execute: bool,
}

/// Everything one walk is, before any key exists.
struct PlanV1 {
    instruction: solana_sdk::instruction::Instruction,
    market: Pubkey,
    source_state: Pubkey,
    certificate: Pubkey,
    funding_ledger: Pubkey,
    generation: u64,
    terminal_sequence: u64,
    phase: SourceResolutionPhaseV1,
    active_attempt: u8,
    bought_ladder: bool,
    decision: DeadlineFailureDecisionV1,
    observed_unix_seconds: i64,
    seat_shortfall_lamports: u64,
    source_reserve_funding: bool,
    source_reserve_available_lamports: u64,
    frame_accounts: usize,
}

pub(crate) fn run_owned_loopback_v1(arguments: Vec<String>) -> Result<()> {
    run_v1(arguments, ExpectedClusterV1::OwnedLoopback).map(|_| ())
}

pub(crate) fn run_devnet_v1(arguments: Vec<String>) -> Result<()> {
    run_v1(arguments, ExpectedClusterV1::Devnet).map(|_| ())
}

/// What one walk was and, when it executed, what it landed.
///
/// A gauntlet tier has to FOLD what this driver did, and a tier that rebuilt
/// the frame, the seat or the arm from its own arithmetic would be measuring a
/// second author instead of the driver a host runs.
pub(crate) struct DeadlineFailureOutcomeV1 {
    /// `primary-deadline` or `ladder-exhausted`, the driver's own word for
    /// the arm the Source selected.
    pub(crate) arm: &'static str,
    /// The leg the market stood on when this walk was planned.
    pub(crate) phase_before: SourceResolutionPhaseV1,
    /// The selector the walk commits: the Product's failure cell.
    pub(crate) failure_selector: u32,
    /// The Product's outcome count.
    pub(crate) outcome_count: u32,
    /// The last second an honest observation may land on the primary arm;
    /// `None` on the exhausted arm.
    pub(crate) due_unix_seconds: Option<i64>,
    /// What the chain's own clock said when the plan was built.
    pub(crate) observed_unix_seconds: i64,
    pub(crate) source_state: Pubkey,
    pub(crate) certificate: Pubkey,
    pub(crate) frame_accounts: usize,
    /// The bounty the certificate records, once landed.
    pub(crate) work_paid: Option<u64>,
    /// `None` on a preflight: no key was opened and nothing was sent.
    pub(crate) landed: Option<TransactionEvidence>,
}

/// One walk, as a value rather than as a process exit code.
pub(crate) fn run_v1(
    arguments: Vec<String>,
    expected: ExpectedClusterV1,
) -> Result<DeadlineFailureOutcomeV1> {
    let arguments = parse(arguments, expected)?;
    expected.authenticate(&arguments.origin)?;
    let mut rpc = Rpc::connect_cluster(
        &arguments.origin,
        if arguments.execute {
            WritePolicyV1::Writes
        } else {
            WritePolicyV1::ReadsOnly
        },
    )?;
    let plan = plan(&mut rpc, &arguments, expected)?;
    report(&plan);

    if !arguments.execute {
        write_evidence(&arguments.output, &plan, expected, None, None)?;
        println!("preflight only; no key was opened and nothing was sent");
        return Ok(outcome(&plan, None, None));
    }

    let path = arguments
        .worker_keypair
        .as_deref()
        .ok_or_else(|| Error::new("--execute requires --worker-keypair"))?;
    let worker = Keypair::new_from_array(read_keypair_file(path, "deadline walk worker")?);
    if worker.pubkey() != arguments.worker {
        return Err(Error::new(format!(
            "the plan was built for worker {} and --worker-keypair holds {}; the worker is \
             position zero of the frame and is the account this walk pays, so it cannot be \
             substituted after planning",
            arguments.worker,
            worker.pubkey()
        )));
    }

    // Single-source markets retain their existing same-transaction prepay.
    // An Ensemble has already capitalized this physical output on its Source
    // before Core accepted liabilities, so a late worker transfer would hide
    // an underfunded founding and is deliberately absent.
    let mut instructions = Vec::with_capacity(2);
    if !plan.source_reserve_funding && plan.seat_shortfall_lamports != 0 {
        instructions.push(transfer(
            &worker.pubkey(),
            &plan.certificate,
            plan.seat_shortfall_lamports,
        ));
    }
    instructions.push(plan.instruction.clone());

    let evidence = rpc.send(&label_for(plan.decision.arm), &instructions, &worker)?;
    if let Some(error) = evidence.error.as_ref() {
        return Err(Error::new(format!(
            "the deadline walk refused on chain: {error}"
        )));
    }
    println!("signature            {}", evidence.signature);
    println!("slot                 {}", evidence.slot);
    println!(
        "compute units        {}",
        evidence
            .compute_units_consumed
            .map_or_else(|| "unreported".to_string(), |units| units.to_string())
    );

    // The chain is asked whether the failure selector was committed, rather
    // than the send being taken as proof that it was.
    let after = rpc.required_account(plan.source_state, "walked Source resolution state")?;
    let state = SourceResolutionStateV2::decode(&after.data)
        .map_err(|error| Error::new(format!("walked Source state: {error:?}")))?;
    if state.phase() != SourceResolutionPhaseV1::FailureCommitted {
        return Err(Error::new(format!(
            "the walk landed and the Source reads phase {:?}, not the FailureCommitted this \
             walk was planned to produce",
            state.phase()
        )));
    }
    let seat = rpc.required_account(plan.certificate, "ResolutionFailure certificate")?;
    let certificate = ResolutionCertificateV2::decode(&seat.data)
        .map_err(|error| Error::new(format!("ResolutionFailure certificate: {error:?}")))?;
    if certificate.kind != ResolutionCertificateKindV2::ResolutionFailure
        || certificate.selector != plan.decision.failure_selector
        || certificate.work_paid == 0
    {
        return Err(Error::new(format!(
            "the certificate at {} reads kind {:?}, selector {} and work_paid {}; this walk \
             was planned to write ResolutionFailure at the Product's failure cell {} with a \
             nonzero bounty",
            plan.certificate,
            certificate.kind,
            certificate.selector,
            certificate.work_paid,
            plan.decision.failure_selector
        )));
    }
    println!(
        "source after         phase {:?}; certificate kind {:?}, selector {} of {}, work paid {} \
         (read back from chain)",
        state.phase(),
        certificate.kind,
        certificate.selector,
        plan.decision.outcome_count,
        certificate.work_paid
    );

    write_evidence(
        &arguments.output,
        &plan,
        expected,
        Some(&evidence),
        Some(certificate.work_paid),
    )?;
    Ok(outcome(&plan, Some(evidence), Some(certificate.work_paid)))
}

fn outcome(
    plan: &PlanV1,
    landed: Option<TransactionEvidence>,
    work_paid: Option<u64>,
) -> DeadlineFailureOutcomeV1 {
    DeadlineFailureOutcomeV1 {
        arm: plan.decision.arm.label(),
        phase_before: plan.phase,
        failure_selector: plan.decision.failure_selector,
        outcome_count: plan.decision.outcome_count,
        due_unix_seconds: plan.decision.due_unix_seconds,
        observed_unix_seconds: plan.observed_unix_seconds,
        source_state: plan.source_state,
        certificate: plan.certificate,
        frame_accounts: plan.frame_accounts,
        work_paid,
        landed,
    }
}

fn finalized_pair(pair: RecordPairV1) -> FinalizedPairV1 {
    FinalizedPairV1 {
        raw: pair.raw,
        staging: pair.staging,
    }
}

/// Build the one instruction this walk will send.
fn plan(rpc: &mut Rpc, arguments: &ArgumentsV1, expected: ExpectedClusterV1) -> Result<PlanV1> {
    let plan_bytes = std::fs::read(&arguments.plan)?;
    let plan: SuccessorPlan = serde_json::from_slice(&plan_bytes)?;
    let evidence_bytes = std::fs::read(&arguments.evidence)?;
    let evidence =
        parse_campaign_terminal_evidence_with_expected_cluster_v1(&evidence_bytes, expected)?;

    let registry = pubkey(&plan.registry.program_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let resolution = pubkey(&plan.resolution.program_id)?;
    let activation = pubkey(&plan.activation)?;
    let market = arguments.market;

    // Every record pair is re-derived from its own schema and body digest by
    // `routed_record`, which refuses a persisted address that is not the
    // canonical one. A campaign report cannot therefore seat a junk coordinate
    // in this frame by naming one.
    let material = routed_record(
        &evidence,
        "source_material_record",
        registry,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
    )?;
    let window = routed_record(
        &evidence,
        "window_spec_record",
        registry,
        WINDOW_SPEC_SCHEMA_ID_V1,
    )?;
    let manifest = routed_record(
        &evidence,
        "capability_manifest_record",
        registry,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    )?;
    let product = routed_record(
        &evidence,
        "product_record",
        registry,
        PRODUCT_RECORD_SCHEMA_ID_V2,
    )?;
    let result_domain = routed_record(
        &evidence,
        "result_domain_record",
        registry,
        RESULT_DOMAIN_SCHEMA_ID_V2,
    )?;
    let portfolio = routed_record(
        &evidence,
        "portfolio_record",
        registry,
        PORTFOLIO_SCHEMA_ID_V2,
    )?;
    // ABSENCE IS THE ARM, NOT A GAP. A founding whose material bought no
    // ladder publishes no policy record and its evidence says so by leaving
    // the row out; the walk then spends the primary deadline itself. A
    // founding that bought one has to be cranked to Exhausted first, and the
    // decision below says so by name.
    let bought_ladder = evidence.accounts.contains_key("recovery_policy_record");
    let funding_ledger = pubkey(
        &evidence
            .accounts
            .get("resolution_funding_ledger")
            .ok_or_else(|| {
                Error::new("the campaign report names no resolution_funding_ledger to spend")
            })?
            .address,
    )?;

    let market_account = rpc.required_account(market, "Core Market")?;
    if market_account.owner != core {
        return Err(Error::new(format!(
            "the account at {market} is owned by {}, not the plan's Core program {core}",
            market_account.owner
        )));
    }
    let state = dclutch_market::CoreState::decode(&market_account.data)
        .map_err(|error| Error::new(format!("Core Market: {error:?}")))?;
    let generation = state.identity.generation;

    let source_state = Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
            market.as_ref(),
            &generation.to_le_bytes(),
        ],
        &resolution,
    )
    .0;
    let source_account = rpc.required_account(source_state, "Source resolution state")?;
    let source = SourceResolutionStateV2::decode(&source_account.data)
        .map_err(|error| Error::new(format!("Source resolution state: {error:?}")))?;
    let material_account = rpc.required_account(material.raw, "SourceMaterialV3 record")?;
    let source_material = SourceMaterialV3::decode(&material_account.data)
        .map_err(|error| Error::new(format!("SourceMaterialV3 record: {error:?}")))?;
    if source.material_id().to_bytes() != material.digest {
        return Err(Error::new(
            "Source state does not name the finalized SourceMaterialV3 record",
        ));
    }

    let window_account = rpc.required_account(window.raw, "WindowSpecV1 record")?;
    let window_spec = WindowSpecV1::decode(&window_account.data)
        .map_err(|error| Error::new(format!("WindowSpecV1 record: {error:?}")))?;
    let domain_account = rpc.required_account(result_domain.raw, "result domain record")?;
    let domain = ResultDomainV2::decode(&domain_account.data)
        .map_err(|error| Error::new(format!("result domain record: {error:?}")))?;

    let decision = deadline_failure_decision_v1(
        source.phase(),
        source.active_attempt(),
        bought_ladder,
        window_spec,
        &domain,
    )
    .map_err(|error| Error::new(format!("market {market}: {error}")))?;

    // THE WAIT, or the refusal. On the primary arm a walk is admissible
    // strictly after the leg's deadline, so the target is one second past it;
    // on the exhausted arm there is nothing to wait for.
    let slot = rpc.finalized_slot()?;
    let mut observed = rpc.block_time(slot)?;
    if arguments.wait && decision.admissible_at(observed).is_err() {
        let ceiling = arguments.max_wait_seconds.ok_or_else(|| {
            Error::new("--wait requires --max-wait-seconds: a wait with no ceiling cannot say what it will cost")
        })?;
        let target = decision
            .first_admissible_second()
            .map_err(|error| Error::new(error.to_string()))?
            .ok_or_else(|| Error::new("an inadmissible walk with no deadline to wait for"))?;
        observed = wait_until_unix_seconds_v1(rpc, target, ceiling)?;
    }
    decision.admissible_at(observed).map_err(|error| {
        Error::new(format!(
            "market {market}: {error}. Pass --wait --max-wait-seconds to sleep to it"
        ))
    })?;

    let coordinates = DeadlineFailureCoordinatesV1 {
        worker: arguments.worker,
        market,
        core_program: core,
        activation,
        resolution_program: resolution,
        source_state,
        material: finalized_pair(material),
        window: finalized_pair(window),
        product: finalized_pair(product),
        result_domain: finalized_pair(result_domain),
        portfolio: finalized_pair(portfolio),
        manifest: finalized_pair(manifest),
        funding_ledger,
    };
    let built =
        build_commit_deadline_failure_v1(&coordinates, generation, arguments.terminal_sequence)
            .map_err(|error| Error::new(format!("CommitDeadlineFailure frame: {error}")))?;

    let rent_account = rpc.required_account(sysvar::rent::ID, "Rent sysvar")?;
    let rent: solana_sdk::rent::Rent = bincode::deserialize(&rent_account.data)
        .map_err(|error| Error::new(format!("Rent sysvar: {error}")))?;
    let required = rent.minimum_balance(RESOLUTION_CERTIFICATE_BYTES_V2);
    let held = rpc
        .account(built.certificate)?
        .map_or(0, |seat| seat.lamports);
    let seat_shortfall_lamports = required.saturating_sub(held);
    let source_reserve_funding = !source_material.ensemble().is_single();
    let source_reserve_available_lamports = source_account
        .lamports
        .saturating_sub(rent.minimum_balance(SOURCE_RESOLUTION_STATE_BYTES_V2));
    if source_reserve_funding && source_reserve_available_lamports < seat_shortfall_lamports {
        return Err(Error::new(format!(
            "Ensemble Source reserve holds {source_reserve_available_lamports} lamports above its rent floor but the canonical failure certificate needs {seat_shortfall_lamports}; founding did not prepay this liability"
        )));
    }

    Ok(PlanV1 {
        frame_accounts: built.frame_accounts,
        instruction: built.instruction,
        market,
        source_state,
        certificate: built.certificate,
        funding_ledger,
        generation,
        terminal_sequence: arguments.terminal_sequence,
        phase: source.phase(),
        active_attempt: source.active_attempt(),
        bought_ladder,
        decision,
        observed_unix_seconds: observed,
        seat_shortfall_lamports,
        source_reserve_funding,
        source_reserve_available_lamports,
    })
}

fn report(plan: &PlanV1) {
    println!("market               {}", plan.market);
    println!("generation           {}", plan.generation);
    println!("source state         {}", plan.source_state);
    println!("phase                {:?}", plan.phase);
    println!("active attempt       {}", plan.active_attempt);
    println!(
        "ladder               {}",
        if plan.bought_ladder {
            "bought; this walk commits from Exhausted"
        } else {
            "none; this walk spends the primary deadline"
        }
    );
    println!("arm                  {}", plan.decision.arm.label());
    println!(
        "failure selector     {} of {} outcomes (the Product's own failure cell)",
        plan.decision.failure_selector, plan.decision.outcome_count
    );
    println!(
        "due at               {}",
        plan.decision.due_unix_seconds.map_or_else(
            || "now (the last crank spent the final rung)".to_owned(),
            |due| due.to_string()
        )
    );
    println!("chain clock          {}", plan.observed_unix_seconds);
    println!("terminal sequence    {}", plan.terminal_sequence);
    println!("certificate seat     {}", plan.certificate);
    println!("seat shortfall       {}", plan.seat_shortfall_lamports);
    println!(
        "certificate funding  {}",
        if plan.source_reserve_funding {
            "pre-liability Source reserve"
        } else {
            "same-transaction worker prepay"
        }
    );
    println!(
        "Source reserve       {} lamports above rent floor",
        plan.source_reserve_available_lamports
    );
    println!("funding ledger       {}", plan.funding_ledger);
    println!("frame accounts       {}", plan.frame_accounts);
}

fn usage_for(expected: ExpectedClusterV1) -> &'static str {
    match expected {
        ExpectedClusterV1::Devnet => devnet_usage(),
        ExpectedClusterV1::OwnedLoopback => usage(),
    }
}

fn write_evidence(
    path: &Path,
    plan: &PlanV1,
    expected: ExpectedClusterV1,
    landed: Option<&TransactionEvidence>,
    work_paid: Option<u64>,
) -> Result<()> {
    let document = json!({
        "schema": "dclutch-deadline-failure-evidence-v1",
        "cluster": expected.evidence_label(),
        "market": plan.market.to_string(),
        "generation": plan.generation,
        "sourceState": plan.source_state.to_string(),
        "phaseBefore": format!("{:?}", plan.phase),
        "activeAttempt": plan.active_attempt,
        "boughtLadder": plan.bought_ladder,
        "arm": plan.decision.arm.label(),
        "failureSelector": plan.decision.failure_selector,
        "outcomeCount": plan.decision.outcome_count,
        "dueUnixSeconds": plan.decision.due_unix_seconds,
        "observedUnixSeconds": plan.observed_unix_seconds,
        "terminalSequence": plan.terminal_sequence,
        "certificate": plan.certificate.to_string(),
        "certificateSeatShortfallLamports": plan.seat_shortfall_lamports,
        "certificateFunding": if plan.source_reserve_funding {
            "pre-liability-source-reserve"
        } else {
            "same-transaction-worker-prepay"
        },
        "sourceReserveAvailableLamports": plan.source_reserve_available_lamports,
        "fundingLedger": plan.funding_ledger.to_string(),
        "frameAccounts": plan.frame_accounts,
        "workPaid": work_paid,
        "landed": landed.map(|evidence| json!({
            "signature": evidence.signature,
            "slot": evidence.slot,
            "computeUnitsConsumed": evidence.compute_units_consumed,
            "feeLamports": evidence.fee_lamports,
        })),
    });
    std::fs::write(
        path,
        format!("{}\n", serde_json::to_string_pretty(&document)?),
    )?;
    Ok(())
}

fn parse(arguments: Vec<String>, expected: ExpectedClusterV1) -> Result<ArgumentsV1> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut plan = None;
    let mut evidence = None;
    let mut market = None;
    let mut terminal_sequence = None;
    let mut worker = None;
    let mut worker_keypair = None;
    let mut output = None;
    let mut max_wait_seconds = None;
    let mut wait = false;
    let mut execute = false;
    let mut cursor = arguments.into_iter();
    while let Some(flag) = cursor.next() {
        if flag == "--execute" {
            if execute {
                return Err(Error::new("--execute was given twice"));
            }
            execute = true;
            continue;
        }
        if flag == "--wait" {
            if wait {
                return Err(Error::new("--wait was given twice"));
            }
            wait = true;
            continue;
        }
        let value = cursor.next().ok_or_else(|| {
            Error::new(format!(
                "{flag} needs a value; usage: {}",
                usage_for(expected)
            ))
        })?;
        let slot = match flag.as_str() {
            "--rpc-url" => &mut rpc_url,
            DEVNET_ACKNOWLEDGMENT_FLAG if expected == ExpectedClusterV1::Devnet => {
                &mut acknowledgment
            }
            "--plan" => &mut plan,
            "--evidence" => &mut evidence,
            "--market" => &mut market,
            "--terminal-sequence" => &mut terminal_sequence,
            "--worker" => &mut worker,
            "--worker-keypair" => &mut worker_keypair,
            "--max-wait-seconds" => &mut max_wait_seconds,
            "--output" => &mut output,
            other => {
                return Err(Error::new(format!(
                    "unknown {} argument: {other}",
                    command(expected)
                )));
            }
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{flag} was given twice")));
        }
    }
    let required = |value: Option<String>, name: &str| {
        value.ok_or_else(|| {
            Error::new(format!(
                "{name} is required; usage: {}",
                usage_for(expected)
            ))
        })
    };
    if max_wait_seconds.is_some() && !wait {
        return Err(Error::new(
            "--max-wait-seconds is the ceiling on --wait and means nothing without it",
        ));
    }
    let rpc_url = required(rpc_url, "--rpc-url")?;
    let terminal_sequence = required(terminal_sequence, "--terminal-sequence")?
        .parse::<u64>()
        .map_err(|_| Error::new("--terminal-sequence must be a decimal u64"))?;
    if terminal_sequence == 0 {
        return Err(Error::new(
            "--terminal-sequence must be positive; the wire refuses zero",
        ));
    }
    Ok(ArgumentsV1 {
        origin: ClusterOriginV1::parse(&rpc_url, acknowledgment.as_deref())?,
        plan: PathBuf::from(required(plan, "--plan")?),
        evidence: PathBuf::from(required(evidence, "--evidence")?),
        market: required(market, "--market")?
            .parse()
            .map_err(|error| Error::new(format!("--market: {error}")))?,
        terminal_sequence,
        worker: required(worker, "--worker")?
            .parse()
            .map_err(|error| Error::new(format!("--worker: {error}")))?,
        worker_keypair: worker_keypair.map(PathBuf::from),
        output: PathBuf::from(required(output, "--output")?),
        wait,
        max_wait_seconds: match max_wait_seconds {
            None => None,
            Some(raw) => Some(
                raw.parse::<i64>()
                    .map_err(|_| Error::new("--max-wait-seconds must be a decimal i64"))?,
            ),
        },
        execute,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(extra: &[&str]) -> Vec<String> {
        let mut out = vec![
            "--rpc-url".to_owned(),
            "http://127.0.0.1:21400".to_owned(),
            "--plan".to_owned(),
            "/abs/plan.json".to_owned(),
            "--evidence".to_owned(),
            "/abs/evidence.json".to_owned(),
            "--market".to_owned(),
            Pubkey::new_from_array([0x21; 32]).to_string(),
            "--terminal-sequence".to_owned(),
            "1".to_owned(),
            "--worker".to_owned(),
            Pubkey::new_from_array([0x22; 32]).to_string(),
            "--output".to_owned(),
            "/abs/out.json".to_owned(),
        ];
        out.extend(extra.iter().map(|value| (*value).to_owned()));
        out
    }

    /// The two arms are two labels, so a census binding has one owner each.
    #[test]
    fn the_label_names_the_arm() {
        assert_eq!(
            label_for(DeadlineFailureArmV1::PrimaryDeadline),
            "Deadline failure walk: primary-deadline"
        );
        assert_eq!(
            label_for(DeadlineFailureArmV1::LadderExhausted),
            "Deadline failure walk: ladder-exhausted"
        );
    }

    #[test]
    fn a_wait_ceiling_without_a_wait_refuses_by_name() {
        let refusal = parse(
            arguments(&["--max-wait-seconds", "600"]),
            ExpectedClusterV1::OwnedLoopback,
        )
        .expect_err("a ceiling with no wait must refuse");
        assert!(
            format!("{refusal}").contains("means nothing without it"),
            "got {refusal}"
        );
        parse(
            arguments(&["--wait", "--max-wait-seconds", "600"]),
            ExpectedClusterV1::OwnedLoopback,
        )
        .expect("a bounded wait parses");
    }

    #[test]
    fn a_zero_terminal_sequence_refuses_at_the_parser() {
        let mut raw = arguments(&[]);
        let position = raw
            .iter()
            .position(|value| value == "--terminal-sequence")
            .expect("the flag");
        raw[position + 1] = "0".to_owned();
        let refusal = parse(raw, ExpectedClusterV1::OwnedLoopback)
            .expect_err("a zero terminal sequence must refuse");
        assert!(
            format!("{refusal}").contains("wire refuses zero"),
            "got {refusal}"
        );
    }

    #[test]
    fn the_loopback_arm_refuses_the_devnet_acknowledgment() {
        let refusal = parse(
            arguments(&[DEVNET_ACKNOWLEDGMENT_FLAG, "whatever"]),
            ExpectedClusterV1::OwnedLoopback,
        )
        .expect_err("the loopback arm takes no devnet acknowledgment");
        assert!(format!("{refusal}").contains(COMMAND_V1), "got {refusal}");
    }
}
