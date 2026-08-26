//! Real-ELF release-waist evidence for Registry-authenticated Trading Hot.
//!
//! The final campaign executes the canonical Direct fixed-topology bundle at
//! the protocol 1.4M compute ceiling.  This test owns only transaction assembly
//! and observations; Registry and Trading remain the executable authorities.

use std::{env, fs, path::PathBuf};

use dclutch_capability_program_contract::hot_v3::{
    HOT_ACTIVATION_CACHE_ACCOUNT_V3, HOT_CORE_PROGRAM_ACCOUNT_V3, HOT_CORE_PROGRAMDATA_ACCOUNT_V3,
    HOT_FIXED_ACCOUNT_COUNT_V3, HOT_REGISTRY_PROGRAM_ACCOUNT_V3, HOT_RENT_SYSVAR_ACCOUNT_V3,
    HOT_TRADING_PROGRAM_ACCOUNT_V3, HOT_TRADING_PROGRAMDATA_ACCOUNT_V3,
};
use dclutch_core_contract::ContentId;
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetV1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, activate_execution_role_into_v1,
    initialize_activation_cache_v1,
};
use dclutch_registry_svm::continuation_v1::{
    REGISTRY_CONTINUATION_REQUEST_BYTES_V1, RegistryContinuationAdmissionSeedsV1,
    RegistryContinuationRequestV1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1,
    ProgramIdentityV1,
};
use solana_account::{Account, AccountSharedData};
use solana_address_lookup_table_interface::state::{AddressLookupTable, LookupTableMeta};
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::Signer;
use solana_sdk_ids::{bpf_loader_upgradeable, sysvar};
use solana_transaction::versioned::VersionedTransaction;

const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x91; 32]);
const TRADING_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x92; 32]);
const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x93; 32]);
const CLAIMS_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x94; 32]);
const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x95; 32]);
const LOOKUP_TABLE: Pubkey = Pubkey::new_from_array([0x96; 32]);
const COMPUTE_LIMIT: u64 = 1_400_000;

struct Elves {
    registry: Vec<u8>,
    trading: Vec<u8>,
    core: Vec<u8>,
    claims: Vec<u8>,
    custody: Vec<u8>,
}

#[derive(Clone, Copy)]
struct Releases {
    release_set: [u8; 32],
    activation: Pubkey,
    activation_digest: [u8; 32],
    core_programdata: Pubkey,
    trading_programdata: Pubkey,
}

fn content(value: [u8; 32]) -> ContentId {
    ContentId::new(value).expect("nonzero content identity")
}

fn program_identity(value: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(value.to_bytes()).expect("nonzero program identity")
}

fn programdata(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn elves() -> Elves {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    let read = |name: &str| fs::read(directory.join(name)).expect("required real ELF");
    Elves {
        registry: read("dclutch_registry_sbf.so"),
        trading: read("dclutch_trading_sbf.so"),
        core: read("dclutch_core_sbf.so"),
        claims: read("dclutch_claims_sbf.so"),
        custody: read("dclutch_custody_sbf.so"),
    }
}

fn immutable_programdata(elf: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 45 + elf.len()];
    bytes
        .get_mut(..4)
        .expect("loader variant")
        .copy_from_slice(&3_u32.to_le_bytes());
    bytes
        .get_mut(4..12)
        .expect("deployment slot")
        .copy_from_slice(&0_u64.to_le_bytes());
    *bytes.get_mut(12).expect("authority option") = 0;
    bytes.get_mut(45..).expect("ELF tail").copy_from_slice(elf);
    bytes
}

fn add_program(test: &mut ProgramTest, name: &'static str, program: Pubkey, elf: &[u8]) {
    test.add_upgradeable_program_to_genesis(name, &program);
    let bytes = immutable_programdata(elf);
    test.add_account(
        programdata(program),
        Account {
            lamports: Rent::default().minimum_balance(bytes.len()),
            data: bytes,
            owner: bpf_loader_upgradeable::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn release(program: Pubkey, semantic: u8, elf: &[u8]) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        program_identity(program),
        program_identity(bpf_loader_upgradeable::ID),
        programdata(program).to_bytes(),
        content([semantic; 32]),
        hash(elf).to_bytes(),
        0,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("immutable artifact release")
}

fn artifact_id(value: ArtifactReleaseV1) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(hash(&value.to_bytes()).to_bytes()).expect("artifact identity")
}

fn binding(value: ArtifactReleaseV1) -> ExecutionRoleBindingV1 {
    ExecutionRoleBindingV1::new(value.program(), artifact_id(value))
}

fn activation_input(value: ArtifactReleaseV1) -> ArtifactActivationInputV1 {
    ArtifactActivationInputV1::new(
        artifact_id(value),
        value,
        DeploymentObservationV1::new(
            value.program().to_bytes(),
            bpf_loader_upgradeable::ID.to_bytes(),
            true,
            value.programdata(),
            bpf_loader_upgradeable::ID.to_bytes(),
            false,
            value.programdata(),
            bpf_loader_upgradeable::ID.to_bytes(),
            value.deployment_slot(),
            value.elf_digest(),
            value.upgrade_authority(),
        )
        .expect("current immutable deployment observation"),
    )
}

fn add_release_waist(test: &mut ProgramTest, artifacts: &Elves) -> Releases {
    let core = release(CORE_PROGRAM_ID, 0x31, &artifacts.core);
    let claims = release(CLAIMS_PROGRAM_ID, 0x32, &artifacts.claims);
    let trading = release(TRADING_PROGRAM_ID, 0x33, &artifacts.trading);
    let custody = release(CUSTODY_PROGRAM_ID, 0x34, &artifacts.custody);
    let release_set = ExecutionReleaseSetV1::new(
        binding(core),
        binding(claims),
        binding(trading),
        binding(core),
        binding(custody),
    )
    .expect("Core+Trading release set");
    let release_set_id = hash(&release_set.to_bytes()).to_bytes();
    let release_set_content = content(release_set_id);
    let mut cache = vec![0; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut cache, release_set_content).expect("activation cache");
    for (role, selected) in [
        (ExecutionRoleV1::Core, core),
        (ExecutionRoleV1::Claims, claims),
        (ExecutionRoleV1::Trading, trading),
        (ExecutionRoleV1::Resolution, core),
        (ExecutionRoleV1::Custody, custody),
    ] {
        activate_execution_role_into_v1(
            &mut cache,
            release_set_content,
            &release_set,
            role,
            &activation_input(selected),
        )
        .expect("activate exact role");
    }
    ActivatedExecutionReleaseSetV1::decode(&cache).expect("complete activation cache");
    let activation_digest = hash(&cache).to_bytes();
    let activation = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set_id],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    test.add_account(
        activation,
        Account {
            lamports: Rent::default().minimum_balance(cache.len()),
            data: cache,
            owner: REGISTRY_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    Releases {
        release_set: release_set_id,
        activation,
        activation_digest,
        core_programdata: programdata(CORE_PROGRAM_ID),
        trading_programdata: programdata(TRADING_PROGRAM_ID),
    }
}

fn registry_hot_instruction(releases: Releases, mut hot: Instruction) -> (Instruction, Pubkey) {
    assert_eq!(hot.program_id, TRADING_PROGRAM_ID);
    assert!(hot.accounts.len() >= HOT_FIXED_ACCOUNT_COUNT_V3);
    let cache_digest = content(releases.activation_digest);
    let continuation_digest = content(hash(&hot.data).to_bytes());
    let continuation = RegistryContinuationRequestV1::new_core_trading_hot(
        content(releases.release_set),
        cache_digest,
        continuation_digest,
        u32::try_from(hot.data.len()).expect("Hot width"),
    )
    .expect("Core+Trading Hot continuation");
    let batch = continuation.role_batch_request().expect("role batch");
    let seeds = RegistryContinuationAdmissionSeedsV1::new(
        continuation,
        releases.activation.to_bytes(),
        content(hash(&batch.to_bytes()).to_bytes()),
    )
    .expect("admission seeds");
    let release = seeds.release_set();
    let cache = seeds.activation_cache();
    let batch = seeds.batch_request_digest();
    let mask = seeds.role_mask();
    let role = seeds.continuation_role();
    let digest = seeds.continuation_digest();
    let admission = Pubkey::find_program_address(
        &[
            seeds.domain(),
            release.as_slice(),
            cache.as_slice(),
            batch.as_slice(),
            mask.as_slice(),
            role.as_slice(),
            digest.as_slice(),
        ],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    hot.accounts.insert(
        HOT_FIXED_ACCOUNT_COUNT_V3,
        AccountMeta::new_readonly(admission, false),
    );
    let mut accounts = vec![
        AccountMeta::new_readonly(releases.activation, false),
        AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
        AccountMeta::new_readonly(releases.core_programdata, false),
        AccountMeta::new_readonly(TRADING_PROGRAM_ID, false),
        AccountMeta::new_readonly(releases.trading_programdata, false),
        AccountMeta::new_readonly(admission, false),
    ];
    accounts.extend(hot.accounts);
    let mut data = Vec::with_capacity(REGISTRY_CONTINUATION_REQUEST_BYTES_V1 + hot.data.len());
    data.extend_from_slice(&continuation.to_bytes());
    data.extend_from_slice(&hot.data);
    (
        Instruction {
            program_id: REGISTRY_PROGRAM_ID,
            accounts,
            data,
        },
        admission,
    )
}

fn canonical_lookup_addresses(instructions: &[Instruction], payer: Pubkey) -> Vec<Pubkey> {
    let programs = instructions
        .iter()
        .map(|instruction| instruction.program_id)
        .collect::<Vec<_>>();
    let mut addresses = instructions
        .iter()
        .flat_map(|instruction| &instruction.accounts)
        .filter(|meta| !meta.is_signer && meta.pubkey != payer && !programs.contains(&meta.pubkey))
        .map(|meta| meta.pubkey)
        .collect::<Vec<_>>();
    addresses.sort_unstable_by_key(Pubkey::to_bytes);
    addresses.dedup();
    addresses
}

fn add_lookup_table(test: &mut ProgramTest, addresses: &[Pubkey]) {
    let data = AddressLookupTable {
        meta: LookupTableMeta::default(),
        addresses: addresses.into(),
    }
    .serialize_for_tests()
    .expect("lookup-table bytes");
    test.add_account(
        LOOKUP_TABLE,
        Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner: solana_address_lookup_table_interface::program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

async fn submit_v0(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    addresses: Vec<Pubkey>,
) -> Result<u64, BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let message = VersionedMessage::V0(
        v0::Message::try_compile(
            &context.payer.pubkey(),
            instructions,
            &[AddressLookupTableAccount {
                key: LOOKUP_TABLE,
                addresses,
            }],
            blockhash,
        )
        .expect("canonical v0 message"),
    );
    let wire = 1_usize
        .checked_add(64)
        .and_then(|prefix| prefix.checked_add(message.serialize().len()))
        .expect("v0 wire width");
    assert!(wire <= 1_232, "canonical continuation packet overflow");
    let transaction = VersionedTransaction {
        signatures: vec![context.payer.sign_message(&message.serialize())],
        message,
    };
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await?;
    processed.result?;
    Ok(processed
        .metadata
        .map(|metadata| metadata.compute_units_consumed)
        .unwrap_or_default())
}

fn program_test(artifacts: &Elves) -> ProgramTest {
    let mut test = ProgramTest::default();
    test.set_compute_max_units(COMPUTE_LIMIT);
    add_program(
        &mut test,
        "dclutch_registry_sbf",
        REGISTRY_PROGRAM_ID,
        &artifacts.registry,
    );
    add_program(
        &mut test,
        "dclutch_trading_sbf",
        TRADING_PROGRAM_ID,
        &artifacts.trading,
    );
    add_program(
        &mut test,
        "dclutch_core_sbf",
        CORE_PROGRAM_ID,
        &artifacts.core,
    );
    add_program(
        &mut test,
        "dclutch_claims_sbf",
        CLAIMS_PROGRAM_ID,
        &artifacts.claims,
    );
    add_program(
        &mut test,
        "dclutch_custody_sbf",
        CUSTODY_PROGRAM_ID,
        &artifacts.custody,
    );
    test
}

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
    Instruction {
        program_id: TRADING_PROGRAM_ID,
        accounts,
        // Registry authenticates these exact bytes before the explicit Trading
        // continuation entrypoint interprets them.  The hostile tests below
        // deliberately refuse before that child boundary.
        data: b"DCLTHOT3registry-boundary-fixture".to_vec(),
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
        submit_v0(&mut context, &[instruction], addresses)
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
fn wrapper_preserves_exact_hot_bytes_and_places_one_nested_admission_at_38() {
    let releases = Releases {
        release_set: [0x41; 32],
        activation: Pubkey::new_from_array([0x42; 32]),
        activation_digest: [0x43; 32],
        core_programdata: Pubkey::new_from_array([0x44; 32]),
        trading_programdata: Pubkey::new_from_array([0x45; 32]),
    };
    let hot = registry_boundary_hot(releases);
    let exact_hot_bytes = hot.data.clone();
    let (outer, admission) = registry_hot_instruction(releases, hot);
    let header = RegistryContinuationRequestV1::decode(
        outer
            .data
            .get(..REGISTRY_CONTINUATION_REQUEST_BYTES_V1)
            .expect("continuation header"),
    )
    .expect("typed continuation");
    assert_eq!(header.role_count(), 2);
    assert_eq!(header.continuation_role(), ExecutionRoleV1::Trading);
    assert_eq!(
        outer.data.get(REGISTRY_CONTINUATION_REQUEST_BYTES_V1..),
        Some(exact_hot_bytes.as_slice())
    );
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
async fn real_registry_refuses_reordered_core_and_trading_roles_atomically() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let (mut instruction, _) = registry_hot_instruction(releases, registry_boundary_hot(releases));
    instruction.accounts.swap(1, 3);
    assert_registry_refusal(test, releases, instruction).await;
}

#[tokio::test]
async fn real_registry_refuses_stale_core_programdata_atomically() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let (instruction, _) = registry_hot_instruction(releases, registry_boundary_hot(releases));
    let addresses =
        canonical_lookup_addresses(core::slice::from_ref(&instruction), Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = test.start_with_context().await;
    let before = activation_snapshot(&mut context, releases.activation).await;
    let mut stale = context
        .banks_client
        .get_account(releases.core_programdata)
        .await
        .expect("ProgramData read")
        .expect("Core ProgramData");
    let last = stale.data.last_mut().expect("Core ELF byte");
    *last ^= 1;
    context.set_account(&releases.core_programdata, &AccountSharedData::from(stale));
    assert!(
        submit_v0(&mut context, &[instruction], addresses)
            .await
            .is_err(),
        "stale Core deployment unexpectedly authenticated"
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
