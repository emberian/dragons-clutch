//! Accepted pre-Market Series Expire and exact hostile rollback through real
//! Registry, Trading, Core, Custody and Claims ELFs in a ProgramTest bank.
//!
//! Measurements, source identities and artifact hashes belong in
//! `docs/evidence/SERIES_PREMARKET_EXPIRY_PROGRAM_TEST_2026_09_07.md`.
//! This campaign does not execute real Series founding on a local validator.
//!
//! The release compiler is `series::release_v5`; the generated Expire frame is
//! the authority for its coordinates. The operator and fixture independently
//! pack the same selected instruction and assert exact agreement here.
//!
//! Expiry keeps the future Market vacant while closing its transient Custody
//! projection, refunding recorded rent to RentCredit, consuming the permit,
//! and committing the Trading replay poststates. Core observes replay accounts
//! through readonly child views; Trading remains their sole commit authority.
//!
//! Caller key, privileges, owner and data have distinct refusal codes. Every
//! hostile case verifies exact account rollback. A preflight walk can reject a
//! later child before any CPI, so an earlier Custody invocation is not evidence
//! that the precommit caller check ran.

#[path = "support/series_premarket_expiry_chain_v1.rs"]
mod series_premarket_expiry_chain_v1;
#[path = "support/series_premarket_expiry_v1.rs"]
mod series_premarket_expiry_v1;

use dclutch_market::SERIES_FOUNDING_PERMIT_BYTES_V1;
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

use dclutch_market::capability_program::CAPABILITY_ROOT_SELECTION_OFFSET;
use dclutch_market::capability_program::hot_v3::DIRECT_HOT_HEAP_FRAME_BYTES_V1;
use dclutch_registry_sbf::RegistryError;
use dclutch_registry::release_set::CAPABILITY_EXECUTION_SELECTION_CONFIG_OFFSET;
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
            TradingSbfError::SeriesPrecommitCallerKey
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
            TradingSbfError::SeriesPrecommitCallerPrivileges
        }
        PrecommitCallerHostileV1::ForeignOwner => {
            let caller = fixture
                .install_accounts
                .iter_mut()
                .find(|candidate| candidate.key == fixture.precommit_caller)
                .expect("precommit caller install account");
            caller.account.owner = CORE_PROGRAM_ID;
            TradingSbfError::SeriesPrecommitCallerOwner
        }
        PrecommitCallerHostileV1::NonemptyBody => {
            let caller = fixture
                .install_accounts
                .iter_mut()
                .find(|candidate| candidate.key == fixture.precommit_caller)
                .expect("precommit caller install account");
            caller.account.data = vec![0x80];
            caller.account.lamports = Rent::default().minimum_balance(caller.account.data.len());
            TradingSbfError::SeriesPrecommitCallerData
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
/// accounts, submits, and refuses at the shared preflight-composition wall
/// with the other two, at the identical 533,198 CU and `Release` (`0x4001`).
/// Its own positive control -- that the permit really is one lamport under
/// today's floor -- runs and holds before the submission, so what is OWED is a
/// parent-ELF green, not a diagnosis. The inversion itself is proved on the
/// native side by
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

/// THE TWO HOSTILES THE CONFIG-IDENTITY RULING OWED, ON THE REAL ELFs.
///
/// `2cf96117a` settled that a Series root's `selection().config()` is the
/// Registry RECORD DIGEST `hash(config_record_bytes)` and not the
/// domain-separated `template_content_id(t)`, and
/// `native_tests::the_series_root_config_identity_has_one_author` proves the
/// two values are distinct and that the artifact join derives the second from
/// the bytes. What neither shows is that the PROGRAM refuses when a root
/// carries the wrong one. These two do, on the deployed ELFs.
///
/// Both perturb the one account the two readers share -- the finalized
/// Template record, which IS this family's config record -- from opposite
/// sides. `RootNamesTheContentId` leaves the record alone and restates the
/// ROOT's config field as `template_content_id(t)`, which is a coordinate at
/// which no Registry record can exist, so the derived raw address is not the
/// account presented. `TemplateBytesRestated` leaves the root alone and flips
/// one byte of the RECORD, so `hash(&data)` is no longer the digest the root
/// and the PDA both name.
///
/// # THE POSITIVE CONTROL IS RUN, NOT ASSERTED FROM THE FIXTURE
///
/// Both legs refuse `Content` (`0x4003`), which is the code 2,124 sites of this
/// program publish and which the shared pre-Market Expire wall publishes too --
/// so matching the discriminant proves nothing on its own (ledger `M-38`), and
/// `borrow_record_against` fuses ELEVEN conjuncts into that one code. The row
/// therefore submits the UNPERTURBED fixture first, in its own bank, and
/// requires each hostile to refuse STRICTLY CHEAPER than it does. That is a
/// measurement of where the transaction stopped, and it is self-calibrating:
/// when the shared wall moves, the control moves with it and this row keeps
/// meaning the same thing. What it does NOT claim is which of the eleven
/// conjuncts fired; that needs the discriminant split, and the CU each leg
/// reports is what a lane doing that split will bind its new code to.
#[derive(Clone, Copy, Debug)]
enum ConfigIdentityHostileV1 {
    RootNamesTheContentId,
    TemplateBytesRestated,
}

/// The exact code each restated identity refuses with, and WHERE.
///
/// They are not the same program, and that is the finding rather than an
/// inconvenience. A root naming the content id never reaches Trading at all:
/// the Registry's own transparent continuation refuses it, so the code is
/// `RegistryError::Continuation` in band 1 and no Series byte is read. A
/// restated Template record does reach Trading and refuses
/// `TradingSbfError::Content` in the family-neutral record borrow. Two
/// perturbations of one account, two programs, two bands -- which is exactly
/// why this row could never have been written as `is_err()`.
const fn config_identity_hostile_code_v1(hostile: ConfigIdentityHostileV1) -> u32 {
    match hostile {
        ConfigIdentityHostileV1::RootNamesTheContentId => RegistryError::Continuation as u32,
        ConfigIdentityHostileV1::TemplateBytesRestated => TradingSbfError::Content as u32,
    }
}

fn apply_config_identity_hostile_v1(
    fixture: &mut SeriesPremarketExpiryChainFixtureV1,
    hostile: ConfigIdentityHostileV1,
) {
    match hostile {
        ConfigIdentityHostileV1::RootNamesTheContentId => {
            let content_id = fixture.template_content_id;
            let root = fixture
                .install_accounts
                .iter_mut()
                .find(|candidate| candidate.key == fixture.parent_root)
                .expect("parent root install account");
            let start = CAPABILITY_ROOT_SELECTION_OFFSET + CAPABILITY_EXECUTION_SELECTION_CONFIG_OFFSET;
            let field = root
                .account
                .data
                .get_mut(start..start + 32)
                .expect("root selection config field");
            assert_ne!(
                field, content_id,
                "the unperturbed root must NOT already name the content id",
            );
            field.copy_from_slice(&content_id);
        }
        ConfigIdentityHostileV1::TemplateBytesRestated => {
            let record = fixture
                .install_accounts
                .iter_mut()
                .find(|candidate| candidate.key == fixture.config_raw)
                .expect("Template/config record install account");
            let byte = record
                .account
                .data
                .first_mut()
                .expect("a nonempty Template record");
            *byte ^= 0x01;
        }
    }
}

#[tokio::test]
async fn a_restated_config_identity_refuses_before_the_composition() {
    // THE CONTROL, in its own bank: what the unperturbed fixture costs before
    // it stops. Every hostile below has to stop cheaper than this or it did not
    // reach its own subject.
    let control = {
        let artifacts = elves();
        let mut test = program_test_without_forced_budget(&artifacts);
        let fixture = build_chain(&mut test, &artifacts);
        install_series_premarket_expiry_accounts_v1(
            &mut test,
            &Rent::default(),
            &fixture.install_accounts,
            &fixture.externally_installed,
        )
        .expect("install control chain accounts");
        let instructions = series_expire_transaction_v1(&fixture);
        let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
        add_lookup_table(&mut test, &addresses);
        let mut context = start_with_substrate(test, fixture_substrate()).await;
        context
            .warp_to_slot(2)
            .expect("warp beyond the Series retry deadline");
        match submit_v0_observed(&mut context, &instructions, addresses, None, &[]).await {
            // The route completes: the control is then the whole successful
            // execution, and any refusal at all is cheaper than it.
            Ok(execution) => execution.compute_units_consumed,
            Err(refusal) => refusal.compute_units_consumed,
        }
    };
    assert!(control > 0, "the control must report a price");

    let mut observed: Vec<(ConfigIdentityHostileV1, Option<u32>, u64)> = Vec::new();
    for hostile in [
        ConfigIdentityHostileV1::RootNamesTheContentId,
        ConfigIdentityHostileV1::TemplateBytesRestated,
    ] {
        let artifacts = elves();
        let mut test = program_test_without_forced_budget(&artifacts);
        let mut fixture = build_chain(&mut test, &artifacts);
        let report = physical_report(&fixture);
        apply_config_identity_hostile_v1(&mut fixture, hostile);
        install_series_premarket_expiry_accounts_v1(
            &mut test,
            &Rent::default(),
            &fixture.install_accounts,
            &fixture.externally_installed,
        )
        .expect("install config-identity hostile chain accounts");
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
                .expect("config-identity hostile prestates");
        let refusal =
            match submit_v0_observed(&mut context, &instructions, addresses, None, &[]).await {
                Ok(_) => panic!("{hostile:?} unexpectedly executed"),
                Err(refusal) => refusal,
            };
        observed.push((
            hostile,
            refusal_code(&refusal.error),
            refusal.compute_units_consumed,
        ));
        let after =
            capture_series_account_snapshots_v1(&mut context, &fixture.material_snapshot_keys)
                .await
                .expect("config-identity hostile poststates");
        assert_series_premarket_expiry_rollback_v1(&report, &before, &after)
            .expect("complete config-identity hostile rollback");
    }
    let wrong = observed
        .iter()
        .filter(|(hostile, code, cost)| {
            *code != Some(config_identity_hostile_code_v1(*hostile)) || *cost >= control
        })
        .count();
    assert_eq!(
        wrong, 0,
        "each restated config identity must refuse its own exact code strictly \
         cheaper than the {control}-CU control: {observed:#?}",
    );
}

/// Coord80 is an exact controller-scoped Trading PDA and only the inner Core
/// CPI may synthesize its signer privilege. The former funded-crank topology,
/// a substituted key, and non-vacant account bodies all refuse atomically.
///
/// Each caller defect now has its own discriminant. Shape checks run before
/// common profile projection and PDA equality runs in Core preflight; neither
/// requires a child CPI. The old assertion that Custody must already have run
/// confused the preflight walk with execution and could never pass. Every leg
/// still runs, names its precise cause, and proves complete state rollback.
#[tokio::test]
async fn precommit_caller_substitutions_refuse_with_exact_state_reversion() {
    let mut observed: Vec<(PrecommitCallerHostileV1, Option<u32>, u32, u64)> = Vec::new();
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
        observed.push((
            hostile,
            refusal_code(&refusal.error),
            expected as u32,
            refusal.compute_units_consumed,
        ));
        let after =
            capture_series_account_snapshots_v1(&mut context, &fixture.material_snapshot_keys)
                .await
                .expect("caller hostile poststates");
        assert_series_premarket_expiry_rollback_v1(&report, &before, &after)
            .expect("complete caller-hostile transaction rollback");
    }
    let wrong = observed
        .iter()
        .filter(|(_, code, expected, _)| *code != Some(*expected))
        .count();
    assert_eq!(
        wrong, 0,
        "each caller defect must name its unique refusal: {observed:#?}",
    );
}
