//! Real-ELF one-time Core infrastructure-root initialization.

use std::{env, fs, path::PathBuf, vec, vec::Vec};

use dclutch_core_contract::ContentId;
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactReleaseV1, ArtifactUpgradePolicyV1,
};
use dclutch_release_set_contract::{
    INITIALIZE_PROTOCOL_INFRASTRUCTURE_BYTES_V1, InitializeProtocolInfrastructureV1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1, ProgramIdentityV1,
    ProtocolInfrastructureProfileV1,
};
use solana_account::Account;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::ProgramTest;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_transaction::Transaction;

const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xe1; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xe2; 32]);
const RENT_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xe3; 32]);

struct Artifacts {
    core: Vec<u8>,
    registry: Vec<u8>,
    rent: Vec<u8>,
}

struct Fixture {
    test: ProgramTest,
    authority: Keypair,
    profile: Pubkey,
    core_programdata: Pubkey,
    registry_programdata: Pubkey,
    rent_programdata: Pubkey,
    registry_raw: Pubkey,
    registry_staging: Pubkey,
    rent_raw: Pubkey,
    rent_staging: Pubkey,
    expected: ProtocolInfrastructureProfileV1,
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
    let expected = ProtocolInfrastructureProfileV1::new(
        dclutch_release_set_contract::ExecutionRoleBindingV1::new(
            registry_release.program(),
            dclutch_release_set_contract::ArtifactReleaseIdV1::new(
                hash(&registry_release.to_bytes()).to_bytes(),
            )
            .expect("Registry artifact ID"),
        ),
        dclutch_release_set_contract::ExecutionRoleBindingV1::new(
            rent_release.program(),
            dclutch_release_set_contract::ArtifactReleaseIdV1::new(
                hash(&rent_release.to_bytes()).to_bytes(),
            )
            .expect("Rent artifact ID"),
        ),
    )
    .expect("profile");
    let (registry_raw, registry_staging) = add_artifact_record(&mut test, registry_release);
    let (rent_raw, rent_staging) = add_artifact_record(&mut test, rent_release);
    let profile = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
        &CORE_PROGRAM_ID,
    )
    .0;
    test.add_account(
        profile,
        Account {
            lamports: 1,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    Fixture {
        test,
        authority,
        profile,
        core_programdata: programdata_address(CORE_PROGRAM_ID),
        registry_programdata: programdata_address(REGISTRY_PROGRAM_ID),
        rent_programdata: programdata_address(RENT_PROGRAM_ID),
        registry_raw,
        registry_staging,
        rent_raw,
        rent_staging,
        expected,
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
        &[instruction.clone()],
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

    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("second blockhash");
    let replay = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer),
        &[&context.payer, &fixture.authority],
        blockhash,
    );
    assert!(
        context
            .banks_client
            .process_transaction(replay)
            .await
            .is_err()
    );
    let after = context
        .banks_client
        .get_account(fixture.profile)
        .await
        .expect("profile query")
        .expect("profile");
    assert_eq!(after.data, bytes_before);
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
        &[instruction],
        Some(&payer),
        &[&context.payer, &wrong],
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
}

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
}
