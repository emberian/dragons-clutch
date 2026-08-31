//! Real-ELF interpreted-vs-AOT CU measurement for the Direct V2 relation.
//!
//! Three ELFs run the same 32 register banks:
//!
//! * `dclutch_direct_aot_sbf` — the shipped comparison-only accelerator, which
//!   anchors the measurement to a real artifact rather than to my twin;
//! * `direct_relation_twin_aot` — the twin built with the AOT evaluator;
//! * `direct_relation_twin_interpreted` — the twin built with the interpreter.
//!
//! The two twins are the same source with one differing call, so their CU
//! difference is the evaluator alone. The shipped accelerator is present to
//! prove the twin's frame is faithful: if the twin's AOT build and the shipped
//! ELF disagree on any acknowledgement byte, the comparison is void.

use dclutch_direct_relation_cu_harness::{REQUEST_BYTES, SEED_COUNT, seed_request};
use dclutch_execution_strategy_contract::{AcceleratorAckV1, ExecutionDispositionV1};
use solana_program::{instruction::Instruction, pubkey::Pubkey};
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::signature::Signer;
use solana_transaction::Transaction;

const SHIPPED_ID: Pubkey = Pubkey::new_from_array([0xa7; 32]);
const TWIN_AOT_ID: Pubkey = Pubkey::new_from_array([0xb7; 32]);
const TWIN_INTERPRETED_ID: Pubkey = Pubkey::new_from_array([0xc7; 32]);
const TWIN_NULL_ID: Pubkey = Pubkey::new_from_array([0xd7; 32]);

/// Instructions in the emitted Direct V2 program the interpreter walks.
const DIRECT_V2_INSTRUCTIONS: u64 = 35;

struct Observation {
    compute_units: u64,
    ack: Vec<u8>,
    accepted: bool,
}

async fn submit(
    context: &mut ProgramTestContext,
    program_id: Pubkey,
    data: [u8; REQUEST_BYTES],
) -> Observation {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[Instruction {
            program_id,
            accounts: Vec::new(),
            data: data.to_vec(),
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
        "stateless evaluation must commit: {:?}",
        processed.result
    );
    let metadata = processed.metadata.expect("transaction metadata");
    let returned = metadata.return_data.expect("accelerator return data");
    assert_eq!(returned.program_id, program_id);
    let accepted = AcceleratorAckV1::decode(&returned.data)
        .expect("ack decodes")
        .disposition()
        == ExecutionDispositionV1::Accepted;
    Observation {
        compute_units: metadata.compute_units_consumed,
        ack: returned.data,
        accepted,
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
        "{label:38} n={count:2} floor={floor:7} median={median:7} mean={mean:7} max={ceiling:7} tail=+{:<7}",
        ceiling.saturating_sub(floor)
    );
}

#[tokio::test]
#[ignore = "requires cargo-build-sbf output via SBF_OUT_DIR"]
async fn interpreted_versus_aot_compute_units_on_real_elfs() {
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.add_program("dclutch_direct_aot_sbf", SHIPPED_ID, None);
    test.add_program("direct_relation_twin_aot", TWIN_AOT_ID, None);
    test.add_program("direct_relation_twin_interpreted", TWIN_INTERPRETED_ID, None);
    test.add_program("direct_relation_twin_null", TWIN_NULL_ID, None);
    let mut context = test.start_with_context().await;
    let mut null_cu = Vec::new();

    let mut shipped_cu = Vec::new();
    let mut aot_cu = Vec::new();
    let mut interpreted_cu = Vec::new();
    let mut accepted_aot = Vec::new();
    let mut accepted_interpreted = Vec::new();
    let mut refused_aot = Vec::new();
    let mut refused_interpreted = Vec::new();
    let mut accept_count = 0_usize;

    println!();
    println!("seed  disposition   shipped-AOT   twin-AOT   interpreted      delta    ratio");
    for seed in 0..SEED_COUNT {
        let request = seed_request(seed);
        let shipped = submit(&mut context, SHIPPED_ID, request).await;
        let aot = submit(&mut context, TWIN_AOT_ID, request).await;
        let interpreted = submit(&mut context, TWIN_INTERPRETED_ID, request).await;
        // The null twin accepts unconditionally, so only its cost is read.
        null_cu.push(submit(&mut context, TWIN_NULL_ID, request).await.compute_units);

        assert_eq!(
            shipped.ack, aot.ack,
            "seed {seed}: twin AOT frame diverged from the shipped accelerator"
        );
        assert_eq!(
            aot.ack, interpreted.ack,
            "seed {seed}: refusal equivalence failed -- interpreted and AOT disagreed"
        );
        assert_eq!(aot.accepted, interpreted.accepted);

        if aot.accepted {
            accept_count += 1;
            accepted_aot.push(aot.compute_units);
            accepted_interpreted.push(interpreted.compute_units);
        } else {
            refused_aot.push(aot.compute_units);
            refused_interpreted.push(interpreted.compute_units);
        }

        let delta = interpreted
            .compute_units
            .saturating_sub(aot.compute_units);
        let ratio = if aot.compute_units == 0 {
            0
        } else {
            interpreted.compute_units * 100 / aot.compute_units
        };
        println!(
            "{seed:4}  {:11}   {:9}   {:8}   {:11}   {:8}   {}.{:02}x",
            if aot.accepted { "accepted" } else { "refused" },
            shipped.compute_units,
            aot.compute_units,
            interpreted.compute_units,
            delta,
            ratio / 100,
            ratio % 100
        );

        shipped_cu.push(shipped.compute_units);
        aot_cu.push(aot.compute_units);
        interpreted_cu.push(interpreted.compute_units);
    }

    println!();
    println!(
        "acceptances: {accept_count} of {SEED_COUNT}; refusals: {}",
        SEED_COUNT - accept_count
    );
    summarize("shipped accelerator (all seeds)", &shipped_cu);
    summarize("twin AOT (all seeds)", &aot_cu);
    summarize("twin interpreted (all seeds)", &interpreted_cu);
    if !accepted_aot.is_empty() {
        summarize("twin AOT (accepted)", &accepted_aot);
        summarize("twin interpreted (accepted)", &accepted_interpreted);
    }
    if !refused_aot.is_empty() {
        summarize("twin AOT (refused)", &refused_aot);
        summarize("twin interpreted (refused)", &refused_interpreted);
    }

    summarize("frame only, no relation (all seeds)", &null_cu);

    let aot_floor = aot_cu.iter().copied().min().unwrap_or_default();
    let interpreted_floor = interpreted_cu.iter().copied().min().unwrap_or_default();
    println!();
    println!(
        "whole-invocation floor saving: {} CU ({} -> {})",
        interpreted_floor.saturating_sub(aot_floor),
        interpreted_floor,
        aot_floor
    );

    // Decompose against the accepted path, where every conjunct is walked.
    let frame = null_cu.iter().copied().min().unwrap_or_default();
    let aot_accepted = accepted_aot.iter().copied().min().unwrap_or_default();
    let interpreted_accepted = accepted_interpreted.iter().copied().min().unwrap_or_default();
    let aot_only = aot_accepted.saturating_sub(frame);
    let interpreted_only = interpreted_accepted.saturating_sub(frame);
    println!();
    println!("accepted-path decomposition (floors):");
    println!("  shared frame (decode, 2x sha256, ack encode) : {frame:7} CU");
    println!("  AOT evaluator alone                          : {aot_only:7} CU");
    println!("  interpreted evaluator alone                  : {interpreted_only:7} CU");
    println!(
        "  evaluator-only saving                        : {:7} CU ({}x)",
        interpreted_only.saturating_sub(aot_only),
        if aot_only == 0 {
            0
        } else {
            interpreted_only / aot_only
        }
    );
    println!(
        "  interpreted cost per emitted instruction     : {:7} CU over {DIRECT_V2_INSTRUCTIONS} instructions",
        interpreted_only / DIRECT_V2_INSTRUCTIONS
    );
}
