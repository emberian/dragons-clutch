//! Real signed-intent, compiled-transition, claim, and custody SBF campaign.
//!
//! The native Ed25519 precompile plus exact controller, claim, custody, and
//! official SPL Token ELFs execute. No native processor or mock token is used.

use std::{env, path::PathBuf};

use dclutch_token_svm::LEGACY_TOKEN_PROGRAM_ID;
use solana_account::Account;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{ed25519_program, system_program, sysvar};
use solana_transaction::Transaction;

const CONTROLLER_PROGRAM_ID: Pubkey = Pubkey::new_from_array([67_u8; 32]);
const CLAIM_PROGRAM_ID: Pubkey = Pubkey::new_from_array([81_u8; 32]);
const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([75_u8; 32]);
const CONTROLLER_SEED: &[u8] = b"dclutch-controller-v1";
const REPLAY_SEED: &[u8] = b"dclutch/direct-replay/v3";
const POSITION_SEED: &[u8] = b"dclutch/position/v1";
const REPLAY_STATE_BYTES: usize = 48;
const POSITION_STATE_BYTES: usize = 56;
const JOURNAL_BYTES: usize = 16;
const TOKEN_ACCOUNT_BYTES: usize = 165;
const MINT_BYTES: usize = 82;
const PROFILE_BYTES: usize = 136;
const INTENT_BYTES: usize = 136;
const CONTROLLER_BYTES: usize = 304;
const GENERATION: u64 = 3;
const FILL: u64 = 2_000;
const PRICE: u64 = 500_000;
const PRICE_SCALE: u64 = 1_000_000;
const FEE_BPS: u16 = 25;

struct TransactionResult {
    accepted: bool,
    compute_units: u64,
    logs: Vec<String>,
}

#[derive(Clone, Copy)]
struct TokenTriplet {
    source: Pubkey,
    seller: Pubkey,
    venue: Pubkey,
}

#[derive(Clone, Copy)]
struct MarketFixture {
    profile: Pubkey,
    seller_replay: Pubkey,
    seller_bump: u8,
    buyer_replay: Pubkey,
    buyer_bump: u8,
    seller_position: Pubkey,
    seller_position_bump: u8,
    buyer_position: Pubkey,
    buyer_position_bump: u8,
    tokens: TokenTriplet,
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

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
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

fn replay_state(authority: Pubkey, nonce: u64) -> Vec<u8> {
    let mut bytes = vec![0_u8; REPLAY_STATE_BYTES];
    bytes[0..8].copy_from_slice(&[b'D', b'C', b'R', b'P', 1, 0, 0, 0]);
    bytes[8..40].copy_from_slice(authority.as_ref());
    put_u64(&mut bytes, 40, nonce);
    bytes
}

fn position_state(authority: Pubkey, outcome: u64, claims: u64) -> Vec<u8> {
    let mut bytes = vec![0_u8; POSITION_STATE_BYTES];
    bytes[0..8].copy_from_slice(&[b'D', b'C', b'P', b'N', 1, 0, 0, 0]);
    bytes[8..40].copy_from_slice(authority.as_ref());
    put_u64(&mut bytes, 40, outcome);
    put_u64(&mut bytes, 48, claims);
    bytes
}

fn journal(counter: u64) -> Vec<u8> {
    let mut bytes = vec![0_u8; JOURNAL_BYTES];
    bytes[0..4].copy_from_slice(b"DCCJ");
    put_u64(&mut bytes, 8, counter);
    bytes
}

fn market_profile(mint: Pubkey, venue: Pubkey) -> Vec<u8> {
    let mut bytes = vec![0_u8; PROFILE_BYTES];
    bytes[0..8].copy_from_slice(b"DCLTPRF1");
    put_u16(&mut bytes, 8, 1);
    bytes[10] = 1;
    bytes[11] = 2;
    put_u64(&mut bytes, 16, GENERATION);
    put_u64(&mut bytes, 24, PRICE_SCALE);
    put_u16(&mut bytes, 32, FEE_BPS);
    bytes[40..72].copy_from_slice(token_program_id().as_ref());
    bytes[72..104].copy_from_slice(mint.as_ref());
    bytes[104..136].copy_from_slice(venue.as_ref());
    bytes
}

fn compact_intent(market: Pubkey, collateral: Pubkey, side: u8, nonce: u64) -> [u8; INTENT_BYTES] {
    let mut bytes = [0_u8; INTENT_BYTES];
    bytes[0..8].copy_from_slice(b"DCLTDIR3");
    put_u16(&mut bytes, 8, 1);
    bytes[10] = side;
    bytes[11] = 1;
    bytes[12] = 0;
    bytes[16..48].copy_from_slice(market.as_ref());
    put_u64(&mut bytes, 48, GENERATION);
    put_u64(&mut bytes, 56, nonce);
    put_u64(&mut bytes, 64, 0);
    put_u64(&mut bytes, 72, u64::MAX);
    put_u64(&mut bytes, 80, FILL);
    put_u64(&mut bytes, 88, if side == 0 { 400_000 } else { 600_000 });
    put_u16(&mut bytes, 96, FEE_BPS);
    bytes[104..136].copy_from_slice(collateral.as_ref());
    bytes
}

#[allow(clippy::too_many_arguments)]
fn controller_data(controller_bump: u8, fixture: MarketFixture, nonce: u64) -> Vec<u8> {
    let mut bytes = vec![0_u8; CONTROLLER_BYTES];
    bytes[0..8].copy_from_slice(b"DCLTCTL1");
    put_u16(&mut bytes, 8, 1);
    bytes[10] = controller_bump;
    bytes[11] = fixture.seller_bump;
    bytes[12] = fixture.buyer_bump;
    bytes[13] = fixture.seller_position_bump;
    bytes[14] = fixture.buyer_position_bump;
    put_u64(&mut bytes, 16, FILL);
    put_u64(&mut bytes, 24, PRICE);
    bytes[32..168].copy_from_slice(&compact_intent(
        fixture.profile,
        fixture.tokens.seller,
        0,
        nonce,
    ));
    bytes[168..304].copy_from_slice(&compact_intent(
        fixture.profile,
        fixture.tokens.source,
        1,
        nonce,
    ));
    bytes
}

fn signed_ed25519_batch(seller: &Keypair, buyer: &Keypair, controller_data: &[u8]) -> Instruction {
    let payload = 2 + 2 * 14;
    let mut data = vec![0_u8; payload + 2 * 96];
    put_u16(&mut data, 0, 2);
    for (index, (maker, message_offset)) in [(seller, 32_usize), (buyer, 168_usize)]
        .into_iter()
        .enumerate()
    {
        let descriptor = 2 + index * 14;
        let public_key_offset = payload + index * 96;
        let signature_offset = public_key_offset + 32;
        put_u16(
            &mut data,
            descriptor,
            u16::try_from(signature_offset).expect("signature offset"),
        );
        put_u16(&mut data, descriptor + 2, u16::MAX);
        put_u16(
            &mut data,
            descriptor + 4,
            u16::try_from(public_key_offset).expect("public-key offset"),
        );
        put_u16(&mut data, descriptor + 6, u16::MAX);
        put_u16(
            &mut data,
            descriptor + 8,
            u16::try_from(message_offset).expect("message offset"),
        );
        put_u16(
            &mut data,
            descriptor + 10,
            u16::try_from(INTENT_BYTES).expect("message length"),
        );
        put_u16(&mut data, descriptor + 12, 1);
        data[public_key_offset..public_key_offset + 32].copy_from_slice(maker.pubkey().as_ref());
        let message = &controller_data[message_offset..message_offset + INTENT_BYTES];
        data[signature_offset..signature_offset + 64]
            .copy_from_slice(maker.sign_message(message).as_ref());
    }
    Instruction {
        program_id: ed25519_program::ID,
        accounts: vec![],
        data,
    }
}

#[allow(clippy::too_many_arguments)]
fn controller_instruction(
    controller: Pubkey,
    journal: Pubkey,
    fixture: MarketFixture,
    mint: Pubkey,
    data: Vec<u8>,
) -> Instruction {
    Instruction {
        program_id: CONTROLLER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(controller, false),
            AccountMeta::new(fixture.seller_replay, false),
            AccountMeta::new(fixture.buyer_replay, false),
            AccountMeta::new(journal, false),
            AccountMeta::new(fixture.seller_position, false),
            AccountMeta::new(fixture.buyer_position, false),
            AccountMeta::new_readonly(CLAIM_PROGRAM_ID, false),
            AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.profile, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(fixture.tokens.source, false),
            AccountMeta::new(fixture.tokens.seller, false),
            AccountMeta::new(fixture.tokens.venue, false),
            AccountMeta::new_readonly(token_program_id(), false),
            AccountMeta::new_readonly(sysvar::instructions::ID, false),
        ],
        data,
    }
}

fn direct_claim_instruction(controller: Pubkey, fixture: MarketFixture) -> Instruction {
    let mut plan = vec![0_u8; 72];
    plan[0..8].copy_from_slice(&[b'D', b'C', b'E', b'F', 1, 4, 0, 0]);
    Instruction {
        program_id: CLAIM_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(controller, false),
            AccountMeta::new(fixture.seller_replay, false),
            AccountMeta::new(fixture.buyer_replay, false),
            AccountMeta::new(fixture.seller_position, false),
            AccountMeta::new(fixture.buyer_position, false),
        ],
        data: plan,
    }
}

async fn submit(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
) -> TransactionResult {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = Transaction::new_signed_with_payer(
        instructions,
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

async fn claim_state_accounts(
    context: &mut ProgramTestContext,
    fixture: MarketFixture,
) -> [Account; 4] {
    [
        account(context, fixture.seller_replay).await,
        account(context, fixture.buyer_replay).await,
        account(context, fixture.seller_position).await,
        account(context, fixture.buyer_position).await,
    ]
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

fn add_program_account(test: &mut ProgramTest, key: Pubkey) {
    test.add_account(key, Account::new(1_000_000, 0, &system_program::ID));
}

fn add_claim_state(test: &mut ProgramTest, key: Pubkey, data: Vec<u8>) {
    test.add_account(
        key,
        Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
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

fn market_fixture(
    market: Pubkey,
    seller: Pubkey,
    buyer: Pubkey,
    tokens: TokenTriplet,
) -> MarketFixture {
    let generation = GENERATION.to_le_bytes();
    let (seller_replay, seller_bump) = Pubkey::find_program_address(
        &[REPLAY_SEED, market.as_ref(), &generation, seller.as_ref()],
        &CONTROLLER_PROGRAM_ID,
    );
    let (buyer_replay, buyer_bump) = Pubkey::find_program_address(
        &[REPLAY_SEED, market.as_ref(), &generation, buyer.as_ref()],
        &CONTROLLER_PROGRAM_ID,
    );
    let outcome = [1_u8];
    let (seller_position, seller_position_bump) = Pubkey::find_program_address(
        &[POSITION_SEED, market.as_ref(), seller.as_ref(), &outcome],
        &CONTROLLER_PROGRAM_ID,
    );
    let (buyer_position, buyer_position_bump) = Pubkey::find_program_address(
        &[POSITION_SEED, market.as_ref(), buyer.as_ref(), &outcome],
        &CONTROLLER_PROGRAM_ID,
    );
    MarketFixture {
        profile: market,
        seller_replay,
        seller_bump,
        buyer_replay,
        buyer_bump,
        seller_position,
        seller_position_bump,
        buyer_position,
        buyer_position_bump,
        tokens,
    }
}

#[tokio::test]
async fn signed_intents_compile_to_claims_and_real_token_transfers_atomically() {
    require_sbf();
    let seller_maker = Keypair::new();
    let buyer_maker = Keypair::new();
    let (controller, controller_bump) =
        Pubkey::find_program_address(&[CONTROLLER_SEED], &CONTROLLER_PROGRAM_ID);
    let journal_key = Pubkey::new_unique();
    let mint_key = Pubkey::new_unique();
    let success = market_fixture(
        Pubkey::new_unique(),
        seller_maker.pubkey(),
        buyer_maker.pubkey(),
        TokenTriplet {
            source: Pubkey::new_unique(),
            seller: Pubkey::new_unique(),
            venue: Pubkey::new_unique(),
        },
    );
    let refusal = market_fixture(
        Pubkey::new_unique(),
        seller_maker.pubkey(),
        buyer_maker.pubkey(),
        TokenTriplet {
            source: Pubkey::new_unique(),
            seller: Pubkey::new_unique(),
            venue: Pubkey::new_unique(),
        },
    );

    let mut test = ProgramTest::new("dclutch_controller_proof_sbf", CONTROLLER_PROGRAM_ID, None);
    test.prefer_bpf(true);
    test.add_program("dclutch_claims_proof_sbf", CLAIM_PROGRAM_ID, None);
    test.add_program("dclutch_custody_proof_sbf", CUSTODY_PROGRAM_ID, None);
    test.add_program("spl_token", token_program_id(), None);
    add_program_account(&mut test, controller);
    for fixture in [success, refusal] {
        add_claim_state(
            &mut test,
            fixture.seller_replay,
            replay_state(controller, 0),
        );
        add_claim_state(&mut test, fixture.buyer_replay, replay_state(controller, 0));
        add_claim_state(
            &mut test,
            fixture.seller_position,
            position_state(controller, 1, 5_000),
        );
        add_claim_state(
            &mut test,
            fixture.buyer_position,
            position_state(controller, 1, 200),
        );
    }
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
    for fixture in [success, refusal] {
        test.add_account(
            fixture.profile,
            Account {
                lamports: Rent::default().minimum_balance(PROFILE_BYTES),
                data: market_profile(mint_key, fixture.tokens.venue),
                owner: CONTROLLER_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        );
    }
    test.add_account(
        mint_key,
        Account {
            lamports: Rent::default().minimum_balance(MINT_BYTES),
            data: mint(40_000, 6),
            owner: token_program_id(),
            executable: false,
            rent_epoch: 0,
        },
    );
    for (fixture, frozen) in [(success, false), (refusal, true)] {
        add_token_account(
            &mut test,
            fixture.tokens.source,
            token_account(
                mint_key,
                buyer_maker.pubkey(),
                2_000,
                Some((fixture.buyer_replay, 1_002)),
                false,
            ),
        );
        add_token_account(
            &mut test,
            fixture.tokens.seller,
            token_account(mint_key, seller_maker.pubkey(), 100, None, false),
        );
        add_token_account(
            &mut test,
            fixture.tokens.venue,
            token_account(mint_key, Pubkey::new_unique(), 20, None, frozen),
        );
    }
    let mut context = test.start_with_context().await;

    let direct = submit(
        &mut context,
        &[direct_claim_instruction(controller, success)],
    )
    .await;
    assert!(
        !direct.accepted,
        "transaction caller cannot sign controller PDA"
    );

    let untouched_journal = account(&mut context, journal_key).await;
    let untouched_claims = claim_state_accounts(&mut context, success).await;
    let untouched_source = account(&mut context, success.tokens.source).await;
    let untouched_seller = account(&mut context, success.tokens.seller).await;
    let untouched_venue = account(&mut context, success.tokens.venue).await;

    let mut wrong_data = controller_data(controller_bump, success, 0);
    wrong_data[12] = success.buyer_bump.wrapping_add(1);
    let wrong = controller_instruction(
        controller,
        journal_key,
        success,
        mint_key,
        wrong_data.clone(),
    );
    let wrong_bump = submit(
        &mut context,
        &[
            signed_ed25519_batch(&seller_maker, &buyer_maker, &wrong_data),
            wrong,
        ],
    )
    .await;
    assert!(!wrong_bump.accepted, "wrong replay coordinate must refuse");
    assert_eq!(account(&mut context, journal_key).await, untouched_journal);
    assert_eq!(
        claim_state_accounts(&mut context, success).await,
        untouched_claims
    );
    assert_eq!(
        account(&mut context, success.tokens.source).await,
        untouched_source
    );
    assert_eq!(
        account(&mut context, success.tokens.seller).await,
        untouched_seller
    );
    assert_eq!(
        account(&mut context, success.tokens.venue).await,
        untouched_venue
    );

    let mut wrong_position_data = controller_data(controller_bump, success, 0);
    wrong_position_data[14] = success.buyer_position_bump.wrapping_add(1);
    let wrong_position = submit(
        &mut context,
        &[
            signed_ed25519_batch(&seller_maker, &buyer_maker, &wrong_position_data),
            controller_instruction(
                controller,
                journal_key,
                success,
                mint_key,
                wrong_position_data,
            ),
        ],
    )
    .await;
    assert!(
        !wrong_position.accepted,
        "wrong maker/outcome Position coordinate must refuse"
    );
    assert_eq!(
        claim_state_accounts(&mut context, success).await,
        untouched_claims
    );
    assert_eq!(account(&mut context, journal_key).await, untouched_journal);

    let mut bad_price_data = controller_data(controller_bump, success, 0);
    put_u64(&mut bad_price_data, 24, 399_999);
    let bad_price = submit(
        &mut context,
        &[
            signed_ed25519_batch(&seller_maker, &buyer_maker, &bad_price_data),
            controller_instruction(controller, journal_key, success, mint_key, bad_price_data),
        ],
    )
    .await;
    assert!(
        !bad_price.accepted,
        "matcher price below the signed seller limit must refuse"
    );
    assert_eq!(account(&mut context, journal_key).await, untouched_journal);
    assert_eq!(
        claim_state_accounts(&mut context, success).await,
        untouched_claims
    );
    assert_eq!(
        account(&mut context, success.tokens.source).await,
        untouched_source
    );

    let signed_data = controller_data(controller_bump, success, 0);
    let signature_batch = signed_ed25519_batch(&seller_maker, &buyer_maker, &signed_data);
    let mut tampered_data = signed_data;
    tampered_data[32 + 96] ^= 1;
    let tampered = submit(
        &mut context,
        &[
            signature_batch,
            controller_instruction(controller, journal_key, success, mint_key, tampered_data),
        ],
    )
    .await;
    assert!(
        !tampered.accepted,
        "mutating a signed fee-rate byte must fail native Ed25519 verification"
    );
    assert_eq!(account(&mut context, journal_key).await, untouched_journal);
    assert_eq!(
        claim_state_accounts(&mut context, success).await,
        untouched_claims
    );
    assert_eq!(
        account(&mut context, success.tokens.source).await,
        untouched_source
    );

    let success_data = controller_data(controller_bump, success, 0);
    let success_result = submit(
        &mut context,
        &[
            signed_ed25519_batch(&seller_maker, &buyer_maker, &success_data),
            controller_instruction(controller, journal_key, success, mint_key, success_data),
        ],
    )
    .await;
    assert!(
        success_result.accepted,
        "compiled physical fill must commit"
    );
    assert_eq!(
        read_u64(&account(&mut context, journal_key).await.data, 8),
        1
    );
    let claims = claim_state_accounts(&mut context, success).await;
    assert_eq!(read_u64(&claims[0].data, 40), 1);
    assert_eq!(read_u64(&claims[1].data, 40), 1);
    assert_eq!(read_u64(&claims[2].data, 48), 3_000);
    assert_eq!(read_u64(&claims[3].data, 48), 2_200);
    let source = account(&mut context, success.tokens.source).await;
    assert_eq!(read_u64(&source.data, 64), 998);
    assert_eq!(read_u64(&source.data, 121), 0);
    assert_eq!(read_u32(&source.data, 72), 0);
    assert_eq!(
        read_u64(&account(&mut context, success.tokens.seller).await.data, 64),
        1_100
    );
    assert_eq!(
        read_u64(&account(&mut context, success.tokens.venue).await.data, 64),
        22
    );

    let journal_before = account(&mut context, journal_key).await;
    let claim_before = claim_state_accounts(&mut context, refusal).await;
    let source_before = account(&mut context, refusal.tokens.source).await;
    let seller_before = account(&mut context, refusal.tokens.seller).await;
    let venue_before = account(&mut context, refusal.tokens.venue).await;
    let refusal_data = controller_data(controller_bump, refusal, 0);
    let late_refusal = submit(
        &mut context,
        &[
            signed_ed25519_batch(&seller_maker, &buyer_maker, &refusal_data),
            controller_instruction(controller, journal_key, refusal, mint_key, refusal_data),
        ],
    )
    .await;
    assert!(
        !late_refusal.accepted,
        "frozen venue must refuse after gross CPI"
    );
    let token_success = format!("Program {} success", token_program_id());
    assert!(
        late_refusal.logs.iter().any(|line| line == &token_success),
        "logs must prove first official Token CPI completed before refusal"
    );
    assert_eq!(account(&mut context, journal_key).await, journal_before);
    assert_eq!(
        claim_state_accounts(&mut context, refusal).await,
        claim_before
    );
    assert_eq!(
        account(&mut context, refusal.tokens.source).await,
        source_before
    );
    assert_eq!(
        account(&mut context, refusal.tokens.seller).await,
        seller_before
    );
    assert_eq!(
        account(&mut context, refusal.tokens.venue).await,
        venue_before
    );

    eprintln!(
        "compiled signed Direct CU: impersonation={}, wrong replay={}, wrong position={}, bad price={}, tamper={}, success={}, late rollback={}",
        direct.compute_units,
        wrong_bump.compute_units,
        wrong_position.compute_units,
        bad_price.compute_units,
        tampered.compute_units,
        success_result.compute_units,
        late_refusal.compute_units
    );
}
