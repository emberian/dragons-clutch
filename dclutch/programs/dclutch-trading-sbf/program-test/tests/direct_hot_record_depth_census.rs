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
//!
//! # Erratum, 2026-08-30: the realm row was reported at double its value
//!
//! This file multiplied the realm pair by TWO Custody invocations from
//! `a0cba859` until 2026-08-30, so it reported the realm carry as worth 18,000
//! CU per transaction. It is worth **9,000**. The route makes ONE Custody CPI on
//! this fixture -- 192 invocations over 32 swept seeds, 6 per transaction --
//! because the fixture's fee floors to zero and only one of the four declared
//! Custody routes is enabled. `a0cba859`'s commit message states the same wrong
//! figure and cannot be corrected in place; this is the correction of record.
//! See [`CUSTODY_INVOCATIONS_PER_TRADE_V1`].

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
use dclutch_record_contract::{ContentDigest, RecordKeyV1, RecordPdaSeedsV1, SchemaReleaseId};

/// Attempts `find_program_address` makes to land on `bump`, at 1,500 CU each.
use dclutch_program_test_evidence::pda_search::{ATTEMPT_COST_CU, attempts};

/// Seeds swept. Thirty-two, for the reason `direct_hot_top_level_margin_gate.rs`
/// gives at length: twelve understated that gate's worst draw by 7,659 CU. A
/// census whose whole claim is "this does not move" must sweep at least as
/// widely as the one that found movement.
const CENSUS_SEEDS: u64 = 32;

/// The record classes whose depth this census reports.
///
/// Deliberately not every record on the route. The manifest, program-set and
/// config records are absent because they do NOT search: `borrow_finalized_record_at`
/// reads them at the bumps `SelectedRecordBumpsV1` carries in the Market root.
/// The six sealed artifacts are absent because `borrow_sealed_record` reads
/// coordinates the seal persisted.
///
/// [`CARRIED_TODAY`] says which of the rest still SEARCH. A row that no longer
/// searches keeps its place, because its depth is the measurement and the
/// measurement is what proves the conversion was worth making -- but the table
/// says so in its own column rather than leaving a reader to infer a cost the
/// route stopped paying.
const SEARCHED_RECORD_SCHEMAS: [(&str, [u8; 32]); 6] = [
    ("product", PRODUCT_RECORD_SCHEMA_ID_V2),
    ("result-domain", RESULT_DOMAIN_SCHEMA_ID_V2),
    ("portfolio", PORTFOLIO_SCHEMA_ID_V2),
    ("linked-basis", GRADED_BASIS_RECORD_SCHEMA_ID_V3),
    ("realm", REALM_SCHEMA_RELEASE_ID_V1),
    ("exec-strategy", EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2),
];

/// Records the route now reads at a CARRIED bump rather than by searching.
///
/// The realm pair left the searching class when `CoreState` began recording the
/// two bumps its founding derived: Custody's `authenticate_realm` reproduces
/// both instead of walking down from 255, once per Custody invocation. It is the
/// largest single row in this table and it is the one that is already banked.
const CARRIED_TODAY: [&str; 1] = ["realm"];

/// Custody CPIs one canonical Direct trade makes on THIS fixture: exactly one.
///
/// This was `2` from `a0cba859` until 2026-08-30, and the realm row was reported
/// at double its real value because of it -- 18,000 CU of saving where the route
/// banks 9,000. `a0cba859`'s own commit message carries the same error.
///
/// The correction is a count, not an argument. The variance census swept 32
/// seeds and observed **192 Custody program invocations, 6 per transaction**,
/// which one Custody route produces and two cannot. The reason is the fixture's
/// economics: `gross = 10 * 50 / 100 = 5` and `fee = 5 * 50bps` floors to ZERO,
/// so of the four declared `CUSTODY_ROUTES_V3` only slot 0, the seller-terminal
/// register, is enabled.
///
/// So this constant is a fact about the ZERO-FEE fixture and not about the
/// protocol. A fee-bearing Direct trade enables a second Custody route, and
/// every per-transaction figure in this file would double for the realm row. No
/// gate in this tree has ever executed that shape; the variance census records
/// it as the open question it is.
const CUSTODY_INVOCATIONS_PER_TRADE_V1: u32 = 1;

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
        let Some(record) = record_key_v1(schema, hash(data).to_bytes()) else {
            continue;
        };
        let (raw, raw_bump) = record_address_v1(record.raw_record_pda_seeds());
        if &raw != key {
            continue;
        }
        let (_, staging_bump) = record_address_v1(record.staging_cursor_pda_seeds());
        return Some(RecordDepth {
            raw_bump,
            staging_bump,
        });
    }
    None
}

/// Bind one schema/digest pair, or refuse a zero component.
///
/// `None` is "not a record", which is exactly how the caller already treats an
/// account whose address it cannot reproduce. `SchemaReleaseId` and
/// `ContentDigest` both refuse zero, and this census hashes arbitrary
/// Registry-owned account data, so the refusal is reachable in principle even
/// though no planted account has ever produced it.
fn record_key_v1(schema: [u8; 32], digest: [u8; 32]) -> Option<RecordKeyV1> {
    Some(RecordKeyV1::new(
        SchemaReleaseId::new(schema).ok()?,
        ContentDigest::new(digest).ok()?,
    ))
}

/// One finalized record's address and bump, under this fixture's Registry.
///
/// The seed tuple is NOT restated here, and that is the seam-audit rule this
/// file used to break (`DOMAIN_RAW_RESTATEMENT`). `dclutch-record-contract`
/// owns `RAW_RECORD_PDA_SEED_V1` and `STAGING_CURSOR_PDA_SEED_V1`; it also
/// exports the constructors that place them, so a crate that merely READS these
/// addresses takes the domain from `seeds.domain()` rather than naming it. A
/// second spelling is a second source of truth, and the seam audit exists
/// because the first time two spellings drifted apart nobody found out until an
/// address stopped resolving.
fn record_address_v1(seeds: RecordPdaSeedsV1) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            seeds.domain(),
            seeds.schema_release_id().as_bytes(),
            seeds.expected_digest().as_bytes(),
        ],
        &REGISTRY_PROGRAM_ID,
    )
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
    let mut banked = 0_u32;
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
    println!("RECDEPTH\trecord\traw_bump\tstaging_bump\tsearch_cu\tcarried_cu\tsaving_cu\tstatus");
    {
        let seal_search = attempts(seal_bump) * ATTEMPT_COST_CU;
        let seal_carried = ATTEMPT_COST_CU;
        searched += seal_search;
        carried += seal_carried;
        println!(
            "RECDEPTH\tcapability-seal\t{seal_bump}\t-\t{seal_search}\t{seal_carried}\t{}\tCARRIED",
            seal_search.saturating_sub(seal_carried),
        );
    }
    for (name, depth) in first {
        // The realm record is authenticated once inside EACH Custody CPI, and
        // this route makes one. Every other record here is read once.
        let invocations = if *name == "realm" {
            CUSTODY_INVOCATIONS_PER_TRADE_V1
        } else {
            1
        };
        let search = search_cost_cu(*depth) * invocations;
        let carry = carried_cost_cu() * invocations;
        searched += search;
        carried += carry;
        let status = if CARRIED_TODAY.contains(name) {
            banked += search.saturating_sub(carry);
            "CARRIED"
        } else {
            "SEARCHES"
        };
        println!(
            "RECDEPTH\t{name}\t{}\t{}\t{search}\t{carry}\t{}\t{status}",
            depth.raw_bump,
            depth.staging_bump,
            search.saturating_sub(carry),
        );
    }
    println!(
        "RECDEPTH TOTAL if every row searched: {searched} CU; if every row carried: {carried} CU; \
         the difference is {} CU per transaction, of which {banked} is ALREADY BANKED by the rows \
         marked CARRIED and {} is still on the table",
        searched.saturating_sub(carried),
        searched.saturating_sub(carried).saturating_sub(banked),
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
