//! Real-ELF one-time Core infrastructure-root initialization.

use std::{env, fs, path::PathBuf, vec, vec::Vec};

use dclutch_core_contract::ContentId;
use dclutch_core_sbf::CoreSbfError;
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry::{
    ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactReleaseV1, ArtifactUpgradePolicyV1,
};
use dclutch_registry::release_set::{
    INITIALIZE_PROTOCOL_INFRASTRUCTURE_BYTES_V1, InitializeProtocolInfrastructureV1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1, PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2,
    ProgramIdentityV1, ProtocolInfrastructureProfileV1, ProtocolInfrastructureProfileV2,
};
use solana_account::Account;
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::{instruction::InstructionError, transaction::TransactionError};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_transaction::Transaction;

const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xe1; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xe2; 32]);
const RENT_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xe3; 32]);
const PROTOCOL_COMPUTE_UNIT_LIMIT: u32 = 1_400_000;
/// Index of the initialization instruction behind the budget request.
///
/// The request is load-bearing, not decoration: initialization costs ~248,000
/// compute units against a 200,000 default, so a transaction submitted
/// without it exhausts the budget before reaching any conjunct, and every
/// refusal a test could name would be `Computational budget exceeded`.
const INITIALIZE_INSTRUCTION_INDEX: u8 = 1;

/// Assert the compiled program refused by NAME, not by a number typed by hand.
#[track_caller]
fn refused(result: Result<(), BanksClientError>, expected: CoreSbfError) {
    let error = match result.expect_err("this transaction must be refused") {
        BanksClientError::TransactionError(error) => error,
        BanksClientError::SimulationError { err, .. } => err,
        other => panic!("unexpected banks error: {other:?}"),
    };
    assert_eq!(
        error,
        TransactionError::InstructionError(
            INITIALIZE_INSTRUCTION_INDEX,
            InstructionError::Custom(expected as u32),
        ),
        "expected {expected:?}"
    );
}

struct Artifacts {
    core: Vec<u8>,
    registry: Vec<u8>,
    rent: Vec<u8>,
}

struct Fixture {
    test: ProgramTest,
    authority: Keypair,
    profile: Pubkey,
    genesis_profile: Pubkey,
    core_programdata: Pubkey,
    registry_programdata: Pubkey,
    rent_programdata: Pubkey,
    registry_raw: Pubkey,
    registry_staging: Pubkey,
    rent_raw: Pubkey,
    rent_staging: Pubkey,
    expected: ProtocolInfrastructureProfileV1,
    expected_genesis: ProtocolInfrastructureProfileV2,
}

fn artifacts() -> Artifacts {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    Artifacts {
        core: fs::read(directory.join("dclutch_core_sbf.so")).expect("Core ELF"),
        registry: fs::read(directory.join("dclutch_registry_sbf.so")).expect("Registry ELF"),
        rent: fs::read(directory.join("dclutch_rent_sbf.so")).expect("Rent ELF"),
    }
}

fn programdata_address(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn programdata_bytes(elf: &[u8], authority: Option<Pubkey>) -> Vec<u8> {
    let mut bytes = vec![0; 45 + elf.len()];
    bytes
        .get_mut(0..4)
        .expect("tag")
        .copy_from_slice(&3_u32.to_le_bytes());
    bytes
        .get_mut(4..12)
        .expect("slot")
        .copy_from_slice(&0_u64.to_le_bytes());
    match authority {
        Some(authority) => {
            *bytes.get_mut(12).expect("authority tag") = 1;
            bytes
                .get_mut(13..45)
                .expect("authority")
                .copy_from_slice(authority.as_ref());
        }
        None => *bytes.get_mut(12).expect("authority tag") = 0,
    }
    bytes.get_mut(45..).expect("ELF").copy_from_slice(elf);
    bytes
}

fn add_program(
    test: &mut ProgramTest,
    name: &'static str,
    program: Pubkey,
    elf: &[u8],
    authority: Option<Pubkey>,
) {
    test.add_upgradeable_program_to_genesis(name, &program);
    let data = programdata_bytes(elf, authority);
    test.add_account(
        programdata_address(program),
        Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner: bpf_loader_upgradeable::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn identity(program: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(program.to_bytes()).expect("program")
}

fn artifact_release(
    program: Pubkey,
    elf: &[u8],
    semantic: u8,
    upgrade_authority: Option<Pubkey>,
) -> ArtifactReleaseV1 {
    let policy = if upgrade_authority.is_some() {
        ArtifactUpgradePolicyV1::ExactAuthority
    } else {
        ArtifactUpgradePolicyV1::Immutable
    };
    ArtifactReleaseV1::new(
        identity(program),
        identity(bpf_loader_upgradeable::ID),
        programdata_address(program).to_bytes(),
        ContentId::new([semantic; 32]).expect("semantic release"),
        hash(elf).to_bytes(),
        0,
        policy,
        upgrade_authority.map(|authority| authority.to_bytes()),
    )
    .expect("artifact")
}

fn add_artifact_record(test: &mut ProgramTest, release: ArtifactReleaseV1) -> (Pubkey, Pubkey) {
    let data = release.to_bytes().to_vec();
    let digest = hash(&data).to_bytes();
    let raw = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &ARTIFACT_RELEASE_SCHEMA_ID_V1,
            &digest,
        ],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    let staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &ARTIFACT_RELEASE_SCHEMA_ID_V1,
            &digest,
        ],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    test.add_account(
        raw,
        Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner: REGISTRY_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    test.add_account(
        staging,
        Account {
            lamports: 1,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    (raw, staging)
}

fn fixture() -> Fixture {
    fixture_with_registry_authority(None)
}

fn fixture_with_registry_authority(registry_authority: Option<Pubkey>) -> Fixture {
    let artifacts = artifacts();
    let authority = Keypair::new();
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    add_program(
        &mut test,
        "dclutch_core_sbf",
        CORE_PROGRAM_ID,
        &artifacts.core,
        Some(authority.pubkey()),
    );
    add_program(
        &mut test,
        "dclutch_registry_sbf",
        REGISTRY_PROGRAM_ID,
        &artifacts.registry,
        registry_authority,
    );
    add_program(
        &mut test,
        "dclutch_rent_sbf",
        RENT_PROGRAM_ID,
        &artifacts.rent,
        None,
    );
    test.add_account(
        authority.pubkey(),
        Account {
            lamports: 1_000_000,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    let registry_release = artifact_release(
        REGISTRY_PROGRAM_ID,
        &artifacts.registry,
        0xa1,
        registry_authority,
    );
    let rent_release = artifact_release(RENT_PROGRAM_ID, &artifacts.rent, 0xa2, None);
    let registry_binding = dclutch_registry::release_set::ExecutionRoleBindingV1::new(
        registry_release.program(),
        dclutch_registry::release_set::ArtifactReleaseIdV1::new(
            hash(&registry_release.to_bytes()).to_bytes(),
        )
        .expect("Registry artifact ID"),
    );
    let rent_binding = dclutch_registry::release_set::ExecutionRoleBindingV1::new(
        rent_release.program(),
        dclutch_registry::release_set::ArtifactReleaseIdV1::new(
            hash(&rent_release.to_bytes()).to_bytes(),
        )
        .expect("Rent artifact ID"),
    );
    let expected =
        ProtocolInfrastructureProfileV1::new(registry_binding, rent_binding).expect("profile");
    // The same two bindings the V1 seals, committed again at the V2 domain
    // with the genesis sentinels: `c60b25e8` made initialization write BOTH
    // profiles in one instruction, so a cohort can never stand half
    // initialized with a V1 nothing reads and no V2 to found against.
    let expected_genesis = ProtocolInfrastructureProfileV2::genesis(registry_binding, rent_binding)
        .expect("genesis profile");
    let (registry_raw, registry_staging) = add_artifact_record(&mut test, registry_release);
    let (rent_raw, rent_staging) = add_artifact_record(&mut test, rent_release);
    let profile = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
        &CORE_PROGRAM_ID,
    )
    .0;
    let genesis_profile = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
        &CORE_PROGRAM_ID,
    )
    .0;
    for vacant in [profile, genesis_profile] {
        test.add_account(
            vacant,
            Account {
                lamports: 1,
                data: Vec::new(),
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            },
        );
    }
    Fixture {
        test,
        authority,
        profile,
        genesis_profile,
        core_programdata: programdata_address(CORE_PROGRAM_ID),
        registry_programdata: programdata_address(REGISTRY_PROGRAM_ID),
        rent_programdata: programdata_address(RENT_PROGRAM_ID),
        registry_raw,
        registry_staging,
        rent_raw,
        rent_staging,
        expected,
        expected_genesis,
    }
}

fn initialize_instruction(fixture: &Fixture, authority: Pubkey) -> Instruction {
    let data = InitializeProtocolInfrastructureV1.to_bytes().to_vec();
    assert_eq!(data.len(), INITIALIZE_PROTOCOL_INFRASTRUCTURE_BYTES_V1);
    Instruction {
        program_id: CORE_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(Pubkey::default(), true),
            AccountMeta::new(fixture.profile, false),
            // The genesis V2, written by this same instruction at the V2
            // domain. `InitializeInfrastructureAccounts::parse` reads it
            // third -- writable, non-signer, distinct from the V1 -- exactly
            // as `bootstrap/successor`'s `initialize_instruction` builds it.
            AccountMeta::new(fixture.genesis_profile, false),
            AccountMeta::new_readonly(fixture.core_programdata, false),
            AccountMeta::new_readonly(authority, true),
            AccountMeta::new_readonly(fixture.registry_raw, false),
            AccountMeta::new_readonly(fixture.registry_staging, false),
            AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.registry_programdata, false),
            AccountMeta::new_readonly(fixture.rent_raw, false),
            AccountMeta::new_readonly(fixture.rent_staging, false),
            AccountMeta::new_readonly(RENT_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.rent_programdata, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data,
    }
}

#[tokio::test]
async fn exact_loader_authority_initializes_once_and_cannot_update() {
    let fixture = fixture();
    let mut instruction = initialize_instruction(&fixture, fixture.authority.pubkey());
    let context = fixture.test.start_with_context().await;
    let payer = context.payer.pubkey();
    instruction.accounts.get_mut(0).expect("payer meta").pubkey = payer;
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(PROTOCOL_COMPUTE_UNIT_LIMIT),
            instruction.clone(),
        ],
        Some(&payer),
        &[&context.payer, &fixture.authority],
        blockhash,
    );
    context
        .banks_client
        .process_transaction(transaction)
        .await
        .expect("initialize profile");
    let account = context
        .banks_client
        .get_account(fixture.profile)
        .await
        .expect("profile query")
        .expect("profile");
    assert_eq!(account.owner, CORE_PROGRAM_ID);
    assert_eq!(
        ProtocolInfrastructureProfileV1::decode(&account.data),
        Ok(fixture.expected),
    );
    let bytes_before = account.data;
    let genesis = context
        .banks_client
        .get_account(fixture.genesis_profile)
        .await
        .expect("genesis profile query")
        .expect("genesis profile");
    assert_eq!(genesis.owner, CORE_PROGRAM_ID);
    assert_eq!(
        ProtocolInfrastructureProfileV2::decode(&genesis.data),
        Ok(fixture.expected_genesis),
    );
    assert!(fixture.expected_genesis.born_at_v2());
    let genesis_bytes_before = genesis.data;

    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("second blockhash");
    let replay = Transaction::new_signed_with_payer(
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(PROTOCOL_COMPUTE_UNIT_LIMIT),
            instruction,
        ],
        Some(&payer),
        &[&context.payer, &fixture.authority],
        blockhash,
    );
    // Write-once: the V1 domain is no longer the vacancy `create_profile`
    // demands, so the replay refuses there rather than rewriting the root.
    refused(
        context.banks_client.process_transaction(replay).await,
        CoreSbfError::Infrastructure,
    );
    let after = context
        .banks_client
        .get_account(fixture.profile)
        .await
        .expect("profile query")
        .expect("profile");
    assert_eq!(after.data, bytes_before);
    let genesis_after = context
        .banks_client
        .get_account(fixture.genesis_profile)
        .await
        .expect("genesis profile query")
        .expect("genesis profile");
    assert_eq!(genesis_after.data, genesis_bytes_before);
}

#[tokio::test]
async fn substituted_core_upgrade_authority_cannot_create_the_root() {
    let fixture = fixture();
    let wrong = Keypair::new();
    let mut instruction = initialize_instruction(&fixture, wrong.pubkey());
    let mut test = fixture.test;
    test.add_account(
        wrong.pubkey(),
        Account {
            lamports: 1_000_000,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    let context = test.start_with_context().await;
    let payer = context.payer.pubkey();
    instruction.accounts.get_mut(0).expect("payer meta").pubkey = payer;
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(PROTOCOL_COMPUTE_UNIT_LIMIT),
            instruction,
        ],
        Some(&payer),
        &[&context.payer, &wrong],
        blockhash,
    );
    refused(
        context.banks_client.process_transaction(transaction).await,
        CoreSbfError::Infrastructure,
    );
    let account = context
        .banks_client
        .get_account(fixture.profile)
        .await
        .expect("profile query")
        .expect("vacant profile");
    assert_eq!(account.owner, system_program::ID);
    assert!(account.data.is_empty());
    let genesis = context
        .banks_client
        .get_account(fixture.genesis_profile)
        .await
        .expect("genesis profile query")
        .expect("vacant genesis profile");
    assert_eq!(genesis.owner, system_program::ID);
    assert!(genesis.data.is_empty());
}

/// GREEN ON NOTHING, AND LEFT THAT WAY DELIBERATELY. NEEDS AN OWNER.
///
/// This test has never reached its subject. It submits no
/// `ComputeBudgetInstruction`, so it runs against the 200,000-CU default while
/// the honest route costs about 245,000: the `is_err()` below is satisfied by
/// `Computational budget exceeded`, 200,000 of 200,000, and has been since
/// long before the frame widened. Measured 2026-09-03 by giving it
/// `PROTOCOL_COMPUTE_UNIT_LIMIT` like its two siblings -- it then SUCCEEDS at
/// 244,699 CU. The program does not refuse a mutable Registry.
///
/// The rule it was written for was real: `d6d5f2d40` carried `require_immutable`
/// and an `upgrade_policy() != Immutable` refusal in `infrastructure.rs`. Both
/// are gone at HEAD, retired by decision 0012 in favour of `ExactAuthority`
/// plus slot-pinned re-authentication. So this names a refusal the protocol
/// deliberately stopped making, and the compute ceiling is why nobody found out.
///
/// Two honest futures, and choosing between them is a design call this lane
/// does not own: DELETE it as superseded by 0012, or REPURPOSE it into a
/// positive that pins "an `ExactAuthority` Registry IS admitted and the root
/// stays usable through slot-pinned re-authentication". Reproduce either way in
/// one edit -- add the budget request ahead of `instruction` and watch it go
/// green-then-red. Weakening or deleting it silently to tidy the file would
/// ratify a design call by omission, which is why it still stands as written.
#[tokio::test]
async fn mutable_registry_cannot_be_frozen_into_an_unusable_root() {
    let mutable_registry_authority = Pubkey::new_unique();
    let fixture = fixture_with_registry_authority(Some(mutable_registry_authority));
    let mut instruction = initialize_instruction(&fixture, fixture.authority.pubkey());
    let context = fixture.test.start_with_context().await;
    let payer = context.payer.pubkey();
    instruction.accounts.get_mut(0).expect("payer meta").pubkey = payer;
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer),
        &[&context.payer, &fixture.authority],
        blockhash,
    );
    assert!(
        context
            .banks_client
            .process_transaction(transaction)
            .await
            .is_err()
    );
    let account = context
        .banks_client
        .get_account(fixture.profile)
        .await
        .expect("profile query")
        .expect("vacant profile");
    assert_eq!(account.owner, system_program::ID);
    assert!(account.data.is_empty());
    let genesis = context
        .banks_client
        .get_account(fixture.genesis_profile)
        .await
        .expect("genesis profile query")
        .expect("vacant genesis profile");
    assert_eq!(genesis.owner, system_program::ID);
    assert!(genesis.data.is_empty());
}
