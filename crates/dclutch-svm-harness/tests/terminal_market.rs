use std::{env, path::PathBuf};

use dclutch_collateral_contract::{COMPACT_TERMINAL_MARKET_BYTES, CompactTerminalMarketV1};
use dclutch_core_contract::{ContentId, MarketIdentity, MarketRoot, Phase};
use dclutch_market_contract::market::{CategoricalMarketV1, CategoricalSettlementSummaryV1};
use dclutch_product_contract::{ContentId as ProductContentId, terminal::ResolutionKind};
use dclutch_rent_contract::{
    CreateRentCreditV1, RENT_CREDIT_BYTES_V1, RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority,
    RentCreditV1,
};
use dclutch_terminal_contract::{TERMINAL_CATEGORICAL_MARKET_BYTES, TerminalCategoricalMarketV1};
use solana_account::Account;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{system_program, sysvar};
use solana_transaction::Transaction;

const PROGRAM_ID: Pubkey = Pubkey::new_from_array([71; 32]);
const GENERATION: u64 = 63;
const BENEFICIARY: Pubkey = Pubkey::new_from_array([94; 32]);
const MARKET_SURPLUS: u64 = 29;
const CREDIT_PAYER_OPENING: u64 = 1_000_000;
const MARKET_SEED: &[u8] = b"dclutch/market-root/v1";

struct Fixture {
    test: ProgramTest,
    market: Pubkey,
    rent_credit: Pubkey,
    rent_credit_state: RentCreditV1,
    rent_credit_payer: Keypair,
    market_before: Account,
}

fn require_sbf_out_dir() {
    let directory = env::var("SBF_OUT_DIR").expect(
        "SBF_OUT_DIR is required; build target/deploy/dclutch_sbf.so first, then run `SBF_OUT_DIR=../../target/deploy cargo test --test terminal_market`",
    );
    let artifact = PathBuf::from(directory).join("dclutch_sbf.so");
    assert!(
        artifact.is_file(),
        "SBF_OUT_DIR must contain the exact compiled dclutch_sbf.so artifact: {}",
        artifact.display()
    );
}

fn core_id(value: u8) -> ContentId {
    ContentId::new([value; 32]).expect("nonzero core identity")
}

fn product_id(value: u8) -> ProductContentId {
    ProductContentId::new([value; 32]).expect("nonzero Product identity")
}

fn retired_active_market() -> (Pubkey, Vec<u8>) {
    let identity = MarketIdentity::new(
        core_id(1),
        core_id(2),
        core_id(3),
        core_id(4),
        core_id(5),
        GENERATION,
    );
    let (market, _) = Pubkey::find_program_address(
        &[MARKET_SEED, &hash(&identity.to_bytes()).to_bytes()],
        &PROGRAM_ID,
    );
    let root = MarketRoot::founding(identity, BENEFICIARY.to_bytes()).expect("founding root");
    let mut active =
        CategoricalMarketV1::<2>::new(root, 0, [0; 2], CategoricalSettlementSummaryV1::empty())
            .expect("founding active market");
    active
        .transition_phase(GENERATION, Phase::Open)
        .expect("open active market");
    let settlement = CategoricalSettlementSummaryV1::resolved::<2>(
        product_id(6),
        ResolutionKind::Occurrence,
        0,
        1,
    )
    .expect("terminal settlement");
    active
        .resolve_with_summary(GENERATION, settlement)
        .expect("resolve active market");
    active
        .transition_phase(GENERATION, Phase::Retiring)
        .expect("retiring market");
    active
        .transition_phase(GENERATION, Phase::Retired)
        .expect("retired market");
    let mut bytes = vec![0; CategoricalMarketV1::<2>::encoded_len().expect("active width")];
    active.encode(&mut bytes).expect("canonical active bytes");
    (market, bytes)
}

fn fixture() -> Fixture {
    require_sbf_out_dir();
    let (market, data) = retired_active_market();
    let rent = Rent::default();
    let market_before = Account {
        lamports: rent
            .minimum_balance(data.len())
            .checked_add(MARKET_SURPLUS)
            .expect("bounded market surplus"),
        data,
        owner: PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };
    let authority = RefundAuthority::new(BENEFICIARY.to_bytes()).expect("beneficiary");
    let authority_bytes = authority.to_bytes();
    let (rent_credit, bump) = Pubkey::find_program_address(
        &[RENT_CREDIT_PDA_DOMAIN_V1, authority_bytes.as_slice()],
        &PROGRAM_ID,
    );
    let rent_credit_payer = Keypair::new();
    let mut test = ProgramTest::new("dclutch_sbf", PROGRAM_ID, None);
    test.prefer_bpf(true);
    test.add_account(market, market_before.clone());
    test.add_account(
        rent_credit_payer.pubkey(),
        Account::new(CREDIT_PAYER_OPENING, 0, &system_program::ID),
    );
    Fixture {
        test,
        market,
        rent_credit,
        rent_credit_state: RentCreditV1::new(authority, bump),
        rent_credit_payer,
        market_before,
    }
}

async fn submit(
    context: &mut ProgramTestContext,
    instruction: Instruction,
) -> Result<(), solana_program_test::BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    context.banks_client.process_transaction(transaction).await
}

async fn create_rent_credit(
    context: &mut ProgramTestContext,
    rent_credit: Pubkey,
    rent_credit_state: RentCreditV1,
) -> Account {
    let instruction = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new(rent_credit, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ],
        data: CreateRentCreditV1::new(
            rent_credit_state.refund_authority(),
            rent_credit_state.pda_bump(),
        )
        .to_bytes()
        .to_vec(),
    };
    submit(context, instruction)
        .await
        .expect("real routed RentCredit creation");
    context
        .banks_client
        .get_account(rent_credit)
        .await
        .expect("RentCredit query")
        .expect("RentCredit exists")
}

fn compact_instruction(fixture: &Fixture, generation: u64) -> Instruction {
    let mut data = [0; COMPACT_TERMINAL_MARKET_BYTES];
    CompactTerminalMarketV1::new(generation)
        .encode(&mut data)
        .expect("exact terminal compaction wire");
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(fixture.market, false),
            AccountMeta::new(fixture.rent_credit, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ],
        data: data.to_vec(),
    }
}

async fn account(context: &mut ProgramTestContext, key: Pubkey) -> Account {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account query")
        .expect("account exists")
}

#[tokio::test]
async fn routed_rent_credit_create_refuses_substituted_pda_and_rolls_back_payer() {
    let fixture = fixture();
    let substituted_credit = Pubkey::new_from_array([95; 32]);
    let rent_credit_state = fixture.rent_credit_state;
    let rent_credit_payer = fixture.rent_credit_payer;
    let mut context = fixture.test.start_with_context().await;
    let transaction_payer = context.payer.pubkey();
    let rent_credit_payer_key = rent_credit_payer.pubkey();
    let instruction = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(rent_credit_payer_key, true),
            AccountMeta::new(substituted_credit, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ],
        data: CreateRentCreditV1::new(
            rent_credit_state.refund_authority(),
            rent_credit_state.pda_bump(),
        )
        .to_bytes()
        .to_vec(),
    };
    let payer_before = account(&mut context, rent_credit_payer_key).await;
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("substituted create blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&transaction_payer),
        &[&context.payer, &rent_credit_payer],
        blockhash,
    );
    assert!(
        context
            .banks_client
            .process_transaction(transaction)
            .await
            .is_err(),
        "substituted RentCredit PDA must refuse"
    );
    assert_eq!(
        account(&mut context, rent_credit_payer_key).await,
        payer_before,
        "refused creation rolls back payer"
    );
    assert!(
        context
            .banks_client
            .get_account(substituted_credit)
            .await
            .expect("substituted credit query")
            .is_none(),
        "refused creation leaves substituted target vacant"
    );
}

#[tokio::test]
async fn terminal_compaction_resizes_same_pda_and_credits_exact_rent_delta() {
    let fixture = fixture();
    let instruction = compact_instruction(&fixture, GENERATION);
    let rent_credit = fixture.rent_credit;
    let rent_credit_state = fixture.rent_credit_state;
    let mut context = fixture.test.start_with_context().await;
    let credit_before = create_rent_credit(&mut context, rent_credit, rent_credit_state).await;
    assert_eq!(
        credit_before.lamports,
        Rent::default().minimum_balance(RENT_CREDIT_BYTES_V1)
    );
    assert_eq!(
        RentCreditV1::decode(&credit_before.data),
        Ok(fixture.rent_credit_state)
    );

    submit(&mut context, instruction.clone())
        .await
        .expect("real SVM same-PDA compaction");
    let market = account(&mut context, fixture.market).await;
    let credit = account(&mut context, fixture.rent_credit).await;
    let rent = Rent::default();
    let delta = rent.minimum_balance(fixture.market_before.data.len())
        - rent.minimum_balance(TERMINAL_CATEGORICAL_MARKET_BYTES);
    assert_eq!(market.owner, PROGRAM_ID);
    assert_eq!(market.data.len(), TERMINAL_CATEGORICAL_MARKET_BYTES);
    assert_eq!(
        market.lamports,
        rent.minimum_balance(TERMINAL_CATEGORICAL_MARKET_BYTES) + MARKET_SURPLUS
    );
    let terminal = TerminalCategoricalMarketV1::<2>::decode(&market.data)
        .expect("exact compact terminal Market");
    assert_eq!(terminal.root().phase(), Phase::Retired);
    assert_eq!(terminal.root().outstanding_children(), 0);
    assert_eq!(
        terminal.settlement(),
        CategoricalMarketV1::<2>::decode(&fixture.market_before.data)
            .expect("pre-compaction market")
            .settlement()
    );
    assert_eq!(credit.lamports, credit_before.lamports + delta);
    assert_eq!(
        RentCreditV1::decode(&credit.data),
        Ok(fixture.rent_credit_state)
    );

    let market_before_replay = market.clone();
    let credit_before_replay = credit.clone();
    assert!(
        submit(&mut context, instruction).await.is_err(),
        "terminal bytes reject replay"
    );
    assert_eq!(
        account(&mut context, fixture.market).await,
        market_before_replay
    );
    assert_eq!(
        account(&mut context, fixture.rent_credit).await,
        credit_before_replay
    );
}

#[tokio::test]
async fn terminal_compaction_wrong_generation_rolls_back_market_and_credit() {
    let fixture = fixture();
    let instruction = compact_instruction(&fixture, GENERATION + 1);
    let rent_credit = fixture.rent_credit;
    let rent_credit_state = fixture.rent_credit_state;
    let mut context = fixture.test.start_with_context().await;
    let credit_before = create_rent_credit(&mut context, rent_credit, rent_credit_state).await;
    assert!(
        submit(&mut context, instruction).await.is_err(),
        "wrong generation must refuse"
    );
    assert_eq!(
        account(&mut context, fixture.market).await,
        fixture.market_before
    );
    assert_eq!(
        account(&mut context, fixture.rent_credit).await,
        credit_before
    );
}
