//! Current-source positive pre-Market Series Expire campaign through the real
//! Registry, Trading, Core, Custody, and Claims ELFs.
//!
//! Fixture construction lives here; the release compiler remains owned by
//! `dclutch_trading_sbf::series::release_v5`, and the physical evidence joins
//! remain owned by the adjacent support module.
//!
//! # THREE ROWS ARE STILL RED, AND THE CUSTODY CALLEE NOW RESOLVES
//!
//! All three rows refuse identically: **Trading consumes 530,018 CU of
//! 1,317,313 and refuses `Content` (`0x4003`)** in the FIRST Custody route's
//! preflight, after `pf-invocation-resolved`. Before this lane they refused
//! `Release` (`0x4001`) at 533,198 in `resolve_carrier_by_representative_v3`,
//! because the activated Custody program was at no coordinate of the frame.
//!
//! ## THE EXPIRE PROFILE WAS THE DEFECT, AND THE REPAIR RENUMBERS NO FRAME
//!
//! A CPI's callee is not a member of its own account list, and
//! `CustodyFrameRoleV1` has no `CustodyProgram` variant at all -- a Custody
//! frame names `CallerProgram`, which is Trading's. So no Custody route window
//! can carry the callee, and `hot_v3::resolve_role_carrier_v3` resolves one by
//! scanning the downgraded LOGICAL vector for the key the activation cache
//! names. Every other Custody-routing topology in this tree therefore declares
//! a coordinate of its own and says so in the same words: Direct's
//! inline-ordinary (90), RegisterBuy (55) and registered-terminal (70),
//! General's `general_custody_callee_coordinate_v3`, Dealer's
//! `DEALER_EQUITY_CUSTODY_CALLEE_ACCOUNT_COUNT_V3`, and
//! `custody_composition_v3::require_custody_frame_shape_v3`'s own doc comment.
//! Series Consume needed none because its Core Found suffix, Claims founding
//! frame and Core Open suffix each name the Custody program inside their own
//! frames -- those are the three carriers `resolve_role_carrier_v3` was taught
//! to dedup. Expire's five frames name it nowhere, and Expire was the only
//! Custody-routing topology in the tree without a callee coordinate.
//!
//! The bundle builder is NOT the defect. It packs exactly the profile's logical
//! coordinates and binds an unbound one to a placeholder; it uses
//! `WaistFactsV1::custody_program` to MINE Custody's two bumps and to leave the
//! bank's own deployment uninstalled, and neither of those is a frame entry. It
//! had nothing to bind because the profile declared nothing to bind to.
//!
//! So `SERIES_EXPIRE_CUSTODY_PROGRAM_COORDINATE_V5` is appended PAST every
//! route range, exactly as Direct's and General's are, and the blast radius the
//! blocked-route entry predicted does not happen: `SERIES_EXPIRE_ROUTE_STARTS_V5`,
//! `SERIES_EXPIRE_ROUTE_COUNTS_V5` and all thirty-seven `ROUTE_ALIASES` pairs
//! are byte-identical. Only the fixed count moves, 81 to 82, and with it the
//! Trading digest.
//!
//! ## THE NEXT WALL IS NAMED, AND IT IS A MARKET APART
//!
//! `dclutch-hot-why:custody-prepare` reports **case 6, bitmap `0xc`, operands
//! 1 and 9**: two of the six parent bindings inside the first escrow refund
//! request disagree with the executing envelope -- `custody.market !=
//! parent.market` and `custody.semantic.generation != parent.generation`, the
//! request naming generation 1 and the envelope 9.
//!
//! That is not a fixture typo. The Series escrow's replay, vault and transfer
//! authority are all PDAs of the FUTURE occurrence Market, derived at Prepare,
//! and `custody-sbf` requires the CoreMarket account at its own frame
//! coordinate 1 to have the key `request.market`. So the request must name the
//! future Market -- while `CustodyCompositionParentV3` binds every child
//! request to `envelope.market()`/`envelope.generation()`, which for a
//! pre-Market Expire is the PARENT Series root's Market at its own generation.
//! The pre-Market Expire topology has always been one Market and eight
//! generations away from the conjunct that binds it. Either the family-neutral
//! Custody composition gains a projected-market authority for this case (route
//! 3 already carries a projected shape whose parent root the Effect patches at
//! runtime), or the Series escrow lifecycle moves into the parent Market's
//! namespace. That ruling moves the child composition Direct and Dealer also
//! ride, and it is NOT taken here.
//!
//! ## THE SERIES ROOT'S CONFIG IDENTITY HAS ONE AUTHOR
//!
//! `native_tests::the_series_root_config_identity_has_one_author` proves it
//! without an ELF, and it was never really a choice between two conventions.
//! A Registry record's coordinate is `[RAW_RECORD_PDA_SEED_V1, schema,
//! digest]` with `digest == hash(bytes)`, and `borrow_record_against` refuses
//! unless `hash(&data) == digest`. So `template_content_id(t) =
//! sha256("dclutch/series-template-v3" || 0x00 || t)` names a coordinate at
//! which no Registry record can ever exist. A Series root's
//! `selection().config()` is `hash(t)` -- the record digest, exactly what
//! every other family's is, and what `selected_manifest_entry_v1` has always
//! written for every family including this one.
//!
//! Both values still exist and each has one author now:
//!
//! - `hash(t)` -- the root's config field, the manifest entry's `config_id`,
//!   the config record's PDA, and what Core's four Series routes compare the
//!   root against (`series_open.rs`, `series_consume.rs`,
//!   `series_permit_expiry.rs`, `series_permit_expiry_precommit_v1.rs`, each
//!   from the Template record's bytes it already borrowed).
//! - `template_content_id(t)` -- the family request's `template()`, the
//!   occurrence proof, the Ticket derivation, and what
//!   `SeriesArtifactSelectionV3::from_config_record` DERIVES from the config
//!   record's bytes. Its fields are private and that constructor is the only
//!   way in, so no caller can hand the artifact join a root's config field
//!   again.
//!
//! The Series config record IS the Template record: every Series action
//! descriptor pins `config_schema() == SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3`,
//! the schema the Template is installed under, and the Expire profile's own
//! `ROUTE_ALIASES` already declares coordinate 71 -- Core's Template raw
//! record -- an alias of coordinate 1, the config raw. The artifact had said
//! they were one account all along; only the root's config field disagreed.
//! `trading-sbf/src/series/accounts.rs::authenticate_root`, the sixth site,
//! was an orphan with zero callers and is deleted rather than resynchronised.
//!
//! ## FOUR MORE WALLS CAME DOWN BEHIND IT, AND ONE WAS A PROGRAM DEFECT
//!
//! 1. **The SPL Token program.** The bank deploys it as a Loader-V3 program;
//!    the fixture modelled an empty native-loader account. Coordinate 19's
//!    rule is `Exact`, so the projection refused `DataLengthMismatch`.
//! 2. **The Rent program.** `program_with_view` models a program the BANK
//!    deploys -- an empty installed stand-in with a 36-byte observed view --
//!    and nothing deploys `dclutch_rent_sbf` into this ProgramTest, so
//!    coordinate 57 held zero bytes where the rule declares 36.
//! 3. **The System builtin.** A native-loader builtin account holds its
//!    registered name, 21 bytes of `solana_system_program`;
//!    `system_program_builtin` is the tree's one author for that. It is the
//!    bank's account, so it is named externally installed, and the installer's
//!    Rent gate now applies only to accounts this campaign actually installs
//!    -- a bank-owned builtin's lamports were never this campaign's to fund.
//! 4. **`sealed_ownership.require` was UNSATISFIABLE for schema V3.** The
//!    static-ownership verdict is minted from `account_profile_token`, which
//!    names the whole Registry record; the require site presented
//!    `funding.base()`, an interior slice 24 bytes in and 24 bytes shorter.
//!    Pointer identity, so no equality test recovers it: 1,712 against a
//!    proved 1,736. That is a TRADING DEFECT, not a fixture one, and it had
//!    gone unnoticed because no schema-V3 family had ever reached the
//!    statement. `hot_v3.rs` now presents the record the token names.
//!
//! Numbers 1 through 3 are one class -- the fixture asserting a width for an
//! account the BANK owns -- and the campaign no longer has to reach a
//! 350,000-CU refusal to find them: `audit_expire_profile_data_lengths_v1`
//! compares every `Exact` rule against the packed frame before a transaction
//! exists and names the coordinate.
//!
//! ## THE INSTRUMENTS THAT FOUND ALL OF IT
//!
//! Four `map_err(|_| Content)` sites became `map_err` plus a diagnostic-only
//! `msg!`, which is AGENTS.md's own prescription and paid for itself four
//! times in one session. Under `--features hot-cu-profile`:
//! `dclutch-hot-why:account-projection` names the
//! `account_profile_contract::v2::Error`;
//! `dclutch-hot-why:data-length` walks the rules and prints the coordinate,
//! its declared width and its observed one; and
//! `dclutch-hot-why:sealed-ownership` names which of the verdict's four
//! artifact ranges strayed, with both lengths and both pointers. Each turned a
//! refusal with 2,126 candidate sites into a named line in one run. Nothing
//! here is compiled into a production ELF.
//!
//! ## THE NUMBER NAMES AN ELF
//!
//! 533,198 is measured on the Trading ELF built from the sources this file is
//! committed beside, not on the one the repair was developed against. Those are
//! different ELFs and they consume different CU: the same three rows read
//! 527,198 on the build that first cleared the sealed-ownership wall, and
//! adding the preflight checkpoints and the role-carrier diagnostics moved it
//! to 533,198 -- 6,000 CU, in a PLAIN build, from code that is entirely behind
//! `hot-cu-profile` and therefore absent. The frame manifest did not move at
//! all, so this is a codegen difference and not a new binding; the honest
//! reading is that the diagnostic scaffolding is not free even when it compiles
//! to nothing, and that a CU figure is a measurement of one artifact or it is
//! decoration.
//!
//! ## WHAT IS OWED
//!
//! The Market/generation ruling above, which no lane owns yet, and the
//! `TicketStateV3` producer in `prepare_funding_artifacts_v5`, which still
//! needs this route to reach Core.
//!
//! `precommit_caller_substitutions_...` is RE-BASED and is red for a stated
//! reason. It used to assert inside its loop, so the first disagreeing leg
//! ended the run and three legs were never measured. Run together they say:
//!
//! | leg | refuses | declared | tx CU | reached the seam |
//! |---|---|---|---|---|
//! | `Substitution` | `0x4003` | `0x4001` | 612,713 | no |
//! | `Writable` | `0x4003` | `0x4003` | 520,935 | no |
//! | `ForeignOwner` | `0x4003` | `0x4003` | 612,713 | no |
//! | `NonemptyBody` | `0x4003` | `0x4003` | 520,991 | no |
//!
//! THREE OF THE FOUR MATCHED ON THE DISCRIMINANT WITHOUT REACHING COORDINATE
//! 80. `ForeignOwner` matched the SHARED wall's `Content` at the identical
//! 612,713 the positive row spends; `Writable` and `NonemptyBody` matched a
//! DIFFERENT `Content`, 91,778 CU earlier, in the account projection. That is
//! ledger `M-38` exactly -- a universal-donor code standing in for a seam
//! nobody had reached -- and the row would have read green on all three the
//! moment its `Substitution` sibling was fixed. Every leg now has to prove it
//! reached the seam before its code is believed, and the witness is the log:
//! each hostile perturbs coordinate 80, which lives in route 4's Core window,
//! and route 4 is preflighted after all four Custody routes, so a run that
//! never invoked the Custody program never got there.
//!
//! ## WHAT CAME OUT EARLIER TO GET HERE
//!
//! Two walls from `8b5d1c96f`, both still gone. The Core route template's
//! revisions: the conjunct compared the Expire artifact's Core route template
//! against the live family request and required its two expected revisions to
//! be EQUAL, where that template is the UN-PATCHED zero placeholder
//! `encode_request_bank` documents. It now asserts the placeholder under its
//! own code, `TradingSbfError::SeriesExpireCoreTemplate` (`0x402A`), in 390
//! CU. And the Ticket's refund owner was the RentCredit's ADDRESS: the kernel,
//! Core and Trading's pre-CPI mirror all require the RentCredit's
//! `refund_wallet` to BE the Ticket's refund owner, and the RentCredit is the
//! account rent lands in, never the beneficiary it is credited to.
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

use dclutch_market::capability_program::hot_v3::DIRECT_HOT_HEAP_FRAME_BYTES_V1;
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

/// Coord80 is an exact controller-scoped Trading PDA and only the inner Core
/// CPI may synthesize its signer privilege. The former funded-crank topology,
/// a substituted key, and non-vacant account bodies all refuse atomically.
///
/// # RE-BASED, AND EVERY LEG IS NOW REPORTED
///
/// This row used to `assert_eq!` inside the loop, so the first leg that
/// disagreed ended the run and the other three never executed while one number
/// was reported for a surface nobody had measured -- the exact accounting
/// defect `run-postjoin-hostiles.sh` paid for. It now runs all four legs,
/// records what each one refused and at what price, and asserts once at the
/// end, so a rebasing lane reads four rows instead of one.
///
/// It also stopped being able to pass by accident. Three of the four legs
/// declare `Content`, which is the code 2,124 sites of this program publish and
/// which the SHARED pre-Market Expire wall publishes too, 200,000 CU upstream
/// of anything coordinate 80 owns -- so a leg matching on the discriminant
/// alone would have read as green while proving nothing (ledger `M-38`). Each
/// leg therefore has to prove it REACHED its subject before its code is
/// believed: every hostile here perturbs coordinate 80, which lives in route
/// 4's Core window, and route 4 is preflighted after all four Custody routes,
/// so a run that never invoked the Custody program never reached the seam. The
/// log is the witness, and `RefusedExecution` already carries it.
#[tokio::test]
async fn precommit_caller_substitutions_refuse_with_exact_state_reversion() {
    let mut observed: Vec<(PrecommitCallerHostileV1, Option<u32>, u32, u64, bool)> = Vec::new();
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
        let reached_seam = refusal
            .logs
            .iter()
            .any(|line| line.starts_with(&format!("Program {CUSTODY_PROGRAM_ID} invoke [")));
        observed.push((
            hostile,
            refusal_code(&refusal.error),
            expected as u32,
            refusal.compute_units_consumed,
            reached_seam,
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
        .filter(|(_, code, expected, _, reached)| !reached || *code != Some(*expected))
        .count();
    assert_eq!(
        wrong, 0,
        "the exact caller seam must own every leg, and every leg must reach it: {observed:#?}",
    );
}
