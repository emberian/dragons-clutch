//! Real two-program SBF campaign for the controller-PDA authority membrane.
//!
//! This is not Direct admission or custody evidence. It tests authentic PDA
//! delegation and cross-program rollback against the exact Effect ELF.

use std::{env, path::PathBuf};

use solana_account::Account;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::signature::Signer;
use solana_sdk_ids::system_program;
use solana_transaction::Transaction;

const CONTROLLER_PROGRAM_ID: Pubkey = Pubkey::new_from_array([67_u8; 32]);
const EFFECT_PROGRAM_ID: Pubkey = Pubkey::new_from_array([83_u8; 32]);
const CONTROLLER_SEED: &[u8] = b"dclutch-controller-v1";
const STATE_BYTES: usize = 104;
const JOURNAL_BYTES: usize = 16;
const VECTOR_HEX: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../formal/dclutch-semantics/vectors/direct-inline-ordinary-v1.hex"
));

fn require_sbf() {
    let directory = env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required for real ELF tests");
    for artifact in [
        "dclutch_controller_proof_sbf.so",
        "dclutch_effect_proof_sbf.so",
    ] {
        assert!(
            PathBuf::from(&directory).join(artifact).is_file(),
            "SBF_OUT_DIR must contain {artifact}"
        );
    }
}

fn decode_hex(value: &str) -> Vec<u8> {
    value
        .trim()
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).expect("fixture is UTF-8");
            u8::from_str_radix(pair, 16).expect("fixture is hexadecimal")
        })
        .collect()
}

fn put_u64(bytes: &mut [u8], index: usize, value: u64) {
    let offset = 48 + index * 8;
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn read_u64(bytes: &[u8], index: usize) -> u64 {
    let offset = 48 + index * 8;
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("u64 field"))
}

fn projection(authority: Pubkey, venue_collateral: u64) -> Vec<u8> {
    let mut bytes = vec![0_u8; STATE_BYTES];
    bytes[0..4].copy_from_slice(b"DCES");
    bytes[4] = 1;
    bytes[8..40].copy_from_slice(authority.as_ref());
    bytes[40..44].copy_from_slice(&1_u32.to_le_bytes());
    for (index, value) in [0, 0, 5_000, 200, 2_000, 100, venue_collateral]
        .into_iter()
        .enumerate()
    {
        put_u64(&mut bytes, index, value);
    }
    bytes
}

fn journal(counter: u64) -> Vec<u8> {
    let mut bytes = vec![0_u8; JOURNAL_BYTES];
    bytes[0..4].copy_from_slice(b"DCCJ");
    bytes[8..16].copy_from_slice(&counter.to_le_bytes());
    bytes
}

fn read_journal(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(bytes[8..16].try_into().expect("journal counter"))
}

fn relay_instruction(
    controller: Pubkey,
    journal: Pubkey,
    projection: Pubkey,
    bump: u8,
    plan: &[u8],
) -> Instruction {
    let mut data = Vec::with_capacity(plan.len() + 1);
    data.push(bump);
    data.extend_from_slice(plan);
    Instruction {
        program_id: CONTROLLER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(controller, false),
            AccountMeta::new(journal, false),
            AccountMeta::new(projection, false),
            AccountMeta::new_readonly(EFFECT_PROGRAM_ID, false),
        ],
        data,
    }
}

fn direct_instruction(authority: Pubkey, projection: Pubkey, plan: &[u8]) -> Instruction {
    Instruction {
        program_id: EFFECT_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(authority, false),
            AccountMeta::new(projection, false),
        ],
        data: plan.to_vec(),
    }
}

async fn submit(context: &mut ProgramTestContext, instruction: Instruction) -> (bool, u64) {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("banks processing");
    (
        processed.result.is_ok(),
        processed
            .metadata
            .expect("transaction metadata")
            .compute_units_consumed,
    )
}

async fn account(context: &mut ProgramTestContext, address: Pubkey) -> Account {
    context
        .banks_client
        .get_account(address)
        .await
        .expect("query")
        .expect("account")
}

#[tokio::test]
async fn only_controller_can_delegate_and_child_failure_rolls_back_caller() {
    require_sbf();
    let plan = decode_hex(VECTOR_HEX);
    let (controller, bump) =
        Pubkey::find_program_address(&[CONTROLLER_SEED], &CONTROLLER_PROGRAM_ID);
    let journal_key = Pubkey::new_unique();
    let success_key = Pubkey::new_unique();
    let refusal_key = Pubkey::new_unique();

    let mut test = ProgramTest::new("dclutch_controller_proof_sbf", CONTROLLER_PROGRAM_ID, None);
    test.prefer_bpf(true);
    test.add_program("dclutch_effect_proof_sbf", EFFECT_PROGRAM_ID, None);
    test.add_account(controller, Account::new(1_000_000, 0, &system_program::ID));
    test.add_account(
        journal_key,
        Account {
            lamports: Rent::default().minimum_balance(JOURNAL_BYTES),
            data: journal(0),
            owner: CONTROLLER_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    test.add_account(
        success_key,
        Account {
            lamports: Rent::default().minimum_balance(STATE_BYTES),
            data: projection(controller, 20),
            owner: EFFECT_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    test.add_account(
        refusal_key,
        Account {
            lamports: Rent::default().minimum_balance(STATE_BYTES),
            data: projection(controller, u64::MAX),
            owner: EFFECT_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    let mut context = test.start_with_context().await;

    let (direct_ok, direct_cu) = submit(
        &mut context,
        direct_instruction(controller, success_key, &plan),
    )
    .await;
    assert!(!direct_ok, "transaction callers cannot sign for the PDA");
    assert_eq!(
        read_journal(&account(&mut context, journal_key).await.data),
        0
    );
    assert_eq!(
        read_u64(&account(&mut context, success_key).await.data, 0),
        0
    );

    let (relay_ok, relay_cu) = submit(
        &mut context,
        relay_instruction(controller, journal_key, success_key, bump, &plan),
    )
    .await;
    assert!(
        relay_ok,
        "controller must lend its authenticated PDA signer"
    );
    assert_eq!(
        read_journal(&account(&mut context, journal_key).await.data),
        1
    );
    let success = account(&mut context, success_key).await;
    assert_eq!(read_u64(&success.data, 0), 1);
    assert_eq!(read_u64(&success.data, 1), 1);
    assert_eq!(read_u64(&success.data, 2), 3_000);
    assert_eq!(read_u64(&success.data, 3), 2_200);

    let journal_before = account(&mut context, journal_key).await;
    let refusal_before = account(&mut context, refusal_key).await;
    let (refusal_ok, refusal_cu) = submit(
        &mut context,
        relay_instruction(controller, journal_key, refusal_key, bump, &plan),
    )
    .await;
    assert!(
        !refusal_ok,
        "late venue overflow must escape as child refusal"
    );
    assert_eq!(account(&mut context, journal_key).await, journal_before);
    assert_eq!(account(&mut context, refusal_key).await, refusal_before);

    let (wrong_bump_ok, wrong_bump_cu) = submit(
        &mut context,
        relay_instruction(
            controller,
            journal_key,
            refusal_key,
            bump.wrapping_add(1),
            &plan,
        ),
    )
    .await;
    assert!(!wrong_bump_ok, "wrong controller bump must refuse");
    assert_eq!(account(&mut context, journal_key).await, journal_before);
    assert_eq!(account(&mut context, refusal_key).await, refusal_before);

    eprintln!(
        "controller membrane CU: direct refusal={direct_cu}, relay success={relay_cu}, child rollback={refusal_cu}, wrong bump={wrong_bump_cu}"
    );
}
