//! Real-ELF measurement for the isolated Lean-owned Effect IR executor.
//!
//! This is not Direct lifecycle evidence: the executor deliberately trusts the
//! signer named by its projection account to have admitted the semantic plan.

use std::{env, path::PathBuf};

use solana_account::Account;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::system_program;
use solana_transaction::Transaction;

const PROGRAM_ID: Pubkey = Pubkey::new_from_array([82; 32]);
const STATE_BYTES: usize = 104;
const PLAN_HEADER_BYTES: usize = 8;
const EFFECT_BYTES: usize = 16;
const EFFECT_COUNT: usize = 7;
const VECTOR_HEX: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../formal/dclutch-semantics/vectors/direct-inline-ordinary-v1.hex"
));

fn require_sbf() {
    let directory = env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required for real ELF tests");
    assert!(
        PathBuf::from(directory)
            .join("dclutch_effect_sbf.so")
            .is_file()
    );
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

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut [u8], index: usize, value: u64) {
    let offset = 48 + index * 8;
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn read_u64(bytes: &[u8], index: usize) -> u64 {
    let offset = 48 + index * 8;
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("u64 field"))
}

fn projection(authority: Pubkey) -> Vec<u8> {
    let mut bytes = vec![0_u8; STATE_BYTES];
    bytes[0..4].copy_from_slice(b"DCES");
    bytes[4] = 1;
    bytes[8..40].copy_from_slice(authority.as_ref());
    put_u32(&mut bytes, 40, 1);
    for (index, value) in [0, 0, 5_000, 200, 2_000, 100, 20].into_iter().enumerate() {
        put_u64(&mut bytes, index, value);
    }
    bytes
}

async fn submit_with_cu(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    authority: &Keypair,
) -> Result<u64, BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer, authority],
        blockhash,
    );
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await?;
    processed.result?;
    processed
        .metadata
        .map(|metadata| metadata.compute_units_consumed)
        .ok_or(BanksClientError::ClientError(
            "missing ProgramTest transaction metadata",
        ))
}

async fn submit_refusal(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    authority: &Keypair,
) -> Result<(), BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    context
        .banks_client
        .process_transaction(Transaction::new_signed_with_payer(
            &[instruction],
            Some(&context.payer.pubkey()),
            &[&context.payer, authority],
            blockhash,
        ))
        .await
}

fn instruction(authority: Pubkey, state: Pubkey, data: Vec<u8>) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(authority, true),
            AccountMeta::new(state, false),
        ],
        data,
    }
}

#[tokio::test]
async fn real_elf_executes_lean_plan_and_rolls_back_late_refusal() {
    require_sbf();
    let authority = Keypair::new();
    let state = Pubkey::new_unique();
    let mut test = ProgramTest::new("dclutch_effect_sbf", PROGRAM_ID, None);
    test.prefer_bpf(true);
    test.add_account(
        authority.pubkey(),
        Account::new(1_000_000, 0, &system_program::ID),
    );
    test.add_account(
        state,
        Account {
            lamports: Rent::default().minimum_balance(STATE_BYTES),
            data: projection(authority.pubkey()),
            owner: PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    let mut context = test.start_with_context().await;

    let plan = decode_hex(VECTOR_HEX);
    assert_eq!(plan.len(), PLAN_HEADER_BYTES + EFFECT_COUNT * EFFECT_BYTES);
    let compute_units = submit_with_cu(
        &mut context,
        instruction(authority.pubkey(), state, plan.clone()),
        &authority,
    )
    .await
    .expect("real Effect executor ELF accepts the Lean plan");
    assert!(compute_units > 0);
    eprintln!("isolated Lean Effect executor CU: {compute_units}");

    let post = context
        .banks_client
        .get_account(state)
        .await
        .expect("query")
        .expect("projection");
    assert_eq!(read_u64(&post.data, 0), 1);
    assert_eq!(read_u64(&post.data, 1), 1);
    assert_eq!(read_u64(&post.data, 2), 3_000);
    assert_eq!(read_u64(&post.data, 3), 2_200);
    assert_eq!(read_u64(&post.data, 4), 998);
    assert_eq!(read_u64(&post.data, 5), 1_100);
    assert_eq!(read_u64(&post.data, 6), 22);

    let before_refusal = post;
    let mut hostile = plan;
    let last_value = PLAN_HEADER_BYTES + (EFFECT_COUNT - 1) * EFFECT_BYTES + 8;
    hostile[last_value..last_value + 8].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(
        submit_refusal(
            &mut context,
            instruction(authority.pubkey(), state, hostile),
            &authority,
        )
        .await
        .is_err()
    );
    let after_refusal = context
        .banks_client
        .get_account(state)
        .await
        .expect("query")
        .expect("projection");
    assert_eq!(after_refusal, before_refusal);
}
