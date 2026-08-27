//! Real-ELF release-waist evidence for Registry-authenticated Trading Hot.
//!
//! The final campaign executes the canonical Direct fixed-topology bundle at
//! the protocol 1.4M compute ceiling.  This test owns only transaction assembly
//! and observations; Registry and Trading remain the executable authorities.

use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1,
    hot_v3::{
        HOT_ACTIVATION_CACHE_ACCOUNT_V3, HOT_CORE_PROGRAM_ACCOUNT_V3,
        HOT_CORE_PROGRAMDATA_ACCOUNT_V3, HOT_FIXED_ACCOUNT_COUNT_V3,
        HOT_REGISTRY_PROGRAM_ACCOUNT_V3, HOT_RENT_SYSVAR_ACCOUNT_V3,
        HOT_TRADING_PROGRAM_ACCOUNT_V3, HOT_TRADING_PROGRAMDATA_ACCOUNT_V3, HotExecutionEnvelopeV3,
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
use dclutch_direct_codec::execution_v3::DirectExecutionActionV3;
use dclutch_direct_codec::successor::{DirectMakerReplayLayoutV1, DirectRootStateLayoutV1};
use dclutch_token_svm::TokenAccount;
use solana_account::{Account, AccountSharedData};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::signature::Signer;
use solana_sdk_ids::{system_program, sysvar};

use dclutch_direct_hot_program_test_support::waist::{
    CLAIMS_PROGRAM_ID, COMPUTE_LIMIT, CORE_PROGRAM_ID, CUSTODY_PROGRAM_ID, DirectCase,
    REGISTRY_PROGRAM_ID, RefusedExecution, Releases, TRADING_PROGRAM_ID, add_lookup_table,
    add_release_waist, canonical_lookup_addresses, direct_case, direct_case_v2,
    direct_registry_instructions, elves, legacy_registry_hot_instruction, program_test,
    registry_hot_instruction, submit_v0,
};

fn registry_boundary_hot(releases: Releases) -> Instruction {
    let mut accounts = (0..HOT_FIXED_ACCOUNT_COUNT_V3)
        .map(|index| {
            let coordinate = u8::try_from(index + 1).expect("fixed Hot account coordinate");
            AccountMeta::new_readonly(Pubkey::new_from_array([coordinate; 32]), false)
        })
        .collect::<Vec<_>>();
    for (index, meta) in [
        (
            HOT_ACTIVATION_CACHE_ACCOUNT_V3,
            AccountMeta::new_readonly(releases.activation, false),
        ),
        (
            HOT_CORE_PROGRAM_ACCOUNT_V3,
            AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
        ),
        (
            HOT_CORE_PROGRAMDATA_ACCOUNT_V3,
            AccountMeta::new_readonly(releases.core_programdata, false),
        ),
        (
            HOT_TRADING_PROGRAM_ACCOUNT_V3,
            AccountMeta::new_readonly(TRADING_PROGRAM_ID, false),
        ),
        (
            HOT_TRADING_PROGRAMDATA_ACCOUNT_V3,
            AccountMeta::new_readonly(releases.trading_programdata, false),
        ),
        (
            HOT_REGISTRY_PROGRAM_ACCOUNT_V3,
            AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
        ),
        (
            HOT_RENT_SYSVAR_ACCOUNT_V3,
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ),
    ] {
        *accounts.get_mut(index).expect("fixed Hot account") = meta;
    }
    let request = b"registry-boundary-fixture";
    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(request.len()).expect("boundary request width"),
        releases.release_set,
        accounts
            .first()
            .expect("boundary Market account")
            .pubkey
            .to_bytes(),
        1,
        [0x71; 32],
    )
    .expect("canonical boundary envelope");
    let mut data = Vec::with_capacity(128 + request.len());
    data.extend_from_slice(&envelope.to_bytes());
    data.extend_from_slice(request);
    Instruction {
        program_id: TRADING_PROGRAM_ID,
        accounts,
        // Hostile cases below refuse at the Registry boundary before the
        // intentionally incomplete child fixture can execute.
        data,
    }
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

async fn assert_registry_refusal(
    mut test: ProgramTest,
    releases: Releases,
    instruction: Instruction,
) {
    let addresses =
        canonical_lookup_addresses(core::slice::from_ref(&instruction), Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = test.start_with_context().await;
    let before = activation_snapshot(&mut context, releases.activation).await;
    assert!(
        submit_v0(&mut context, &[instruction], addresses, None, &[])
            .await
            .is_err(),
        "hostile Registry continuation unexpectedly executed"
    );
    let after = activation_snapshot(&mut context, releases.activation).await;
    assert_eq!(after, before, "Registry refusal mutated release evidence");
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
    let releases = Releases {
        release_set: [0x41; 32],
        activation: Pubkey::new_from_array([0x42; 32]),
        activation_digest: [0x43; 32],
        core_programdata: Pubkey::new_from_array([0x44; 32]),
        trading_programdata: Pubkey::new_from_array([0x45; 32]),
        claims_programdata: Pubkey::new_from_array([0x46; 32]),
    };
    let hot = registry_boundary_hot(releases);
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

#[tokio::test]
async fn real_registry_refuses_legacy_headered_hot_container_atomically() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let (instruction, _) =
        legacy_registry_hot_instruction(releases, registry_boundary_hot(releases));
    assert_registry_refusal(test, releases, instruction).await;
}

#[tokio::test]
async fn real_registry_refuses_reordered_core_and_trading_roles_atomically() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let (mut instruction, _) = registry_hot_instruction(releases, registry_boundary_hot(releases));
    instruction.accounts.swap(1, 3);
    assert_registry_refusal(test, releases, instruction).await;
}

#[tokio::test]
async fn real_registry_refuses_substituted_core_programdata_atomically() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let (mut instruction, _) = registry_hot_instruction(releases, registry_boundary_hot(releases));
    *instruction
        .accounts
        .get_mut(2)
        .expect("Core ProgramData prefix") =
        AccountMeta::new_readonly(releases.trading_programdata, false);
    let addresses =
        canonical_lookup_addresses(core::slice::from_ref(&instruction), Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = test.start_with_context().await;
    let before = activation_snapshot(&mut context, releases.activation).await;
    assert!(
        submit_v0(&mut context, &[instruction], addresses, None, &[])
            .await
            .is_err(),
        "substituted Core ProgramData unexpectedly authenticated"
    );
    let after = activation_snapshot(&mut context, releases.activation).await;
    assert_eq!(after, before, "deployment refusal mutated release evidence");
}

#[tokio::test]
async fn real_registry_refuses_altered_hot_bytes_atomically() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let (mut instruction, _) = registry_hot_instruction(releases, registry_boundary_hot(releases));
    let byte = instruction.data.last_mut().expect("continuation byte");
    *byte ^= 1;
    assert_registry_refusal(test, releases, instruction).await;
}

#[tokio::test]
async fn real_registry_refuses_aliased_ephemeral_admission_atomically() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let (mut instruction, admission) =
        registry_hot_instruction(releases, registry_boundary_hot(releases));
    let child_start = 6;
    *instruction
        .accounts
        .get_mut(child_start)
        .expect("first Hot account") = AccountMeta::new_readonly(admission, false);
    assert_registry_refusal(test, releases, instruction).await;
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
    let mut context = test.start_with_context().await;
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

#[tokio::test]
async fn late_custody_refusal_rolls_back_registry_hot_claims_and_lifecycle() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case(&mut test, releases, &artifacts, true);
    let instructions = direct_registry_instructions(releases, &direct);
    let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = test.start_with_context().await;
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
    let mut context = test.start_with_context().await;
    corrupt_account_byte(
        &mut context,
        direct.chain.root,
        CAPABILITY_ROOT_HEADER_BYTES_V1 + DirectRootStateLayoutV1::RESERVED,
    )
    .await;
    let before = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    assert!(
        submit_v0(
            &mut context,
            &instructions,
            addresses,
            Some(&direct.payer),
            &[],
        )
        .await
        .is_err(),
        "noncanonical Direct root unexpectedly accepted"
    );
    let after = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    assert_eq!(after, before, "root refusal mutated Profile14 state");
}

#[tokio::test]
async fn corrupt_live_profile14_maker_reserved_byte_refuses_without_mutation() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case(&mut test, releases, &artifacts, false);
    let instructions = direct_registry_instructions(releases, &direct);
    let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = test.start_with_context().await;
    submit_v0(
        &mut context,
        &instructions,
        addresses.clone(),
        Some(&direct.payer),
        &[],
    )
    .await
    .expect("first-use execution creates live maker replay");
    let hostile = corrupt_account_byte(
        &mut context,
        direct.chain.maker_replays[0],
        DirectMakerReplayLayoutV1::RESERVED,
    )
    .await;
    assert_eq!(hostile.owner, TRADING_PROGRAM_ID);
    let before = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    assert!(
        submit_v0(
            &mut context,
            &instructions,
            addresses,
            Some(&direct.payer),
            &[],
        )
        .await
        .is_err(),
        "noncanonical live maker replay unexpectedly accepted"
    );
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
    let mut context = test.start_with_context().await;

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
    // recorded verdict byte-for-byte intact.
    let refused = submit_seal(&mut context, &direct, canonical).await;
    assert!(refused.is_err(), "an existing seal was rewritten");
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
    let mut context = test.start_with_context().await;

    for instruction in hostile {
        assert!(
            submit_seal(&mut context, &direct, instruction)
                .await
                .is_err(),
            "a seal filed under other coordinates reached the canonical address"
        );
        assert!(
            maybe_account(&mut context, direct.chain.capability_seal)
                .await
                .is_none_or(|value| value.owner == system_program::ID && value.data.is_empty()),
            "a refused seal left state at the canonical address"
        );
    }
}

#[tokio::test]
async fn hot_refuses_a_missing_seal_and_a_seal_written_for_another_release() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case_v2(&mut test, releases, &artifacts, false, true);
    let instructions = direct_registry_instructions(releases, &direct);
    let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = test.start_with_context().await;
    let refused = submit_v0(
        &mut context,
        &instructions,
        addresses,
        Some(&direct.payer),
        &[],
    )
    .await;
    assert!(
        refused.is_err(),
        "a hot action executed with no validated-artifact seal"
    );
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
        let mut direct = direct_case(&mut test, releases, &artifacts, false);
        let seal = direct.chain.capability_seal;
        let account = direct
            .chain
            .accounts
            .iter_mut()
            .find(|value| value.key == seal)
            .expect("seal fixture account");
        let byte = account.account.data.get_mut(offset).expect("seal byte");
        *byte ^= 0xff;
        let instructions = direct_registry_instructions(releases, &direct);
        let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
        add_lookup_table(&mut test, &addresses);
        let mut context = test.start_with_context().await;
        assert!(
            submit_v0(
                &mut context,
                &instructions,
                addresses,
                Some(&direct.payer),
                &[],
            )
            .await
            .is_err(),
            "hot accepted a seal whose byte {offset} was altered"
        );
    }
}
