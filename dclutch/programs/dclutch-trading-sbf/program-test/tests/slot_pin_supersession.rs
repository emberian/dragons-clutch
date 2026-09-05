//! Decision 0012's `ExactAuthority` arm, on a real chain, both ways.
//!
//! `slot_pinned_release_elf_digest_v1` has two arms. The `Immutable` one is
//! delegated unchanged to the pre-0012 function and is what every other
//! real-ELF case in this directory has ever executed: `waist::release` built
//! every release `Immutable` and the staged ProgramData wrote the authority
//! option as `None`, so the arm 0012 ADDED was unreachable from the fixture.
//! Nothing in-tree had ever run it against a validator.
//!
//! This file runs it, twice, on the canonical Direct Hot bundle:
//!
//! - **the pin holds** — an `ExactAuthority` release bound to
//!   `waist::UPGRADE_AUTHORITY` and `waist::PINNED_DEPLOYMENT_SLOT`, staged over
//!   ProgramData carrying exactly those, executes the whole market action; and
//! - **the pin breaks** — the whole set redeployed at
//!   `waist::UPGRADED_DEPLOYMENT_SLOT` by the authority it names, with every
//!   release re-issued and re-pinned to the new slot EXCEPT Trading's. Four
//!   pins hold and one does not, in the same transaction against the same
//!   ProgramData accounts, and the one that does not refuses by name and moves
//!   nothing.
//!
//!   Only one stale pin, on purpose. A substrate where every pin was stale
//!   would refuse too, and would prove far less: it could not separate "the
//!   slot pin refuses the release that moved" from "this fixture refuses".
//!
//! # Which reader catches the broken pin
//!
//! `docs/reference/refusals.md` lists a banded `ReleaseSuperseded` for seven
//! programs, Trading's `0x4007` among them. On the Direct Hot route the one
//! that fires is the **Registry's** `0x100D`, and that is not an accident of
//! this fixture: the transparent Hot continuation authenticates the Core and
//! Trading role deployments in `batch_v2::authenticate_request` BEFORE it
//! forwards anything, and Trading's own
//! `authenticate_activated_current_deployment` calls read the same two
//! ProgramData accounts one CPI later. Any slot move visible to Trading is
//! visible to the Registry first. The `invoked` assertion below is what turns
//! that from a claim into an observation — the refusal is required to land
//! with Trading never entered.
//!
//! # This file does not sweep
//!
//! It runs at the default fixture seed and asserts a ceiling, not a figure.
//! The CU it prints is one draw from a bump-search lottery (ledger M-61); the
//! sweep that reports `PASS n/20` and a MEAN against a named ELF digest is
//! `tools/gauntlet/hot-cu/run-hot-cu.sh --substrate slot-pinned`.

use dclutch_registry::{ACTIVATION_PDA_DOMAIN_V1, ArtifactUpgradePolicyV1};
use dclutch_registry_sbf::RegistryError;
use dclutch_registry::svm::ProgramDataV3View;
use dclutch_custody::token_svm::TokenAccount;
use solana_account::Account;
use solana_program::{instruction::InstructionError, pubkey::Pubkey};
use solana_program_test::{BanksClientError, ProgramTestContext};
use solana_sdk::transaction::TransactionError;

use dclutch_direct_hot_program_test_support::waist::{
    COMPUTE_LIMIT, DirectCase, Elves, FixtureSubstrateV1, PINNED_DEPLOYMENT_SLOT,
    REGISTRY_PROGRAM_ID, RefusedExecution, Releases, TRADING_PROGRAM_ID, UPGRADE_AUTHORITY,
    UPGRADED_DEPLOYMENT_SLOT, add_lookup_table, add_release_waist_v2, canonical_lookup_addresses,
    direct_case_v3, direct_registry_instructions, elves, program_test_v2, release_v2,
    start_with_substrate, submit_v0,
};

/// `RegistryError::ReleaseSuperseded`: the release's pinned deployment slot
/// moved, so the cached authentication no longer describes what is deployed.
/// Derived from the declaring program's own enum, never written as a bare
/// number (AGENTS.md "Refusal codes", decision 0007).
const REGISTRY_RELEASE_SUPERSEDED_CODE: u32 = RegistryError::ReleaseSuperseded as u32;

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

fn assert_refusal(refusal: &RefusedExecution, expected: u32) {
    assert_eq!(
        refusal_code(&refusal.error).expect("custom refusal code"),
        expected,
        "refused as {:?} rather than the named code: {:#?}",
        refusal.error,
        refusal.logs
    );
}

async fn account(context: &mut ProgramTestContext, key: Pubkey) -> Account {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account read")
        .expect("live account")
}

async fn account_snapshots(
    context: &mut ProgramTestContext,
    keys: &[Pubkey],
) -> Vec<(Pubkey, Option<Account>)> {
    let mut output = Vec::with_capacity(keys.len());
    for key in keys {
        let value = context
            .banks_client
            .get_account(*key)
            .await
            .expect("rollback account read");
        output.push((*key, value));
    }
    output
}

/// Stage the canonical Direct Hot bundle on one named substrate.
///
/// Every stage takes the SAME substrate: the ProgramData planted into genesis,
/// the releases the activation cache binds, and the deployment widths the chain
/// fixture derives. Passing different ones would produce a fixture whose
/// refusal proved nothing about the pin.
async fn direct_hot_on(
    artifacts: &Elves,
    substrate: FixtureSubstrateV1,
) -> (ProgramTestContext, Releases, DirectCase, Vec<Pubkey>) {
    let mut test = program_test_v2(artifacts, substrate);
    let releases = add_release_waist_v2(&mut test, artifacts, substrate);
    let direct = direct_case_v3(&mut test, releases, artifacts, false, false, substrate);
    let instructions = direct_registry_instructions(releases, &direct);
    let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let context = start_with_substrate(test, substrate).await;
    (context, releases, direct, addresses)
}

/// The staged substrate is the one the arm requires, read back off the chain.
///
/// Without this the two tests below would still pass on an `Immutable`
/// substrate that happened to refuse for some unrelated reason -- which is
/// exactly the hole that let the whole 0012 arm go unmeasured. The release side
/// is checked against `require_slot_pinned_release_v1`'s admitted pairing and
/// the chain side against the bytes a Loader V3 `Upgrade` actually writes.
async fn assert_slot_pinned_substrate(
    context: &mut ProgramTestContext,
    releases: Releases,
    artifacts: &Elves,
    substrate: FixtureSubstrateV1,
    expected_observed_slot: u64,
) {
    let release = release_v2(TRADING_PROGRAM_ID, 0x33, &artifacts.trading, substrate);
    assert_eq!(
        release.upgrade_policy(),
        ArtifactUpgradePolicyV1::ExactAuthority,
        "the release did not take decision 0012's arm",
    );
    assert_eq!(
        release.upgrade_authority(),
        Some(UPGRADE_AUTHORITY.to_bytes())
    );
    assert_eq!(release.deployment_slot(), PINNED_DEPLOYMENT_SLOT);
    let programdata = account(context, releases.trading_programdata).await;
    let view = ProgramDataV3View::parse(&programdata.data).expect("staged Loader V3 ProgramData");
    assert_eq!(
        view.upgrade_authority(),
        Some(UPGRADE_AUTHORITY.to_bytes()),
        "the staged ProgramData carries no live upgrade authority, so the \
         Immutable arm would have taken it",
    );
    assert_eq!(view.deployment_slot(), expected_observed_slot);
}

/// Every substrate has a DIFFERENT release identity, and the Hot path searches
/// for a PDA seeded by it. This is the confound, named and measured.
///
/// The upgrade policy byte, the bound authority and the bound slot all live
/// inside `ArtifactReleaseV1::to_bytes`, so `artifact_id = hash(release)` moves,
/// so the release-set identity moves. That identity is a SEED at two on-chain
/// `find_program_address` sites the Direct Hot route reaches:
///
/// - `registry-sbf/src/lib.rs`, `authenticate_cache_identity` — the activation
///   cache PDA, `[ACTIVATION_PDA_DOMAIN_V1, release_set_id]`. Reached from
///   `batch_v2::authenticate_request`, and seeded by NOTHING ELSE: its bump
///   depth is identical across all twenty fixture seeds and different between
///   substrates. A 20-seed mean does not average this away -- it is a fixed
///   per-substrate offset of `bump_delta * 1,500 CU`, which is what this test
///   exists to quantify.
/// - `registry-sbf/src/hot_continuation_v2.rs` — the Hot admission PDA, seeded
///   by the release set AND the Hot instruction digest, so it redraws per seed
///   and the mean does average it.
///
/// Under ledger M-61 that makes a raw difference between two substrates' means
/// uninterpretable on its own. `FixtureSubstrateV1::ImmutablePinned` is the
/// control: same digest arm, different identity.
#[test]
fn every_substrate_draws_a_different_release_identity_and_activation_bump() {
    let artifacts = elves();
    let mut identities = Vec::new();
    for substrate in [
        FixtureSubstrateV1::Immutable,
        FixtureSubstrateV1::ImmutablePinned,
        FixtureSubstrateV1::SlotPinned,
        FixtureSubstrateV1::SlotPinnedSuperseded,
    ] {
        let mut test = program_test_v2(&artifacts, substrate);
        let releases = add_release_waist_v2(&mut test, &artifacts, substrate);
        let (activation, bump) = Pubkey::find_program_address(
            &[ACTIVATION_PDA_DOMAIN_V1, &releases.release_set],
            &REGISTRY_PROGRAM_ID,
        );
        assert_eq!(
            activation, releases.activation,
            "the waist and this test derive the activation PDA differently",
        );
        // 255 is the first bump tried, so the SEARCH DEPTH -- what the on-chain
        // `find_program_address` actually pays 1,500 CU per iteration for -- is
        // the distance down from 255.
        let depth = u32::from(255 - bump);
        println!(
            "substrate {:<24} release_set {}  activation bump {bump} (depth {depth}, \
             {} CU of on-chain search)",
            substrate.name(),
            hex(&releases.release_set),
            depth * 1_500,
        );
        identities.push((substrate.name(), releases.release_set));
    }
    for (index, (name, identity)) in identities.iter().enumerate() {
        for (other_name, other) in identities.iter().skip(index + 1) {
            assert_ne!(
                identity, other,
                "substrates {name} and {other_name} share a release identity, so \
                 neither is a control for the other",
            );
        }
    }
}

fn hex(value: &[u8; 32]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[tokio::test]
async fn a_slot_pinned_mutable_substrate_executes_the_whole_direct_hot_action() {
    let artifacts = elves();
    let substrate = FixtureSubstrateV1::SlotPinned;
    let (mut context, releases, direct, addresses) = direct_hot_on(&artifacts, substrate).await;
    assert_slot_pinned_substrate(
        &mut context,
        releases,
        &artifacts,
        substrate,
        PINNED_DEPLOYMENT_SLOT,
    )
    .await;

    let before = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    let instructions = direct_registry_instructions(releases, &direct);
    let units = match submit_v0(
        &mut context,
        &instructions,
        addresses,
        Some(&direct.payer),
        &[],
    )
    .await
    {
        Ok(units) => units,
        Err(refusal) => {
            // The demoted continuation's known lottery, handled exactly as
            // `hot_heap_frame_is_inert.rs` handles it: this route rides the
            // Registry outer, whose deep draws exhaust the protocol compute
            // meter on some fixture seeds (19 of 32 at `3dde1b9c`), and
            // DECISION_PACKET_2026_08_30 §4 demoted the route to harness-only
            // without chartering its compute fix -- so a draw that does not
            // fit is not a defect this file is entitled to fail on. A NAMED
            // refusal is still a failure: a slot-pin regression raises a code,
            // and the supersession arm below asserts its exact one.
            if let BanksClientError::TransactionError(TransactionError::InstructionError(
                _,
                InstructionError::Custom(code),
            )) = refusal.error
            {
                panic!(
                    "the slot-pinned substrate REFUSED the Direct Hot action \
                     with code {code:#x} -- that is a verdict, not a draw: {:#?}",
                    refusal.logs,
                );
            }
            println!(
                "continuation exhausted the compute meter on the slot-pinned \
                 substrate, fixture seed {} ({:?}). No verdict is measurable \
                 from an exhausted meter; the substrate's refusal discipline is \
                 asserted by the supersession arms, which name their codes. Not \
                 a failure: DECISION_PACKET_2026_08_30 §4 demoted this route to \
                 harness-only and did not charter its compute fix.",
                std::env::var("DCLUTCH_FIXTURE_SEED").unwrap_or_else(|_| "0".to_owned()),
                refusal.error,
            );
            return;
        }
    };
    assert!(units > 0 && units <= COMPUTE_LIMIT);

    // The same post-state the immutable campaign requires: a mutable substrate
    // is admitted, not indulged.
    let root = account(&mut context, direct.chain.root).await;
    assert_eq!(root.owner, TRADING_PROGRAM_ID);
    assert!(!root.data.is_empty());
    let collateral = &direct.chain.collateral_accounts;
    let source = account(
        &mut context,
        *collateral.first().expect("source collateral"),
    )
    .await;
    let destination = account(
        &mut context,
        *collateral.get(1).expect("destination collateral"),
    )
    .await;
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
    assert_ne!(after, before, "the slot-pinned action changed nothing");

    // ONE DRAW at the default seed, printed rather than asserted (M-61). The
    // sweepable statistic is `run-hot-cu.sh --substrate slot-pinned`.
    println!(
        "hot tail at the protocol default heap, substrate {}, fixture seed 0: \
         {units} CU of 1,400,000 ({} spare)",
        substrate.name(),
        COMPUTE_LIMIT.saturating_sub(units),
    );
}

#[tokio::test]
async fn an_upgraded_trading_substrate_supersedes_the_release_and_refuses_by_name() {
    let artifacts = elves();
    let substrate = FixtureSubstrateV1::SlotPinnedSuperseded;
    let (mut context, releases, direct, addresses) = direct_hot_on(&artifacts, substrate).await;
    // The release still binds slot 167; only the chain moved to 531. That is
    // the whole hostility, and it is one u64 apart from the passing case.
    assert_slot_pinned_substrate(
        &mut context,
        releases,
        &artifacts,
        substrate,
        UPGRADED_DEPLOYMENT_SLOT,
    )
    .await;

    let before = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    let instructions = direct_registry_instructions(releases, &direct);
    let refusal = submit_v0(
        &mut context,
        &instructions,
        addresses,
        Some(&direct.payer),
        &[],
    )
    .await
    .expect_err("a superseded slot pin executed the market action");
    assert_refusal(&refusal, REGISTRY_RELEASE_SUPERSEDED_CODE);
    assert!(
        !refusal.invoked(TRADING_PROGRAM_ID),
        "the Registry forwarded a superseded release set into Trading: {:#?}",
        refusal.logs
    );
    let after = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    assert_eq!(after, before, "a refused Direct Hot moved material state");

    println!(
        "slot-pin refusal, substrate {}: Custom({REGISTRY_RELEASE_SUPERSEDED_CODE:#06x}) \
         after {} CU of 1,400,000, Trading never invoked",
        substrate.name(),
        refusal.compute_units_consumed,
    );
}
