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
const GATE_SEEDS: u64 = 32;

/// Ceiling this route's worst swept seed must stay under.
///
/// Measured at `fd8cad39`, all five ELFs built from that commit: 32/32 pass,
/// 1,341,077 to 1,381,576 CU, mean 1,360,206.
///
/// This sits 8,424 CU above that worst observation, deliberately: `df404c56`
/// cost 7,520 CU while believing it changed no program at all, so a change of
/// that size has to trip this rather than disappear into the margin. It is not
/// the protocol ceiling -- it stands 10,000 CU below it -- and it must never be
/// raised to meet a regression. Raising it IS the act of spending margin, and
/// it should cost a decision and a sentence saying what got cheaper in
/// exchange.
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
/// That gap is not a defect in the gate; it is the finding the gate exposed,
/// and it is why `tools/gauntlet/CU_BUDGETS.json`'s own tolerance rule cannot
/// write a budget for this route at all: `roundup(40499, 10000) + 10000` is a
/// 60,000 CU tolerance, and 1,381,576 + 60,000 is past 1,400,000. That file
/// says a budget above the ceiling is how it "says out loud that a transaction
/// has stopped fitting". This route is there.
const TOP_LEVEL_CU_GATE_V1: u64 = 1_390_000;

/// The protocol maximum a transaction may consume.
const PROTOCOL_CEILING: u64 = 1_400_000;

#[tokio::test]
async fn the_public_direct_route_holds_its_compute_margin_across_thirty_two_seeds() {
    let artifacts = elves();
    let mut observations = Vec::with_capacity(GATE_SEEDS as usize);

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

        let execution = submit_v0_observed(
            &mut context,
            &instructions,
            addresses,
            Some(&direct.payer),
            &[],
        )
        .await
        .unwrap_or_else(|error| {
            panic!(
                "seed {seed}: the public Direct route must EXECUTE, not refuse. \
                 This is the acceptance criteria for a public trade, and a \
                 refusal here is a broken route, not a margin question: {error:?}"
            )
        });

        observations.push((seed, execution.compute_units_consumed));
    }

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

    println!(
        "public Direct route across {GATE_SEEDS} seeds: {best} to {worst} CU, mean {mean}, \
         worst margin {} of {PROTOCOL_CEILING}",
        PROTOCOL_CEILING.saturating_sub(worst),
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
