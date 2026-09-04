//! Current-source positive pre-Market Series Expire campaign through the real
//! Registry, Trading, Core, Custody, and Claims ELFs.
//!
//! Fixture construction lives here; the release compiler remains owned by
//! `dclutch_trading_sbf::series::release_v5`, and the physical evidence joins
//! remain owned by the adjacent support module.
//!
//! # THREE ROWS ARE STILL RED, AND THE WALL IS NOW ONE CONTRADICTION
//!
//! The transaction reaches the bank on every row, the Registry continuation
//! lands, Trading is invoked at depth two, and all three rows refuse the same
//! way at the same place: **Trading consumes 330,987 CU of 1,318,157 and
//! refuses `Content` (`0x4003`) inside the FAMILY-NEUTRAL config-record
//! borrow** -- `borrow_finalized_record_at(descriptor.config_schema(),
//! context.selection().config(), ..)` in `authenticate_and_execute_hot_v3`.
//! Instrumented (`--features hot-cu-profile` plus temporary markers, not
//! committed), `borrow_record_against` reports three flags at once: the raw
//! key is not the expected one, the staging key is not the expected one, and
//! `hash(&data) != digest`. It is one cause, not three.
//!
//! ## THE SERIES ROOT'S CONFIG IDENTITY HAS TWO AUTHORS
//!
//! `native_tests::the_series_root_config_identity_has_two_authors_that_cannot_agree`
//! proves the arithmetic without an ELF, and the two authorities are:
//!
//! - **Family-neutral.** `borrow_record_against` refuses unless
//!   `hash(&config_record_bytes) == context.selection().config()`, so a root's
//!   config identity is the Registry RECORD DIGEST of the account at
//!   `HOT_CONFIG_RAW_ACCOUNT_V3`. `dealer/mod.rs` spells the same rule inline,
//!   and `crates/dclutch-operator/src/series_hot_v3.rs` -- the production Series
//!   Hot instruction builder -- requires those bytes to be the Template record
//!   itself (`hash(&config.account.data) == hash(template_bytes)`).
//! - **Series.** Six sites require the same field to be the DOMAIN-SEPARATED
//!   Template content identity, `template_content_id(t) =
//!   sha256("dclutch/series-template-v3" || 0x00 || t)`:
//!   `trading-sbf/src/series/accounts.rs::authenticate_root`,
//!   `series/artifacts_v3.rs` (`request.template() != selection.template`, and
//!   the same operator supplies `selection.template =
//!   header.selection().config()`), and four Core routes -- `series_open.rs`,
//!   `series_consume.rs`, `series_permit_expiry.rs` and
//!   `series_permit_expiry_precommit_v1.rs` -- each of which independently pins
//!   `request.template() == template_content_id(&template_bytes)`.
//!
//! `sha256(t)` and `sha256(domain || 0x00 || t)` are different values, so no
//! Series root satisfies both and no fixture can stage one. Measured from the
//! other end too: staging the record digest instead moves the refusal to the
//! Series artifact selection, BEFORE the Series expiry prelude engages at all
//! -- 119,620 CU against that same ELF's 337,005 -- because the family
//! request's template field no longer matches the root. The same contradiction
//! seen from its opposite side. This is why nothing Series has ever executed through
//! the family-neutral Hot path, and it is a program ruling with two candidate
//! repairs, both of which move two ELFs and a witnessed route's convention:
//! either the Series sites read the record digest, or the Series Template's
//! content identity stops being domain-separated. This lane names it rather
//! than picking, because `core/series_consume` is witnessed today under the
//! second convention.
//!
//! ## WHAT CAME OUT TO GET HERE
//!
//! Two walls, both now gone, and the campaign walks the ENTIRE Series Expire
//! pre-Market authentication chain -- records, future projection, future-Market
//! vacancy, replay, Core template, permit and RentCredit -- before reaching the
//! one above.
//!
//! 1. **The Core route template's revisions.** The conjunct compared the Expire
//!    artifact's Core route template against the live family request and
//!    required the two expected revisions to be EQUAL. That template is the
//!    UN-PATCHED zero placeholder `encode_request_bank` documents; the Effect
//!    VM writes the request-projected scalars into those two slots immediately
//!    before the Core CPI. It admitted no reachable state. It now asserts what
//!    the emitter promises -- the placeholder is zero -- under its own code,
//!    `TradingSbfError::SeriesExpireCoreTemplate` (`0x402A`), and passes in 390
//!    CU. The live agreement is owned downstream, twice.
//! 2. **The Ticket's refund owner was the RentCredit's ADDRESS.** The kernel
//!    (`terminal.rs::requires_wallet`), Core
//!    (`series_permit_expiry.rs::authenticate_rent_credit_coordinates`) and
//!    Trading's pre-CPI mirror all require the RentCredit's `refund_wallet` to
//!    BE the Ticket's refund owner, and the RentCredit is the account rent
//!    lands in, never the beneficiary it is credited to. Staging the address
//!    also made the bundle's fee payer and the RentCredit the same key, which
//!    is why the below-minimum row could not find a RentCredit prestate -- the
//!    gap `05b15ffac` queued. Both are gone: all three rows now reach the
//!    transaction and refuse identically.
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

use dclutch_capability_program_contract::hot_v3::DIRECT_HOT_HEAP_FRAME_BYTES_V1;
use dclutch_direct_hot_program_test_support::waist::{
    CLAIMS_PROGRAM_ID, COMPUTE_LIMIT, CORE_PROGRAM_ID, CUSTODY_PROGRAM_ID, REGISTRY_PROGRAM_ID,
    RENT_PROGRAM_ID, TRADING_PROGRAM_ID, add_lookup_table, add_release_waist,
    canonical_lookup_addresses, elves, fixture_substrate, program_test_without_forced_budget,
    start_with_substrate, submit_v0_observed,
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

/// The complete transaction a Series Expire caller sends: compute limit, heap
/// request, then the transparent Registry continuation.
///
/// The heap request is not optional and is not a tuning knob. Every `DCLTHOT3`
/// route declares the extended heap profile
/// (`entrypoint_adapter::declares_extended_heap_profile_v1`), and BOTH arms of
/// `authenticate_root_against_market_boxed_v3` call
/// `require_declared_heap_ceiling_above_default_v1`, which refuses `HeapFrame`
/// by name when the declaration is not matched by a grant. Declaring makes a
/// grant admissible; only asking for one makes it arrive. A fixture that omits
/// it is refused before it reads a single Series byte -- which is exactly what
/// this one did, and it is the same absence that was the whole of
/// `registry_hot_continuation`'s five reds.
///
/// The outer goes LAST: the runtime clears return data at the start of every
/// top-level instruction, so a trailing ComputeBudget instruction would erase
/// the commit-last acknowledgement the Hot execution just produced. This route
/// carries no ed25519 native evidence -- a Series occurrence action is
/// authorized by its finalized records, not by maker signatures -- so nothing
/// here pins an instruction index the way a Direct trade's evidence does.
fn series_expire_transaction_v1(
    fixture: &SeriesPremarketExpiryChainFixtureV1,
) -> [solana_program::instruction::Instruction; 3] {
    [
        compute_budget_instruction_v1(
            2,
            &u32::try_from(COMPUTE_LIMIT)
                .expect("compute limit width")
                .to_le_bytes(),
        ),
        compute_budget_instruction_v1(1, &DIRECT_HOT_HEAP_FRAME_BYTES_V1.to_le_bytes()),
        fixture.top_level_instruction.clone(),
    ]
}

fn compute_budget_instruction_v1(
    discriminant: u8,
    argument: &[u8],
) -> solana_program::instruction::Instruction {
    let mut data = vec![discriminant];
    data.extend_from_slice(argument);
    solana_program::instruction::Instruction {
        program_id: solana_sdk_ids::compute_budget::ID,
        accounts: Vec::new(),
        data,
    }
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
    let instructions = series_expire_transaction_v1(&fixture);
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
/// IT IS ALSO ONE OF THE THREE RED ROWS THIS FILE'S HEADER DESCRIBES. It is no
/// longer red for a reason of its own: this row builds its chain, installs its
/// accounts, submits, and refuses at the shared config-identity wall with the
/// other two, at the identical 330,987 CU. Its own positive control -- that the
/// permit really is one lamport under today's floor -- runs and holds before
/// the submission, so what is OWED is a parent-ELF green, not a diagnosis. The
/// inversion itself is proved on the native side by
/// `series_expiry_permit_requires_exact_prefunded_writable_system_vacancy`.
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
    let instructions = series_expire_transaction_v1(&fixture);
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
        let instructions = series_expire_transaction_v1(&fixture);
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
