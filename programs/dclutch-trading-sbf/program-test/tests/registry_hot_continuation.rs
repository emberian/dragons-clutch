//! Real-ELF release-waist evidence for Registry-authenticated Trading Hot.
//!
//! The final campaign executes the canonical Direct fixed-topology bundle at
//! the protocol 1.4M compute ceiling.  This test owns only transaction assembly
//! and observations; Registry and Trading remain the executable authorities.

use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1,
    hot_v3::{HOT_FIXED_ACCOUNT_COUNT_V3, HotExecutionEnvelopeV3},
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
use dclutch_direct_codec::execution_v3::DirectExecutionActionV3;
use dclutch_direct_codec::ordinary_geometry_v3::DirectOrdinaryGeometryV3;
use dclutch_direct_codec::successor::{DirectMakerReplayLayoutV1, DirectRootStateLayoutV1};
use dclutch_registry_sbf::RegistryError;
use dclutch_token_svm::TokenAccount;
use dclutch_trading_sbf::TradingSbfError;
use solana_account::{Account, AccountSharedData};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction, InstructionError},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::Signer;
use solana_sdk::transaction::TransactionError;
use solana_sdk_ids::system_program;

use dclutch_direct_hot_program_test_support::waist::{
    CLAIMS_PROGRAM_ID, COMPUTE_LIMIT, CUSTODY_PROGRAM_ID, DirectCase, Elves, RefusedExecution,
    Releases, TRADING_PROGRAM_ID, add_lookup_table, add_release_waist, canonical_lookup_addresses,
    direct_case, direct_case_v2, direct_case_v4, direct_registry_instructions, elves,
    fixture_substrate, legacy_registry_hot_instruction, program_test, registry_hot_instruction,
    start_with_substrate, submit_v0,
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
/// `TradingSbfError::NativeSignature`: instructions-sysvar or native-signature
/// evidence was absent or not exact.
const TRADING_NATIVE_SIGNATURE_REFUSAL_CODE: u32 = TradingSbfError::NativeSignature as u32;

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
    let child_start = 6;
    *instruction
        .accounts
        .get_mut(child_start)
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
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case(&mut test, releases, &artifacts, false);
    let instructions = direct_registry_instructions(releases, &direct);
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
    .expect("Registry-authenticated Direct Hot execution");
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
#[tokio::test]
async fn a_four_outcome_market_trades_on_the_canonical_artifacts() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
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
    let instructions = direct_registry_instructions(releases, &direct);
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
    .expect("Registry-authenticated Direct Hot execution at four outcomes");
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
async fn trade_at_geometry(artifacts: &Elves, outcomes: u32) -> Result<u64, RefusedExecution> {
    let mut test = program_test(artifacts);
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
    let instructions = direct_registry_instructions(releases, &direct);
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
        match trade_at_geometry(&artifacts, outcomes).await {
            Ok(units) => {
                println!(
                    "geometry: {outcomes} outcomes ({} cuts) traded at {units} CU",
                    outcomes - 2
                );
                assert!(units > 0 && units <= COMPUTE_LIMIT);
            }
            Err(refusal) => {
                a_geometry_that_does_not_fit_ran_out_of_compute(outcomes, &refusal);
                panic!(
                    "a {outcomes}-outcome market ran out of compute; the measured wall was 31 \
                     outcomes and something has made the hot path much more expensive"
                );
            }
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

#[tokio::test]
async fn late_custody_refusal_rolls_back_registry_hot_claims_and_lifecycle() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case(&mut test, releases, &artifacts, true);
    let instructions = direct_registry_instructions(releases, &direct);
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

#[tokio::test]
async fn corrupt_profile14_root_reserved_byte_refuses_without_mutation() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
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
/// same execution advances the root -- which the Registry's Hot coordinate
/// check pins by prestate digest, so a second submission of the same bundle is
/// refused at the boundary before Trading reads any replay at all. Rewinding
/// everything but the replays is what puts a live replay in front of a bundle
/// the Registry will still forward.
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
/// refused by the Registry at the root prestate digest, corrupt maker byte or
/// not, and Trading never runs.
#[tokio::test]
async fn corrupt_live_profile14_maker_reserved_byte_refuses_without_mutation() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case(&mut test, releases, &artifacts, false);
    let instructions = direct_registry_instructions(releases, &direct);
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
    let mut accounts = direct
        .chain
        .hot_instruction
        .accounts
        .get(..HOT_FIXED_ACCOUNT_COUNT_V3)
        .expect("hot fixed prefix")
        .to_vec();
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

async fn submit_seal(
    context: &mut ProgramTestContext,
    direct: &DirectCase,
    instruction: Instruction,
) -> Result<u64, RefusedExecution> {
    let addresses =
        canonical_lookup_addresses(core::slice::from_ref(&instruction), direct.payer.pubkey());
    submit_v0(context, &[instruction], addresses, Some(&direct.payer), &[]).await
}

#[tokio::test]
async fn the_seal_outer_writes_exactly_the_bytes_the_hot_path_expects() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case_v2(&mut test, releases, &artifacts, false, true);
    let descriptor_digest = descriptor_digest(&direct);
    let canonical = seal_instruction(&direct, direct_action(), descriptor_digest);
    let addresses =
        canonical_lookup_addresses(core::slice::from_ref(&canonical), direct.payer.pubkey());
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;

    assert!(
        maybe_account(&mut context, direct.chain.capability_seal)
            .await
            .is_none_or(|value| value.owner == system_program::ID && value.data.is_empty()),
        "the seal PDA is not vacant before the seal outer runs"
    );

    let units = submit_seal(&mut context, &direct, canonical.clone())
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
    let refused = submit_seal(&mut context, &direct, canonical)
        .await
        .expect_err("an existing seal was rewritten");
    assert_refusal(&refused, TRADING_CONTENT_REFUSAL_CODE);
    let after = account(&mut context, direct.chain.capability_seal).await;
    assert_eq!(after.data, sealed.data);
}

#[tokio::test]
async fn a_seal_for_another_action_or_descriptor_never_lands_at_this_address() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case_v2(&mut test, releases, &artifacts, false, true);
    let descriptor_digest = descriptor_digest(&direct);
    let hostile = [
        seal_instruction(&direct, direct_action() ^ 1, descriptor_digest),
        seal_instruction(&direct, direct_action(), [0x5a; 32]),
    ];
    let addresses = canonical_lookup_addresses(&hostile, direct.payer.pubkey());
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;

    // The control is `the_seal_outer_writes_exactly_the_bytes_the_hot_path_expects`:
    // the same fixture and the same instruction builder, differing only in the
    // action and the descriptor digest, and it executes. So `Content` here is
    // the coordinates being wrong, not the seal outer refusing everything.
    for instruction in hostile {
        let refused = submit_seal(&mut context, &direct, instruction)
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
/// `real_registry_executes_profile14_direct_hot_under_protocol_limit`: the same
/// bundle with the fixture's own seal installed, and it executes.
#[tokio::test]
async fn hot_refuses_a_missing_seal_and_a_seal_written_for_another_release() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case_v2(&mut test, releases, &artifacts, false, true);
    let instructions = direct_registry_instructions(releases, &direct);
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
        let mut test = program_test(&artifacts);
        let releases = add_release_waist(&mut test, &artifacts);
        let direct = direct_case(&mut test, releases, &artifacts, false);
        let seal = direct.chain.capability_seal;
        let instructions = direct_registry_instructions(releases, &direct);
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
        // caught somewhere else -- by the Registry's own frame checks, say, which
        // would mean this test had stopped exercising the seal at all.
        assert_refusal(&refusal, TRADING_CONTENT_REFUSAL_CODE);
        assert!(
            refusal.invoked(TRADING_PROGRAM_ID),
            "the altered seal at byte {offset} never reached Trading: {:#?}",
            refusal.logs
        );
    }
}
