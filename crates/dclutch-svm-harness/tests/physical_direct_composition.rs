//! Real four-program SBF campaign for claim plus physical custody composition.
//!
//! The exact claim, controller, custody, and official SPL Token ELFs all execute.
//! No native processor or mock token implementation is registered.

use std::{env, path::PathBuf};

use dclutch_token_svm::LEGACY_TOKEN_PROGRAM_ID;
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
const CLAIM_PROGRAM_ID: Pubkey = Pubkey::new_from_array([81_u8; 32]);
const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([75_u8; 32]);
const CONTROLLER_SEED: &[u8] = b"dclutch-controller-v1";
const REPLAY_SEED: &[u8] = b"dclutch/direct-replay/v2";
const CLAIM_STATE_BYTES: usize = 80;
const JOURNAL_BYTES: usize = 16;
const TOKEN_ACCOUNT_BYTES: usize = 165;
const MINT_BYTES: usize = 82;
const CLAIM_VECTOR_HEX: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../formal/dclutch-semantics/vectors/direct-inline-ordinary-claims-v1.hex"
));
const CUSTODY_VECTOR_HEX: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../formal/dclutch-semantics/vectors/direct-inline-ordinary-custody-v1.hex"
));

struct TransactionResult {
    accepted: bool,
    compute_units: u64,
    logs: Vec<String>,
}

struct TokenTriplet {
    source: Pubkey,
    seller: Pubkey,
    venue: Pubkey,
}

fn token_program_id() -> Pubkey {
    Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID)
}

fn require_sbf() {
    let directory = env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required for real ELF tests");
    for artifact in [
        "dclutch_controller_proof_sbf.so",
        "dclutch_claims_proof_sbf.so",
        "dclutch_custody_proof_sbf.so",
        "spl_token.so",
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

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("u64 field"))
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("u32 field"))
}

fn claim_projection(authority: Pubkey) -> Vec<u8> {
    let mut bytes = vec![0_u8; CLAIM_STATE_BYTES];
    bytes[0..4].copy_from_slice(b"DCCS");
    bytes[4] = 1;
    bytes[8..40].copy_from_slice(authority.as_ref());
    bytes[40..44].copy_from_slice(&1_u32.to_le_bytes());
    for (index, value) in [0_u64, 0, 5_000, 200].into_iter().enumerate() {
        put_u64(&mut bytes, 48 + index * 8, value);
    }
    bytes
}

fn journal(counter: u64) -> Vec<u8> {
    let mut bytes = vec![0_u8; JOURNAL_BYTES];
    bytes[0..4].copy_from_slice(b"DCCJ");
    put_u64(&mut bytes, 8, counter);
    bytes
}

fn mint(supply: u64, decimals: u8) -> Vec<u8> {
    let mut bytes = vec![0_u8; MINT_BYTES];
    put_u64(&mut bytes, 36, supply);
    bytes[44] = decimals;
    bytes[45] = 1;
    bytes
}

fn token_account(
    mint: Pubkey,
    owner: Pubkey,
    amount: u64,
    delegate: Option<(Pubkey, u64)>,
    frozen: bool,
) -> Vec<u8> {
    let mut bytes = vec![0_u8; TOKEN_ACCOUNT_BYTES];
    bytes[0..32].copy_from_slice(mint.as_ref());
    bytes[32..64].copy_from_slice(owner.as_ref());
    put_u64(&mut bytes, 64, amount);
    if let Some((delegate, allowance)) = delegate {
        put_u32(&mut bytes, 72, 1);
        bytes[76..108].copy_from_slice(delegate.as_ref());
        put_u64(&mut bytes, 121, allowance);
    }
    bytes[108] = if frozen { 2 } else { 1 };
    bytes
}

#[allow(clippy::too_many_arguments)]
fn controller_instruction(
    controller: Pubkey,
    controller_bump: u8,
    replay: Pubkey,
    replay_bump: u8,
    market: Pubkey,
    generation: u64,
    maker: Pubkey,
    journal: Pubkey,
    projection: Pubkey,
    mint: Pubkey,
    tokens: &TokenTriplet,
    claim_plan: &[u8],
    custody_plan: &[u8],
) -> Instruction {
    let mut data = Vec::with_capacity(186);
    data.push(controller_bump);
    data.push(replay_bump);
    data.extend_from_slice(market.as_ref());
    data.extend_from_slice(&generation.to_le_bytes());
    data.extend_from_slice(maker.as_ref());
    data.extend_from_slice(claim_plan);
    data.extend_from_slice(custody_plan);
    assert_eq!(data.len(), 186);
    Instruction {
        program_id: CONTROLLER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(controller, false),
            AccountMeta::new_readonly(replay, false),
            AccountMeta::new(journal, false),
            AccountMeta::new(projection, false),
            AccountMeta::new_readonly(CLAIM_PROGRAM_ID, false),
            AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(tokens.source, false),
            AccountMeta::new(tokens.seller, false),
            AccountMeta::new(tokens.venue, false),
            AccountMeta::new_readonly(token_program_id(), false),
        ],
        data,
    }
}

fn direct_claim_instruction(controller: Pubkey, projection: Pubkey, plan: &[u8]) -> Instruction {
    Instruction {
        program_id: CLAIM_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(controller, false),
            AccountMeta::new(projection, false),
        ],
        data: plan.to_vec(),
    }
}

async fn submit(context: &mut ProgramTestContext, instruction: Instruction) -> TransactionResult {
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
    let metadata = processed.metadata.expect("transaction metadata");
    TransactionResult {
        accepted: processed.result.is_ok(),
        compute_units: metadata.compute_units_consumed,
        logs: metadata.log_messages,
    }
}

async fn account(context: &mut ProgramTestContext, address: Pubkey) -> Account {
    context
        .banks_client
        .get_account(address)
        .await
        .expect("query")
        .expect("account")
}

fn add_claim_account(test: &mut ProgramTest, key: Pubkey, controller: Pubkey) {
    test.add_account(
        key,
        Account {
            lamports: Rent::default().minimum_balance(CLAIM_STATE_BYTES),
            data: claim_projection(controller),
            owner: CLAIM_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn add_token_account(test: &mut ProgramTest, key: Pubkey, data: Vec<u8>) {
    test.add_account(
        key,
        Account {
            lamports: Rent::default().minimum_balance(TOKEN_ACCOUNT_BYTES),
            data,
            owner: token_program_id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

#[tokio::test]
async fn claims_and_two_real_token_transfers_commit_or_roll_back_together() {
    require_sbf();
    let claim_plan = decode_hex(CLAIM_VECTOR_HEX);
    let custody_plan = decode_hex(CUSTODY_VECTOR_HEX);
    assert_eq!(claim_plan.len(), 72);
    assert_eq!(custody_plan.len(), 40);

    let market = Pubkey::new_unique();
    let maker = Pubkey::new_unique();
    let generation = 3_u64;
    let generation_bytes = generation.to_le_bytes();
    let (controller, controller_bump) =
        Pubkey::find_program_address(&[CONTROLLER_SEED], &CONTROLLER_PROGRAM_ID);
    let (replay, replay_bump) = Pubkey::find_program_address(
        &[
            REPLAY_SEED,
            market.as_ref(),
            &generation_bytes,
            maker.as_ref(),
        ],
        &CONTROLLER_PROGRAM_ID,
    );
    let journal_key = Pubkey::new_unique();
    let success_claim = Pubkey::new_unique();
    let refusal_claim = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let success_tokens = TokenTriplet {
        source: Pubkey::new_unique(),
        seller: Pubkey::new_unique(),
        venue: Pubkey::new_unique(),
    };
    let refusal_tokens = TokenTriplet {
        source: Pubkey::new_unique(),
        seller: Pubkey::new_unique(),
        venue: Pubkey::new_unique(),
    };

    let mut test = ProgramTest::new("dclutch_controller_proof_sbf", CONTROLLER_PROGRAM_ID, None);
    test.prefer_bpf(true);
    test.add_program("dclutch_claims_proof_sbf", CLAIM_PROGRAM_ID, None);
    test.add_program("dclutch_custody_proof_sbf", CUSTODY_PROGRAM_ID, None);
    test.add_program("spl_token", token_program_id(), None);
    test.add_account(controller, Account::new(1_000_000, 0, &system_program::ID));
    test.add_account(replay, Account::new(1_000_000, 0, &system_program::ID));
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
    add_claim_account(&mut test, success_claim, controller);
    add_claim_account(&mut test, refusal_claim, controller);
    test.add_account(
        mint_key,
        Account {
            lamports: Rent::default().minimum_balance(MINT_BYTES),
            data: mint(20_000, 6),
            owner: token_program_id(),
            executable: false,
            rent_epoch: 0,
        },
    );
    add_token_account(
        &mut test,
        success_tokens.source,
        token_account(mint_key, maker, 2_000, Some((replay, 1_002)), false),
    );
    add_token_account(
        &mut test,
        success_tokens.seller,
        token_account(mint_key, Pubkey::new_unique(), 100, None, false),
    );
    add_token_account(
        &mut test,
        success_tokens.venue,
        token_account(mint_key, Pubkey::new_unique(), 20, None, false),
    );
    add_token_account(
        &mut test,
        refusal_tokens.source,
        token_account(mint_key, maker, 2_000, Some((replay, 1_002)), false),
    );
    add_token_account(
        &mut test,
        refusal_tokens.seller,
        token_account(mint_key, Pubkey::new_unique(), 100, None, false),
    );
    add_token_account(
        &mut test,
        refusal_tokens.venue,
        token_account(mint_key, Pubkey::new_unique(), 20, None, true),
    );
    let mut context = test.start_with_context().await;

    let direct = submit(
        &mut context,
        direct_claim_instruction(controller, success_claim, &claim_plan),
    )
    .await;
    assert!(
        !direct.accepted,
        "transaction caller cannot sign controller PDA"
    );

    let wrong_bump = submit(
        &mut context,
        controller_instruction(
            controller,
            controller_bump,
            replay,
            replay_bump.wrapping_add(1),
            market,
            generation,
            maker,
            journal_key,
            success_claim,
            mint_key,
            &success_tokens,
            &claim_plan,
            &custody_plan,
        ),
    )
    .await;
    assert!(!wrong_bump.accepted, "wrong replay coordinate must refuse");

    let success = submit(
        &mut context,
        controller_instruction(
            controller,
            controller_bump,
            replay,
            replay_bump,
            market,
            generation,
            maker,
            journal_key,
            success_claim,
            mint_key,
            &success_tokens,
            &claim_plan,
            &custody_plan,
        ),
    )
    .await;
    assert!(success.accepted, "complete physical plan must commit");
    assert_eq!(
        read_u64(&account(&mut context, journal_key).await.data, 8),
        1
    );
    let claims = account(&mut context, success_claim).await;
    assert_eq!(read_u64(&claims.data, 48), 1);
    assert_eq!(read_u64(&claims.data, 56), 1);
    assert_eq!(read_u64(&claims.data, 64), 3_000);
    assert_eq!(read_u64(&claims.data, 72), 2_200);
    let source = account(&mut context, success_tokens.source).await;
    assert_eq!(read_u64(&source.data, 64), 998);
    assert_eq!(read_u64(&source.data, 121), 0);
    assert_eq!(read_u32(&source.data, 72), 0);
    assert_eq!(
        read_u64(&account(&mut context, success_tokens.seller).await.data, 64),
        1_100
    );
    assert_eq!(
        read_u64(&account(&mut context, success_tokens.venue).await.data, 64),
        22
    );

    let journal_before = account(&mut context, journal_key).await;
    let claim_before = account(&mut context, refusal_claim).await;
    let source_before = account(&mut context, refusal_tokens.source).await;
    let seller_before = account(&mut context, refusal_tokens.seller).await;
    let venue_before = account(&mut context, refusal_tokens.venue).await;
    let late_refusal = submit(
        &mut context,
        controller_instruction(
            controller,
            controller_bump,
            replay,
            replay_bump,
            market,
            generation,
            maker,
            journal_key,
            refusal_claim,
            mint_key,
            &refusal_tokens,
            &claim_plan,
            &custody_plan,
        ),
    )
    .await;
    assert!(
        !late_refusal.accepted,
        "frozen venue must refuse after gross CPI"
    );
    let token_success = format!("Program {} success", token_program_id());
    assert!(
        late_refusal.logs.iter().any(|line| line == &token_success),
        "logs must prove the first real token CPI completed before refusal"
    );
    assert_eq!(account(&mut context, journal_key).await, journal_before);
    assert_eq!(account(&mut context, refusal_claim).await, claim_before);
    assert_eq!(
        account(&mut context, refusal_tokens.source).await,
        source_before
    );
    assert_eq!(
        account(&mut context, refusal_tokens.seller).await,
        seller_before
    );
    assert_eq!(
        account(&mut context, refusal_tokens.venue).await,
        venue_before
    );

    eprintln!(
        "physical Direct CU: impersonation={}, wrong replay={}, success={}, late second-destination rollback={}",
        direct.compute_units,
        wrong_bump.compute_units,
        success.compute_units,
        late_refusal.compute_units
    );
}
