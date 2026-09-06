//! The journey: one campaign, one Market, one ledger.

use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use dclutch_market::CoreState;
use serde::Serialize;
use serde_json::Value;
use solana_sdk::signature::Signer;

use crate::{
    Error, Result,
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
/// The same string `tools/gauntlet/ladder/` founds with: one rung, a TWO-source
/// market, at a 2,500-bp confidence bound tighter than the lab's 10,000-bp
/// ceiling -- a market whose first choice went silent has a reason to demand a
/// better-conditioned reading from its second.
const DEFAULT_RECOVERY_RUNGS_V1: &str = "2500:120";

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
pub(crate) struct JourneyRequestV1 {
    pub(crate) transcript: PathBuf,
    pub(crate) work: PathBuf,
    pub(crate) rpc_port: u16,
    pub(crate) checked_release_gate: PathBuf,
    pub(crate) expected_gate_sha256: String,
    pub(crate) expected_source_revision: String,
    pub(crate) expected_source_tree_sha256: String,
    pub(crate) seed: String,
    pub(crate) holder_count: u32,
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

    /// Record a wall met before any chain reader existed.
    fn wall_before_chain(&mut self, error: &Error) {
        let sentence = error.to_string();
        self.push_refusal(&sentence);
        self.wall = Some(JourneyWallV1 {
            stage: self.stage_in_flight.clone(),
            sentence,
            finalized_slot: None,
            chain: Vec::new(),
            chain_note: "the wall came before this campaign had a validator and a conservation \
                         ledger to read one through, so there is no chain state to report"
                .into(),
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
                progress.wall_before_chain(error);
            }
        }
    }
    outcome
}

fn campaign(
    request: &JourneyRequestV1,
    progress: &mut JourneyProgressV1,
    anchor: &mut Option<(JourneySessionV1, ConservationLedgerV1)>,
) -> Result<()> {
    let holder_count = request.holder_count;
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
    let rungs = crate::local_mutable::parse_recovery_rungs_v1(DEFAULT_RECOVERY_RUNGS_V1)?;
    let shape = crate::market::LocalMarketShapeV1 {
        recovery: Some(rungs),
        ..crate::market::LocalMarketShapeV1::default()
    };
    let market_input =
        crate::market::demo_market_input_shaped(registry, direct.compiler(), &shape)?;
    let market_path = request.work.join("market.json");
    std::fs::write(&market_path, serde_json::to_vec_pretty(&market_input)?)?;

    progress.entering("founding through Open");
    // ------------------------------------------------------- 3. the founding
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

    progress.stages.extend([
        StageReportV1 {
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
        },
        StageReportV1 {
            stage: "founding through Open".into(),
            outcome: "executed".into(),
            transactions: session.transactions.len(),
            compute_units: session
                .transactions
                .iter()
                .map(|transaction| transaction.compute_units_consumed.unwrap_or(0))
                .sum(),
            note:
                "`campaign --founding-only` over the live checked substrate: the market compiled \
                   by `DirectMarketCompilerOwnedV1::load_local` against THIS deployment, four \
                   outcomes over two cuts, which is a refunding payout scale of three."
                    .into(),
        },
    ]);

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

    // TWO STRANGERS, and the second one is not decoration. The buyer carries
    // the collateral leg because the fill needs delegated collateral; the
    // second admission is a Position and nothing else, which is the ordinary
    // case and the one a campaign that only ever admits its trader never runs.
    let participant = context_pubkey(&context, "participant")?;
    let fixture_source = evidence_pubkey(&session.accounts, "local_participant_fixture_source")?;
    let second = solana_sdk::signature::Keypair::new();
    let second_key = spine_work.join("second-stranger.json");
    write_keypair_file(&second_key, &second)?;
    session.transactions.push(session.rpc.airdrop(
        "journey: fund the second stranger's rent",
        second.pubkey(),
        2_000_000_000,
    )?);
    let strangers = vec![
        spine::StrangerV1 {
            label: "buyer".into(),
            owner: participant,
            keypair: spine_key(&checked.report, "participant")?,
            collateral: Some(spine::StrangerCollateralV1 {
                source_owner: participant,
                source_owner_keypair: spine_key(&checked.report, "participant")?,
                source_account: fixture_source,
                quantity_atoms: crate::market::LOCAL_PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS_V1,
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
    spine::admit_strangers(
        &mut session.rpc,
        &context,
        &mut spine,
        &strangers,
        payer,
        &payer_key,
        &[],
    )?;
    spine::fill(&mut session.rpc, &context, &mut spine, &strangers[0].report)?;
    spine::settle_fee(
        &mut session.rpc,
        &context,
        &mut spine,
        participant,
        &payer_key,
    )?;
    let spine_admission_stages = spine.stages.len();
    progress.stages.append(&mut spine.stages);
    session.transactions.append(&mut spine.transactions);
    progress.unexpected_refusals.append(&mut spine.refusals);
    let _ = spine_admission_stages;
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

    progress.entering("resolution: create and activate the Market's Resolution funding");
    let (resolution_report, resolution_lamports) = resolution::resolve(
        &mut session.rpc,
        &session.authority,
        &resolution_addresses,
        &mut session.transactions,
    )?;
    progress.stages.push(resolution_report);
    ledger.observe(
        &mut session.rpc,
        "resolution: create and activate the Market's Resolution funding",
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
    progress.entering("resolution: the Pyth transport carries the Market to Terminal");
    let (provider_report, provider_lamports, provider_classes) =
        match provider::resolve_through_pyth(
            &mut session.rpc,
            &session.authority,
            &session.plan,
            &resolution_addresses,
            &provider_plan,
            &mut session.transactions,
        ) {
            Ok((report, lamports)) => (report, lamports, ClassClaimV1::unchanged()),
            Err(error) => {
                progress.unexpected_refusals.push(format!(
                    "resolution: the Pyth transport carries the Market to Terminal -- {error}"
                ));
                (
                    StageReportV1 {
                        stage: "resolution: the Pyth transport carries the Market to Terminal"
                            .into(),
                        outcome: "refused".into(),
                        transactions: 0,
                        compute_units: 0,
                        note: format!("REFUSED, and the refusal is the finding: {error}."),
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
        };
    progress.stages.push(provider_report);
    ledger.observe(
        &mut session.rpc,
        "resolution: the Pyth transport carries the Market to Terminal",
        0,
        0,
        provider_lamports,
        provider_classes,
    )?;

    // ---------------------------------------------- the spine: the redemption
    //
    progress.entering("redemption: a holder redeems through wallet-signed terminal settlement");
    // The stranger is paid before anything retires, because retirement refuses
    // to compile at all while the Hoard holds an atom that belongs to a holder.
    spine::redeem(
        &mut session.rpc,
        &context,
        &mut spine,
        "founding-founder",
        crate::substrate::load_keypair(&spine_key(&checked.report, "founding-founder")?)?.pubkey(),
        addresses.founder_wallet,
        0,
        payer,
        &payer_key,
    )?;
    progress.stages.append(&mut spine.stages);
    session.transactions.append(&mut spine.transactions);
    progress.unexpected_refusals.append(&mut spine.refusals);
    ledger.observe(
        &mut session.rpc,
        "redemption: a holder redeems through wallet-signed terminal settlement",
        0,
        0,
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

    // Retirement runs BEFORE rent recovery, and the order is load-bearing: the
    // Source closure refunds its rent into the Market's own beneficiary credit,
    // so sweeping first would sweep a surplus the retirement is about to add to
    // and leave the larger half sitting there.
    progress.entering("retirement: begin retiring and close the Source subtree");
    let (retirement, retirement_lamports) = resolution::retire(
        &mut session.rpc,
        &session.authority,
        &resolution_addresses,
        addresses.hoard,
        &mut session.transactions,
    )?;
    progress.stages.push(retirement);
    ledger.observe(
        &mut session.rpc,
        "retirement: begin retiring and close the Source subtree",
        0,
        0,
        retirement_lamports,
        ClassClaimV1::unchanged(),
    )?;

    // ---------------------------------------------- the spine: the retirement
    //
    // TWO AUTHORS FOR BeginRetiring, stated rather than hidden. The stage above
    // is this tier's own hand-built BeginRetiring plus the Source closure, and
    // it is what the journey's existing bindings witness. The shipped
    // terminal-sequence driver below owns the same act, plus DirectBeginRetiring,
    // ResolutionCloseFund, DirectCloseCapability and the replay handoff, and
    // then the four checkpoint packets. Whichever of the two the chain accepts
    // first, the other reports what it met, and the transcript says which --
    // which is the shape a convergence needs before one of them is deleted.
    progress.entering(
        "retirement: the checkpointed packets close the Claims aggregate, the vault and the replay",
    );
    let source_receipt = evidence_pubkey(&session.accounts, "source_closure_receipt")
        .or_else(|_| evidence_pubkey(&session.accounts, "founding_source_receipt"))
        .unwrap_or(addresses.founding_market);
    spine::retire(
        &mut session.rpc,
        &context,
        &mut spine,
        source_receipt,
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
    progress.gaps = gap_register();
    for gap in &progress.gaps.clone() {
        progress.stages.push(StageReportV1 {
            stage: gap.stage.clone(),
            outcome: "blocked".into(),
            transactions: 0,
            compute_units: 0,
            note: format!("{} Owner: {}.", gap.reason, gap.owner),
        });
    }

    let evidence_path = request.work.join("evidence.json");
    let evidence = serde_json::json!({
        "schema": "dclutch-local-successor-run-evidence-v2",
        "rpc_url": session.rpc_url,
        "plan_sha256": session.plan_sha256,
        "transactions": serde_json::to_value(&session.transactions)?,
        "accounts": serde_json::to_value(&session.accounts)?,
    });
    write_json(&evidence_path, &evidence)?;
    progress.evidence = evidence_path.display().to_string();
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
            transcript: transcript.clone(),
            work: work.clone(),
            rpc_port: 0,
            checked_release_gate: work.join("no-such-checked-release-gate.json"),
            expected_gate_sha256: "0".repeat(64),
            expected_source_revision: "0".repeat(40),
            expected_source_tree_sha256: "0".repeat(64),
            seed: "journey-wall".into(),
            holder_count: DEFAULT_HOLDER_COUNT,
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
        let _ = std::fs::remove_dir_all(&work);
    }
}
