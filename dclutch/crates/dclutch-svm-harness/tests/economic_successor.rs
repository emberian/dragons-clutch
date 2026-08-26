//! Real-SVM campaign for the bounded economic successor.
//!
//! The compiled economic ELF and official legacy SPL Token ELF execute. No
//! native processor, token mock, or Direct program participates.

use std::{env, path::PathBuf};

use dclutch_economic_adapter_contract::{
    FoundingV1, OperationV1, PROJECTION_BYTES_V1, ProjectionV1,
};
use dclutch_economic_kernel::{Holder, PhaseKind, Representation};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, EXECUTION_RELEASE_SET_BYTES_V1, ExecutionReleaseSetV1,
    ExecutionRoleBindingV1, ProgramIdentityV1,
};
use dclutch_token_svm::{ACCOUNT_BYTES, LEGACY_TOKEN_PROGRAM_ID, MINT_BYTES, TokenAccount};
use solana_account::Account;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::system_program;
use solana_transaction::Transaction;

const ECONOMIC_PROGRAM_ID: Pubkey = Pubkey::new_from_array([2; 32]);
const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([1; 32]);
const TRADING_PROGRAM_ID: Pubkey = Pubkey::new_from_array([3; 32]);
const RESOLUTION_PROGRAM_ID: Pubkey = Pubkey::new_from_array([4; 32]);
const SUBSTITUTE_RESOLUTION_PROGRAM_ID: Pubkey = Pubkey::new_from_array([44; 32]);
const HOARD_AUTHORITY_SEED_V1: &[u8] = b"dclutch-economic-hoard-v1";
const OUTCOME_COUNT: u8 = 3;

#[derive(Clone, Copy)]
struct Fixture {
    projection: Pubkey,
    release_set: Pubkey,
    substitute_release_set: Pubkey,
    mint: Pubkey,
    source_token: Pubkey,
    destination_token: Pubkey,
    hoard_token: Pubkey,
    hoard_authority: Pubkey,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Snapshot {
    projection: Account,
    mint: Account,
    source_token: Account,
    destination_token: Account,
    hoard_token: Account,
}

#[derive(Debug)]
struct Submission {
    accepted: bool,
    compute_units: u64,
    logs: Vec<String>,
}

struct ExpectedState {
    revision: u64,
    hoard: u64,
    supply: [u64; 3],
    native_supply: [u64; 3],
    materialized_supply: [u64; 3],
    source_native: [u64; 3],
    destination_materialized: [u64; 3],
}

fn token_program_id() -> Pubkey {
    Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID)
}

fn require_real_elfs() {
    let directory = env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required for real ELF tests");
    for artifact in ["dclutch_economic_sbf.so", "spl_token.so"] {
        assert!(
            PathBuf::from(&directory).join(artifact).is_file(),
            "SBF_OUT_DIR must contain {artifact}"
        );
    }
}

fn role(program: Pubkey, release_byte: u8) -> ExecutionRoleBindingV1 {
    ExecutionRoleBindingV1::new(
        ProgramIdentityV1::new(program.to_bytes()).expect("nonzero program"),
        ArtifactReleaseIdV1::new([release_byte; 32]).expect("nonzero release"),
    )
}

fn release_set(resolution_program: Pubkey) -> [u8; EXECUTION_RELEASE_SET_BYTES_V1] {
    let economic = role(ECONOMIC_PROGRAM_ID, 12);
    ExecutionReleaseSetV1::new(
        role(CORE_PROGRAM_ID, 11),
        economic,
        role(TRADING_PROGRAM_ID, 13),
        role(resolution_program, 14),
        economic,
    )
    .expect("canonical release set")
    .to_bytes()
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn mint_state(supply: u64, decimals: u8) -> Vec<u8> {
    let mut bytes = vec![0_u8; MINT_BYTES];
    put_u64(&mut bytes, 36, supply);
    bytes[44] = decimals;
    bytes[45] = 1;
    bytes
}

fn token_state(mint: Pubkey, owner: Pubkey, amount: u64) -> Vec<u8> {
    let mut bytes = vec![0_u8; ACCOUNT_BYTES];
    bytes[0..32].copy_from_slice(mint.as_ref());
    bytes[32..64].copy_from_slice(owner.as_ref());
    put_u64(&mut bytes, 64, amount);
    bytes[108] = 1;
    bytes
}

fn add_account(test: &mut ProgramTest, key: Pubkey, owner: Pubkey, data: Vec<u8>) {
    test.add_account(
        key,
        Account {
            lamports: Rent::default().minimum_balance(data.len()).max(1_000_000),
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn add_authority(test: &mut ProgramTest, authority: &Keypair, owner: Pubkey) {
    add_account(test, authority.pubkey(), owner, Vec::new());
}

fn founding_instruction(
    fixture: Fixture,
    core_authority: Pubkey,
    founding: FoundingV1,
) -> Instruction {
    Instruction {
        program_id: ECONOMIC_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(core_authority, true),
            AccountMeta::new(fixture.projection, false),
            AccountMeta::new_readonly(fixture.release_set, false),
        ],
        data: founding.to_bytes().to_vec(),
    }
}

fn custody_instruction(
    fixture: Fixture,
    release_set: Pubkey,
    semantic_authority: Pubkey,
    holder_authority: Pubkey,
    holder_token: Pubkey,
    holder_signs: bool,
    operation: OperationV1,
) -> Instruction {
    Instruction {
        program_id: ECONOMIC_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(semantic_authority, true),
            AccountMeta::new(fixture.projection, false),
            AccountMeta::new_readonly(release_set, false),
            AccountMeta::new_readonly(holder_authority, holder_signs),
            AccountMeta::new_readonly(fixture.mint, false),
            AccountMeta::new(holder_token, false),
            AccountMeta::new(fixture.hoard_token, false),
            AccountMeta::new_readonly(fixture.hoard_authority, false),
            AccountMeta::new_readonly(token_program_id(), false),
        ],
        data: operation.to_bytes().to_vec(),
    }
}

fn logical_instruction(
    fixture: Fixture,
    release_set: Pubkey,
    semantic_authority: Pubkey,
    operation: OperationV1,
) -> Instruction {
    Instruction {
        program_id: ECONOMIC_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(semantic_authority, true),
            AccountMeta::new(fixture.projection, false),
            AccountMeta::new_readonly(release_set, false),
        ],
        data: operation.to_bytes().to_vec(),
    }
}

async fn submit(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    additional_signers: &[&Keypair],
) -> Submission {
    let blockhash = context
        .get_new_latest_blockhash()
        .await
        .expect("fresh blockhash");
    let mut signers: Vec<&dyn Signer> = vec![&context.payer];
    signers.extend(
        additional_signers
            .iter()
            .copied()
            .map(|signer| signer as &dyn Signer),
    );
    let transaction = Transaction::new_signed_with_payer(
        instructions,
        Some(&context.payer.pubkey()),
        &signers,
        blockhash,
    );
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("banks processing");
    let (compute_units, logs) = processed
        .metadata
        .map(|metadata| (metadata.compute_units_consumed, metadata.log_messages))
        .unwrap_or_else(|| (0, Vec::new()));
    Submission {
        accepted: processed.result.is_ok(),
        compute_units,
        logs,
    }
}

async fn account(context: &mut ProgramTestContext, key: Pubkey) -> Account {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account query")
        .expect("fixture account")
}

async fn snapshot(context: &mut ProgramTestContext, fixture: Fixture) -> Snapshot {
    Snapshot {
        projection: account(context, fixture.projection).await,
        mint: account(context, fixture.mint).await,
        source_token: account(context, fixture.source_token).await,
        destination_token: account(context, fixture.destination_token).await,
        hoard_token: account(context, fixture.hoard_token).await,
    }
}

fn token_amount(account: &Account) -> u64 {
    TokenAccount::parse(&account.data)
        .expect("exact token Account")
        .amount
}

fn projection(account: &Account) -> ProjectionV1 {
    ProjectionV1::decode(&account.data).expect("canonical economic projection")
}

fn assert_open_state(projection: ProjectionV1, expected: ExpectedState) {
    let state = projection.state();
    assert_eq!(projection.revision(), expected.revision);
    assert_eq!(state.phase().kind(), PhaseKind::Open);
    assert_eq!(state.hoard(), expected.hoard);
    assert_eq!(state.supply(), expected.supply.as_slice());
    assert_eq!(state.native_supply(), expected.native_supply.as_slice());
    assert_eq!(
        state.materialized_supply(),
        expected.materialized_supply.as_slice()
    );
    assert_eq!(state.source_native(), expected.source_native.as_slice());
    assert_eq!(
        state.destination_materialized(),
        expected.destination_materialized.as_slice()
    );
    assert_eq!(state.validate(), Ok(()));
}

#[tokio::test]
async fn economic_success_and_late_refusal_are_physically_atomic() {
    require_real_elfs();

    let core_authority = Keypair::new();
    let trading_authority = Keypair::new();
    let wrong_authority = Keypair::new();
    let source_holder = Keypair::new();
    let destination_holder = Keypair::new();
    let projection_key = Pubkey::new_unique();
    let release_key = Pubkey::new_unique();
    let substitute_release_key = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let source_token = Pubkey::new_unique();
    let destination_token = Pubkey::new_unique();
    let hoard_token = Pubkey::new_unique();
    let (hoard_authority, _) = Pubkey::find_program_address(
        &[HOARD_AUTHORITY_SEED_V1, projection_key.as_ref()],
        &ECONOMIC_PROGRAM_ID,
    );
    let fixture = Fixture {
        projection: projection_key,
        release_set: release_key,
        substitute_release_set: substitute_release_key,
        mint: mint_key,
        source_token,
        destination_token,
        hoard_token,
        hoard_authority,
    };
    let release_bytes = release_set(RESOLUTION_PROGRAM_ID);
    let substitute_release_bytes = release_set(SUBSTITUTE_RESOLUTION_PROGRAM_ID);
    let release_id = hash(&release_bytes).to_bytes();
    assert_ne!(release_id, hash(&substitute_release_bytes).to_bytes());
    let founding = FoundingV1::new(
        [8; 32],
        release_id,
        source_holder.pubkey().to_bytes(),
        destination_holder.pubkey().to_bytes(),
        mint_key.to_bytes(),
        hoard_token.to_bytes(),
        OUTCOME_COUNT,
    )
    .expect("canonical founding");

    let mut test = ProgramTest::new("dclutch_economic_sbf", ECONOMIC_PROGRAM_ID, None);
    test.prefer_bpf(true);
    test.add_program("spl_token", token_program_id(), None);
    add_authority(&mut test, &core_authority, CORE_PROGRAM_ID);
    add_authority(&mut test, &trading_authority, TRADING_PROGRAM_ID);
    add_authority(&mut test, &wrong_authority, CORE_PROGRAM_ID);
    add_authority(&mut test, &source_holder, system_program::ID);
    add_authority(&mut test, &destination_holder, system_program::ID);
    add_account(
        &mut test,
        projection_key,
        ECONOMIC_PROGRAM_ID,
        vec![0; PROJECTION_BYTES_V1],
    );
    add_account(
        &mut test,
        release_key,
        CORE_PROGRAM_ID,
        release_bytes.to_vec(),
    );
    add_account(
        &mut test,
        substitute_release_key,
        CORE_PROGRAM_ID,
        substitute_release_bytes.to_vec(),
    );
    add_account(
        &mut test,
        mint_key,
        token_program_id(),
        mint_state(2_000, 6),
    );
    add_account(
        &mut test,
        source_token,
        token_program_id(),
        token_state(mint_key, source_holder.pubkey(), 1_000),
    );
    add_account(
        &mut test,
        destination_token,
        token_program_id(),
        token_state(mint_key, destination_holder.pubkey(), 1_000),
    );
    add_account(
        &mut test,
        hoard_token,
        token_program_id(),
        token_state(mint_key, hoard_authority, 0),
    );
    add_account(&mut test, hoard_authority, ECONOMIC_PROGRAM_ID, Vec::new());
    let mut context = test.start_with_context().await;
    let initial = snapshot(&mut context, fixture).await;

    let found = submit(
        &mut context,
        &[founding_instruction(
            fixture,
            core_authority.pubkey(),
            founding,
        )],
        &[&core_authority],
    )
    .await;
    assert!(found.accepted, "founding must commit: {:?}", found.logs);
    let founded = snapshot(&mut context, fixture).await;
    assert_ne!(founded.projection.data, initial.projection.data);
    assert_eq!(founded.mint, initial.mint);
    assert_eq!(founded.source_token, initial.source_token);
    assert_eq!(founded.destination_token, initial.destination_token);
    assert_eq!(founded.hoard_token, initial.hoard_token);
    assert_open_state(
        projection(&founded.projection),
        ExpectedState {
            revision: 0,
            hoard: 0,
            supply: [0; 3],
            native_supply: [0; 3],
            materialized_supply: [0; 3],
            source_native: [0; 3],
            destination_materialized: [0; 3],
        },
    );

    let source_split = custody_instruction(
        fixture,
        release_key,
        trading_authority.pubkey(),
        source_holder.pubkey(),
        source_token,
        true,
        OperationV1::split(Holder::Source, Representation::Native, 100, 0),
    );
    let split = submit(
        &mut context,
        std::slice::from_ref(&source_split),
        &[&trading_authority, &source_holder],
    )
    .await;
    assert!(split.accepted, "source split must commit: {:?}", split.logs);
    let after_source_split = snapshot(&mut context, fixture).await;
    assert_open_state(
        projection(&after_source_split.projection),
        ExpectedState {
            revision: 1,
            hoard: 100,
            supply: [100; 3],
            native_supply: [100; 3],
            materialized_supply: [0; 3],
            source_native: [100; 3],
            destination_materialized: [0; 3],
        },
    );
    assert_eq!(token_amount(&after_source_split.source_token), 900);
    assert_eq!(token_amount(&after_source_split.destination_token), 1_000);
    assert_eq!(token_amount(&after_source_split.hoard_token), 100);
    assert_eq!(after_source_split.mint, initial.mint);

    let destination_split = custody_instruction(
        fixture,
        release_key,
        trading_authority.pubkey(),
        destination_holder.pubkey(),
        destination_token,
        true,
        OperationV1::split(Holder::Destination, Representation::Materialized, 30, 1),
    );
    let second_split = submit(
        &mut context,
        &[destination_split],
        &[&trading_authority, &destination_holder],
    )
    .await;
    assert!(
        second_split.accepted,
        "destination split must commit: {:?}",
        second_split.logs
    );
    let after_destination_split = snapshot(&mut context, fixture).await;
    assert_open_state(
        projection(&after_destination_split.projection),
        ExpectedState {
            revision: 2,
            hoard: 130,
            supply: [130; 3],
            native_supply: [100; 3],
            materialized_supply: [30; 3],
            source_native: [100; 3],
            destination_materialized: [30; 3],
        },
    );
    assert_eq!(token_amount(&after_destination_split.source_token), 900);
    assert_eq!(
        token_amount(&after_destination_split.destination_token),
        970
    );
    assert_eq!(token_amount(&after_destination_split.hoard_token), 130);

    let source_merge = custody_instruction(
        fixture,
        release_key,
        trading_authority.pubkey(),
        source_holder.pubkey(),
        source_token,
        false,
        OperationV1::merge(Holder::Source, Representation::Native, 40, 2),
    );
    let merge = submit(
        &mut context,
        std::slice::from_ref(&source_merge),
        &[&trading_authority],
    )
    .await;
    assert!(merge.accepted, "source merge must commit: {:?}", merge.logs);
    let committed = snapshot(&mut context, fixture).await;
    assert_open_state(
        projection(&committed.projection),
        ExpectedState {
            revision: 3,
            hoard: 90,
            supply: [90; 3],
            native_supply: [60; 3],
            materialized_supply: [30; 3],
            source_native: [60; 3],
            destination_materialized: [30; 3],
        },
    );
    assert_eq!(token_amount(&committed.source_token), 940);
    assert_eq!(token_amount(&committed.destination_token), 970);
    assert_eq!(token_amount(&committed.hoard_token), 90);
    assert_eq!(committed.mint, initial.mint);

    let replay = submit(&mut context, &[source_merge], &[&trading_authority]).await;
    assert!(!replay.accepted, "stale revision must refuse");
    assert_eq!(snapshot(&mut context, fixture).await, committed);

    let valid_transfer = OperationV1::transfer(Representation::Native, 0, 1, 3);
    let authority_substitution = submit(
        &mut context,
        &[logical_instruction(
            fixture,
            release_key,
            wrong_authority.pubkey(),
            valid_transfer,
        )],
        &[&wrong_authority],
    )
    .await;
    assert!(
        !authority_substitution.accepted,
        "Core-owned signer cannot substitute for Trading"
    );
    assert_eq!(snapshot(&mut context, fixture).await, committed);

    let release_substitution = submit(
        &mut context,
        &[logical_instruction(
            fixture,
            fixture.substitute_release_set,
            trading_authority.pubkey(),
            valid_transfer,
        )],
        &[&trading_authority],
    )
    .await;
    assert!(
        !release_substitution.accepted,
        "different canonical release-set content must refuse"
    );
    assert_eq!(snapshot(&mut context, fixture).await, committed);

    let late_split = custody_instruction(
        fixture,
        release_key,
        trading_authority.pubkey(),
        source_holder.pubkey(),
        source_token,
        true,
        OperationV1::split(Holder::Source, Representation::Native, 5, 3),
    );
    let late = submit(
        &mut context,
        &[late_split.clone(), late_split],
        &[&trading_authority, &source_holder],
    )
    .await;
    assert!(
        !late.accepted,
        "second stale instruction must reject the transaction"
    );
    let token_success = format!("Program {} success", token_program_id());
    assert!(
        late.logs.iter().any(|log| log == &token_success),
        "logs must prove the first instruction completed official Token CPI: {:?}",
        late.logs
    );
    assert_eq!(
        snapshot(&mut context, fixture).await,
        committed,
        "late refusal must roll back projection and every completed Token delta"
    );

    eprintln!(
        "economic successor CU: found={}, source_split={}, destination_split={}, merge={}, replay={}, authority_sub={}, release_sub={}, late_rollback={}",
        found.compute_units,
        split.compute_units,
        second_split.compute_units,
        merge.compute_units,
        replay.compute_units,
        authority_substitution.compute_units,
        release_substitution.compute_units,
        late.compute_units,
    );
}
