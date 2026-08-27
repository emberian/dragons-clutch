//! What the canonical Direct bundle spends PAST the 32 KiB heap wall.
//!
//! This is a measuring instrument, not a gate, and it does not assert an
//! outcome: `registry_hot_continuation` owns the verdict and
//! `hot_heap_frame_is_inert` owns the heap decision. What this owns is the only
//! way to see the tail at all -- the six lifecycle creates and the child role
//! CPIs -- while the shipped run still refuses at `pf-enter` with the heap
//! exhausted. Two lanes rebuilt this as a throwaway patch before it was
//! written down; this is it, written down.
//!
//! It needs the `hot-cu-profile` Trading ELF, whose
//! `entrypoint_adapter::hot_cu_profile_lifts_every_route_v1` makes the
//! `RequestHeapFrame` below land. Against a shipped ELF the grant is inert and
//! this prints the same refusal `registry_hot_continuation` reports.
//!
//! ```text
//! cargo build-sbf --manifest-path programs/dclutch-trading-sbf/Cargo.toml \
//!     --features hot-cu-profile --sbf-out-dir <dir>
//! # plus registry, core, claims and custody into the same <dir>
//! SBF_OUT_DIR=<dir> cargo test \
//!     --manifest-path programs/dclutch-trading-sbf/program-test/Cargo.toml \
//!     --test w2l_tail_probe -- --nocapture
//! ```

use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_program::{instruction::Instruction, pubkey::Pubkey};
use solana_program_test::ProgramTest;
use solana_sdk::signature::Signer;
use solana_sdk_ids::compute_budget;
use solana_transaction::versioned::VersionedTransaction;

use dclutch_direct_hot_program_test_support::waist::{
    CLAIMS_PROGRAM_ID, CORE_PROGRAM_ID, CUSTODY_PROGRAM_ID, Elves, LOOKUP_TABLE,
    REGISTRY_PROGRAM_ID, TRADING_PROGRAM_ID, add_lookup_table, add_program, add_release_waist,
    canonical_lookup_addresses, direct_case, direct_registry_instructions, elves,
};

fn request_heap_frame(bytes: u32) -> Instruction {
    compute_budget_instruction(1, &bytes.to_le_bytes())
}

fn set_compute_unit_limit(units: u32) -> Instruction {
    compute_budget_instruction(2, &units.to_le_bytes())
}

fn compute_budget_instruction(discriminant: u8, payload: &[u8]) -> Instruction {
    let mut data = Vec::with_capacity(1 + payload.len());
    data.push(discriminant);
    data.extend_from_slice(payload);
    Instruction {
        program_id: Pubkey::new_from_array(compute_budget::ID.to_bytes()),
        accounts: Vec::new(),
        data,
    }
}

fn budget_free_program_test(artifacts: &Elves) -> ProgramTest {
    // No `set_compute_max_units`: it installs a whole `ComputeBudget` override
    // on the bank, and an overridden budget makes the runtime ignore the
    // transaction's own ComputeBudget instructions -- `RequestHeapFrame`
    // included. Measured: with the override in place the grant never lands and
    // the run dies on an access violation writing 1,464 bytes at the 32 KiB
    // boundary instead of reaching the tail.
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

#[tokio::test]
async fn the_hot_tail_is_measurable_on_a_diagnostically_lifted_heap() {
    let artifacts = elves();
    let mut test = budget_free_program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case(&mut test, releases, &artifacts, false);
    let canonical = direct_registry_instructions(releases, &direct);
    let addresses = canonical_lookup_addresses(&canonical, Pubkey::default());
    add_lookup_table(&mut test, &addresses);

    let mut instructions = Vec::with_capacity(canonical.len() + 2);
    instructions.extend(canonical);
    // APPENDED, never prepended: the Direct native-signature path binds the
    // ed25519 precompile and the continuation to their exact instruction
    // indices. The runtime scans the whole message for ComputeBudget
    // instructions, so trailing position costs nothing.
    instructions.push(request_heap_frame(262_144));
    instructions.push(set_compute_unit_limit(1_400_000));

    let context = test.start_with_context().await;
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
    let transaction =
        VersionedTransaction::try_new(message, &[&direct.payer]).expect("signed transaction");
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("execution completed");
    let metadata = processed.metadata.expect("transaction metadata");
    // The tables live in the `--nocapture` log, not in an assertion: this test
    // exists to make the tail READABLE, and pinning a compute figure here would
    // turn every unrelated improvement into a failing test.
    println!("hot-tail: consumed {}", metadata.compute_units_consumed);
    println!("hot-tail: result {:?}", processed.result);
}
