//! The journey: one campaign, one Market, one ledger.

use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use dclutch_market_core_codec::CoreState;
use serde::Serialize;
use solana_sdk::signature::Signer;

use crate::{
    Error, Result,
    ledger::{ConservationLedgerV1, LamportClaimV1, ObservationV1},
    provider, resolution,
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
    pub(crate) observations: Vec<ObservationV1>,
    pub(crate) transactions_total: usize,
    pub(crate) compute_units_total: u64,
}

/// Live one Market's whole life, and account for every atom while doing it.
pub(crate) fn execute(
    spec_path: &Path,
    transcript_path: &Path,
    holder_count: u32,
    keypair_seed: Option<&str>,
) -> Result<JourneyTranscriptV1> {
    validate_new_path(transcript_path, "--transcript")?;
    let mut session = crate::runtime::found_through_open(spec_path, keypair_seed)?;
    let addresses = MarketAddressesV1::from_evidence(&session.accounts)?;

    let mut ledger = ConservationLedgerV1::new(addresses.mint, session.authority.pubkey());
    let (claim_unit_atoms, decimals) = stages::admit_open_market(
        &mut session.rpc,
        &addresses,
        &session.accounts,
        crate::plan::pubkey(&session.plan.custody.program_id)?,
        crate::plan::pubkey(&session.plan.rent_credit.program_id)?,
        &mut ledger,
    )?;
    // The whole cast is registered with the ledger BEFORE the first census, so
    // that every account the journey later creates is first seen as a checked
    // vacancy rather than as a balance with no predecessor. That ordering is
    // what makes L7 applicable across the stages that spend the most lamports;
    // see `stages::plan_holders` and `resolution::watch`.
    let mut holders = stages::plan_holders(holder_count, &mut ledger)?;
    let resolution_addresses = resolution::derive(
        &mut session.rpc,
        &session.plan,
        &addresses,
        &session.accounts,
    )?;
    resolution::watch(&mut ledger, &resolution_addresses);
    let provider_plan = provider::ProviderPlanV1::derive(&mut session.rpc, &session.plan)?;
    provider::watch(&mut ledger, &provider_plan);

    ledger.observe(
        &mut session.rpc,
        "founding through Open",
        0,
        0,
        // The founding's lamport placements are the tier-1 producer's, and this
        // ledger does not restate them: it would be re-deriving another
        // campaign's arithmetic and calling the agreement evidence. L7 begins
        // at the first journey-owned boundary.
        LamportClaimV1::inapplicable(
            "the founding's lamport movements belong to the tier-1 producer and are covered by \
             tier 1's own witnesses; this ledger accounts for lamports from the first \
             journey-owned boundary onward",
        ),
    )?;

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

    let (distribution, distribution_fees) = stages::distribute_collateral(
        &mut session.rpc,
        &addresses,
        &session.authority,
        decimals,
        &mut holders,
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
        0,
        LamportClaimV1::fees(distribution_fees),
    )?;

    let (ring, ring_fees) = stages::holder_to_holder(
        &mut session.rpc,
        &addresses,
        &session.authority,
        decimals,
        &holders,
        &mut session.transactions,
    )?;
    stages.push(ring);
    ledger.observe(
        &mut session.rpc,
        "post-open life: holder-to-holder collateral",
        0,
        0,
        LamportClaimV1::fees(ring_fees),
    )?;

    let mut unexpected_refusals = Vec::new();

    let (resolution_report, resolution_fees) = resolution::resolve(
        &mut session.rpc,
        &session.authority,
        &resolution_addresses,
        &mut session.transactions,
    )?;
    stages.push(resolution_report);
    // The funding ladder moves lamports and no collateral at all: the Source
    // state and three Funds are lamport compartments, and the Hoard is not a
    // party to any of it. Declaring zero on both atom laws is the strong claim.
    ledger.observe(
        &mut session.rpc,
        "resolution: create and activate the Market's Resolution funding",
        0,
        0,
        LamportClaimV1::fees(resolution_fees),
    )?;

    // The provider legs bootstrap two captured third-party programs and then
    // drive the Market to Terminal. A refusal here is a FINDING, and a finding
    // is worth more written down beside a complete ledger than thrown as an
    // error that discards the rest of the journey -- so the stage is recorded
    // either way and the run fails at the end, after the transcript exists.
    let (provider_report, provider_fees) = match provider::resolve_through_pyth(
        &mut session.rpc,
        &session.authority,
        &session.plan,
        &resolution_addresses,
        &provider_plan,
        &mut session.transactions,
    ) {
        Ok(value) => value,
        Err(error) => {
            unexpected_refusals.push(format!(
                "resolution: the Pyth transport carries the Market to Terminal -- {error}"
            ));
            (
                StageReportV1 {
                    stage: "resolution: the Pyth transport carries the Market to Terminal".into(),
                    outcome: "refused".into(),
                    transactions: 0,
                    compute_units: 0,
                    note: format!(
                        "REFUSED, and the refusal is the finding: {error}. Everything the legs \
                         need is on this chain -- the receiver and router ELFs are loaded by the \
                         launcher, the Pyth release record is published by the infrastructure \
                         plan, and the Market's own source spec, window spec, statistic spec, \
                         provider release and adapter configuration are finalized records at the \
                         identities its SourceMaterialV2 names."
                    ),
                },
                0,
            )
        }
    };
    stages.push(provider_report);
    // Resolution moves no collateral either: a terminal certificate is an
    // assertion about which outcome won, not a transfer. The Hoard must not
    // move by one atom while the Market becomes resolvable-against.
    ledger.observe(
        &mut session.rpc,
        "resolution: the Pyth transport carries the Market to Terminal",
        0,
        0,
        LamportClaimV1::fees(provider_fees),
    )?;

    // Retirement runs BEFORE rent recovery, and the order is load-bearing: the
    // Source closure refunds its rent into the Market's own beneficiary credit,
    // so sweeping first would sweep a surplus the retirement is about to add to
    // and leave the larger half sitting there.
    let (retirement, retirement_fees) = resolution::retire(
        &mut session.rpc,
        &session.authority,
        &resolution_addresses,
        addresses.hoard,
        &mut session.transactions,
    )?;
    stages.push(retirement);
    ledger.observe(
        &mut session.rpc,
        "retirement: begin retiring and close the Source subtree",
        0,
        0,
        LamportClaimV1::fees(retirement_fees),
    )?;

    let (rent, rent_fees) = stages::recover_rent(
        &mut session.rpc,
        &session.plan,
        &session.accounts,
        &session.authority,
        &mut session.transactions,
    )?;
    stages.push(rent);
    ledger.observe(
        &mut session.rpc,
        "rent recovery",
        0,
        0,
        LamportClaimV1::fees(rent_fees),
    )?;

    let markets = vec![
        market_phase(
            &mut session.rpc,
            "founding_market",
            addresses.founding_market,
        )?,
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
        deterministic_keypairs: keypair_seed.is_some(),
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
        unexpected_refusals: unexpected_refusals.clone(),
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
    if !unexpected_refusals.is_empty() {
        return Err(Error::new(format!(
            "{} stage(s) that were supposed to execute refused; the transcript and the complete \
             conservation ledger are at {}:\n  {}",
            unexpected_refusals.len(),
            transcript_path.display(),
            unexpected_refusals.join("\n  ")
        )));
    }
    Ok(transcript)
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
/// Two of JRNY-1's five entries are gone from this list, because the campaign
/// now executes them: the resolution FUNDING ladder is a stage, and the rent
/// SWEEP always was. What is left is the trading half, which is behind three
/// independent walls rather than the one the Hot gate looked like, and the
/// terminal half, which is behind one.
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
                     caller is the deployed Trading program -- and Trading\'s outer dispatch routes \
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
            reason: "Custody\'s nine-account common prefix has the same shape: index 0 is a signing \
                     CallerAuthority PDA and index 4 is the caller program, re-authenticated \
                     against the activation cache. A holder cannot open, deposit to, or withdraw \
                     from a vault directly; the operation has to arrive by CPI from an activated \
                     role, which post-Open means Trading Hot."
                .into(),
        },
        GapV1 {
            stage: "trading: N holders fill through the real Registry Hot continuation".into(),
            routes: vec![
                "registry/hot_continuation_v2::process".into(),
                "trading/hot_v3::process_hot_execution_v3".into(),
                "direct/ordinary_bundle_v4 (InlineOrdinary)".into(),
            ],
            owner: "BUNDLE (pattern 1, the artifact-derived chain-fixture builder); the packet \
                    arithmetic below is a Registry/Direct wire decision"
                .into(),
            reason: "The Hot DOOR is open -- `registry_hot_continuation` is 15/15 at the real \
                     32,768-byte heap under a 1,400,000 CU ceiling -- and this campaign still \
                     cannot walk through it, for three independent reasons that are worth keeping \
                     separate because they have different owners and different fixes.\n\n\
                     (1) PRESTATE. The gate proves the bundle against a ProgramTest bank into \
                     which the whole Direct artifact family is PLANTED: nine finalized Registry \
                     record pairs (manifest, ProgramSetV2, CapabilityProgramV4 descriptor, \
                     DirectExecutionConfigV1, AccountProfile, RequestProfile, Transition, Effect, \
                     LifecyclePolicy), the capability root, the capability seal, a Claims Position \
                     per party and two maker replay roots. The founding publishes NONE of them. \
                     Six of those planting sites do have real routes (Registry Begin/Append/\
                     Finalize for every record, and the seal outer, which the gate itself proves \
                     executes) -- but the Claims Positions do not: admitting a second party a \
                     Position is itself a Claims mutation, so the trading prestate is behind the \
                     Hot gate that the trade is behind. That circularity is the finding, and it is \
                     what pattern 1 has to break.\n\n\
                     (2) SHAPE. The canonical family is not protocol-wide; it is derived for ONE \
                     market geometry. `direct-hot/src/lib.rs` builds its AccountProfile widths from \
                     a THREE-claim aggregate (coordinate 13 = header + 3 rows; coordinates 32/33 = \
                     Position header + 3 rows) and a THREE-cut result domain (coordinate 18). This \
                     campaign\'s Market is the SOL/USD range product: four outcomes, two cuts. So \
                     even with the prestate published, the shipped artifact identities do not \
                     describe this Market, and the family has to be regenerated per market shape. \
                     A builder that takes the artifacts as given would bend the MARKET to the \
                     fixture, which is the fixture-is-never-the-authority failure in its purest \
                     form.\n\n\
                     (3) PACKET. Computed, not measured, and it needs measuring: the harness pins \
                     its canonical two-instruction wire at exactly 1,228 bytes with FOUR bytes of \
                     margin under the 1,232-byte limit, and it reaches its 1,400,000 CU ceiling \
                     through ProgramTest\'s `set_compute_max_units` bank override. A validator has \
                     no such override: the default limit is 200,000 per instruction, so a real \
                     submission must carry `SetComputeUnitLimit` itself. That instruction adds the \
                     ComputeBudget program id to the static key list (a program id can never be \
                     resolved through an address lookup table) plus its own compiled instruction: \
                     32 + 8 = 40 bytes, for 1,268 -- 36 bytes OVER the packet limit. If that \
                     arithmetic holds, the canonical Direct Hot continuation is provable in \
                     ProgramTest and unsubmittable on a chain, and the fix is a wire decision at \
                     its owner, not a campaign trick."
                .into(),
        },
        GapV1 {
            stage: "trading: replay pressure and concurrent submission".into(),
            routes: vec!["direct/successor::MakerReplayRootV1 nonce advance".into()],
            owner: "this tier, the day a fill executes".into(),
            reason: "Both probes are written to run the moment a fill lands, and both are stated \
                     here so the day they run is not the day they are designed. REPLAY: resubmit \
                     the byte-identical bundle and require SuccessorError::NonceMismatch, surfaced \
                     as TradingSbfError::Transition. That refusal has no on-chain test anywhere in \
                     the tree today -- the nearest thing submits the same bundle twice inside a \
                     mutation test whose assertion is about a corrupted byte, so a replay refusal \
                     and a corruption refusal are indistinguishable in it. CONCURRENCY: two \
                     holders submitting fills against the same maker in one slot; exactly one must \
                     commit and the other must refuse on the nonce rather than both committing or \
                     both refusing. Neither is constructible until (3) above is answered."
                .into(),
        },
        GapV1 {
            stage: "redemption: winners redeem through terminal settlement".into(),
            routes: vec!["claims/terminal_settlement_v3::process".into()],
            owner: "the resolution gap above, then the Hot gate".into(),
            reason: "Two walls, in this order. Terminal settlement needs a Market carrying a \
                     terminal receipt, and this campaign now gets closer to one than any before it \
                     -- the Source state exists, the three Funds are Active -- and still stops at \
                     the provider legs for the reason the resolution gap gives. Behind that, \
                     settlement is a SignedDeltaV3-framed Claims route and carries the same \
                     signing-CallerAuthority requirement as every other Claims mutation, so it is \
                     also behind the Hot gate. The loser side is worth stating precisely because \
                     it is the half that gets skipped: a zero-payout redemption must refuse or \
                     move exactly zero atoms, and which of the two it does is the thing to \
                     measure, not to assume."
                .into(),
        },
        GapV1 {
            stage: "retirement: the atomic close of Claims, Custody and the Hoard".into(),
            routes: vec![
                "core/retire_v1::process#Retire".into(),
                "core/resolution::process#Retire".into(),
                "rent/process_close_v2#Close".into(),
            ],
            owner: "the Hot gate, which is not where this gap was expected to be".into(),
            reason: "This gap used to read \'behind the terminal receipt\'. The receipt now exists, \
                     this campaign begins retiring and closes the whole Source subtree, and the \
                     gap MOVED rather than closing. `build_market_retirement_v1` compiles ONE \
                     atomic Registry continuation that closes the Claims aggregate, the Custody \
                     replay and the Hoard vault together, and it refuses to compile at all while \
                     the Hoard holds a single atom -- partial Custody settlement cannot retire. \
                     That is the correct rule. But this Market\'s Hoard holds the entire founding \
                     principal, emptying it means redeeming, and redemption is a Claims mutation \
                     behind the Hot gate. So the LAST step of the Market\'s life is behind the \
                     same door as the middle of it, and the retirement stage reports it with the \
                     Hoard\'s measured balance rather than by reading the operator. `rent/\
                     process_close_v2#Close` is behind the retirement in turn: it needs a retired \
                     Market and a Core close-authority signer."
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
