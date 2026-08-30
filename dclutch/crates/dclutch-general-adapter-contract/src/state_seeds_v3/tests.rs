//! Controls for the sole General state-seed derivation.
//!
//! The test that matters is
//! [`a_wrong_seed_order_is_not_something_this_module_can_express`]: the point of
//! the module is not that the order is right today, it is that there is nowhere
//! to write a second, different one.

use super::*;

extern crate std;
use std::{vec, vec::Vec};

fn id(value: u8) -> [u8; 32] {
    [value; 32]
}

/// The three recipes and nothing else. A fourth would be a visible edit here.
const EVERY_RECIPE: [GeneralStateRecipeV3; 3] = [
    GeneralStateRecipeV3::Selection,
    GeneralStateRecipeV3::Settlement,
    GeneralStateRecipeV3::Terminal,
];

#[test]
fn every_seed_is_short_enough_and_long_enough_to_actually_derive() {
    for seed in [
        GENERAL_STATE_SEED_DOMAIN_V3,
        GENERAL_SELECTION_STATE_SEED_V3,
        GENERAL_SETTLEMENT_STATE_SEED_V3,
        GENERAL_TERMINAL_STATE_SEED_V3,
    ] {
        assert!(
            !seed.is_empty() && seed.len() <= MAX_PDA_SEED_BYTES,
            "a PDA seed must be nonempty and at most 32 bytes to derive an address"
        );
    }
}

/// The bytes themselves, pinned once.
///
/// Every other assertion in this file compares a projection against the very
/// constant it is built from, so all of them hold for any spelling -- they pin
/// the ORDER and are blind to the bytes. This one pins the bytes, and it is the
/// control that fails if a release respells a live seed domain.
#[test]
fn the_seed_domains_are_the_exact_spellings_the_accelerator_executed() {
    assert_eq!(GENERAL_STATE_SEED_DOMAIN_V3, b"dclutch-general-state-v3");
    assert_eq!(GENERAL_SELECTION_STATE_SEED_V3, b"selection");
    assert_eq!(GENERAL_SETTLEMENT_STATE_SEED_V3, b"settlement");
    assert_eq!(GENERAL_TERMINAL_STATE_SEED_V3, b"terminal");
}

/// The recipe's declared geometry is READ OFF its seed table, never beside it.
///
/// Before this module the counts were hand-written next to the tables
/// (`seed_count: if selection { 4 } else { 5 }`), which is two authors for one
/// fact: a seed added to the table without touching the literal would encode a
/// policy that derives an address from a truncated seed program.
#[test]
fn the_declared_geometry_is_the_table_and_not_a_number_beside_it() {
    for recipe in EVERY_RECIPE {
        let seeds = recipe.lifecycle_seeds();
        assert_eq!(usize::from(recipe.seed_count()), seeds.len());
        assert_eq!(usize::from(recipe.bump_offset()), seeds.len() - 1);
        assert_eq!(recipe.supplied_seed_count(), seeds.len() - 1);
        assert!(matches!(
            seeds[seeds.len() - 1],
            LifecycleSeedInputV3::CanonicalBump
        ));
        // Exactly one bump, and it is the last one.
        assert_eq!(
            seeds
                .iter()
                .filter(|seed| matches!(seed, LifecycleSeedInputV3::CanonicalBump))
                .count(),
            1
        );
    }
    assert_eq!(GeneralStateRecipeV3::Selection.seed_count(), 4);
    assert_eq!(GeneralStateRecipeV3::Settlement.seed_count(), 5);
    assert_eq!(GeneralStateRecipeV3::Terminal.seed_count(), 6);
}

/// THE CONTROL THE LANE EXISTS FOR.
///
/// A wrong-seed policy is the failure that AUTHENTICATES and derives the wrong
/// address. It is refused here not by comparison against a second copy of the
/// right answer, but because the address projection has no seed order of its
/// own to be wrong with: it reads the policy's table. Substituting the order
/// therefore moves BOTH sides, which is the definition of one author.
#[test]
fn a_wrong_seed_order_is_not_something_this_module_can_express() {
    let root = id(3);
    let candidate = id(4);
    let seeds = GeneralStateAddressSeedsV3::settlement(root, candidate).expect("settlement");
    let projected = seeds.as_slices().expect("slices");

    // The projection is the table, entry for entry, with the caller's bytes
    // substituted -- so this assertion is derived on both sides.
    let expected: Vec<&[u8]> = GENERAL_SETTLEMENT_STATE_RECIPE_V3
        .iter()
        .filter_map(|seed| match *seed {
            LifecycleSeedInputV3::Literal(literal) => Some(literal),
            LifecycleSeedInputV3::CommonIdentity(GENERAL_ROOT_IDENTITY_REGISTER_V3) => {
                Some(root.as_slice())
            }
            LifecycleSeedInputV3::CommonIdentity(_) => Some(candidate.as_slice()),
            _ => None,
        })
        .collect();
    assert_eq!(projected.as_slice(), expected.as_slice());

    // And it is the order the accelerator campaign actually executed.
    assert_eq!(
        projected.as_slice(),
        &[
            GENERAL_STATE_SEED_DOMAIN_V3,
            root.as_slice(),
            candidate.as_slice(),
            GENERAL_SETTLEMENT_STATE_SEED_V3,
        ]
    );
}

#[test]
fn the_selection_and_terminal_orders_are_the_ones_the_campaign_executed() {
    let root = id(3);
    let candidate = id(4);

    let selection = GeneralStateAddressSeedsV3::selection(root).expect("selection");
    assert_eq!(
        selection.as_slices().expect("slices").as_slice(),
        &[
            GENERAL_STATE_SEED_DOMAIN_V3,
            root.as_slice(),
            GENERAL_SELECTION_STATE_SEED_V3,
        ]
    );

    let coordinate = 0x0102_0304_0506_0708_u64;
    let encoded = coordinate.to_le_bytes();
    let terminal =
        GeneralStateAddressSeedsV3::terminal(root, candidate, coordinate).expect("terminal");
    assert_eq!(
        terminal.as_slices().expect("slices").as_slice(),
        &[
            GENERAL_STATE_SEED_DOMAIN_V3,
            root.as_slice(),
            candidate.as_slice(),
            encoded.as_slice(),
            GENERAL_TERMINAL_STATE_SEED_V3,
        ]
    );
    assert_eq!(
        encoded.len(),
        usize::from(GENERAL_TERMINAL_COORDINATE_SEED_BYTES_V3)
    );
}

/// Two states that must never collide, and the coordinates that separate them.
#[test]
fn distinct_coordinates_never_project_onto_one_address() {
    let root = id(3);
    let left = GeneralStateAddressSeedsV3::settlement(root, id(4)).expect("left");
    let right = GeneralStateAddressSeedsV3::settlement(root, id(5)).expect("right");
    assert_ne!(
        left.as_slices().expect("left").as_slice(),
        right.as_slices().expect("right").as_slice()
    );

    // Selection carries no candidate, so it can never alias a settlement state
    // even under the same root -- the phase discriminator is what separates them.
    let selection = GeneralStateAddressSeedsV3::selection(root).expect("selection");
    assert_ne!(
        selection.as_slices().expect("selection").as_slice(),
        left.as_slices().expect("left").as_slice()
    );

    // One settlement closes into one terminal record PER coordinate.
    let first = GeneralStateAddressSeedsV3::terminal(root, id(4), 0).expect("first");
    let second = GeneralStateAddressSeedsV3::terminal(root, id(4), 1).expect("second");
    assert_ne!(
        first.as_slices().expect("first").as_slice(),
        second.as_slices().expect("second").as_slice()
    );
}

#[test]
fn the_action_to_phase_mapping_is_published_rather_than_restated() {
    for action in [Action::Consider, Action::Freeze] {
        assert_eq!(
            GeneralStateRecipeV3::primary_for_action(action),
            GeneralStateRecipeV3::Selection
        );
    }
    for action in [
        Action::InitializeSettlement,
        Action::Collect,
        Action::Materialize,
        Action::Distribute,
        Action::Close,
    ] {
        assert_eq!(
            GeneralStateRecipeV3::primary_for_action(action),
            GeneralStateRecipeV3::Settlement
        );
    }
}

/// Hostiles, each with the exact refusal it must produce.
#[test]
fn zero_aliased_and_missing_coordinates_refuse_with_pinned_codes() {
    assert_eq!(
        GeneralStateAddressSeedsV3::selection([0; 32]),
        Err(GeneralStateSeedErrorV3::ZeroIdentity)
    );
    assert_eq!(
        GeneralStateAddressSeedsV3::settlement([0; 32], id(4)),
        Err(GeneralStateSeedErrorV3::ZeroIdentity)
    );
    assert_eq!(
        GeneralStateAddressSeedsV3::settlement(id(3), [0; 32]),
        Err(GeneralStateSeedErrorV3::ZeroIdentity)
    );
    // A root that is also the candidate would make two seed positions carry one
    // value, so two logically distinct states could share an address.
    assert_eq!(
        GeneralStateAddressSeedsV3::settlement(id(3), id(3)),
        Err(GeneralStateSeedErrorV3::AccountAlias)
    );
    assert_eq!(
        GeneralStateAddressSeedsV3::terminal(id(3), id(3), 7),
        Err(GeneralStateSeedErrorV3::AccountAlias)
    );
    assert_eq!(
        GeneralStateAddressSeedsV3::terminal([0; 32], id(4), 7),
        Err(GeneralStateSeedErrorV3::ZeroIdentity)
    );
}

/// A coordinate set walked against a recipe that demands more than it carries
/// refuses rather than silently projecting a shorter, different seed program.
#[test]
fn a_recipe_demanding_an_absent_coordinate_refuses_rather_than_shortening() {
    let mut selection = GeneralStateAddressSeedsV3::selection(id(3)).expect("selection");
    // Force the mismatch the public constructors make unreachable, to prove the
    // walk is total rather than relying on construction alone.
    selection.recipe = GeneralStateRecipeV3::Settlement;
    assert_eq!(
        selection.as_slices().err(),
        Some(GeneralStateSeedErrorV3::MissingCoordinate)
    );

    let mut settlement = GeneralStateAddressSeedsV3::settlement(id(3), id(4)).expect("settlement");
    settlement.recipe = GeneralStateRecipeV3::Terminal;
    assert_eq!(
        settlement.as_slices().err(),
        Some(GeneralStateSeedErrorV3::MissingCoordinate)
    );
}

#[test]
fn the_address_join_is_closed_against_mismatch_zero_and_aliasing() {
    let seeds = GeneralStateAddressSeedsV3::settlement(id(3), id(4)).expect("settlement");
    assert_eq!(seeds.authenticate_address(id(9), id(9)), Ok(id(9)));
    assert_eq!(
        seeds.authenticate_address(id(9), id(8)),
        Err(GeneralStateSeedErrorV3::AddressMismatch)
    );
    assert_eq!(
        seeds.authenticate_address([0; 32], id(9)),
        Err(GeneralStateSeedErrorV3::ZeroIdentity)
    );
    assert_eq!(
        seeds.authenticate_address(id(9), [0; 32]),
        Err(GeneralStateSeedErrorV3::ZeroIdentity)
    );
    // A state whose address equals one of its own seed coordinates is a
    // derivation that ate its own input.
    for alias in [id(3), id(4)] {
        assert_eq!(
            seeds.authenticate_address(alias, alias),
            Err(GeneralStateSeedErrorV3::AccountAlias)
        );
    }
}

/// The seed table is the same object the policy encoder is handed, so a caller
/// cannot obtain one order for encoding and a different one for deriving.
#[test]
fn both_projections_read_one_table() {
    for recipe in EVERY_RECIPE {
        let table = recipe.lifecycle_seeds();
        assert!(core::ptr::eq(table, recipe.lifecycle_seeds()));
        let supplied = recipe.supplied_seed_count();
        assert_eq!(supplied + 1, table.len());
        assert!(supplied <= GENERAL_MAX_STATE_SEEDS_V3);
    }
    // Distinct recipes are distinct orders; none is a prefix rename of another.
    let orders: Vec<_> = EVERY_RECIPE
        .iter()
        .map(|recipe| vec![recipe.lifecycle_seeds()])
        .collect();
    assert_ne!(orders[0], orders[1]);
    assert_ne!(orders[1], orders[2]);
    assert_ne!(orders[0], orders[2]);
}
