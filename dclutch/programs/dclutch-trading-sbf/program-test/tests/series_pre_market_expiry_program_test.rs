//! Current-source positive pre-Market Series Expire campaign through the real
//! Registry, Trading, Core, Custody, and Claims ELFs.
//!
//! Fixture construction lives here; the release compiler remains owned by
//! `dclutch_trading_sbf::series::release_v5`, and the physical evidence joins
//! remain owned by the adjacent support module.
//!
//! # THREE ROWS ARE RED AND THE FIXTURE IS NOT THE AUTHOR
//!
//! `build_chain` refuses `BuilderError::Projection("borrowed-range-resolve")`
//! on all three campaign rows, and the two `native_tests` beside it pass. This
//! is not a fixture that stages the wrong request; the Series Expire ARTIFACT
//! SET contradicts itself, and no request width satisfies both halves:
//!
//! * `series::expire_funding_artifacts_v5::emit_request_profile` encodes a **V1**
//!   RequestProfile at `RequestGeometryV1::new(SERIES_ACTION_HEADER_BYTES_V3, 0, ...)`,
//!   so the family request is fixed at exactly 128 bytes. That is an EQUALITY on
//!   both sides of the boundary: `dclutch_request_profile_contract::project_atomic`
//!   refuses `InvalidLength` otherwise, and `hot_v3`'s `require_request_shape`
//!   refuses `TradingSbfError::Content` for the same reason on the real ELF.
//! * `series::expire_funding_artifacts_v5::emit_effect` declares route 4 -- the
//!   Core unallocated-permit leg -- one borrowed range at `Fixed(128)` of length
//!   `CommonScalar(SERIES_EXPIRE_PROOF_BYTES_SCALAR_V5)`, which the transition
//!   computes as `proof_count * SERIES_WITNESS_ITEM_BYTES_V3`. A borrowed range
//!   is canonically nonempty, so `dclutch_effect_kernel::v4::BorrowedRangeV4::resolve`
//!   refuses when that length is zero.
//!
//! `proof_count = 0` refuses in the effect; `proof_count >= 1` refuses in the
//! request profile. Measured 2026-09-03 by staging both: a two-occurrence
//! template with one Merkle sibling moves the refusal from
//! `borrowed-range-resolve` to `request-projection`/`InvalidLength` at 160
//! bytes, and moves it back. Neither is reachable from this file.
//!
//! Two candidate repairs, both program code in an SBF link and both changing
//! shipped artifact digests, so neither is taken here: widen the Expire request
//! profile to a shape that admits a proof tail, or withdraw route 4's range and
//! let Core authenticate the occurrence without a borrowed proof. The sibling
//! families are NOT in the same position and that is the clue --
//! `prepare_funding_artifacts_v5` attaches its proof range to
//! `SEMANTIC_RANGE_ROUTE_V4` rather than to a child route, so nothing resolves
//! it per invocation and nothing refuses.
//!
//! # DECIDED, 2026-09-03, BY THE KERNEL RATHER THAN BY A PREFERENCE
//!
//! "Whoever owns Series decides" stood here, and it turns out the Series kernel
//! had already decided. Three lines settle it, and none of them is in this
//! file:
//!
//! * `dclutch-series-v3-kernel/src/lib.rs:868` refuses unless
//!   `proof_count == proof_height(template.occurrence_count)`. An EQUALITY, not
//!   a floor.
//! * `:1140` makes `proof_height(count)` **zero** for `count <= 1`.
//! * so for a single-occurrence Series -- which is what this campaign stages,
//!   and `support/series_premarket_expiry_chain_v1.rs:2343` admits its
//!   occurrence with a literally EMPTY proof and succeeds -- `proof_count = 0`
//!   is not merely permitted. It is the only value the kernel admits.
//!
//! `SeriesActionRequestV3::decode` agrees from the other side: it pins
//! `proof_count == 0` for `Retire` and `Close` and constrains `Expire` not at
//! all, which is exactly the line that would say so if a nonempty proof were
//! meant.
//!
//! THAT ELIMINATES THE FIRST REPAIR, including the shape one would reach for
//! first. A V3 borrowed-witness profile cannot express an empty proof either:
//! `BorrowedWitnessPolicyV3::validate`
//! (`crates/dclutch-request-profile-contract/src/v3.rs:90`) refuses
//! `minimum_bytes == 0`, for the same reason
//! `BorrowedRangeV4::resolve` (`crates/dclutch-effect-kernel/src/v4.rs:334`)
//! refuses a zero length. Both spellings of "a borrowed thing is here" are
//! canonically nonempty, and the canonical Series Expire has nothing to borrow.
//!
//! So the repair is the SECOND one: **route 4 must not declare a borrowed
//! range**, and the V1 profile fixed at 128 is already right for the case this
//! route serves. It is still not taken here, and now for a stated reason rather
//! than for indecision -- it has TWO authors and the second is not in the
//! artifact file:
//!
//! * `series/expire_funding_artifacts_v5.rs:766-782`, the range itself, plus
//!   the module's own `range_count() == 1` pin at `:1282` and the fixed
//!   `SERIES_EXPIRE_EFFECT_V4_BYTES_V5` width;
//! * `hot_v3.rs:12251`, which selects the Series expiry local replay overlap
//!   ONLY when `ranges.count() == 1 && ranges.range(0) == family.proof_bytes()`.
//!   Withdrawing the range makes that read zero and silently returns
//!   `AllowedLocalOverlapV3::None`, so the conjunct has to be rewritten to
//!   admit "no range, and no proof to borrow" in the same commit or the
//!   replay overlap disappears without a word.
//!
//! AND A SECOND DEFECT IS EXPOSED BY THE SAME READING, larger than this one and
//! not this test's: a MULTI-occurrence Series expiry needs `proof_count >= 1`
//! and therefore a 128 + 32n family request, which the V1 profile's
//! `item_request_bytes = 0` forbids at every tail count. Withdrawing the range
//! does not fix that; it makes it visible. Multi-occurrence Series expiry is
//! unreachable as shipped, and `consume_artifacts_v4` is in the same position
//! with no real-ELF program-test to report it.

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
