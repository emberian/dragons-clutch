//! The Direct route as a FEE-CHARGING market runs it, executed rather than argued.
//!
//! # The measurement this repository had never taken
//!
//! Every compute figure ever quoted for the top-level Direct route -- the
//! variance census, the margin gate's constant, `CU_BUDGETS.json`, the Wall #28
//! acceptance -- was taken on a trade whose fee is ZERO. Not zero by design:
//! zero by flooring. The fixture trades `FILL = 10` at `EXECUTION_PRICE = 50`
//! against a `PRICE_SCALE` of 100, so `gross = 5`, and the market's 50 basis
//! points of that is `5 * 50 / 10_000`, which floors to nothing. A zero combined
//! fee sets the `seller_terminal` enable register and clears both fee registers,
//! so the transition projects ONE live Custody route out of the four the Effect
//! declares. The fee leg -- the second Custody route, its own caller authority,
//! its own replay revision step, its own delegated transfer -- had never
//! executed anywhere in this tree.
//!
//! That is not a rounding detail. It is the difference between the trade the
//! acceptance evidence covers and the trade a market that charges a fee would
//! actually make, and it was found on 2026-08-30 by the variance census
//! (`docs/evidence/DIRECT_HOT_CU_VARIANCE_CENSUS_2026-08-30.md`, finding 3),
//! which could only estimate it: "a fee-bearing trade is around 1.49-1.52M CU
//! and DOES NOT FIT. That is arithmetic on a measured single route, not a
//! measurement of the two-route shape, and it wants its own lane."
//!
//! This file is that lane's instrument. It executes both shapes.
//!
//! # Why both arms run here, in one test, rather than one arm against a number
//!
//! `release_set_id` hashes the five deployed role ELF digests and seeds the
//! Market identity, so every bump depth on this route is redrawn by ANY source
//! change to ANY role. A fee-bearing figure taken today and compared against the
//! zero-fee figure in an evidence doc written against a different tree is not a
//! comparison at all -- the census measured exactly that hazard at 27,000 CU on
//! a one-compute-unit code change.
//!
//! The trade scenario, by contrast, is a HOST-side fixture input. It changes no
//! ELF. So the two arms below are a genuinely controlled pair: same programs,
//! same keys, same seeds, same substrate, differing in what the transaction
//! does and in nothing else. The delta they report is the fee leg and only the
//! fee leg.
//!
//! # What is asserted and what is printed
//!
//! Asserted, because each is a property of the code:
//!
//! * the zero-fee arm invokes Custody ONCE and the fee-bearing arm TWICE, on
//!   every seed, read out of the program log rather than inferred from the
//!   fixture's own arithmetic;
//! * every seed's residual sits on the 1,500 CU bump-search grid, so the site
//!   census below is known to be stale rather than quietly wrong the day
//!   something key-dependent that is not a search appears;
//! * the fee-bearing route's KEY-INDEPENDENT cost has not regressed past
//!   [`FEE_BEARING_LOWER_BOUND_CU_V1`], which the second test measures because
//!   the first cannot: a route over the meter reports the meter;
//! * a refusal is a BUDGET refusal. A fee-bearing route over the ceiling refuses
//!   with `ComputationalBudgetExceeded` and that is a margin fact; a refusal
//!   with any other code is a broken fixture or a broken route and this file
//!   must not report it as a margin result.
//!
//! Printed and NOT asserted: whether the route fits. Fitting is a question about
//! a stranger's keys, which is a geometric distribution and not a constant --
//! the margin gate's own argument, reproduced here because the fee-bearing shape
//! searches at MORE sites and so has a fatter tail than the shape that argument
//! was written for.

use solana_program::{instruction::InstructionError, pubkey::Pubkey};
use solana_program_test::BanksClientError;
use solana_sdk::{signature::Signer, transaction::TransactionError};

use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{CallerRoleV1, CustodyAuthoritySeedsV1, CustodyReplaySeedsV1};
use dclutch_direct_codec::ordinary_geometry_v3::DirectOrdinaryGeometryV3;
use dclutch_direct_codec::successor::{DirectCoordinatesV1, MakerReplaySeedsV1};
use dclutch_direct_hot_program_test_support::fixture::DirectTradeScenarioV1;
use dclutch_direct_hot_program_test_support::waist::{
    CORE_PROGRAM_ID, CUSTODY_PROGRAM_ID, DirectCase, Releases, TRADING_PROGRAM_ID,
    add_lookup_table, add_release_waist, canonical_lookup_addresses, direct_case_v5,
    direct_top_level_instructions, elves, fixture_substrate, program_test_without_forced_budget,
    start_with_substrate, submit_v0_observed, with_fixture_seed,
};
use dclutch_market_core_codec::{CoreState, MarketCoreStateSeedsV2};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};

/// Fixture seeds swept per arm, matching `direct_hot_top_level_margin_gate.rs`.
///
/// Thirty-two for the reason that file gives at length: twelve seeds understated
/// its worst draw by 7,659 CU. Two arms at thirty-two seeds is sixty-four real
/// transactions and about a minute and a half.
const GATE_SEEDS: u64 = 32;

/// 1,500 CU per candidate a bump search rejects, and per `create_program_address`.
const ATTEMPT_COST_CU: u64 = 1_500;

/// The protocol maximum a transaction may consume, and the budget this route asks for.
const PROTOCOL_CEILING: u64 = 1_400_000;

/// Attempts `find_program_address` makes to land on `bump`.
const fn attempts(bump: u8) -> u64 {
    256 - bump as u64
}

/// The two shapes the Direct route has, named by what they cost.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArmV1 {
    /// The historical fixture: gross 5, fee floors to zero, one Custody route.
    ZeroFee,
    /// Gross 200 at the same 50 bps: fee 1 per side, two Custody routes.
    FeeBearing,
}

impl ArmV1 {
    const fn label(self) -> &'static str {
        match self {
            Self::ZeroFee => "zero-fee",
            Self::FeeBearing => "fee-bearing",
        }
    }

    const fn scenario(self) -> DirectTradeScenarioV1 {
        match self {
            Self::ZeroFee => DirectTradeScenarioV1::ZERO_FEE,
            Self::FeeBearing => DirectTradeScenarioV1::FEE_BEARING,
        }
    }

    /// Custody CPIs this arm's enable registers select, and therefore which of
    /// the four declared `CUSTODY_ROUTES_V3` slots carry a live caller authority.
    ///
    /// Slot 0 is `SellerTerminal`, 1 is `SellerIntermediate`, 2 is
    /// `FeeContinuation`, 3 is `FeeSole`. A zero fee enables slot 0 alone; a
    /// nonzero fee with a nonzero seller net enables 1 and 2. This is the
    /// fixture's model of the transition's own `select_zero` arithmetic and it
    /// is CHECKED against the chain: the Custody invocation count in the
    /// program log must equal this slice's length on every seed.
    const fn live_route_slots(self) -> &'static [usize] {
        match self {
            Self::ZeroFee => &[0],
            Self::FeeBearing => &[1, 2],
        }
    }
}

/// The key-varying searches one arm pays, at one fixture draw.
///
/// # Which sites, and why the two Custody-side ones carry a multiplicity
///
/// Custody re-derives its replay PDA and its transfer authority PDA on EVERY
/// invocation, because each CPI is a fresh program invocation that runs the
/// whole handler. Both routes name the same two addresses -- their seeds carry
/// no route-specific term -- so the fee-bearing arm pays each of those searches
/// TWICE at one drawn depth, rather than paying two independent draws. The
/// caller authorities are the opposite: their seeds carry the route's own
/// child-request digest, so the two live routes search two DIFFERENT addresses
/// at two independent depths.
///
/// That asymmetry is why this struct models multiplicity explicitly instead of
/// summing a site list, and it is also the whole shape of the tail: doubling a
/// multiplicity doubles a term's cost without adding a draw, while adding a
/// route adds a draw.
#[derive(Clone, Debug)]
struct DepthsV1 {
    /// Carried, not searched -- printed so a reader can see what the carry is worth.
    market: u64,
    root: u64,
    seller_replay: u64,
    buyer_replay: u64,
    /// One per live Custody route, each its own address.
    caller_authorities: Vec<u64>,
    custody_replay: u64,
    custody_transfer_authority: u64,
    /// Custody invocations OBSERVED in the program log, not assumed.
    custody_invocations: u64,
}

impl DepthsV1 {
    /// Attempts across every modelled site, with the Custody-side multiplicity.
    fn modelled_attempts(&self) -> u64 {
        self.root
            + self.seller_replay
            + self.buyer_replay
            + self.caller_authorities.iter().sum::<u64>()
            + self
                .custody_invocations
                .saturating_mul(self.custody_replay + self.custody_transfer_authority)
    }

    /// The distinct ADDRESSES this arm searches and how many times each is searched.
    ///
    /// The Claims caller authority is absent for the same reason the top-level
    /// gate leaves it out of its model: its packet digest is the one seed no
    /// public fixture field carries. It is one more draw at multiplicity one,
    /// and the floor statistic below already absorbs its luckiest attempt.
    fn address_multiplicities(&self) -> Vec<u64> {
        let mut out = vec![1, 1, 1];
        out.extend(self.caller_authorities.iter().map(|_| 1));
        out.push(self.custody_invocations);
        out.push(self.custody_invocations);
        // The Claims caller authority, unmodelled above and real all the same.
        out.push(1);
        out
    }
}

/// One executed seed.
#[derive(Clone, Debug)]
struct ObservationV1 {
    seed: u64,
    units: u64,
    depths: DepthsV1,
}

/// Reproduce this arm's modelled searches from the fixture's own planted state.
///
/// Every seed tuple is read out of an account the fixture installed, and each
/// derived address is checked against the address the fixture reports. A model
/// that addressed something else would still produce a tidy number, so the
/// equality checks are the point.
fn depths(
    direct: &DirectCase,
    releases: Releases,
    arm: ArmV1,
    custody_invocations: u64,
) -> DepthsV1 {
    let chain = &direct.chain;
    let data_of = |key: Pubkey| -> Vec<u8> {
        chain
            .accounts
            .iter()
            .find(|installed| installed.key == key)
            .map(|installed| installed.account.data.clone())
            .unwrap_or_default()
    };

    let core = CoreState::decode(&data_of(chain.market)).expect("the fixture plants a Core state");
    let (market_key, market_bump) = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(core.identity).as_slices(),
        &CORE_PROGRAM_ID,
    );
    assert_eq!(
        market_key, chain.market,
        "the Market identity read out of the planted CoreState does not reproduce the Market \
         address the fixture reports, so this model is measuring a different account",
    );
    assert!(
        core.bumps.market.is_some(),
        "the planted CoreState carries no Market bump, so all three Market readers are back on \
         their search fallback and this arm is measuring a market no widened founding produces. \
         `direct_hot_top_level_margin_gate.rs` owns that assertion in full; this is its echo.",
    );

    let root_bytes = data_of(chain.root);
    let header = CapabilityRootHeaderV1::decode(
        root_bytes
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .expect("the fixture plants a capability root header"),
    )
    .expect("capability root header");
    let (root_key, root_bump) =
        Pubkey::find_program_address(&header.seeds().as_slices(), &TRADING_PROGRAM_ID);
    assert_eq!(
        root_key, chain.root,
        "the Direct root model missed its own address"
    );

    let coordinates = DirectCoordinatesV1::new(chain.market.to_bytes(), core.identity.generation)
        .expect("direct coordinates");
    let mut replays = [0_u64; 2];
    for (index, slot) in replays.iter_mut().enumerate() {
        let maker = chain
            .maker_replays
            .get(index)
            .copied()
            .expect("two maker replays");
        let seeds = MakerReplaySeedsV1::new(
            coordinates,
            direct
                .makers
                .get(index)
                .map(|keypair| keypair.pubkey().to_bytes())
                .expect("two makers"),
        )
        .expect("maker replay seeds");
        let (key, bump) = Pubkey::find_program_address(&seeds.as_slices(), &TRADING_PROGRAM_ID);
        assert_eq!(key, maker, "the maker replay model missed its own address");
        *slot = attempts(bump);
    }

    // One caller authority per LIVE route, derived from that route's own
    // projected child-request digest and checked against the address the
    // fixture states for it.
    let caller_authorities = arm
        .live_route_slots()
        .iter()
        .map(|slot| {
            let route = chain
                .custody_routes
                .get(*slot)
                .copied()
                .expect("four declared Custody routes");
            let seeds = CallerAuthoritySeedsV1::new(
                ContentId::new(releases.release_set).expect("release set"),
                chain.market.to_bytes(),
                ExecutionRoleV1::Trading,
                chain
                    .maker_replays
                    .get(1)
                    .copied()
                    .expect("buyer root")
                    .to_bytes(),
                route.request_digest,
            )
            .expect("caller authority seeds");
            let (key, bump) = Pubkey::find_program_address(&seeds.as_slices(), &TRADING_PROGRAM_ID);
            assert_eq!(
                key, route.authority,
                "the caller-authority model for Custody route slot {slot} missed the address \
                 the fixture reports",
            );
            attempts(bump)
        })
        .collect::<Vec<_>>();

    let replay_seeds = CustodyReplaySeedsV1::new(
        chain.market.to_bytes(),
        releases.release_set,
        CallerRoleV1::Trading,
        chain
            .maker_replays
            .get(1)
            .copied()
            .expect("buyer root")
            .to_bytes(),
    );
    let (replay_key, replay_bump) =
        Pubkey::find_program_address(&replay_seeds.as_slices(), &CUSTODY_PROGRAM_ID);
    assert_eq!(
        replay_key, chain.custody_replay,
        "the Custody replay model missed the address the fixture reports",
    );

    // Through Custody's own exported seed constructor, never a restatement of
    // its three-seed tuple: the domain has one speller and it is the crate that
    // owns it (`6e689907`, and the seam gate refuses a second one).
    let transfer_bump = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::new(chain.market.to_bytes(), releases.release_set).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .1;

    DepthsV1 {
        market: attempts(market_bump),
        root: attempts(root_bump),
        seller_replay: replays.first().copied().unwrap_or_default(),
        buyer_replay: replays.get(1).copied().unwrap_or_default(),
        caller_authorities,
        custody_replay: attempts(replay_bump),
        custody_transfer_authority: attempts(transfer_bump),
        custody_invocations,
    }
}

/// Custody program invocations in one transaction's log.
///
/// The runtime logs `Program <id> invoke [depth]` once per invocation, so this
/// is the chain's own count of how many times the Custody route ran -- not the
/// fixture's model of the enable registers, which is what needed checking.
fn custody_invocations(logs: &[String]) -> u64 {
    let prefix = format!("Program {CUSTODY_PROGRAM_ID} invoke");
    logs.iter().filter(|line| line.starts_with(&prefix)).count() as u64
}

/// Total program invocations in the log, as the variance census counted them.
fn program_invocations(logs: &[String]) -> u64 {
    logs.iter()
        .filter(|line| line.starts_with("Program ") && line.contains(" invoke ["))
        .count() as u64
}

/// The custom program code a refusal carried, when it carried one.
fn refusal_code(error: &BanksClientError) -> Option<u32> {
    match error {
        BanksClientError::TransactionError(TransactionError::InstructionError(
            _,
            InstructionError::Custom(code),
        ))
        | BanksClientError::SimulationError {
            err: TransactionError::InstructionError(_, InstructionError::Custom(code)),
            ..
        } => Some(*code),
        _ => None,
    }
}

/// Whether a refusal is the compute ceiling and not a defect.
fn is_budget_refusal(error: &BanksClientError) -> bool {
    matches!(
        error,
        BanksClientError::TransactionError(TransactionError::InstructionError(
            _,
            InstructionError::ComputationalBudgetExceeded,
        )) | BanksClientError::SimulationError {
            err: TransactionError::InstructionError(
                _,
                InstructionError::ComputationalBudgetExceeded,
            ),
            ..
        }
    )
}

/// One refused seed, with the last of the program log it reached.
///
/// The log tail is retained because a bare `ProgramFailedToComplete` is exactly
/// the refusal that says least: the runtime reports a compute-exhausted program
/// and a program that touched unmapped memory the same way at the transaction
/// level, and those are two entirely different walls with two entirely
/// different fixes. The distinguishing sentence is in the log and nowhere else.
#[derive(Clone, Debug)]
struct RefusalV1 {
    seed: u64,
    code: Option<u32>,
    budget: bool,
    error: String,
    custody_invocations: u64,
    tail: Vec<String>,
}

/// One arm's sweep.
struct SweepV1 {
    arm: ArmV1,
    observations: Vec<ObservationV1>,
    refusals: Vec<RefusalV1>,
}

impl SweepV1 {
    fn best(&self) -> u64 {
        self.observations
            .iter()
            .map(|value| value.units)
            .min()
            .unwrap_or_default()
    }

    fn worst(&self) -> u64 {
        self.observations
            .iter()
            .map(|value| value.units)
            .max()
            .unwrap_or_default()
    }

    fn mean(&self) -> u64 {
        if self.observations.is_empty() {
            return 0;
        }
        self.observations
            .iter()
            .map(|value| value.units)
            .sum::<u64>()
            / self.observations.len() as u64
    }

    /// `CU(seed) - 1,500 * T_known(seed)` over the executed seeds.
    fn residuals(&self) -> Vec<u64> {
        self.observations
            .iter()
            .map(|value| {
                value.units.saturating_sub(
                    ATTEMPT_COST_CU.saturating_mul(value.depths.modelled_attempts()),
                )
            })
            .collect()
    }

    /// The key-independent floor statistic: the minimum residual.
    ///
    /// This equals `C0 + 1,500 * k`, `k` the unmodelled Claims caller
    /// authority's attempt count on the luckiest of the swept draws, and `k = 1`
    /// unless every draw missed on its first candidate.
    fn floor(&self) -> u64 {
        self.residuals().into_iter().min().unwrap_or_default()
    }

    fn residual_spread(&self) -> u64 {
        let residuals = self.residuals();
        let low = residuals.iter().copied().min().unwrap_or_default();
        let high = residuals.iter().copied().max().unwrap_or_default();
        high.saturating_sub(low)
    }
}

/// Sweep one arm across `GATE_SEEDS` fixture draws.
async fn sweep(arm: ArmV1) -> SweepV1 {
    let artifacts = elves();
    let mut observations = Vec::new();
    let mut refusals = Vec::new();

    for seed in 0..GATE_SEEDS {
        let (mut test, direct, instructions, releases) = with_fixture_seed(seed, || {
            let mut test = program_test_without_forced_budget(&artifacts);
            let releases = add_release_waist(&mut test, &artifacts);
            let direct = direct_case_v5(
                &mut test,
                releases,
                &artifacts,
                false,
                false,
                fixture_substrate(),
                DirectOrdinaryGeometryV3::CANONICAL,
                arm.scenario(),
            );
            let instructions = direct_top_level_instructions(&direct);
            (test, direct, instructions, releases)
        });
        assert_eq!(
            instructions[3].program_id, TRADING_PROGRAM_ID,
            "seed {seed}: this file must measure the TOP-LEVEL route",
        );

        let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
        add_lookup_table(&mut test, &addresses);
        let mut context = start_with_substrate(test, fixture_substrate()).await;

        match submit_v0_observed(
            &mut context,
            &instructions,
            addresses,
            Some(&direct.payer),
            &[],
        )
        .await
        {
            Ok(execution) => {
                let custody = custody_invocations(&execution.logs);
                let programs = program_invocations(&execution.logs);
                println!(
                    "SHAPE\t{}\t{seed}\tcustody invocations {custody}\tprogram invocations {programs}",
                    arm.label(),
                );
                observations.push(ObservationV1 {
                    seed,
                    units: execution.compute_units_consumed,
                    depths: depths(&direct, releases, arm, custody),
                });
            }
            Err(refusal) => {
                let budget = is_budget_refusal(&refusal.error)
                    || refusal
                        .logs
                        .iter()
                        .any(|line| line.contains("exceeded CUs meter"));
                let custody = custody_invocations(&refusal.logs);
                println!(
                    "SHAPE\t{}\t{seed}\tREFUSED\tcustody invocations reached {custody}",
                    arm.label(),
                );
                let tail = refusal
                    .logs
                    .iter()
                    .rev()
                    .take(6)
                    .rev()
                    .cloned()
                    .collect::<Vec<_>>();
                refusals.push(RefusalV1 {
                    seed,
                    code: refusal_code(&refusal.error),
                    budget,
                    error: format!("{:?}", refusal.error),
                    custody_invocations: custody,
                    tail,
                });
            }
        }
    }

    SweepV1 {
        arm,
        observations,
        refusals,
    }
}

/// Print one arm's whole story, and return its floor.
fn report(sweep: &SweepV1) -> Option<u64> {
    let label = sweep.arm.label();
    for observation in &sweep.observations {
        let depth = &observation.depths;
        println!(
            "SEEDDEPTH\t{label}\t{}\tmarket {} CARRIED x3\troot {}\treplay {} {}\t\
             caller-authorities {:?}\tcustody-replay {} x{}\tcustody-authority {} x{}\t\
             modelled attempts {}",
            observation.seed,
            depth.market,
            depth.root,
            depth.seller_replay,
            depth.buyer_replay,
            depth.caller_authorities,
            depth.custody_replay,
            depth.custody_invocations,
            depth.custody_transfer_authority,
            depth.custody_invocations,
            depth.modelled_attempts(),
        );
    }
    for observation in &sweep.observations {
        println!(
            "SEEDCU\t{label}\t{}\t{}",
            observation.seed, observation.units
        );
    }
    for refusal in &sweep.refusals {
        println!(
            "SEEDREFUSED\t{label}\t{}\tbudget {}\tcode {:?}\tcustody reached {}\t{}",
            refusal.seed, refusal.budget, refusal.code, refusal.custody_invocations, refusal.error,
        );
        for line in &refusal.tail {
            println!("SEEDREFUSEDLOG\t{label}\t{}\t{line}", refusal.seed);
        }
    }

    if sweep.observations.is_empty() {
        println!(
            "ARM\t{label}\tNO SEED EXECUTED. {} of {GATE_SEEDS} refused; every one of them is a \
             budget refusal: {}. The route does not fit on ANY key draw, so there is no CU \
             figure to report and the deliverable is that sentence.",
            sweep.refusals.len(),
            sweep.refusals.iter().all(|refusal| refusal.budget),
        );
        return None;
    }

    let floor = sweep.floor();
    let implied_c0 = floor.saturating_sub(ATTEMPT_COST_CU);
    let multiplicities = sweep
        .observations
        .first()
        .map(|observation| observation.depths.address_multiplicities())
        .unwrap_or_default();
    let instances: u64 = multiplicities.iter().sum();
    let all_first_try = implied_c0.saturating_add(ATTEMPT_COST_CU.saturating_mul(instances));

    println!(
        "ARM\t{label}\texecuted {}/{GATE_SEEDS}\tbest {}\tworst {}\tmean {}\tband {}\t\
         worst margin {}",
        sweep.observations.len(),
        sweep.best(),
        sweep.worst(),
        sweep.mean(),
        sweep.worst().saturating_sub(sweep.best()),
        PROTOCOL_CEILING.saturating_sub(sweep.worst()),
    );
    println!(
        "FLOOR\t{label}\t{floor}\t(residual spread {}, implied C0 {implied_c0}, \
         {} distinct searched addresses over {instances} search instances, \
         all-first-try {all_first_try})",
        sweep.residual_spread(),
        multiplicities.len(),
    );

    // The HEADROOM, in the only unit this route can spend it: bump attempts.
    //
    // The tail probability itself is deliberately NOT computed here.
    // `direct_hot_top_level_margin_gate.rs` owns that statistic for the
    // zero-fee route and prints it; recomputing it in a second file would be
    // two things to keep true. For the fee-bearing arm there is no tail to
    // compute at all -- see the branch below, which is the whole finding.
    if all_first_try > PROTOCOL_CEILING {
        println!(
            "HEADROOM\t{label}\tNONE, AND NOT AS A MATTER OF LUCK: the route costs \
             {all_first_try} CU with EVERY search landing on its first candidate, which is {} CU \
             past the ceiling of {PROTOCOL_CEILING} before a single participant key is drawn. \
             The tail is not a probability here. No key draw rescues it and no gate constant \
             makes it fit.",
            all_first_try.saturating_sub(PROTOCOL_CEILING),
        );
    } else {
        println!(
            "HEADROOM\t{label}\t{} bump attempts ({} CU) above an all-first-try route, over \
             {} distinct searched addresses. The refusal share this implies is derived in \
             `direct_hot_top_level_margin_gate.rs` and in the evidence doc, not here.",
            PROTOCOL_CEILING.saturating_sub(all_first_try) / ATTEMPT_COST_CU,
            PROTOCOL_CEILING.saturating_sub(all_first_try),
            multiplicities.len(),
        );
    }
    Some(floor)
}

/// Every executed seed's residual sits on the 1,500 CU grid.
fn assert_on_grid(sweep: &SweepV1) {
    let residuals = sweep.residuals();
    let Some(floor) = residuals.iter().copied().min() else {
        return;
    };
    for (index, residual) in residuals.iter().enumerate() {
        let above = residual.saturating_sub(floor);
        let off_grid = above % ATTEMPT_COST_CU;
        assert!(
            off_grid <= 200 || off_grid >= ATTEMPT_COST_CU - 200,
            "{}: seed at index {index} sits {above} CU above the floor, which is {off_grid} CU \
             off the 1,500 CU grid. Every key-dependent cost on this route is supposed to be \
             bump search depth; something now varies with a participant key that is not a \
             search, or the Custody-side multiplicity in DepthsV1 is wrong.",
            sweep.arm.label(),
        );
    }
}

#[tokio::test]
async fn the_fee_bearing_direct_route_executes_two_custody_routes_and_this_is_what_it_costs() {
    let zero = sweep(ArmV1::ZeroFee).await;
    let fee = sweep(ArmV1::FeeBearing).await;

    // The control arm is the accepted route. If it refuses, nothing downstream
    // of it in this file means anything.
    assert!(
        zero.refusals.is_empty(),
        "the zero-fee control arm refused on {} of {GATE_SEEDS} seeds: {:?}. That is the route \
         `direct_hot_top_level_margin_gate.rs` accepts, so this is a broken tree and not a fee \
         measurement.",
        zero.refusals.len(),
        zero.refusals,
    );

    // THE SHAPE, read off the chain. This is the claim the whole file rests on:
    // the fee-bearing scenario really does run the second Custody route, and the
    // zero-fee scenario really does not.
    for observation in &zero.observations {
        assert_eq!(
            observation.depths.custody_invocations, 1,
            "zero-fee seed {}: expected ONE Custody invocation and the log shows {}. Either the \
             fee stopped flooring or the enable registers changed.",
            observation.seed, observation.depths.custody_invocations,
        );
    }
    for observation in &fee.observations {
        assert_eq!(
            observation.depths.custody_invocations, 2,
            "fee-bearing seed {}: expected TWO Custody invocations -- SellerIntermediate then \
             FeeContinuation -- and the log shows {}. If it is 1 the scenario's fee floored \
             after all and this arm is measuring the zero-fee route under another name, which \
             is exactly the defect this file exists to retire.",
            observation.seed, observation.depths.custody_invocations,
        );
    }

    assert_on_grid(&zero);
    assert_on_grid(&fee);

    let zero_floor = report(&zero);
    let fee_floor = report(&fee);

    // A fee-bearing refusal is a MARGIN fact and this file reports it. A refusal
    // with any other cause is a defect and must not be dressed as one.
    for refusal in &fee.refusals {
        assert!(
            refusal.budget,
            "fee-bearing seed {} refused as {:?} rather than on the compute ceiling: {}. That is \
             a broken route or a broken fixture -- the fee leg's accounts, its allowance \
             arithmetic or its replay revision step -- and reporting it as a margin result would \
             be a lie about what was measured. Log tail: {:?}",
            refusal.seed, refusal.code, refusal.error, refusal.tail,
        );
    }

    if let (Some(zero_floor), Some(fee_floor)) = (zero_floor, fee_floor) {
        let delta = fee_floor.saturating_sub(zero_floor);
        println!(
            "FEELEG\tthe second Custody route's KEY-INDEPENDENT cost is {delta} CU \
             ({zero_floor} -> {fee_floor}). Both floors are C0 plus one bump attempt for the \
             unmodelled Claims caller authority, so the difference is the code's constant part \
             and nothing about these keys. It is measured on ONE ELF SET and one substrate; a \
             source change to any of the five roles redraws every depth in both arms but not \
             this difference.",
        );
    }

    println!(
        "SUMMARY\tzero-fee worst {} of {PROTOCOL_CEILING} over {} executed seeds; fee-bearing \
         {} executed and {} refused at the meter, so it has no worst -- the number it does not \
         have is measured in \
         `the_fee_bearing_route_exceeds_the_ceiling_by_at_least_this_much`.",
        zero.worst(),
        zero.observations.len(),
        fee.observations.len(),
        fee.refusals.len(),
    );
}

// ---------------------------------------------------------------------------
// WHAT THE FEE-BEARING ROUTE COSTS, when it cannot be run to completion.
//
// The sweep above says the route exceeds the meter. It cannot say by how much,
// and the difference matters enormously: a route 5,000 CU over is one lane's
// work on the fee leg, and a route 150,000 over is a lifecycle decision. So the
// number has to come from somewhere.
//
// TWO WAYS THAT DO NOT WORK, both tried and both recorded so the next lane does
// not spend the afternoon again:
//
// * Ask for a bigger budget. 1,400,000 is the runtime's maximum; a
//   `SetComputeUnitLimit` above it is clamped, so there is nothing to request.
// * Lift the meter under the harness with `ProgramTest::set_compute_max_units`.
//   That replaces the whole `ComputeBudget`, which resets the heap to the
//   protocol default of 32 KiB. Submitting WITH the heap-frame instruction then
//   lets `admit_heap_frame_v1` read the 64 KiB grant out of the instructions
//   sysvar and lift the allocator ceiling past what the runtime mapped:
//   measured, every seed dies at `Access violation writing 8 bytes at address
//   0x30000fa58`, which is heap offset 64,088. Submitting WITHOUT it refuses as
//   `TradingSbfError::HeapFrame` (0x4008) at 47,835 CU, because this route
//   declares an extended heap profile and refuses rather than allocating until
//   it dies. There is no third door: the route needs 64 KiB and the lifted
//   meter cannot grant it.
//
// So the cost is assembled from the program log, which reports the compute
// meter at every invocation boundary and does so whether or not the transaction
// completes. `Program <id> consumed X of Y` gives both the invocation's cost
// and the meter remaining when it started, so the transaction's consumption at
// any child boundary is exactly `budget - Y`.
//
// The decomposition, per seed:
//
//   A       everything before the first Custody CPI          = budget - Y(c1)
//   C       the Custody CPIs and the Trading work between    (measured)
//   P       everything after the last Custody CPI returns    (measured on the
//                                                             ZERO-FEE arm)
//
// The zero-fee arm runs to completion, so `A`, `C` and `P` are all measured
// there and `A + C + P` reproduces its total exactly -- which this test ASSERTS,
// because a decomposition that does not add up is not a measurement.
//
// The fee-bearing arm runs as far as its second Custody CPI's return and dies
// after it, so `A` and `C` are measured there too and only `P` is not. The
// figure reported is `A_fee + C_fee + P_zero`, and it is a LOWER BOUND: the fee
// arm's commit phase writes one more child's poststate than the zero-fee arm's,
// so its real `P` is at least `P_zero`. Stated as a bound rather than as an
// estimate, because the conclusion it supports -- the route does not fit -- is
// one a bound is enough for and an estimate would only weaken.
// ---------------------------------------------------------------------------

/// The fee-bearing route's key-independent cost, as a measured LOWER BOUND.
///
/// Measured at **1,435,274 CU** by the decomposition below, on five role ELFs
/// built from this tree. It is the same residual statistic the sweep's floor is
/// -- `total - 1,500 * modelled attempts` -- so it is a property of the code and
/// not of a key draw, and it is comparable across trees the way a worst-seed
/// sample is not.
///
/// # It fell 66,229 CU, and none of that is the fee leg
///
/// It read 1,501,503 before decision 0017's option B took the two Registry
/// reauthentication CPIs and the third cache decode off the TOP-LEVEL arm, which
/// both arms of this file enter through. The zero-fee arm of the same run fell
/// 1,319,672 -> 1,252,751, so the implied fee leg went 182,386 -> 182,523: the
/// same leg to within 137 CU, on ELFs whose every bump draw was redrawn. The
/// saving is entirely upstream of the fee work and the fee work is unchanged.
///
/// What it buys is real and does not close the question. This route is still
/// over the protocol ceiling -- by 35,274 CU rather than 101,503 -- so 0017's
/// option B moved the fee-bearing shape from two thirds of a route away from
/// fitting to about a quarter. It does not fit. The variance census's
/// 1.49-1.52M two-Custody shape is still the largest thing on it and still
/// nobody's.
///
/// It earned that claim rather than asserting it. Measured on two different ELF
/// sets -- before and after a rebase onto four commits of main -- this floor
/// read 1,501,294 and 1,501,503, moving **209 CU**, which is the same 209 the
/// zero-fee arm moved by over the same rebase. The fee leg the two differences
/// imply is 182,386 CU on both, identical to the compute unit.
///
/// One bump attempt of slack above the measurement, the smallest unit this
/// route can spend, for the same reason `TOP_LEVEL_KEY_INDEPENDENT_CU_V1` takes
/// it: a change costing less than a single PDA search should not go red here.
///
/// Raising it is not "spending margin" the way raising the zero-fee gate's
/// constant is, because this route HAS no margin -- it is 120,000 CU over the
/// ceiling. Raising it is recording that the thing already too expensive to run
/// got more expensive. Do that with a sentence saying why.
const FEE_BEARING_LOWER_BOUND_CU_V1: u64 = 1_436_774;

/// Seeds decomposed. Eight is enough for a floor and the parse is the cost.
const DECOMPOSITION_SEEDS: u64 = 8;

/// One program invocation the log reported, with the meter at its boundaries.
#[derive(Clone, Debug)]
struct InvocationV1 {
    program: String,
    depth: u32,
    /// The compute meter remaining when this invocation started.
    remaining_at_entry: u64,
    /// What this invocation and everything below it consumed.
    consumed: u64,
}

/// Parse the runtime's stable log into invocation records.
///
/// The runtime prints `Program <id> invoke [depth]` on entry and `Program <id>
/// consumed X of Y compute units` immediately before the matching `success` or
/// `failed`, so a stack of open invocations pairs them without ambiguity even
/// though Custody's own Token CPI nests inside.
fn invocations(logs: &[String]) -> Vec<InvocationV1> {
    let mut open: Vec<(String, u32)> = Vec::new();
    let mut out: Vec<InvocationV1> = Vec::new();
    for line in logs {
        let Some(rest) = line.strip_prefix("Program ") else {
            continue;
        };
        let mut words = rest.split_whitespace();
        let Some(program) = words.next() else {
            continue;
        };
        match words.next() {
            Some("invoke") => {
                let depth = words
                    .next()
                    .and_then(|token| token.trim_matches(['[', ']']).parse::<u32>().ok())
                    .unwrap_or(0);
                open.push((program.to_owned(), depth));
            }
            Some("consumed") => {
                // `consumed X of Y compute units`
                let consumed = words.next().and_then(|token| token.parse::<u64>().ok());
                let remaining = words.nth(1).and_then(|token| token.parse::<u64>().ok());
                if let (Some(consumed), Some(remaining), Some((id, depth))) =
                    (consumed, remaining, open.last().cloned())
                    && id == program
                {
                    out.push(InvocationV1 {
                        program: id,
                        depth,
                        remaining_at_entry: remaining,
                        consumed,
                    });
                }
            }
            Some("success" | "failed:")
                if open.last().map(|(id, _)| id.as_str()) == Some(program) =>
            {
                open.pop();
            }
            _ => {}
        }
    }
    out
}

/// One seed's decomposition into before-Custody, Custody, and after-Custody.
#[derive(Clone, Copy, Debug)]
struct PartsV1 {
    /// Trading's whole allotment, the meter it started with.
    budget: u64,
    /// Consumption up to the first Custody CPI's entry.
    before: u64,
    /// The Custody CPIs plus the Trading work between and around them, up to
    /// the last Custody CPI's return.
    custody_span: u64,
    /// Consumption after the last Custody CPI returned. `None` when the
    /// transaction did not survive to report it.
    after: Option<u64>,
    /// Whether the LAST Custody CPI returned rather than dying at the meter.
    ///
    /// When it died the span above is truncated at the meter, so a bound built
    /// on it is looser than one built on a seed whose Custody leg completed --
    /// still a bound, and worth marking so nobody reads the two as equally
    /// tight.
    last_custody_completed: bool,
}

impl PartsV1 {
    /// Consumption reached by the last Custody CPI's return.
    const fn reached(self) -> u64 {
        self.before + self.custody_span
    }
}

/// Decompose one transaction's log around its Custody CPIs.
fn parts(logs: &[String], custody: &str, trading: &str) -> Option<PartsV1> {
    let records = invocations(logs);
    let top = records
        .iter()
        .find(|record| record.depth == 1 && record.program == trading)?;
    let budget = top.remaining_at_entry;
    let custody_calls = records
        .iter()
        .filter(|record| record.depth == 2 && record.program == custody)
        .collect::<Vec<_>>();
    let first = custody_calls.first()?;
    let last = custody_calls.last()?;
    let before = budget.saturating_sub(first.remaining_at_entry);
    let end_of_last = last.remaining_at_entry.saturating_sub(last.consumed);
    let custody_span = first.remaining_at_entry.saturating_sub(end_of_last);
    // Trading's own reported consumption is the whole invocation, so the tail
    // is only meaningful when the invocation actually completed. A budget death
    // reports the meter, not the work, so `after` is None there.
    let after = if top.consumed < budget.saturating_sub(1_000) {
        Some(top.consumed.saturating_sub(before + custody_span))
    } else {
        None
    };
    // A CPI is handed the remaining meter, so a Custody leg that RETURNED left
    // meter behind and one that died at the meter left about eight units.
    let last_custody_completed = end_of_last > 1_000;
    Some(PartsV1 {
        budget,
        before,
        custody_span,
        after,
        last_custody_completed,
    })
}

/// Run one seed of one arm and return its log and its outcome.
async fn one_seed(arm: ArmV1, seed: u64) -> (Vec<String>, Option<u64>, DepthsV1) {
    let artifacts = elves();
    let (mut test, direct, instructions, releases) = with_fixture_seed(seed, || {
        let mut test = program_test_without_forced_budget(&artifacts);
        let releases = add_release_waist(&mut test, &artifacts);
        let direct = direct_case_v5(
            &mut test,
            releases,
            &artifacts,
            false,
            false,
            fixture_substrate(),
            DirectOrdinaryGeometryV3::CANONICAL,
            arm.scenario(),
        );
        let instructions = direct_top_level_instructions(&direct);
        (test, direct, instructions, releases)
    });
    let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;
    match submit_v0_observed(
        &mut context,
        &instructions,
        addresses,
        Some(&direct.payer),
        &[],
    )
    .await
    {
        Ok(execution) => {
            let custody = custody_invocations(&execution.logs);
            let depth = depths(&direct, releases, arm, custody);
            (
                execution.logs,
                Some(execution.compute_units_consumed),
                depth,
            )
        }
        Err(refusal) => {
            let custody = custody_invocations(&refusal.logs);
            let depth = depths(&direct, releases, arm, custody.max(1));
            (refusal.logs, None, depth)
        }
    }
}

#[tokio::test]
async fn the_fee_bearing_route_exceeds_the_ceiling_by_at_least_this_much() {
    let custody = CUSTODY_PROGRAM_ID.to_string();
    let trading = TRADING_PROGRAM_ID.to_string();

    let mut zero_after: Vec<u64> = Vec::new();
    let mut zero_totals: Vec<u64> = Vec::new();
    let mut zero_floors: Vec<u64> = Vec::new();
    let mut zero_outside: Vec<u64> = Vec::new();
    let mut fee_bounds: Vec<(u64, u64, u64)> = Vec::new();

    for seed in 0..DECOMPOSITION_SEEDS {
        let (logs, total, depth) = one_seed(ArmV1::ZeroFee, seed).await;
        let parts =
            parts(&logs, &custody, &trading).expect("the zero-fee log names its Custody CPI");
        let total = total.expect("the zero-fee arm executes");
        let after = parts
            .after
            .expect("the zero-fee arm completed, so its tail is measured");
        // The decomposition must reconstruct the transaction, or it is a story
        // about a log rather than a measurement of a route. Trading's own
        // consumption plus the two ComputeBudget instructions and the Ed25519
        // precompile is the whole transaction, and that remainder must be a
        // small CONSTANT -- if it drifts with the seed, something outside
        // Trading is key-dependent and this decomposition is not closed.
        let trading_total = parts.before + parts.custody_span + after;
        let outside = total.saturating_sub(trading_total);
        assert!(
            trading_total <= total && outside < 10_000,
            "seed {seed}: Trading's decomposed consumption {trading_total} against a \
             transaction total of {total} leaves {outside} CU outside Trading, which is too \
             much to be the precompile and the two ComputeBudget instructions. The log parse \
             is wrong or something else in the transaction is doing work.",
        );
        println!(
            "PARTS\tzero-fee\t{seed}\tbudget {}\tbefore {}\tcustody {}\tafter {after}\t\
             trading total {trading_total}\ttx total {total}\toutside Trading {outside}\t\
             modelled attempts {}",
            parts.budget,
            parts.before,
            parts.custody_span,
            depth.modelled_attempts(),
        );
        zero_after.push(after);
        zero_totals.push(total);
        zero_outside.push(outside);
        zero_floors
            .push(total.saturating_sub(ATTEMPT_COST_CU.saturating_mul(depth.modelled_attempts())));
    }
    let outside = zero_outside.iter().copied().max().unwrap_or_default();

    // The tail is the same work on every draw: no bump search happens after the
    // last child returns, so this is a constant and not a distribution.
    let after_low = zero_after.iter().copied().min().unwrap_or_default();
    let after_high = zero_after.iter().copied().max().unwrap_or_default();
    println!(
        "TAILWORK\tafter the last Custody CPI the zero-fee route spends {after_low} to \
         {after_high} CU across {DECOMPOSITION_SEEDS} draws"
    );

    for seed in 0..DECOMPOSITION_SEEDS {
        let (logs, total, depth) = one_seed(ArmV1::FeeBearing, seed).await;
        assert!(
            total.is_none(),
            "the fee-bearing arm EXECUTED at seed {seed} ({total:?} CU). That is the good news \
             this file was written to be able to report, and the decomposition below is then \
             unnecessary -- use the sweep's own figure.",
        );
        let Some(parts) = parts(&logs, &custody, &trading) else {
            println!("PARTS\tfee-bearing\t{seed}\tthe log names no Custody CPI");
            continue;
        };
        // A TRANSACTION total, because the ceiling is a transaction ceiling:
        // Trading's decomposed consumption plus the measured constant the two
        // ComputeBudget instructions and the Ed25519 precompile cost outside it.
        let bound = parts
            .reached()
            .saturating_add(after_low)
            .saturating_add(outside);
        println!(
            "PARTS\tfee-bearing\t{seed}\tbudget {}\tbefore {}\tcustody {}\treached {}\t\
             second Custody leg {}\tLOWER BOUND {bound}\tover the ceiling by at least {}\t\
             modelled attempts {}",
            parts.budget,
            parts.before,
            parts.custody_span,
            parts.reached(),
            if parts.last_custody_completed {
                "returned"
            } else {
                "died at the meter, so this row's bound is the loosest kind"
            },
            bound.saturating_sub(PROTOCOL_CEILING),
            depth.modelled_attempts(),
        );
        // ONLY seeds whose second Custody leg RETURNED enter the floor.
        //
        // A seed whose Custody leg died at the meter has a `reached` that is the
        // METER and not a measurement -- capped at the budget, while its
        // modelled attempts are subtracted in full. That produces an
        // artificially LOW residual and it silently captures the minimum, which
        // is exactly what happened when this file was first written: the floor
        // it reported came from a truncated seed and moved 4,500 CU on a rebase
        // that cost the route 209. Computed over completed seeds only, the same
        // rebase moves this floor by 209 as well -- the same 209 the zero-fee
        // arm moves by -- and the fee leg below reproduces to the compute unit
        // across both ELF sets. The truncated rows are still printed; they are
        // just not evidence about a constant.
        if parts.last_custody_completed {
            fee_bounds.push((seed, bound, depth.modelled_attempts()));
        }
    }

    assert!(
        !fee_bounds.is_empty(),
        "no fee-bearing seed's second Custody CPI RETURNED, so every row is truncated at the \
         meter and there is no honest floor to take. The route got expensive enough that it \
         now dies inside the fee leg on every draw; decompose it with more seeds, or read the \
         per-seed `reached` figures above as the only thing that is still measured.",
    );

    // The key-independent form of the bound, so it is comparable across trees
    // the way the sweep's floor statistic is.
    let floor = fee_bounds
        .iter()
        .map(|(_, bound, attempts)| bound.saturating_sub(ATTEMPT_COST_CU.saturating_mul(*attempts)))
        .min()
        .unwrap_or_default();
    let worst = fee_bounds
        .iter()
        .map(|(_, bound, _)| *bound)
        .max()
        .unwrap_or_default();
    let best = fee_bounds
        .iter()
        .map(|(_, bound, _)| *bound)
        .min()
        .unwrap_or_default();
    let zero_worst = zero_totals.iter().copied().max().unwrap_or_default();
    let zero_floor = zero_floors.iter().copied().min().unwrap_or_default();
    println!(
        "BOUND\tthe fee-bearing Direct route costs AT LEAST {best} to {worst} CU across \
         {DECOMPOSITION_SEEDS} draws, against a ceiling of {PROTOCOL_CEILING} -- over by at \
         least {} to {}. The zero-fee route on the same ELFs, keys and substrate reached \
         {zero_worst} at its worst of the same draws. Key-independent lower bound (the same \
         residual statistic the sweep reports): {floor}.",
        best.saturating_sub(PROTOCOL_CEILING),
        worst.saturating_sub(PROTOCOL_CEILING),
    );
    println!(
        "FEELEG\tthe second Custody route's KEY-INDEPENDENT cost is AT LEAST {} CU \
         ({zero_floor} -> {floor} over the same {DECOMPOSITION_SEEDS} draws, same ELFs, same \
         substrate, same keys). Both statistics are `total - 1,500 * modelled attempts`, so \
         the difference is the code's constant part and nothing about these keys. It is a \
         BOUND and not a measurement of the whole leg, because the fee arm's commit phase \
         never ran: the tail added to it is the ZERO-FEE tail, which writes one child's \
         poststate where the fee route writes two.",
        floor.saturating_sub(zero_floor),
    );
    assert!(
        floor <= FEE_BEARING_LOWER_BOUND_CU_V1,
        "the fee-bearing Direct route's key-independent cost is now AT LEAST {floor} CU, past \
         the {FEE_BEARING_LOWER_BOUND_CU_V1} recorded here. This statistic does not move when \
         keys move, so this red is a CODE change and the fee leg just got more expensive -- on \
         a route that is already {} CU over the ceiling and cannot afford it. The zero-fee arm \
         of the same run read {zero_floor}.",
        floor.saturating_sub(PROTOCOL_CEILING),
    );
}
