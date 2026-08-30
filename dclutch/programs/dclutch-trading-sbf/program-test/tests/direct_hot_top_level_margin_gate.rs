//! The compute margin of the public Direct route, asserted rather than hoped.
//!
//! # Why a gate and not a note in a report
//!
//! Wall #28 was ruled ACCEPT on 2026-08-30: the top-level Direct route passes
//! every seed measured, on a margin of a percent and a bit. Accepting a margin
//! that thin is only honest if something notices when it erodes, and the
//! evidence that it erodes silently is in this repository's own history.
//! `df404c56` grew Core by 7,520 CU -- and its commit message says "the record
//! type only, no route and no program change yet", because it changed two
//! SHARED contract crates and nobody expected a program to move. It was true
//! and it was still costly.
//!
//! So this file is the condition of that ruling. The next commit that quietly
//! costs eight thousand compute units goes red here, at its author, instead of
//! on devnet a month later with nobody able to say which change did it.
//!
//! # Why thirty-two seeds and not one, and not twelve
//!
//! Ledger M-61, and the lane that wrote this file walked into exactly what M-61
//! forbids TWICE on its way here. Fixture keys determine `find_program_address`
//! bump depth, 1,500 CU an iteration, so ONE seed is a draw and not a
//! measurement -- the cross-seed band on this route is 40,499 CU.
//!
//! First: that lane bisected a continuation-route regression to `df404c56` on
//! the strength of seed 0 alone, and only then ran the control -- at the parent
//! commit the continuation ALREADY failed 4 of 12 seeds. The commit had shifted
//! a boundary and flipped one seed, not created the problem, and a single-seed
//! bisect would have shipped a confident and wrong culprit.
//!
//! Second: having learned that, the same lane then set this gate's number from
//! TWELVE seeds, which is more than one and still not enough. Twelve said the
//! worst draw was 1,373,917 CU. Thirty-two said 1,381,576. The gate would have
//! been born red on a key draw that was never a regression.
//!
//! The lesson both times is the same one and it is cheap to forget: a sample
//! that has not stopped moving is not a bound. Nothing here quotes one seed,
//! and the number below is a MAXIMUM over the sweep, never a mean.

use solana_program::pubkey::Pubkey;

use dclutch_direct_hot_program_test_support::waist::{
    TRADING_PROGRAM_ID, add_lookup_table, add_release_waist, canonical_lookup_addresses,
    direct_case, direct_top_level_instructions, elves, fixture_substrate,
    program_test_without_forced_budget, start_with_substrate, submit_v0_observed,
    with_fixture_seed,
};

/// Fixture seeds swept by the gate.
///
/// Twelve was the first answer and it was WRONG, in the direction that matters.
/// Twelve seeds put this route's worst draw at 1,373,917 CU; thirty-two put it
/// at 1,381,576. A gate pinned to the twelve-seed figure would have gone red on
/// seed 15 -- a legitimate key draw, not a regression -- and the first person to
/// meet that red would have learned to distrust this file.
///
/// Thirty-two is not magic either. It is enough draws for the band to stop
/// moving much, and the cost is about forty seconds.
///
/// # The lottery this sweep CANNOT see, which is not the maker's keys
///
/// Redrawing the fixture seed redraws the payer and the two makers. It does not
/// redraw the other input to every bump search on this route: `release_set_id`
/// is `hash(ExecutionReleaseSetV1)` over the deployed ELF DIGESTS, and it seeds
/// the activation cache directly and the Market identity transitively -- and
/// the Market PDA seeds the Claims market, the positions, the maker replays and
/// every caller authority downstream of them.
///
/// So a REBUILD redraws all of it, with no source change at all. Measured on
/// this route: the activation cache's bump was 254 for one build of a tree and
/// 255 for a build whose only source difference was caller-side, and that one
/// step moved five separate searches (three in Trading, two inside the Registry
/// CPIs) for 7,500 CU before anything else was counted. The whole 32-seed
/// distribution shifted with it, and its band went 36,001 -> 42,000.
///
/// This is almost certainly what an earlier lane recorded as "codegen noise of
/// +-20,000 CU between builds of the same source". It is not codegen and it is
/// not noise: it is this, and it means two CU figures taken from two builds are
/// not comparable, however careful the rest of the method was.
/// `direct_hot_pda_depth_census.rs` reports the depth outright so the two
/// effects can be told apart.
const GATE_SEEDS: u64 = 32;

/// Ceiling this route's worst swept seed must stay under.
///
/// Measured at `fd8cad39`, all five ELFs built from that commit: 32/32 pass,
/// 1,341,077 to 1,381,576 CU, mean 1,360,206.
///
/// This sat 8,424 CU above that worst observation, deliberately: `df404c56`
/// cost 7,520 CU while believing it changed no program at all, so a change of
/// that size has to trip this rather than disappear into the margin. It is not
/// the protocol ceiling -- and it must never be raised to meet a regression.
/// Raising it IS the act of spending margin, and it should cost a decision and
/// a sentence saying what got cheaper in exchange.
///
/// # Why this is 1,387,000 and not this build's worst draw
///
/// The public route used to search for the activation cache's address THREE
/// times per execution -- inside each of the two `reauthenticate_role` calls
/// and again in the child-program decode -- for one address with one answer.
/// It now searches once. A bump search costs 1,500 CU per attempt and makes at
/// least one attempt, so removing two of them is worth AT LEAST 2 x 1 x 1,500
/// = 3,000 CU against the same build, whatever depth that build happens to
/// draw. 1,390,000 - 3,000 is the tightening that argument supports, and it
/// cannot manufacture a red that the old number would not also have taken.
///
/// It is deliberately NOT set from the sweep this change was measured on, which
/// came back at a worst draw of 1,380,178. Most of that headroom is not the
/// change: it is the redraw described under `GATE_SEEDS`, and pinning a gate to
/// it would go red on the next rebuild with nothing wrong. That is the exact
/// mistake this file was already written to warn about, one level further out.
///
/// # What this gate does NOT do, stated because it would otherwise be assumed
///
/// It does not bound what a real trade costs. It sweeps THIRTY-TWO PINNED key
/// draws, so for a fixed ELF it is deterministic and any red is a code change.
/// Real makers bring whatever keys they bring, and the cross-seed band here is
/// 40,499 CU against a worst margin of 18,424 -- the band is more than twice
/// the margin. An unluckier draw than any of these thirty-two can exceed the
/// protocol ceiling and refuse a real user's trade, and nothing in this file
/// would have said so.
///
/// # What the band is made of, measured rather than supposed
///
/// All of it is `find_program_address` depth. The search walks bump 255 down
/// and pays 1,500 CU per candidate it rejects, and every gap between two
/// distinct observations in this sweep is a multiple of 1,500 -- there is no
/// other component. Decomposed from the runtime's own per-CPI accounting over
/// the 32 transactions: Claims carries a band of 16,499 CU, Custody 13,500,
/// Trading's own code about 6,000, and the Registry exactly ZERO, because the
/// only address it derives is seeded by the release set, which no key draw
/// moves.
///
/// One transaction makes roughly fifty of these searches and about twenty of
/// them move with the keys. The cure is not to make them cheaper, it is to stop
/// searching: `Pubkey::create_program_address` at a bump the caller already
/// holds costs one attempt instead of however many, and a wrong bump reproduces
/// a different address and refuses, so the derivation is still the check. This
/// route already does that in one place -- see `borrow_finalized_record_at` in
/// `hot_v3.rs` -- and the remaining duplicates are almost all across a CPI
/// boundary: Trading finds an address, discards the bump, and the child program
/// it calls searches for the very same address again. The Market PDA alone is
/// searched four times per transaction from identical seeds.
///
/// That gap is not a defect in the gate; it is the finding the gate exposed,
/// and it is why `tools/gauntlet/CU_BUDGETS.json`'s own tolerance rule cannot
/// write a budget for this route at all: `roundup(40499, 10000) + 10000` is a
/// 60,000 CU tolerance, and 1,381,576 + 60,000 is past 1,400,000. That file
/// says a budget above the ceiling is how it "says out loud that a transaction
/// has stopped fitting". This route is there.
const TOP_LEVEL_CU_GATE_V1: u64 = 1_387_000;

/// The protocol maximum a transaction may consume.
const PROTOCOL_CEILING: u64 = 1_400_000;

/// `tools/gauntlet/CU_BUDGETS.json`'s tolerance, for the band this sweep saw.
///
/// The rule is `roundup(band, 10000) + 10000`, floor 15,000, and a budget of
/// `measured + tolerance` above the ceiling is that file "saying out loud that
/// a transaction has stopped fitting". This route says it. The sweep prints the
/// verdict rather than asserting it, because the assertion that would fail is
/// the one this gate already makes -- and a second red row saying the same
/// thing in different words teaches a reader to skip both.
fn cu_budgets_tolerance(band: u64) -> u64 {
    let rounded = band.div_ceil(10_000).saturating_mul(10_000);
    rounded.saturating_add(10_000).max(15_000)
}

#[tokio::test]
async fn the_public_direct_route_holds_its_compute_margin_across_thirty_two_seeds() {
    let artifacts = elves();
    let mut observations = Vec::with_capacity(GATE_SEEDS as usize);
    let mut refusals: Vec<(u64, String)> = Vec::new();

    for seed in 0..GATE_SEEDS {
        // Every fixture key is drawn inside here, on this thread, with no
        // environment mutation -- see `with_fixture_seed`.
        let (mut test, direct, instructions) = with_fixture_seed(seed, || {
            let mut test = program_test_without_forced_budget(&artifacts);
            let releases = add_release_waist(&mut test, &artifacts);
            let direct = direct_case(&mut test, releases, &artifacts, false);
            let instructions = direct_top_level_instructions(&direct);
            (test, direct, instructions)
        });

        // If this ever becomes the Registry the gate has quietly turned into a
        // measurement of the continuation, which is a different route with a
        // different margin and is not what was accepted.
        assert_eq!(
            instructions[3].program_id, TRADING_PROGRAM_ID,
            "seed {seed}: the gate must measure the top-level route",
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
            Ok(execution) => observations.push((seed, execution.compute_units_consumed)),
            Err(error) => refusals.push((seed, format!("{error:?}"))),
        }
    }

    // Every seed is reported before anything is asserted. The first version of
    // this loop panicked at the first refusal, which is the one shape of red
    // that tells a reader least: a route over the ceiling refuses on SOME
    // draws, and "seed 13 refused" does not say whether that is one unlucky
    // key or twenty. The sweep costs the same either way, so it finishes.
    for (seed, units) in &observations {
        println!("SEEDCU\t{seed}\t{units}");
    }
    for (seed, error) in &refusals {
        println!("SEEDREFUSED\t{seed}\t{error}");
    }
    assert!(
        refusals.is_empty(),
        "{} of {GATE_SEEDS} seeds REFUSED rather than executed, {} of them executing: {refusals:?}. \
         This is the acceptance criteria for a public trade, and a refusal here is a broken \
         route, not a margin question. If every refusal is ComputationalBudgetExceeded, the \
         route is over the protocol ceiling on those key draws -- see GATE_SEEDS on why a \
         REBUILD alone redraws every bump depth on this route.",
        refusals.len(),
        observations.len(),
    );

    let (worst_seed, worst) = observations
        .iter()
        .copied()
        .max_by_key(|(_, units)| *units)
        .expect("the sweep ran at least one seed");
    let best = observations
        .iter()
        .map(|(_, units)| *units)
        .min()
        .expect("the sweep ran at least one seed");
    let mean = observations.iter().map(|(_, units)| *units).sum::<u64>() / GATE_SEEDS;

    let band = worst.saturating_sub(best);
    let tolerance = cu_budgets_tolerance(band);
    println!(
        "public Direct route across {GATE_SEEDS} seeds: {best} to {worst} CU, mean {mean}, \
         band {band}, worst margin {} of {PROTOCOL_CEILING}",
        PROTOCOL_CEILING.saturating_sub(worst),
    );
    println!(
        "CU_BUDGETS tolerance for a band of {band} is {tolerance}; a budget for this route \
         would be {} against a ceiling of {PROTOCOL_CEILING} -- {}",
        worst.saturating_add(tolerance),
        if worst.saturating_add(tolerance) > PROTOCOL_CEILING {
            "OVER: by that file's own rule this transaction has stopped fitting, and the \
             band is the reason, not the mean"
        } else {
            "under: the route fits for an arbitrary key draw, not merely for these ones"
        },
    );

    assert!(
        worst <= TOP_LEVEL_CU_GATE_V1,
        "the public Direct route's worst seed ({worst_seed}) now consumes {worst} CU, past the \
         {TOP_LEVEL_CU_GATE_V1} gate. Wall #28 was accepted on a margin of about 1.3% ON \
         CONDITION that erosion is caught here. Something in this change, or in a shared \
         contract crate it pulled in, made the route more expensive -- find it before raising \
         this number, because only {} CU stand between the gate and the protocol ceiling. \
         Check the shared contract crates first: the last change to cost this route real \
         margin believed it had changed no program at all.",
        PROTOCOL_CEILING.saturating_sub(TOP_LEVEL_CU_GATE_V1),
    );
}
