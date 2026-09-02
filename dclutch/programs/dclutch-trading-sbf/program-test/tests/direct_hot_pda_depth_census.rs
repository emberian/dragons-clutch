//! What the Direct route's bump searches actually cost, and what they depend on.
//!
//! `Pubkey::find_program_address` walks bump 255 downward and pays 1,500 CU for
//! every candidate it rejects. The public Direct route runs close enough to the
//! 1,400,000 ceiling that this search depth is not a detail: the 32-seed sweep
//! in `direct_hot_top_level_margin_gate.rs` spans 36,001 CU, and every gap
//! between two distinct observations in it is a multiple of 1,500.
//!
//! This census reports the depths themselves, so that a CU number can be read
//! as "the code got more expensive" or "this deployment drew a deeper bump"
//! rather than the two being confused. They have been confused before.
//!
//! # The part that is not about maker keys
//!
//! The activation cache's address is `[domain, release_set_id]` under the
//! Registry, and `release_set_id` is a HASH OF THE DEPLOYED ELF BYTES. It does
//! not move with the fixture seed -- every key draw against one build pays the
//! same cache depth -- but it moves on every REBUILD, and it feeds the Market
//! identity, which seeds the PDAs downstream of it.
//!
//! So a rebuild redraws this lottery even when the source did not change, and a
//! before/after CU comparison taken across two builds is measuring both effects
//! at once. That is what this file exists to disentangle: run it against each
//! ELF directory and the cache depth is stated outright.

use solana_program::pubkey::Pubkey;

use dclutch_direct_hot_program_test_support::waist::{
    REGISTRY_PROGRAM_ID, add_release_waist, elves, program_test_without_forced_budget,
    with_fixture_seed,
};
use dclutch_registry_contract::ACTIVATION_PDA_DOMAIN_V1;

/// Attempts `find_program_address` makes to land on `bump`, at 1,500 CU each.
use dclutch_program_test_evidence::pda_search::{ATTEMPT_COST_CU, attempts};

#[test]
fn the_activation_cache_depth_is_a_property_of_the_build_not_of_the_keys() {
    let artifacts = elves();
    let mut seen: Vec<(u64, u8)> = Vec::new();

    for seed in 0..8_u64 {
        let releases = with_fixture_seed(seed, || {
            let mut test = program_test_without_forced_budget(&artifacts);
            add_release_waist(&mut test, &artifacts)
        });
        let (_, bump) = Pubkey::find_program_address(
            &[ACTIVATION_PDA_DOMAIN_V1, &releases.release_set],
            &REGISTRY_PROGRAM_ID,
        );
        seen.push((seed, bump));
    }

    let first = seen.first().expect("the census ran at least one seed").1;
    println!(
        "CENSUS activation-cache bump {first}, {} attempts, {} CU per search",
        attempts(first),
        attempts(first) * ATTEMPT_COST_CU,
    );
    for (seed, bump) in &seen {
        println!("CENSUS\tseed {seed}\tcache bump {bump}");
    }

    assert!(
        seen.iter().all(|(_, bump)| *bump == first),
        "the activation cache's seeds contain no fixture key, so its depth must \
         not move with the seed. It moved: {seen:?}. Either the fixture started \
         drawing the release set, or this census is measuring the wrong address."
    );
}
