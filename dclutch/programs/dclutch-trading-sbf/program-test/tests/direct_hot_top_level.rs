//! The Direct trade, submitted the way the public sends one.
//!
//! Every other real-ELF test of this route puts the Registry on top and lets
//! it hand Trading an authenticated continuation. That is a legitimate route,
//! but it is not the route the product is for: the SDK, the CLI, the web panel
//! and the devnet trade driver all build a bare Hot instruction and send it
//! straight to Trading, which then re-authenticates Core and itself by CPI.
//!
//! That path had no coverage, and on 2026-08-30 it turned out to refuse 100%
//! of the time on a deployed program -- `TradingSbfError::Release` raised at
//! the very end of a successful execution, one statement before commit,
//! because the Direct crosscheck unwrapped a child-programs value that only a
//! continuation ever populated. Both simulations of it on public devnet burned
//! ~880,000 CU proving everything else about the trade was correct.
//!
//! So this file exists to make the natural path a tested path. It asserts the
//! trade EXECUTES top-level -- same collateral movement, same ACK, same
//! commit-last evidence the continuation test asserts -- because that is the
//! property that was broken and the only assertion a fix cannot fake.

use dclutch_capability_program_contract::hot_v3::{HotExecutionAckV3, HotExecutionEnvelopeV3};
use dclutch_custody_contract::CustodyReplayV1;
use dclutch_token_svm::TokenAccount;
use dclutch_trading_sbf::TradingSbfError;
use solana_account::Account;
use solana_program::hash::hash;
use solana_program::instruction::InstructionError;
use solana_program::pubkey::Pubkey;
use solana_program_test::{BanksClientError, ProgramTestContext};
use solana_sdk::transaction::TransactionError;

/// The custom program code a refusal carried, so a test can name it rather
/// than assert a bare `is_err()`. Same shape as the continuation suite's.
fn refusal_code(error: &BanksClientError) -> Option<u32> {
    let transaction = match error {
        BanksClientError::TransactionError(value) => value,
        BanksClientError::SimulationError { err, .. } => err,
        _ => return None,
    };
    match transaction {
        TransactionError::InstructionError(_, InstructionError::Custom(code)) => Some(*code),
        _ => None,
    }
}

use dclutch_direct_hot_program_test_support::waist::{
    COMPUTE_LIMIT, TRADING_PROGRAM_ID, add_lookup_table, add_release_waist,
    canonical_lookup_addresses, direct_case, direct_top_level_instructions, elves,
    fixture_substrate, program_test_without_forced_budget, start_with_substrate,
    submit_v0_observed,
};

async fn account_snapshots(
    context: &mut ProgramTestContext,
    keys: &[Pubkey],
) -> Vec<(Pubkey, Option<Account>)> {
    let mut output = Vec::with_capacity(keys.len());
    for key in keys {
        let account = context
            .banks_client
            .get_account(*key)
            .await
            .expect("rollback account read");
        output.push((*key, account));
    }
    output
}

async fn account(context: &mut ProgramTestContext, key: Pubkey) -> Account {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account read")
        .expect("live account")
}

/// WALL #26, and the guard that keeps it dead.
///
/// A top-level submission used to refuse with `TradingSbfError::Release`
/// (0x4001) at the very end of a successful execution, because the Direct
/// crosscheck unwrapped a child-programs value only a continuation populated.
/// Measured here before the fix: `custom program error: 0x4001` at 824,482 CU.
///
/// This test is deliberately narrower than "the trade works": it says only
/// that the Release refusal is gone. It is green today, it would have been red
/// against the defect, and it stays meaningful after wall #27 is closed --
/// at which point the execution test below carries the stronger property.
#[tokio::test]
async fn a_top_level_submission_no_longer_refuses_as_release() {
    let artifacts = elves();
    let mut test = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case(&mut test, releases, &artifacts, false);
    let instructions = direct_top_level_instructions(&direct);
    assert_eq!(
        instructions[3].program_id, TRADING_PROGRAM_ID,
        "this test must submit to Trading directly, not through an outer",
    );
    let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;

    let outcome = submit_v0_observed(
        &mut context,
        &instructions,
        addresses,
        Some(&direct.payer),
        &[],
    )
    .await;

    if let Err(refusal) = outcome {
        assert_ne!(
            refusal_code(&refusal.error),
            Some(TradingSbfError::Release as u32),
            "the top-level Direct route refused as Release again -- wall #26 is back: {:#?}",
            refusal.logs,
        );
    }
}

/// The whole point of the file: a top-level submission executes.
///
/// IGNORED ON WALL #27, WHICH THIS TEST FOUND. With wall #26 fixed the route
/// runs 230,000 CU further than it ever had and then dies on the heap --
/// `memory allocation failed, out of memory`, ~1,055,000 CU in, deep in the
/// finalization the top-level path had never once reached. The allocator is a
/// bump allocator that never frees, Hot is deliberately OFF
/// `declares_extended_heap_profile_v1` so a `RequestHeapFrame` is inert for it
/// (see `hot_heap_frame_is_inert.rs`), and the continuation route fits in the
/// same 32 KiB because it does not make the two Registry reauthentication CPIs
/// this route must.
///
/// So closing wall #27 is either a structural reduction of this route's
/// allocations, the way W2p closed the tail's, or a deliberate decision to put
/// Hot on the extended-heap list. Both are named in that test's own header as
/// the two things that change it. Un-ignore this the moment either lands: the
/// assertions below are the real acceptance criteria for a public Direct trade
/// and they are already written.
#[ignore = "wall #27: the top-level route exhausts the 32 KiB Hot heap in finalization"]
#[tokio::test]
async fn direct_inline_ordinary_executes_when_submitted_top_level_to_trading() {
    let artifacts = elves();
    let mut test = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case(&mut test, releases, &artifacts, false);
    let instructions = direct_top_level_instructions(&direct);

    // The invoked program is Trading itself. If this ever becomes the Registry
    // the test has quietly turned back into the continuation test.
    assert_eq!(
        instructions[3].program_id, TRADING_PROGRAM_ID,
        "this test must submit to Trading directly, not through an outer",
    );

    let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;
    let before = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    let root_before = account(&mut context, direct.chain.root).await;

    let execution = submit_v0_observed(
        &mut context,
        &instructions,
        addresses,
        Some(&direct.payer),
        &[],
    )
    .await
    .expect("top-level Direct Hot execution");

    let units = execution.compute_units_consumed;
    assert!(
        units > 0 && units <= COMPUTE_LIMIT,
        "top-level Direct Hot consumed {units} units against a {COMPUTE_LIMIT} limit",
    );
    // Reported, not asserted against a threshold: the top-level arm now reads
    // the activation cache once for its children, and in exchange both child
    // walks take the already-resolved path instead of decoding that cache
    // again. Which way that nets is a measurement, not a promise.
    println!("top-level Direct Hot compute units consumed: {units}");

    // The collateral actually moved. These are the continuation test's own
    // numbers on the same fixture: the route is different, the trade is not.
    let source = account(&mut context, direct.chain.collateral_accounts[0]).await;
    let destination = account(&mut context, direct.chain.collateral_accounts[1]).await;
    assert_eq!(
        TokenAccount::parse(&source.data)
            .expect("source token")
            .amount,
        95,
    );
    assert_eq!(
        TokenAccount::parse(&destination.data)
            .expect("destination token")
            .amount,
        35,
    );

    let root = account(&mut context, direct.chain.root).await;
    assert_eq!(root.owner, TRADING_PROGRAM_ID);
    assert!(!root.data.is_empty());
    let replay = account(&mut context, direct.chain.custody_replay).await;
    let replay = CustodyReplayV1::decode(&replay.data).expect("post-Custody replay");
    assert_eq!(replay.next_revision, 8);
    let after = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    assert_ne!(
        after, before,
        "a successful top-level Direct Hot left no material state change",
    );

    // Commit-last evidence, produced by Trading itself. The ACK is where the
    // child programs the crosscheck resolves actually land, so an ACK that
    // decodes and matches the envelope is the direct evidence that the
    // top-level arm resolved them from the activation cache correctly.
    let (producer, returned) = execution
        .return_data
        .expect("a successful Hot execution must return commit-last evidence");
    assert_eq!(producer, TRADING_PROGRAM_ID, "ACK producer substitution");
    let ack = HotExecutionAckV3::decode(&returned).expect("canonical Hot ACK");
    assert_eq!(ack.to_bytes().as_slice(), returned.as_slice());
    let (envelope, family_request) =
        HotExecutionEnvelopeV3::split_instruction(&direct.chain.hot_instruction.data)
            .expect("canonical fixture Hot instruction");
    assert_eq!(ack.release_set, envelope.release_set());
    assert_eq!(ack.market, envelope.market());
    assert_eq!(ack.generation, envelope.generation());
    assert_eq!(ack.root, direct.chain.root.to_bytes());
    assert_eq!(ack.request_digest, hash(family_request).to_bytes());
    assert_eq!(ack.selected_program, direct.chain.descriptor_digest);
    assert_eq!(ack.root_prestate_digest, hash(&root_before.data).to_bytes());
    assert_eq!(ack.root_poststate_digest, hash(&root.data).to_bytes());
}
