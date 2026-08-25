//! Real-ELF adversarial campaign for the generated exact-account proof target.
//!
//! This remains projection evidence, not Direct lifecycle or SPL custody
//! evidence. No native processor or mock adapter is registered.

use std::{env, path::PathBuf};

use solana_account::Account;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::system_program;
use solana_transaction::Transaction;

const PROGRAM_ID: Pubkey = Pubkey::new_from_array([83; 32]);
const STATE_BYTES: usize = 104;
const PLAN_HEADER_BYTES: usize = 8;
const EFFECT_BYTES: usize = 16;
const EFFECT_COUNT: usize = 7;
const VECTOR_HEX: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../formal/dclutch-semantics/vectors/direct-inline-ordinary-v1.hex"
));

struct RefusalCase {
    name: &'static str,
    state: Pubkey,
    data: Vec<u8>,
    owner: Pubkey,
    plan: Vec<u8>,
    authority_writable: bool,
    projection_writable: bool,
}

fn require_sbf() {
    let directory = env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required for real ELF tests");
    assert!(
        PathBuf::from(directory)
            .join("dclutch_effect_proof_sbf.so")
            .is_file(),
        "SBF_OUT_DIR must contain dclutch_effect_proof_sbf.so"
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

fn instruction(
    authority: Pubkey,
    state: Pubkey,
    data: Vec<u8>,
    authority_writable: bool,
    projection_writable: bool,
) -> Instruction {
    let authority_meta = if authority_writable {
        AccountMeta::new(authority, true)
    } else {
        AccountMeta::new_readonly(authority, true)
    };
    let state_meta = if projection_writable {
        AccountMeta::new(state, false)
    } else {
        AccountMeta::new_readonly(state, false)
    };
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![authority_meta, state_meta],
        data,
    }
}

async fn submit_success(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    authority: &Keypair,
) -> u64 {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer, authority],
        blockhash,
    );
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("banks processing");
    processed.result.expect("proof-target success");
    processed
        .metadata
        .expect("transaction metadata")
        .compute_units_consumed
}

async fn submit_refusal(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    authority: &Keypair,
) -> u64 {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer, authority],
        blockhash,
    );
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("banks processing");
    assert!(processed.result.is_err(), "hostile frame must refuse");
    processed
        .metadata
        .expect("transaction metadata")
        .compute_units_consumed
}

fn mutate_plan(plan: &[u8], offset: usize, value: &[u8]) -> Vec<u8> {
    let mut mutated = plan.to_vec();
    mutated[offset..offset + value.len()].copy_from_slice(value);
    mutated
}

#[tokio::test]
async fn generated_exact_account_elf_executes_and_refuses_hostile_space() {
    require_sbf();
    let authority = Keypair::new();
    let success_state = Pubkey::new_unique();
    let plan = decode_hex(VECTOR_HEX);
    assert_eq!(plan.len(), PLAN_HEADER_BYTES + EFFECT_COUNT * EFFECT_BYTES);

    let mut cases = Vec::new();
    let mut push_case = |name: &'static str,
                         data: Vec<u8>,
                         owner: Pubkey,
                         hostile_plan: Vec<u8>,
                         authority_writable: bool,
                         projection_writable: bool| {
        cases.push(RefusalCase {
            name,
            state: Pubkey::new_unique(),
            data,
            owner,
            plan: hostile_plan,
            authority_writable,
            projection_writable,
        });
    };

    let mut reserved_state = projection(authority.pubkey());
    reserved_state[5] = 1;
    push_case(
        "noncanonical state padding",
        reserved_state,
        PROGRAM_ID,
        plan.clone(),
        false,
        true,
    );
    push_case(
        "stored authority mismatch",
        projection(Pubkey::new_unique()),
        PROGRAM_ID,
        plan.clone(),
        false,
        true,
    );
    push_case(
        "wrong state owner",
        projection(authority.pubkey()),
        system_program::ID,
        plan.clone(),
        false,
        true,
    );
    push_case(
        "writable authority",
        projection(authority.pubkey()),
        PROGRAM_ID,
        plan.clone(),
        true,
        true,
    );
    push_case(
        "readonly projection",
        projection(authority.pubkey()),
        PROGRAM_ID,
        plan.clone(),
        false,
        false,
    );
    push_case(
        "short instruction",
        projection(authority.pubkey()),
        PROGRAM_ID,
        plan[..plan.len() - 1].to_vec(),
        false,
        true,
    );
    push_case(
        "noncanonical plan header",
        projection(authority.pubkey()),
        PROGRAM_ID,
        mutate_plan(&plan, 6, &[1]),
        false,
        true,
    );
    push_case(
        "outcome mismatch",
        projection(authority.pubkey()),
        PROGRAM_ID,
        mutate_plan(&plan, 44, &2_u32.to_le_bytes()),
        false,
        true,
    );
    push_case(
        "claim nonconservation",
        projection(authority.pubkey()),
        PROGRAM_ID,
        mutate_plan(&plan, 64, &1_999_u64.to_le_bytes()),
        false,
        true,
    );
    push_case(
        "collateral nonconservation",
        projection(authority.pubkey()),
        PROGRAM_ID,
        mutate_plan(&plan, 112, &3_u64.to_le_bytes()),
        false,
        true,
    );
    let mut buyer_overflow = projection(authority.pubkey());
    put_u64(&mut buyer_overflow, 3, u64::MAX - 1_000);
    push_case(
        "buyer claim overflow",
        buyer_overflow,
        PROGRAM_ID,
        plan.clone(),
        false,
        true,
    );
    let mut late_overflow = projection(authority.pubkey());
    put_u64(&mut late_overflow, 6, u64::MAX);
    push_case(
        "late venue overflow",
        late_overflow,
        PROGRAM_ID,
        plan.clone(),
        false,
        true,
    );

    let mut test = ProgramTest::new("dclutch_effect_proof_sbf", PROGRAM_ID, None);
    test.prefer_bpf(true);
    test.add_account(
        authority.pubkey(),
        Account::new(1_000_000, 0, &system_program::ID),
    );
    test.add_account(
        success_state,
        Account {
            lamports: Rent::default().minimum_balance(STATE_BYTES),
            data: projection(authority.pubkey()),
            owner: PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    for case in &cases {
        test.add_account(
            case.state,
            Account {
                lamports: Rent::default().minimum_balance(STATE_BYTES),
                data: case.data.clone(),
                owner: case.owner,
                executable: false,
                rent_epoch: 0,
            },
        );
    }
    let mut context = test.start_with_context().await;

    let success_cu = submit_success(
        &mut context,
        instruction(authority.pubkey(), success_state, plan.clone(), false, true),
        &authority,
    )
    .await;
    eprintln!("generated exact-account Effect success CU: {success_cu}");
    let post = context
        .banks_client
        .get_account(success_state)
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

    for case in cases {
        let before = context
            .banks_client
            .get_account(case.state)
            .await
            .expect("query before")
            .expect("hostile projection");
        let refusal_cu = submit_refusal(
            &mut context,
            instruction(
                authority.pubkey(),
                case.state,
                case.plan,
                case.authority_writable,
                case.projection_writable,
            ),
            &authority,
        )
        .await;
        eprintln!("{} refusal CU: {refusal_cu}", case.name);
        let after = context
            .banks_client
            .get_account(case.state)
            .await
            .expect("query after")
            .expect("hostile projection");
        assert_eq!(after, before, "{} must roll back exactly", case.name);
    }
}
