//! Real-ELF release-waist evidence for Direct Hot, on both of its routes.
//!
//! The campaign executes the canonical Direct fixed-topology bundle at the
//! protocol 1.4M compute ceiling. This test owns only transaction assembly and
//! observations; Registry and Trading remain the executable authorities.
//!
//! # Two routes live in this file, and which one a case takes is a RULING
//!
//! `DECISION_PACKET_2026_08_30` §4 ruled the TOP-LEVEL route production for
//! Direct Hot and demoted the Registry Hot continuation to harness-only, on
//! HEAPRED's evidence (`docs/evidence/CONTINUATION_ROUTE_FIX_OR_RETIRE_2026_08_30.md`):
//! the outer composition authenticates the same two roles over the same
//! activation cache, hands Trading the same children, executes the same trade
//! to the compute unit -- and charges a measured six-figure CU delta for it. No
//! caller outside this harness builds it: not the SDK, whose types cannot even
//! express the frame, not the web panel, not the CLI, not the devnet drivers.
//!
//! So the rule that decides where a case belongs is **what it proves**, never
//! what it happens to submit:
//!
//! * a case whose claim is about TRADING, its children, the market geometry,
//!   the validated-artifact seal or the rollback of a refusal takes
//!   `direct_top_level_instructions` -- the route that ships. What it proves
//!   has to be proven on the route the public actually sends.
//! * a case whose claim IS the outer composition -- the transparent wrapper's
//!   byte preservation, the admission PDA, the release-batch authentication,
//!   the V1/V2 seam split, the reentrancy wall that only exists because the
//!   Registry sits at CPI depth one -- takes `direct_registry_instructions` and
//!   stays there. Retiring the route retires those cases with it; that is the
//!   point, and it is why they are the ones left.
//!
//! # What the port bought, measured
//!
//! The continuation is at the wall and past it. On this tree it traded a
//! two-outcome market at 1,385,133 CU of 1,399,850 and a three-outcome market
//! at 1,396,465 -- margins of 14,717 and 3,385 -- and **exhausted the meter
//! outright at four outcomes**, which is the journey's own shipped geometry.
//! `the_family_trades_every_geometry_it_is_given` and
//! `a_four_outcome_market_trades_on_the_canonical_artifacts` were red for
//! exactly that and are green on the top-level route, which runs the same trade
//! with room to spare. The measured wall for geometry was thirty outcomes; on
//! the demoted route it had fallen to three.
//!
//! The ruling did not charter the compute fix, so this file does not attempt
//! one. It moves the evidence onto the route that ships and leaves the
//! continuation's own cases on the continuation, where a compute figure is the
//! harness's problem and no longer the product's.
//!
//! # Where the continuation's cost grew -- diagnosed, deliberately not fixed
//!
//! Recorded because the next person to look at this route will ask, and because
//! the obvious answer is the wrong one. Measured on one tree, one seed, one
//! pair of ELFs, at the canonical three-outcome geometry, both routes:
//!
//! | stage | continuation | top-level |
//! |---|---:|---:|
//! | Registry outer prologue | 94,550 | -- |
//! | Trading hot path | 1,282,544 | 1,277,869 |
//! | Claims child | 146,514 | 146,514 |
//! | Custody child | 128,016 | 128,024 |
//! | transaction total | 1,380,977 | 1,278,177 |
//!
//! Against `docs/evidence/DIRECT_GEOMETRY_2026_08_27.md`'s sweep of the same
//! route at `1b0fe8be`, the continuation has gained about forty thousand CU at
//! every geometry: 1,352,967 -> 1,395,295 at two outcomes, 1,341,795 ->
//! 1,381,127 at three, and 1,363,637 -> exhausted at four.
//!
//! **It is not the outer composition.** The prologue is 94,550 here against the
//! 95,778 modal draw HEAPRED measured on 2026-08-30, and every value on that
//! grid is `95,778 + 1500k` -- so today's is one bump attempt shallower, not a
//! cheaper outer. The children are identical to the COMPUTE UNIT across routes
//! (Claims 146,514 both), so they cannot explain a route-specific gap either.
//! What is left is the stage both routes share: **Trading's own hot path grew,
//! and only the top-level route was given anything to pay for it with.**
//! Decision 0017's option B took 66,921 CU off the top-level arm and explicitly
//! did not touch the continuation, which never paid those CPIs -- so one route
//! absorbed the growth into fresh headroom while the other met it with the
//! two-to-eighteen thousand CU of margin it already had.
//!
//! The continuation did not get worse at being a continuation. The trade got
//! more expensive for everybody and the continuation had nowhere to put it.
//!
//! Stated as a rate rather than a draw, because under ledger M-61 a one-byte
//! ELF change redraws every seed and a single-geometry comparison across
//! commits carries up to ~12,000 CU of bump noise. The ~40,000 holds across
//! three geometries, and the widest market this route can trade fell from
//! thirty outcomes to three, which is not something a draw does.

use std::{env, fs, path::PathBuf};

use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1,
    hot_v3::{
        DIRECT_HOT_HEAP_FRAME_BYTES_V1, HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3,
        HOT_CONFIG_RAW_ACCOUNT_V3, HOT_CONFIG_STAGING_ACCOUNT_V3, HOT_DESCRIPTOR_RAW_ACCOUNT_V3,
        HOT_DESCRIPTOR_STAGING_ACCOUNT_V3, HOT_FIXED_ACCOUNT_COUNT_V3, HotExecutionAckV3,
        HotExecutionEnvelopeV3,
    },
};
use dclutch_capability_seal_contract::{
    CAPABILITY_SEAL_ACTION_OFFSET_V1, CAPABILITY_SEAL_DESCRIPTOR_DIGEST_OFFSET_V1,
    CAPABILITY_SEAL_HEADER_BYTES_V1, CAPABILITY_SEAL_MAGIC_OFFSET_V1,
    CAPABILITY_SEAL_REGISTRY_OFFSET_V1, CAPABILITY_SEAL_ROW_BYTES_V1,
    CAPABILITY_SEAL_ROW_DIGEST_OFFSET_V1, CAPABILITY_SEAL_ROW_RAW_OFFSET_V1,
    CAPABILITY_SEAL_TRADING_RELEASE_OFFSET_V1, CAPABILITY_SEAL_VERDICTS_OFFSET_V1,
    CapabilitySealRequestV1,
};
use dclutch_custody_contract::CustodyReplayV1;
use dclutch_custody_sbf::CustodySbfError;
use dclutch_direct_codec::execution_v3::DirectExecutionActionV3;
use dclutch_direct_codec::ordinary_geometry_v3::DirectOrdinaryGeometryV3;
use dclutch_direct_codec::successor::{DirectMakerReplayLayoutV1, DirectRootStateLayoutV1};
use dclutch_registry_sbf::RegistryError;
use dclutch_token_svm::state::{MintLayoutV1, TokenAccountLayoutV1};
use dclutch_token_svm::{LEGACY_TOKEN_PROGRAM_ID, TokenAccount};
use dclutch_trading_sbf::TradingSbfError;
use solana_account::{Account, AccountSharedData};
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction, InstructionError},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::Signer;
use solana_sdk::transaction::TransactionError;
use solana_sdk_ids::{bpf_loader, system_program};

use dclutch_direct_hot_program_test_support::waist::{
    CLAIMS_PROGRAM_ID, COMPUTE_LIMIT, CUSTODY_PROGRAM_ID, DirectCase, Elves, REGISTRY_PROGRAM_ID,
    RefusedExecution, Releases, TRADING_PROGRAM_ID, add_lookup_table, add_release_waist,
    canonical_lookup_addresses, direct_case, direct_case_v2, direct_case_v4,
    direct_registry_instructions, direct_top_level_instructions, elves, fixture_substrate,
    legacy_registry_hot_instruction, program_test, program_test_without_forced_budget,
    registry_hot_instruction, start_with_substrate, submit_v0, submit_v0_observed,
};

// --- Named refusals ----------------------------------------------------------
//
// Every refusal assertion in this file names the code it requires, and every
// code is derived from the declaring program's own enum -- never written as a
// bare number (AGENTS.md "Refusal codes", decision 0007).
//
// A bare `is_err()` here was never a shortcut, it was a hole. For the whole
// heap-wall era every submission of the Direct bundle refused on the heap
// before it read anything the test cared about, so `is_err()` held for the
// hostile and the canonical submission alike. W2p took the wall down. Each
// case below now states the code it raises and, in a comment, the control that
// separates that code from one any submission of the same bundle would produce.

/// `RegistryError::Deployment`: Loader Program, ProgramData, linkage, slot, ELF,
/// or authority refused inside the Registry's release-batch authentication.
const REGISTRY_DEPLOYMENT_REFUSAL_CODE: u32 = RegistryError::Deployment as u32;
/// `RegistryError::Continuation`: the transparent Hot continuation refused its
/// header, its admission PDA, or the Hot coordinate frame it was handed.
const REGISTRY_CONTINUATION_REFUSAL_CODE: u32 = RegistryError::Continuation as u32;
/// `TradingSbfError::Content`: the validated-artifact seal, the seal key, or a
/// sealed record refused.
const TRADING_CONTENT_REFUSAL_CODE: u32 = TradingSbfError::Content as u32;
/// `TradingSbfError::Transition`: the checked data-defined transition refused.
const TRADING_TRANSITION_REFUSAL_CODE: u32 = TradingSbfError::Transition as u32;
/// `TradingSbfError::ChildReceipt`: a child CPI committed and handed back a
/// receipt that does not answer the request that asked for it.
const TRADING_CHILD_RECEIPT_REFUSAL_CODE: u32 = TradingSbfError::ChildReceipt as u32;
/// `TradingSbfError::NativeSignature`: instructions-sysvar or native-signature
/// evidence was absent or not exact.
const TRADING_NATIVE_SIGNATURE_REFUSAL_CODE: u32 = TradingSbfError::NativeSignature as u32;
/// `TradingSbfError::Commit`: an invoked child or local poststate differed
/// from the exact precomputed candidate after all authorized child effects.
const TRADING_COMMIT_REFUSAL_CODE: u32 = TradingSbfError::Commit as u32;
/// `CustodySbfError::TokenState`: a Mint, vault, or token account taking part
/// in a Custody transfer refused its parsed state or authority policy.
///
/// A CHILD's code, named here because a refusal Custody raises inside the
/// child walk reaches the transaction verbatim -- Trading does not rewrite it,
/// on either route and at whatever CPI depth that route puts it -- so a case
/// that means "the Custody child refused" has to say so in Custody's
/// vocabulary. `dclutch-custody-sbf` ships an rlib beside its cdylib for
/// exactly this (see this crate's `Cargo.toml`).
const CUSTODY_TOKEN_STATE_REFUSAL_CODE: u32 = CustodySbfError::TokenState as u32;

fn postjoin_hostile_elf(name: &str) -> Vec<u8> {
    let directory =
        PathBuf::from(env::var("POSTJOIN_SBF_OUT_DIR").expect("POSTJOIN_SBF_OUT_DIR is required"));
    fs::read(directory.join(name)).expect("required postjoin hostile ELF")
}

fn install_postjoin_hostile_token(test: &mut ProgramTest, elf: Vec<u8>) {
    test.add_account(
        Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID),
        Account {
            lamports: Rent::default().minimum_balance(elf.len()).max(1),
            data: elf,
            owner: bpf_loader::ID,
            executable: true,
            rent_epoch: 0,
        },
    );
}

/// The custom program code the refusal carried, so a test can name it.
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

/// Require one refusal to be exactly the named custom code.
fn assert_refusal(refusal: &RefusedExecution, expected: u32) {
    assert_eq!(
        refusal_code(&refusal.error).expect("custom refusal code"),
        expected,
        "refused as {:?} rather than the named code: {:#?}",
        refusal.error,
        refusal.logs
    );
}

/// One Hot frame the Registry boundary accepts, carrying a request Trading will
/// not run.
///
/// The coordinates are the live Direct case's own: the same activation cache,
/// the same Core and Trading deployment pair, the same Market and the same root
/// at the same prestate digest. Only the request body is a stub, so the Registry
/// boundary authenticates the frame all the way through
/// `authenticate_hot_coordinates` and the admission PDA, invokes Trading, and
/// Trading refuses the stub.
///
/// The coordinates were NOT always live. This fixture used to fabricate all
/// thirty-eight accounts as `[coordinate; 32]` placeholders, which meant the
/// nested root was an address with no account behind it -- so
/// `authenticate_hot_coordinates` refused every submission of this bundle on
/// `root.owner != trading` with `RegistryError::Continuation`, hostile or not.
/// Two of the four cases below claim a refusal that is exactly
/// `RegistryError::Continuation`, so under the old fixture their `is_err()` (and
/// even a named-code assertion) was satisfied by a refusal the untouched bundle
/// produced on its own. Live coordinates make the canonical submission reach
/// Trading, which is what
/// `the_registry_boundary_fixture_reaches_trading_when_nothing_is_hostile` pins,
/// and that is the control every hostile case below is measured against.
fn registry_boundary_hot(direct: &DirectCase) -> Instruction {
    let mut accounts = direct
        .chain
        .hot_instruction
        .accounts
        .get(..HOT_FIXED_ACCOUNT_COUNT_V3)
        .expect("hot fixed prefix")
        .to_vec();
    for meta in accounts.iter_mut() {
        meta.is_signer = false;
        meta.is_writable = false;
    }
    let (canonical, _) =
        HotExecutionEnvelopeV3::split_instruction(&direct.chain.hot_instruction.data)
            .expect("canonical Direct Hot envelope");
    let request = b"registry-boundary-fixture";
    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(request.len()).expect("boundary request width"),
        canonical.release_set(),
        canonical.market(),
        canonical.generation(),
        canonical.root_prestate_digest(),
    )
    .expect("canonical boundary envelope");
    let mut data = Vec::with_capacity(128 + request.len());
    data.extend_from_slice(&envelope.to_bytes());
    data.extend_from_slice(request);
    Instruction {
        program_id: TRADING_PROGRAM_ID,
        accounts,
        data,
    }
}

/// Build the release waist and one canonical Direct case for a boundary test.
fn boundary_case(test: &mut ProgramTest, artifacts: &Elves) -> (Releases, DirectCase) {
    let releases = add_release_waist(test, artifacts);
    let direct = direct_case(test, releases, artifacts, false);
    (releases, direct)
}

async fn activation_snapshot(context: &mut ProgramTestContext, activation: Pubkey) -> Account {
    context
        .banks_client
        .get_account(activation)
        .await
        .expect("activation read")
        .expect("activation account")
}

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

/// The collateral Mint, read from the token account that names it.
///
/// The chain fixture publishes the three collateral token accounts but not the
/// Mint behind them, and a test must not restate an address the chain already
/// states: bytes `TokenAccountLayoutV1::MINT..+32` of any of the three ARE the
/// Mint this trade settles in, so reading it back is the only derivation that
/// cannot drift from the fixture.
async fn collateral_mint(context: &mut ProgramTestContext, token_account: Pubkey) -> Pubkey {
    let value = account(context, token_account).await;
    let identity: [u8; 32] = value
        .data
        .get(TokenAccountLayoutV1::MINT..TokenAccountLayoutV1::MINT + 32)
        .expect("collateral token account Mint coordinate")
        .try_into()
        .expect("32-byte Mint identity");
    Pubkey::new_from_array(identity)
}

async fn corrupt_account_byte(
    context: &mut ProgramTestContext,
    key: Pubkey,
    offset: usize,
) -> Account {
    let mut value = account(context, key).await;
    let byte = value
        .data
        .get_mut(offset)
        .expect("hostile state byte in bounds");
    *byte ^= 1;
    context.set_account(&key, &AccountSharedData::from(value.clone()));
    value
}

/// Submit one boundary container, require the named refusal, and require the
/// release evidence to be byte-identical afterwards.
///
/// `expected` is not decoration. The four hostile containers below do not share
/// one refusal -- two are caught by the release-batch deployment check and two
/// by the transparent continuation -- and flattening them onto a single
/// `is_err()` would let any of them drift onto another's refusal, or onto the
/// canonical container's, without the test noticing.
async fn assert_registry_refusal(
    mut test: ProgramTest,
    releases: Releases,
    instruction: Instruction,
    expected: u32,
) -> RefusedExecution {
    let addresses =
        canonical_lookup_addresses(core::slice::from_ref(&instruction), Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;
    let before = activation_snapshot(&mut context, releases.activation).await;
    let refusal = submit_v0(&mut context, &[instruction], addresses, None, &[])
        .await
        .expect_err("hostile Registry continuation unexpectedly executed");
    assert_refusal(&refusal, expected);
    let after = activation_snapshot(&mut context, releases.activation).await;
    assert_eq!(after, before, "Registry refusal mutated release evidence");
    refusal
}

#[test]
fn release_fixture_uses_five_distinct_real_artifacts() {
    let artifacts = elves();
    for bytes in [
        &artifacts.registry,
        &artifacts.trading,
        &artifacts.core,
        &artifacts.claims,
        &artifacts.custody,
    ] {
        assert!(!bytes.is_empty());
    }
    let digests = [
        hash(&artifacts.registry).to_bytes(),
        hash(&artifacts.trading).to_bytes(),
        hash(&artifacts.core).to_bytes(),
        hash(&artifacts.claims).to_bytes(),
        hash(&artifacts.custody).to_bytes(),
    ];
    for (index, digest) in digests.iter().enumerate() {
        assert!(
            digests
                .get(index + 1..)
                .expect("digest suffix")
                .iter()
                .all(|other| other != digest)
        );
    }
}

#[test]
fn transparent_wrapper_preserves_exact_hot_bytes_and_places_one_admission_at_38() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let (releases, direct) = boundary_case(&mut test, &artifacts);
    let hot = registry_boundary_hot(&direct);
    let exact_hot_bytes = hot.data.clone();
    let (outer, admission) = registry_hot_instruction(releases, hot);
    assert_eq!(outer.data, exact_hot_bytes);
    let child = outer.accounts.get(6..).expect("nested Hot frame");
    assert_eq!(
        child
            .get(HOT_FIXED_ACCOUNT_COUNT_V3)
            .map(|meta| meta.pubkey),
        Some(admission)
    );
    assert_eq!(
        child.iter().filter(|meta| meta.pubkey == admission).count(),
        1
    );
}

/// The control every hostile boundary case below is measured against.
///
/// Nothing is tampered with, so the Registry authenticates the release batch,
/// the Hot coordinate frame and the ephemeral admission, and forwards the exact
/// bytes to Trading -- which refuses the stub request. A hostile case that
/// refuses *at the Registry* therefore refuses on its own merit, because this
/// container proves the boundary does not refuse the frame on its own.
///
/// This test exists because the boundary fixture used to fail this claim: with
/// fabricated coordinates the canonical container was refused by the Registry
/// with `RegistryError::Continuation`, the exact code two of the hostile cases
/// assert. Every one of them was a `is_err()` satisfied by the container, not
/// by the hostility.
#[tokio::test]
async fn the_registry_boundary_fixture_reaches_trading_when_nothing_is_hostile() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let (releases, direct) = boundary_case(&mut test, &artifacts);
    let (instruction, _) = registry_hot_instruction(releases, registry_boundary_hot(&direct));
    let refusal =
        assert_registry_refusal(test, releases, instruction, TRADING_CONTENT_REFUSAL_CODE).await;
    assert!(
        refusal.invoked(TRADING_PROGRAM_ID),
        "the canonical boundary container never reached Trading: {:#?}",
        refusal.logs
    );
}

/// A legacy `RegistryContinuationRequestV1` header takes the V1 seam, and the
/// bytes Trading is handed there are not the bytes the transparent seam hands
/// it.
///
/// UNRESOLVED (W2q-VAC): this test used to be called
/// `real_registry_refuses_legacy_headered_hot_container_atomically`, and that
/// claim is FALSE. The legacy magic still routes to the live `continuation_v1`
/// seam (`dclutch-registry-sbf/src/lib.rs` `process_instruction`), which ACCEPTS
/// the container and forwards it to Trading. Nothing in the Registry refuses a
/// legacy header, so the old `is_err()` was recording the stub request being
/// refused downstream and nothing about the header at all.
///
/// What is true, and what this now asserts, is the seam split. The identical
/// Hot frame reaches Trading through both seams and is refused with two
/// DIFFERENT named Trading codes: `NativeSignature` here, because the V1 seam
/// forwards the wrapper header as part of the request and the native evidence
/// no longer covers it, against `Content` for the transparent seam in
/// `the_registry_boundary_fixture_reaches_trading_when_nothing_is_hostile`. The
/// header is therefore observable at the child, which is exactly what
/// "transparent" is supposed to rule out for the V2 seam.
///
/// Whether `continuation_v1` should still admit a Core+Trading Hot container at
/// all is a question for its owner. This test cannot settle it and does not
/// pretend to.
#[tokio::test]
async fn a_legacy_headered_hot_container_takes_the_v1_seam_and_not_the_transparent_one() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let (releases, direct) = boundary_case(&mut test, &artifacts);
    let (instruction, _) =
        legacy_registry_hot_instruction(releases, registry_boundary_hot(&direct));
    let refusal = assert_registry_refusal(
        test,
        releases,
        instruction,
        TRADING_NATIVE_SIGNATURE_REFUSAL_CODE,
    )
    .await;
    assert!(
        refusal.invoked(TRADING_PROGRAM_ID),
        "the legacy headered container did not reach Trading: {:#?}",
        refusal.logs
    );
}

/// Swapping the Core and Trading program roles is caught by the release batch,
/// which runs before the continuation ever looks at the Hot frame -- so this
/// refusal is `Deployment`, not the `Continuation` the frame checks raise.
#[tokio::test]
async fn real_registry_refuses_reordered_core_and_trading_roles_atomically() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let (releases, direct) = boundary_case(&mut test, &artifacts);
    let (mut instruction, _) = registry_hot_instruction(releases, registry_boundary_hot(&direct));
    instruction.accounts.swap(1, 3);
    let refusal = assert_registry_refusal(
        test,
        releases,
        instruction,
        REGISTRY_DEPLOYMENT_REFUSAL_CODE,
    )
    .await;
    assert!(
        !refusal.invoked(TRADING_PROGRAM_ID),
        "a reordered role batch was forwarded to Trading: {:#?}",
        refusal.logs
    );
}

/// Same seam as the reordered roles: the substituted ProgramData fails the
/// Loader linkage inside the release batch, so `Deployment` is the merit here.
#[tokio::test]
async fn real_registry_refuses_substituted_core_programdata_atomically() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let (releases, direct) = boundary_case(&mut test, &artifacts);
    let (mut instruction, _) = registry_hot_instruction(releases, registry_boundary_hot(&direct));
    *instruction
        .accounts
        .get_mut(2)
        .expect("Core ProgramData prefix") =
        AccountMeta::new_readonly(releases.trading_programdata, false);
    let refusal = assert_registry_refusal(
        test,
        releases,
        instruction,
        REGISTRY_DEPLOYMENT_REFUSAL_CODE,
    )
    .await;
    assert!(
        !refusal.invoked(TRADING_PROGRAM_ID),
        "a substituted Core ProgramData was forwarded to Trading: {:#?}",
        refusal.logs
    );
}

/// One flipped continuation byte changes the Hot instruction digest, so the
/// admission PDA the Registry derives is not the one in the account list.
#[tokio::test]
async fn real_registry_refuses_altered_hot_bytes_atomically() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let (releases, direct) = boundary_case(&mut test, &artifacts);
    let (mut instruction, _) = registry_hot_instruction(releases, registry_boundary_hot(&direct));
    let byte = instruction.data.last_mut().expect("continuation byte");
    *byte ^= 1;
    let refusal = assert_registry_refusal(
        test,
        releases,
        instruction,
        REGISTRY_CONTINUATION_REFUSAL_CODE,
    )
    .await;
    assert!(
        !refusal.invoked(TRADING_PROGRAM_ID),
        "altered Hot bytes were forwarded to Trading: {:#?}",
        refusal.logs
    );
}

/// Aliasing the ephemeral admission over the Market coordinate breaks the Hot
/// coordinate frame the envelope names, which is a `Continuation` refusal.
#[tokio::test]
async fn real_registry_refuses_aliased_ephemeral_admission_atomically() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let (releases, direct) = boundary_case(&mut test, &artifacts);
    let (mut instruction, admission) =
        registry_hot_instruction(releases, registry_boundary_hot(&direct));
    *instruction
        .accounts
        .get_mut(REGISTRY_HOT_CHILD_START)
        .expect("first Hot account") = AccountMeta::new_readonly(admission, false);
    let refusal = assert_registry_refusal(
        test,
        releases,
        instruction,
        REGISTRY_CONTINUATION_REFUSAL_CODE,
    )
    .await;
    assert!(
        !refusal.invoked(TRADING_PROGRAM_ID),
        "an aliased ephemeral admission was forwarded to Trading: {:#?}",
        refusal.logs
    );
}

#[tokio::test]
async fn real_registry_executes_profile14_direct_hot_under_protocol_limit() {
    let artifacts = elves();
    let mut test = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case(&mut test, releases, &artifacts, false);
    let instructions = direct_registry_instructions(releases, &direct);
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
    .expect("Registry-authenticated Direct Hot execution");
    let units = execution.compute_units_consumed;
    assert!(units > 0 && units <= COMPUTE_LIMIT);

    let root = account(&mut context, direct.chain.root).await;
    assert_eq!(root.owner, TRADING_PROGRAM_ID);
    assert!(!root.data.is_empty());
    let replay = account(&mut context, direct.chain.custody_replay).await;
    let replay = CustodyReplayV1::decode(&replay.data).expect("post-Custody replay");
    assert_eq!(replay.next_revision, 8);
    let source = account(&mut context, direct.chain.collateral_accounts[0]).await;
    let destination = account(&mut context, direct.chain.collateral_accounts[1]).await;
    assert_eq!(
        TokenAccount::parse(&source.data)
            .expect("source token")
            .amount,
        95
    );
    assert_eq!(
        TokenAccount::parse(&destination.data)
            .expect("destination token")
            .amount,
        35
    );
    let after = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    assert_ne!(
        after, before,
        "successful Direct Hot left no material state change"
    );
    let (producer, returned) = execution
        .return_data
        .expect("successful Hot execution must return commit-last evidence");
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

#[derive(Clone, Copy, Debug)]
enum SealedExecutionAliasHostile {
    Partial,
    WrongRaw,
    SeventhAlias,
    WritableRaw,
}

/// The Registry transparent-continuation prefix precedes the exact child Hot
/// frame in the outer instruction assembled by `registry_hot_instruction`.
///
/// Only the cases that remain on the continuation need it. A top-level
/// submission carries the Hot instruction itself, so a fixed-frame coordinate
/// sits at its own index and there is no prefix to skip.
const REGISTRY_HOT_CHILD_START: usize = 6;

/// Apply one sealed-execution alias hostile to the Hot instruction itself.
///
/// PORTED to the top-level route (`DECISION_PACKET_2026_08_30` §4). What this
/// proves is a TRADING property -- that the six duplicate metas are an exact,
/// sealed execution shape rather than a general relaxation of Hot fixed-account
/// distinctness -- so it belongs on the route the public sends. The hostiles
/// index the Hot fixed frame directly now; under the continuation they reached
/// the same metas through the outer's six-account prefix, which is the only
/// thing that changed.
fn apply_sealed_execution_alias_hostile(
    hostile: SealedExecutionAliasHostile,
    hot: &mut Instruction,
    direct: &DirectCase,
) {
    match hostile {
        SealedExecutionAliasHostile::Partial => {
            let distinct_staging = direct
                .chain
                .capability_seal_accounts
                .get(HOT_DESCRIPTOR_STAGING_ACCOUNT_V3)
                .expect("distinct descriptor staging")
                .clone();
            *hot.accounts
                .get_mut(HOT_DESCRIPTOR_STAGING_ACCOUNT_V3)
                .expect("descriptor staging meta") = distinct_staging;
        }
        SealedExecutionAliasHostile::WrongRaw => {
            let wrong = direct
                .chain
                .hot_instruction
                .accounts
                .get(HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3)
                .expect("account-profile raw")
                .clone();
            *hot.accounts
                .get_mut(HOT_DESCRIPTOR_STAGING_ACCOUNT_V3)
                .expect("descriptor staging meta") = wrong;
        }
        SealedExecutionAliasHostile::SeventhAlias => {
            let config_raw = direct
                .chain
                .hot_instruction
                .accounts
                .get(HOT_CONFIG_RAW_ACCOUNT_V3)
                .expect("config raw")
                .clone();
            *hot.accounts
                .get_mut(HOT_CONFIG_STAGING_ACCOUNT_V3)
                .expect("config staging meta") = config_raw;
        }
        SealedExecutionAliasHostile::WritableRaw => {
            hot.accounts
                .get_mut(HOT_DESCRIPTOR_RAW_ACCOUNT_V3)
                .expect("descriptor raw meta")
                .is_writable = true;
        }
    }
}

/// The six duplicate metas are an exact, sealed execution shape rather than a
/// general relaxation of Hot fixed-account distinctness. Exercise the real
/// Trading ELF for every boundary: an incomplete set, a staging coordinate
/// pointed at the wrong raw record, a seventh duplicate outside the closed set,
/// and a privilege escalation caused by message-key coalescing. Each refusal
/// reaches Trading, carries the semantic Content code, and rolls back every
/// tracked byte and lamport.
///
/// PORTED to the top-level route. The alias set is a property of
/// `HotFrameV3::parse` and the seal projection, both of which run identically
/// on either route -- and the top-level arm is the STRICTER of the two to
/// assert it on, because `authenticate_hot_invocation_v3` grants the
/// continuation `permits_fixed_market_union` and the top-level arm nothing.
/// Proving the closed alias set here proves it where the public sends it.
#[tokio::test]
async fn real_hot_refuses_noncanonical_sealed_execution_aliases_atomically() {
    for hostile in [
        SealedExecutionAliasHostile::Partial,
        SealedExecutionAliasHostile::WrongRaw,
        SealedExecutionAliasHostile::SeventhAlias,
        SealedExecutionAliasHostile::WritableRaw,
    ] {
        let artifacts = elves();
        let mut test = program_test_without_forced_budget(&artifacts);
        let releases = add_release_waist(&mut test, &artifacts);
        let direct = direct_case(&mut test, releases, &artifacts, false);
        let mut instructions = direct_top_level_instructions(&direct);
        apply_sealed_execution_alias_hostile(hostile, &mut instructions[3], &direct);
        let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
        add_lookup_table(&mut test, &addresses);
        let mut context = start_with_substrate(test, fixture_substrate()).await;
        let before = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
        let refusal = submit_v0(
            &mut context,
            &instructions,
            addresses,
            Some(&direct.payer),
            &[],
        )
        .await
        .expect_err("noncanonical sealed execution aliases unexpectedly executed");
        assert_refusal(&refusal, TRADING_CONTENT_REFUSAL_CODE);
        assert!(
            refusal.invoked(TRADING_PROGRAM_ID),
            "{hostile:?} never reached Trading: {:#?}",
            refusal.logs
        );
        let after = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
        assert_eq!(
            after, before,
            "{hostile:?} changed a tracked byte or lamport"
        );
    }
}

async fn assert_postjoin_hostile_rolls_back(
    test: ProgramTest,
    releases: Releases,
    direct: DirectCase,
    required_child: Pubkey,
    expected_refusal: u32,
) {
    let instructions = direct_registry_instructions(releases, &direct);
    let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
    let mut test = test;
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;
    let before = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    let refusal = submit_v0(
        &mut context,
        &instructions,
        addresses,
        Some(&direct.payer),
        &[],
    )
    .await
    .expect_err("hostile child poststate unexpectedly committed");
    assert_refusal(&refusal, expected_refusal);
    assert!(
        refusal.invoked(CLAIMS_PROGRAM_ID),
        "Claims did not commit before the postjoin refusal: {:#?}",
        refusal.logs
    );
    assert!(
        refusal.invoked(required_child),
        "the hostile child was not invoked before the postjoin refusal: {:#?}",
        refusal.logs
    );
    let after = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    assert_eq!(
        after, before,
        "postjoin refusal failed to roll back every tracked byte and lamport"
    );
}

/// The Claims adversary commits the genuine transfer, returns the genuine ACK,
/// and flips one bit of a NONSELECTED aggregate supply on its way out.
///
/// # This case names `Commit`, and it did not always
///
/// It named `Transition` until `39b75718` ("trading: avoid duplicate direct
/// claims poststate hash"), and the code moved because the OWNER of the check
/// moved -- not because the refusal weakened.
///
/// Before `39b75718`, `execute_claims_route_v3` hashed the three actual child
/// bodies immediately and compared them to the digest the receipt declared;
/// `verify_sparse_native_receipt` raised `TradingSbfError::Transition` on a
/// mismatch. `39b75718` observed that Direct ordinary already re-proves that
/// same conjunction after the complete child walk, and made the sparse
/// post-resource join `SparsePostResourceVerificationV3::DirectFinalization`
/// on this route -- so the body hash is no longer taken inside the Claims
/// composition at all. The corruption is now caught where the projection is
/// joined to the physical bytes, `verify_direct_inline_account_poststate_v3`,
/// which raises `TradingSbfError::Commit`.
///
/// **`Commit` is the semantically correct code here, and `Transition` was
/// always the weaker fit.** The refusal registry gives `Transition` as "the
/// checked data-defined transition refused" and `Commit` as "a projected
/// physical mutation or account write could not commit". This adversary runs
/// the REAL Claims transfer and returns its REAL ACK, so the data-defined
/// transition did not refuse anything -- every declared fact checks out. What
/// fails is that the bytes on the chain afterwards are not the bytes Trading
/// projected. That is a commit-class fact by the registry's own words, and it
/// is exactly what this file's `TRADING_COMMIT_REFUSAL_CODE` doc comment
/// already describes: an invoked child poststate differing from the exact
/// precomputed candidate after all authorized child effects.
///
/// The sibling below is the control that keeps this from being a blanket
/// shift: `omitted_custody_replay_lineage_corruption_after_real_child_commit_rolls_back`
/// still names `Transition` and still gets it, because a corrupted replay
/// lineage IS a declared-fact mismatch that Custody receipt verification
/// catches before any digest comparison.
#[tokio::test]
async fn nonselected_claims_supply_corruption_after_real_child_commit_rolls_back() {
    let mut artifacts = elves();
    artifacts.claims = postjoin_hostile_elf("dclutch_postjoin_claims_hostile_sbf.so");
    let mut test = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case(&mut test, releases, &artifacts, false);
    assert_postjoin_hostile_rolls_back(
        test,
        releases,
        direct,
        CLAIMS_PROGRAM_ID,
        TRADING_COMMIT_REFUSAL_CODE,
    )
    .await;
}

#[tokio::test]
async fn omitted_token_close_authority_corruption_after_real_custody_commit_rolls_back() {
    let artifacts = elves();
    let mut test = program_test_without_forced_budget(&artifacts);
    install_postjoin_hostile_token(
        &mut test,
        postjoin_hostile_elf("dclutch_postjoin_token_hostile_sbf.so"),
    );
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case(&mut test, releases, &artifacts, false);
    assert_postjoin_hostile_rolls_back(
        test,
        releases,
        direct,
        CUSTODY_PROGRAM_ID,
        TRADING_COMMIT_REFUSAL_CODE,
    )
    .await;
}

#[tokio::test]
async fn omitted_custody_replay_lineage_corruption_after_real_child_commit_rolls_back() {
    let mut artifacts = elves();
    artifacts.custody = postjoin_hostile_elf("dclutch_postjoin_custody_hostile_sbf.so");
    let mut test = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case(&mut test, releases, &artifacts, false);
    // `ChildReceipt` 0x4020 and not `Transition`, and the disagreement is this
    // hostile finally REACHING its subject rather than a code moving. Until the
    // continuation carried a heap grant this case died out of memory before
    // Custody ran, and its expectation was never tested against anything (ledger
    // M-38). The code it names was split out of `Transition` in the interval,
    // and this hostile is precisely the class it was split for -- its own doc's
    // precondition is this test's own name: "the child COMMITTED -- this code is
    // only reachable after its CPI returned success -- and then its receipt
    // ... decoded and did not `verify_for` the request digest that asked for
    // it." The adversary corrupts `LAST_POSTSTATE_COMMITMENT` in the replay
    // account after the genuine Custody ACK, so `verify_custody_receipt_v3`
    // recomputes a replay digest the receipt does not answer.
    //
    // Its two siblings keep `Transition` and pass: they corrupt state the
    // transition itself reads, which is a refused state change and a different
    // investigation.
    assert_postjoin_hostile_rolls_back(
        test,
        releases,
        direct,
        CUSTODY_PROGRAM_ID,
        TRADING_CHILD_RECEIPT_REFUSAL_CODE,
    )
    .await;
}

/// The journey's SHAPE wall, executed: a four-outcome market trades.
///
/// JRNY-2 named this as an independent wall -- "the shipped Direct profile is
/// emitted for the canonical geometry, this market is another one" -- on the
/// premise that a non-canonical market would need its own artifact emission.
/// It does not. Product Runtime V2 pins `outcome_count = cut_count + 2`, so
/// this market is four outcomes and two cuts, and it selects the SAME
/// descriptor, the SAME ProgramSet, the SAME validated-artifact seal and the
/// SAME six artifacts the canonical three-outcome market does -- proven byte
/// for byte in `the_artifacts_are_the_same_bytes_at_every_geometry`. What
/// changes is the market: a wider result domain, a wider portfolio, wider
/// Claims aggregate and Position records, and a Product tail of four that the
/// executor resolves every runtime-width rule against.
///
/// This is the whole claim under real ELFs at the real ceiling. The economics
/// are the canonical ones -- the same fill at the same price for the same
/// collateral -- because the geometry must not move them: the traded outcome
/// is one coordinate of the tail either way, and the other coordinates carry
/// a Claims quantity of zero that the transition's epilogue requires to sum
/// away.
///
/// # PORTED, and this case is why the port was urgent
///
/// This ran on the Registry Hot continuation until `DECISION_PACKET_2026_08_30`
/// §4, and by 2026-08-31 it was RED: the continuation exhausted all 1,399,850
/// CU at four outcomes. Not a shape refusal -- `ProgramFailedToComplete`, the
/// meter. Measured on this tree, the continuation traded two outcomes at
/// 1,385,133 CU and three at 1,396,465 and had nothing left for a fourth, so
/// the journey's own shipped geometry did not fit on the route the journey was
/// being measured on.
///
/// The claim was never about the Registry outer. It is that ONE artifact set
/// serves every geometry, which is a Product Runtime V2 and Trading property,
/// so it belongs on the route that ships -- and it fits there with room. The
/// continuation's compute is a harness problem the ruling declined to charter.
#[tokio::test]
async fn a_four_outcome_market_trades_on_the_canonical_artifacts() {
    let artifacts = elves();
    let mut test = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case_v4(
        &mut test,
        releases,
        &artifacts,
        false,
        false,
        fixture_substrate(),
        DirectOrdinaryGeometryV3::from_outcome_count(4).expect("four-outcome geometry"),
    );
    let instructions = direct_top_level_instructions(&direct);
    let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;
    let before = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    let units = submit_v0(
        &mut context,
        &instructions,
        addresses,
        Some(&direct.payer),
        &[],
    )
    .await
    .expect("top-level Direct Hot execution at four outcomes");
    assert!(units > 0 && units <= COMPUTE_LIMIT);
    println!("four-outcome Direct Hot: consumed {units} compute units");

    let root = account(&mut context, direct.chain.root).await;
    assert_eq!(root.owner, TRADING_PROGRAM_ID);
    assert!(!root.data.is_empty());
    let replay = account(&mut context, direct.chain.custody_replay).await;
    let replay = CustodyReplayV1::decode(&replay.data).expect("post-Custody replay");
    assert_eq!(replay.next_revision, 8);
    // The identical collateral movement as the canonical geometry: a fill of
    // ten at fifty against a scale of a hundred, at a zero venue rate.
    let source = account(&mut context, direct.chain.collateral_accounts[0]).await;
    let destination = account(&mut context, direct.chain.collateral_accounts[1]).await;
    assert_eq!(
        TokenAccount::parse(&source.data)
            .expect("source token")
            .amount,
        95
    );
    assert_eq!(
        TokenAccount::parse(&destination.data)
            .expect("destination token")
            .amount,
        35
    );
    let after = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    assert_ne!(
        after, before,
        "successful Direct Hot left no material state change"
    );
}

/// Trade one market at one geometry, or report why it did not.
///
/// PORTED to the top-level route with the two cases it serves. A geometry sweep
/// measures how wide a market the HOT PATH can trade, so running it through the
/// demoted outer measured the outer instead: it charged a six-figure constant
/// for authentication Trading performs anyway, and that constant came directly
/// out of the width the sweep could report.
async fn trade_at_geometry(artifacts: &Elves, outcomes: u32) -> Result<u64, RefusedExecution> {
    let mut test = program_test_without_forced_budget(artifacts);
    let releases = add_release_waist(&mut test, artifacts);
    let direct = direct_case_v4(
        &mut test,
        releases,
        artifacts,
        false,
        false,
        fixture_substrate(),
        DirectOrdinaryGeometryV3::from_outcome_count(outcomes).expect("geometry"),
    );
    let instructions = direct_top_level_instructions(&direct);
    let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;
    submit_v0(
        &mut context,
        &instructions,
        addresses,
        Some(&direct.payer),
        &[],
    )
    .await
}

/// A market too wide to fit must run out of COMPUTE, never be refused its shape.
///
/// This is the invariant the lane's whole finding rests on, stated where it can
/// fail. A geometry refusal at any width would mean a market's own dimensions
/// had reached an artifact after all -- that some coordinate resolved a width
/// instead of stating an affine rule -- which is precisely what
/// `the_artifacts_are_the_same_bytes_at_every_geometry` says is not so. The
/// widest geometry that actually fits is a measured-profile bound and lives in
/// `the_widest_geometry_the_shipped_hot_path_can_trade`, not here: pinning a
/// compute figure in a gate turns every unrelated improvement into a red test.
fn a_geometry_that_does_not_fit_ran_out_of_compute(outcomes: u32, refusal: &RefusedExecution) {
    let exceeded = refusal.logs.iter().any(|line| {
        line.contains("exceeded CUs meter")
            || line.contains("exceeded maximum number of instructions")
    });
    assert!(
        exceeded,
        "a {outcomes}-outcome market was refused for something other than compute: {:#?}",
        refusal.logs
    );
}

/// Nine consecutive geometries trade on one artifact set, and none is refused
/// its shape.
///
/// The routine gate. Every market from two outcomes (zero cuts, the protocol
/// floor) to ten trades on the same descriptor, seal and six artifacts, at the
/// real ceiling under real ELFs, with no re-emission of anything between them.
#[tokio::test]
async fn the_family_trades_every_geometry_it_is_given() {
    let artifacts = elves();
    for outcomes in 2..=10_u32 {
        let execution = trade_at_geometry(&artifacts, outcomes).await;
        if let Err(refusal) = &execution {
            a_geometry_that_does_not_fit_ran_out_of_compute(outcomes, refusal);
        }
        assert!(
            execution.is_ok(),
            "a {outcomes}-outcome market ran out of compute; the measured wall was 31 outcomes \
             and something has made the hot path much more expensive"
        );
        if let Ok(units) = execution {
            println!(
                "geometry: {outcomes} outcomes ({} cuts) traded at {units} CU",
                outcomes - 2
            );
            assert!(units > 0 && units <= COMPUTE_LIMIT);
        }
    }
}

/// The widest market the shipped Hot path can trade, measured on demand.
///
/// MEASURED-PROFILE BOUND, 2026-08-27, `main` at the commit that added this
/// test, against the prebuilt role ELFs: **thirty outcomes, twenty-eight
/// cuts**. Thirty-one does not fit, and it does not fit because it exhausts
/// the 1.4M compute ceiling -- asserted, not assumed.
///
/// The striking part is that compute is nearly FLAT in the geometry across
/// that whole range: 1,333,997 CU at eight outcomes and 1,390,325 at nine, with
/// the ordering non-monotone throughout. The per-outcome cost -- three folded
/// TransitionVM instructions, two projected scalar registers, one row in each
/// runtime-width record -- is smaller than the run-to-run variation in PDA
/// bump-seed searching, which moves with the market's content-addressed
/// identities and therefore with the geometry. Thirty is where the accumulated
/// tail finally overruns a hot path that already sits at ~96% of the ceiling
/// for its own reasons; it is not a Direct-family width limit, and it will move
/// with every hot-path compute change in either direction.
///
/// # PORTED, and the old bound was never this route's
///
/// The thirty-outcome figure above was measured through the Registry Hot
/// continuation, which is exactly the contamination HEAPRED's §3.5 names: the
/// test is called `the_shipped_hot_path` and it was measuring the route that is
/// NOT shipped, carrying an outer composition no public caller can build. Every
/// width it ever reported was therefore a lower bound on the real one, short by
/// whatever the outer charged.
///
/// It now sweeps `direct_top_level_instructions`. **The number above is stale
/// by construction and will move UP** -- it is left as written because a
/// measurement is a dated fact and overwriting it with a guess would be worse
/// than leaving it labelled. Re-run this case to replace it, and record the new
/// bound with the ELF digest it belongs to, per ledger M-61.
///
/// Ignored by default: it is a minute of real-ELF execution, and the number it
/// produces is a measurement, not a gate. Re-measure with
///
/// ```text
/// SBF_OUT_DIR=target/deploy cargo test \
///     --manifest-path programs/dclutch-trading-sbf/program-test/Cargo.toml \
///     --test registry_hot_continuation -- --ignored --nocapture \
///     the_widest_geometry_the_shipped_hot_path_can_trade
/// ```
#[tokio::test]
#[ignore = "one minute of real-ELF execution; produces a measured bound, not a verdict"]
async fn the_widest_geometry_the_shipped_hot_path_can_trade() {
    let artifacts = elves();
    let mut widest = 0_u32;
    for outcomes in 2..=96_u32 {
        match trade_at_geometry(&artifacts, outcomes).await {
            Ok(units) => {
                println!(
                    "geometry sweep: {outcomes} outcomes ({} cuts) traded at {units} CU",
                    outcomes - 2
                );
                widest = outcomes;
            }
            Err(refusal) => {
                println!(
                    "geometry sweep: {outcomes} outcomes ({} cuts) did not fit at \
                     {} CU; the widest market that traded was {widest} outcomes ({} cuts)",
                    outcomes - 2,
                    refusal.compute_units_consumed,
                    widest - 2,
                );
                a_geometry_that_does_not_fit_ran_out_of_compute(outcomes, &refusal);
                break;
            }
        }
    }
    assert!(
        widest >= 4,
        "the journey's four-outcome market must trade; widest was {widest}"
    );
}

/// A late Custody refusal rolls back everything the earlier children wrote.
///
/// PORTED to the top-level route. Atomicity across the child walk is a Trading
/// property -- it is Trading that invokes Claims, then Custody, and Trading's
/// crosscheck that refuses -- so the route it is proven on should be the one
/// that ships. The `registry_hot` in the name is a fossil of where it used to
/// run and is kept only so the evidence documents that cite it still resolve.
///
/// # RE-AIMED: the staged corruption moved from the destination to the Mint
///
/// This case used to stage an UNINITIALIZED DESTINATION -- `direct_case`'s
/// `corrupt_destination`, which clears the destination's
/// `TokenAccountLayoutV1::STATE` byte -- and rely on Custody's
/// `check_transfer_account` to refuse it one CPI level below Trading, after
/// Claims had already committed. `410320ac` ("direct: authenticate exact hot
/// finalization") took that failure mode away, CORRECTLY, and the case went
/// red on its own depth assertions rather than on its rollback claim.
///
/// `410320ac` added `project_tokens_v3`
/// (`crates/dclutch-direct-codec/src/direct_finalization_v3.rs`), the typed
/// candidate precompute Trading runs at `prepare_direct_inline_hot_crosscheck_v3`
/// BEFORE the first child CPI. Its conjunct `seller.state !=
/// AccountState::Initialized` catches exactly this fixture's byte, so Trading
/// now refuses `Transition` at ~809,000 CU having invoked nothing. That is
/// strictly better than the old behavior -- it refuses without paying for two
/// child CPIs and without any child write to unwind -- so the hardening stands
/// and the TEST is what had to move.
///
/// **The old failure mode is not merely harder to reach, it is GONE.** For a
/// destination in `CompartmentV1::External`, Custody's entire refusal surface
/// is `ExactTransferProfileV1::check_transfer_account` plus two owner
/// comparisons: token-program mismatch, unparseable bytes, `check_active`
/// (uninitialized OR frozen), a native reserve, a Mint mismatch, and a
/// `semantic.destination_owner` mismatch. `project_tokens_v3` checks every one
/// of those pre-CPI and is strictly stronger besides -- it pins the exact
/// address, the exact owner and the exact balance. Custody's profile
/// deliberately admits a destination's own delegate and close authority
/// ("those facts do not affect the amount credited by this transfer",
/// `crates/dclutch-token-svm/src/profile.rs`), so corrupting THOSE refuses
/// nowhere at all -- the candidate is projected from the observed prestate, so
/// the poststate still matches and the trade succeeds. There is no byte of the
/// destination account that reaches a late Custody refusal any more.
///
/// So the case is re-aimed onto a corruption the precompute genuinely does not
/// see: the MINT. Trading's hot path never parses the Mint account -- it only
/// compares each token account's `mint` FIELD to the context address, and the
/// sole `check_mint` in Trading belongs to `direct_token_setup_v1`, a different
/// instruction. Custody parses it on every transfer. Clearing
/// `MintLayoutV1::IS_INITIALIZED` therefore sails through the whole precompute,
/// through the Claims child, and refuses inside Custody as
/// `Error::MintUninitialized` -> `CustodySbfError::TokenState`. Same depth,
/// same rollback claim -- reached by a failure mode that is still late.
///
/// The case now also NAMES its code, which it never did before: it asserted
/// only `is_err()` plus the two depth assertions, and AGENTS.md is explicit
/// that this is not a refusal assertion. Measured on this tree, Custody's
/// `0x6006` reaches the transaction verbatim -- Trading does not rewrite a
/// child refusal that propagates out of the child walk -- so the code named
/// here is Custody's, derived from `CustodySbfError`.
///
/// The early refusal the old fixture now produces did not lose its coverage:
/// `an_uninitialized_custody_destination_refuses_before_any_child_runs` below
/// keeps `corrupt_destination` alive and pins `410320ac`'s hardening by name.
#[tokio::test]
async fn late_custody_refusal_rolls_back_registry_hot_claims_and_lifecycle() {
    let artifacts = elves();
    let mut test = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case(&mut test, releases, &artifacts, false);
    let instructions = direct_top_level_instructions(&direct);
    let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;
    // Through the BANK, not through `direct.chain.accounts`: those were already
    // installed into the ProgramTest by `direct_case`, so mutating them here
    // would never reach the chain and would leave the assertion below satisfied
    // by whatever the untouched bundle refuses on (ledger `M-38`).
    let mint = collateral_mint(&mut context, direct.chain.collateral_accounts[1]).await;
    corrupt_account_byte(&mut context, mint, MintLayoutV1::IS_INITIALIZED).await;
    let before = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    let refusal = submit_v0(
        &mut context,
        &instructions,
        addresses,
        Some(&direct.payer),
        &[],
    )
    .await
    .expect_err("uninitialized collateral Mint unexpectedly accepted");
    // The control that keeps this code from being a universal donor is
    // `direct_hot_top_level::direct_inline_ordinary_executes_when_submitted_top_level_to_trading`,
    // which submits THIS bundle on THIS route with nothing corrupted and
    // executes. So the refusal below belongs to the staged Mint and to nothing
    // the canonical submission would have produced on its own.
    assert_refusal(&refusal, CUSTODY_TOKEN_STATE_REFUSAL_CODE);
    // A rollback assertion over an execution that never started is vacuous: it
    // holds for any refusal, including one raised before the first child CPI.
    // The claim under test is specifically that Trading reached its Custody
    // child, that child refused, and everything the earlier children wrote was
    // rolled back. Require the depth the name claims.
    assert!(
        refusal.invoked(CLAIMS_PROGRAM_ID),
        "the Claims children this test claims to roll back never ran: {:#?}",
        refusal.logs
    );
    assert!(
        refusal.invoked(CUSTODY_PROGRAM_ID),
        "the late Custody refusal was never reached: {:#?}",
        refusal.logs
    );
    let after = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    assert_eq!(
        after, before,
        "late Custody refusal failed to roll back Claims/lifecycle bytes or lamports"
    );
}

/// An uninitialized Custody destination is refused before ANY child is invoked.
///
/// The other half of the re-aim above, and the case that keeps `direct_case`'s
/// `corrupt_destination` fixture in use. `410320ac` moved this corruption from
/// a Custody refusal inside the child walk to a Trading refusal before it, and that
/// move is a property worth pinning in its own right: the exact-finalization
/// precompute is what makes a malformed destination cost two child CPIs less
/// than it used to, and a later change that quietly deferred the check back to
/// Custody would be a regression this file should catch.
///
/// The negative depth assertions are the whole point, because the code alone
/// does not locate the refusal: `TradingSbfError::Transition` is raised from
/// dozens of sites across Trading's hot path, on both sides of the child walk,
/// and `prepare_direct_inline_finalization_into_v3`'s eleven typed error
/// variants are all collapsed onto it by one `map_err` in
/// `prepare_direct_inline_account_finalization_v3`. Only the ABSENCE of both
/// child invocations says the precompute is what refused, which is exactly the
/// fact `410320ac` established and the fact a regression would take away.
#[tokio::test]
async fn an_uninitialized_custody_destination_refuses_before_any_child_runs() {
    let artifacts = elves();
    let mut test = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case(&mut test, releases, &artifacts, true);
    let instructions = direct_top_level_instructions(&direct);
    let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;
    let before = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    let refusal = submit_v0(
        &mut context,
        &instructions,
        addresses,
        Some(&direct.payer),
        &[],
    )
    .await
    .expect_err("uninitialized Custody destination unexpectedly accepted");
    assert_refusal(&refusal, TRADING_TRANSITION_REFUSAL_CODE);
    assert!(
        !refusal.invoked(CLAIMS_PROGRAM_ID),
        "the destination precompute was expected to refuse before the Claims child ran: {:#?}",
        refusal.logs
    );
    assert!(
        !refusal.invoked(CUSTODY_PROGRAM_ID),
        "the destination precompute was expected to refuse before the Custody child ran: {:#?}",
        refusal.logs
    );
    let after = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    assert_eq!(
        after, before,
        "a refusal raised before the first child CPI still moved a tracked byte or lamport"
    );
}

#[tokio::test]
async fn corrupt_profile14_root_reserved_byte_refuses_without_mutation() {
    let artifacts = elves();
    let mut test = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case(&mut test, releases, &artifacts, false);
    let instructions = direct_registry_instructions(releases, &direct);
    let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;
    corrupt_account_byte(
        &mut context,
        direct.chain.root,
        CAPABILITY_ROOT_HEADER_BYTES_V1 + DirectRootStateLayoutV1::RESERVED,
    )
    .await;
    let before = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    let refusal = submit_v0(
        &mut context,
        &instructions,
        addresses,
        Some(&direct.payer),
        &[],
    )
    .await
    .expect_err("noncanonical Direct root unexpectedly accepted");
    // The refusal is the Registry's, not Trading's: `authenticate_hot_coordinates`
    // hashes the root and requires the envelope's `root_prestate_digest`, so one
    // flipped reserved byte is caught before the child is ever invoked. The
    // control is `real_registry_executes_profile14_direct_hot_under_protocol_limit`,
    // which is this same bundle without the flip and executes.
    assert_refusal(&refusal, REGISTRY_CONTINUATION_REFUSAL_CODE);
    assert!(
        !refusal.invoked(TRADING_PROGRAM_ID),
        "a noncanonical root reached Trading: {:#?}",
        refusal.logs
    );
    let after = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    assert_eq!(after, before, "root refusal mutated Profile14 state");
}

/// Restore every rollback-tracked account except the maker replays.
///
/// A maker replay is only *live* after an execution has written it, and that
/// same execution advances the root -- which the Hot envelope pins by prestate
/// digest, so a second submission of the same bundle is refused before any
/// replay is read at all. Rewinding everything but the replays is what puts a
/// live replay in front of a bundle that will still be forwarded.
///
/// Which PROGRAM raises that refusal is the one thing the route changes, and it
/// is why this helper is still needed after the port rather than in spite of
/// it. Under the continuation the Registry's `authenticate_hot_coordinates`
/// caught the stale digest at the boundary; top-level, Trading checks the same
/// envelope field itself. Same pin, same rewind, one less program.
async fn rewind_except_maker_replays(
    context: &mut ProgramTestContext,
    direct: &DirectCase,
    prestate: &[(Pubkey, Option<Account>)],
) {
    for (key, value) in prestate {
        if direct.chain.maker_replays.contains(key) {
            continue;
        }
        let account = value.clone().unwrap_or(Account {
            lamports: 0,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        });
        context.set_account(key, &AccountSharedData::from(account));
    }
}

/// A live maker replay whose reserved byte is not canonical is refused, and the
/// refusal is the replay's own content -- not the fact that it is live.
///
/// The control is the first half of this test: the identical rewound bundle
/// with the live replays left exactly as the execution wrote them refuses with
/// `Transition`, because the replay revision has moved past the one the request
/// names. Flipping one reserved byte moves the refusal to `Content`, which is
/// the maker replay failing to decode. Two different named codes over the same
/// bundle is the whole evidence: it is what a bare `is_err()` here could never
/// have shown, and for as long as this test asserted only `is_err()` after a
/// plain second submission it was showing nothing at all -- that submission is
/// refused at the root prestate digest, corrupt maker byte or not, before any
/// replay is read.
///
/// PORTED to the top-level route: the maker replay's decode and its revision
/// are Trading's, on either route, so this proves them where the public sends
/// them. It also executes a full successful trade first, to make the replay
/// live, which is the reason it belongs off a route that no longer reliably
/// completes one.
#[tokio::test]
async fn corrupt_live_profile14_maker_reserved_byte_refuses_without_mutation() {
    let artifacts = elves();
    let mut test = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case(&mut test, releases, &artifacts, false);
    let instructions = direct_top_level_instructions(&direct);
    let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;
    let prestate = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    submit_v0(
        &mut context,
        &instructions,
        addresses.clone(),
        Some(&direct.payer),
        &[],
    )
    .await
    .expect("first-use execution creates live maker replay");

    rewind_except_maker_replays(&mut context, &direct, &prestate).await;
    let control = submit_v0(
        &mut context,
        &instructions,
        addresses.clone(),
        Some(&direct.payer),
        &[],
    )
    .await
    .expect_err("a spent maker replay was accepted a second time");
    assert_refusal(&control, TRADING_TRANSITION_REFUSAL_CODE);
    assert!(
        control.invoked(TRADING_PROGRAM_ID),
        "the live maker replay was never read: {:#?}",
        control.logs
    );

    rewind_except_maker_replays(&mut context, &direct, &prestate).await;
    let hostile = corrupt_account_byte(
        &mut context,
        direct.chain.maker_replays[0],
        DirectMakerReplayLayoutV1::RESERVED,
    )
    .await;
    assert_eq!(hostile.owner, TRADING_PROGRAM_ID);
    let before = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    let refusal = submit_v0(
        &mut context,
        &instructions,
        addresses,
        Some(&direct.payer),
        &[],
    )
    .await
    .expect_err("noncanonical live maker replay unexpectedly accepted");
    assert_refusal(&refusal, TRADING_CONTENT_REFUSAL_CODE);
    let after = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    assert_eq!(after, before, "maker refusal mutated Profile14 state");
}

// --- Decision 0005: the validated-artifact seal ------------------------------
//
// The hot campaign above installs the seal already written, which is what a
// Market that has sealed a closure once actually finds. That would be circular
// evidence on its own: it proves the hot path accepts a seal the *fixture*
// wrote. These tests close the circle by making the on-chain seal outer write
// it and requiring the result to equal the fixture's bytes exactly, and then by
// refusing every seal that is not the canonical one.

/// Build the seal outer for one Direct case.
///
/// The account list is the hot fixed prefix with the root read-only and the
/// seal writable, followed by the rent payer and the System Program.
fn seal_instruction(direct: &DirectCase, action: u32, descriptor_digest: [u8; 32]) -> Instruction {
    let mut accounts = direct.chain.capability_seal_accounts.clone();
    assert_eq!(accounts.len(), HOT_FIXED_ACCOUNT_COUNT_V3);
    for meta in accounts.iter_mut() {
        meta.is_writable = meta.pubkey == direct.chain.capability_seal;
        meta.is_signer = false;
    }
    accounts.push(AccountMeta::new(direct.payer.pubkey(), true));
    accounts.push(AccountMeta::new_readonly(system_program::ID, false));
    Instruction {
        program_id: TRADING_PROGRAM_ID,
        accounts,
        data: CapabilitySealRequestV1::new(action, descriptor_digest)
            .expect("canonical seal request")
            .to_bytes()
            .to_vec(),
    }
}

async fn maybe_account(context: &mut ProgramTestContext, key: Pubkey) -> Option<Account> {
    context.banks_client.get_account(key).await.expect("read")
}

fn descriptor_digest(direct: &DirectCase) -> [u8; 32] {
    direct.chain.descriptor_digest
}

fn direct_action() -> u32 {
    DirectExecutionActionV3::InlineOrdinary as u32
}

/// The seal outer's transaction: two ComputeBudget instructions, then the outer.
///
/// The heap grant is REQUIRED, not decoration.
/// `process_capability_seal_v1` authenticates its Market and root through
/// `reauthenticate_top_level_root_roles_v3`, whose first act is
/// `require_declared_heap_ceiling_above_default_v1`, so a seal transaction carrying no
/// `RequestHeapFrame` refuses `TradingSbfError::HeapFrame` before it reads an
/// artifact. These cases ran without one until 2026-08-31 and went red the
/// moment the Hot arm's heap declaration landed (2026-08-30) -- the seal outer
/// calls the same prologue and was not on
/// `declares_extended_heap_profile_v1`'s list, so it refused unconditionally
/// and no new capability seal could be written on chain at all.
///
/// The forced compute budget has to go with it: `set_compute_max_units` makes
/// solana-program-test install one fixed budget and IGNORE the transaction's
/// own ComputeBudget instructions, `RequestHeapFrame` included, so the adapter
/// would lift its ceiling over a mapping the runtime never widened.
fn seal_transaction(instruction: Instruction) -> Vec<Instruction> {
    vec![
        ComputeBudgetInstruction::set_compute_unit_limit(
            u32::try_from(COMPUTE_LIMIT).expect("compute limit width"),
        ),
        ComputeBudgetInstruction::request_heap_frame(DIRECT_HOT_HEAP_FRAME_BYTES_V1),
        instruction,
    ]
}

async fn submit_seal(
    context: &mut ProgramTestContext,
    direct: &DirectCase,
    instruction: Instruction,
    addresses: &[Pubkey],
) -> Result<u64, RefusedExecution> {
    submit_v0(
        context,
        &seal_transaction(instruction),
        addresses.to_vec(),
        Some(&direct.payer),
        &[],
    )
    .await
}

#[tokio::test]
async fn the_seal_outer_writes_exactly_the_bytes_the_hot_path_expects() {
    let artifacts = elves();
    let mut test = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case_v2(&mut test, releases, &artifacts, false, true);
    let descriptor_digest = descriptor_digest(&direct);
    let canonical = seal_instruction(&direct, direct_action(), descriptor_digest);
    let addresses =
        canonical_lookup_addresses(&seal_transaction(canonical.clone()), direct.payer.pubkey());
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;

    assert!(
        maybe_account(&mut context, direct.chain.capability_seal)
            .await
            .is_none_or(|value| value.owner == system_program::ID && value.data.is_empty()),
        "the seal PDA is not vacant before the seal outer runs"
    );

    let units = submit_seal(&mut context, &direct, canonical.clone(), &addresses)
        .await
        .expect("canonical validated-artifact seal");
    assert!(units > 0 && units <= COMPUTE_LIMIT);

    let sealed = account(&mut context, direct.chain.capability_seal).await;
    assert_eq!(sealed.owner, TRADING_PROGRAM_ID);
    assert_eq!(
        sealed.data, direct.chain.capability_seal_bytes,
        "the on-chain seal outer and the fixture disagree about the verdict"
    );
    assert!(sealed.lamports >= Rent::default().minimum_balance(sealed.data.len()));

    // Write-once: a second seal of the same closure refuses and leaves the
    // recorded verdict byte-for-byte intact. The control is two lines up -- the
    // byte-identical first submission executed -- so the named refusal here is
    // the seal already being there and nothing else.
    let refused = submit_seal(&mut context, &direct, canonical, &addresses)
        .await
        .expect_err("an existing seal was rewritten");
    assert_refusal(&refused, TRADING_CONTENT_REFUSAL_CODE);
    let after = account(&mut context, direct.chain.capability_seal).await;
    assert_eq!(after.data, sealed.data);
}

#[tokio::test]
async fn a_seal_for_another_action_or_descriptor_never_lands_at_this_address() {
    let artifacts = elves();
    let mut test = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case_v2(&mut test, releases, &artifacts, false, true);
    let descriptor_digest = descriptor_digest(&direct);
    let hostile = [
        seal_instruction(&direct, direct_action() ^ 1, descriptor_digest),
        seal_instruction(&direct, direct_action(), [0x5a; 32]),
    ];
    let addresses = canonical_lookup_addresses(
        &hostile
            .iter()
            .cloned()
            .flat_map(seal_transaction)
            .collect::<Vec<_>>(),
        direct.payer.pubkey(),
    );
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;

    // The control is `the_seal_outer_writes_exactly_the_bytes_the_hot_path_expects`:
    // the same fixture and the same instruction builder, differing only in the
    // action and the descriptor digest, and it executes. So `Content` here is
    // the coordinates being wrong, not the seal outer refusing everything.
    for instruction in hostile {
        let refused = submit_seal(&mut context, &direct, instruction, &addresses)
            .await
            .expect_err("a seal filed under other coordinates reached the canonical address");
        assert_refusal(&refused, TRADING_CONTENT_REFUSAL_CODE);
        assert!(
            maybe_account(&mut context, direct.chain.capability_seal)
                .await
                .is_none_or(|value| value.owner == system_program::ID && value.data.is_empty()),
            "a refused seal left state at the canonical address"
        );
    }
}

/// Two hostile prestates at the canonical seal address: nothing there at all,
/// and a seal whose recorded Trading release is some other release.
///
/// The name used to claim both and the body exercised only the first. The
/// second is built here by writing a Trading-owned, rent-exempt seal at the
/// canonical address whose `trading_semantic_release` field is another
/// identity -- which is exactly what a seal minted under another release is,
/// since that field is one of the seeds the address is derived from, so such a
/// seal can only ever arrive here by being planted.
///
/// The control for both is
/// `direct_inline_ordinary_executes_when_submitted_top_level_to_trading` in
/// `direct_hot_top_level.rs`: the same bundle on the same route with the
/// fixture's own seal installed, and it executes.
///
/// PORTED to the top-level route. The validated-artifact seal is authenticated
/// by Trading, from the seal PDA, on both routes alike -- the Registry outer
/// never reads it -- so the route this is proven on should be the shipped one.
#[tokio::test]
async fn hot_refuses_a_missing_seal_and_a_seal_written_for_another_release() {
    let artifacts = elves();
    let mut test = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case_v2(&mut test, releases, &artifacts, false, true);
    let instructions = direct_top_level_instructions(&direct);
    let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;
    let refused = submit_v0(
        &mut context,
        &instructions,
        addresses.clone(),
        Some(&direct.payer),
        &[],
    )
    .await
    .expect_err("a hot action executed with no validated-artifact seal");
    assert_refusal(&refused, TRADING_CONTENT_REFUSAL_CODE);

    let mut data = direct.chain.capability_seal_bytes.clone();
    let release = data
        .get_mut(
            CAPABILITY_SEAL_TRADING_RELEASE_OFFSET_V1
                ..CAPABILITY_SEAL_TRADING_RELEASE_OFFSET_V1 + 32,
        )
        .expect("sealed Trading release field");
    release.copy_from_slice(&[0x6d; 32]);
    context.set_account(
        &direct.chain.capability_seal,
        &AccountSharedData::from(Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner: TRADING_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        }),
    );
    let refused = submit_v0(
        &mut context,
        &instructions,
        addresses,
        Some(&direct.payer),
        &[],
    )
    .await
    .expect_err("a seal minted under another Trading release was honoured");
    assert_refusal(&refused, TRADING_CONTENT_REFUSAL_CODE);
}

/// Every field of a written seal is authenticated, and altering any one of them
/// is refused by Trading inside the seal authentication itself.
///
/// PORTED to the top-level route, with the rest of the seal evidence. The eight
/// offsets swept below are all seal-body fields Trading hashes and compares; no
/// Registry outer participates in that at all.
#[tokio::test]
async fn hot_refuses_a_seal_whose_body_was_altered_after_it_was_written() {
    for offset in [
        CAPABILITY_SEAL_MAGIC_OFFSET_V1,
        CAPABILITY_SEAL_VERDICTS_OFFSET_V1,
        CAPABILITY_SEAL_ACTION_OFFSET_V1,
        CAPABILITY_SEAL_DESCRIPTOR_DIGEST_OFFSET_V1,
        CAPABILITY_SEAL_TRADING_RELEASE_OFFSET_V1,
        CAPABILITY_SEAL_REGISTRY_OFFSET_V1,
        CAPABILITY_SEAL_HEADER_BYTES_V1 + CAPABILITY_SEAL_ROW_RAW_OFFSET_V1,
        CAPABILITY_SEAL_HEADER_BYTES_V1
            + 2 * CAPABILITY_SEAL_ROW_BYTES_V1
            + CAPABILITY_SEAL_ROW_DIGEST_OFFSET_V1,
    ] {
        let artifacts = elves();
        let mut test = program_test_without_forced_budget(&artifacts);
        let releases = add_release_waist(&mut test, &artifacts);
        let direct = direct_case(&mut test, releases, &artifacts, false);
        let seal = direct.chain.capability_seal;
        let instructions = direct_top_level_instructions(&direct);
        let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
        add_lookup_table(&mut test, &addresses);
        let mut context = start_with_substrate(test, fixture_substrate()).await;
        // The alteration is made THROUGH THE BANK, and this is not a style
        // choice. It used to be made against `direct.chain.accounts` after
        // `direct_case` returned -- and `direct_case` installs those accounts
        // into the `ProgramTest` before it returns, so the flipped byte never
        // reached the chain and every iteration of this loop submitted the
        // canonical, unaltered seal. The assertion still passed, because for
        // the whole of the heap-wall era every submission of this bundle
        // refused on the heap before it read a seal at all. W2p took the wall
        // down and the vacuity surfaced as this test's first real failure.
        let mut account = maybe_account(&mut context, seal)
            .await
            .expect("the seal fixture is installed");
        let byte = account.data.get_mut(offset).expect("seal byte");
        *byte ^= 0xff;
        context.set_account(&seal, &AccountSharedData::from(account));
        let refusal = submit_v0(
            &mut context,
            &instructions,
            addresses,
            Some(&direct.payer),
            &[],
        )
        .await
        .expect_err(&format!(
            "hot accepted a seal whose byte {offset} was altered"
        ));
        // Every offset is refused by Trading, inside the seal authentication
        // itself, and carries the same named code: the seal is one authenticated
        // object, so an altered magic, an altered verdict word and an altered row
        // digest are all "this is not the seal for this closure". Requiring the
        // code and the depth is what would catch an offset that starts being
        // caught somewhere else -- by the Hot frame parse, before the seal is
        // read at all, say -- which would mean this test had stopped exercising
        // the seal.
        assert_refusal(&refusal, TRADING_CONTENT_REFUSAL_CODE);
        assert!(
            refusal.invoked(TRADING_PROGRAM_ID),
            "the altered seal at byte {offset} never reached Trading: {:#?}",
            refusal.logs
        );
    }
}

// ---------------------------------------------------------------------------
// The ninth wall's tripwire (decision 0017 §7, the ratification condition)
// ---------------------------------------------------------------------------

/// One `Program <id> invoke [<depth>]` line, as the runtime writes it.
fn invoked_depths(logs: &[String], program: Pubkey) -> Vec<usize> {
    let prefix = format!("Program {program} invoke [");
    logs.iter()
        .filter_map(|line| line.strip_prefix(&prefix))
        .filter_map(|tail| tail.strip_suffix(']'))
        .filter_map(|depth| depth.parse().ok())
        .collect()
}

/// The depth at which a Registry CPI is reentrancy and the runtime refuses it.
///
/// The Registry enters at one. A child at three has the Registry already on the
/// stack, so `RegistryInstructionV1::Reauthenticate` from there is not slow, it
/// is `ReentrancyNotAllowed` -- the whole transaction, unconditionally.
const REENTRANT_CHILD_DEPTH: usize = 3;

/// Every child family this fixture can reach runs at the depth that would
/// refuse a Registry CPI, and the transaction succeeds anyway.
///
/// # What this is for
///
/// Decision 0017 ratified "children read the activation cache instead of
/// invoking the Registry", and §7 asked for one piece of implementation with
/// it, because **the rule is enforced by deletion and nothing refuses a
/// contributor who re-adds the import**:
///
/// > A future contributor who re-adds the import gets a route that works in
/// > every test that does not run under a continuation and fails on the one
/// > that does. If ratification comes with a single piece of implementation,
/// > make it that: a test that exercises a child under a real continuation for
/// > each family, so the wall has a tripwire and not only a comment.
///
/// This is that test for the families this fixture reaches. Claims and Custody
/// are OBSERVED at depth three -- not assumed from the account frame, read out
/// of the runtime's own invoke log -- and the execution is required to succeed.
/// Re-add a Registry CPI to either program and this goes red immediately, on
/// `ReentrancyNotAllowed`, naming the family.
///
/// `real_registry_executes_profile14_direct_hot_under_protocol_limit` has been
/// executing this same shape all along. What it never did was SAY so, so a
/// contributor who broke the wall would have met a red test about token
/// balances and a Custody replay revision. The wall deserves a red that says
/// the wall.
///
/// # It has been seen to fire
///
/// Not asserted -- run. A Registry `invoke` was put back into Claims'
/// `sparse_native_transfer_v1::authenticate_releases`, the Claims ELF was
/// rebuilt, and this case failed with
/// `InstructionError(2, ReentrancyNotAllowed)` and the runtime log
/// *"Cross-program invocation reentrancy not allowed for this instruction"*
/// against Claims, Trading and the Registry in turn. The source change was
/// reverted; the evidence is that the red exists and says the right thing.
///
/// # And the first attempt at that control did NOT fire, which is the point
///
/// The same `invoke` placed in `lib.rs::authenticate_activated_role` -- the
/// helper THIRTEEN Claims sites share -- left this case GREEN, because the route
/// this fixture drives is `sparse_native_transfer_v1`, which takes the
/// bump-witness API instead and never touches that helper. So the dynamic half
/// of the tripwire covers one route of Claims, measured, not "Claims". That is
/// exactly the gap `assert_no_family_reaches_the_registry_by_cpi` exists to
/// close, and it is why both halves are here.
///
/// # The three families this does NOT cover, stated rather than implied
///
/// 0017 §5 lists five converted families: claims, custody, core, dealer and
/// rent. This fixture reaches two of them as children. Measured, by reading the
/// runtime's invoke log on this very execution: Registry at [1], Trading at [2],
/// Claims at [3], Custody at [3] -- and **Core is never invoked at all** on the
/// Direct Hot route, nor are Dealer or Rent, which have no continuation fixture
/// anywhere in the tree.
///
/// So the dynamic half of the tripwire is two families of five, and the other
/// three are named in this lane's report as unbuilt rather than left to be
/// assumed covered. `assert_no_family_reaches_the_registry_by_cpi` below is the
/// structural half, which does cover all five, and covers every route of each
/// rather than the one this fixture drives.
#[tokio::test]
async fn claims_and_custody_execute_as_children_under_a_real_continuation() {
    let artifacts = elves();
    let mut test = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case(&mut test, releases, &artifacts, false);
    let instructions = direct_registry_instructions(releases, &direct);
    let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;
    let execution = submit_v0_observed(
        &mut context,
        &instructions,
        addresses,
        Some(&direct.payer),
        &[],
    )
    .await
    .expect(
        "a child under a real Registry continuation must EXECUTE. If this is \
         ReentrancyNotAllowed, some role program has re-acquired a \
         RegistryInstructionV1::Reauthenticate CPI and decision 0017's wall is \
         down -- read the invoke log below for which family died at depth three.",
    );

    // The Registry really is on the stack, at the depth that makes the CPI
    // illegal. Without this the case below could pass on a transaction that
    // never entered through the continuation at all.
    assert_eq!(
        invoked_depths(&execution.logs, REGISTRY_PROGRAM_ID),
        vec![1],
        "this must be a REAL continuation: the Registry enters at depth one \
         exactly once. Logs: {:#?}",
        execution.logs,
    );
    assert_eq!(
        invoked_depths(&execution.logs, TRADING_PROGRAM_ID),
        vec![2],
        "Trading must run as the continuation's child, not top-level",
    );

    for (family, program) in [
        ("Claims", CLAIMS_PROGRAM_ID),
        ("Custody", CUSTODY_PROGRAM_ID),
    ] {
        let depths = invoked_depths(&execution.logs, program);
        assert!(
            !depths.is_empty(),
            "{family} never executed, so this transaction proves nothing about \
             {family}'s wall. Logs: {:#?}",
            execution.logs,
        );
        assert!(
            depths.iter().all(|depth| *depth >= REENTRANT_CHILD_DEPTH),
            "{family} ran at depths {depths:?}, and a Registry CPI is only \
             reentrancy from {REENTRANT_CHILD_DEPTH} or deeper. This fixture has \
             stopped exercising the wall.",
        );
    }
}

/// No role adapter can construct the Registry instruction the wall forbids.
///
/// The structural half of decision 0017 §7's tripwire, and the half that covers
/// all five converted families rather than the two the continuation fixture
/// reaches. §3 states the enforcement plainly -- *"The illegal call is not
/// refused; it is unwriteable without re-adding an import"* -- so this reads the
/// source for that import and refuses it by name.
///
/// # What each half proves, and neither proves alone
///
/// The dynamic case above proves a child really executes with the Registry on
/// the stack, on the one route its fixture drives. It cannot speak for the other
/// routes of the same family: Claims alone had thirteen release-set read sites.
/// This one speaks for every route of every family and proves something weaker
/// but categorical -- that no code path in a role adapter can name the
/// instruction at all, so none of them can invoke it.
///
/// # Why the Registry and the operator are exempt, and Trading is not
///
/// `RegistryInstructionV1::Reauthenticate` is not deprecated: it is still the
/// Registry's own public route, still dispatched by `process_instruction`, and
/// `dclutch-operator` still submits it top-level as an attestation. What is
/// forbidden is a ROLE ADAPTER invoking it, because a role adapter is what runs
/// under the continuation. Trading is in the list even though its top-level arm
/// was at depth one and legal: decision 0017's option B converted it anyway for
/// 66,921 measured CU, and leaving it exempt would leave the one program with
/// both arms free to reacquire the CPI on the arm that cannot have it.
///
/// Comments and doc comments are allowed to name it -- five of these programs
/// carry a paragraph explaining what was deleted and why, and deleting those
/// paragraphs to satisfy a checker would be exactly the wrong trade.
#[test]
fn assert_no_family_reaches_the_registry_by_cpi() {
    // Six, not seven: `dclutch-dealer-sbf` was deleted 2026-09-02. It was a
    // standalone measurement prototype its own header disclaimed as "not a
    // second accepted Trading release identity", `false` in SHIPPED_LINKS, and
    // its only consumer was its own program-test. The shipped Dealer path is
    // Trading's dealer family through the accelerator, and `dclutch-trading-sbf`
    // below already covers it.
    const ROLE_ADAPTERS: [&str; 6] = [
        "dclutch-claims-sbf",
        "dclutch-custody-sbf",
        "dclutch-core-sbf",
        "dclutch-rent-sbf",
        "dclutch-trading-sbf",
        "dclutch-resolution-proof-sbf",
    ];
    let programs = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("programs")
        .canonicalize()
        .expect("the programs directory");
    let mut offences: Vec<String> = Vec::new();
    let mut scanned = 0_usize;
    for adapter in ROLE_ADAPTERS {
        let source = programs.join(adapter).join("src");
        assert!(
            source.is_dir(),
            "{adapter} has no src/ at {}: this list is stale and the gate is \
             silently covering fewer families than it claims",
            source.display(),
        );
        for file in rust_sources(&source) {
            scanned += 1;
            let text = std::fs::read_to_string(&file).expect("role adapter source");
            for (index, line) in text.lines().enumerate() {
                let code = line.trim_start();
                if code.starts_with("//") {
                    continue;
                }
                if code.contains("RegistryInstructionV1") {
                    offences.push(format!(
                        "{}:{}  {}",
                        file.display(),
                        index.saturating_add(1),
                        code.trim(),
                    ));
                }
            }
        }
    }
    assert!(
        scanned > 40,
        "only {scanned} role-adapter sources were scanned, which cannot be the \
         whole set -- the walker is broken and this gate is vacuous",
    );
    assert!(
        offences.is_empty(),
        "a role adapter names RegistryInstructionV1 in code. Under a Registry \
         continuation the Registry sits at CPI depth one, so invoking it from a \
         child is ReentrancyNotAllowed and costs the whole transaction -- \
         decision 0017, and the wall the five families were converted off. Read \
         the activation cache with dclutch-registry-activation-auth-v1 instead. \
         Offending lines:\n{}",
        offences.join("\n"),
    );
}

/// Claims reads the activation cache in exactly three places, and thirteen of
/// its routes share one of them.
///
/// # The gap this closes, quoted from the case above
///
/// `claims_and_custody_execute_as_children_under_a_real_continuation` records
/// that a Registry `invoke` planted in `lib.rs::authenticate_activated_role` --
/// "the helper THIRTEEN Claims sites share" -- left it GREEN, because the route
/// its fixture drives is `sparse_native_transfer_v1`, which takes the
/// bump-witness API and never touches that helper. CACHEREAD reported the same
/// thing as an honest negative: the dynamic tripwire covers ONE Claims route.
///
/// `assert_no_family_reaches_the_registry_by_cpi` answers half of that. It
/// refuses `RegistryInstructionV1` anywhere in role-adapter code, which covers
/// every route of every family -- but only against the reacquisition that
/// spells the instruction type. It says nothing about WHERE a family reads the
/// cache, so a fourteenth Claims route that authenticated a role by hand would
/// pass it, and the dynamic case would not reach that route either.
///
/// # What this asserts instead, and why it is what makes one route enough
///
/// Not "the wall is up" -- the FUNNEL. Every Claims source that names a role
/// authentication entry point of `dclutch-registry-activation-auth-v1` is one
/// of three, each named here with the reason, and each is pinned to the exact
/// entry points it may name:
///
///   * `lib.rs` may name the plain `authenticate_activated_role_v1`, and it is
///     the ONLY file that may. That is the shared helper, and it is what makes
///     the thirteen release-set read sites literally one function rather than
///     thirteen implementations that happen to agree today.
///   * `sparse_native_transfer_v1.rs` and `founding_v5.rs` may name the
///     bump-witness variants, because they carry a caller-mined bump the plain
///     variant has no parameter for. The first of those is the route the
///     dynamic case above actually drives.
///
/// The count was FOURTEEN when this table was written and is thirteen now, and
/// the reason is worth a line because it is the funnel working rather than
/// drifting: `9c25e741` moved `founding_v5` off the shared helper onto the
/// bump-witness variants, which is exactly the row-2 migration this table
/// permits. Nothing asserts the number -- the gates below are
/// `helper_definitions == 1` and `helper_callers >= 5`, neither of which a
/// legal migration can break -- so the number is prose and prose goes stale.
/// Re-measure it rather than trusting it:
/// `grep -rn 'authenticate_activated_role(' programs/dclutch-claims-sbf/src`
/// minus the one definition line. Counted 2026-08-31: thirteen.
///
/// A fourteenth reader -- a new route that authenticates a role in its own
/// module, or `lib.rs`'s helper quietly switching to the bump-witness API --
/// fails here by name, which is the signal that the dynamic half's one route no
/// longer stands for the rest.
///
/// The table is a two-way ratchet: a row that stops naming its entry point
/// fails too, because a reader list with a dead row has stopped describing the
/// program.
///
/// # What it deliberately does NOT claim
///
/// This is a source scan, so it proves a shape and never an execution. It does
/// not show that the helper is correct, that any route runs at depth three, or
/// that Claims has no other reentrancy. Those are the other two cases in this
/// file, and all three are needed.
#[test]
fn claims_reads_the_activation_cache_only_through_its_three_named_readers() {
    /// Every role-authentication entry point the auth crate exports.
    const ENTRY_POINTS: [&str; 5] = [
        "authenticate_activated_role_v1",
        "authenticate_activated_role_and_bump_v1",
        "authenticate_activated_role_with_bump_v1",
        "authenticate_activated_role_in_frame_v1",
        "authenticate_activated_role_in_cache_v1",
    ];
    /// The Claims sources allowed to name one, which entry points each may
    /// name, and why that file is allowed to.
    const READERS: [(&str, &[&str], &str); 3] = [
        (
            "lib.rs",
            &["authenticate_activated_role_v1"],
            "the shared helper the thirteen Claims release-set read sites call",
        ),
        (
            "sparse_native_transfer_v1.rs",
            &[
                "authenticate_activated_role_and_bump_v1",
                "authenticate_activated_role_with_bump_v1",
            ],
            "the bump-witness route, and the one the dynamic case above drives",
        ),
        (
            "founding_v5.rs",
            &[
                "authenticate_activated_role_and_bump_v1",
                "authenticate_activated_role_with_bump_v1",
            ],
            "the founding leg, which carries bumps mined off chain",
        ),
    ];

    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("programs")
        .join("dclutch-claims-sbf")
        .join("src")
        .canonicalize()
        .expect("the Claims adapter source directory");
    let mut offences: Vec<String> = Vec::new();
    let mut exercised: Vec<(&str, &str)> = Vec::new();
    let mut scanned = 0_usize;
    let mut helper_definitions = 0_usize;
    let mut helper_callers: Vec<String> = Vec::new();
    for file in rust_sources(&source) {
        scanned += 1;
        let name = file
            .file_name()
            .and_then(|value| value.to_str())
            .expect("a Claims source file name")
            .to_owned();
        let text = std::fs::read_to_string(&file).expect("Claims adapter source");
        let mut calls_helper = false;
        for (index, line) in text.lines().enumerate() {
            let code = line.trim_start();
            // A doc comment naming the helper is the explanation of the wall,
            // and deleting those to satisfy a checker is the wrong trade --
            // the same exemption the seven-adapter scan makes.
            if code.starts_with("//") {
                continue;
            }
            if code.contains("fn authenticate_activated_role(") {
                helper_definitions += 1;
            }
            if code.contains("authenticate_activated_role(") {
                calls_helper = true;
            }
            for entry in ENTRY_POINTS {
                if !code.contains(entry) {
                    continue;
                }
                match READERS
                    .iter()
                    .find(|(reader, _, _)| *reader == name.as_str())
                {
                    Some((_, allowed, _)) if allowed.contains(&entry) => {
                        exercised.push((
                            READERS
                                .iter()
                                .find(|(reader, _, _)| *reader == name.as_str())
                                .expect("the matched reader")
                                .0,
                            entry,
                        ));
                    }
                    _ => offences.push(format!(
                        "{}:{}  names {entry}\n      {}",
                        file.display(),
                        index.saturating_add(1),
                        code.trim(),
                    )),
                }
            }
        }
        if calls_helper && name != "lib.rs" {
            helper_callers.push(name);
        }
    }

    assert!(
        scanned > 15,
        "only {scanned} Claims sources were scanned, which cannot be the whole \
         adapter -- the walker is broken and this gate is vacuous",
    );
    assert_eq!(
        helper_definitions, 1,
        "the shared helper must be defined exactly once in the Claims adapter; \
         {helper_definitions} definitions means the thirteen sites are no \
         longer one function and the dynamic tripwire's one route stands for \
         nothing",
    );
    assert!(
        helper_callers.len() >= 5,
        "only {} Claims modules outside lib.rs call the shared helper ({:?}). \
         The funnel this case exists to protect has been dismantled, or the \
         scan stopped seeing it",
        helper_callers.len(),
        helper_callers,
    );
    assert!(
        offences.is_empty(),
        "a Claims source authenticates an activated role outside the three \
         named readers. Every release-set read must go through \
         lib.rs::authenticate_activated_role, whose one code path is what the \
         dynamic case above actually exercises -- a reader added elsewhere is \
         covered by neither half of decision 0017 section 7's tripwire. Add the file \
         to READERS with its reason only if it genuinely needs an entry point \
         the helper cannot give it. Offending lines:\n{}",
        offences.join("\n"),
    );
    for (reader, allowed, reason) in READERS {
        for entry in allowed {
            assert!(
                exercised.contains(&(reader, entry)),
                "{reader} is listed as a cache reader for {entry} ({reason}), \
                 but no longer names it. An allowance that has outlived its \
                 reason describes a program that no longer exists: delete the \
                 entry point from its row",
            );
        }
    }
}

/// Every `.rs` file below a directory.
fn rust_sources(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        for entry in std::fs::read_dir(&directory).expect("readable source directory") {
            let path = entry.expect("directory entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|value| value == "rs") {
                out.push(path);
            }
        }
    }
    out
}
