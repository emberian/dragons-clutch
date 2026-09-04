//! Current-source positive pre-Market Series Expire campaign through the real
//! Registry, Trading, Core, Custody, and Claims ELFs.
//!
//! Fixture construction lives here; the release compiler remains owned by
//! `dclutch_trading_sbf::series::release_v5`, and the physical evidence joins
//! remain owned by the adjacent support module.
//!
//! # THREE ROWS ARE STILL RED, AND THE CAMPAIGN NOW SUBMITS A TRANSACTION
//!
//! It reaches the bank. The Registry continuation lands, Trading is invoked at
//! depth two, and it refuses inside its ORDINARY Hot prelude, before any
//! Series semantics run -- `authenticate_market`'s first conjunct, because
//! this fixture stages the fixed controller Market VACANT. That is the current
//! wall and it is named at the end of this list.
//!
//! Two walls came out to get there, and BOTH of them were the fixture reading
//! an authority wrong rather than an account it had failed to build:
//!
//! - `validate_physical_bindings` compared the instruction's runtime TAIL
//!   against the release's full physical count. `series_hot_v3.rs` assembles a
//!   Hot instruction as 39 fixed accounts, the action's strategy extras, then
//!   `runtime_physical_accounts.skip(HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3)` --
//!   physical ordinals 0..5 ARE five members of the fixed prefix and are never
//!   repeated. So `runtime=39` against `geometry.physical=44` was not five
//!   accounts missing from the fixture; all forty-four are built, installed and
//!   bound at coordinates 72..76 and have been since `1b8191f9d`. It was five
//!   the reader declined to count, and an ordinal space shifted under every
//!   role lookup by exactly the same five.
//! - The top-level wrapper was `legacy_registry_hot_instruction`, the headered
//!   `continuation_v1` container. Trading refuses that seam ON PURPOSE:
//!   `authenticate_hot_invocation_v3` requires the instructions-sysvar record
//!   of the top-level instruction to carry the same bytes Trading received, and
//!   the legacy seam forwards only the stripped continuation, so the header is
//!   observable at the child. `registry_hot_continuation.rs` asserts exactly
//!   that outcome, with exactly that code, in
//!   `a_legacy_headered_hot_container_takes_the_v1_seam_and_not_the_transparent_one`.
//!   The transparent `hot_continuation_v2` seam is the one a Series Expire
//!   takes, and taking it moved the refusal from `NativeSignature` to a Trading
//!   prelude conjunct 29,000 CU further in.
//!
//! The Series Expire ARTIFACT SET no longer contradicts itself:
//! `97ce7a748` keyed the family's proof geometry on the Template that owns it,
//! so the Expire RequestProfile pins `series_action_request_bytes_v3(count)`
//! and route 4 declares its borrowed range only when the canonical proof is
//! nonempty. This campaign stages a ONE-occurrence Template, `proof_height(1)`
//! is zero, and both halves now agree on 128 bytes with no range at all.
//!
//! Running it against real ELFs built from that commit walked the refusal
//! through three walls in one afternoon, and each one is worth having named:
//!
//! 1. `BuilderError::Projection("borrowed-range-resolve")` -- GONE. This was
//!    the artifact contradiction and it was the only one of the three that was
//!    program code.
//! 2. The operator and the bundle builder disagreed in exactly ONE byte of a
//!    256-byte instruction: envelope offset 127, the eighth bump hint, the
//!    Custody transfer authority. Both envelopes are VALID -- an absent hint
//!    means the route searches rather than refuses -- so no program would ever
//!    have reported it, and only the fixture's exact cross-check could. The
//!    cause was that this fixture handed the operator a ZERO-LENGTH activation
//!    cache at Hot coordinate 22 while the bank held the real one:
//!    `activated_custody_program_v1` read nothing out of it and the operator
//!    honestly mined an absent hint. Repaired here by giving the operator the
//!    account the bank actually holds (`Releases::activation_data`).
//! 3. Where it stops today, measured on chain and localized with the tree's own
//!    `hot-cu-profile` checkpoints: Trading consumes 53,491 CU and refuses
//!    `Content` (`0x4003`) inside `authenticate_market`, ~230 CU past its
//!    entry, which is its first conjunct --
//!    `frame.market.owner != frame.core_program.key || data_len() != STATE_BYTES`.
//!    `build_controller_corpus_v1` builds the controller as
//!    `market: vacant(controller_market)`, and `process_hot_execution_v3` says
//!    in its own comment why that cannot stand: "the fixed Market is always the
//!    live Series controller ... the occurrence's distinct future Market is a
//!    route-local account, never a substitute for the fixed controller
//!    coordinate." Pre-Market names the FUTURE Market, which is correctly
//!    vacant; the controller is a live founded Market and this fixture has
//!    never staged one. The next unit is to build it the way a founding writes
//!    it -- `direct-hot/src/fixture.rs::market_and_claims` is the worked
//!    example, bumps and all -- at its canonical
//!    `market_core_state_address_v2`, which cascades into the root header, the
//!    Ticket state PDA, and the permit and caller seeds that derive from it.
//!    A second fixture gap is queued behind it and is separate: the
//!    below-minimum row needs a `RentCredit` prestate in the bank, which
//!    `capture_series_account_snapshots_v1` cannot find today.
//!
//! # WHAT THE ARTIFACT REPAIR ACTUALLY WAS, kept because it is not obvious
//!
//! The Series kernel had already decided it. `series_proof_count_v3` (formerly
//! the private `proof_height`) is compared by EQUALITY in
//! `admit_occurrence_bytes`, not as a floor, and it is a function of immutable
//! Template config alone. So `128 + 32 * count` is a per-Template CONSTANT
//! knowable before any request exists, and the artifacts had been written as
//! if the proof width were a runtime variable. It never was.
//!
//! Both spellings of "a borrowed thing is here" are canonically nonempty --
//! `BorrowedRangeV4::resolve` refuses a zero length and
//! `BorrowedWitnessPolicyV3::validate` refuses a zero minimum -- so a Template
//! whose canonical proof is empty declares NO range rather than one that
//! resolves to zero. Coverage still closes on its own:
//! `validate_request_coverage` starts its cursor at the 128-byte semantic
//! prefix and requires it to reach the request's exact end.
//!
//! The repair had four authors and this file was none of them: the artifact,
//! `hot_v3.rs`'s replay-overlap conjunct (which required `ranges.count() == 1`
//! and would have gone silent), `effect_v4.rs`'s per-route `borrowed_range_count()`
//! pin, and the shadow generator's source manifest. `consume_artifacts_v4` was
//! HALF in the same position: its Effect shared the defect, but its
//! RequestProfile did not -- `authenticate_series_consume_artifacts_v4` splits
//! the proof off itself and REQUIRES a 128-byte profile, where Expire is
//! authenticated by the generic Hot path against the complete request.

#[path = "support/series_premarket_expiry_chain_v1.rs"]
mod series_premarket_expiry_chain_v1;
#[path = "support/series_premarket_expiry_v1.rs"]
mod series_premarket_expiry_v1;

use dclutch_market_core_codec::SERIES_FOUNDING_PERMIT_BYTES_V1;
use dclutch_trading_sbf::TradingSbfError;
use series_premarket_expiry_chain_v1::{
    SeriesPremarketExpiryChainFixtureV1, SeriesPremarketExpiryChainInputV1,
    build_series_premarket_expiry_chain_v1,
};
use series_premarket_expiry_v1::{
    SeriesExpiryReplayExpectationV1, SeriesPremarketExpiryPhysicalInputV1,
    SeriesPremarketExpiryPhysicalReportV1, assert_series_premarket_expiry_rollback_v1,
    assert_series_premarket_expiry_success_v1,
    authenticate_series_premarket_expiry_physical_report_v1, capture_series_account_snapshots_v1,
    install_series_premarket_expiry_accounts_v1,
};
use solana_program::{instruction::InstructionError, pubkey::Pubkey, rent::Rent};
use solana_program_test::BanksClientError;
use solana_sdk::transaction::TransactionError;

use dclutch_direct_hot_program_test_support::waist::{
    CLAIMS_PROGRAM_ID, CORE_PROGRAM_ID, CUSTODY_PROGRAM_ID, REGISTRY_PROGRAM_ID, RENT_PROGRAM_ID,
    TRADING_PROGRAM_ID, add_lookup_table, add_release_waist, canonical_lookup_addresses, elves,
    fixture_substrate, program_test_without_forced_budget, start_with_substrate,
    submit_v0_observed,
};

fn build_chain(
    test: &mut solana_program_test::ProgramTest,
    artifacts: &dclutch_direct_hot_program_test_support::waist::Elves,
) -> SeriesPremarketExpiryChainFixtureV1 {
    let releases = add_release_waist(test, artifacts);
    build_series_premarket_expiry_chain_v1(SeriesPremarketExpiryChainInputV1 {
        releases,
        elves: artifacts,
        rent: Rent::default(),
        registry_program: REGISTRY_PROGRAM_ID,
        trading_program: TRADING_PROGRAM_ID,
        core_program: CORE_PROGRAM_ID,
        claims_program: CLAIMS_PROGRAM_ID,
        custody_program: CUSTODY_PROGRAM_ID,
        rent_program: RENT_PROGRAM_ID,
    })
    .expect("canonical current-source Series Expire chain")
}

fn physical_report(
    fixture: &SeriesPremarketExpiryChainFixtureV1,
) -> SeriesPremarketExpiryPhysicalReportV1 {
    let report = authenticate_series_premarket_expiry_physical_report_v1(
        &fixture.selected,
        SeriesPremarketExpiryPhysicalInputV1 {
            registry_program: REGISTRY_PROGRAM_ID,
            trading_program: TRADING_PROGRAM_ID,
            core_program: CORE_PROGRAM_ID,
            parent_root: fixture.parent_root,
            parent_root_prestate: fixture.parent_root_prestate.clone(),
            ticket_state: fixture.ticket_state,
            permit_account: fixture.permit_account,
            rent: Rent::default(),
            precommit_caller: fixture.precommit_caller,
            hot_instruction: fixture.hot_instruction.clone(),
            top_level_instruction: fixture.top_level_instruction.clone(),
            replay: SeriesExpiryReplayExpectationV1 {
                root_before: fixture.parent_root_prestate.clone(),
                root_after: fixture.root_poststate.clone(),
                ticket_before: fixture.ticket_prestate.clone(),
                ticket_after: fixture.ticket_poststate.clone(),
            },
            success_transitions: fixture.success_transitions.clone(),
            rollback_snapshot_keys: fixture.material_snapshot_keys.clone(),
        },
    )
    .expect("selected release/physical Expire authority join");
    let operator = &fixture.operator_report;
    assert_eq!(operator.selected, fixture.selected);
    assert_eq!(operator.instruction, report.hot_instruction);
    assert_eq!(operator.trading_program, TRADING_PROGRAM_ID);
    assert_eq!(operator.parent_market, report.parent_market);
    assert_eq!(operator.parent_generation, report.parent_generation);
    assert_eq!(operator.release_set, report.release_set);
    assert_eq!(operator.roles.root, report.parent_root);
    assert_eq!(operator.roles.ticket, Some(report.ticket_state));
    assert_eq!(operator.roles.rent_credit, Some(report.rent_credit));
    assert_eq!(operator.roles.occurrence_market, Some(report.future_market));
    assert_eq!(
        operator.roles.occurrence_generation,
        Some(report.future_generation),
    );
    assert_eq!(operator.roles.permit, Some(report.permit_account));
    assert_eq!(operator.roles.payer, None);
    assert_eq!(operator.roles.refund, None);
    assert_eq!(operator.roles.system_program, None);
    report
}

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

#[derive(Clone, Copy, Debug)]
enum PrecommitCallerHostileV1 {
    Substitution,
    Writable,
    ForeignOwner,
    NonemptyBody,
}

fn apply_precommit_caller_hostile_v1(
    fixture: &mut SeriesPremarketExpiryChainFixtureV1,
    hostile: PrecommitCallerHostileV1,
) -> TradingSbfError {
    match hostile {
        PrecommitCallerHostileV1::Substitution => {
            let wrong_caller = Pubkey::new_unique();
            let mut wrong_account = fixture
                .install_accounts
                .iter()
                .find(|candidate| candidate.key == fixture.precommit_caller)
                .expect("precommit caller install account")
                .clone();
            wrong_account.key = wrong_caller;
            wrong_account.snapshot_for_rollback = false;
            fixture.install_accounts.push(wrong_account);
            let mut replacements = 0;
            for meta in &mut fixture.top_level_instruction.accounts {
                if meta.pubkey == fixture.precommit_caller {
                    meta.pubkey = wrong_caller;
                    replacements += 1;
                }
            }
            assert_eq!(replacements, 1, "one physical coord80 caller");
            TradingSbfError::Release
        }
        PrecommitCallerHostileV1::Writable => {
            let mut matches = 0;
            for meta in &mut fixture.top_level_instruction.accounts {
                if meta.pubkey == fixture.precommit_caller {
                    meta.is_writable = true;
                    matches += 1;
                }
            }
            assert_eq!(matches, 1, "one physical coord80 caller");
            TradingSbfError::Content
        }
        PrecommitCallerHostileV1::ForeignOwner => {
            let caller = fixture
                .install_accounts
                .iter_mut()
                .find(|candidate| candidate.key == fixture.precommit_caller)
                .expect("precommit caller install account");
            caller.account.owner = CORE_PROGRAM_ID;
            TradingSbfError::Content
        }
        PrecommitCallerHostileV1::NonemptyBody => {
            let caller = fixture
                .install_accounts
                .iter_mut()
                .find(|candidate| candidate.key == fixture.precommit_caller)
                .expect("precommit caller install account");
            caller.account.data = vec![0x80];
            caller.account.lamports = Rent::default().minimum_balance(caller.account.data.len());
            TradingSbfError::Content
        }
    }
}

/// One exact canonical release, one Registry continuation, and five real child
/// routes carry a prepared Series occurrence through Expire while its future
/// Market remains vacant. The only Trading writes are the kernel replay
/// replacements; Core drains the still-unallocated permit under its exact PDA.
#[tokio::test]
async fn current_source_series_expire_lands_before_the_future_market_exists() {
    let artifacts = elves();
    let mut test = program_test_without_forced_budget(&artifacts);
    let fixture = build_chain(&mut test, &artifacts);
    let report = physical_report(&fixture);
    assert_eq!(
        report.runtime_physical_accounts, fixture.runtime_physical_accounts,
        "operator and physical-report packing must be identical",
    );
    install_series_premarket_expiry_accounts_v1(
        &mut test,
        &Rent::default(),
        &fixture.install_accounts,
        &fixture.externally_installed,
    )
    .expect("install Series chain accounts");
    let instructions = [fixture.top_level_instruction.clone()];
    let addresses =
        canonical_lookup_addresses(&instructions, solana_program::pubkey::Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;
    context
        .warp_to_slot(2)
        .expect("warp beyond the Series retry deadline");
    let before = capture_series_account_snapshots_v1(&mut context, &fixture.material_snapshot_keys)
        .await
        .expect("success prestates");
    let execution = submit_v0_observed(&mut context, &instructions, addresses, None, &[])
        .await
        .expect("real-ELF Series Expire");
    let after = capture_series_account_snapshots_v1(&mut context, &fixture.material_snapshot_keys)
        .await
        .expect("success poststates");
    assert_series_premarket_expiry_success_v1(&report, &before, &after)
        .expect("exact landed Series Expire poststates");

    for program in [
        REGISTRY_PROGRAM_ID,
        TRADING_PROGRAM_ID,
        CUSTODY_PROGRAM_ID,
        CORE_PROGRAM_ID,
    ] {
        assert!(
            execution
                .logs
                .iter()
                .any(|line| line.starts_with(&format!("Program {program} invoke ["))),
            "successful campaign did not reach real program {program}: {:#?}",
            execution.logs,
        );
    }
    assert!(execution.compute_units_consumed <= 1_400_000);
}

/// A PERMIT PREPAID AT A CHEAPER RATE STILL EXPIRES, ON THE DEPLOYED ELF.
///
/// This route REFUNDS a permit Core never allocated: the founding prepaid the
/// slot in an earlier transaction, at that transaction's rate, and Expire hands
/// the lamports back to the RentCredit. Until the ruling of 2026-09-04 05:50
/// the pre-Market vacancy conjunct floored the slot at
/// `Rent::minimum_balance(SERIES_FOUNDING_PERMIT_BYTES_V1)` of the moment, and
/// this test asserted that one lamport below it refused with `Content`. That is
/// backwards: the cluster may raise its rate at any epoch boundary, and a slot
/// nobody owns can never be topped up, so the floor stranded the prepayment
/// permanently. The seeds say WHICH slot this is; `funded_rent_persists_v1`
/// says whether there is anything left in it.
///
/// Same fixture, same real ELFs, one lamport short -- and it lands.
///
/// IT IS ALSO ONE OF THE THREE RED ROWS THIS FILE'S HEADER DESCRIBES: `build_chain`
/// refuses `Projection("borrowed-range-resolve")` before a single ELF executes,
/// for the artifact-set contradiction named above, which predates this test and
/// is not rent. So the inversion here is proved by
/// `series_expiry_permit_requires_exact_prefunded_writable_system_vacancy` on the
/// native side and is OWED a parent-ELF red-proof until the Expire artifact set
/// is repaired. Measured at `edfdc22ac`, before this lane began: the same three
/// rows fail identically.
#[tokio::test]
async fn a_permit_prepaid_below_todays_minimum_still_expires_on_the_deployed_elf() {
    let artifacts = elves();
    let mut test = program_test_without_forced_budget(&artifacts);
    let mut fixture = build_chain(&mut test, &artifacts);
    let report = physical_report(&fixture);
    let permit = fixture
        .install_accounts
        .iter_mut()
        .find(|candidate| candidate.key == fixture.permit_account)
        .expect("permit install account");
    permit.account.lamports = permit
        .account
        .lamports
        .checked_sub(1)
        .expect("positive permit prefund");
    install_series_premarket_expiry_accounts_v1(
        &mut test,
        &Rent::default(),
        &fixture.install_accounts,
        &fixture.externally_installed,
    )
    .expect("install hostile Series chain accounts");
    let instructions = [fixture.top_level_instruction.clone()];
    let addresses =
        canonical_lookup_addresses(&instructions, solana_program::pubkey::Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;
    context
        .warp_to_slot(2)
        .expect("warp beyond the Series retry deadline");
    let before = capture_series_account_snapshots_v1(&mut context, &fixture.material_snapshot_keys)
        .await
        .expect("stranded-permit prestates");
    let permit_before = before
        .accounts
        .iter()
        .find(|snapshot| snapshot.key == report.permit_account)
        .and_then(|snapshot| snapshot.account.clone())
        .expect("the stranded permit must exist before the expiry");
    let credit_before = before
        .accounts
        .iter()
        .find(|snapshot| snapshot.key == report.rent_credit)
        .and_then(|snapshot| snapshot.account.clone())
        .expect("RentCredit prestate");
    // THE POSITIVE CONTROL: the perturbation really did put this slot under
    // what the bank charges today, or the admission below proves nothing.
    assert_eq!(
        permit_before.lamports,
        Rent::default()
            .minimum_balance(SERIES_FOUNDING_PERMIT_BYTES_V1)
            .checked_sub(1)
            .expect("a positive permit minimum"),
        "the fixture must be exactly one lamport under today's floor"
    );
    submit_v0_observed(&mut context, &instructions, addresses, None, &[])
        .await
        .expect("real-ELF Series Expire over a permit the cluster repriced");
    let after = capture_series_account_snapshots_v1(&mut context, &fixture.material_snapshot_keys)
        .await
        .expect("stranded-permit poststates");
    let permit_after = after
        .accounts
        .iter()
        .find(|snapshot| snapshot.key == report.permit_account)
        .expect("permit poststate row")
        .account
        .clone();
    assert!(
        permit_after.is_none_or(|account| account.lamports == 0 && account.data.is_empty()),
        "the expiry must drain the slot it refunded"
    );
    let credit_after = after
        .accounts
        .iter()
        .find(|snapshot| snapshot.key == report.rent_credit)
        .and_then(|snapshot| snapshot.account.clone())
        .expect("RentCredit poststate");
    assert!(
        credit_after.lamports > credit_before.lamports,
        "the stranded prepayment must reach the RentCredit, not stay stranded"
    );
}

/// Coord80 is an exact controller-scoped Trading PDA and only the inner Core
/// CPI may synthesize its signer privilege. The former funded-crank topology,
/// a substituted key, and non-vacant account bodies all refuse atomically.
#[tokio::test]
async fn precommit_caller_substitutions_refuse_with_exact_state_reversion() {
    for hostile in [
        PrecommitCallerHostileV1::Substitution,
        PrecommitCallerHostileV1::Writable,
        PrecommitCallerHostileV1::ForeignOwner,
        PrecommitCallerHostileV1::NonemptyBody,
    ] {
        let artifacts = elves();
        let mut test = program_test_without_forced_budget(&artifacts);
        let mut fixture = build_chain(&mut test, &artifacts);
        let report = physical_report(&fixture);
        let expected = apply_precommit_caller_hostile_v1(&mut fixture, hostile);
        install_series_premarket_expiry_accounts_v1(
            &mut test,
            &Rent::default(),
            &fixture.install_accounts,
            &fixture.externally_installed,
        )
        .expect("install hostile caller chain accounts");
        let instructions = [fixture.top_level_instruction.clone()];
        let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
        add_lookup_table(&mut test, &addresses);
        let mut context = start_with_substrate(test, fixture_substrate()).await;
        context
            .warp_to_slot(2)
            .expect("warp beyond the Series retry deadline");
        let before =
            capture_series_account_snapshots_v1(&mut context, &fixture.material_snapshot_keys)
                .await
                .expect("caller hostile prestates");
        let refusal =
            match submit_v0_observed(&mut context, &instructions, addresses, None, &[]).await {
                Ok(_) => panic!("{hostile:?} unexpectedly executed"),
                Err(refusal) => refusal,
            };
        assert_eq!(
            refusal_code(&refusal.error),
            Some(expected as u32),
            "exact caller seam must own {hostile:?}: {:#?}",
            refusal.logs,
        );
        let after =
            capture_series_account_snapshots_v1(&mut context, &fixture.material_snapshot_keys)
                .await
                .expect("caller hostile poststates");
        assert_series_premarket_expiry_rollback_v1(&report, &before, &after)
            .expect("complete caller-hostile transaction rollback");
    }
}
