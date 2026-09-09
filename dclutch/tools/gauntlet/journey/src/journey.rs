//! The journey: one campaign, one Market, one ledger.

use std::{
    fs::OpenOptions,
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    process::Command,
};

use dclutch_market::{CoreState, Phase};
use dclutch_source::{SourceResolutionStateV2, resolution::SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V3};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use solana_sdk::signature::Signer;

use crate::{
    Error, Result, failure,
    ledger::{ClassClaimV1, ConservationLedgerV1, LamportClaimV1, ObservationV1},
    provider, resolution, spine,
    stages::{self, MarketAddressesV1, StageReportV1},
};

/// The load knob's default. Four holders is the smallest number that makes the
/// ring in `holder_to_holder` a ring rather than a swap.
pub(crate) const DEFAULT_HOLDER_COUNT: u32 = 4;

const TRANSCRIPT_SCHEMA_V1: &str = "dclutch-journey-transcript-v1";

/// The rung this campaign's Market buys, in the SHIPPED flag's spelling.
///
/// One rung, at a 2,500-bp confidence bound tighter than the lab's 10,000-bp
/// ceiling -- a market whose first choice went silent has a reason to demand a
/// better-conditioned reading from its second.
///
/// The 900-second interval is a measured local-validator profile for this
/// journey, not a protocol bound.  The 2026-09-07 recovery capture reached
/// Core's provider execute 833 seconds after the primary window ended: the
/// former 120-second interval had expired before the real Pyth transport began
/// and the kernel correctly refused `DeadlineElapsed`.  This preserves at
/// 187 seconds of execution headroom on that measured path; remeasure it
/// when the provider route's transaction profile changes.
const DEFAULT_RECOVERY_RUNGS_V1: &str = "2500:900";

/// One account the chain was read at, at the instant a stage refused.
///
/// Deliberately shallow -- owner, lamports, width, executable, presence. A
/// wall is diagnosed by which accounts EXIST and who owns them far more often
/// than by their contents, and a transcript that tried to decode every watched
/// account would fail to write exactly when the state is strange.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct ChainAccountV1 {
    pub(crate) label: String,
    pub(crate) address: String,
    pub(crate) present: bool,
    pub(crate) owner: String,
    pub(crate) lamports: u64,
    pub(crate) data_len: usize,
    pub(crate) executable: bool,
}

/// The stage a helper hard-errored in, its own sentence, and the chain under it.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct JourneyWallV1 {
    /// The stage the campaign had entered. `entering` is called once per stage
    /// and nowhere else, so this is a fact about control flow rather than a
    /// guess read back off the error text.
    pub(crate) stage: String,
    /// The helper's own sentence, verbatim and unsummarised.
    pub(crate) sentence: String,
    /// The finalized slot the chain was read at, when a reader existed.
    pub(crate) finalized_slot: Option<u64>,
    /// Every account the conservation ledger watches, plus the Market, as the
    /// chain held them at the wall. Empty when the wall came before any chain.
    pub(crate) chain: Vec<ChainAccountV1>,
    /// Why `chain` is empty, when it is.
    pub(crate) chain_note: String,
}

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
    /// `honest` or `failure`: which end of the window this run walked.
    pub(crate) walk: String,
    pub(crate) holder_count: u32,
    /// Whether the producer derived its signing keys deterministically. Without
    /// it the bump-search noise makes two runs' compute numbers incomparable,
    /// so a transcript that does not say which it was is a transcript whose
    /// numbers cannot be used.
    pub(crate) deterministic_keypairs: bool,
    pub(crate) evidence: String,
    /// `conserved` when no law was violated at any boundary.
    pub(crate) conservation_verdict: String,
    pub(crate) conservation_violations: Vec<String>,
    pub(crate) claim_unit_atoms: u64,
    pub(crate) markets: Vec<MarketPhaseV1>,
    pub(crate) stages: Vec<StageReportV1>,
    /// Stages that were supposed to execute and did not, with the exact
    /// refusal. A journey that meets one still writes its transcript and its
    /// ledger -- the evidence is the point -- and then fails, so a wall is
    /// never traded for a green run.
    pub(crate) unexpected_refusals: Vec<String>,
    pub(crate) gaps: Vec<GapV1>,
    /// The stage a helper HARD-ERRORED in, if one did, with that helper's own
    /// sentence and what the chain held at that instant.
    ///
    /// A refusal the campaign can carry on past is an `unexpected_refusals`
    /// row and a `refused` stage. This is the other kind: the stage whose
    /// error stopped the run. Until 2026-09-06 there was no such field and no
    /// transcript either -- `execute` propagated the first `Err` with `?`, so
    /// the document was written only on the paths that reached the end. Three
    /// consecutive hbox runs hard-errored, each after thirty-five minutes and
    /// two hundred landed transactions, and each left one line of stderr and
    /// no evidence. The ladder's `drive_crank` had already been repaired the
    /// same way; this is that repair on this tier.
    pub(crate) wall: Option<JourneyWallV1>,
    /// One machine-readable row per shipped-command stage: the admission, the
    /// fill, the fee settlement, the redemption and the three retirement
    /// drivers, each carrying the driver's OWN document rather than this
    /// tier's summary of it.
    pub(crate) spine: Value,
    pub(crate) observations: Vec<ObservationV1>,
    pub(crate) transactions_total: usize,
    pub(crate) compute_units_total: u64,
}

/// Everything the whole-life campaign needs from its runner.
///
/// The journey used to take a `--spec` and a `--market` somebody else had
/// compiled, and it could not stand up its own substrate at all: a Market can
/// only be compiled by `DirectMarketCompilerOwnedV1::load_local`, which
/// observes a LIVE checked deployment, and the runner had no way to produce
/// one whose program identities matched the ones it was about to deploy. So
/// the tier accepted a market compiled against SOME OTHER deployment, which is
/// a market this campaign could not found. Since 2026-09-06 it brings the
/// substrate up itself, exactly as `tools/gauntlet/ladder/` does, and compiles
/// the Market against the deployment it is standing on.
/// Which end of the window the campaign walks.
///
/// The honest walk answers the market through the real Pyth receiver. The
/// failure walk lets the window close unobserved, cranks the ladder to
/// `Exhausted`, commits the Product's own failure selector, and then runs the
/// SAME terminal admission, redemption and retirement stages -- which read a
/// `ResolutionFailure` certificate and refund every ordinary holder pro rata
/// from the Hoard, pay the founder only their holdings, and burn the escrow's
/// column at closure (decisions 0025 and 0027). One campaign, two walks, one
/// set of stages after the terminal: the refund is not a second protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum JourneyWalkV1 {
    Honest,
    Failure,
}

impl JourneyWalkV1 {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Honest => "honest",
            Self::Failure => "failure",
        }
    }
}

pub(crate) struct JourneyRequestV1 {
    pub(crate) walk: JourneyWalkV1,
    pub(crate) transcript: PathBuf,
    pub(crate) work: PathBuf,
    pub(crate) rpc_port: u16,
    pub(crate) checked_release_gate: PathBuf,
    pub(crate) expected_gate_sha256: String,
    pub(crate) expected_source_revision: String,
    pub(crate) expected_source_tree_sha256: String,
    pub(crate) seed: String,
    pub(crate) holder_count: u32,
    pub(crate) hold_after_participant: Option<PathBuf>,
    pub(crate) bootstrap_bin: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ParticipantHandoffV1 {
    schema: String,
    rpc_url: String,
    validator_pid: u32,
    supervisor_pid: u32,
    plan: String,
    market_input: String,
    founding_evidence: String,
    participant_evidence: String,
    key_directory: String,
    campaign_payer_keypair: String,
    bootstrap_bin: String,
    campaign_public_identities: std::collections::BTreeMap<String, String>,
    census: crate::ledger::HandoffCensusV1,
    resume_action: String,
}

/// Resume the canonical Journey after an externally driven Direct fill has
/// reached Terminal on the validator retained by `--hold-after-participant`.
#[derive(Debug)]
pub(crate) struct HeldContinuationRequestV1 {
    pub(crate) handoff: PathBuf,
    pub(crate) direct_finalized: PathBuf,
    pub(crate) direct_public: PathBuf,
    pub(crate) work: PathBuf,
    pub(crate) evidence: PathBuf,
}

/// Resume the same held Journey from its authenticated post-Direct boundary,
/// drive the canonical Resolution owner through durable completion, and then
/// use [`HeldContinuationRequestV1`] for payout and retirement.
#[derive(Debug)]
pub(crate) struct HeldLifecycleRequestV1 {
    pub(crate) continuation: HeldContinuationRequestV1,
    pub(crate) pyth_facts: PathBuf,
    pub(crate) resolution_submitter_keypair: PathBuf,
    pub(crate) resolution_resolver_keypair: PathBuf,
    pub(crate) resolution_fee_payer_keypair: PathBuf,
    pub(crate) resolution_update_keypair: PathBuf,
    pub(crate) max_wait_seconds: i64,
}

struct AuthenticatedHeldBoundaryV1 {
    handoff_path: PathBuf,
    direct_finalized: PathBuf,
    direct_public: PathBuf,
    plan: PathBuf,
    market_input: PathBuf,
    campaign_report: PathBuf,
    stranger_report: PathBuf,
    stranger_keypair: PathBuf,
    handoff: ParticipantHandoffV1,
    plan_sha256: String,
    market_sha256: String,
    handoff_sha256: String,
    direct_finalized_bytes: Vec<u8>,
    direct_public_sha256: String,
    market: solana_sdk::pubkey::Pubkey,
    claims_market: solana_sdk::pubkey::Pubkey,
    source_state: solana_sdk::pubkey::Pubkey,
    direct: crate::direct_trade::AuthenticatedDirectTerminalEvidenceV1,
    stranger: crate::user_position_admission::FinalizedPositionAdmissionEvidenceV1,
    rpc: crate::rpc::Rpc,
}

/// A live campaign session over the checked-mutable substrate.
///
/// The shape `found_through_open` used to return, rebuilt from the campaign's
/// own founding report -- the same substitution the relayed vertical made when
/// it needed a validator that outlives one command. The validator is this
/// process's child and dropping this kills it.
struct JourneySessionV1 {
    #[allow(dead_code)]
    validator: crate::substrate::ValidatorGuardV1,
    rpc: crate::rpc::Rpc,
    rpc_url: String,
    plan: crate::model::SuccessorPlan,
    plan_sha256: String,
    plan_path: PathBuf,
    authority: solana_sdk::signature::Keypair,
    transactions: Vec<crate::model::TransactionEvidence>,
    accounts: std::collections::BTreeMap<String, crate::model::AccountEvidence>,
}

/// Everything the transcript is built from, kept OUTSIDE the campaign.
///
/// The campaign body appends to this as it goes, so the document can be
/// written from whatever the run reached -- including from a stage that
/// refused. `execute` owns it; the body only ever borrows it.
#[derive(Default)]
struct JourneyProgressV1 {
    walk: String,
    holder_count: u32,
    claim_unit_atoms: u64,
    evidence: String,
    markets: Vec<MarketPhaseV1>,
    stages: Vec<StageReportV1>,
    unexpected_refusals: Vec<String>,
    gaps: Vec<GapV1>,
    observations: Vec<ObservationV1>,
    spine: serde_json::Map<String, Value>,
    conservation_violations: Vec<String>,
    transactions_total: usize,
    compute_units_total: u64,
    /// The validator's own URL, as soon as the substrate has one.
    ///
    /// A wall met during the FOUNDING -- fifteen of this campaign's
    /// thirty-five minutes -- has no session and no ledger to read the chain
    /// through, and reported nothing at all. It does have a validator, and the
    /// first question a founding wall raises is whether that validator was
    /// still answering: `... did not reach finalized transaction history` is
    /// what a refusing program and a STALLED CHAIN both look like from the
    /// campaign's side, and they are not the same finding. The wall opens its
    /// own short-lived reader on this URL rather than borrowing the campaign's,
    /// which keeps the diagnosis out of the founding's own borrow graph.
    rpc_url: Option<String>,
    /// Accounts a wall reads that the conservation ledger deliberately does
    /// not watch. The Core Market above all: its phase decides which laws
    /// apply and it is kept OUT of the ledger's aperture on purpose (see
    /// `ConservationLedgerV1::market`), so a wall that could not report it
    /// would be missing the first thing a reader asks.
    probe_accounts: Vec<(String, solana_sdk::pubkey::Pubkey)>,
    /// The stage the campaign is inside RIGHT NOW. Set by `entering` and read
    /// only when a wall is recorded.
    stage_in_flight: String,
    wall: Option<JourneyWallV1>,
}

impl JourneyProgressV1 {
    fn new(holder_count: u32) -> Self {
        Self {
            holder_count,
            stage_in_flight: "before the first stage".into(),
            ..Self::default()
        }
    }

    /// Name the stage the campaign is about to run.
    fn entering(&mut self, stage: &str) {
        self.stage_in_flight = stage.to_owned();
    }

    /// Take the running totals and the ledger's observations as they stand.
    fn sync(&mut self, session: &JourneySessionV1, ledger: &ConservationLedgerV1) {
        self.transactions_total = session.transactions.len();
        self.compute_units_total = session
            .transactions
            .iter()
            .map(|transaction| transaction.compute_units_consumed.unwrap_or(0))
            .sum();
        self.observations = ledger.observations().to_vec();
        self.conservation_violations = ledger.violations();
    }

    /// Record a wall met before the conservation ledger existed.
    ///
    /// There is no aperture to read -- the ledger is what names the accounts --
    /// but there may be a VALIDATOR, and whether it is still answering is the
    /// first thing a founding wall has to say.
    fn wall_before_ledger(&mut self, error: &Error) {
        let sentence = error.to_string();
        self.push_refusal(&sentence);
        let (finalized_slot, chain_note) = match self.rpc_url.clone() {
            None => (
                None,
                "the wall came before this campaign had a validator at all, so there is no chain \
                 state to report"
                    .to_owned(),
            ),
            Some(url) => match crate::rpc::Rpc::connect(&url)
                .and_then(|mut rpc| rpc.finalized_slot().map(|slot| (rpc, slot)))
            {
                Ok((_, slot)) => (
                    Some(slot),
                    format!(
                        "the wall came before the conservation ledger existed, so there is no \
                         aperture to report -- but the validator at {url} ANSWERED, at finalized \
                         slot {slot}, so the chain was live when the stage refused"
                    ),
                ),
                Err(error) => (
                    None,
                    format!(
                        "the wall came before the conservation ledger existed, and the validator \
                         at {url} DID NOT ANSWER a fresh reader either: {error}. A stage that \
                         timed out against a chain in this state timed out on the chain, not on a \
                         refusal"
                    ),
                ),
            },
        };
        self.wall = Some(JourneyWallV1 {
            stage: self.stage_in_flight.clone(),
            sentence,
            finalized_slot,
            chain: Vec::new(),
            chain_note,
        });
    }

    /// Record a wall met with a live validator under it.
    fn wall_on_chain(
        &mut self,
        session: &mut JourneySessionV1,
        ledger: &ConservationLedgerV1,
        error: &Error,
    ) {
        let sentence = error.to_string();
        self.push_refusal(&sentence);
        self.sync(session, ledger);
        let finalized_slot = session.rpc.finalized_slot().ok();
        let chain = chain_state(&mut session.rpc, ledger, &self.probe_accounts);
        self.wall = Some(JourneyWallV1 {
            stage: self.stage_in_flight.clone(),
            sentence,
            finalized_slot,
            chain,
            chain_note: String::new(),
        });
    }

    fn push_refusal(&mut self, sentence: &str) {
        let stage = self.stage_in_flight.clone();
        self.unexpected_refusals
            .push(format!("{stage} -- {sentence}"));
        self.stages.push(StageReportV1 {
            stage,
            outcome: "refused".into(),
            transactions: 0,
            compute_units: 0,
            note: format!(
                "REFUSED, and the refusal STOPPED the campaign: {sentence}. Every stage below \
                 this one in the register was never entered."
            ),
        });
    }

    fn into_transcript(self) -> JourneyTranscriptV1 {
        JourneyTranscriptV1 {
            schema: TRANSCRIPT_SCHEMA_V1.into(),
            walk: self.walk,
            holder_count: self.holder_count,
            deterministic_keypairs: true,
            evidence: self.evidence,
            conservation_verdict: if self.conservation_violations.is_empty() {
                "conserved".into()
            } else {
                "violated".into()
            },
            conservation_violations: self.conservation_violations,
            claim_unit_atoms: self.claim_unit_atoms,
            markets: self.markets,
            stages: self.stages,
            unexpected_refusals: self.unexpected_refusals,
            gaps: self.gaps,
            spine: Value::Object(self.spine),
            observations: self.observations,
            transactions_total: self.transactions_total,
            compute_units_total: self.compute_units_total,
            wall: self.wall,
        }
    }
}

/// Read the ledger's whole aperture, plus the Market, at one instant.
///
/// Every account is reported whether or not it is there: "the funding ledger
/// this campaign derived does not exist" and "it exists and something else
/// owns it" are different findings and a probe that skipped absent accounts
/// would print the same thing for both.
fn chain_state(
    rpc: &mut crate::rpc::Rpc,
    ledger: &ConservationLedgerV1,
    extra: &[(String, solana_sdk::pubkey::Pubkey)],
) -> Vec<ChainAccountV1> {
    let mut probes: Vec<(String, solana_sdk::pubkey::Pubkey)> = ledger
        .market()
        .map(|market| ("core_market".to_owned(), market))
        .into_iter()
        .collect();
    probes.extend(extra.iter().cloned());
    probes.extend(
        ledger
            .watched()
            .map(|(label, address)| (label.to_owned(), address)),
    );
    probes
        .into_iter()
        .map(|(label, address)| match rpc.account(address) {
            Ok(Some(account)) => ChainAccountV1 {
                label,
                address: address.to_string(),
                present: true,
                owner: account.owner.to_string(),
                lamports: account.lamports,
                data_len: account.data.len(),
                executable: account.executable,
            },
            Ok(None) => ChainAccountV1 {
                label,
                address: address.to_string(),
                present: false,
                owner: String::new(),
                lamports: 0,
                data_len: 0,
                executable: false,
            },
            Err(error) => ChainAccountV1 {
                label: format!("{label} (unread: {error})"),
                address: address.to_string(),
                present: false,
                owner: String::new(),
                lamports: 0,
                data_len: 0,
                executable: false,
            },
        })
        .collect()
}

/// Live one Market's whole life, and account for every atom while doing it.
///
/// THE TRANSCRIPT IS WRITTEN ON EVERY PATH. `execute` owns the document and
/// the campaign body only fills it, so a stage whose helper hard-errors ends
/// the run with a transcript naming that stage, quoting the helper's own
/// sentence and reporting what the chain held at that instant -- and then
/// fails. This is not a softened refusal: the error still propagates and the
/// run still fails.
pub(crate) fn execute(request: JourneyRequestV1) -> Result<JourneyTranscriptV1> {
    validate_new_path(&request.transcript, "--transcript")?;
    std::fs::create_dir_all(&request.work)?;
    let mut progress = JourneyProgressV1::new(request.holder_count);
    progress.walk = request.walk.label().to_owned();
    let outcome = run(&request, &mut progress);
    let transcript = progress.into_transcript();
    write_json(&request.transcript, &transcript)?;
    if let Err(error) = outcome {
        let stage = transcript
            .wall
            .as_ref()
            .map_or("an unnamed stage", |wall| wall.stage.as_str());
        return Err(Error::new(format!(
            "the journey stopped at `{stage}`; the transcript names the stage, quotes the \
             helper's own sentence and reports the chain under it, and the conservation ledger \
             up to that boundary is in it, at {}:\n  {error}",
            request.transcript.display()
        )));
    }
    if !transcript.conservation_violations.is_empty() {
        return Err(Error::new(format!(
            "the conservation ledger reported {} violated law(s); the transcript is at {}:\n  {}",
            transcript.conservation_violations.len(),
            request.transcript.display(),
            transcript.conservation_violations.join("\n  ")
        )));
    }
    if !transcript.unexpected_refusals.is_empty() {
        return Err(Error::new(format!(
            "{} stage(s) that were supposed to execute refused; the transcript and the complete \
             conservation ledger are at {}:\n  {}",
            transcript.unexpected_refusals.len(),
            request.transcript.display(),
            transcript.unexpected_refusals.join("\n  ")
        )));
    }
    Ok(transcript)
}

fn authenticate_held_boundary_v1(
    request: &HeldContinuationRequestV1,
) -> Result<AuthenticatedHeldBoundaryV1> {
    let handoff_path = canonical_existing_file(&request.handoff, "--handoff")?;
    let direct_finalized =
        canonical_existing_file(&request.direct_finalized, "--direct-finalized")?;
    let direct_public = canonical_existing_file(&request.direct_public, "--direct-public")?;

    let handoff_bytes = std::fs::read(&handoff_path)?;
    let handoff: ParticipantHandoffV1 = serde_json::from_slice(&handoff_bytes)?;
    if handoff.schema != "dclutch-private-validator-participant-handoff-v1"
        || handoff.resume_action
            != "SIGCONT performs cleanup only after the external lifecycle has finished"
    {
        return Err(Error::new(
            "--handoff is not the canonical held-Journey participant boundary",
        ));
    }
    let plan = canonical_existing_file(Path::new(&handoff.plan), "handoff plan")?;
    let market_input =
        canonical_existing_file(Path::new(&handoff.market_input), "handoff Market input")?;
    let campaign_report = canonical_existing_file(
        Path::new(&handoff.founding_evidence),
        "handoff founding evidence",
    )?;
    let buyer_report = canonical_existing_file(
        Path::new(&handoff.participant_evidence),
        "handoff participant evidence",
    )?;
    let admission_dir = buyer_report
        .parent()
        .ok_or_else(|| Error::new("handoff participant evidence omitted its parent"))?;
    if buyer_report.file_name().and_then(|value| value.to_str()) != Some("admission-buyer.json") {
        return Err(Error::new(
            "the v1 handoff participant evidence is not the canonical admission-buyer.json",
        ));
    }
    let stranger_report = canonical_existing_file(
        &admission_dir.join("admission-stranger.json"),
        "held stranger admission evidence",
    )?;
    let stranger_keypair = canonical_existing_file(
        &admission_dir.join("second-stranger.json"),
        "held stranger keypair",
    )?;

    let plan_bytes = std::fs::read(&plan)?;
    let market_bytes = std::fs::read(&market_input)?;
    let campaign_bytes = std::fs::read(&campaign_report)?;
    let plan_sha256 = sha256_hex(&plan_bytes);
    let market_sha256 = sha256_hex(&market_bytes);
    let handoff_sha256 = sha256_hex(&handoff_bytes);
    let campaign = crate::campaign::parse_campaign_terminal_evidence_with_expected_cluster_v1(
        &campaign_bytes,
        crate::cluster::ExpectedClusterV1::OwnedLoopback,
    )?;
    if campaign.plan_sha256 != plan_sha256 || campaign.market_sha256 != market_sha256 {
        return Err(Error::new(
            "held plan or Market input differs from the founding evidence",
        ));
    }
    let campaign_key = |label: &str| -> Result<solana_sdk::pubkey::Pubkey> {
        campaign
            .accounts
            .get(label)
            .ok_or_else(|| Error::new(format!("founding evidence omitted {label}")))?
            .address
            .parse()
            .map_err(|error| Error::new(format!("founding {label}: {error}")))
    };
    let market = campaign_key("founding_market")?;
    let claims_market = campaign_key("claims_aggregate")?;
    let source_state = campaign_key("resolution_source_state")?;

    let mut rpc = crate::rpc::Rpc::connect(&handoff.rpc_url)?;
    let direct = crate::direct_trade::authenticate_owned_loopback_terminal_evidence_v1(
        &mut rpc,
        &direct_finalized,
        market,
        &plan_sha256,
        &market_sha256,
    )?;
    let direct_finalized_bytes = std::fs::read(&direct_finalized)?;
    let direct_document: serde_json::Value = serde_json::from_slice(&direct_finalized_bytes)?;
    let direct_public_bytes = std::fs::read(&direct_public)?;
    let direct_public_sha256 = sha256_hex(&direct_public_bytes);
    if direct_document
        .get("publicManifestSha256")
        .and_then(serde_json::Value::as_str)
        != Some(direct_public_sha256.as_str())
        || direct.direct.market != market
        || direct.claims_market != claims_market
        || handoff.census.aggregate != claims_market.to_string()
        || handoff.census.mint != direct.direct.mint.to_string()
    {
        return Err(Error::new(
            "held handoff, Direct evidence, public manifest and founding evidence do not join",
        ));
    }

    let buyer =
        crate::user_position_admission::parse_finalized_position_admission_evidence_for_cluster_v1(
            &std::fs::read(&buyer_report)?,
            &mut rpc,
            crate::cluster::ExpectedClusterV1::OwnedLoopback,
        )?;
    let stranger =
        crate::user_position_admission::parse_finalized_position_admission_evidence_for_cluster_v1(
            &std::fs::read(&stranger_report)?,
            &mut rpc,
            crate::cluster::ExpectedClusterV1::OwnedLoopback,
        )?;
    if buyer.market != market
        || buyer.claims_market != claims_market
        || buyer.owner != direct.direct.buyer_owner
        || buyer.position != direct.direct.buyer_position
        || stranger.market != market
        || stranger.claims_market != claims_market
    {
        return Err(Error::new(
            "held participant admissions do not join the authenticated Direct/founding roots",
        ));
    }

    Ok(AuthenticatedHeldBoundaryV1 {
        handoff_path,
        direct_finalized,
        direct_public,
        plan,
        market_input,
        campaign_report,
        stranger_report,
        stranger_keypair,
        handoff,
        plan_sha256,
        market_sha256,
        handoff_sha256,
        direct_finalized_bytes,
        direct_public_sha256,
        market,
        claims_market,
        source_state,
        direct,
        stranger,
        rpc,
    })
}

/// Continue one retained Journey from authenticated Direct history after the
/// Market has reached Terminal.
///
/// The held supervisor remains the validator's lifetime owner. This process
/// connects to that exact loopback genesis and calls the same shipped payout,
/// Position-close, terminal-sequence, maker-close and checkpoint drivers as a
/// normal Journey. Until the Resolution producer is wired immediately before
/// this seam, a non-Terminal Market refuses before any key file is opened.
pub(crate) fn continue_held_after_terminal(
    request: HeldContinuationRequestV1,
) -> Result<serde_json::Value> {
    validate_new_path(&request.evidence, "--evidence")?;
    validate_work_directory(&request.work)?;
    let boundary = authenticate_held_boundary_v1(&request)?;
    finish_held_after_terminal_v1(request, boundary, None)
}

/// Continue the held Journey from Open through the existing Resolution owner,
/// then through the post-Terminal seam above. Resolution's own durable input,
/// table journal and checkpoint remain the restart authority in `--work`.
pub(crate) fn continue_held_lifecycle(
    request: HeldLifecycleRequestV1,
) -> Result<serde_json::Value> {
    validate_new_path(&request.continuation.evidence, "--evidence")?;
    validate_work_directory(&request.continuation.work)?;
    let pyth_facts = canonical_existing_file(&request.pyth_facts, "--pyth-facts")?;
    let submitter_keypair = canonical_existing_file(
        &request.resolution_submitter_keypair,
        "--resolution-submitter-keypair",
    )?;
    let resolver_keypair = canonical_existing_file(
        &request.resolution_resolver_keypair,
        "--resolution-resolver-keypair",
    )?;
    let fee_payer_keypair = canonical_existing_file(
        &request.resolution_fee_payer_keypair,
        "--resolution-fee-payer-keypair",
    )?;
    let update_keypair = canonical_existing_file(
        &request.resolution_update_keypair,
        "--resolution-update-keypair",
    )?;
    let mut boundary = authenticate_held_boundary_v1(&request.continuation)?;
    let campaign_payer_keypair = canonical_existing_file(
        Path::new(&boundary.handoff.campaign_payer_keypair),
        "held campaign payer keypair",
    )?;
    if submitter_keypair != campaign_payer_keypair {
        return Err(Error::new(
            "--resolution-submitter-keypair must be the held Journey's named campaign payer keypair",
        ));
    }
    let open = CoreState::decode(
        &boundary
            .rpc
            .required_account(boundary.market, "held pre-Resolution Core Market")?
            .data,
    )
    .map_err(|error| Error::new(format!("held pre-Resolution Core Market: {error:?}")))?;
    let resolution_work = request.continuation.work.join("resolution");
    let resumable_resolution = resolution_work.join("input.json").is_file()
        && resolution_work.join("checkpoint.json").is_file();
    let initial_open = open.phase == Phase::Open && open.terminal_receipt.is_none();
    let durable_terminal_resume =
        open.phase == Phase::Terminal && open.terminal_receipt.is_some() && resumable_resolution;
    if !initial_open && !durable_terminal_resume {
        return Err(Error::new(
            "full held continuation requires the authenticated post-Direct Market to be Open, or Terminal with an existing native Resolution input and checkpoint to reauthenticate",
        ));
    }
    std::fs::create_dir_all(&resolution_work)?;
    let resolution = spine::resolve_held_market_v1(
        &mut boundary.rpc,
        spine::HeldResolutionContextV1 {
            rpc_url: &boundary.handoff.rpc_url,
            plan: &boundary.plan,
            plan_sha256: &boundary.plan_sha256,
            campaign_report: &boundary.campaign_report,
            market_input: &boundary.market_input,
            market_sha256: &boundary.market_sha256,
            market: boundary.market,
            pyth_facts: &pyth_facts,
            submitter_keypair: &submitter_keypair,
            resolver_keypair: &resolver_keypair,
            fee_payer_keypair: &fee_payer_keypair,
            update_keypair: &update_keypair,
            work: &resolution_work,
            max_wait_seconds: request.max_wait_seconds,
            terminal_resume: durable_terminal_resume,
        },
    )?;
    finish_held_after_terminal_v1(request.continuation, boundary, Some(resolution))
}

fn finish_held_after_terminal_v1(
    request: HeldContinuationRequestV1,
    boundary: AuthenticatedHeldBoundaryV1,
    resolution: Option<spine::HeldResolutionEvidenceV1>,
) -> Result<serde_json::Value> {
    let AuthenticatedHeldBoundaryV1 {
        handoff_path,
        direct_finalized,
        direct_public,
        plan,
        market_input,
        campaign_report,
        stranger_report,
        stranger_keypair,
        handoff,
        plan_sha256,
        market_sha256,
        handoff_sha256,
        direct_finalized_bytes,
        direct_public_sha256,
        market,
        claims_market,
        source_state,
        direct,
        stranger,
        mut rpc,
    } = boundary;

    let terminal = CoreState::decode(&rpc.required_account(market, "held Core Market")?.data)
        .map_err(|error| Error::new(format!("held Core Market: {error:?}")))?;
    if terminal.phase != Phase::Terminal || terminal.terminal_receipt.is_none() {
        return Err(Error::new(
            "held continuation requires the existing Resolution owner step to leave one authenticated Terminal Market",
        ));
    }
    let source_account = rpc.required_account(source_state, "held Resolution Source state")?;
    let source = SourceResolutionStateV2::decode(&source_account.data)
        .map_err(|error| Error::new(format!("held Resolution Source state: {error:?}")))?;
    let terminal_source = source
        .terminal_projection()
        .map_err(|error| Error::new(format!("held terminal Source projection: {error:?}")))?;
    if source.market() != market.to_bytes() {
        return Err(Error::new(
            "held terminal Source state belongs to another Market",
        ));
    }
    let closure_sequence = terminal_source
        .terminal_sequence()
        .checked_add(1)
        .ok_or_else(|| Error::new("held Source closure sequence overflowed"))?;
    let source_receipt = solana_sdk::pubkey::Pubkey::find_program_address(
        &[
            SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V3,
            source_state.as_ref(),
            &closure_sequence.to_le_bytes(),
        ],
        &source_account.owner,
    )
    .0;

    let payer_keypair = canonical_existing_file(
        Path::new(&handoff.campaign_payer_keypair),
        "held campaign payer keypair",
    )?;
    let payer = crate::substrate::load_keypair(&payer_keypair)?.pubkey();
    let key_directory = canonical_existing_directory(
        Path::new(&handoff.key_directory),
        "held Direct key directory",
    )?;
    let seller_keypair = canonical_existing_file(
        &key_directory.join("founding-founder.json"),
        "held seller keypair",
    )?;
    let buyer_keypair = canonical_existing_file(
        &key_directory.join("participant.json"),
        "held buyer keypair",
    )?;
    let keypairs = std::collections::BTreeMap::from([(
        "participant".to_owned(),
        buyer_keypair.display().to_string(),
    )]);
    let founding_keypairs = std::collections::BTreeMap::from([(
        "founding-founder".to_owned(),
        seller_keypair.display().to_string(),
    )]);
    let context = spine::SpineContextV1 {
        rpc_url: &handoff.rpc_url,
        plan: &plan,
        campaign_report: &campaign_report,
        market_input: &market_input,
        market,
        work: &request.work,
        keypairs: &keypairs,
        founding_keypairs: &founding_keypairs,
    };
    let mut continuation = spine::SpineV1::new();
    let (schema, resolution_report) = match resolution {
        None => (
            "dclutch-held-journey-continuation-evidence-v1",
            serde_json::Value::Null,
        ),
        Some(resolution) => {
            let compute_units = resolution
                .transactions
                .iter()
                .map(|transaction| transaction.compute_units_consumed.unwrap_or(0))
                .sum();
            continuation.stages.push(StageReportV1 {
                stage: "canonical durable Resolution through Complete".into(),
                outcome: "executed".into(),
                transactions: resolution.transactions.len(),
                compute_units,
                note: "The existing flagship Resolution producer, table provisioner and durable executor authenticated the held Market, reached Core Terminal, reclaimed the provider update after its immutable deadline, and passed its read-only Complete restart.".into(),
            });
            continuation.transactions.extend(resolution.transactions);
            (
                "dclutch-held-journey-continuation-evidence-v2",
                resolution.report,
            )
        }
    };
    spine::settle_fee(
        &mut rpc,
        &context,
        &mut continuation,
        &direct_public,
        direct.direct.buyer_owner,
        &payer_keypair,
    )?;
    let holders = [
        (
            "seller",
            "founding-founder",
            direct.direct.seller_owner,
            direct.direct.seller_collateral_destination,
        ),
        (
            "buyer",
            "participant",
            direct.direct.buyer_owner,
            direct.direct.buyer_collateral_source,
        ),
    ];
    for (holder, role, owner, recipient) in &holders {
        for claim_index in 0..direct.direct.outcome_count {
            spine::redeem(
                &mut rpc,
                &context,
                &mut continuation,
                &format!("{holder}-claim-{claim_index}"),
                role,
                *owner,
                *recipient,
                claim_index,
                payer,
                &payer_keypair,
            )?;
        }
    }
    for (holder, role, owner, _) in &holders {
        spine::close_direct_position(
            &mut rpc,
            &context,
            &mut continuation,
            holder,
            role,
            *owner,
            &direct_finalized,
            payer,
            &payer_keypair,
        )?;
    }
    spine::close_participant_position(
        &mut rpc,
        &context,
        &mut continuation,
        "stranger",
        stranger.owner,
        &stranger_keypair,
        &stranger_report,
        payer,
        &payer_keypair,
    )?;
    spine::retire(
        &mut rpc,
        &context,
        &mut continuation,
        &direct_public,
        &direct_finalized,
        source_receipt,
        payer,
        &payer_keypair,
    )?;
    let post = CoreState::decode(&rpc.required_account(market, "retired Core Market")?.data)
        .map_err(|error| Error::new(format!("retired Core Market: {error:?}")))?;
    let completed = continuation.refusals.is_empty() && post.phase == Phase::Retired;
    let result = serde_json::json!({
        "schema": schema,
        "cluster": "owned-loopback",
        "completed": completed,
        "rpcUrl": handoff.rpc_url,
        "market": market.to_string(),
        "claimsMarket": claims_market.to_string(),
        "terminalReceipt": terminal.terminal_receipt.map(|value| {
            solana_sdk::pubkey::Pubkey::new_from_array(value.to_bytes()).to_string()
        }),
        "finalPhase": format!("{:?}", post.phase),
        "inputs": {
            "handoff": handoff_path.display().to_string(),
            "handoffSha256": handoff_sha256,
            "directFinalized": direct_finalized.display().to_string(),
            "directFinalizedSha256": sha256_hex(&direct_finalized_bytes),
            "directPublic": direct_public.display().to_string(),
            "directPublicSha256": direct_public_sha256,
            "planSha256": plan_sha256,
            "marketInputSha256": market_sha256,
        },
        "resolution": resolution_report,
        "stages": continuation.stages,
        "reports": continuation.reports,
        "transactions": continuation.transactions,
        "refusals": continuation.refusals,
        "retainedSupervisorPid": handoff.supervisor_pid,
        "retainedValidatorPid": handoff.validator_pid,
        "cleanupAction": "after archiving this evidence, SIGCONT the retained supervisor",
    });
    write_json(&request.evidence, &result)?;
    if !completed {
        return Err(Error::new(format!(
            "held Journey continuation did not reach Retired; evidence is at {}",
            request.evidence.display()
        )));
    }
    Ok(result)
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    crate::plan::hex(&sha2::Sha256::digest(bytes))
}

fn canonical_existing_file(path: &Path, label: &str) -> Result<PathBuf> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| Error::new(format!("{label} {}: {error}", path.display())))?;
    if !path.is_absolute()
        || metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > 16 * 1024 * 1024
    {
        return Err(Error::new(format!(
            "{label} must be one absolute regular file within 1..16777216 bytes; the file itself \
             may not be a symlink"
        )));
    }
    let canonical = std::fs::canonicalize(path)?;
    let canonical_metadata = std::fs::symlink_metadata(&canonical)?;
    if !canonical_metadata.is_file()
        || canonical_metadata.len() == 0
        || canonical_metadata.len() > 16 * 1024 * 1024
    {
        return Err(Error::new(format!(
            "{label} did not resolve to one regular file within 1..16777216 bytes"
        )));
    }
    Ok(canonical)
}

fn canonical_existing_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| Error::new(format!("{label} {}: {error}", path.display())))?;
    if !path.is_absolute() || metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(Error::new(format!(
            "{label} must be one absolute directory; the directory itself may not be a symlink"
        )));
    }
    let canonical = std::fs::canonicalize(path)?;
    if !std::fs::symlink_metadata(&canonical)?.is_dir() {
        return Err(Error::new(format!(
            "{label} did not resolve to one directory"
        )));
    }
    Ok(canonical)
}

fn validate_work_directory(path: &Path) -> Result<()> {
    if !path.is_absolute() {
        return Err(Error::new("--work must be absolute"));
    }
    if path.exists() || std::fs::symlink_metadata(path).is_ok() {
        let metadata = std::fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || std::fs::canonicalize(path)? != path
        {
            return Err(Error::new(
                "--work must be one canonical absolute directory and never a symlink",
            ));
        }
        return Ok(());
    }
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("--work omitted its parent directory"))?;
    if !parent.is_dir() || std::fs::canonicalize(parent)? != parent {
        return Err(Error::new(
            "--work parent must be one canonical absolute directory",
        ));
    }
    std::fs::create_dir(path)?;
    Ok(())
}

/// Run the campaign, and record the wall it met if it met one.
///
/// The session and the ledger live HERE rather than inside the body, so that
/// the wall recorder can still read the chain through them -- the validator is
/// the session's child, and a body that owned it would have killed it on the
/// way out of the very error being diagnosed.
fn run(request: &JourneyRequestV1, progress: &mut JourneyProgressV1) -> Result<()> {
    let mut anchor: Option<(JourneySessionV1, ConservationLedgerV1)> = None;
    let outcome = campaign(request, progress, &mut anchor);
    match anchor.as_mut() {
        Some((session, ledger)) => {
            if let Err(error) = &outcome {
                progress.wall_on_chain(session, ledger, error);
            } else {
                progress.sync(session, ledger);
            }
        }
        None => {
            if let Err(error) = &outcome {
                progress.wall_before_ledger(error);
            }
        }
    }
    write_run_evidence(request, progress, anchor.as_ref());
    outcome
}

/// Write the run-evidence document the census folds, on EVERY path.
///
/// `run-journey.sh` says in its own comment that "a campaign that met a wall
/// still writes both documents -- that is the whole design", and then dies on
/// a missing `evidence.json`. It was right about the design and wrong about
/// the code: this document was written by the last statements of the campaign,
/// so a wall discarded it exactly as it discarded the transcript, and the
/// runner's `die` turned every wall into a run with no census row either.
///
/// A wall met before the substrate came up has no transactions and no
/// accounts, and says so with an empty pair rather than with an absent file:
/// "this run landed nothing" is a fact the census can fold, and "the document
/// is missing" is not.
///
/// A failure to write it is recorded in the transcript's own `evidence` field
/// and never replaces the campaign's refusal, which is the finding.
fn write_run_evidence(
    request: &JourneyRequestV1,
    progress: &mut JourneyProgressV1,
    anchor: Option<&(JourneySessionV1, ConservationLedgerV1)>,
) {
    let (rpc_url, plan_sha256, transactions, accounts) = match anchor {
        Some((session, _)) => (
            session.rpc_url.clone(),
            session.plan_sha256.clone(),
            serde_json::to_value(&session.transactions)
                .unwrap_or_else(|_| Value::Array(Vec::new())),
            serde_json::to_value(&session.accounts)
                .unwrap_or_else(|_| Value::Object(serde_json::Map::new())),
        ),
        None => (
            String::new(),
            String::new(),
            Value::Array(Vec::new()),
            Value::Object(serde_json::Map::new()),
        ),
    };
    let path = request.work.join("evidence.json");
    let evidence = serde_json::json!({
        "schema": "dclutch-local-successor-run-evidence-v2",
        "rpc_url": rpc_url,
        "plan_sha256": plan_sha256,
        "transactions": transactions,
        "accounts": accounts,
    });
    progress.evidence = match write_json(&path, &evidence) {
        Ok(()) => path.display().to_string(),
        Err(error) => format!("UNWRITTEN ({}): {error}", path.display()),
    };
}

fn campaign(
    request: &JourneyRequestV1,
    progress: &mut JourneyProgressV1,
    anchor: &mut Option<(JourneySessionV1, ConservationLedgerV1)>,
) -> Result<()> {
    let holder_count = request.holder_count;
    // The gap register is READ OFF THE CODE, not off this run, so it belongs in
    // the document from the first line: a wall met in the founding still has
    // the same doors closed as a run that finishes, and a transcript that
    // dropped them would read as though the campaign had no known gaps.
    progress.gaps = gap_register();
    progress.entering("checked-mutable substrate");

    // ---------------------------------------- 1. the checked-mutable substrate
    let substrate_dir = request.work.join("substrate");
    std::fs::create_dir_all(&substrate_dir)?;
    let checked = crate::substrate::bring_up(&crate::substrate::SubstrateRequestV1 {
        work: &substrate_dir,
        checked_release_gate: &request.checked_release_gate,
        expected_gate_sha256: &request.expected_gate_sha256,
        expected_source_revision: &request.expected_source_revision,
        expected_source_tree_sha256: &request.expected_source_tree_sha256,
        seed: &request.seed,
        rpc_port: request.rpc_port,
    })?;

    // A ROW IS RECORDED WHEN ITS STAGE FINISHES, not when the campaign gets
    // around to it. Both of these used to be pushed after `resolution::derive`,
    // eleven fallible calls later, so a wall anywhere in between produced a
    // stage register whose only row was the refusal -- a transcript that said
    // nothing had executed on a chain that had just taken two hundred
    // transactions.
    progress.stages.push(StageReportV1 {
        stage: "checked-mutable substrate".into(),
        outcome: "executed".into(),
        transactions: 0,
        compute_units: 0,
        note: format!(
            "`local-mutable-prepare-v1` derived the seven-role mutable substrate from the \
             checked release gate ({}), a fresh solana-test-validator booted the prepared \
             account directory, and the administration campaign published, initialized and \
             activated through the retained authority. THE VALIDATOR STAYS UP for every stage \
             below -- that is why this tier can drive shipped commands at all, and why the \
             in-process tier-1 supervisor could not host them.",
            request.expected_gate_sha256
        ),
    });
    progress.entering("the Market, compiled against the deployment it stands on");
    // ------------------------------- 2. the Market, compiled against the chain
    //
    // The default shape is FOUR outcomes over two cuts, which at
    // `categorical_founding_payout_scale_v3` is a payout scale of three -- a
    // REFUNDING market, whose failure column the founding seats rather than
    // issues. That is not a knob this tier turns: it is what the lab's default
    // market has been since the refunding scale landed, and it is the shape the
    // closure burn at the end of this campaign exists for.
    let registry = crate::plan::pubkey(&checked.plan.registry.program_id)?;
    let fee_recipient = solana_sdk::signature::Keypair::new();
    let direct = crate::direct_market::DirectMarketCompilerOwnedV1::load_local(
        &checked.plan_path,
        &checked.rpc_url,
        registry,
        Some(50),
        Some(fee_recipient.pubkey()),
    )?;
    // THE MARKET BUYS A LADDER, and it has to: `resolution::derive` locates a
    // `recovery_policy_record` in the founding's evidence and refuses a Market
    // whose record shape it does not recognise, while `LocalMarketShapeV1`'s
    // default is NO ladder ("defaulting a market into buying one would be
    // spending on the caller's behalf"). Those two have been unsatisfiable
    // together for as long as the tier has been unrunnable, and the first live
    // run of this rebuilt tier found it after 189 founding transactions. One
    // rung is the width that leaves the founding's own shape unmoved -- the
    // Resolution manifest's hard four is `1 + rungs.max(1) + 2`, four at zero
    // rungs and four at one -- so this differs from the lab default in exactly
    // the record the resolution stage needs and in nothing else. The rung
    // string goes through the SHIPPED `--recovery-rungs` parser rather than a
    // second one, the way the ladder tier does it.
    let shape = match request.walk {
        JourneyWalkV1::Honest => crate::market::LocalMarketShapeV1 {
            recovery: Some(crate::local_mutable::parse_recovery_rungs_v1(
                DEFAULT_RECOVERY_RUNGS_V1,
            )?),
            ..crate::market::LocalMarketShapeV1::default()
        },
        // THE FAILURE WALK'S SHAPE. The same one rung, and a shelf life short
        // enough that the primary leg's deadline -- the captured publication
        // instant plus `max_age` -- is behind the chain clock before the market
        // is Open. That is the lab stating a fact about its frozen fixture, and
        // the honest alternative to warping the validator's clock; what it
        // cannot do is answer a rung, which is exactly why it is this walk's
        // shape. See `failure.rs`.
        JourneyWalkV1::Failure => crate::market::LocalMarketShapeV1 {
            recovery: Some(crate::local_mutable::parse_recovery_rungs_v1(
                failure::DEFAULT_FAILURE_RECOVERY_RUNGS_V1,
            )?),
            terminal_max_age_seconds: Some(failure::FAILURE_WALK_MAX_AGE_SECONDS_V1),
            ..crate::market::LocalMarketShapeV1::default()
        },
    };
    let market_input =
        crate::market::demo_market_input_shaped(registry, direct.compiler(), &shape)?;
    let market_path = request.work.join("market.json");
    std::fs::write(&market_path, serde_json::to_vec_pretty(&market_input)?)?;

    progress.entering("founding through Open");
    // ------------------------------------------------------- 3. the founding
    progress.rpc_url = Some(checked.rpc_url.clone());
    let mut rpc = crate::rpc::Rpc::connect(&checked.rpc_url)?;
    let campaign_report = request.work.join("founding-evidence.json");
    let founding =
        crate::substrate::found_market(&checked, &mut rpc, &market_path, &campaign_report)?;
    let authority = crate::substrate::authority_keypair(&checked)?;
    // The founder's collateral wallet answers to the founding's `campaign-payer`
    // role, not to the administration authority above; see
    // `distribute_collateral`.
    let collateral_owner = crate::substrate::campaign_payer_keypair(&checked)?;
    let session = JourneySessionV1 {
        validator: checked.validator,
        rpc,
        rpc_url: checked.rpc_url.clone(),
        plan: checked.plan,
        plan_sha256: checked.plan_sha256,
        plan_path: checked.plan_path.clone(),
        authority,
        transactions: founding.transactions,
        accounts: founding.market.accounts,
    };
    let addresses = MarketAddressesV1::from_evidence(&session.accounts)?;
    let ledger = ConservationLedgerV1::new(addresses.mint, session.authority.pubkey());
    // FROM HERE A WALL CAN BE READ OFF THE CHAIN. The session owns the
    // validator's lifetime, so handing both to `run` is what keeps the
    // validator alive while the refusal that killed the campaign is being
    // written down; a body that owned the guard would have killed the chain on
    // its way out of the very error being diagnosed.
    progress.probe_accounts = vec![
        ("founding_market".to_owned(), addresses.founding_market),
        ("found31_market".to_owned(), addresses.found31_market),
        ("claims_aggregate".to_owned(), addresses.aggregate),
        ("collateral_mint".to_owned(), addresses.mint),
        ("hoard".to_owned(), addresses.hoard),
    ];
    let anchored = anchor.insert((session, ledger));
    let session = &mut anchored.0;
    let ledger = &mut anchored.1;

    progress.stages.push(StageReportV1 {
        stage: "founding through Open".into(),
        outcome: "executed".into(),
        transactions: session.transactions.len(),
        compute_units: session
            .transactions
            .iter()
            .map(|transaction| transaction.compute_units_consumed.unwrap_or(0))
            .sum(),
        note: "`campaign --founding-only` over the live checked substrate: the market compiled by \
               `DirectMarketCompilerOwnedV1::load_local` against THIS deployment, four outcomes \
               over two cuts, which is a refunding payout scale of three."
            .into(),
    });

    // THE PHASE L4 RETIRES ON, BOUND BEFORE THE FIRST CENSUS.
    //
    // `ConservationLedgerV1::track_market` and the whole `market_phase` arm of
    // L4 -- the named `inapplicable`, its reason, the three phases it admits --
    // were written for cohort-14b's post-payout boundary and nothing ever
    // called the binder: `cargo check` reported `method track_market is never
    // used` and the phase read as `None`, so L4 went on asking "Hoard >= worst
    // outcome" of a Market that had paid. The first run of this tier that ever
    // redeemed a holder read three L4 VIOLATIONS at boundaries where the
    // protocol had done exactly what it should (hbox `20260906T174856Z`). A
    // reader, a schema and a refusal with no producer is the shape; this is the
    // producer.
    // The Core Market this campaign resolves and redeems is the founding's own;
    // `found31_market` is Found37's hostile-control market and is never redeemed.
    ledger.track_market(addresses.founding_market);
    progress.entering("admission: the founding really left an Open Market");
    let (claim_unit_atoms, decimals) = stages::admit_open_market(
        &mut session.rpc,
        &addresses,
        &session.accounts,
        crate::plan::pubkey(&session.plan.custody.program_id)?,
        crate::plan::pubkey(&session.plan.rent_credit.program_id)?,
        &mut *ledger,
    )?;
    // The whole cast is registered with the ledger BEFORE the first census, so
    // that every account the journey later creates is first seen as a checked
    // vacancy rather than as a balance with no predecessor. That ordering is
    // what makes L7 applicable across the stages that spend the most lamports;
    // see `stages::plan_holders` and `resolution::watch`.
    let mut holders = stages::plan_holders(holder_count, &mut *ledger)?;
    progress.entering("resolution: derive this Market's funding coordinates");
    let resolution_addresses = resolution::derive(
        &mut session.rpc,
        &session.plan,
        &addresses,
        &session.accounts,
    )?;
    resolution::watch(&mut *ledger, &resolution_addresses);
    progress.entering("resolution: derive the provider transport's coordinates");
    let provider_plan = provider::ProviderPlanV1::derive(&mut session.rpc, &session.plan)?;
    provider::watch(&mut *ledger, &provider_plan);

    ledger.observe(
        &mut session.rpc,
        "founding through Open",
        0,
        0,
        // The founding's lamport placements are the founding campaign's, and
        // this ledger does not restate them: it would be re-deriving another
        // campaign's arithmetic and calling the agreement evidence. L7 begins
        // at the first journey-owned boundary.
        LamportClaimV1::inapplicable(
            "the founding's lamport movements belong to `campaign --founding-only` and are \
             covered by tier 1's own witnesses; this ledger accounts for lamports from the first \
             journey-owned boundary onward",
        ),
        // The same boundary, and the same reason. Declaring `unchanged()` here
        // would be false -- the founding funds the Hoard out of nothing -- and
        // it would also be unevaluated, since the first census has no
        // predecessor.
        ClassClaimV1::inapplicable(
            "the founding's compartment placements belong to the founding campaign; this ledger \
             accounts per compartment class from the first journey-owned boundary onward",
        ),
    )?;

    progress.entering("post-open life: collateral distribution");
    let (distribution, distribution_fees) = stages::distribute_collateral(
        &mut session.rpc,
        &addresses,
        &session.authority,
        &collateral_owner,
        decimals,
        &mut holders,
        &mut session.transactions,
        &mut session.accounts,
    )?;
    progress.stages.push(distribution);
    // Every transfer here is between two accounts the ledger already tracks, so
    // the tracked total must not move at all. Declaring zero is the strong
    // claim: it fails if a single atom went anywhere else.
    ledger.observe(
        &mut session.rpc,
        "post-open life: collateral distribution",
        0,
        0,
        LamportClaimV1::fees(distribution_fees),
        ClassClaimV1::unchanged(),
    )?;

    let (ring, ring_fees) = stages::holder_to_holder(
        &mut session.rpc,
        &addresses,
        &session.authority,
        decimals,
        &holders,
        &mut session.transactions,
    )?;
    progress.stages.push(ring);
    ledger.observe(
        &mut session.rpc,
        "post-open life: holder-to-holder collateral",
        0,
        0,
        LamportClaimV1::fees(ring_fees),
        ClassClaimV1::unchanged(),
    )?;

    progress.entering("post-open life: holder-to-holder collateral");
    // ------------------------------------------- the spine: admission and fill
    //
    // Everything from here is a SHIPPED command, called in this process with
    // the argument vector a host would type. See `spine.rs` for why that is the
    // whole design rather than a convenience.
    let spine_work = request.work.join("spine");
    std::fs::create_dir_all(&spine_work)?;
    let direct_public = spine_work.join("fill").join("direct-trade-public.json");
    let direct_finalized = spine_work.join("fill").join("direct-trade-finalized.json");
    let payer_key = spine_key(&checked.report, "campaign-payer")?;
    let payer = crate::substrate::load_keypair(&payer_key)?.pubkey();
    let context = spine::SpineContextV1 {
        rpc_url: &session.rpc_url,
        plan: &session.plan_path,
        campaign_report: &campaign_report,
        market_input: &market_path,
        market: addresses.founding_market,
        work: &spine_work,
        keypairs: &checked.report.keypairs,
        founding_keypairs: &checked.report.campaign_founding_keypairs,
    };
    let mut spine = spine::SpineV1::new();
    progress.entering("trading: admission, the Direct Hot fill, and the fee settlement");

    // THE ACTIVATION COMES FIRST, as it does on devnet: the Direct execution
    // root is what the fill's root check reads and what the terminal sequence's
    // Direct first-use pair is missing, and nothing else in this tree creates
    // one. It is idempotent, so a resumed run converges.
    spine::activate_direct_capability(&mut session.rpc, &context, &mut spine, payer, &payer_key)?;

    // TWO STRANGERS, and the second one is not decoration. The buyer carries
    // the collateral leg because the fill needs delegated collateral; the
    // second admission is a Position and nothing else, which is the ordinary
    // case and the one a campaign that only ever admits its trader never runs.
    let participant = context_pubkey(&context, "participant")?;
    let fixture_source = evidence_pubkey(&session.accounts, "local_participant_fixture_source")?;
    // THE BUYER'S WALLET HAS TO EXIST, and it did not. The founding gives the
    // participant a collateral token account and never gives the participant's
    // own key a lamport, and an address that has never held one is not an
    // account: `getMultipleAccounts` answers null for it. The shipped admission
    // driver requires the collateral source's OWNER to be observable -- it
    // authenticates the owner rather than taking the token account's word for
    // it -- so the first run that ever reached this stage refused with
    // "snapshot missing required account 2HazeEr9...", which is that wallet
    // (hbox `20260906T104204Z`; the address resolves to the founding evidence's
    // own `localParticipantFixtureLiquidity.sourceOwner`).
    //
    // Funded here rather than in the founding because it is THIS campaign that
    // decided to trade as that participant; a founding that pre-funded every
    // key some later campaign might sign with would be spending on its behalf.
    session.transactions.push(session.rpc.airdrop(
        "journey: fund the buyer's own wallet, whose key the founding never funded",
        participant,
        2_000_000_000,
    )?);
    let second = solana_sdk::signature::Keypair::new();
    let second_key = spine_work.join("second-stranger.json");
    write_keypair_file(&second_key, &second)?;
    session.transactions.push(session.rpc.airdrop(
        "journey: fund the second stranger's rent",
        second.pubkey(),
        2_000_000_000,
    )?);
    // THE ALLOWANCE IS THE TRADE'S, NOT THE FIXTURE'S. This delegated exactly
    // the fixture's whole liquidity, and the first admission that ever landed
    // on a validator then refused its own fill: *its delegated allowance
    // 100000000 is not exactly the 50250000 atoms this trade debits -- the
    // allowance authorizes one trade and is spent to zero, so more is as
    // refused as less* (hbox `20260906T131304Z`). Two acts sized the same
    // quantity independently, which is one author too many; the devnet spine
    // has derived the allowance from the trade's own economics since
    // cohort-16.1 (`tools/cohort/build-sim-config.py`) and this asks the
    // producer that authors those terms rather than restating its arithmetic.
    // The fixture keeps the remainder, which is what a source account is for.
    let buyer_allowance_atoms = crate::direct_trade_producer::required_buyer_collateral_v1(
        &crate::direct_trade_producer::owned_loopback_default_terms_v1(0),
        crate::direct_trade_producer::EXPECTED_PRICE_SCALE_V1,
    )?;
    let strangers = vec![
        spine::StrangerV1 {
            label: "buyer".into(),
            owner: participant,
            keypair: spine_key(&checked.report, "participant")?,
            collateral: Some(spine::StrangerCollateralV1 {
                source_owner: participant,
                source_owner_keypair: spine_key(&checked.report, "participant")?,
                source_account: fixture_source,
                quantity_atoms: buyer_allowance_atoms,
            }),
            report: spine_work.join("admission-buyer.json"),
        },
        spine::StrangerV1 {
            label: "stranger".into(),
            owner: second.pubkey(),
            keypair: second_key,
            collateral: None,
            report: spine_work.join("admission-stranger.json"),
        },
    ];
    // THE ADMISSION ROUTES THROUGH THE FOUNDING'S OWN FROZEN TABLE, and the
    // reason it did not is a measurement read backwards.
    //
    // `local-private-validator-user-position-admission-v1` compiles a v0
    // message and takes `--routing-table`; this tier passed the empty slice and
    // refused `admission message compilation: PacketTooLarge` (hbox
    // `20260906T110501Z`). A frozen table over the FOUNDING'S EVIDENCE ACCOUNT
    // MAP was then published and the admission refused identically (hbox
    // `20260906T112833Z`), which was read as "not a key-count problem". It was
    // a wrong-table result: that map holds about a dozen market coordinates and
    // almost none of this route's own addresses, so the compiler moved a
    // handful of keys against an overrun of roughly two hundred bytes.
    //
    // MEASURED, host, no chain
    // (`dclutch_operator::user_position_admission_v1::the_admission_packet_is_keys_and_a_routing_table_makes_it_fit`):
    // the admission is **1,477 wire bytes over 28 static keys** with 366 bytes
    // of instruction data and two signatures, against a 1,232 packet limit --
    // keys are the overrun and the data is not -- and over the route's OWN
    // canonical lookup addresses it compiles to **798 bytes, 5 static keys, 23
    // loaded**. The devnet cohorts have routed this exact frame through the
    // founding's own frozen DCLTGMF3 table since cohort-16.1 stopped dead
    // without one; `recover_frozen_routing_table_v1` is that recovery for a
    // loopback driver, and it publishes nothing, because the founding already
    // paid for the table it reads.
    //
    // A RECOVERY THAT FAILS COSTS ONE STAGE, NEVER THE WALK. This is an
    // optimization over an admission that would otherwise be compiled bare, and
    // on hbox `20260906T120811Z` the first version of it read the create
    // transaction through a JSON view it could not use and ended the run with a
    // hard error -- after the founding, the ring, and the first Direct
    // capability activation this tier has ever landed, and before the four
    // stages that were the point of the run. The refusal is printed and the
    // admission is compiled bare, which is what every previous run did.
    let admission_routing = match crate::market::recover_frozen_routing_table_v1(
        &mut session.rpc,
        &session.transactions,
        addresses.founding_market,
        payer,
    ) {
        Ok(Some(table)) => {
            eprintln!("journey: the admission routes through the founding's frozen table {table}");
            vec![table]
        }
        Ok(None) => {
            eprintln!(
                "journey: the founding recorded no frozen routing table; the admission is \
                 compiled bare and the driver's sized refusal will say by how much it misses"
            );
            Vec::new()
        }
        Err(error) => {
            eprintln!(
                "journey: the founding's frozen routing table could not be recovered ({error}); \
                 the admission is compiled bare and the walk goes on"
            );
            Vec::new()
        }
    };
    spine::admit_strangers(
        &mut session.rpc,
        &context,
        &mut spine,
        &strangers,
        payer,
        &payer_key,
        &admission_routing,
    )?;
    crate::user_position_admission::parse_finalized_direct_participant_evidence_v1(
        &std::fs::read(&strangers[0].report)?,
        &mut session.rpc,
    )?;
    // Admission creates accounts that the external Direct driver must census.
    // Fold them before publishing the handoff; the later fill will add its own
    // token accounts to the same aperture after a normal, unheld run resumes.
    for entry in spine.aperture.drain(..) {
        match entry.role {
            spine::ApertureRoleV1::Collateral => {
                ledger.track_token_account(&entry.label, entry.address);
            }
            spine::ApertureRoleV1::Position => {
                ledger.track_position(&entry.label, entry.address);
            }
        }
    }
    if let Some(handoff_path) = request.hold_after_participant.as_ref() {
        progress.entering("handoff: accepted participant before Direct fill");
        let bootstrap_bin = request.bootstrap_bin.as_ref().ok_or_else(|| {
            Error::new("--hold-after-participant requires the checked bootstrap binary")
        })?;
        let key_directory = spine::fill_key_directory(&context)?;
        let handoff = ParticipantHandoffV1 {
            schema: "dclutch-private-validator-participant-handoff-v1".into(),
            rpc_url: session.rpc_url.clone(),
            validator_pid: session.validator.pid(),
            supervisor_pid: std::process::id(),
            plan: session.plan_path.display().to_string(),
            market_input: market_path.display().to_string(),
            founding_evidence: campaign_report.display().to_string(),
            participant_evidence: strangers[0].report.display().to_string(),
            key_directory: key_directory.display().to_string(),
            campaign_payer_keypair: payer_key.display().to_string(),
            bootstrap_bin: bootstrap_bin.display().to_string(),
            campaign_public_identities: checked.report.campaign_public_identities.clone(),
            // The Journey ledger began while the retained Core authority still
            // paid the early post-open acts. The held exterior starts at the
            // participant boundary and pays through the campaign payer, which
            // is the signer the simulator and every terminal driver receive.
            // Project that actual payer instead of leaking the earlier ledger
            // observer into an external signing contract.
            census: ledger.handoff_census_for(payer)?,
            resume_action:
                "SIGCONT performs cleanup only after the external lifecycle has finished".into(),
        };
        write_private_json(handoff_path, &handoff)?;
        eprintln!(
            "journey: held after accepted participant and before Direct fill; handoff {} supervisor {}",
            handoff_path.display(),
            std::process::id()
        );
        let status = Command::new("/bin/kill")
            .arg("-STOP")
            .arg(std::process::id().to_string())
            .status()
            .map_err(|error| Error::new(format!("stop journey supervisor: {error}")))?;
        if !status.success() {
            return Err(Error::new(format!(
                "stop journey supervisor exited {status}"
            )));
        }
        return Err(Error::new(
            "the held journey resumed for cleanup; external driver results are separate, and the journey deliberately did not submit its own duplicate Direct fill",
        ));
    }
    spine::fill(&mut session.rpc, &context, &mut spine, &strangers[0].report)?;
    spine::settle_fee(
        &mut session.rpc,
        &context,
        &mut spine,
        &direct_public,
        participant,
        &payer_key,
    )?;
    progress.stages.append(&mut spine.stages);
    session.transactions.append(&mut spine.transactions);
    progress.unexpected_refusals.append(&mut spine.refusals);
    // THE APERTURE FOLLOWS THE ACTS, and it has to: L1 totals over the accounts
    // it names, and every account this stage created was named for the first
    // time by the driver that created it. Folded here, before the census that
    // closes the stage, so the laws are stated over the world the stage left.
    for entry in spine.aperture.drain(..) {
        match entry.role {
            spine::ApertureRoleV1::Collateral => {
                ledger.track_token_account(&entry.label, entry.address);
            }
            spine::ApertureRoleV1::Position => {
                ledger.track_position(&entry.label, entry.address);
            }
        }
    }
    ledger.observe(
        &mut session.rpc,
        "trading: admission, the Direct Hot fill, and the fee settlement",
        0,
        0,
        // The spine's acts move lamports through drivers whose own receipts
        // account for them; this ledger does not restate that arithmetic, and
        // says so rather than declaring a number it did not derive.
        LamportClaimV1::inapplicable(
            "the admission, fill and fee-settlement drivers each write their own lamport receipt; \
             L7 does not restate another author's arithmetic",
        ),
        // COLLATERAL, on the other hand, is exactly what this ledger is for.
        // A fill moves atoms between two token accounts of the same class and
        // opens no vault, so every compartment must move zero -- which is the
        // strong claim, and it fails if one atom reaches a vault.
        ClassClaimV1::unchanged(),
    )?;

    progress.entering(resolution::FUNDING_LADDER_STAGE_V1);
    let (resolution_report, resolution_lamports) = resolution::resolve(
        &mut session.rpc,
        &session.authority,
        &resolution_addresses,
        &mut session.transactions,
    )?;
    progress.stages.push(resolution_report);
    ledger.observe(
        &mut session.rpc,
        resolution::FUNDING_LADDER_STAGE_V1,
        0,
        0,
        resolution_lamports,
        ClassClaimV1::unchanged(),
    )?;

    // The provider legs bootstrap two captured third-party programs and then
    // drive the Market to Terminal. A refusal here is a FINDING, and a finding
    // is worth more written down beside a complete ledger than thrown as an
    // error that discards the rest of the journey -- so the stage is recorded
    // either way and the run fails at the end, after the transcript exists.
    // WHICH END OF THE WINDOW. One label per walk, because the stage is a
    // different fact -- answered, or walked to the failure selector -- and a
    // ledger boundary and a census binding may not have two owners.
    let resolution_stage_label = match request.walk {
        JourneyWalkV1::Honest => provider::PYTH_TRANSPORT_STAGE_V1,
        JourneyWalkV1::Failure => failure::FAILURE_WALK_STAGE_V1,
    };
    progress.entering(resolution_stage_label);
    // WHERE A REFUSING FRAME IS KEPT. A run tears its validator down, so a wall
    // met here is unaskable afterwards unless the frame and the accounts it
    // named are on disk. The captures live beside the transcript.
    let capture_dir = request.work.join("captures");
    // WHAT A REFUSED STAGE DID BEFORE IT REFUSED. The stage appends its
    // evidence to `session.transactions` as it goes, so the work it landed
    // survives its own error -- but the report it used to write on the error
    // path stated `0` transactions and `0` compute units, and the transcript
    // then said run 14's provider stage did nothing while its own transaction
    // list held nine finalized provider transactions including the 134,743-unit
    // Pyth submit. The counters are read off the evidence rather than off the
    // return value, which is the only reading a refusal cannot zero.
    let before_provider = session.transactions.len();
    // WHAT THE WALK COMMITTED, HELD UNTIL CORE HAS SPOKEN. The deadline driver
    // reads the Product's failure cell off the FINALIZED result domain before
    // it signs; Core writes `terminal_winner` after the admission accepts the
    // certificate. Those are two authorities reading one number through two
    // paths, and a campaign that did not compare them would redeem every holder
    // against whichever one happened to be right.
    let mut failure_walk: Option<failure::FailureWalkOutcomeV1> = None;
    let (provider_report, provider_lamports, provider_classes) = match request.walk {
        // NOBODY ANSWERS. Three shipped drivers -- advance, exhaust, commit --
        // walk the market to the Product's own failure selector. A refusal is
        // a FINDING recorded on the stage exactly as the honest arm's is.
        JourneyWalkV1::Failure => {
            let walk_context = failure::FailureWalkContextV1 {
                rpc_url: &session.rpc_url,
                plan: &session.plan_path,
                campaign_report: &campaign_report,
                market: addresses.founding_market,
                work: &request.work,
                worker: payer,
                worker_keypair: &payer_key,
            };
            match failure::walk_to_failure(&walk_context, &mut session.transactions) {
                Ok((report, lamports, document, outcome)) => {
                    progress.spine.insert("failure-walk".into(), document);
                    failure_walk = Some(outcome);
                    (report, lamports, ClassClaimV1::unchanged())
                }
                Err(error) => {
                    progress
                        .unexpected_refusals
                        .push(format!("{} -- {error}", failure::FAILURE_WALK_STAGE_V1));
                    let landed = session.transactions.len().saturating_sub(before_provider);
                    let compute = session
                        .transactions
                        .iter()
                        .skip(before_provider)
                        .filter_map(|evidence| evidence.compute_units_consumed)
                        .sum::<u64>();
                    (
                        StageReportV1 {
                            stage: failure::FAILURE_WALK_STAGE_V1.into(),
                            outcome: "refused".into(),
                            transactions: landed,
                            compute_units: compute,
                            note: format!(
                                "REFUSED, and the refusal is the finding: {error}. {landed} \
                                 transactions had already finalized, for {compute} compute \
                                 units; they are in the transcript's transaction list under \
                                 their own labels."
                            ),
                        },
                        LamportClaimV1::inapplicable(
                            "the walk refused part way through, so what it placed and where is \
                             exactly what is not known; L7 does not guess across a wall",
                        ),
                        ClassClaimV1::inapplicable(
                            "the walk refused part way through, so which compartments it \
                             touched is exactly what is not known; L8 does not guess across a \
                             wall either",
                        ),
                    )
                }
            }
        }
        JourneyWalkV1::Honest => match provider::resolve_through_pyth(
            &mut session.rpc,
            &session.authority,
            &session.plan,
            &resolution_addresses,
            &provider_plan,
            &capture_dir,
            &mut session.transactions,
            // THE PINNED CAPTURE, AND THE PRIMARY LEG. This journey's market
            // sells the window that ENDS at the captured publication instant,
            // so the capture is the publication that answers it, and the
            // market's first choice is the source that answers.
            &provider::PublicationV1::captured(),
            None,
            1,
        ) {
            Ok((report, lamports)) => (report, lamports, ClassClaimV1::unchanged()),
            Err(error) => {
                progress
                    .unexpected_refusals
                    .push(format!("{} -- {error}", provider::PYTH_TRANSPORT_STAGE_V1));
                let landed = session.transactions.len().saturating_sub(before_provider);
                let compute = session
                    .transactions
                    .iter()
                    .skip(before_provider)
                    .filter_map(|evidence| evidence.compute_units_consumed)
                    .sum::<u64>();
                (
                    StageReportV1 {
                        stage: provider::PYTH_TRANSPORT_STAGE_V1.into(),
                        outcome: "refused".into(),
                        transactions: landed,
                        compute_units: compute,
                        note: format!(
                            "REFUSED, and the refusal is the finding: {error}. {landed} \
                             transactions had already finalized, for {compute} compute units; \
                             they are in the transcript's transaction list under their own \
                             labels."
                        ),
                    },
                    LamportClaimV1::inapplicable(
                        "the stage refused part way through, so what it placed and where is \
                         exactly what is not known; L7 does not guess across a wall",
                    ),
                    ClassClaimV1::inapplicable(
                        "the stage refused part way through, so which compartments it touched is \
                         exactly what is not known; L8 does not guess across a wall either",
                    ),
                )
            }
        },
    };
    progress.stages.push(provider_report);
    ledger.observe(
        &mut session.rpc,
        resolution_stage_label,
        0,
        0,
        provider_lamports,
        provider_classes,
    )?;

    // ------------------------------------- the phase byte, the third spine act
    //
    // The devnet spine drives capture, then settle, then admit-terminal, and
    // this tier had only the first two. `resolution::admit_terminal` is the
    // third, as the same builder every shipped driver calls.
    progress.entering(resolution::ADMIT_TERMINAL_STAGE_V1);
    let before_admit = session.transactions.len();
    let (admit_report, admit_lamports, outcome_count) = match resolution::admit_terminal(
        &mut session.rpc,
        &session.authority,
        &resolution_addresses,
        &mut session.transactions,
    ) {
        Ok(pair) => pair,
        Err(error) => {
            progress.unexpected_refusals.push(format!(
                "{} -- {error}",
                resolution::ADMIT_TERMINAL_STAGE_V1
            ));
            let landed = session.transactions.len().saturating_sub(before_admit);
            let compute = session
                .transactions
                .iter()
                .skip(before_admit)
                .filter_map(|evidence| evidence.compute_units_consumed)
                .sum::<u64>();
            (
                StageReportV1 {
                    stage: resolution::ADMIT_TERMINAL_STAGE_V1.into(),
                    outcome: "refused".into(),
                    transactions: landed,
                    compute_units: compute,
                    note: format!("REFUSED, and the refusal is the finding: {error}."),
                },
                LamportClaimV1::inapplicable(
                    "the terminal admission refused, so what it placed and where is exactly what \
                     is not known; L7 does not guess across a wall",
                ),
                None,
            )
        }
    };
    progress.stages.push(admit_report);
    // THE TWO READINGS OF THE FAILURE CELL MUST BE ONE NUMBER.
    //
    // On the failure walk the campaign now holds both: the selector the
    // deadline driver read off the finalized `ResultDomainV2` before it
    // committed, and the `terminal_winner` byte Core wrote when it accepted the
    // certificate. Every redemption after this point pays "one atom to every
    // ordinary claim and nothing to the failure coordinate" against the SECOND
    // one, so a disagreement is not a reporting defect -- it means the holders
    // are about to be paid on a different Product than the one that failed.
    // This is a hard error rather than a recorded refusal for that reason; the
    // transcript is written on the way out either way.
    if let Some(walk) = &failure_walk {
        let admitted = CoreState::decode(
            &session
                .rpc
                .required_account(addresses.founding_market, "Core Market")?
                .data,
        )
        .map_err(|error| Error::new(format!("Core Market: {error:?}")))?;
        if admitted.terminal_winner != walk.failure_selector {
            return Err(Error::new(format!(
                "the deadline walk committed the Product's failure cell {} of {} and the Market \
                 records terminal winner {}: two authorities read one selector and disagreed, \
                 so no holder may be redeemed against either",
                walk.failure_selector, walk.outcome_count, admitted.terminal_winner
            )));
        }
        // AND THE SEAT. `resolution::admit_terminal` already refuses a receipt
        // that is not the certificate IT derived; this asks the same question
        // of the seat the WALK minted, reached through a different driver and a
        // different derivation.
        let receipt = admitted
            .terminal_receipt
            .ok_or_else(|| Error::new("a Terminal Market carries no terminal receipt"))?;
        if receipt.to_bytes() != walk.certificate.to_bytes() {
            return Err(Error::new(format!(
                "the deadline walk minted the ResolutionFailure seat at {} and the Market's \
                 terminal receipt names {}",
                walk.certificate,
                solana_sdk::pubkey::Pubkey::new_from_array(receipt.to_bytes())
            )));
        }
        // THE WALK IS PAID OR NOBODY WALKS IT. Decision 0027 funds the legs out
        // of the market's own prepaid Resolution compartment precisely so that
        // a stranger has a reason to spend a signature on somebody else's
        // outage. A zero bounty is a market whose failure terminal nobody would
        // ever reach.
        if walk.work_paid == 0 {
            return Err(Error::new(
                "the deadline walk paid its worker nothing: the failure terminal is reachable \
                 only because the founding prepaid the walk, and a zero bounty means the funding \
                 ledger did not pay for the leg that was just walked",
            ));
        }
    }
    ledger.observe(
        &mut session.rpc,
        resolution::ADMIT_TERMINAL_STAGE_V1,
        0,
        0,
        admit_lamports,
        // AdmitTerminal writes the Market's phase byte, its terminal receipt and
        // its terminal winner. It moves no collateral at all -- no vault is
        // opened, no atom is transferred -- which is the strong claim, and it
        // fails if one atom reaches a compartment.
        ClassClaimV1::unchanged(),
    )?;

    // ---------------------------------------------- the spine: the redemption
    //
    progress.entering("redemption: every holder of the winning outcome is paid");
    // EVERY HOLDER, AND THE RECIPIENT IS AN ACCOUNT THE HOLDER OWNS.
    //
    // Two corrections the chain made, both on hbox. The recipient was
    // `addresses.founder_wallet`, which is the FOUNDING's `collateral_wallet`
    // and is owned by the founding authority rather than by the
    // `founding-founder` role whose Position the payout debits; the builder
    // refused `WalletTerminalPayoutErrorV3::Custody` and, once that code named
    // its conjunct, said which of its twenty-two it was: *the recipient token
    // account is owned by somebody other than the stated recipient owner*
    // (`20260906T163803Z`). The accounts a holder owns are the Direct token
    // accounts the fill's token setup created, and the fill's own public
    // manifest names them beside their owners.
    //
    // And one payout is not the whole redemption: `BeginRetiring is blocked:
    // Claims supply at index 0 is 166666667; produce and execute wallet
    // terminal payouts first` (`20260906T155320Z`) is the protocol saying the
    // retirement is gated on the winning outcome's whole supply, so both sides
    // of the fill redeem before anything retires.
    let fill_manifest = direct_public.clone();
    let holders: Vec<(
        String,
        String,
        solana_sdk::pubkey::Pubkey,
        solana_sdk::pubkey::Pubkey,
    )> = match std::fs::read(&fill_manifest)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
    {
        Some(document) => {
            let key = |pointer: &str| -> Option<solana_sdk::pubkey::Pubkey> {
                document
                    .pointer(pointer)
                    .and_then(serde_json::Value::as_str)
                    .and_then(|text| text.parse::<solana_sdk::pubkey::Pubkey>().ok())
            };
            [
                (
                    "seller",
                    "founding-founder",
                    "/seller/maker",
                    "/route/custody/sellerToken",
                ),
                (
                    "buyer",
                    "participant",
                    "/buyer/maker",
                    "/route/custody/buyerToken",
                ),
            ]
            .into_iter()
            .filter_map(|(holder, role, owner, token)| {
                Some((holder.to_owned(), role.to_owned(), key(owner)?, key(token)?))
            })
            .collect()
        }
        None => Vec::new(),
    };
    if holders.is_empty() {
        progress.unexpected_refusals.push(
            "redemption: the fill left no public manifest, so no holder's own token account could              be named and nothing was redeemed"
                .to_owned(),
        );
    }
    // EVERY CLAIM INDEX, NOT THE WINNER'S. `BeginRetiring is blocked: Claims
    // supply at index 0 ...` became `... at index 1 ...` the moment both
    // holders had redeemed index 0 (hbox `20260906T170113Z`): the retirement is
    // gated on the aggregate's WHOLE liability record, and a Position carries a
    // claim on every outcome. A losing claim discharges for zero collateral --
    // `terminal_settlement_v3` names positive and zero payouts as two admitted
    // shapes of one route -- so this walks the Product's own outcome count,
    // which the terminal admission read off the finalized Product graph and
    // handed on rather than being re-derived here.
    let claim_indices = outcome_count.unwrap_or(1);
    for (holder, role, owner, recipient) in &holders {
        for claim_index in 0..claim_indices {
            spine::redeem(
                &mut session.rpc,
                &context,
                &mut spine,
                &format!("{holder}-claim-{claim_index}"),
                role,
                *owner,
                *recipient,
                claim_index,
                payer,
                &payer_key,
            )?;
        }
    }
    progress.stages.append(&mut spine.stages);
    session.transactions.append(&mut spine.transactions);
    progress.unexpected_refusals.append(&mut spine.refusals);
    let spine_paid_atoms = spine.paid_atoms;
    // WHAT THE PAYOUTS DECLARED THEY MOVED. Summed off each driver's own
    // `payout` field; L2 then compares the chain's Hoard delta against the
    // drivers' declaration rather than against a zero nobody meant.
    let declared_payout = i128::try_from(spine_paid_atoms)
        .map_err(|_| Error::new("the declared payout total exceeded an i128"))?;
    ledger.observe(
        &mut session.rpc,
        "redemption: every holder of the winning outcome is paid",
        0,
        declared_payout.saturating_neg(),
        LamportClaimV1::inapplicable(
            "the payout driver writes its own lamport receipt; L7 does not restate it",
        ),
        // A payout DEBITS the Hoard, which is the one compartment claim this
        // campaign cannot state as `unchanged`. Declaring it inapplicable would
        // be a lie of omission, so the ledger records the movement and the
        // per-class law reads it from the chain rather than from the driver.
        ClassClaimV1::inapplicable(
            "a terminal payout debits the HoardPrincipal compartment by exactly the atoms it \
             pays, and the amount is the driver's own derivation from the Position's claim \
             vector; L8 records the compartment rather than asserting a number this tier did not \
             derive",
        ),
    )?;

    // Positions are user-owned accounts and aggregate retirement refuses while
    // any remain. Payout empties both Direct parties; the second stranger was
    // admitted with a zero vector and never traded. Close all three through
    // the wallet exterior before the market-level retirement begins.
    for (holder, role, owner, _) in &holders {
        spine::close_direct_position(
            &mut session.rpc,
            &context,
            &mut spine,
            holder,
            role,
            *owner,
            &direct_finalized,
            payer,
            &payer_key,
        )?;
    }
    let stranger = strangers
        .get(1)
        .ok_or_else(|| Error::new("the canonical zero-vector stranger disappeared"))?;
    spine::close_participant_position(
        &mut session.rpc,
        &context,
        &mut spine,
        &stranger.label,
        stranger.owner,
        &stranger.keypair,
        &stranger.report,
        payer,
        &payer_key,
    )?;
    progress.stages.append(&mut spine.stages);
    session.transactions.append(&mut spine.transactions);
    progress.unexpected_refusals.append(&mut spine.refusals);
    ledger.observe(
        &mut session.rpc,
        "redemption: every emptied wallet Position and admission record is closed",
        0,
        0,
        LamportClaimV1::inapplicable(
            "each Position-close driver writes the exact live balances credited to RentCredit; \
             L7 does not restate those three receipts",
        ),
        ClassClaimV1::unchanged(),
    )?;

    // ---------------------------------------------- the spine: the retirement
    //
    // ONE AUTHOR FOR THE TERMINAL SEQUENCE, since 2026-09-06. This tier used to
    // run its own hand-built `BeginRetiring` plus a Source closure here first,
    // and `resolution.rs` records why that stage is deleted rather than kept
    // beside this one: it ran `ResolutionCloseFund` ahead of
    // `DirectCloseCapability`, which is the pair PROGRAMS-18A reversed and the
    // one ordering `TerminalStageV1::ORDERED` forbids. The shipped driver walks
    // the ruled order, and it drives the same corrected V7 close, so the
    // deletion costs no route its author.
    //
    // Retirement runs BEFORE rent recovery, and the order is load-bearing: the
    // Source closure refunds its rent into the Market's own beneficiary credit,
    // so sweeping first would sweep a surplus the retirement is about to add to
    // and leave the larger half sitting there.
    progress.entering(
        "retirement: the checkpointed packets close the Claims aggregate, the vault and the replay",
    );
    // DERIVED, NOT FISHED FOR. This used to read `source_closure_receipt` out
    // of the founding's evidence, fall back to a second label, and then fall
    // back to the founding Market's own address -- which is not a closure
    // receipt at all, so a missing label became a wrong argument rather than a
    // refusal. `ResolutionAddressesV1::closure_receipt` is this campaign's one
    // derivation of that PDA, checked against the operator's own on the run
    // that used to prepay it.
    spine::retire(
        &mut session.rpc,
        &context,
        &mut spine,
        &direct_public,
        &direct_finalized,
        resolution_addresses.closure_receipt,
        payer,
        &payer_key,
    )?;
    progress.stages.append(&mut spine.stages);
    session.transactions.append(&mut spine.transactions);
    progress.unexpected_refusals.append(&mut spine.refusals);
    ledger.observe(
        &mut session.rpc,
        "retirement: the checkpointed packets close the Claims aggregate, the vault and the replay",
        0,
        0,
        LamportClaimV1::inapplicable(
            "the aggregate-retirement driver writes a conservation receipt that classifies every \
             lamport its four packets moved; restating it here would be a second arithmetic",
        ),
        ClassClaimV1::inapplicable(
            "the checkpoint chain CLOSES the HoardPrincipal vault, so the compartment set itself \
             changes across this boundary and `unchanged` is not the honest claim",
        ),
    )?;

    progress.entering("rent recovery");
    let (rent, rent_fees) = stages::recover_rent(
        &mut session.rpc,
        &session.plan,
        &session.accounts,
        &session.authority,
        &mut session.transactions,
    )?;
    progress.stages.push(rent);
    ledger.observe(
        &mut session.rpc,
        "rent recovery",
        0,
        0,
        LamportClaimV1::fees(rent_fees),
        ClassClaimV1::unchanged(),
    )?;

    progress.entering("the campaign's own closing census");
    progress.markets = vec![
        market_phase(
            &mut session.rpc,
            "founding_market",
            addresses.founding_market,
        )?,
        market_phase(&mut session.rpc, "found31_market", addresses.found31_market)?,
    ];
    for gap in &progress.gaps.clone() {
        progress.stages.push(StageReportV1 {
            stage: gap.stage.clone(),
            outcome: "blocked".into(),
            transactions: 0,
            compute_units: 0,
            note: format!("{} Owner: {}.", gap.reason, gap.owner),
        });
    }

    progress.claim_unit_atoms = claim_unit_atoms;
    progress.spine = spine.reports.clone();
    progress.sync(session, ledger);
    Ok(())
}

/// One role key file out of the prepare report, founding roles first.
fn spine_key(
    report: &crate::local_mutable::LocalMutablePrepareReportV1,
    role: &str,
) -> Result<PathBuf> {
    report
        .campaign_founding_keypairs
        .get(role)
        .or_else(|| report.keypairs.get(role))
        .map(PathBuf::from)
        .ok_or_else(|| {
            Error::new(format!(
                "the prepare report names no key file for role `{role}`"
            ))
        })
}

fn context_pubkey(
    context: &spine::SpineContextV1<'_>,
    role: &str,
) -> Result<solana_sdk::pubkey::Pubkey> {
    let path = context
        .founding_keypairs
        .get(role)
        .or_else(|| context.keypairs.get(role))
        .ok_or_else(|| Error::new(format!("the prepare report names no role `{role}`")))?;
    Ok(crate::substrate::load_keypair(Path::new(path))?.pubkey())
}

fn evidence_pubkey(
    accounts: &std::collections::BTreeMap<String, crate::model::AccountEvidence>,
    label: &str,
) -> Result<solana_sdk::pubkey::Pubkey> {
    let evidence = accounts.get(label).ok_or_else(|| {
        Error::new(format!(
            "the founding's evidence names no `{label}` account"
        ))
    })?;
    crate::plan::pubkey(&evidence.address)
}

/// Write one Solana-convention keypair file for a disposable loopback role.
fn write_keypair_file(path: &Path, keypair: &solana_sdk::signature::Keypair) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes: Vec<u8> = keypair.to_bytes().to_vec();
    std::fs::write(path, serde_json::to_vec(&bytes)?)?;
    Ok(())
}

fn market_phase(
    rpc: &mut crate::rpc::Rpc,
    label: &str,
    address: solana_sdk::pubkey::Pubkey,
) -> Result<MarketPhaseV1> {
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
///
/// # TWO OF THIS REGISTER'S REASONS WERE FALSE, and both are deleted here
///
/// A gap register earns its keep only if a lane that reads it can trust it, and
/// this one carried two claims that had stopped being true. They are recorded
/// as corrections rather than quietly dropped, because the next lane's cost of
/// believing a stale entry is exactly what this paragraph is for.
///
/// **"The whole trading half is behind three independent walls, the first of
/// which is a prestate circularity."** The first wall said admitting a second
/// party a Claims Position is itself a Claims mutation and therefore behind the
/// same Hot gate the trade is behind. It is not: `trading/
/// user_position_admission_v1::process_user_position_admission_v1#Admit` is a
/// TOP-LEVEL Trading route a wallet signs for itself, shipped as
/// `local-private-validator-user-position-admission-v1`, and this campaign now
/// calls it twice before it trades. The third wall -- the packet arithmetic
/// that computed the canonical Direct Hot continuation as 36 bytes over the
/// legacy limit -- was answered by measurement, not by argument: a Direct Hot
/// fill landed on a loopback validator on 2026-08-31 at 1,282,624 CU
/// (`docs/evidence/FIRST_LOCAL_DIRECT_FILL_2026_08_31.md`), riding a frozen
/// address lookup table as a v0 packet, which is the shape the arithmetic did
/// not consider. The second wall -- that the canonical artifact family is
/// derived for ONE market geometry -- is the one that survived, and it survived
/// as a fact about `direct-hot/`'s fixture family rather than about this tier,
/// which compiles its Direct capability against the market it founds.
///
/// **"Redemption is a SignedDeltaV3-framed Claims route and carries the same
/// signing-CallerAuthority requirement as every other Claims mutation, so it is
/// also behind the Hot gate."** False since terminal settlement became
/// family-neutral: `crates/dclutch-claims/src/terminal_settlement_v3.rs` is
/// "the sole wire authority between an authenticated orchestration caller and
/// Claims for terminal settlement", the owner signs for their own Position and
/// a fee payer signs the packet, and the shipped
/// `local-private-validator-wallet-terminal-payout-v1` is a wallet-signed
/// top-level act. This campaign drives it.
///
/// The retirement entry is gone for the plainest reason: the campaign runs it.
/// `local-private-validator-terminal-sequence-v1` walks CloseFund and
/// BeginRetiring and the replay handoff, and
/// `local-private-validator-aggregate-retirement-v1` drives the four
/// checkpointed packets. What each of them MET on a live chain is in the
/// transcript's stage rows, which is where a measurement belongs; a wall this
/// campaign hit at run time is not a gap register entry, it is a refusal with a
/// signature next to it.
fn gap_register() -> Vec<GapV1> {
    vec![
        GapV1 {
            stage: "post-open life: outcome-token distribution and holder-to-holder transfers"
                .into(),
            routes: vec![
                "claims/protocol_position_v2::process".into(),
                "claims/sparse_native_transfer_v1::process".into(),
            ],
            owner: "W2i (Trading Hot gate)".into(),
            reason: "NARROWED, and the narrowing is the point. `sparse_native_transfer_v1` -- one \
                     holder handing outcome tokens to another after Open -- puts a CallerAuthority \
                     at index 0 that must be BOTH a signer and the CallerAuthoritySeedsV1 PDA \
                     under the calling program, and re-authenticates that program against the \
                     Registry activation cache as the Trading role. Only a program can sign its \
                     own PDA, so the sole admissible caller is the deployed Trading program, and \
                     Trading's outer dispatch routes everything that is not DCLTGMF1, DCLTPCB1, \
                     DCLTPCA1 or the capability seal into hot_v3. That much is unchanged. What is \
                     NOT behind that gate, and used to be listed here as though it were: admitting \
                     a Position (a top-level Trading route this campaign now drives twice) and \
                     terminal settlement (a wallet-signed top-level Claims route this campaign now \
                     drives). So the remaining gap is the SECONDARY MARKET -- holder to holder in \
                     outcome tokens between Open and Terminal -- and not post-Open Claims life as \
                     a whole."
                .into(),
        },
        GapV1 {
            stage: "post-open life: a Custody vault cycle per holder".into(),
            routes: vec![
                "custody/process_instruction#OpenVault".into(),
                "custody/process_instruction#Transfer".into(),
                "custody/process_instruction#CloseVault".into(),
            ],
            owner: "W2i (Trading Hot gate)".into(),
            reason:
                "Custody's nine-account common prefix has the same shape: index 0 is a signing \
                     CallerAuthority PDA and index 4 is the caller program, re-authenticated \
                     against the activation cache. A holder cannot open, deposit to, or withdraw \
                     from a vault directly; the operation has to arrive by CPI from an activated \
                     role. This campaign reaches Custody through the fill and through the \
                     retirement's close-vault packet, both of which are CPI from an activated \
                     role, and never as a holder's own vault cycle."
                    .into(),
        },
        GapV1 {
            stage: "trading: replay pressure and concurrent submission".into(),
            routes: vec!["direct/successor::MakerReplayRootV1 nonce advance".into()],
            owner: "this tier, the day after its first fill lands".into(),
            reason: "Both probes are written to run and neither is driven, and they are stated \
                     here so the day they run is not the day they are designed. REPLAY: resubmit \
                     the byte-identical bundle and require SuccessorError::NonceMismatch, surfaced \
                     as TradingSbfError::Transition. That refusal has no on-chain test anywhere in \
                     the tree -- the nearest thing submits the same bundle twice inside a mutation \
                     test whose assertion is about a corrupted byte, so a replay refusal and a \
                     corruption refusal are indistinguishable in it. CONCURRENCY: two holders \
                     submitting fills against the same maker in one slot; exactly one must commit \
                     and the other must refuse on the nonce rather than both committing or both \
                     refusing. The shipped Direct trade driver advances ONE durable action per \
                     invocation against a journal, which is the right shape for a resumable host \
                     and the wrong one for a concurrency probe: the probe needs two packets in \
                     flight, which means a second signer and a deliberate race, and neither is \
                     something a resumption loop can express."
                .into(),
        },
        GapV1 {
            stage: "trading: the canonical Direct artifact family is derived for ONE market \
                    geometry"
                .into(),
            routes: vec!["direct/ordinary_bundle_v4 (InlineOrdinary)".into()],
            owner: "BUNDLE (the artifact-derived chain-fixture builder)".into(),
            reason: "The one wall of the old three-wall entry that survived measurement. \
                     `direct-hot/src/lib.rs` builds its AccountProfile widths from a THREE-claim \
                     aggregate (coordinate 13 = header + 3 rows; coordinates 32/33 = Position \
                     header + 3 rows) and a THREE-cut result domain (coordinate 18). This \
                     campaign's Market is the four-outcome, two-cut lab default, so the SHIPPED \
                     fixture identities do not describe it and the family has to be regenerated \
                     per market shape. This tier is not blocked by that -- it compiles its Direct \
                     capability against the market it founds, through \
                     `DirectMarketCompilerOwnedV1::load_local` -- but any campaign that took the \
                     shipped artifacts as given would be bending the MARKET to the fixture, which \
                     is the fixture-is-never-the-authority failure in its purest form. Kept here \
                     so nobody re-derives it as a discovery."
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

fn write_private_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    validate_new_path(path, "--hold-after-participant")?;
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|error| Error::new(format!("create {}: {error}", path.display())))?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    if let Some(parent) = path.parent() {
        std::fs::File::open(parent)?.sync_all()?;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrated_job_root_is_canonicalized_but_a_file_symlink_is_refused() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "dclutch-held-path-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let real = root.join("real");
        let alias = root.join("alias");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&real).expect("real migrated job root");
        std::fs::write(real.join("handoff.json"), b"{}\n").expect("regular handoff");
        symlink(&real, &alias).expect("old job-root alias");

        let resolved = canonical_existing_file(&alias.join("handoff.json"), "handoff")
            .expect("an ancestor alias resolves to the checksummed migrated tree");
        assert_eq!(resolved, real.join("handoff.json"));

        symlink(real.join("handoff.json"), real.join("file-link.json")).expect("hostile leaf link");
        let error = canonical_existing_file(&real.join("file-link.json"), "handoff")
            .expect_err("the evidence file itself must not be a symlink");
        assert!(error.0.contains("file itself may not be a symlink"));
        let _ = std::fs::remove_dir_all(root);
    }

    /// A HELPER THAT HARD-ERRORS STILL LEAVES A TRANSCRIPT NAMING IT.
    ///
    /// This is the property three hbox runs did not have. Each ran thirty-five
    /// minutes, landed around two hundred transactions, hit one `?` and left a
    /// single line of stderr: the campaign's own document was written only on
    /// the paths that reached the end, so the evidence for every wall was
    /// thrown away with the run that found it.
    ///
    /// The wall this test builds is the FIRST stage, and it is built by naming
    /// a checked release gate that does not exist -- so it needs no validator,
    /// no artifacts and no network, and it exercises the same `execute` the
    /// runner calls. What it asserts is the shape a reader needs: the stage the
    /// campaign was inside, the helper's own sentence rather than a summary of
    /// it, and an explicit statement that there was no chain to read rather
    /// than an empty list that could equally mean "read it and found nothing".
    #[test]
    fn a_stage_that_hard_errors_still_writes_a_transcript_naming_it() {
        let work = std::env::temp_dir().join(format!(
            "dclutch-journey-wall-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&work);
        std::fs::create_dir_all(&work).expect("a scratch work directory");
        let transcript = work.join("transcript.json");
        let error = execute(JourneyRequestV1 {
            walk: JourneyWalkV1::Honest,
            transcript: transcript.clone(),
            work: work.clone(),
            rpc_port: 0,
            checked_release_gate: work.join("no-such-checked-release-gate.json"),
            expected_gate_sha256: "0".repeat(64),
            expected_source_revision: "0".repeat(40),
            expected_source_tree_sha256: "0".repeat(64),
            seed: "journey-wall".into(),
            holder_count: DEFAULT_HOLDER_COUNT,
            hold_after_participant: None,
            bootstrap_bin: None,
        })
        .expect_err("a checked release gate that does not exist must stop the campaign");

        assert!(
            transcript.is_file(),
            "the transcript must exist even though the first stage refused"
        );
        let document: Value =
            serde_json::from_slice(&std::fs::read(&transcript).expect("read the transcript back"))
                .expect("the transcript is JSON");
        let wall = document
            .get("wall")
            .and_then(Value::as_object)
            .expect("the transcript names the wall it met");
        assert_eq!(
            wall.get("stage").and_then(Value::as_str),
            Some("checked-mutable substrate"),
            "the wall names the stage the campaign had entered"
        );
        let sentence = wall
            .get("sentence")
            .and_then(Value::as_str)
            .expect("the wall quotes the helper");
        assert!(
            !sentence.is_empty(),
            "the wall carries the helper's own sentence"
        );
        assert!(
            wall.get("chain")
                .and_then(Value::as_array)
                .expect("a chain list")
                .is_empty(),
            "there was no validator to read, and an empty list says so with `chain_note`"
        );
        assert!(
            wall.get("chain_note")
                .and_then(Value::as_str)
                .is_some_and(|note| !note.is_empty()),
            "an empty chain list must say WHY it is empty"
        );
        assert!(
            document
                .get("unexpected_refusals")
                .and_then(Value::as_array)
                .is_some_and(|refusals| refusals.len() == 1),
            "the stopped stage is also an unexpected refusal"
        );
        assert!(
            document
                .get("stages")
                .and_then(Value::as_array)
                .is_some_and(|stages| stages
                    .iter()
                    .any(|stage| stage.get("outcome").and_then(Value::as_str) == Some("refused"))),
            "the stage register carries the refusal"
        );
        assert!(
            error.to_string().contains("checked-mutable substrate")
                && error
                    .to_string()
                    .contains(&transcript.display().to_string()),
            "the run still FAILS, and its failure points at the transcript: {error}"
        );
        // The runner's own comment promises BOTH documents on a wall, and then
        // dies on a missing evidence.json. A run that landed nothing says so
        // with an empty pair rather than with an absent file.
        let evidence: Value = serde_json::from_slice(
            &std::fs::read(work.join("evidence.json")).expect("the run-evidence document"),
        )
        .expect("the run-evidence document is JSON");
        assert_eq!(
            evidence.get("schema").and_then(Value::as_str),
            Some("dclutch-local-successor-run-evidence-v2")
        );
        assert!(
            evidence
                .get("transactions")
                .and_then(Value::as_array)
                .is_some_and(|rows| rows.is_empty()),
            "a wall before the substrate landed no transactions, and says so"
        );
        assert_eq!(
            document.get("evidence").and_then(Value::as_str),
            Some(work.join("evidence.json").display().to_string().as_str()),
            "the transcript names the evidence document it wrote"
        );
        let _ = std::fs::remove_dir_all(&work);
    }
}
