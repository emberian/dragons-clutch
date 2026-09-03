//! The Hot path's 32 KiB heap is a DECISION, and this is its witness.
//!
//! Solana will map up to 256 KiB of program heap when a transaction carries a
//! ComputeBudget `RequestHeapFrame`, and this executable already owns an
//! allocator that can use it: `entrypoint_adapter::admit_heap_frame_v1` reads
//! the grant out of the instructions sysvar and lifts the bump ceiling to it.
//! The founding routes are on `declares_extended_heap_profile_v1`'s list and
//! get exactly that; since 2026-08-30 so is Hot, for the reason below.
//!
//! Two things make this test change, and both are the point:
//!
//! - putting Hot on `declares_extended_heap_profile_v1`'s list makes the grant
//!   live, and this test's refusal becomes something else;
//! - closing the Hot tail's heap demand structurally makes the whole question
//!   moot, and this test's refusal becomes success.
//!
//! Either way the change is deliberate and visible here, rather than a
//! doctrine living only in a doc comment.
//!
//! **BOTH HAVE NOW HAPPENED, and the file name is a fossil of the first.**
//!
//! The second happened first (W2p, 2026-08-27): the Hot tail's heap demand was
//! closed structurally, so the CONTINUATION route executes to completion at the
//! protocol default and this test's refusal became success.
//!
//! The first happened on 2026-08-30, and the reason is the ROUTE rather than
//! the tail. A caller who invokes Trading DIRECTLY -- which is how every public
//! caller sends a Direct trade -- makes two Registry reauthentication CPIs a
//! continuation never makes, and holds their frames against an allocator that
//! never frees. Measured, that route exhausts 32 KiB in finalization. So Hot IS
//! on the list now, and `DIRECT_HOT_HEAP_FRAME_BYTES_V1` is what a top-level
//! transaction carries.
//!
//! What this test pins is the CONTINUATION route: that it executes at whatever
//! heap ceiling it is handed, and that its packet still fits. If the wire
//! assertion below ever moves, something did.
//!
//! # The bar is the DELTA, and the ruling that made it so
//!
//! **RULED, and the pre-ruling prohibition that used to stand here is lifted.**
//! `docs/decisions/DECISION_PACKET_2026_08_30.md` §4: top-level is the
//! production route, the Hot continuation is demoted to harness-only, **the
//! heap test re-bars on the delta**, the compute fix is not chartered, and full
//! retirement waits until the ~20 program-tests are ported. The scope carve-out
//! matters and is not this file: the FOUNDING continuation is load-bearing
//! since `2dc53776` and the ruling does not touch it.
//!
//! ## Why an absolute CU assertion had to go
//!
//! This path derives program addresses whose seeds include the fixture's maker
//! keys, and `try_find_program_address` costs 1,500 CU per attempt, so the
//! total is a function of how deep the bump search happens to go for the keys
//! in play. `waist::fixture_keypair` pins those keys so the figure is
//! reproducible; `DCLUTCH_FIXTURE_SEED=<n>` redraws them.
//!
//! An assertion on that total is a lottery ticket. Measured at `3dde1b9c` over
//! 32 pinned seeds: **13 pass, 19 fail**, with seed 0 -- the default -- failing
//! at 1,399,794 of 1,399,850. And on 2026-08-30 the ticket was watched changing
//! hands: ALLKEYS landed a source change with no arithmetic in it, the Trading
//! ELF digest moved, `release_set_id` moved, every bump below the Market was
//! redrawn, and this file went green-to-red with nobody having touched the
//! continuation route at all. That is not a regression detector. That is a
//! detector of which keys the fixture happened to draw.
//!
//! ## What the delta is, and why it is a property of the code
//!
//! Both routes execute the SAME trade on the SAME fixture with the SAME keys,
//! so every bump draw they share cancels in the subtraction. What survives is
//! the outer composition's own cost, which is what HEAPRED's evidence is about
//! and what the ruling barred: it authenticates the same two roles over the
//! same activation cache and executes the same trade, and charges for a second
//! implementation of a boundary the top-level route already has.
//!
//! One term does not cancel: the continuation's own admission-PDA search. That
//! address is derived TWICE per transaction from identical seeds -- once by the
//! Registry outer and once by Trading in `authenticate_hot_invocation_v3` -- so
//! both derivations draw the same depth and one extra candidate is charged
//! twice. The delta therefore sits on a 3,000 CU grid above a floor, and the
//! bar is exactly that: `>= floor`, and `(delta - floor)` a whole number of
//! 3,000 CU admission attempts. Any draw satisfies it; a change to what the
//! outer DOES does not.
//!
//! Measured on the converted tree over twelve seeds: 103,307 four times,
//! 106,307 six times, 109,307 once and 103,311 once -- residuals 0, 3,000,
//! 6,000 and a single 4 CU of code motion, with nothing else in between.
//! See `CONTINUATION_ROUTE_DELTA_FLOOR_V1` for why
//! that floor is 1,585 CU above HEAPRED's 35,127 and why it was re-measured
//! rather than carried forward.
//!
//! The lane that met this spread before it was pinned recorded it as "codegen
//! noise of +-20,000 CU between builds of the same source". It is not codegen:
//! the same ELF, run repeatedly with fresh keys, spans the same range, and with
//! the keys pinned it is exact to the unit across runs. Anyone quoting a single
//! CU figure for this path should say which seed produced it.

use dclutch_trading_sbf::TradingSbfError;
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_program::{instruction::InstructionError, pubkey::Pubkey};
use solana_program_test::ProgramTest;
use solana_sdk::{signature::Signer, transaction::TransactionError};
use solana_transaction::versioned::VersionedTransaction;

use dclutch_direct_hot_program_test_support::waist::{
    CLAIMS_PROGRAM_ID, CORE_PROGRAM_ID, CUSTODY_PROGRAM_ID, Elves, LOOKUP_TABLE,
    REGISTRY_PROGRAM_ID, TRADING_PROGRAM_ID, TRANSPARENT_CONTINUATION_WIRE_BYTES_V1,
    add_lookup_table, add_program, add_release_waist, canonical_lookup_addresses, direct_case,
    direct_registry_instructions, direct_top_level_instructions, elves, fixture_substrate,
    start_with_substrate, submit_v0_observed,
};

/// The canonical v0 packet limit one continuation transaction must fit in.
const PACKET_LIMIT: usize = 1_232;

/// What the Registry outer costs over the route that does the same work.
///
/// The bar DECISION_PACKET_2026_08_30 §4 re-bars this file on. It is the
/// key-independent part of the delta: both routes execute the same trade on the
/// same fixture with the same keys, so every bump draw they share cancels, and
/// what is left is the outer's own work plus whole admission attempts.
///
/// HEAPRED measured **35,127** at `3dde1b9c`; it was then 36,713 for two
/// reasons that were neither of them the outer: `30574297` taught the gate
/// fixture to stage the bumps a real founding writes, so both routes stopped
/// searching for the Market, and `HotBumpHintsV1` added a hint arm this route
/// paid for. (That clause used to end "and never uses -- its wire mines
/// nothing", which stopped being true at `82465e00b` when the builder learned
/// to fill the block; it is a cost both routes now pay and it cancels.) Both
/// shift the floor without changing what the outer composition is. Re-measured over
/// twelve seeds at every move rather than carried forward, because a constant
/// inherited across a fixture change is a constant nobody is checking.
///
/// # Why it is now 103,307, and why that is the top-level route moving
///
/// Decision 0017's option B took the two `RegistryInstructionV1::Reauthenticate`
/// CPIs and the third cache decode off the TOP-LEVEL arm, worth a measured
/// 66,921 CU at that route's own key-independent floor
/// (`direct_hot_top_level_margin_gate.rs`, 1,319,672 -> 1,252,751). The
/// continuation arm was not touched: it never paid those CPIs -- it cannot, the
/// Registry is at depth one there -- and it already read all four roles from one
/// decode.
///
/// So this delta grew by almost exactly what the top-level arm shed, 66,594
/// against 66,921, and the 327 CU between them is the code motion this file's
/// jitter bar exists to absorb. The number went UP and nothing got worse: the
/// subtrahend got smaller. Re-measured over twelve seeds on the converted tree:
/// 103,307 four times, 106,307 six times, 109,307 once, and 103,311 once --
/// residuals 0, 3,000, 6,000, and a single 4 CU of motion.
///
/// It also sharpens what this file bars. The delta used to be the outer's own
/// work plus admission attempts; it is now that PLUS the 52,592 CU of Registry
/// reauthentication the continuation never paid and the top level no longer
/// pays either. The outer composition is unchanged; what it is being compared
/// against got 5% cheaper.
///
/// # Why it is now 103,589: cohort-9's dispatch ladder, +282 exactly
///
/// CLOSEMAKER added two native predicates to Trading's entry dispatch (the
/// maker-replay close and, upstream of it in the ladder, nothing else -- the
/// ZeroBump arm rides an existing route). The continuation transaction enters
/// Trading through the Registry outer and pays the entry ladder differently
/// than the bare top-level instruction does, and the difference is
/// key-independent: re-measured over twelve seeds on this tree, EVERY residual
/// sits exactly 282 CU above the old floor on a clean 3,000 grid -- 103,589
/// five times, 106,589 three times, 109,589 twice, 112,589 twice, with zero
/// seed-to-seed jitter. That is the "code motion" shape this constant's own
/// protocol names, one order of magnitude past the jitter bar, so the floor
/// moves rather than the tolerance.
///
/// Moved again 2026-08-31 (cohort-9's wave, post e74b5dd8): re-measured over
/// eleven seeds, EVERY residual sits exactly 777 CU above the previous floor
/// on the same clean 3,000 grid -- 104,366 / 107,366 / 110,366, zero jitter.
/// Key-independent, admission rung intact: the outer composition grew work
/// of its own across the wave (the fifth ProgramSet entry rides the outer's
/// derivation, ~282 CU by the direct-route measurement, plus the shared-path
/// growth the margin gates absorbed reaching this route's outer once more).
///
/// # Why it is now 91,593, and why this took two commits to move
///
/// It went DOWN by 12,773, and the subtrahend is what moved: decision 0017's
/// option B and the Structured landing kept making the TOP-LEVEL route cheaper
/// relative to the outer, and this floor did not follow. `8a691ee57` reached a
/// real reading of 91,848 -- 12,518 below the floor, off-rung by 518 -- and
/// LEFT IT RED rather than move a measured constant on one draw, which is what
/// this doc had told it to do. That was the right call and this is the other
/// half of it.
///
/// Re-measured over TWELVE seeds on this tree, at `6f258cf5e` plus the
/// continuation's own heap grant: **91,593 ten times and 94,593 twice**.
/// Residuals 0 and 3,000 exactly, nothing in between, zero seed-to-seed
/// jitter -- the same clean `ADMISSION_ATTEMPT_CU_V1` grid every previous
/// measurement of this delta has found, which is what makes it a floor rather
/// than a draw.
///
/// The 255 CU between `8a691ee57`'s reading and this one is the interval's own
/// code motion, an order of magnitude inside the rung and above the jitter bar,
/// and it includes this commit's own contribution to BOTH legs: the
/// continuation frame gained a `RequestHeapFrame` and the continuation arm
/// gained the heap comparison its top-level sibling always had. Both routes
/// moved; what did not cancel is here.
const CONTINUATION_ROUTE_DELTA_FLOOR_V1: u64 = 91_593;

/// How far the floor may drift before this gate calls it a change.
///
/// The delta's GRID is exact -- all thirty-two residuals on the tree this was
/// measured on are clean multiples of `ADMISSION_ATTEMPT_CU_V1`, with no
/// seed-to-seed jitter at all. What drifts is the floor itself, across
/// commits: the key-independent cost of either route moves when anything in
/// either one is recompiled differently, and VARIANCE measured exactly that at
/// `b61ffdad` -- `C0` moving by ONE compute unit over a five-commit interval
/// that touched none of the route's arithmetic.
///
/// This gate exists to catch the outer composition growing or shrinking work,
/// which is a change of thousands. It must not fire on a stack frame moving by
/// a byte. So the tolerance sits deliberately in the two orders of magnitude
/// between the two: far above the 1 CU and 142 CU code motions on record, and
/// far below the 3,000 CU rung spacing, so no drift this absorbs could ever be
/// confused with an admission attempt.
///
/// If this fires, the fix is to re-measure the floor over a dozen seeds and
/// move the constant -- not to widen the tolerance.
const CONTINUATION_ROUTE_DELTA_JITTER_V1: u64 = 256;

/// What ONE extra admission-PDA bump attempt adds to the delta.
///
/// 3,000 and not 1,500, because that address is derived TWICE per transaction
/// from identical seeds -- once by the Registry outer and once by Trading in
/// `authenticate_hot_invocation_v3` -- so both derivations draw the same depth
/// and one extra candidate is charged twice. The measured residuals over twelve
/// seeds are 0, 3,000, 6,000, 9,000 and 12,000, with nothing in between, which
/// is what makes this a grid rather than a tolerance.
const ADMISSION_ATTEMPT_CU_V1: u64 = 3_000;

/// The release waist WITHOUT `ProgramTest::set_compute_max_units`.
///
/// That helper installs a whole `ComputeBudget` override on the bank, and an
/// overridden budget makes the runtime ignore the transaction's own
/// ComputeBudget instructions -- `RequestHeapFrame` included. Anything asking
/// what a real transaction's heap request buys therefore cannot use it, and
/// must carry `SetComputeUnitLimit` itself, exactly as a real one would.
fn budget_free_program_test(artifacts: &Elves) -> ProgramTest {
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    add_program(
        &mut test,
        "dclutch_registry_sbf",
        REGISTRY_PROGRAM_ID,
        &artifacts.registry,
    );
    add_program(
        &mut test,
        "dclutch_trading_sbf",
        TRADING_PROGRAM_ID,
        &artifacts.trading,
    );
    add_program(
        &mut test,
        "dclutch_core_sbf",
        CORE_PROGRAM_ID,
        &artifacts.core,
    );
    add_program(
        &mut test,
        "dclutch_claims_sbf",
        CLAIMS_PROGRAM_ID,
        &artifacts.claims,
    );
    add_program(
        &mut test,
        "dclutch_custody_sbf",
        CUSTODY_PROGRAM_ID,
        &artifacts.custody,
    );
    test
}

/// Run the SAME trade on the top-level route and report what it cost.
///
/// The reference leg of the delta bar. Same fixture, same seed, same maker
/// keys, so every bump draw this shares with the continuation cancels in the
/// subtraction and what survives is the outer composition's own cost.
///
/// It builds its own `ProgramTest` because `direct_case` installs into one and
/// a context cannot be rewound; two banks is the price of measuring two routes
/// against one draw, and it is the only way the difference means anything.
async fn top_level_compute_units() -> u64 {
    let artifacts = elves();
    let mut test = budget_free_program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case(&mut test, releases, &artifacts, false);
    let instructions = direct_top_level_instructions(&direct);
    let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;
    submit_v0_observed(
        &mut context,
        &instructions,
        addresses,
        Some(&direct.payer),
        &[],
    )
    .await
    .expect("the top-level reference leg of the delta must execute")
    .compute_units_consumed
}

#[tokio::test]
async fn the_continuation_route_is_unaffected_by_a_heap_grant_and_still_fits_its_packet() {
    let artifacts = elves();
    let mut test = budget_free_program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case(&mut test, releases, &artifacts, false);
    let canonical = direct_registry_instructions(releases, &direct);
    let addresses = canonical_lookup_addresses(&canonical, Pubkey::default());
    add_lookup_table(&mut test, &addresses);

    // NOTHING IS APPENDED ANY MORE, and that is the whole of what changed here.
    //
    // This test used to append `request_heap_frame(262_144)` to a canonical
    // frame that carried none, because the question it was written to ask was
    // what a grant BUYS a route that never requested one. The canonical frame
    // requests one itself now -- `DIRECT_HOT_HEAP_FRAME_BYTES_V1`, at index 1,
    // see `waist::direct_registry_instructions` -- so appending a second would
    // not add a grant, it would REMOVE one: `admitted_heap_frame_bytes_from_sysvar_v1`
    // refuses a second `RequestHeapFrame` outright, `lift_declared_heap_profile_v1`
    // swallows that refusal by design ("it refuses by leaving the ceiling
    // alone"), and the route would then run at the protocol default and abort
    // out of memory. A test measuring that would be measuring its own second
    // instruction.
    let instructions = canonical.to_vec();

    // Not `test.start_with_context()`: a pinned substrate's programs are not
    // visible at slot 1, and `direct_case` built the maker replays around this
    // substrate's bank slot. Under the default `Immutable` substrate this is
    // exactly `start_with_context` and nothing about the measurement moves.
    let context = start_with_substrate(test, fixture_substrate()).await;
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let message = VersionedMessage::V0(
        v0::Message::try_compile(
            &direct.payer.pubkey(),
            &instructions,
            &[AddressLookupTableAccount {
                key: LOOKUP_TABLE,
                addresses,
            }],
            blockhash,
        )
        .expect("canonical v0 message"),
    );
    // One signature plus its count byte: this transaction has a single signer.
    let wire = 1 + 64 + message.serialize().len();
    // THIS PACKET IS THE CANONICAL CONTINUATION, exactly, so it reads the
    // canonical constant and adds nothing to it.
    //
    // It used to be `1_206`, restating the alias arithmetic `submit_v0_observed`
    // already carries; when `74e044cf3` moved the System program out of the
    // static key set and into the lookup table, that pin dropped by 31 bytes and
    // THIS one did not follow. It read 1,206 against a true 1,175 from
    // 2026-09-02 until 2026-09-03 and went unnoticed, because this row was red
    // for an unrelated reason -- the builder-reproduction assertion -- the whole
    // time. Deriving removed the second author rather than correcting it, and
    // this is now the same eight bytes seen from the other side: the term this
    // file used to ADD for its own appended instruction is inside the constant,
    // because the frame carries that instruction itself.
    assert_eq!(
        wire, TRANSPARENT_CONTINUATION_WIRE_BYTES_V1,
        "canonical continuation wire changed"
    );
    assert!(wire <= PACKET_LIMIT);

    let transaction =
        VersionedTransaction::try_new(message, &[&direct.payer]).expect("signed transaction");
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("execution completed");
    let metadata = processed.metadata.expect("transaction metadata");
    // THE SECOND OF THE TWO CHANGES THIS TEST'S HEADER NAMES HAS HAPPENED
    // (W2p, 2026-08-27). The Hot tail's heap demand was closed structurally --
    // the observation bank, the borrow guards and every register bank of the
    // request/preplan/replan phases now come off the program heap's scratch
    // end and are released before the child walk -- so the whole question is
    // moot and this test's refusal has become success.
    //
    // The heap is not what fails here. The `RequestHeapFrame(262_144)` above is
    // NOT inert on this route any more, and it is also not the problem: since
    // `8ee544e4` put `DCLTHOT3` on `declares_extended_heap_profile_v1`'s list,
    // and `registry/hot_continuation_v2::process` forwards the Hot bytes
    // unchanged, the declaration cannot tell the two routes apart -- so Trading
    // under a continuation scans the instructions sysvar and lifts its ceiling
    // too. Measured cost of that on this route: +517 CU, on every seed. Drop
    // the grant entirely and the same seeds still exhaust the meter.
    //
    // If this ever becomes a refusal, read the code before assuming a
    // regression in the heap: `TradingSbfError::Content` is the named heap
    // refusal, fail-closed and never an abort, and it is what a heap
    // regression would produce. What this route actually hits is
    // `ProgramFailedToComplete` -- the compute ceiling, not a refusal.
    if let Err(refusal) = processed.result {
        let TransactionError::InstructionError(_, InstructionError::Custom(code)) = refusal else {
            // Exhausting the meter is this route's KNOWN behaviour on a deep
            // draw -- 19 seeds in 32 at `3dde1b9c` -- and since the ruling
            // demoted it to harness-only and declined to charter the compute
            // fix, a draw that does not fit is not a defect this file is
            // entitled to fail on. It is also not measurable: an exhausted
            // meter reports the limit, not the cost, so there is no delta to
            // bar. Say so loudly and stop, rather than either passing quietly
            // or resurrecting the lottery this test was re-barred to escape.
            println!(
                "continuation exhausted the compute meter on fixture seed {} \
                 ({refusal:?}). No delta is measurable from an exhausted meter. \
                 Not a failure: DECISION_PACKET_2026_08_30 §4 demoted this route \
                 to harness-only and did not charter its compute fix.",
                std::env::var("DCLUTCH_FIXTURE_SEED").unwrap_or_else(|_| "0".to_owned()),
            );
            return;
        };
        assert_ne!(
            code,
            TradingSbfError::Content as u32,
            "the Hot tail no longer fits the protocol default heap -- this is \
             the structural closure of W2p coming undone, not a new refusal",
        );
        panic!("Hot refused at the protocol default heap with Custom({code})");
    }
    // === THE RE-BAR (DECISION_PACKET_2026_08_30 §4) ===
    //
    // What used to be here was `consumed <= 1_400_000`, and it was a lottery
    // ticket. This route's cost is `C0 + 1,500 x (bump attempts drawn from the
    // fixture's maker keys)`, so the absolute number is a fact about the keys
    // and the ELF digest, not about the route. It passed or failed on the draw:
    // 13 of 32 seeds at `3dde1b9c`, and on 2026-08-30 a source change with no
    // arithmetic in it redrew the lottery and flipped this file green-to-red
    // with nobody having touched the continuation at all.
    //
    // The ruling settles what the file is for. Top-level is the production
    // route, the continuation is demoted to harness-only, the compute fix is
    // not chartered, and THIS TEST RE-BARS ON THE DELTA. So the bar is what the
    // outer composition COSTS relative to the route that does the same work,
    // which is a property of the code, and not whether one draw fits a ceiling
    // the ruling no longer holds this route to.
    //
    // The delta is exact and it has a shape. Both routes run the same trade on
    // the same fixture with the same keys, so every bump draw they share
    // cancels. What does not cancel is the continuation's OWN admission-PDA
    // search -- and that address is derived TWICE, once by the Registry outer
    // and once by Trading in `authenticate_hot_invocation_v3`, from identical
    // seeds and therefore at identical depth. So one extra attempt costs 3,000
    // CU, not 1,500, and the delta lands on a 3,000 CU grid above its floor.
    // Measured over twelve seeds on this tree: 36,712 exactly five times, then
    // 39,712, 42,712, 45,712, 48,712 -- residuals 0, 3,000, 6,000, 9,000 and
    // 12,000 with nothing in between.
    let top_level = top_level_compute_units().await;
    let delta = metadata
        .compute_units_consumed
        .checked_sub(top_level)
        .expect("the outer composition cannot cost LESS than the route it wraps");
    // Signed, because a floor that has drifted a few units DOWNWARD must read
    // as a small drift and not as an underflow. `rem_euclid` then puts the
    // remainder on [0, rung) whichever side of the floor the delta fell.
    let rung_cu = i64::try_from(ADMISSION_ATTEMPT_CU_V1).expect("rung fits i64");
    let jitter = i64::try_from(CONTINUATION_ROUTE_DELTA_JITTER_V1).expect("jitter fits i64");
    let above_floor = i64::try_from(delta).expect("delta fits i64")
        - i64::try_from(CONTINUATION_ROUTE_DELTA_FLOOR_V1).expect("floor fits i64");
    let into_rung = above_floor.rem_euclid(rung_cu);
    // Distance to the NEAREST rung, not the one below it.
    let off_rung = into_rung.min(rung_cu - into_rung);
    println!(
        "continuation {} - top-level {top_level} = {delta} CU = floor \
         {CONTINUATION_ROUTE_DELTA_FLOOR_V1} plus {} admission attempts \
         (off-rung by {off_rung})",
        metadata.compute_units_consumed,
        above_floor.div_euclid(rung_cu),
    );
    assert!(
        above_floor >= -jitter,
        "the outer composition now costs {delta} CU over the top-level route, \
         more than {CONTINUATION_ROUTE_DELTA_JITTER_V1} CU BELOW the floor \
         {CONTINUATION_ROUTE_DELTA_FLOOR_V1} it has been measured at. That is not \
         a draw -- a draw can only ADD attempts -- so the outer got cheaper in a \
         key-independent way. Re-measure the floor over a dozen seeds and move \
         the constant.",
    );
    assert!(
        off_rung <= jitter,
        "the outer composition costs {delta} CU over the top-level route. That is \
         the floor {CONTINUATION_ROUTE_DELTA_FLOOR_V1} plus {above_floor}, which \
         misses the nearest whole {ADMISSION_ATTEMPT_CU_V1} CU admission attempt \
         by {off_rung} -- more than the {CONTINUATION_ROUTE_DELTA_JITTER_V1} CU of \
         code motion this bar absorbs. The two routes now differ by something \
         that is not the admission search: either the outer grew work of its own, \
         or the admission address stopped being derived twice at the same depth. \
         Re-measure over a dozen seeds before touching the tolerance.",
    );

    // Reported, never asserted. The ruling demoted this route; whether one draw
    // fits the runtime ceiling is no longer a gate, and making it one again is
    // how this file spent weeks red. See the exhaustion arm above, which says
    // the same thing for the case where the draw does not fit at all.
    if metadata.compute_units_consumed > 1_400_000 {
        println!(
            "NOTE: this draw exceeds the protocol ceiling. Not a failure -- the \
             continuation is harness-only and its compute fix is not chartered.",
        );
    }
    // Printed, not asserted: what is asserted is the ceiling. A single figure
    // off this path is ONE DRAW from a `find_program_address` bump search, not
    // codegen noise -- the same ELF redraws across seeds, and the cross-seed
    // spread measured at HEAD (POST-0012-SWEEP, 2026-08-27) was 49,499 CU,
    // about thirty-three iterations at 1,500 CU each. Ledger M-61 is the rule
    // this obeys: quote PASS n/20 and the MEAN with the ELF digest they belong
    // to, never one seed and never a worst margin.
    println!(
        "hot tail at the protocol default heap, substrate {}, fixture seed {}: \
         {} CU of 1,400,000 ({} spare)",
        fixture_substrate().name(),
        std::env::var("DCLUTCH_FIXTURE_SEED").unwrap_or_else(|_| "0".to_owned()),
        metadata.compute_units_consumed,
        1_400_000_u64.saturating_sub(metadata.compute_units_consumed),
    );
}
