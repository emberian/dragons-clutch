//! The Hot path's 32 KiB heap is a DECISION, and this is its witness.
//!
//! Solana will map up to 256 KiB of program heap when a transaction carries a
//! ComputeBudget `RequestHeapFrame`, and this executable already owns an
//! allocator that can use it: `entrypoint_adapter::admit_heap_frame_v1` reads
//! the grant out of the instructions sysvar and lifts the bump ceiling to it.
//! Two founding routes are on `declares_extended_heap_profile_v1`'s list and
//! get exactly that.
//!
//! Hot is deliberately NOT on that list, and the reason recorded there is that
//! its continuation packet has no room for the instruction. This test measures
//! that reason instead of restating it, and pins the consequence: a Hot
//! transaction may carry `RequestHeapFrame` and it changes nothing, because the
//! route does not declare the profile.
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
//! **The second one happened (W2p, 2026-08-27).** The Hot tail now executes to
//! completion at the protocol default 32 KiB heap, so what this test asserts is
//! success with the grant still inert. Hot stays off the extended-heap list and
//! the packet assertion below still records why -- but nothing rides on it any
//! more, which is the outcome the decision was holding out for.

use dclutch_trading_sbf::TradingSbfError;
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_program::{
    instruction::{Instruction, InstructionError},
    pubkey::Pubkey,
};
use solana_program_test::ProgramTest;
use solana_sdk::{signature::Signer, transaction::TransactionError};
use solana_sdk_ids::compute_budget;
use solana_transaction::versioned::VersionedTransaction;

use dclutch_direct_hot_program_test_support::waist::{
    CLAIMS_PROGRAM_ID, CORE_PROGRAM_ID, CUSTODY_PROGRAM_ID, Elves, LOOKUP_TABLE,
    REGISTRY_PROGRAM_ID, TRADING_PROGRAM_ID, add_lookup_table, add_program, add_release_waist,
    canonical_lookup_addresses, direct_case, direct_registry_instructions, elves,
};

/// The canonical v0 packet limit one continuation transaction must fit in.
const PACKET_LIMIT: usize = 1_232;

/// `ComputeBudgetInstruction::RequestHeapFrame(bytes)`, hand-encoded.
///
/// Built by hand so this evidence adds no dependency to the harness; the
/// encoding is borsh and the discriminant is the enum's second variant, which
/// is what `entrypoint_adapter::REQUEST_HEAP_FRAME_DISCRIMINANT` pins.
fn request_heap_frame(bytes: u32) -> Instruction {
    compute_budget_instruction(1, &bytes.to_le_bytes())
}

/// `ComputeBudgetInstruction::SetComputeUnitLimit(units)`, hand-encoded.
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

#[tokio::test]
async fn a_requested_heap_frame_is_inert_for_hot_and_does_not_fit_its_packet() {
    let artifacts = elves();
    let mut test = budget_free_program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case(&mut test, releases, &artifacts, false);
    let canonical = direct_registry_instructions(releases, &direct);
    let addresses = canonical_lookup_addresses(&canonical, Pubkey::default());
    add_lookup_table(&mut test, &addresses);

    // APPENDED, never prepended. The Direct native-signature path binds the
    // ed25519 precompile and the continuation to their exact instruction
    // indices, so a prepended instruction shifts both and the transaction
    // refuses for an unrelated reason. The runtime scans the whole message for
    // ComputeBudget instructions, so trailing position costs nothing.
    let mut instructions = Vec::with_capacity(canonical.len() + 2);
    instructions.extend(canonical);
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
    // One signature plus its count byte: this transaction has a single signer.
    let wire = 1 + 64 + message.serialize().len();
    assert!(
        wire > PACKET_LIMIT,
        "the packet fits now -- the recorded reason Hot is off the extended \
         heap profile has expired and the decision should be revisited"
    );

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
    // What it now pins is stronger than what it pinned before: the canonical
    // Direct continuation EXECUTES TO COMPLETION at the protocol default 32
    // KiB heap, with the `RequestHeapFrame(262_144)` above still inert. That
    // is the same inertness as before -- Hot is not on
    // `declares_extended_heap_profile_v1`'s list and the packet assertion
    // above still says why it cannot be -- but now with nothing riding on it.
    //
    // If this ever becomes a refusal again, read the code before assuming a
    // regression in the heap: `TradingSbfError::Content` is the named heap
    // refusal, fail-closed and never an abort, and it is what a heap
    // regression would produce.
    if let Err(refusal) = processed.result {
        let TransactionError::InstructionError(_, InstructionError::Custom(code)) = refusal else {
            panic!("Hot refused outside its own error taxonomy: {refusal:?}");
        };
        assert_ne!(
            code,
            TradingSbfError::Content as u32,
            "the Hot tail no longer fits the protocol default heap -- this is \
             the structural closure of W2p coming undone, not a new refusal",
        );
        panic!("Hot refused at the protocol default heap with Custom({code})");
    }
    assert!(
        metadata.compute_units_consumed <= 1_400_000,
        "the protocol ceiling is the ceiling",
    );
    // Printed, not asserted: the margin is the lane's evidence and a single
    // figure off this path is codegen noise of about +-20,000 CU. What is
    // asserted is the ceiling.
    println!(
        "hot tail at the protocol default heap: {} CU of 1,400,000 ({} spare)",
        metadata.compute_units_consumed,
        1_400_000_u64.saturating_sub(metadata.compute_units_consumed),
    );
}
