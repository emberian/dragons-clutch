use std::{env, path::PathBuf};

use dclutch_core_contract::{ContentId, MarketIdentity, MarketRoot, Phase};
use dclutch_kernel::resolution::categorical_pyth_v1::{
    CategoricalPythV1PolicyInput, MAX_PRICE_CELLS,
};
use dclutch_pyth_contract::{
    feed_profile::PythFeedProfileV1,
    funding::{FUNDING_BYTES, ResolutionFundV1},
    instruction::{RESOLVE_FAILURE_BYTES, ResolveCategoricalFailureV1},
    market::MarketStateV1,
    policy::CategoricalPythPolicyRecordV1,
    receipt::{ReceiptKind, ResolutionReceiptV1},
};
use solana_account::Account;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::ProgramTest;
use solana_sdk::signature::Signer;
use solana_sdk_ids::system_program;
use solana_transaction::Transaction;

const PROGRAM_ID: Pubkey = Pubkey::new_from_array([71; 32]);
const GENERATION: u64 = 17;
const PROVIDER_REIMBURSEMENT: u64 = 29;
const BOUNTY: u64 = 31;
const SPONSOR_EXCESS: u64 = 37;
const BOUNTY_OPENING: u64 = 41;
const SPONSOR_OPENING: u64 = 43;

struct Fixture {
    test: ProgramTest,
    market: Pubkey,
    fund: Pubkey,
    bounty: Pubkey,
    sponsor: Pubkey,
    market_before: Vec<u8>,
    fund_before: Account,
    bounty_before: u64,
    sponsor_before: u64,
}

fn require_sbf_out_dir() {
    let directory = env::var("SBF_OUT_DIR").expect(
        "SBF_OUT_DIR is required; build target/deploy/dclutch_sbf.so first, then run `SBF_OUT_DIR=../../target/deploy cargo test --test failure_route`",
    );
    let artifact = PathBuf::from(directory).join("dclutch_sbf.so");
    assert!(
        artifact.is_file(),
        "SBF_OUT_DIR must contain the exact compiled dclutch_sbf.so artifact: {}",
        artifact.display()
    );
}

fn policy(target_time: i64, window: u32) -> CategoricalPythPolicyRecordV1 {
    let edges = [0u128; MAX_PRICE_CELLS];
    let profile = PythFeedProfileV1::new([4; 32], [5; 32], [6; 32]).expect("profile");
    CategoricalPythPolicyRecordV1::new(CategoricalPythV1PolicyInput {
        pyth_release_id: [1; 32],
        feed_profile_id: hash(&profile.to_bytes()).to_bytes(),
        target_time,
        grace: 0,
        window,
        max_crossing_lag: 1,
        max_age: 1,
        max_future_skew: 1,
        confidence_multiplier: 1,
        max_confidence_bps: 1,
        max_normalized_confidence_atoms: 1,
        normalized_decimals: 0,
        price_cell_count: 1,
        upper_edges: edges,
        failure_outcome_index: 1,
    })
    .expect("failure-eligible policy")
}

fn fixture(target_time: i64, window: u32) -> Fixture {
    require_sbf_out_dir();
    let policy = policy(target_time, window);
    let feed = PythFeedProfileV1::new([4; 32], [5; 32], [6; 32]).expect("feed");
    let identity = MarketIdentity::new(
        ContentId::new([11; 32]).expect("realm"),
        ContentId::new([12; 32]).expect("terms"),
        ContentId::new([13; 32]).expect("basis"),
        ContentId::new(hash(&policy.to_bytes()).to_bytes()).expect("policy id"),
        ContentId::new([14; 32]).expect("capability manifest"),
        GENERATION,
    );
    let (market, _) = Pubkey::find_program_address(
        &[
            b"dclutch/market-root/v1",
            &hash(&identity.to_bytes()).to_bytes(),
        ],
        &PROGRAM_ID,
    );
    let (fund, _) = Pubkey::find_program_address(
        &[b"dclutch/resolution-fund/v1", market.as_ref()],
        &PROGRAM_ID,
    );
    let bounty = Pubkey::new_from_array([81; 32]);
    let sponsor = Pubkey::new_from_array([82; 32]);

    let mut root = MarketRoot::founding(identity, [15; 32]).expect("founding root");
    root.transition_phase(GENERATION, Phase::Open)
        .expect("open");
    root.register_child(GENERATION, 0).expect("fund child");
    root.register_child(GENERATION, 1)
        .expect("surviving vault placeholder");
    let market_state = MarketStateV1::<2>::new(
        root,
        policy,
        feed,
        101,
        [101, 101],
        ResolutionReceiptV1::empty(2).expect("empty receipt"),
    )
    .expect("market state");
    let mut market_before = vec![0; MarketStateV1::<2>::encoded_len().expect("market length")];
    market_state
        .encode(&mut market_before)
        .expect("market bytes");

    let fund_state = ResolutionFundV1::new(
        market.to_bytes(),
        GENERATION,
        sponsor.to_bytes(),
        PROVIDER_REIMBURSEMENT,
        BOUNTY,
    )
    .expect("fund state");
    let rent = Rent::default().minimum_balance(FUNDING_BYTES);
    let fund_lamports = rent + PROVIDER_REIMBURSEMENT + BOUNTY + SPONSOR_EXCESS;
    let fund_before = Account {
        lamports: fund_lamports,
        data: fund_state.to_bytes().to_vec(),
        owner: PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };

    let mut test = ProgramTest::new("dclutch_sbf", PROGRAM_ID, None);
    test.prefer_bpf(true);
    test.add_account(
        market,
        Account {
            lamports: 1_000_000,
            data: market_before.clone(),
            owner: PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    test.add_account(fund, fund_before.clone());
    test.add_account(bounty, Account::new(BOUNTY_OPENING, 0, &system_program::ID));
    test.add_account(
        sponsor,
        Account::new(SPONSOR_OPENING, 0, &system_program::ID),
    );
    Fixture {
        test,
        market,
        fund,
        bounty,
        sponsor,
        market_before,
        fund_before,
        bounty_before: BOUNTY_OPENING,
        sponsor_before: SPONSOR_OPENING,
    }
}

fn failure_instruction(fixture: &Fixture) -> Instruction {
    let mut data = [0; RESOLVE_FAILURE_BYTES];
    ResolveCategoricalFailureV1::new(GENERATION, 2)
        .encode(&mut data)
        .expect("failure encoding");
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(fixture.bounty, false),
            AccountMeta::new(fixture.market, false),
            AccountMeta::new(fixture.fund, false),
            AccountMeta::new(fixture.sponsor, false),
        ],
        data: data.to_vec(),
    }
}

async fn submit(
    context: &mut solana_program_test::ProgramTestContext,
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

#[tokio::test]
async fn body_free_failure_resolves_closes_fund_and_refuses_replay() {
    let fixture = fixture(-1_000_000, 1);
    let instruction = failure_instruction(&fixture);
    let mut context = fixture.test.start_with_context().await;
    submit(&mut context, instruction.clone())
        .await
        .expect("eligible failure resolves");

    let market = context
        .banks_client
        .get_account(fixture.market)
        .await
        .expect("market query")
        .expect("market exists");
    let state = MarketStateV1::<2>::decode(&market.data).expect("resolved market");
    assert_eq!(state.root().phase(), Phase::Resolved);
    assert_eq!(state.root().outstanding_children(), 1);
    assert_eq!(state.receipt().kind(), ReceiptKind::Failure);
    assert_eq!(state.receipt().winner(), 1);
    assert_eq!(state.hoard_atoms(), 101, "hoard principal is untouched");

    // `close_fund` sets lamports to zero, truncates data, then assigns System.
    // ProgramTest's Bank immediately purges that exact zero-lamport System
    // account, so absence is the observable terminal representation (rather
    // than an Account value with 0 lamports, empty data, and System owner).
    assert!(
        context
            .banks_client
            .get_account(fixture.fund)
            .await
            .expect("fund query")
            .is_none(),
        "the drained, empty, System-owned fund is purged by the real runtime"
    );
    let bounty = context
        .banks_client
        .get_account(fixture.bounty)
        .await
        .expect("bounty query")
        .expect("bounty exists");
    let sponsor = context
        .banks_client
        .get_account(fixture.sponsor)
        .await
        .expect("sponsor query")
        .expect("sponsor exists");
    let rent = Rent::default().minimum_balance(FUNDING_BYTES);
    assert_eq!(bounty.lamports, fixture.bounty_before + BOUNTY);
    assert_eq!(
        sponsor.lamports,
        fixture.sponsor_before + rent + PROVIDER_REIMBURSEMENT + SPONSOR_EXCESS
    );
    assert!(
        submit(&mut context, instruction).await.is_err(),
        "terminal market rejects replay"
    );
}

#[tokio::test]
async fn early_failure_rolls_back_every_touched_account() {
    let fixture = fixture(9_999_999_999, 1);
    let instruction = failure_instruction(&fixture);
    let mut context = fixture.test.start_with_context().await;
    assert!(
        submit(&mut context, instruction).await.is_err(),
        "early deadline must refuse"
    );
    let market = context
        .banks_client
        .get_account(fixture.market)
        .await
        .expect("market query")
        .expect("market exists");
    let fund = context
        .banks_client
        .get_account(fixture.fund)
        .await
        .expect("fund query")
        .expect("fund exists");
    let bounty = context
        .banks_client
        .get_account(fixture.bounty)
        .await
        .expect("bounty query")
        .expect("bounty exists");
    let sponsor = context
        .banks_client
        .get_account(fixture.sponsor)
        .await
        .expect("sponsor query")
        .expect("sponsor exists");
    assert_eq!(market.data, fixture.market_before);
    assert_eq!(fund.lamports, fixture.fund_before.lamports);
    assert_eq!(fund.data, fixture.fund_before.data);
    assert_eq!(fund.owner, fixture.fund_before.owner);
    assert_eq!(bounty.lamports, fixture.bounty_before);
    assert_eq!(sponsor.lamports, fixture.sponsor_before);
}
