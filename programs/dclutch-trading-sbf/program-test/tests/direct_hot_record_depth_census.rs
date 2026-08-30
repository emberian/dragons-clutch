//! What the Direct route's CONSTANT record searches cost, and what they do not depend on.
//!
//! `direct_hot_pda_depth_census.rs` reports the activation cache's depth, which
//! moves on every REBUILD because `release_set_id` hashes the deployed ELF
//! digests. This file reports the other half of the constant class, and its
//! whole point is that this half moves on NOTHING.
//!
//! A finalized record's address is `[domain, schema_release_id, content_digest]`
//! under the REGISTRY program. Not one of those four inputs is an ELF digest and
//! not one is a participant key:
//!
//! * the domains are `dclutch-record-contract` constants,
//! * `schema_release_id` is a schema constant,
//! * `content_digest` is the hash of the record's own immutable bytes,
//! * the Registry program id is fixed for a deployment.
//!
//! So where the cache's depth is a lottery redrawn by every build, a record's
//! depth is a constant of the protocol. That is the claim this census exists to
//! ASSERT rather than assume, because it is the argument for why removing a
//! record search is worth more than the CU it removes: it is a saving that
//! cannot be undone by a rebuild, and a cost that can be known before it is
//! paid.
//!
//! # Why the depths are printed and not just the pass/fail
//!
//! `find_program_address` pays 1,500 CU per candidate it rejects and makes at
//! least one attempt. A search at bump 255 costs 1,500 CU and converting it to
//! `create_program_address` saves NOTHING; a search at bump 250 costs 9,000 and
//! converting it saves 7,500. A lane that converts these without reading their
//! depths first is guessing at its own result, so the depths are the deliverable
//! here, not a diagnostic.

use solana_program::{hash::hash, pubkey::Pubkey};

use dclutch_capability_seal_contract::CAPABILITY_SEAL_BUMP_OFFSET_V1;
use dclutch_direct_hot_program_test_support::waist::{
    REGISTRY_PROGRAM_ID, add_release_waist, direct_case, elves, program_test_without_forced_budget,
    with_fixture_seed,
};
use dclutch_execution_strategy_contract::v2::EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2;
use dclutch_product_payoff_v2_codec::registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3;
use dclutch_product_runtime_v2_admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_realm_contract::REALM_SCHEMA_RELEASE_ID_V1;
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};

/// Attempts `find_program_address` makes to land on `bump`, at 1,500 CU each.
const fn attempts(bump: u8) -> u32 {
    256 - bump as u32
}

const ATTEMPT_COST_CU: u32 = 1_500;

/// Seeds swept. Thirty-two, for the reason `direct_hot_top_level_margin_gate.rs`
/// gives at length: twelve understated that gate's worst draw by 7,659 CU. A
/// census whose whole claim is "this does not move" must sweep at least as
/// widely as the one that found movement.
const CENSUS_SEEDS: u64 = 32;

/// The record classes the Direct Hot route authenticates by SEARCHING.
///
/// Deliberately not every record on the route. The manifest, program-set and
/// config records are absent because they do NOT search: `borrow_finalized_record_at`
/// reads them at the bumps `SelectedRecordBumpsV1` carries in the Market root.
/// The six sealed artifacts are absent because `borrow_sealed_record` reads
/// coordinates the seal persisted. What is left is what still pays.
const SEARCHED_RECORD_SCHEMAS: [(&str, [u8; 32]); 6] = [
    ("product", PRODUCT_RECORD_SCHEMA_ID_V2),
    ("result-domain", RESULT_DOMAIN_SCHEMA_ID_V2),
    ("portfolio", PORTFOLIO_SCHEMA_ID_V2),
    ("linked-basis", GRADED_BASIS_RECORD_SCHEMA_ID_V3),
    ("realm", REALM_SCHEMA_RELEASE_ID_V1),
    ("exec-strategy", EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2),
];

/// One record's canonical coordinate, as the route would have to search for it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RecordDepth {
    raw_bump: u8,
    staging_bump: u8,
}

/// What one raw/staging pair costs a reader that must SEARCH for both.
const fn search_cost_cu(depth: RecordDepth) -> u32 {
    (attempts(depth.raw_bump) + attempts(depth.staging_bump)) * ATTEMPT_COST_CU
}

/// What the same pair costs a reader handed both canonical bumps.
///
/// Not zero, and this is the correction that decides whether a conversion is
/// worth making at all: `create_program_address` is ITSELF a 1,500 CU syscall,
/// so a converted pair still pays two of them. A record already at bump 255 is
/// therefore already at the floor, and converting it saves exactly nothing.
const fn carried_cost_cu() -> u32 {
    2 * ATTEMPT_COST_CU
}

/// Find the finalized record of `schema` among the fixture's installed accounts.
///
/// The fixture plants each record at its canonical address with its exact bytes,
/// so the digest is recoverable without reaching into the fixture's private
/// types: hash the account's own data and ask whether that reproduces its key.
/// A record that does not reproduce its own address is not a record, and the
/// census says so rather than skipping it.
fn locate(accounts: &[(Pubkey, Vec<u8>, Pubkey)], schema: [u8; 32]) -> Option<RecordDepth> {
    for (key, data, owner) in accounts {
        if owner != &REGISTRY_PROGRAM_ID || data.is_empty() {
            continue;
        }
        let digest = hash(data).to_bytes();
        let (raw, raw_bump) = Pubkey::find_program_address(
            &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
            &REGISTRY_PROGRAM_ID,
        );
        if &raw != key {
            continue;
        }
        let (_, staging_bump) = Pubkey::find_program_address(
            &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
            &REGISTRY_PROGRAM_ID,
        );
        return Some(RecordDepth {
            raw_bump,
            staging_bump,
        });
    }
    None
}

#[test]
fn the_searched_record_depths_are_constants_of_the_protocol_not_of_the_keys() {
    let artifacts = elves();
    let mut sweep: Vec<(u64, Vec<(&str, RecordDepth)>)> = Vec::new();
    let mut seal_bumps: Vec<(u64, u8)> = Vec::new();

    for seed in 0..CENSUS_SEEDS {
        let (accounts, seal) = with_fixture_seed(seed, || {
            let mut test = program_test_without_forced_budget(&artifacts);
            let releases = add_release_waist(&mut test, &artifacts);
            let direct = direct_case(&mut test, releases, &artifacts, false);
            let accounts = direct
                .chain
                .accounts
                .iter()
                .map(|installed| {
                    (
                        installed.key,
                        installed.account.data.clone(),
                        installed.account.owner,
                    )
                })
                .collect::<Vec<_>>();
            // The seal's persisted bump IS the canonical one: the fixture
            // records what the mint would have signed with. Reading it back is
            // therefore the depth, without restating the seed projection here.
            let seal_bump = direct
                .chain
                .capability_seal_bytes
                .get(CAPABILITY_SEAL_BUMP_OFFSET_V1)
                .copied()
                .expect("the staged seal carries its own bump");
            (accounts, seal_bump)
        });
        seal_bumps.push((seed, seal));

        let mut found = Vec::new();
        for (name, schema) in SEARCHED_RECORD_SCHEMAS {
            let depth = locate(&accounts, schema).unwrap_or_else(|| {
                panic!(
                    "seed {seed}: the census could not find the {name} record among the \
                     fixture's installed accounts. Either the fixture stopped planting it at \
                     its canonical content-addressed coordinate, or this census is looking \
                     for a schema the route no longer reads -- both are findings, neither is \
                     a flake."
                )
            });
            found.push((name, depth));
        }
        sweep.push((seed, found));
    }

    let (_, first) = sweep.first().expect("the census ran at least one seed");
    let (_, seal_bump) = *seal_bumps
        .first()
        .expect("the census ran at least one seed");

    let mut searched = 0_u32;
    let mut carried = 0_u32;
    // The capability seal is not a finalized record, but it is the same CLASS
    // of cost and it belongs in the same table: one search, on an address whose
    // seeds are a descriptor digest, an action selector, the Trading SEMANTIC
    // release id and the Registry program. None of those is a participant key
    // and none is an ELF digest, so like the records above -- and unlike the
    // activation cache -- its depth is a constant of the deployment.
    //
    // It is the only constant-seeded search on this route whose carrier was not
    // full: the seal now records its own bump in the first of its four reserved
    // bytes, and `authenticate_capability_seal_v3` reproduces the address from
    // it. This row is what that saving actually was, on this build.
    println!("RECDEPTH\trecord\traw_bump\tstaging_bump\tsearch_cu\tcarried_cu\tsaving_cu");
    {
        let seal_search = attempts(seal_bump) * ATTEMPT_COST_CU;
        let seal_carried = ATTEMPT_COST_CU;
        searched += seal_search;
        carried += seal_carried;
        println!(
            "RECDEPTH\tcapability-seal\t{seal_bump}\t-\t{seal_search}\t{seal_carried}\t{}",
            seal_search.saturating_sub(seal_carried),
        );
    }
    for (name, depth) in first {
        // The realm record is authenticated once inside EACH of the two Custody
        // CPIs, so its pair is paid twice per transaction. Every other record
        // here is read once.
        let invocations = if *name == "realm" { 2 } else { 1 };
        let search = search_cost_cu(*depth) * invocations;
        let carry = carried_cost_cu() * invocations;
        searched += search;
        carried += carry;
        println!(
            "RECDEPTH\t{name}\t{}\t{}\t{search}\t{carry}\t{}",
            depth.raw_bump,
            depth.staging_bump,
            search.saturating_sub(carry),
        );
    }
    println!(
        "RECDEPTH TOTAL searched {searched} CU, carried {carried} CU, SAVING {} CU per transaction",
        searched.saturating_sub(carried),
    );

    // The seal's seeds carry no participant key either, so its depth must be as
    // still as the records'. It is checked separately because it is not a
    // record and does not have a raw/staging pair.
    for (seed, observed) in &seal_bumps {
        assert_eq!(
            *observed, seal_bump,
            "seed {seed} drew a different capability-seal bump than seed 0. The seal's \
             address is [domain, descriptor schema, descriptor digest, action, Trading \
             semantic release, Registry] and contains NO participant key, so this cannot \
             happen from a key draw."
        );
    }

    // The claim. Every seed redraws the payer and both makers; not one of them
    // is a seed of any record address, so not one depth may move.
    for (seed, found) in &sweep {
        assert_eq!(
            found, first,
            "seed {seed} drew different record depths than seed 0. A record's address is \
             [domain, schema, content digest] under the Registry and contains NO participant \
             key, so this cannot happen from a key draw. Either a record's content became \
             key-dependent -- which would make every one of these searches a per-draw CU \
             variance on a route that is already over its ceiling -- or this census is \
             measuring the wrong accounts."
        );
    }
}
