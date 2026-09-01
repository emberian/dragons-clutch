//! Registered Sell then Buy, submitted as two real transactions on one bank.
//!
//! `a9181daf` built `build_direct_registered_creation_chain_fixture_v4` and
//! proved it agrees with itself: nineteen unit cases over its accounts, its
//! seals, its PDAs and its poststates. What it could not prove is the only
//! thing that matters about a creation route -- that a deployed program accepts
//! it. Until this file, **no registered order had ever been created on a
//! chain**, by any test, in any harness. So every fact below either moves from
//! "the fixture says" to "the ELF did", or names the exact wall that stopped it.
//!
//! # What this file asserts today, and why it is not the campaign
//!
//! On the current release the campaign is BLOCKED, at two independent walls
//! that no fixture can route around. Both are named here by their exact
//! discriminant, at their exact measured depth, because a wall that is only
//! described drifts and a wall that is only `is_err()` is indistinguishable
//! from any other refusal.
//!
//! **Wall A -- `hot_v3` admits exactly one Direct action through generic Hot.**
//! `prepare_direct_inline_hot_crosscheck_v3` opens
//! (`programs/dclutch-trading-sbf/src/hot_v3.rs`):
//!
//! ```text
//! if selected_kind != DIRECT_SUCCESSOR_KIND_ID_V3 { return Ok(None); }
//! if selected_action != DirectExecutionActionV3::InlineOrdinary as u32 {
//!     return Err(TradingSbfError::UnsupportedContent.into());
//! }
//! ```
//!
//! A foreign KIND passes through with no crosscheck; a Direct kind carrying any
//! action but `InlineOrdinary` is refused outright. That is every registered
//! action there is -- RegisterSell, RegisterBuy, FillRegisteredOrdinary,
//! CancelRegistered, ExpireRegistered, CloseInvalidated, CloseMakerReplay,
//! CloseDirectRoot, both splits and both merges. The refusal lands AFTER the
//! whole action has authenticated, planned, projected and preflighted its
//! children, which is what `registered_sell_creation_refuses_at_the_direct_
//! action_wall` measures: the Sell is otherwise complete when it dies.
//!
//! **Wall B is CROSSED.** It said one manifest entry per root pins one
//! lifecycle policy, so the Sell's and the Buy's `LifecyclePolicyV5` -- which
//! differed by the Custody replay and vault quotes -- could not share a Direct
//! root. Direct now emits ONE policy for both sides (`53da59a4`, `6fed9720`),
//! and `the_two_registered_creation_lifecycle_policies_cannot_share_a_manifest_
//! entry` stays below as the record of WHY it existed; the case that ends it is
//! `registered_state_artifacts_v4::unified_lifecycle_tests::
//! one_policy_serves_both_registered_creation_actions`. A recorded wall decays
//! exactly like a recorded total; this one did.
//!
//! **Wall C, measured 2026-09-01, is the commit's lamport plan.** Behind the
//! wall-A probe, on a `hot-cu-profile` build (so these figures are diagnostic
//! and not comparable with the production numbers below), the registered Sell
//! executes and the registered Buy now runs ALL THREE of its Custody children
//! to success -- `InitializeReplay` 123,796 CU, `OpenVault` 141,105 CU, the
//! delegated deposit 136,253 CU -- and then refuses `Commit` 0x4005 at
//! 1,205,519 CU in `require_committed_rent_exemption_v3`, on **coordinate 20,
//! the Custody replay: 0 lamports against 288 bytes needing 2,895,360**.
//!
//! The cause is one line up. `output_lamports` is seeded from the OBSERVED
//! prestate, and at observation the replay is vacant, so the plan says zero;
//! `commit_output_lamports_v3` then writes that zero back over the rent the
//! Custody child just deposited. It skips only coordinates an
//! `EffectProgramV5` FUNDING ACTION names, and funding actions describe
//! accounts TRADING creates through the rent lifecycle -- the maker replay and
//! the record. Nothing in the declaration says "this coordinate is created and
//! funded by a child route," which no family needed before: the inline family
//! opens no Custody account, and registered creation is the first route in the
//! protocol that does. Declaring it as a local `TransferLamports` is not the
//! repair -- `require_child_disjoint_from_local` refuses a child invocation
//! that reaches a coordinate the Effect's own operations mutate, and
//! coordinate 20 is in the child's frame. The missing thing is an exemption,
//! and it is family-neutral machinery, not a Direct artifact.
//!
//! Neither wall may be relaxed to make a campaign pass.
//! `registered_sell_then_buy_execute_on_current_elves` below is the complete
//! acceptance gate, written and ignored. Removing its `#[ignore]` is the one
//! edit that turns a wall closure into evidence.
//!
//! # What IS live here, measured on the current ELF pack
//!
//! ```text
//! release substitution      Content            0x4003     39,036 CU
//! heap grant omitted        HeapFrame          0x4008     41,115 CU
//! Buy without its Sell      Root               0x4002     42,159 CU
//! WALL A: registered Sell   UnsupportedContent 0x4000    329,791 CU
//! ```
//!
//! Re-measured 2026-09-01 at `66566cd4` on ELFs built from that commit. Three
//! of the four moved by single-digit or low-thousand CU against the figures
//! this table carried before, which is the artifact band doing what ledger
//! `M-61` says it does: the registered creation artifacts changed, so every
//! fixture seed did.
//!
//! The ordering is the evidence, not the absolute figures. Three refusals that
//! belong in the prologue cost about 40,000 CU each; wall A costs 7.8 times
//! that, because the Sell is otherwise a complete, admitted, fully preflighted
//! action by the time its ACTION is rejected. Every one of the four rolls back
//! every snapshot key and reaches no child program.
//!
//! # The two transactions are one campaign, not two cases
//!
//! `RegisterSell` advances the mutable Direct root from zero open maker roots
//! to one; `RegisterBuy` authenticates *that exact poststate* as its own
//! prestate and opens the ordered Custody replay/vault/deposit chain on top of
//! it. So they share one account installation, one bank and one slot, and the
//! second is submitted only after the first has committed. Splitting them
//! across two `ProgramTest` banks would test two independent creations and
//! would silently stop testing the chain.
//!
//! # The Rent program is a live participant, not a placeholder
//!
//! Both creations fund a maker replay and a registered record against ONE
//! Market-lifecycle `LifecycleRentCreditV2`, and that credit is owned by the
//! Rent program (`programs/dclutch-rent-sbf`), not by the Registry.
//! `hot_v3::authenticate_lifecycle_credit_v3` re-derives the credit's own
//! address under `account.owner` and separately requires that owner to be
//! present in the frame as an executable readonly account, which is why the
//! registered fixture hands `rent_program` to the harness as an externally
//! installed key rather than staging it itself.
//!
//! This file stages it as a real upgradeable deployment of the real Rent ELF.
//! The profile rule is `opaque(executable)` and would also admit a one-lamport
//! stub with no data at all, which is exactly the reason not to use one: the
//! stub proves the rule and nothing about the deployment the rule stands for.
//!
//! # Running it
//!
//! ```text
//! for p in dclutch-registry-sbf dclutch-trading-sbf dclutch-core-sbf \
//!          dclutch-claims-sbf dclutch-custody-sbf dclutch-rent-sbf; do
//!   cargo build-sbf --manifest-path "programs/$p/Cargo.toml" --sbf-out-dir <elves>
//! done
//! SBF_OUT_DIR=<elves> cargo test \
//!   --manifest-path programs/dclutch-trading-sbf/program-test/Cargo.toml \
//!   --test direct_registered_creation_hot -- --nocapture --test-threads=1
//! ```
//!
//! Six ELFs, not five: `elves()` reads the five release roles and this campaign
//! reads the Rent deployment beside them.

use dclutch_capability_program_contract::hot_v3::{
    DIRECT_HOT_HEAP_FRAME_BYTES_V1, HOT_EXECUTION_ENVELOPE_BYTES_V3, HotExecutionAckV3,
    HotExecutionEnvelopeV3,
};
use dclutch_custody_contract::CustodyReplayV1;
use dclutch_direct_codec::execution_v3::{
    DIRECT_REGISTRATION_REQUEST_BYTES_V3, DirectExecutionActionV3, native_signature_slice_v3,
};
use dclutch_direct_codec::native_evidence_v3::{
    DirectNativeEvidenceContainerV3, direct_native_evidence_bytes_v3,
    encode_direct_native_evidence_many_v3_atomic,
};
use dclutch_direct_codec::ordinary_geometry_v3::DirectOrdinaryGeometryV3;
use dclutch_direct_codec::registered_state_artifacts_v4::{
    DIRECT_REGISTER_BUY_LIFECYCLE_BYTES_V5, DIRECT_REGISTER_SELL_LIFECYCLE_BYTES_V5,
    DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5,
};
use dclutch_direct_codec::successor::{
    DIRECT_MAKER_REPLAY_BYTES_V1, DIRECT_REGISTERED_RECORD_BYTES_V2,
};
use dclutch_direct_hot_program_test_support::chain::install_direct_hot_chain_accounts_v5;
use dclutch_direct_hot_program_test_support::fixture::{
    DirectRegisteredCreationChainFixtureV4, DirectTradeScenarioV1,
    build_direct_registered_creation_chain_fixture_v4,
};
use dclutch_direct_hot_program_test_support::waist::{
    CLAIMS_PROGRAM_ID, COMPUTE_LIMIT, CUSTODY_PROGRAM_ID, Elves, RENT_PROGRAM_ID, RefusedExecution,
    Releases, SuccessfulExecution, TRADING_PROGRAM_ID, add_lookup_table, add_program,
    add_release_waist, canonical_lookup_addresses, direct_chain_input_v5, elves, fixture_substrate,
    program_test_without_forced_budget, start_with_substrate, submit_v0_observed,
};
use dclutch_rent_contract::lifecycle_v2::{LIFECYCLE_RENT_CREDIT_BYTES_V2, LifecycleRentCreditV2};
use dclutch_token_svm::TokenAccount;
use dclutch_trading_sbf::TradingSbfError;
use solana_account::Account;
use solana_program::hash::hash;
use solana_program::instruction::{Instruction, InstructionError};
use solana_program::pubkey::Pubkey;
use solana_program::rent::Rent;
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::TransactionError;
use solana_sdk_ids::{compute_budget, ed25519_program, system_program, sysvar};
use std::{env, fs, path::PathBuf};

/// The custom program code a refusal carried, so a case can name it rather
/// than assert a bare `is_err()`. Same shape as the sibling suites'.
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

/// The real Rent ELF, from the same directory the five role ELFs come from.
///
/// `elves()` deliberately does not read it: the Rent program is not one of the
/// five `ExecutionRoleV1` roles and is not in the activated release set, so it
/// has no release, no ProgramData width and no artifact identity in the waist.
/// It is a plain deployment that owns a PDA namespace, and this is where a
/// registered-creation campaign says so.
fn rent_elf() -> Vec<u8> {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    fs::read(directory.join("dclutch_rent_sbf.so")).expect("real Rent ELF")
}

/// `waist::fixture_keypair`, which is private to that module.
///
/// Reproduced rather than exported because these three keys are an input to
/// `direct_chain_input_v5`, which draws them itself: if this drifts from the
/// waist's derivation the fixture will name makers whose signatures this file
/// cannot produce, and the campaign will refuse `NativeSignature` for a reason
/// that has nothing to do with Direct. The seed read is identical for the same
/// reason -- a sweep must move both sides together.
fn fixture_keypair(role: u8) -> Keypair {
    let seed = env::var("DCLUTCH_FIXTURE_SEED")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let mut secret = [0_u8; 32];
    secret[0] = role;
    secret[1..9].copy_from_slice(&seed.to_le_bytes());
    Keypair::new_from_array(secret)
}

/// One installed registered-creation campaign.
struct CreationCase {
    fixture: DirectRegisteredCreationChainFixtureV4,
    payer: Keypair,
    makers: [Keypair; 2],
}

/// Install the whole campaign: payer, clock, Rent deployment, chain accounts.
///
/// Mirrors `waist::direct_case_v5` for the registered fixture, which cannot use
/// it: `direct_case_v5` builds the ORDINARY chain, and the registered fixture
/// hands the Rent program to the harness where the ordinary one stages a stub
/// itself.
fn creation_case(test: &mut ProgramTest, releases: Releases, artifacts: &Elves) -> CreationCase {
    let substrate = fixture_substrate();
    let payer = fixture_keypair(0);
    let makers = [fixture_keypair(1), fixture_keypair(2)];
    let clock = solana_program::clock::Clock {
        slot: substrate.bank_slot(),
        ..solana_program::clock::Clock::default()
    };
    test.add_sysvar_account(sysvar::clock::ID, &clock);
    test.add_account(
        payer.pubkey(),
        Account {
            lamports: 10_000_000_000,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    // The one account this campaign installs that the ordinary campaign does
    // not, and the reason the file exists as its own harness.
    add_program(test, "dclutch_rent_sbf", RENT_PROGRAM_ID, &rent_elf());
    let input = direct_chain_input_v5(
        releases,
        artifacts,
        substrate,
        DirectOrdinaryGeometryV3::CANONICAL,
        DirectTradeScenarioV1::ZERO_FEE,
    );
    assert_eq!(input.rent_program, RENT_PROGRAM_ID);
    assert_eq!(input.payer, payer.pubkey());
    assert_eq!(input.makers, [makers[0].pubkey(), makers[1].pubkey()]);
    let fixture = build_direct_registered_creation_chain_fixture_v4(input)
        .expect("canonical registered Sell->Buy creation chain fixture");
    // The fixture hands the Rent program over as externally installed; if it
    // ever stopped doing so, the installer below would overwrite the real
    // deployment staged above with the fixture's own record and this campaign
    // would quietly go back to measuring a placeholder.
    assert!(
        fixture.externally_installed_keys.contains(&RENT_PROGRAM_ID),
        "the registered fixture must defer the Rent deployment to this harness",
    );
    let installed = install_direct_hot_chain_accounts_v5(
        test,
        &Rent::default(),
        &fixture.accounts,
        &fixture.externally_installed_keys,
    )
    .expect("install registered-creation chain accounts");
    assert_eq!(
        installed.rollback_snapshot_keys,
        fixture.rollback_snapshot_keys
    );
    CreationCase {
        fixture,
        payer,
        makers,
    }
}

/// The four instructions one side's creation is submitted as.
///
/// Identical in shape to `waist::direct_top_level_instructions` and different in
/// exactly one place: a registered creation carries ONE maker signature, not
/// two (`native_signature_count_v3(RegisterSell|RegisterBuy, _) == 1`), so the
/// two-participant fixed encoder cannot express it and the `many` encoder is
/// used with a one-element slice.
///
/// The Hot instruction sits at index 3 for the same two forced reasons the
/// ordinary top-level route documents: it cannot go last, because the runtime
/// clears return data at the start of every top-level instruction and a
/// trailing ComputeBudget instruction would erase the commit-last ACK; and it
/// cannot move forward silently, because the native evidence names the index it
/// expects to find the signed Hot bytes at.
fn side_instructions(case: &CreationCase, side: usize) -> Vec<Instruction> {
    let sell = side == 0;
    assert!(
        sell || side == 1,
        "registered creation has exactly two sides"
    );
    let (hot, action, signer) = if sell {
        (
            &case.fixture.sell_hot_instruction,
            DirectExecutionActionV3::RegisterSell,
            &case.makers[0],
        )
    } else {
        (
            &case.fixture.buy_hot_instruction,
            DirectExecutionActionV3::RegisterBuy,
            &case.makers[1],
        )
    };
    signed_instructions(hot, action, signer)
}

/// Assemble the four instructions over arbitrary Hot bytes and an arbitrary
/// signer, so a hostile can move exactly one of the two and nothing else.
fn signed_instructions(
    hot: &Instruction,
    action: DirectExecutionActionV3,
    signer: &Keypair,
) -> Vec<Instruction> {
    let width = direct_native_evidence_bytes_v3(action, u32::MAX)
        .expect("registered native evidence width");
    let message = signed_message(hot, action);
    let signature: [u8; 64] = signer
        .sign_message(&message)
        .as_ref()
        .try_into()
        .expect("maker signature width");
    let mut scratch = vec![0_u8; width];
    let mut evidence = vec![0_u8; width];
    encode_direct_native_evidence_many_v3_atomic(
        DirectNativeEvidenceContainerV3::TradingHot,
        3,
        &hot.data,
        u32::MAX,
        &[signature],
        &mut scratch,
        &mut evidence,
    )
    .expect("detached native evidence over the top-level registered Hot instruction");
    vec![
        Instruction {
            program_id: compute_budget::ID,
            accounts: Vec::new(),
            data: {
                let mut data = vec![2];
                data.extend_from_slice(
                    &u32::try_from(COMPUTE_LIMIT)
                        .expect("compute limit width")
                        .to_le_bytes(),
                );
                data
            },
        },
        Instruction {
            program_id: compute_budget::ID,
            accounts: Vec::new(),
            data: {
                let mut data = vec![1];
                data.extend_from_slice(&DIRECT_HOT_HEAP_FRAME_BYTES_V1.to_le_bytes());
                data
            },
        },
        Instruction {
            program_id: ed25519_program::ID,
            accounts: Vec::new(),
            data: evidence,
        },
        hot.clone(),
    ]
}

/// The exact signed preimage, located by the request codec rather than by hand.
///
/// Taken from the instruction rather than from `fixture.signed_messages` so a
/// hostile that mutates the request bytes automatically moves the message the
/// maker signs with it; a case that wants a signature over the ORIGINAL bytes
/// says so by passing the original instruction. The offset and width come from
/// `native_signature_slice_v3`, the same semantic owner the on-chain evidence
/// check reads them from -- a literal 64 here would be a second authority for
/// a coordinate that already has one.
fn signed_message(hot: &Instruction, action: DirectExecutionActionV3) -> Vec<u8> {
    let request = hot
        .data
        .get(HOT_EXECUTION_ENVELOPE_BYTES_V3..)
        .expect("registered Hot request tail");
    assert_eq!(request.len(), DIRECT_REGISTRATION_REQUEST_BYTES_V3);
    let slice = native_signature_slice_v3(action, u32::MAX, 0).expect("signed-preimage coordinate");
    let offset = usize::try_from(slice.message_offset).expect("preimage offset");
    let bytes = usize::from(slice.message_bytes);
    request
        .get(offset..offset + bytes)
        .expect("signed preimage span")
        .to_vec()
}

/// Unwrap a submission, printing the program log when it refused.
///
/// A refusal on this route is a custom code and nothing else in the panic
/// message, and a bare `0x4003` names neither the conjunct nor the depth. The
/// log is the only thing that distinguishes a refusal raised in the Registry
/// outer from one raised after the whole Direct action was paid for.
fn expect_execution(
    outcome: Result<SuccessfulExecution, RefusedExecution>,
    what: &str,
) -> SuccessfulExecution {
    if let Err(refusal) = &outcome {
        for line in &refusal.logs {
            println!("LOG {line}");
        }
        println!(
            "{what} refused: code={:?} after {} CU",
            refusal_code(&refusal.error),
            refusal.compute_units_consumed,
        );
    }
    outcome.expect(what)
}

async fn account(context: &mut ProgramTestContext, key: Pubkey) -> Account {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account read")
        .expect("live account")
}

async fn maybe_account(context: &mut ProgramTestContext, key: Pubkey) -> Option<Account> {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account read")
}

async fn snapshots(
    context: &mut ProgramTestContext,
    keys: &[Pubkey],
) -> Vec<(Pubkey, Option<Account>)> {
    let mut output = Vec::with_capacity(keys.len());
    for key in keys {
        output.push((*key, maybe_account(context, *key).await));
    }
    output
}

async fn token_amount(context: &mut ProgramTestContext, key: Pubkey) -> u64 {
    let account = account(context, key).await;
    TokenAccount::parse(&account.data)
        .expect("canonical token account")
        .amount
}

/// Start the bank with the whole campaign installed.
async fn started() -> (ProgramTestContext, CreationCase) {
    let artifacts = elves();
    let mut test = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let case = creation_case(&mut test, releases, &artifacts);
    let sell = side_instructions(&case, 0);
    let buy = side_instructions(&case, 1);
    let mut both = sell;
    both.extend(buy);
    let addresses = canonical_lookup_addresses(&both, Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let context = start_with_substrate(test, fixture_substrate()).await;
    (context, case)
}

/// The address table both transactions resolve against.
///
/// Rebuilt identically to the one installed at genesis, because
/// `submit_v0_observed` needs the address list to compile the message and the
/// harness does not hand it back.
fn table(case: &CreationCase) -> Vec<Pubkey> {
    let mut both = side_instructions(case, 0);
    both.extend(side_instructions(case, 1));
    canonical_lookup_addresses(&both, Pubkey::default())
}

// ---------------------------------------------------------------------------
// The campaign
// ---------------------------------------------------------------------------

/// A registered Sell and a registered Buy execute, in that order, on real ELFs.
///
/// # This is the acceptance gate, and it is ignored because it is BLOCKED
///
/// Not skipped, not weakened: written in full, run against a probe build, and
/// held one attribute away from being live. Behind wall A (the Direct action
/// gate, relaxed to `Ok(None)` in a throwaway build) every assertion down to
/// the Sell's Claims-conservation checks passes on real Core/Claims/Custody/
/// Registry/Rent ELFs. The Buy now creates its maker replay and registered
/// record, opens a Custody replay and a TradingPrincipal vault, and moves the
/// maker's collateral into that vault -- three child CPIs, all successful --
/// before wall C stops it in the commit's lamport plan. See the file header.
///
/// Remove `#[ignore]` when the walls close and this file states whether the
/// campaign completes -- do not soften an assertion to get there.
#[tokio::test]
#[ignore = "blocked: hot_v3 admits only InlineOrdinary through the Direct \
            crosscheck (wall A -- a missing registered crosscheck, not a gate \
            to remove), and behind that probe the Buy refuses Commit 0x4005 \
            because commit_output_lamports_v3 writes the observed-vacant zero \
            over the rent the Custody child deposited into coordinate 20 \
            (wall C). See this file's header."]
async fn registered_sell_then_buy_execute_on_current_elves() {
    let (mut context, case) = started().await;
    let fixture = &case.fixture;
    let addresses = table(&case);

    let claims_before = account(&mut context, fixture.claims_market).await;
    let positions_before = [
        account(&mut context, fixture.claims_positions[0]).await,
        account(&mut context, fixture.claims_positions[1]).await,
    ];
    let credit_before = account(&mut context, fixture.lifecycle_rent_credit).await;
    let source_before = token_amount(&mut context, fixture.collateral_accounts[0]).await;
    let root_before = account(&mut context, fixture.root).await;

    // ---- RegisterSell -----------------------------------------------------
    let sell = expect_execution(
        submit_v0_observed(
            &mut context,
            &side_instructions(&case, 0),
            addresses.clone(),
            Some(&case.payer),
            &[],
        )
        .await,
        "registered Sell creation",
    );
    println!(
        "REGSELL compute units consumed: {}",
        sell.compute_units_consumed
    );
    assert!(sell.compute_units_consumed <= COMPUTE_LIMIT);

    let root_after_sell = account(&mut context, fixture.root).await;
    assert_eq!(
        root_after_sell.data, fixture.root_poststates[0],
        "root poststate after Sell",
    );
    let maker = account(&mut context, fixture.maker_replays[0]).await;
    assert_eq!(maker.owner, TRADING_PROGRAM_ID);
    assert_eq!(maker.data.len(), DIRECT_MAKER_REPLAY_BYTES_V1);
    assert_eq!(maker.data, fixture.maker_poststates[0]);
    let record = account(&mut context, fixture.registered_records[0]).await;
    assert_eq!(record.owner, TRADING_PROGRAM_ID);
    assert_eq!(record.data.len(), DIRECT_REGISTERED_RECORD_BYTES_V2);
    assert_eq!(record.data, fixture.record_poststates[0]);

    // The economic seam, observed rather than assumed: a Sell reserves claims
    // in its own record and moves NOTHING in Claims. The aggregate and both
    // Positions are byte-identical after a Sell that reserved
    // `fixture.reserved_claims`.
    assert_eq!(
        account(&mut context, fixture.claims_market).await.data,
        claims_before.data,
        "a registered Sell must not mutate the Claims aggregate",
    );
    assert_eq!(
        account(&mut context, fixture.claims_positions[0])
            .await
            .data,
        positions_before[0].data,
        "a registered Sell must not mutate the seller Claims Position",
    );

    // ---- RegisterBuy ------------------------------------------------------
    let buy = expect_execution(
        submit_v0_observed(
            &mut context,
            &side_instructions(&case, 1),
            addresses,
            Some(&case.payer),
            &[],
        )
        .await,
        "registered Buy creation",
    );
    println!(
        "REGBUY compute units consumed: {}",
        buy.compute_units_consumed
    );
    assert!(buy.compute_units_consumed <= COMPUTE_LIMIT);

    let root_after_buy = account(&mut context, fixture.root).await;
    assert_eq!(
        root_after_buy.data, fixture.root_poststates[1],
        "root poststate after Buy",
    );
    let maker = account(&mut context, fixture.maker_replays[1]).await;
    assert_eq!(maker.owner, TRADING_PROGRAM_ID);
    assert_eq!(maker.data, fixture.maker_poststates[1]);
    let record = account(&mut context, fixture.registered_records[1]).await;
    assert_eq!(record.owner, TRADING_PROGRAM_ID);
    assert_eq!(record.data, fixture.record_poststates[1]);

    // Custody: replay opened and advanced through three revisions --
    // InitializeReplay, OpenVault, and the terminal delegated deposit.
    let replay = account(&mut context, fixture.custody_replay).await;
    assert_eq!(replay.owner, CUSTODY_PROGRAM_ID);
    let replay = CustodyReplayV1::decode(&replay.data).expect("post-creation Custody replay");
    assert_eq!(
        replay.next_revision, 3,
        "the Buy's three Custody routes must leave the replay at revision three",
    );

    // The collateral actually moved into the vault, exactly once and exactly
    // the record's reservation.
    assert_eq!(
        token_amount(&mut context, fixture.custody_vault).await,
        fixture.reserved_collateral,
    );
    assert_eq!(
        token_amount(&mut context, fixture.collateral_accounts[0]).await,
        source_before - fixture.reserved_collateral,
    );

    // Claims is untouched by BOTH creations: no claim moves until a fill.
    assert_eq!(
        account(&mut context, fixture.claims_market).await.data,
        claims_before.data,
    );
    assert_eq!(
        account(&mut context, fixture.claims_positions[0])
            .await
            .data,
        positions_before[0].data,
    );
    assert_eq!(
        account(&mut context, fixture.claims_positions[1])
            .await
            .data,
        positions_before[1].data,
    );

    // The lifecycle RentCredit is the same credit both creations named, still
    // canonical and still bound to this Market. Its BODY may not move: the
    // credit records a beneficiary and a coordinate, not a running balance.
    let credit = account(&mut context, fixture.lifecycle_rent_credit).await;
    assert_eq!(credit.owner, RENT_PROGRAM_ID);
    assert_eq!(credit.data.len(), LIFECYCLE_RENT_CREDIT_BYTES_V2);
    assert_eq!(credit.data, credit_before.data);
    LifecycleRentCreditV2::decode(&credit.data).expect("canonical lifecycle RentCredit");
    println!(
        "REGRENT credit lamports before={} after={}",
        credit_before.lamports, credit.lamports
    );

    // Commit-last evidence for the Buy, produced by Trading itself.
    let (producer, returned) = buy
        .return_data
        .expect("a successful Hot execution must return commit-last evidence");
    assert_eq!(producer, TRADING_PROGRAM_ID, "ACK producer substitution");
    let ack = HotExecutionAckV3::decode(&returned).expect("canonical Hot ACK");
    let (envelope, family_request) =
        HotExecutionEnvelopeV3::split_instruction(&fixture.buy_hot_instruction.data)
            .expect("canonical registered Buy Hot instruction");
    assert_eq!(ack.release_set, envelope.release_set());
    assert_eq!(ack.market, envelope.market());
    assert_eq!(ack.root, fixture.root.to_bytes());
    assert_eq!(ack.request_digest, hash(family_request).to_bytes());
    assert_eq!(
        ack.root_prestate_digest,
        hash(&fixture.root_poststates[0]).to_bytes(),
    );
    assert_eq!(
        ack.root_poststate_digest,
        hash(&root_after_buy.data).to_bytes(),
    );
    assert_ne!(root_before.data, root_after_buy.data);
}

// ---------------------------------------------------------------------------
// The walls, by name and by depth
// ---------------------------------------------------------------------------

/// Submit one side and require a refusal carrying exactly `expected`.
///
/// Returns the compute the refusal cost, so a case can state its DEPTH by
/// comparison with another case on the same route rather than against a
/// threshold constant that drifts with every codegen change.
async fn refusal(
    context: &mut ProgramTestContext,
    case: &CreationCase,
    instructions: &[Instruction],
    expected: TradingSbfError,
    what: &str,
) -> u64 {
    let before = snapshots(context, &case.fixture.rollback_snapshot_keys).await;
    let outcome =
        submit_v0_observed(context, instructions, table(case), Some(&case.payer), &[]).await;
    assert!(
        outcome.is_err(),
        "{what} was expected to refuse and executed",
    );
    let refused = outcome.err().expect("refusal");
    assert_eq!(
        refusal_code(&refused.error),
        Some(expected as u32),
        "{what}: {:#?}",
        refused.logs,
    );
    // Rollback. A refusal that left one authenticated account moved would be a
    // partial write, and on this route the accounts at stake are the mutable
    // Direct root, the maker replays, the registered records, the lifecycle
    // RentCredit and every Custody/collateral coordinate the Buy touches.
    let after = snapshots(context, &case.fixture.rollback_snapshot_keys).await;
    assert_eq!(
        after, before,
        "{what} refused but did not roll back its material state",
    );
    assert!(
        !refused.invoked(CUSTODY_PROGRAM_ID) && !refused.invoked(CLAIMS_PROGRAM_ID),
        "{what} refused after reaching a child program: {:#?}",
        refused.logs,
    );
    refused.compute_units_consumed
}

/// The cheapest refusal on the route, and the control the others are read
/// against.
///
/// `DCLTHOT3` is on `declares_extended_heap_profile_v1`'s list, so the grant is
/// ADMISSIBLE rather than assumed; a caller who omits it gets the protocol
/// default ceiling and `require_extended_heap_admitted_v1` refuses BY NAME in
/// the prologue rather than allocating until an out-of-memory abort that names
/// nothing. This is that caller.
#[tokio::test]
async fn a_registered_creation_without_its_heap_grant_refuses_as_heap_frame() {
    let (mut context, case) = started().await;
    // Everything except the RequestHeapFrame instruction, and nothing else
    // moves: the native evidence names index 3, so the Hot instruction must
    // STAY at index 3 rather than sliding up when the grant is dropped. A
    // second SetComputeUnitLimit cannot hold the slot -- the runtime rejects a
    // duplicate ComputeBudget instruction before any program runs, and the
    // transaction then fails with no custom code at all, which is a harness
    // artefact and not this refusal. `SetComputeUnitPrice(0)` is the inert
    // stand-in: a distinct ComputeBudget instruction that changes nothing.
    let mut without = side_instructions(&case, 0);
    without
        .get_mut(1)
        .expect("heap grant slot")
        .data
        .clone_from(&{
            let mut data = vec![3];
            data.extend_from_slice(&0_u64.to_le_bytes());
            data
        });
    let units = refusal(
        &mut context,
        &case,
        &without,
        TradingSbfError::HeapFrame,
        "a registered Sell with no heap grant",
    )
    .await;
    println!("REGWALL heap-frame refusal cost: {units} CU");
}

/// A release-set substitution refuses, rolls back, and refuses EARLY.
///
/// The substituted set is not the one the activation cache authenticates and
/// not the one the Core Market names, so the market authentication in the
/// prologue cannot admit it. The maker's signature is untouched and still
/// valid: it covers the family request, which begins at a fixed offset past
/// the envelope this rebuilds, so this moves the release identity and nothing
/// else -- which is what makes it a substitution rather than a corruption.
#[tokio::test]
async fn a_release_substitution_refuses_before_the_direct_action_wall() {
    let (mut context, case) = started().await;
    let honest = side_instructions(&case, 0);
    let hot = honest.get(3).expect("Hot instruction").clone();
    let (envelope, request) =
        HotExecutionEnvelopeV3::split_instruction(&hot.data).expect("canonical Hot instruction");
    let substituted = HotExecutionEnvelopeV3::new(
        envelope.request_bytes(),
        [0x5b; 32],
        envelope.market(),
        envelope.generation(),
        envelope.root_prestate_digest(),
    )
    .expect("substituted envelope")
    .with_bump_hints(envelope.bump_hints());
    assert_ne!(substituted.release_set(), envelope.release_set());
    let mut data = substituted.to_bytes().to_vec();
    data.extend_from_slice(request);
    let mut instructions = honest;
    instructions
        .get_mut(3)
        .expect("Hot instruction")
        .data
        .clone_from(&data);
    let units = refusal(
        &mut context,
        &case,
        &instructions,
        TradingSbfError::Content,
        "a registered Sell on a substituted release set",
    )
    .await;
    println!("REGWALL release-substitution refusal cost: {units} CU");
}

/// WALL A, measured: the registered Sell authenticates completely and is then
/// refused for its ACTION.
///
/// `TradingSbfError::UnsupportedContent` is raised in exactly one place a Direct
/// Hot execution can reach this late --
/// `prepare_direct_inline_hot_crosscheck_v3`'s second statement, quoted in this
/// file's header. The two `UnsupportedContent` sites near it in `hot_v3` belong
/// to `authenticate_strategy_for_accelerator_boxed_v4` and
/// `authenticate_strategy_from_sealed_boxed_v3`, which run hundreds of thousands
/// of compute units earlier, before the artifact band closes.
///
/// The depth is the point. A `hot-cu-profile` build of this exact transaction
/// puts the refusal 451 CU after the `preflight-children` checkpoint -- past
/// the manifest, the program set, the validated-artifact seal, the descriptor,
/// the config, the lifecycle policy, the account profile, the request profile,
/// the transition, the effect projection, the lifecycle preplan, the candidate,
/// the replan and the child preflight. Everything about this registered Sell is
/// correct and admitted; only its action is not on the crosscheck's list.
#[tokio::test]
async fn registered_sell_creation_refuses_at_the_direct_action_wall() {
    let (mut context, case) = started().await;
    let instructions = side_instructions(&case, 0);
    let units = refusal(
        &mut context,
        &case,
        &instructions,
        TradingSbfError::UnsupportedContent,
        "wall A: registered Sell at the Direct crosscheck",
    )
    .await;
    println!("REGWALL sell action-wall refusal cost: {units} CU");
}

/// The chain is ordered, and the root prestate commitment is what orders it.
///
/// The Buy's envelope commits to `hash(root_after_sell)`, so submitting the Buy
/// on a bank where no Sell has run refuses `TradingSbfError::Root` -- an
/// optimistic-prestate mismatch -- long before anything about the Buy's own
/// content is read. That is the property that makes these two transactions one
/// campaign rather than two independent creations, and it is worth pinning: a
/// registered Buy that could be admitted against a stale root would be a Buy
/// whose maker-root accounting nobody had checked.
///
/// It is also why every wall past it is measured behind a probe build with wall
/// A relaxed: reaching the artifact band on the Buy needs a committed Sell
/// poststate, and wall A is what stops the Sell from committing. Wall B has
/// since been crossed; see this file's header for wall C, which is where the
/// Buy stops today.
#[tokio::test]
async fn a_registered_buy_refuses_root_when_its_sell_has_not_committed() {
    let (mut context, case) = started().await;
    let instructions = side_instructions(&case, 1);
    let units = refusal(
        &mut context,
        &case,
        &instructions,
        TradingSbfError::Root,
        "a registered Buy rooted on a Sell that never ran",
    )
    .await;
    println!("REGWALL buy stale-root refusal cost: {units} CU");
}

/// Wall B is structural, and this is the half of it that needs no chain.
///
/// `CapabilityProgramV4::validate_selection` requires `self.derivation_policy
/// == entry.child_derivation_id()`, and `derivation_policy` is the digest of
/// the descriptor's own `LifecyclePolicyV5` record -- `hot_v3` additionally
/// requires `descriptor.derivation_policy() == descriptor.lifecycle().program()`
/// before it reads that record. So two actions can share one manifest entry
/// only if their lifecycle policies are byte-identical, and a Buy's is not even
/// the same WIDTH as a Sell's: it carries two more
/// `LifecycleCurrentRentQuoteInputV5` rows, for the Custody replay and vault a
/// Buy opens and a Sell has no business quoting.
///
/// Different widths, different digests, different `derivation_policy`.
///
/// RESOLVED, and by neither of the two routes this comment used to offer. It
/// said the wall stood "until either the entry stops pinning the lifecycle for
/// multi-action program sets, or a root can select an entry per action" -- one
/// weakening a capability-contract gate, the other changing the persisted root
/// header. There was a third way and it touches neither: give the QUOTES an
/// action, so one policy carries both sides.
///
/// `c8396b0b` added `LifecycleCurrentRentQuoteV5`'s action tag inside bytes that
/// were already canonical zeros -- no width moved and no pinned digest moved --
/// and `registered_state_artifacts_v4::
/// encode_direct_registered_creation_unified_lifecycle_v5_atomic` emits the
/// policy that uses it. `one_policy_serves_both_registered_creation_actions`
/// decodes that policy and asks it, per action, for exactly what each side used
/// to get from a policy of its own.
///
/// This case stays as the record of WHY the wall existed: the two per-action
/// policies still have different widths, and that is still what made a shared
/// entry impossible for as long as a root had to choose one of them.
#[test]
fn the_two_registered_creation_lifecycle_policies_cannot_share_a_manifest_entry() {
    assert_ne!(
        DIRECT_REGISTER_SELL_LIFECYCLE_BYTES_V5, DIRECT_REGISTER_BUY_LIFECYCLE_BYTES_V5,
        "a Sell and a Buy would share a lifecycle policy, and wall B would not exist",
    );
    assert!(DIRECT_REGISTER_BUY_LIFECYCLE_BYTES_V5 > DIRECT_REGISTER_SELL_LIFECYCLE_BYTES_V5);
    // And the policy that ends it is wider than either, because it carries both
    // sides' plans and bindings and the union of their quotes.
    assert!(
        DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5 > DIRECT_REGISTER_BUY_LIFECYCLE_BYTES_V5,
        "the unified policy must carry more than the larger side, or it is not the union",
    );
}
