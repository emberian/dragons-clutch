use std::{env, path::PathBuf};

use dclutch_collateral_contract::{CREATE_REALM_BYTES, CreateRealmV1};
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_BYTES, REALM_PDA_DOMAIN, RealmV1,
    RealmV1Input,
};
use dclutch_token_svm::{CollateralAdapterReleaseV1, LEGACY_TOKEN_PROGRAM_ID, MINT_BYTES};
use solana_account::Account;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{bpf_loader, system_program, sysvar};
use solana_transaction::Transaction;

const PROGRAM_ID: Pubkey = Pubkey::new_from_array([71; 32]);
const MINT: Pubkey = Pubkey::new_from_array([61; 32]);
const WRONG_REALM: Pubkey = Pubkey::new_from_array([62; 32]);
const SPONSOR_OPENING: u64 = 10_000_000;

#[derive(Clone, Copy)]
enum Fault {
    None,
    MintAuthority,
    WrongRealmPda,
    UnderfundedSponsor,
    NonExecutableTokenProgram,
}

struct Fixture {
    test: ProgramTest,
    sponsor: Keypair,
    sponsor_before: u64,
    submitted_realm: Pubkey,
    canonical_realm: Pubkey,
    realm_value: RealmV1,
}

fn require_sbf_out_dir() {
    let directory = env::var("SBF_OUT_DIR").expect(
        "SBF_OUT_DIR is required; build target/deploy/dclutch_sbf.so first, then run `SBF_OUT_DIR=../../target/deploy cargo test --test realm_creation`",
    );
    let artifact = PathBuf::from(directory).join("dclutch_sbf.so");
    assert!(
        artifact.is_file(),
        "SBF_OUT_DIR must contain the exact compiled dclutch_sbf.so artifact: {}",
        artifact.display()
    );
}

fn mint_bytes(with_authority: bool) -> [u8; MINT_BYTES] {
    let mut bytes = [0; MINT_BYTES];
    bytes[36..44].copy_from_slice(&1_000u64.to_le_bytes());
    bytes[44] = 6;
    bytes[45] = 1;
    if with_authority {
        bytes[0..4].copy_from_slice(&1u32.to_le_bytes());
        bytes[4..36].copy_from_slice(&[9; 32]);
    }
    bytes
}

fn fixture(fault: Fault) -> Fixture {
    require_sbf_out_dir();
    let release = CollateralAdapterReleaseV1::legacy_exact_transfer();
    let release_id = hash(&release.to_bytes()).to_bytes();
    let realm_value = RealmV1::new(RealmV1Input {
        collateral_semantic_id: [51; 32],
        token_program: LEGACY_TOKEN_PROGRAM_ID,
        collateral_mint: MINT.to_bytes(),
        collateral_adapter_release_id: release_id,
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    })
    .expect("canonical Realm");
    let realm_digest = hash(&realm_value.to_bytes()).to_bytes();
    let (canonical_realm, _) =
        Pubkey::find_program_address(&[REALM_PDA_DOMAIN, &realm_digest], &PROGRAM_ID);
    let submitted_realm = if matches!(fault, Fault::WrongRealmPda) {
        WRONG_REALM
    } else {
        canonical_realm
    };

    let rent = Rent::default();
    let realm_rent = rent.minimum_balance(REALM_BYTES);
    let sponsor_before = if matches!(fault, Fault::UnderfundedSponsor) {
        realm_rent.saturating_sub(1)
    } else {
        SPONSOR_OPENING
    };
    let sponsor = Keypair::new();
    let token_program = Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID);

    let mut test = ProgramTest::new("dclutch_sbf", PROGRAM_ID, None);
    test.prefer_bpf(true);
    test.add_account(
        sponsor.pubkey(),
        Account::new(sponsor_before, 0, &system_program::ID),
    );
    test.add_account(
        MINT,
        Account {
            lamports: rent.minimum_balance(MINT_BYTES),
            data: mint_bytes(matches!(fault, Fault::MintAuthority)).to_vec(),
            owner: token_program,
            executable: false,
            rent_epoch: 0,
        },
    );
    test.add_account(
        token_program,
        Account {
            lamports: 1,
            data: Vec::new(),
            owner: bpf_loader::ID,
            executable: !matches!(fault, Fault::NonExecutableTokenProgram),
            rent_epoch: 0,
        },
    );

    Fixture {
        test,
        sponsor,
        sponsor_before,
        submitted_realm,
        canonical_realm,
        realm_value,
    }
}

fn create_realm_instruction(fixture: &Fixture) -> Instruction {
    let mut data = [0; CREATE_REALM_BYTES];
    CreateRealmV1::new(fixture.realm_value)
        .encode(&mut data)
        .expect("exact Realm instruction");
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(fixture.sponsor.pubkey(), true),
            AccountMeta::new(fixture.submitted_realm, false),
            AccountMeta::new_readonly(MINT, false),
            AccountMeta::new_readonly(Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID), false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ],
        data: data.to_vec(),
    }
}

async fn submit(
    context: &mut ProgramTestContext,
    sponsor: &Keypair,
    instruction: Instruction,
) -> Result<(), solana_program_test::BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer, sponsor],
        blockhash,
    );
    context.banks_client.process_transaction(transaction).await
}

#[tokio::test]
async fn create_realm_executes_real_system_cpi_and_persists_exact_state() {
    let fixture = fixture(Fault::None);
    let instruction = create_realm_instruction(&fixture);
    let mut context = fixture.test.start_with_context().await;
    submit(&mut context, &fixture.sponsor, instruction)
        .await
        .expect("valid Realm creation");

    let realm = context
        .banks_client
        .get_account(fixture.canonical_realm)
        .await
        .expect("Realm query")
        .expect("Realm exists");
    let sponsor = context
        .banks_client
        .get_account(fixture.sponsor.pubkey())
        .await
        .expect("sponsor query")
        .expect("sponsor exists");
    let expected_rent = Rent::default().minimum_balance(REALM_BYTES);
    assert_eq!(realm.owner, PROGRAM_ID);
    assert_eq!(realm.lamports, expected_rent);
    assert_eq!(realm.data, fixture.realm_value.to_bytes());
    assert_eq!(RealmV1::decode(&realm.data), Ok(fixture.realm_value));
    assert_eq!(sponsor.lamports, fixture.sponsor_before - expected_rent);
}

#[tokio::test]
async fn hostile_realm_creation_refuses_and_rolls_back_every_program_target() {
    for fault in [
        Fault::MintAuthority,
        Fault::WrongRealmPda,
        Fault::UnderfundedSponsor,
        Fault::NonExecutableTokenProgram,
    ] {
        let fixture = fixture(fault);
        let instruction = create_realm_instruction(&fixture);
        let mut context = fixture.test.start_with_context().await;
        assert!(
            submit(&mut context, &fixture.sponsor, instruction)
                .await
                .is_err(),
            "hostile Realm creation must refuse"
        );
        let sponsor = context
            .banks_client
            .get_account(fixture.sponsor.pubkey())
            .await
            .expect("sponsor query")
            .expect("sponsor remains");
        assert_eq!(sponsor.lamports, fixture.sponsor_before);
        assert!(
            context
                .banks_client
                .get_account(fixture.submitted_realm)
                .await
                .expect("submitted Realm query")
                .is_none(),
            "failed transaction must not leave its submitted Realm"
        );
        assert!(
            context
                .banks_client
                .get_account(fixture.canonical_realm)
                .await
                .expect("canonical Realm query")
                .is_none(),
            "failed transaction must not leave the canonical Realm"
        );
    }
}
