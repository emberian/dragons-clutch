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
//! # The publication is minted, and that is what makes a rung reachable
//!
//! One `WindowSpecV1.max_age_seconds` governs both the crank's admissibility
//! and the publication's freshness, and against a FROZEN capture no value of it
//! leaves both legs walkable inside one run -- so this tier mints its own Pyth
//! publication (`pyth_lab_publication.rs`) about the CLUSTER's own block time,
//! compiles its market against that projection, and answers the rung with a
//! second publication about the same instant under a second Wormhole sequence.
//! The shelf life is this tier's parameter and the lab's tolerance for an
//! artifact it made twenty minutes ago; it is never a market's staleness policy.
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
use crate::ledger::{ClassClaimV1, ConservationLedgerV1, LamportClaimV1};
use crate::market::LocalMarketShapeV1;
use crate::model::{MarketRunInput, TransactionEvidence};
use crate::plan::pubkey;
use crate::provider::{ProviderPlanV1, PublicationV1, RungCaptureV1};
use crate::pyth_lab_publication::{
    LabPublicationRequestV1, LabPublicationV1, mint_lab_publication_v1, recover_vaa_signers_v1,
};
use crate::resolution::{RecordPairV1, ResolutionAddressesV1};
use crate::rpc::Rpc;
use crate::stages::MarketAddressesV1;
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
const CRANK_TOO_EARLY_MARKER_V1: &str = crate::recovery_crank::TOO_EARLY_MARKER_V1;

/// `wait_until_unix_seconds_v1`'s own sentence for a target it will not sleep
/// to (`sponsored_schedule.rs`).
///
/// A preflight that refused with the marker above builds NO PLAN, so the
/// not-yet-due guard below -- which measures `due - observed` off a plan --
/// cannot fire, and the walk goes on into the driver's bounded wait. That wait
/// is the second half of the same measurement and it refuses for the same
/// reason, and until 2026-09-06 its refusal was propagated with `?` and killed
/// the campaign before a transcript existed. It is a FINDING, recorded on this
/// stage exactly as the preflight's own refusals are: nothing was sent, the
/// conjunct held, and the distance is in the driver's sentence. Only a wait
/// that refuses for THIS reason is recorded; any other error still stops the
/// walk.
const CRANK_CEILING_MARKER_V1: &str = crate::recovery_crank::WAIT_CEILING_MARKER_V1;

/// How old a minted publication may be before the transport refuses it.
///
/// A TIER PARAMETER, NEVER A MARKET'S. It is the lab's tolerance for the age of
/// the publication the lab itself minted, and it is what makes both legs
/// reachable inside one run: the window ends at the mint instant, so the primary
/// leg is due `shelf_life` seconds later and the rung its own committed interval
/// after that. Twenty minutes is long enough that a founding, a crank and a
/// capture all fit before the primary leg closes, and short enough that a walk
/// which waits for it finishes inside `--max-wait-seconds`.
pub(crate) const DEFAULT_PUBLICATION_SHELF_LIFE_SECONDS_V1: i64 = 1_200;

/// The Wormhole sequences the two publications of one run carry.
///
/// Distinct sequences make distinct VAAs about one instant, which is exactly
/// what a rung needs: the leg it answers is a SECOND publication about the same
/// period, not a re-post of the one the primary leg was offered.
const PRIMARY_PUBLICATION_SEQUENCE_V1: u64 = 1;
const RUNG_PUBLICATION_SEQUENCE_V1: u64 = 2;

/// The terminal replay sequence a capture writes its certificate under.
///
/// One, and `resolution::derive` derives the seat at one: the ADVANCE and
/// EXHAUST sequences above are the CRANK's receipts, which are a different
/// certificate kind at a different seat.
const CAPTURE_TERMINAL_SEQUENCE_V1: u64 = 1;

/// The write authority the projected `PriceUpdateV2` image names.
///
/// A PROJECTION, NOT AN ACCOUNT. The image is read by the market compiler for
/// the feed identity, the exponent and the publication instant, and by nothing
/// else; the account the receiver actually writes on chain names whatever
/// authority the Resolution submit route derives, and that is the image a
/// submission digests. Zero says plainly that this field is not a key anybody
/// holds.
const PROJECTION_WRITE_AUTHORITY_V1: [u8; 32] = [0; 32];

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
    pub(crate) publication_shelf_life_seconds: i64,
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

    // -------------------------------- 2. the publication, minted at this hour
    //
    // THE FIXTURE QUESTION, ANSWERED BY A PRODUCER RATHER THAN BY A CONSTANT.
    // The pinned capture's publication instant is frozen in August 2026, and one
    // `WindowSpecV1.max_age_seconds` governs BOTH the crank's admissibility and
    // the publication's freshness -- so against a frozen capture no value of
    // that field leaves both legs walkable, and this tier's rung witness was red
    // for exactly that arithmetic. The lab's guardian set is the nineteen
    // derivable dummy keys, so a fresh 13-of-19 VAA over a fresh
    // `PriceFeedMessage` is signable here, offline, about any instant -- and the
    // real router and receiver ELFs verify it exactly as they verify the
    // capture.
    //
    // The instant is the CLUSTER's, read off its own block time. A campaign that
    // minted about its host's wall clock would be asserting a market's window
    // against a clock the chain does not keep, which is the same defect as
    // warping one.
    let mut rpc = Rpc::connect(&checked.rpc_url)?;
    let minted_at = {
        let slot = rpc.finalized_slot()?;
        rpc.block_time(slot)?
    };
    let primary_publication = mint_lab_publication_v1(
        LabPublicationRequestV1::at(minted_at, PRIMARY_PUBLICATION_SEQUENCE_V1),
        PROJECTION_WRITE_AUTHORITY_V1,
    )?;
    let mut publications = vec![publication_report_v1(&primary_publication)?];
    let shelf_life_seconds = request.publication_shelf_life_seconds;
    let terminal_max_age_seconds = u32::try_from(shelf_life_seconds).map_err(|_| {
        Error::new("--publication-shelf-life-seconds must fit the window record's u32 max_age")
    })?;
    stages.push(StageV1::new(
        "publication minted",
        "executed",
        format!(
            "A fresh Pyth publication about unix {minted_at} -- the CLUSTER's own block time, not \
             this host's wall clock -- signed thirteen of nineteen against the lab's derivable \
             guardian set, over a single-leaf accumulator root. Sequence \
             {PRIMARY_PUBLICATION_SEQUENCE_V1}. The market below is compiled against its \
             projection, so its window ENDS at {minted_at}, its primary leg falls due at \
             {} (that instant plus the {shelf_life_seconds}-second shelf life this tier states), \
             and the rung `{}` buys falls due its own committed interval after that. Both legs are \
             therefore inside the hour this run occupies, which is what no value of \
             `max_age_seconds` could buy against a frozen capture.",
            minted_at.saturating_add(shelf_life_seconds),
            request.recovery_rungs
        ),
    ));

    // ------------------------------- 3. the two-source market, compiled live
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
        // The window's end, the feed and the exponent all come off the minted
        // projection, and the shelf life is stated beside it: a market whose
        // rungs were anchored on one number while its published window carried
        // another would be a ladder nobody could walk.
        price_update_image: Some(primary_publication.projected_price_update.clone()),
        terminal_max_age_seconds: Some(terminal_max_age_seconds),
        ..LocalMarketShapeV1::default()
    };
    let market_input =
        crate::market::demo_market_input_shaped(registry, direct.compiler(), &shape)?;
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

    // ------------------------------------------------------- 4. the founding
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

    // ------------------------------------- 5. the census, bound before it runs
    //
    // The journey's conservation ledger, over this tier's own market. Every
    // account the walk will touch is registered BEFORE the first boundary, so a
    // census that later meets one holding a balance has a predecessor to
    // difference against; L7 says `inapplicable` and names the labels rather
    // than counting a whole balance as growth, and a boundary that earns a green
    // has earned it.
    let market_addresses = MarketAddressesV1::from_evidence(&accounts)?;
    let payer = substrate::campaign_payer_keypair(&checked)?;
    let mut ledger = ConservationLedgerV1::new(market_addresses.mint, payer.pubkey());
    ledger.track_market(market_addresses.founding_market);
    crate::stages::admit_open_market(
        &mut rpc,
        &market_addresses,
        &accounts,
        pubkey(&checked.plan.custody.program_id)?,
        pubkey(&checked.plan.rent_credit.program_id)?,
        &mut ledger,
    )?;
    let resolution_addresses =
        crate::resolution::derive(&mut rpc, &checked.plan, &market_addresses, &accounts)?;
    crate::resolution::watch(&mut ledger, &resolution_addresses);
    let provider_plan = ProviderPlanV1::derive(&mut rpc, &checked.plan)?;
    crate::provider::watch(&mut ledger, &provider_plan);

    // ------------------------------------------------------ 6. the crank(s)
    let worker = Keypair::new();
    let worker_keypair_path = request.work.join("worker.json");
    write_keypair_file(&worker_keypair_path, &worker)?;
    // WATCHED, AND FUNDED BEFORE THE FIRST CENSUS. The crank's bounty leaves the
    // market's Resolution funding compartment and lands on the stranger who
    // cranked, and a ledger that watched the compartment but not the stranger
    // would read that payment as a leak. Both sides of it are inside the watched
    // set, so L7 can hold the boundary to `debit == credit + fee` exactly.
    ledger.watch("ladder_crank_worker", worker.pubkey());
    transactions.push(rpc.airdrop(
        "ladder: fund the stranger who cranks",
        worker.pubkey(),
        WORKER_FUNDING_LAMPORTS,
    )?);
    ledger.observe(
        &mut rpc,
        "founding through Open, and funded",
        0,
        0,
        // The founding's lamport placements are the founding campaign's, and
        // this ledger does not restate them: it would be re-deriving another
        // campaign's arithmetic and calling the agreement evidence.
        LamportClaimV1::inapplicable(
            "the founding's lamport movements belong to `campaign --founding-only` and are \
             covered by tier 1's own witnesses; this ledger accounts for lamports from the first \
             ladder-owned boundary onward",
        ),
        ClassClaimV1::inapplicable(
            "the founding's compartment placements belong to the founding campaign; this ledger \
             accounts per compartment class from the first ladder-owned boundary onward",
        ),
    )?;

    // THE RUNG'S OWN RECORDS, and the hostile that says the ladder is not a
    // free choice. Only the capture walk needs them; the exhaust walk is about
    // two legs expiring and answers on neither.
    let rung = match request.walk {
        WalkV1::Exhaust => None,
        WalkV1::Capture => Some(rung_capture_v1(
            &market_input,
            registry,
            &resolution_addresses,
        )?),
    };
    if let Some(rung) = &rung {
        let refusal = crate::provider::refuse_capture_before_the_rung_v1(
            &mut rpc,
            &checked.plan,
            &resolution_addresses,
            &provider_plan,
            &PublicationV1::captured(),
            rung,
        )?;
        match refusal {
            Some(text) => stages.push(StageV1::new(
                "capture before the rung is due",
                "refused-by-name",
                format!(
                    "The chain-derived operator was asked to build a capture that answers this \
                     market's rung while the market still stands on its primary leg, and it \
                     refused off chain with no key open and nothing sent: {text}. The refusal is \
                     recorded verbatim rather than asserted, because more than one of the \
                     builder's ordered conjuncts is false at this instant -- the Source stands on \
                     Primary while the request brings the ladder, and no update has been \
                     submitted, so the lifecycle the request names is a vacancy. The \
                     phase-versus-ladder conjunct is convicted where it is REACHABLE, in the \
                     capture stage below, against a Source that really is standing on the rung."
                ),
            )),
            None => stages.push(StageV1::new(
                "capture before the rung is due",
                "built-a-capture",
                "FINDING, NOT A STOP. The operator BUILT a rung capture against a market standing \
                 on its primary leg. A capture that can answer a leg the market has not reached \
                 is a leg the holders paid for that nobody had to walk, and the walk continues so \
                 that the transcript carries both this and whatever the chain then says."
                    .to_owned(),
            )),
        }
    }

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
    if advanced {
        if let Some(certificate) = advance.certificate {
            ledger.watch("ladder_advance_certificate", certificate);
        }
        ledger.observe(
            &mut rpc,
            "advance onto the funded alternative",
            0,
            0,
            // The crank's fee, read off its own finalized evidence. The bounty
            // it pays the stranger moves between two accounts this ledger
            // watches, so the strong claim is that nothing else moved.
            LamportClaimV1::fees(advance.fee_lamports),
            // A crank moves no collateral at all: it writes a certificate and a
            // Source phase byte, and no vault is opened and no atom transferred.
            ClassClaimV1::unchanged(),
        )?;
    }

    let mut refund: Option<serde_json::Value> = None;
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
        let exhausted = exhaust.landed;
        let fee = exhaust.fee_lamports;
        let certificate = exhaust.certificate;
        cranks.push(exhaust.report);
        // THE EXHAUSTED PATH CONTINUES TO THE REFUND. An `Exhausted` receipt
        // is not an end; decisions 0027 and 0025 say what follows it, and
        // until this tier drove it nothing on any chain had.
        //
        // The census is taken at the exhaustion's own boundary, BEFORE the
        // refund: the refund's stages are the failure arm's and it does not
        // carry this ledger, so what is claimed here is the crank and nothing
        // past it.
        if exhausted {
            if let Some(certificate) = certificate {
                ledger.watch("ladder_exhaust_certificate", certificate);
            }
            ledger.observe(
                &mut rpc,
                "exhaust the last funded rung",
                0,
                0,
                LamportClaimV1::fees(fee),
                ClassClaimV1::unchanged(),
            )?;
            refund = Some(continue_to_the_refund(
                &checked,
                &request,
                &mut rpc,
                &mut stages,
                &mut transactions,
                &accounts,
                market,
                &worker,
                &worker_keypair_path,
            )?);
        }
    }

    // ------------------------------------------------- 7. the rung, ANSWERED
    let mut capture = serde_json::Value::Null;
    if let (WalkV1::Capture, true, Some(rung)) = (request.walk, advanced, rung.as_ref()) {
        // A SECOND PUBLICATION ABOUT THE SAME PERIOD. The rung answers the
        // question the primary leg was asked and did not answer, so the
        // observation has to be ABOUT the same instant -- and it has to be a
        // different VAA, or the rung would be answered by a re-post of the
        // publication its own market already declined.
        let rung_publication = mint_lab_publication_v1(
            LabPublicationRequestV1::at(minted_at, RUNG_PUBLICATION_SEQUENCE_V1),
            PROJECTION_WRITE_AUTHORITY_V1,
        )?;
        publications.push(publication_report_v1(&rung_publication)?);
        let capture_dir = request.work.join("captures");
        let (report, lamports) = crate::provider::resolve_through_pyth(
            &mut rpc,
            &payer,
            &checked.plan,
            &resolution_addresses,
            &provider_plan,
            &capture_dir,
            &mut transactions,
            &PublicationV1 {
                signed_vaa: rung_publication.signed_vaa.clone(),
                post_update_body: rung_publication.post_update_body.clone(),
                price_update_image: rung_publication.projected_price_update.clone(),
                shelf_life_seconds,
            },
            Some(rung),
            CAPTURE_TERMINAL_SEQUENCE_V1,
        )?;
        stages.push(StageV1::new(
            &report.stage,
            &report.outcome,
            report.note.clone(),
        ));
        // WHICH LEG THE CHAIN SAYS ANSWERED. Read back off the Source and the
        // certificate rather than carried out of the stage that wrote them: the
        // transcript's claim about the route is the chain's own answer.
        capture = capture_reading_v1(&mut rpc, &resolution_addresses)?;
        ledger.observe(
            &mut rpc,
            &report.stage,
            0,
            0,
            lamports,
            // The transport moves no collateral: it posts a publication, mints
            // a certificate and resolves a Source. Not one atom crosses a
            // compartment boundary, and this fails if one does.
            ClassClaimV1::unchanged(),
        )?;

        // --------------------------------- 8. the Market's own phase byte
        let (admit_report, admit_lamports, _outcomes) = crate::resolution::admit_terminal(
            &mut rpc,
            &payer,
            &resolution_addresses,
            &mut transactions,
        )?;
        stages.push(StageV1::new(
            &admit_report.stage,
            &admit_report.outcome,
            admit_report.note.clone(),
        ));
        ledger.observe(
            &mut rpc,
            &admit_report.stage,
            0,
            0,
            admit_lamports,
            // AdmitTerminal writes the Market's phase byte, its terminal
            // receipt and its terminal winner, and moves no collateral at all.
            ClassClaimV1::unchanged(),
        )?;
    } else if request.walk == WalkV1::Capture {
        stages.push(StageV1::new(
            "the rung is answered",
            "unreachable",
            "The ladder never advanced, so there is no rung to answer on. The crank stage above \
             says why, in the two seconds it names."
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
        "publication_shelf_life_seconds": shelf_life_seconds,
        "publications": publications,
        "cranks": cranks,
        "refund": refund,
        "capture": capture,
        "conservation": {
            "observations": ledger.observations(),
            "violations": ledger.violations(),
        },
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

/// How many times the payout driver may be re-entered for one refund.
///
/// The driver advances one durable action per invocation -- four routing-table
/// acts and then the payout -- so this bounds ACTS, not retries.
const REFUND_RESUMPTION_CEILING_V1: usize = 24;

/// SPL Token `InitializeAccount3`: the owner rides inline, so no Rent sysvar
/// and no second transaction. The same discriminant the journey's collateral
/// distribution and the successor's Direct token setup spell; a shared
/// constant is owed to the convergence (BUILD_FAILURE-ARM.md, seams).
const INITIALIZE_ACCOUNT_3: u8 = 18;

/// From `Exhausted` to the refund: the deadline walk, Core's admission, and the
/// founder drawing only their holdings -- through the shipped drivers, in order.
///
/// The ladder's market has no stranger (it never fills), so the founder holds
/// every ordinary claim and the escrow the whole failure column. Under the
/// failure selector every ordinary claim is one atom on the refunding scale,
/// so the founder's refunds over the ordinary indices sum to exactly the Hoard,
/// and the escrow's payout is a NO-OP the transcript records: the producer
/// refuses an owner that is the Market's own escrow by name, because nothing
/// can sign for a keyless PDA and its column is discharged by the closure burn.
#[allow(clippy::too_many_arguments)]
fn continue_to_the_refund(
    checked: &substrate::CheckedSubstrateV1,
    request: &LadderRequestV1,
    rpc: &mut Rpc,
    stages: &mut Vec<StageV1>,
    transactions: &mut Vec<TransactionEvidence>,
    accounts: &std::collections::BTreeMap<String, crate::model::AccountEvidence>,
    market: Pubkey,
    worker: &Keypair,
    worker_keypair: &Path,
) -> Result<serde_json::Value> {
    use dclutch_custody::token_svm::{ACCOUNT_BYTES, TOKEN_2022_PROGRAM_ID, TokenAccount};
    use solana_sdk::instruction::{AccountMeta, Instruction};
    use solana_system_interface::instruction::create_account;

    let evidence_path = request.work.join("founding-evidence.json");
    let base = |sequence: u64| -> Vec<String> {
        vec![
            "--rpc-url".to_owned(),
            checked.rpc_url.clone(),
            "--plan".to_owned(),
            checked.plan_path.display().to_string(),
            "--evidence".to_owned(),
            evidence_path.display().to_string(),
            "--market".to_owned(),
            market.to_string(),
            "--terminal-sequence".to_owned(),
            sequence.to_string(),
        ]
    };

    // ------------------------------------------ 1. the failure selector
    let mut walk = base(FAILURE_SEQUENCE_V1);
    walk.extend([
        "--worker".to_owned(),
        worker.pubkey().to_string(),
        "--output".to_owned(),
        request
            .work
            .join("deadline-failure.json")
            .display()
            .to_string(),
        "--wait".to_owned(),
        "--max-wait-seconds".to_owned(),
        request.max_wait_seconds.to_string(),
        "--execute".to_owned(),
        "--worker-keypair".to_owned(),
        worker_keypair.display().to_string(),
    ]);
    let walked = crate::deadline_failure::run_v1(walk, ExpectedClusterV1::OwnedLoopback)?;
    let walk_evidence = walked
        .landed
        .ok_or_else(|| Error::new("an executed deadline walk reported no landed transaction"))?;
    let work_paid = walked.work_paid.unwrap_or(0);
    stages.push(StageV1::new(
        "the failure selector is committed",
        "executed",
        format!(
            "The shipped commit-deadline-failure driver took the {} arm from the Source's own \
             phase ({:?}), built the 22-account frame from relay_frame_roles_v1, and committed \
             the Product's failure cell {} of {} into the ResolutionFailure seat {} -- paying the \
             same stranger who cranked the ladder {work_paid} lamports out of the market's own \
             prepaid compartment. Signature {}, {} compute units. A ladder that exhausts is not \
             an end: this is the terminal decision 0027 says it exhausts INTO.",
            walked.arm,
            walked.phase_before,
            walked.failure_selector,
            walked.outcome_count,
            walked.certificate,
            walk_evidence.signature,
            walk_evidence
                .compute_units_consumed
                .map_or_else(|| "unreported".to_owned(), |value| value.to_string())
        ),
    ));
    let walk_signature = walk_evidence.signature.clone();
    transactions.push(walk_evidence);

    // ----------------------------------------------- 2. Core admits it
    let mut admit = base(FAILURE_SEQUENCE_V1);
    admit.extend([
        "--fee-payer".to_owned(),
        worker.pubkey().to_string(),
        "--output".to_owned(),
        request
            .work
            .join("admit-terminal.json")
            .display()
            .to_string(),
        "--execute".to_owned(),
        "--fee-payer-keypair".to_owned(),
        worker_keypair.display().to_string(),
    ]);
    let admitted = crate::admit_terminal::run_v1(admit, ExpectedClusterV1::OwnedLoopback)?;
    let admit_units: u64 = admitted
        .transactions
        .iter()
        .filter_map(|evidence| evidence.compute_units_consumed)
        .sum();
    stages.push(StageV1::new(
        "Core admits the failure certificate",
        "executed",
        format!(
            "The shipped admit-terminal driver read the certificate kind off the Source's \
             FailureCommitted phase ({:?}), built the frame through \
             build_resolution_admit_terminal_v3 -- the one author every terminal admission \
             calls -- rode it over one frozen routing table, and the Market's phase byte moved \
             1 to 2 with terminal winner {} of {}. {} transactions, {admit_units} compute units.",
            admitted.kind,
            admitted.selector,
            admitted.outcome_count,
            admitted.transactions.len()
        ),
    ));
    transactions.extend(admitted.transactions.iter().cloned());

    // ------------------------- 3. an account the FOUNDER KEY owns to be paid into
    //
    // The founding's collateral wallet answers to the campaign payer, not to
    // the founder role whose Position the payout debits, and the builder
    // refuses a recipient owned by anybody but the stated owner (JOURNEY-8).
    let founder_keypair_path = prepare_role_key(&checked.report, "founding-founder")?;
    let founder = substrate::load_keypair(&founder_keypair_path)?;
    let label_address = |label: &str| -> Result<Pubkey> {
        pubkey(
            &accounts
                .get(label)
                .ok_or_else(|| Error::new(format!("the founding's evidence names no `{label}`")))?
                .address,
        )
    };
    let mint = label_address("collateral_mint")?;
    let hoard = label_address("founding_hoard_vault_open")?;
    let aggregate = label_address("claims_aggregate")?;
    let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);
    let recipient = Keypair::new();
    let mut initialize = Vec::with_capacity(33);
    initialize.push(INITIALIZE_ACCOUNT_3);
    initialize.extend_from_slice(founder.pubkey().as_ref());
    let account_rent = rpc.minimum_balance(ACCOUNT_BYTES)?;
    let opened = rpc.send_with_signers(
        "ladder: open the founder's refund account",
        &[
            create_account(
                &worker.pubkey(),
                &recipient.pubkey(),
                account_rent,
                ACCOUNT_BYTES as u64,
                &token_program,
            ),
            Instruction {
                program_id: token_program,
                accounts: vec![
                    AccountMeta::new(recipient.pubkey(), false),
                    AccountMeta::new_readonly(mint, false),
                ],
                data: initialize,
            },
        ],
        worker,
        &[&recipient],
    )?;
    transactions.push(opened);
    let hoard_amount = |rpc: &mut Rpc| -> Result<u64> {
        let account = rpc.required_account(hoard, "Hoard vault")?;
        Ok(TokenAccount::parse(&account.data)
            .map_err(|error| Error::new(format!("Hoard vault: {error:?}")))?
            .amount)
    };
    let hoard_before = hoard_amount(rpc)?;

    // --------------------------- 4. the founder draws only their holdings
    let ordinary_count = admitted
        .outcome_count
        .checked_sub(1)
        .ok_or_else(|| Error::new("a Product with no outcomes"))?;
    let mut refunds = Vec::new();
    let mut paid_total: u64 = 0;
    for claim_index in 0..ordinary_count {
        let input =
            crate::terminal_lifecycle::produce_wallet_terminal_input_owned_loopback_v1(vec![
                "--rpc-url".to_owned(),
                checked.rpc_url.clone(),
                "--plan".to_owned(),
                checked.plan_path.display().to_string(),
                "--evidence".to_owned(),
                evidence_path.display().to_string(),
                "--market".to_owned(),
                market.to_string(),
                "--owner".to_owned(),
                founder.pubkey().to_string(),
                "--recipient".to_owned(),
                recipient.pubkey().to_string(),
                "--claim-index".to_owned(),
                claim_index.to_string(),
            ])?;
        let input_path = request
            .work
            .join(format!("refund-{claim_index}-input.json"));
        std::fs::write(&input_path, serde_json::to_vec_pretty(&input)?)?;
        let journal_dir = request.work.join(format!("refund-{claim_index}-journal"));
        std::fs::create_dir_all(&journal_dir)?;
        let evidence = request
            .work
            .join(format!("refund-{claim_index}-evidence.json"));
        let arguments = vec![
            "--rpc-url".to_owned(),
            checked.rpc_url.clone(),
            "--input".to_owned(),
            input_path.display().to_string(),
            "--fee-payer".to_owned(),
            worker.pubkey().to_string(),
            "--fee-payer-keypair".to_owned(),
            worker_keypair.display().to_string(),
            "--owner-keypair".to_owned(),
            founder_keypair_path.display().to_string(),
            "--journal-dir".to_owned(),
            journal_dir.display().to_string(),
            "--evidence".to_owned(),
            evidence.display().to_string(),
            "--execute".to_owned(),
        ];
        // ONE DURABLE ACTION PER INVOCATION, re-entered until the evidence
        // exists: that is the driver's crash-safety contract, not a loop.
        let mut passes = 0_usize;
        while !evidence.exists() {
            passes += 1;
            if passes > REFUND_RESUMPTION_CEILING_V1 {
                return Err(Error::new(format!(
                    "refund at claim index {claim_index}: the payout driver was re-entered \
                     {REFUND_RESUMPTION_CEILING_V1} times without writing its evidence"
                )));
            }
            crate::wallet_terminal_payout_exterior::run(arguments.clone())?;
        }
        let document: serde_json::Value = serde_json::from_slice(&std::fs::read(&evidence)?)?;
        let payout = document
            .get("payout")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| Error::new("the payout evidence declared no `payout` amount"))?;
        paid_total = paid_total.saturating_add(payout);
        refunds.push(serde_json::json!({
            "claimIndex": claim_index,
            "payout": payout,
            "passes": passes,
            "evidence": evidence.display().to_string(),
        }));
        stages.push(StageV1::new(
            &format!("the founder is refunded at ordinary index {claim_index}"),
            "executed",
            format!(
                "wallet-terminal-payout-input then the shipped payout driver ({passes} \
                 invocations, one durable stage each) under the ResolutionFailure certificate: \
                 the evaluator took the refunding failure arm and paid {payout} atoms -- the \
                 founder's own balance at this index, one atom per ordinary claim, and nothing \
                 for having chosen the oracle. The recipient is an account the founder key \
                 owns, opened by this tier."
            ),
        ));
    }
    let hoard_after = hoard_amount(rpc)?;

    // ----------------------- 5. the escrow's payout is a no-op, recorded
    let claims = pubkey(&checked.plan.claims.program_id)?;
    let escrow = dclutch_operator::failure_escrow_v1::failure_escrow_v1(
        claims,
        market.to_bytes(),
        aggregate,
        admitted.outcome_count,
    )
    .map_err(|error| Error::new(format!("failure escrow: {error}")))?;
    let escrow_refusal =
        crate::terminal_lifecycle::produce_wallet_terminal_input_owned_loopback_v1(vec![
            "--rpc-url".to_owned(),
            checked.rpc_url.clone(),
            "--plan".to_owned(),
            checked.plan_path.display().to_string(),
            "--evidence".to_owned(),
            evidence_path.display().to_string(),
            "--market".to_owned(),
            market.to_string(),
            "--owner".to_owned(),
            escrow.owner.to_string(),
            "--recipient".to_owned(),
            recipient.pubkey().to_string(),
            "--claim-index".to_owned(),
            escrow.failure_selector.to_string(),
        ]);
    let escrow_outcome = match escrow_refusal {
        Err(error) if error.to_string().contains("own failure escrow") => {
            ("recorded-no-op", error.to_string())
        }
        Err(error) => {
            return Err(Error::new(format!(
                "the producer refused the escrow's payout for a reason other than the escrow \
                 being keyless: {error}"
            )));
        }
        Ok(_) => {
            return Err(Error::new(
                "the producer BUILT a payout input for the Market's own failure escrow; nothing \
                 can sign for it and the column is the closure burn's to discharge",
            ));
        }
    };
    stages.push(StageV1::new(
        "the escrow's payout is a no-op the census records",
        escrow_outcome.0,
        format!(
            "No transaction. The escrow {} is owned by {}, a program-derived address with no \
             key, and wallet-terminal-payout-input refused an --owner naming it before a key \
             opened: {}. Its column at index {} is the seated residue decision 0025 shape A \
             burns inside the retirement's prepare packet.",
            escrow.position, escrow.owner, escrow_outcome.1, escrow.failure_selector
        ),
    ));

    Ok(serde_json::json!({
        "failureWalk": {
            "arm": walked.arm,
            "certificate": walked.certificate.to_string(),
            "failureSelector": walked.failure_selector,
            "outcomeCount": walked.outcome_count,
            "workPaid": work_paid,
            "signature": walk_signature,
            "frameAccounts": walked.frame_accounts,
        },
        "admission": {
            "certificateKind": format!("{:?}", admitted.kind),
            "selector": admitted.selector,
            "transactions": admitted.transactions.len(),
            "routingTableRentLamports": admitted.table_rent_lamports,
        },
        "founder": founder.pubkey().to_string(),
        "founderRecipient": recipient.pubkey().to_string(),
        "refunds": refunds,
        "paidTotal": paid_total,
        "hoardBefore": hoard_before,
        "hoardAfter": hoard_after,
        "hoardDrained": hoard_before.saturating_sub(hoard_after) == paid_total && hoard_after == 0,
        "escrow": {
            "owner": escrow.owner.to_string(),
            "position": escrow.position.to_string(),
            "failureSelector": escrow.failure_selector,
            "payout": escrow_outcome.0,
        },
    }))
}

/// Return the one durable local key path for a role that this tier must use.
///
/// The founding projection intentionally contains only the key files accepted
/// by the founding command. `founding-founder` is instead a retained local
/// identity: the command receives its public key but does not open its secret.
/// A post-terminal refund does open that same retained signer, so it must take
/// the general prepare projection as the other shipped campaign clients do.
fn prepare_role_key(
    report: &crate::local_mutable::LocalMutablePrepareReportV1,
    role: &str,
) -> Result<PathBuf> {
    report
        .campaign_founding_keypairs
        .get(role)
        .or_else(|| report.keypairs.get(role))
        .map(PathBuf::from)
        .ok_or_else(|| Error::new(format!("the prepare report names no key file for `{role}`")))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::local_mutable::LocalMutablePrepareReportV1;

    use super::prepare_role_key;

    #[test]
    fn founding_founder_uses_the_retained_prepare_key() {
        let report = LocalMutablePrepareReportV1 {
            schema: "test".into(),
            plan: "/tmp/plan.json".into(),
            account_dir: "/tmp/accounts".into(),
            checked_local_mutable_set_sha256: "test".into(),
            retained_upgrade_authority: "test".into(),
            programs: BTreeMap::new(),
            keypairs: BTreeMap::from([(
                "founding-founder".to_owned(),
                "/tmp/founding-founder.json".to_owned(),
            )]),
            campaign_keypairs: BTreeMap::new(),
            campaign_administration_keypairs: BTreeMap::new(),
            campaign_founding_keypairs: BTreeMap::new(),
            campaign_public_identities: BTreeMap::new(),
        };

        assert_eq!(
            prepare_role_key(&report, "founding-founder")
                .expect("the retained founder must remain available to the refund tier"),
            std::path::PathBuf::from("/tmp/founding-founder.json")
        );
    }
}

/// The terminal certificate sequence the failure walk writes, and the one the
/// admission and every refund then read.
const FAILURE_SEQUENCE_V1: u64 = 1;

/// What one publication of this run was, in the transcript's own words.
///
/// The instant, the sequence, the accumulator root and the guardians whose
/// signatures RECOVER to the lab set -- recovered here rather than counted, so a
/// transcript that says thirteen signed is saying thirteen verified.
fn publication_report_v1(publication: &LabPublicationV1) -> Result<serde_json::Value> {
    let signers = recover_vaa_signers_v1(&publication.signed_vaa)?;
    Ok(serde_json::json!({
        "sequence": publication.request.sequence,
        "publishTime": publication.request.publish_time,
        "root": publication
            .root
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        "signerCount": signers.len(),
        "signers": signers,
    }))
}

/// The rung's own two records, derived from the bodies the compiler emitted.
///
/// A RECORD'S IDENTITY IS THE HASH OF ITS BODY, so this is a derivation and not
/// a lookup -- the same one `resolution::derive` performs for the market's own
/// records, from the same author. It has to be: the founding's evidence names
/// the `RecoveryPolicyV2` record and does NOT name the alternative source pair,
/// so the only place the rung's coordinates exist is the compiled input this
/// tier founded from, and deriving them from those bodies is what makes a rung
/// whose record moved a named mismatch rather than a later refusal.
fn rung_capture_v1(
    input: &MarketRunInput,
    registry: Pubkey,
    addresses: &ResolutionAddressesV1,
) -> Result<RungCaptureV1> {
    let records = input.recovery_source_records.first().ok_or_else(|| {
        Error::new(
            "the compiled market publishes no recovery source records: there is no alternative \
             source to answer a rung on",
        )
    })?;
    let spec = crate::runtime::decode_hex(&records.source_spec_hex)?;
    let adapter = crate::runtime::decode_hex(&records.pyth_adapter_config_hex)?;
    Ok(RungCaptureV1 {
        policy: addresses.recovery_policy,
        source_spec: RecordPairV1::derive(
            registry,
            dclutch_source::SOURCE_SPEC_SCHEMA_ID_V1,
            &spec,
        ),
        adapter_config: RecordPairV1::derive(
            registry,
            dclutch_source::PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1,
            &adapter,
        ),
    })
}

/// What the chain says about the leg that answered.
///
/// The route and the attempt index are the whole difference between a market
/// whose first choice answered and a market whose funded alternative did, and
/// both are written by the program. This reads them back off the Source's own
/// terminal projection and the certificate the capture minted.
fn capture_reading_v1(
    rpc: &mut Rpc,
    addresses: &ResolutionAddressesV1,
) -> Result<serde_json::Value> {
    use dclutch_source::SourceResolutionStateV2;
    use dclutch_source::resolution::ResolutionCertificateV2;
    let source = SourceResolutionStateV2::decode(
        &rpc.required_account(addresses.source_state, "Source resolution state")?
            .data,
    )
    .map_err(|error| Error::new(format!("Source after the capture: {error:?}")))?;
    let projection = source
        .terminal_projection()
        .map_err(|error| Error::new(format!("Source terminal projection: {error:?}")))?;
    let certificate = ResolutionCertificateV2::decode(
        &rpc.required_account(addresses.certificate, "terminal certificate")?
            .data,
    )
    .map_err(|error| Error::new(format!("terminal certificate: {error:?}")))?;
    Ok(serde_json::json!({
        "route": format!("{:?}", projection.route()),
        "attemptIndex": certificate.attempt_index,
        "phase": format!("{:?}", source.phase()),
        "selector": certificate.selector,
        "terminalSequence": projection.terminal_sequence(),
        "certificate": addresses.certificate.to_string(),
    }))
}

/// One crank of the ladder, driven through the SHIPPED command.
struct CrankRunV1 {
    landed: bool,
    /// The fee the crank's own transaction paid, off its finalized evidence.
    fee_lamports: u64,
    /// The certificate seat the driver derived for this crank, so the census
    /// can watch the account the worker prefunds.
    certificate: Option<Pubkey>,
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
                fee_lamports: 0,
                certificate: None,
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
        let planned = planned
            .as_ref()
            .expect("a not-yet-due report carries its plan");
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
            fee_lamports: 0,
            certificate: Some(planned.certificate),
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
    let landed = match crate::recovery_crank::run_v1(execute, ExpectedClusterV1::OwnedLoopback) {
        Ok(landed) => landed,
        Err(error) if error.to_string().contains(CRANK_CEILING_MARKER_V1) => {
            let reason = error.to_string();
            stages.push(StageV1::new(
                label,
                "not-yet-due",
                format!(
                    "The shipped advance-recovery driver refused the preflight as too early and \
                     then refused its own bounded wait: {reason} No key was opened and nothing was \
                     sent, which is the conjunct the ladder rests on holding. The market this tier \
                     founds is compiled against a publication minted at the run's own hour, so its \
                     primary leg is due one stated shelf life past that instant and its rung a \
                     committed interval after: a leg still past `--max-wait-seconds` from here is \
                     a shelf life or a rung interval wider than this walk's budget, and both are \
                     this tier's own parameters."
                ),
            ));
            return Ok(CrankRunV1 {
                landed: false,
                fee_lamports: 0,
                certificate: None,
                report: serde_json::json!({
                    "sequence": terminal_sequence,
                    "outcome": "not-yet-due",
                    "reason": reason,
                    "refusedBeforeTheDeadline": hostile,
                }),
            });
        }
        Err(error) => return Err(error),
    };
    let evidence = landed
        .landed
        .ok_or_else(|| Error::new("an executed crank reported no landed transaction"))?;
    let units = evidence.compute_units_consumed;
    let signature = evidence.signature.clone();
    let fee_lamports = evidence.fee_lamports.unwrap_or(0);
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
        fee_lamports,
        certificate: Some(landed.certificate),
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
