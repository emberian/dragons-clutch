//! Real-ELF interpreted-vs-AOT CU measurement for the *current* Direct relation.
//!
//! The V2 measurement in `measure.rs` prices a descriptor the live route no
//! longer runs. This one prices the InlineOrdinary TransitionVMV3 program the
//! route does run: 70 instructions, 1,712 bytes, folded over a three-item
//! Product tail.
//!
//! Building the AOT side at all requires a two-line change to
//! `dclutch-direct-aot-v3-contract`, because that crate cannot compile for
//! `target_os = "solana"` as committed. See the evidence document; the ELFs
//! this test loads were produced with that change applied locally and NOT
//! committed, so a bare checkout cannot reproduce the AOT column without it.
//!
//! Each ELF takes a four-byte seed selector and returns a disposition byte
//! followed by a digest of its output banks, so the two evaluators are checked
//! for agreement on acceptance and on every output byte before any CU number
//! is believed.

use solana_program::{instruction::Instruction, pubkey::Pubkey};
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::signature::Signer;
use solana_transaction::Transaction;

const V3_AOT_ID: Pubkey = Pubkey::new_from_array([0xe7; 32]);
const V3_INTERPRETED_ID: Pubkey = Pubkey::new_from_array([0xf7; 32]);
const V3_NULL_ID: Pubkey = Pubkey::new_from_array([0x17; 32]);
const V3_DECODE_ONLY_ID: Pubkey = Pubkey::new_from_array([0x27; 32]);

/// Seeds, matching the V2 measurement. Thirty-two, never twelve.
const SEED_COUNT: u32 = 32;
/// Instructions in the emitted InlineOrdinary program.
const ORDINARY_V3_INSTRUCTIONS: u64 = 70;
/// Product tail width, matching the gate fixture's canonical three outcomes.
const TAIL_COUNT: u64 = 3;
/// Instruction dispatches the fold actually performs: prelude 66, item stride 3
/// per tail entry, epilogue 1. The fold does not walk all 70 emitted
/// instructions; it walks a prelude, the item range once per Product outcome,
/// and an epilogue.
const ORDINARY_V3_DISPATCHES: u64 = 66 + TAIL_COUNT * 3 + 1;
/// Instructions in the emitted registered ordinary fill program.
const REGISTERED_FILL_V4_INSTRUCTIONS: u64 = 112;
/// Instructions in the emitted Direct V2 descriptor, for the cross-check.
const DIRECT_V2_INSTRUCTIONS: u64 = 35;

struct Observation {
    compute_units: u64,
    report: Vec<u8>,
}

async fn submit(context: &mut ProgramTestContext, program_id: Pubkey, seed: u32) -> Observation {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[Instruction {
            program_id,
            accounts: Vec::new(),
            data: seed.to_le_bytes().to_vec(),
        }],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("Banks RPC");
    assert!(
        processed.result.is_ok(),
        "seed {seed} on {program_id}: stateless evaluation must commit: {:?}",
        processed.result
    );
    let metadata = processed.metadata.expect("transaction metadata");
    let returned = metadata.return_data.expect("return data");
    Observation {
        compute_units: metadata.compute_units_consumed,
        report: returned.data,
    }
}

fn summarize(label: &str, values: &[u64]) {
    let floor = values.iter().copied().min().unwrap_or_default();
    let ceiling = values.iter().copied().max().unwrap_or_default();
    let total: u64 = values.iter().sum();
    let count = values.len() as u64;
    let mean = if count == 0 { 0 } else { total / count };
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let median = sorted.get(sorted.len() / 2).copied().unwrap_or_default();
    println!(
        "{label:40} n={count:2} floor={floor:7} median={median:7} mean={mean:7} max={ceiling:7} tail=+{}",
        ceiling.saturating_sub(floor)
    );
}

#[tokio::test]
#[ignore = "requires cargo-build-sbf output via SBF_OUT_DIR and a locally patched aot-v3 crate"]
async fn current_relation_interpreted_versus_aot_on_real_elfs() {
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.add_program("direct_relation_twin_v3_aot", V3_AOT_ID, None);
    test.add_program("direct_relation_twin_v3_interpreted", V3_INTERPRETED_ID, None);
    test.add_program("direct_relation_twin_v3_null", V3_NULL_ID, None);
    test.add_program(
        "direct_relation_twin_v3_decode_only",
        V3_DECODE_ONLY_ID,
        None,
    );
    let mut context = test.start_with_context().await;
    let mut decode_only_cu = Vec::new();

    let mut aot_cu = Vec::new();
    let mut interpreted_cu = Vec::new();
    let mut null_cu = Vec::new();
    let mut accepted_aot = Vec::new();
    let mut accepted_interpreted = Vec::new();
    let mut refused_aot = Vec::new();
    let mut refused_interpreted = Vec::new();
    let mut accept_count = 0_u32;

    println!();
    println!("seed  disposition     twin-AOT   interpreted      delta    ratio");
    for seed in 0..SEED_COUNT {
        let aot = submit(&mut context, V3_AOT_ID, seed).await;
        let interpreted = submit(&mut context, V3_INTERPRETED_ID, seed).await;
        null_cu.push(submit(&mut context, V3_NULL_ID, seed).await.compute_units);
        decode_only_cu.push(
            submit(&mut context, V3_DECODE_ONLY_ID, seed)
                .await
                .compute_units,
        );

        assert_eq!(
            aot.report, interpreted.report,
            "seed {seed}: the AOT translation and the interpreter disagreed on the current relation"
        );
        let accepted = aot.report.first().copied() == Some(1);

        if accepted {
            accept_count += 1;
            accepted_aot.push(aot.compute_units);
            accepted_interpreted.push(interpreted.compute_units);
        } else {
            refused_aot.push(aot.compute_units);
            refused_interpreted.push(interpreted.compute_units);
        }

        let delta = interpreted.compute_units.saturating_sub(aot.compute_units);
        let ratio = if aot.compute_units == 0 {
            0
        } else {
            interpreted.compute_units * 100 / aot.compute_units
        };
        println!(
            "{seed:4}  {:11}   {:8}   {:11}   {:8}   {}.{:02}x",
            if accepted { "accepted" } else { "refused" },
            aot.compute_units,
            interpreted.compute_units,
            delta,
            ratio / 100,
            ratio % 100
        );
        aot_cu.push(aot.compute_units);
        interpreted_cu.push(interpreted.compute_units);
    }

    println!();
    println!(
        "acceptances: {accept_count} of {SEED_COUNT}; refusals: {}",
        SEED_COUNT - accept_count
    );
    summarize("v3 AOT (all seeds)", &aot_cu);
    summarize("v3 interpreted (all seeds)", &interpreted_cu);
    if !accepted_aot.is_empty() {
        summarize("v3 AOT (accepted)", &accepted_aot);
        summarize("v3 interpreted (accepted)", &accepted_interpreted);
    }
    if !refused_aot.is_empty() {
        summarize("v3 AOT (refused)", &refused_aot);
        summarize("v3 interpreted (refused)", &refused_interpreted);
    }
    summarize("v3 surrounding work, no relation", &null_cu);

    let frame = null_cu.iter().copied().min().unwrap_or_default();
    let aot_accepted = accepted_aot.iter().copied().min().unwrap_or_default();
    let interpreted_accepted = accepted_interpreted.iter().copied().min().unwrap_or_default();
    let aot_only = aot_accepted.saturating_sub(frame);
    let interpreted_only = interpreted_accepted.saturating_sub(frame);

    summarize("v3 decode only, no fold", &decode_only_cu);
    let decode_only = decode_only_cu.iter().copied().min().unwrap_or_default();
    let full_decode = decode_only.saturating_sub(frame);
    let fold_only = interpreted_accepted.saturating_sub(decode_only);

    println!();
    println!("accepted-path decomposition (floors):");
    println!("  shared bank build and transition encode : {frame:7} CU");
    println!("  AOT evaluator alone                     : {aot_only:7} CU");
    println!("  interpreted, decode plus fold           : {interpreted_only:7} CU");
    println!("    of which full decode (shape + body)   : {full_decode:7} CU");
    println!("    of which the fold alone               : {fold_only:7} CU");

    println!();
    println!("what the LIVE ROUTE actually pays:");
    println!(
        "  the route calls TransitionProgramV3::from_sealed (hot_v3.rs:2250), which skips"
    );
    println!(
        "  validate_body -- the per-instruction sweep -- because the write-once seal already"
    );
    println!("  ran it. So the route pays the cheap shape decode plus the fold, NOT {full_decode} CU");
    println!("  of decode. The honest per-invocation saving is the fold against the AOT:");
    println!(
        "  route-relevant saving                   : {:7} CU ({} -> {})",
        fold_only.saturating_sub(aot_only),
        fold_only,
        aot_only
    );

    println!();
    let per_dispatch = fold_only / ORDINARY_V3_DISPATCHES;
    println!(
        "  the fold executes {ORDINARY_V3_DISPATCHES} dispatches (prelude 66 + {TAIL_COUNT} items x 3 + epilogue 1),"
    );
    println!("  not the {ORDINARY_V3_INSTRUCTIONS} emitted instructions: {per_dispatch} CU per dispatch.");
    println!(
        "  (V2's {DIRECT_V2_INSTRUCTIONS}-instruction descriptor is a different shape; rates are not transferable.)"
    );
    println!(
        "  registered fill v4 would add {REGISTERED_FILL_V4_INSTRUCTIONS} instructions, but it is a"
    );
    println!("  separate capability and is not on this route.");
}
