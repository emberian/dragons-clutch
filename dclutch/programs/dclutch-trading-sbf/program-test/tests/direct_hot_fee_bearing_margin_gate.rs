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
//! # The shape this file measures CHANGED, and the header says so
//!
//! When this file was written, a fee-bearing fill dispatched TWO Custody CPIs
//! and the pair did not fit: the measured key-independent floor was a LOWER
//! BOUND of 1,435,274 CU against a 1,400,000 ceiling, and the decomposition test
//! that produced it existed only because a route over the meter reports the
//! meter and cannot report itself.
//!
//! `docs/design/FEE_SECOND_TRANSACTION_V1.md` moved the fee leg into a
//! transaction of its own. The transition pins
//! `SCALAR_FEE_CONTINUATION_ROUTE_ENABLED_V3` to zero, so a fee-bearing fill now
//! dispatches ONE Custody CPI -- `SellerIntermediate`, non-terminal, leaving the
//! residual delegation standing -- and it fits. That retired the decomposition
//! test, which had written its own retirement condition into an assertion:
//! *"the fee-bearing arm EXECUTED at seed N. That is the good news this file was
//! written to be able to report, and the decomposition below is then
//! unnecessary -- use the sweep's own figure."* It is deleted rather than left
//! passing on a premise that is false.
//!
//! What this file still owns is the CONTROLLED PAIR. The two arms differ in a
//! host-side fixture input and in nothing else, so their difference is the
//! seller leg's non-terminal-versus-terminal cost and nothing about keys.
//! `direct_hot_fee_pair.rs` owns the other half -- the second transaction, its
//! ledger and its refusals -- and neither file measures the other's.
//!
//! # What is asserted and what is printed
//!
//! Asserted, because each is a property of the code:
//!
//! * BOTH arms invoke Custody exactly ONCE, on every seed, read out of the
//!   program log rather than inferred from the fixture's own arithmetic. The
//!   arms are no longer told apart by a route COUNT, so the assertions say which
//!   count each must have rather than that the two differ;
//! * every seed's residual sits on the 1,500 CU bump-search grid, so the site
//!   census below is known to be stale rather than quietly wrong the day
//!   something key-dependent that is not a search appears;
//! * NEITHER arm refuses. A fee-bearing refusal used to be a margin fact this
//!   file reported; it is a defect now, and reporting it as a margin result
//!   would be a lie about a route that fits;
//! * the fee-bearing arm's KEY-INDEPENDENT floor has not regressed past
//!   [`FEE_BEARING_FLOOR_CU_V1`].
//!
//! Printed and NOT asserted: the worst observed margin. Fitting is a question
//! about a stranger's keys, which is a geometric distribution and not a
//! constant -- the margin gate's own argument.

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
    /// nonzero fee enables slot 1 alone. This is the fixture's model of the
    /// transition's own `select_zero` arithmetic and it is CHECKED against the
    /// chain: the Custody invocation count in the program log must equal this
    /// slice's length on every seed.
    ///
    /// **This read `&[1, 2]` until the fee leg left the transaction.** The
    /// transition now pins `SCALAR_FEE_CONTINUATION_ROUTE_ENABLED_V3` to zero,
    /// so a fee-bearing fill emits ONE Custody CPI and leaves the residual
    /// delegation standing for the second transaction. What separates the two
    /// arms is therefore no longer a route COUNT: it is which slot runs and
    /// whether the delegation survives it. Zero fee takes terminal slot 0 and
    /// closes the delegation; a banded fee takes non-terminal slot 1 and leaves
    /// `combined_fee` behind. `direct_hot_fee_pair.rs` spends that residue.
    const fn live_route_slots(self) -> &'static [usize] {
        match self {
            Self::ZeroFee => &[0],
            Self::FeeBearing => &[1],
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
    /// The Claims caller authority, modelled since 2026-09-02.
    ///
    /// It was the last search left inside the residual, and leaving it there is
    /// what let this file's floor move 4,836 CU between two builds that compile
    /// to byte-identical code -- 948 symbols, 941 stack frames, none differing.
    /// A relink reseeds it, the floor is a MINIMUM over draws that include it,
    /// and the minimum of a resampled distribution moves when nothing it
    /// measures does.
    claims_caller_authority: u64,
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
            + self.claims_caller_authority
    }

    /// The distinct ADDRESSES this arm searches and how many times each is searched.
    ///
    /// The Claims caller authority is among them since 2026-09-02.
    ///
    /// The sentence that stood here -- "its packet digest is the one seed no
    /// public fixture field carries" -- was never true, and it is the reason the
    /// search went sixteen days unsubtracted. Every seed it needs is a value
    /// `claims_caller_authority_v5` already computes to install the account; the
    /// fixture simply discarded the bump. It now reports it.
    fn address_multiplicities(&self) -> Vec<u64> {
        let mut out = vec![1, 1, 1];
        out.extend(self.caller_authorities.iter().map(|_| 1));
        out.push(self.custody_invocations);
        out.push(self.custody_invocations);
        // The Claims caller authority, modelled above and subtracted with the rest.
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

    // The one search this file used to leave in the residual.
    //
    // Every OTHER site here re-derives its address and asserts it against the one
    // the fixture reports, because a model that addressed something else would
    // still produce a tidy number. This one cannot do that and does not pretend
    // to: its guarantee is stronger and of a different kind. The fixture derives
    // it ONCE in `claims_caller_authority_v5` and that single call both installs
    // the account and reports the bump, so there is no second derivation to
    // disagree. The check below is only that the fixture handed over something.
    let (claims_authority_key, claims_authority_bump) = chain
        .claims_caller_authority
        .expect("the ordinary Direct chain dispatches a Claims child and reports its authority");
    assert_ne!(
        claims_authority_key,
        Pubkey::default(),
        "the Claims caller-authority model was handed a zero address",
    );

    DepthsV1 {
        market: attempts(market_bump),
        root: attempts(root_bump),
        seller_replay: replays.first().copied().unwrap_or_default(),
        buyer_replay: replays.get(1).copied().unwrap_or_default(),
        caller_authorities,
        custody_replay: attempts(replay_bump),
        custody_transfer_authority: attempts(transfer_bump),
        custody_invocations,
        claims_caller_authority: attempts(claims_authority_bump),
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
    /// Since 2026-09-02 this is `C0` and nothing else, because the model has no
    /// unmodelled search left in it. It used to be `C0 + 1,500 * k` with `k` the
    /// Claims caller authority's attempt count on the luckiest of the swept
    /// draws -- and a MINIMUM OVER A LOTTERY IS NOT A CONSTANT. Every unmodelled
    /// search is reseeded by `release_set_id`, so a relink resamples the whole
    /// distribution, and the minimum of a resampled distribution moves even when
    /// nothing it measures does. Measured across three relink pairs whose
    /// Trading ELF alone differs: 0 CU, +17 CU, and +4,836 CU, the last between
    /// two builds with 948 identical symbols and 941 identical stack frames.
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
             claims-caller-authority {}\tmodelled attempts {}",
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
            depth.claims_caller_authority,
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
async fn the_fee_bearing_direct_route_fits_now_that_the_fee_leg_left_it() {
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
            observation.depths.custody_invocations, 1,
            "fee-bearing seed {}: expected ONE Custody invocation -- SellerIntermediate, \
             non-terminal, leaving the residual delegation for the second transaction -- and \
             the log shows {}. Two means the fee continuation register came unpinned and the \
             fee leg is back inside the fill, which is the shape that does not fit.",
            observation.seed, observation.depths.custody_invocations,
        );
    }
    // The two arms run the same NUMBER of Custody routes now, so a count can no
    // longer tell them apart. What still must differ is the leg's SHAPE, and
    // that shows up in the floors below rather than in the log.
    assert_eq!(
        ArmV1::ZeroFee.live_route_slots().len(),
        ArmV1::FeeBearing.live_route_slots().len(),
        "the arms' modelled route counts must match what the chain showed above",
    );

    assert_on_grid(&zero);
    assert_on_grid(&fee);

    let zero_floor = report(&zero);
    let fee_floor = report(&fee);

    // A fee-bearing refusal used to be a MARGIN fact this file reported. It is a
    // defect now: the route this arm measures fits, and the only way it can
    // refuse is that something about the fee-bearing shape is broken.
    assert!(
        fee.refusals.is_empty(),
        "the fee-bearing arm refused on {} of {GATE_SEEDS} seeds: {:?}. Since the fee leg moved \
         into a second transaction this arm dispatches ONE Custody route, exactly as the \
         zero-fee arm does, and it is nowhere near the ceiling -- so a refusal here is a broken \
         route or a broken fixture and never a margin fact. A BUDGET refusal in particular \
         means the fee-bearing shape got a hundred thousand CU more expensive, which nothing in \
         this file's model can explain.",
        fee.refusals.len(),
        fee.refusals,
    );

    let (Some(zero_floor), Some(fee_floor)) = (zero_floor, fee_floor) else {
        panic!("both arms executed on every seed, so both floors exist");
    };
    println!(
        "FEELEG\tthe fee-bearing fill's KEY-INDEPENDENT floor is {fee_floor} against the \
         zero-fee arm's {zero_floor} on the same ELFs, keys and substrate. Both arms dispatch \
         ONE Custody route; what differs is the seller leg's non-terminal shape -- six patched \
         scalars in a request of identical width, a delegation left standing instead of \
         revoked, and the `fee_owed` the Effect writes into the buyer's replay. Both floors are \
         C0 plus one bump attempt for the unmodelled Claims caller authority, so this \
         difference is the code's constant part and nothing about these keys.",
    );
    println!(
        "SUMMARY\tzero-fee worst {} and fee-bearing worst {} of {PROTOCOL_CEILING}, over {} and \
         {} executed seeds. Section 4.3 of the design predicted this: tx1 returns to the \
         zero-fee cost profile, because it keeps the shipped `SellerIntermediate` route, which \
         differs from `SellerTerminal` in six patched scalars of an identical-width request.",
        zero.worst(),
        fee.worst(),
        zero.observations.len(),
        fee.observations.len(),
    );

    // The ratchet. Same residual statistic the zero-fee gate takes, for the same
    // reason: it does not move when keys move, so a red here is a CODE change.
    assert!(
        fee_floor <= FEE_BEARING_FLOOR_CU_V1,
        "the fee-bearing Direct fill's key-independent floor is now {fee_floor} CU, past the \
         {FEE_BEARING_FLOOR_CU_V1} recorded here. This statistic does not move when keys move, \
         so this red is a code change and the fee-bearing fill just got more expensive. The \
         zero-fee arm of the same run read {zero_floor}; if BOTH moved by the same amount the \
         change is upstream of the fee work and belongs in `direct_hot_top_level_margin_gate`'s \
         constant too.",
    );
}

/// The fee-bearing fill's key-independent floor, plus one bump attempt.
///
/// `min over seeds of (CU(seed) - 1500 * modelled attempts)` -- the same
/// residual the sweep reports and the same one
/// `direct_hot_top_level_margin_gate.rs` takes, for the reason that file gives
/// at length: a source change to any role redraws `release_set_id` and every
/// bump search under it, so two worst-seed figures across that boundary are not
/// comparable and this is.
///
/// It replaces `FEE_BEARING_LOWER_BOUND_CU_V1`, which was a LOWER BOUND
/// assembled from a program log because the route could not be run to
/// completion. That constant read 1,436,774 and bounded a route over the
/// protocol ceiling by at least 35,274 CU. This one measures a route that lands.
///
/// The slack is one bump attempt, the smallest unit this route can spend, for
/// the same reason `TOP_LEVEL_KEY_INDEPENDENT_CU_V1` takes it: a change costing
/// less than a single PDA search should not go red here.
const FEE_BEARING_FLOOR_CU_V1: u64 = FEE_BEARING_MEASURED_FLOOR_CU_V1 + ATTEMPT_COST_CU;

/// The measured value the constant above takes its slack from.
///
/// 2026-08-31, thirty-two seeds, `Immutable` substrate, five role ELFs built
/// from this tree: **32/32 executed**, best 1,271,994, worst 1,295,997, mean
/// 1,280,672, band 24,003, worst margin against the ceiling **104,003 CU**.
///
/// The zero-fee arm of the same run floored at 1,263,125, so the fee-bearing
/// fill's key-independent cost is **131 CU BELOW** the zero-fee one -- less than
/// a tenth of one bump attempt, which is to say the two shapes are the same
/// route. That is the design's section 4.3 prediction stated as a measurement.
/// The same 131 was measured on the pre-merge tree, on a different ELF set with
/// every bump depth redrawn, which is what a key-independent statistic is for.
///
/// Before the split this statistic was a LOWER BOUND of 1,435,274 on a route
/// that could not be run to completion, 35,274 over the ceiling.
///
/// # 2026-08-31, re-measured: 1,262,994 -> 1,269,919
///
/// The assertion above predicted its own diagnosis -- "if BOTH moved by the same
/// amount the change is upstream of the fee work" -- and that is exactly what
/// happened, so this constant moves for a reason that has nothing to do with
/// fees. Measured on this sweep at five points along one ancestor chain, the two
/// arms moved IDENTICALLY at every point and the 131 CU gap between them held to
/// the unit throughout, which is what a key-independent statistic is supposed to
/// do and the strongest evidence that both are measuring the same route.
///
/// The cost is the `basis:` lane's `authenticate_product_basis_v3`, which this
/// arm reaches by the same two call sites the zero-fee arm does. The full
/// itemisation is in `direct_hot_top_level_margin_gate.rs` on
/// `TOP_LEVEL_KEY_INDEPENDENT_CU_V1` and is not restated here -- one measurement,
/// one place, per this repository's rule about a value duplicated instead of
/// read. The short version: `ProductBasisV3::decode` was rewritten and runs four
/// times per trade, a price-gate digest probe was added and runs twice, and
/// `docs/design/BASIS_ABI_UNIFICATION_V1.md` says the hot path gains exactly zero
/// CU, which this sweep falsifies by about 4,500.
///
/// At the BASIS founding-hoist correction, both arms got cheaper together as
/// this file predicted. On the same five ELFs and 32 seeds the fee-bearing
/// floor moved **1,269,919 -> 1,266,429** (-3,490), while the zero-fee arm read
/// 1,266,559. Both arms executed 32/32; their 130-CU gap remains less than a
/// tenth of one bump attempt and the saving is upstream of fee work.
///
/// # 2026-09-01: measured 1,292,895, and DELIBERATELY NOT PINNED
///
/// The assertion above predicted its own diagnosis for the second time. Over
/// the overnight completion wave (`5b6a5849..371409f4`) this arm moved
/// 1,266,429 -> 1,292,895 and the zero-fee arm of the same run moved
/// 1,266,559 -> 1,293,025: **the same +26,466 on both**, with the 130 CU
/// between them intact to the unit. So the cause is upstream of fee work
/// again, and by this file's rule the measurement lives in one place --
/// `direct_hot_top_level_margin_gate.rs` on `TOP_LEVEL_KEY_INDEPENDENT_CU_V1`,
/// which carries the bisected commit, the per-phase attribution and the reason
/// neither constant moves. Both arms still executed 32/32.
/// # 2026-09-02: PINNED AGAIN, on a statistic that is finally a constant
///
/// The four entries above record this number moving and refusing to be pinned,
/// three times, because a minimum over a lottery is not a constant. It is one
/// now: `d43cc47c` subtracts the Claims caller authority, the last search left
/// in the residual, and the residual stops wandering.
///
/// ```text
///                       floor before -> after        residual spread before -> after
///   fee-pair-elves       1,299,128 -> 1,297,628            4,502 -> 142
///   settle-elves         1,299,145 -> 1,297,645            9,000 ->  91
///   lot-X                1,299,292 -> 1,297,792           10,502 ->   2
///   lot-Y                1,299,292 -> 1,297,792            7,502 ->   2
/// ```
///
/// Every floor falls by EXACTLY 1,500 -- the luckiest of thirty-two draws was
/// one attempt on all four sets, which the old comment assumed and could not
/// guarantee -- and the residual becomes two compute units across thirty-two key
/// draws where it used to span ten thousand.
///
/// **The null pair reads zero.** `lot-X` and `lot-Y` differ by forty comment
/// lines prepended to one module and nothing else, and they now report the SAME
/// floor on both arms. They are the canonical set this constant is taken from,
/// for exactly that reason.
///
/// **The residual noise is +/-124 and it is not a search.** The real-change pair
/// (`fee-pair-elves` to `settle-elves`, a code change confined to `DCLTDFS1`)
/// reads +17 with per-seed deltas from -124 to +106. Those are NOT on the 1,500
/// grid, so no bump census can subtract them: they are data-dependent
/// instruction count, and a floor over thirty-two seeds already absorbs them.
/// Read this constant with that width in mind and do not chase a hundred CU.
const FEE_BEARING_MEASURED_FLOOR_CU_V1: u64 = 1_297_792;
