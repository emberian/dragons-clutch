//! The journey: one campaign, one Market, one ledger.

use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use dclutch_market_core_codec::CoreState;
use serde::Serialize;

use crate::{
    Error, Result,
    ledger::{ConservationLedgerV1, ObservationV1},
    stages::{self, MarketAddressesV1, StageReportV1},
};

/// The load knob's default. Four holders is the smallest number that makes the
/// ring in `holder_to_holder` a ring rather than a swap.
pub(crate) const DEFAULT_HOLDER_COUNT: u32 = 4;

const TRANSCRIPT_SCHEMA_V1: &str = "dclutch-journey-transcript-v1";

/// A stage the journey could not run, and exactly what stands in the way.
///
/// A gap is not a TODO. It names the route, the code that refuses, and the lane
/// that owns the refusal, so that the day the lane lands the gap can be checked
/// off by deleting it rather than by rediscovering what it meant.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct GapV1 {
    pub(crate) stage: String,
    pub(crate) routes: Vec<String>,
    pub(crate) owner: String,
    pub(crate) reason: String,
}

/// What the chain says about each Market this campaign founded.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct MarketPhaseV1 {
    pub(crate) label: String,
    pub(crate) address: String,
    pub(crate) phase: String,
    pub(crate) readiness: String,
    pub(crate) terminal_receipt: bool,
}

/// The journey's own document. The census consumes the run-evidence document
/// beside it; this one is for a human, and for witnesses that want to assert
/// something about the journey rather than about one transaction.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct JourneyTranscriptV1 {
    pub(crate) schema: String,
    pub(crate) holder_count: u32,
    pub(crate) evidence: String,
    /// `conserved` when no law was violated at any boundary.
    pub(crate) conservation_verdict: String,
    pub(crate) conservation_violations: Vec<String>,
    pub(crate) claim_unit_atoms: u64,
    pub(crate) markets: Vec<MarketPhaseV1>,
    pub(crate) stages: Vec<StageReportV1>,
    pub(crate) gaps: Vec<GapV1>,
    pub(crate) observations: Vec<ObservationV1>,
    pub(crate) transactions_total: usize,
    pub(crate) compute_units_total: u64,
}

/// Live one Market's whole life, and account for every atom while doing it.
pub(crate) fn execute(
    spec_path: &Path,
    transcript_path: &Path,
    holder_count: u32,
) -> Result<JourneyTranscriptV1> {
    validate_new_path(transcript_path, "--transcript")?;
    let mut session = crate::runtime::found_through_open(spec_path)?;
    let addresses = MarketAddressesV1::from_evidence(&session.accounts)?;

    let mut ledger = ConservationLedgerV1::new(addresses.mint);
    let (claim_unit_atoms, decimals) =
        stages::admit_open_market(&mut session.rpc, &addresses, &session.accounts, &mut ledger)?;
    ledger.observe(&mut session.rpc, "founding through Open", 0)?;

    let mut stages = vec![StageReportV1 {
        stage: "founding through Open".into(),
        outcome: "executed".into(),
        transactions: session.transactions.len(),
        compute_units: session
            .transactions
            .iter()
            .map(|transaction| transaction.compute_units_consumed.unwrap_or(0))
            .sum(),
        note: "the tier-1 producer's own code, called in this process rather than copied: \
               seven-artifact genesis, immutable five-role activation, Found31, DCLTPCB1, and \
               DCLTGMF1 with Open last."
            .into(),
    }];

    let (holders, distribution) = stages::distribute_collateral(
        &mut session.rpc,
        &addresses,
        &session.authority,
        decimals,
        holder_count,
        &mut ledger,
        &mut session.transactions,
        &mut session.accounts,
    )?;
    stages.push(distribution);
    // Every transfer here is between two accounts the ledger already tracks, so
    // the tracked total must not move at all. Declaring zero is the strong
    // claim: it fails if a single atom went anywhere else.
    ledger.observe(
        &mut session.rpc,
        "post-open life: collateral distribution",
        0,
    )?;

    stages.push(stages::holder_to_holder(
        &mut session.rpc,
        &addresses,
        &session.authority,
        decimals,
        &holders,
        &mut session.transactions,
    )?);
    ledger.observe(
        &mut session.rpc,
        "post-open life: holder-to-holder collateral",
        0,
    )?;

    stages.push(stages::recover_rent(
        &mut session.rpc,
        &session.plan,
        &addresses,
        &session.authority,
        &mut session.transactions,
    )?);
    ledger.observe(&mut session.rpc, "rent recovery", 0)?;

    let markets = vec![
        market_phase(&mut session.rpc, "founding_market", addresses.founding_market)?,
        market_phase(&mut session.rpc, "found31_market", addresses.found31_market)?,
    ];
    let gaps = gap_register();
    for gap in &gaps {
        stages.push(StageReportV1 {
            stage: gap.stage.clone(),
            outcome: "blocked".into(),
            transactions: 0,
            compute_units: 0,
            note: format!("{} Owner: {}.", gap.reason, gap.owner),
        });
    }

    let evidence = session.evidence();
    let evidence_path = PathBuf::from(&session.spec.output);
    write_json(&evidence_path, &evidence)?;

    let violations = ledger.violations();
    let transcript = JourneyTranscriptV1 {
        schema: TRANSCRIPT_SCHEMA_V1.into(),
        holder_count,
        evidence: evidence_path.display().to_string(),
        conservation_verdict: if violations.is_empty() {
            "conserved".into()
        } else {
            "violated".into()
        },
        conservation_violations: violations.clone(),
        claim_unit_atoms,
        markets,
        stages,
        gaps,
        observations: ledger.observations().to_vec(),
        transactions_total: session.transactions.len(),
        compute_units_total: session
            .transactions
            .iter()
            .map(|transaction| transaction.compute_units_consumed.unwrap_or(0))
            .sum(),
    };
    write_json(transcript_path, &transcript)?;
    if !violations.is_empty() {
        return Err(Error::new(format!(
            "the conservation ledger reported {} violated law(s); the transcript is at {}:\n  {}",
            violations.len(),
            transcript_path.display(),
            violations.join("\n  ")
        )));
    }
    Ok(transcript)
}

fn market_phase(rpc: &mut crate::rpc::Rpc, label: &str, address: solana_sdk::pubkey::Pubkey) -> Result<MarketPhaseV1> {
    let account = rpc.required_account(address, label)?;
    let state = CoreState::decode(&account.data)
        .map_err(|error| Error::new(format!("{label} Core state: {error:?}")))?;
    Ok(MarketPhaseV1 {
        label: label.into(),
        address: address.to_string(),
        phase: format!("{:?}", state.phase),
        readiness: format!("{:?}", state.readiness),
        terminal_receipt: state.terminal_receipt.is_some(),
    })
}

/// Everything a user would do next that this journey cannot do yet.
///
/// These are read off the code, not off a refused transaction: the routes are
/// unreachable for a reason that stops the frame from being CONSTRUCTIBLE by a
/// wallet at all, so there is no honest transaction to submit and record. Each
/// entry names the exact predicate, so the claim is checkable without rerunning
/// anything.
fn gap_register() -> Vec<GapV1> {
    vec![
        GapV1 {
            stage: "post-open life: outcome-token distribution and holder-to-holder transfers".into(),
            routes: vec![
                "claims/protocol_position_v2::process".into(),
                "claims/sparse_native_transfer_v1::process".into(),
            ],
            owner: "W2i (Trading Hot gate)".into(),
            reason: "Every Claims mutation frame puts a CallerAuthority at index 0 that must be \
                     BOTH a signer and the CallerAuthoritySeedsV1 PDA under the calling program, \
                     and then re-authenticates that program against the Registry activation cache \
                     as the Trading role (protocol_position_v2.rs authenticate at \
                     ExecutionRoleV1::Trading; sparse_native_transfer_v1.rs authenticate_authority \
                     and authenticate_releases). Only a program can sign its own PDA, so on a \
                     validator carrying the immutable five-role release set the sole admissible \
                     caller is the deployed Trading program -- and Trading's outer dispatch routes \
                     everything that is not DCLTGMF1, DCLTPCB1, DCLTPCA1, or the capability seal \
                     into hot_v3::process_hot_execution_v3. So the whole of post-Open Claims life \
                     is behind the Hot gate, not just Direct fills. There is no wallet-constructible \
                     frame to submit, which is why this is stated from the code rather than from a \
                     refusal.".into(),
        },
        GapV1 {
            stage: "post-open life: a Custody vault cycle per holder".into(),
            routes: vec![
                "custody/process_instruction#OpenVault".into(),
                "custody/process_instruction#Transfer".into(),
                "custody/process_instruction#CloseVault".into(),
            ],
            owner: "W2i (Trading Hot gate)".into(),
            reason: "Custody's nine-account common prefix has the same shape: index 0 is a signing \
                     CallerAuthority PDA and index 4 is the caller program, re-authenticated \
                     against the activation cache. A holder cannot open, deposit to, or withdraw \
                     from a vault directly; the operation has to arrive by CPI from an activated \
                     role, which post-Open means Trading Hot."
                .into(),
        },
        GapV1 {
            stage: "resolution: deliver the Pyth provider evidence and reach Terminal".into(),
            routes: vec![
                "core/resolution::process#CreateFund".into(),
                "core/persist_state#VerifyFundReady".into(),
                "core/execute_provider_v3::process#ExecuteProvider".into(),
                "core/persist_state#AdmitTerminal".into(),
            ],
            owner: "the Source/provider tier; and an owner decision on the founding ladder".into(),
            reason: "NEW FINDING, and the one worth acting on: at HEAD the atomic founding and the \
                     resolution lifecycle are mutually exclusive prestates. Every route that can \
                     put a terminal receipt on a Market consumes a SourceResolutionStateV2 -- \
                     execute_provider_v3 requires one at phase Primary, funded::process_funded_ \
                     transition takes one as account 0, and resolution::process#AdmitTerminal \
                     requires the certificate those produce. The ONLY route that creates a Source \
                     state is core/resolution::process#CreateFund, whose phase gate \
                     (core-sbf/src/resolution.rs:331) admits Founding+Prepaid and nothing else. \
                     DCLTGMF1's commit-last stage is open_series_market \
                     (core-sbf/src/generic_founding_v1.rs:1671, market-core-codec generated.rs:922), \
                     which goes Founding+Prepaid -> Open+Consumed in ONE transition and never \
                     passes through Ready. So the moment a Market is founded atomically, the route \
                     that would give it a Source state has already closed behind it. Note the \
                     precise shape: the founded Market PASSES AdmitTerminal's own phase gate \
                     (Open+Consumed) -- what it can never obtain is the certificate, because the \
                     thing that mints one needs the Source state. The reachable prestate is on the \
                     same ledger: the canonical Found31 Market, still Founding+Prepaid, which is \
                     what a Source/provider tier should drive."
                .into(),
        },
        GapV1 {
            stage: "redemption: winners redeem through terminal settlement".into(),
            routes: vec!["claims/terminal_settlement_v3::process".into()],
            owner: "TA-CL, behind the resolution gap and the Hot gate".into(),
            reason: "Terminal settlement needs a Market with terminal_receipt set, which needs the \
                     resolution gap closed, AND it is a SignedDeltaV3-framed Claims route, so it \
                     carries the same signing-CallerAuthority requirement as every other Claims \
                     mutation. Two independent gates, either of which alone is enough to block it."
                .into(),
        },
        GapV1 {
            stage: "retirement and rent closure".into(),
            routes: vec![
                "core/begin_retiring::process#BeginRetiring".into(),
                "core/retire_v1::process#Retire".into(),
                "rent/process_close_v2#Close".into(),
            ],
            owner: "cycle-2 retirement, behind the resolution gap".into(),
            reason: "BeginRetiring admits only Phase::Terminal, and rent close_v2 additionally \
                     requires a retired Market plus a Core close-authority signer. Both sit behind \
                     the resolution gap above. The rent SWEEP half of recovery is NOT behind it and \
                     this journey executes it."
                .into(),
        },
    ]
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| Error::new(format!("create {}: {error}", path.display())))?
        .write_all(&bytes)?;
    Ok(())
}

fn validate_new_path(path: &Path, label: &str) -> Result<()> {
    if !path.is_absolute() || path.exists() || std::fs::symlink_metadata(path).is_ok() {
        return Err(Error::new(format!(
            "{label} must be an absolute path that does not exist yet; the journey never \
             overwrites prior evidence"
        )));
    }
    let parent = path
        .parent()
        .ok_or_else(|| Error::new(format!("{label} omitted its parent directory")))?;
    if !parent.is_dir() {
        return Err(Error::new(format!(
            "{label} parent must be an existing directory"
        )));
    }
    Ok(())
}
