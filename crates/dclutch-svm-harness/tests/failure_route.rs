use std::{env, path::PathBuf};

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityManifestV1,
    FundingAmountsV1, MANIFEST_HEADER_BYTES,
};
use dclutch_core_contract::{ContentId, MarketIdentity, MarketRoot, Phase};
use dclutch_kernel::resolution::categorical_pyth_v1::{
    CategoricalPythV1PolicyInput, MAX_PRICE_CELLS,
};
use dclutch_market_contract::market::{CategoricalMarketV1, CategoricalSettlementSummaryV1};
use dclutch_product_contract::terminal::ResolutionKind;
use dclutch_pyth_contract::{
    feed_profile::PythFeedProfileV1,
    funding::{
        FUNDING_BYTES, construct_required_resolution_funding, required_resolution_minimum_balance,
    },
    instruction::{RESOLVE_FAILURE_BYTES, ResolveCategoricalFailureV1},
    policy::CategoricalPythPolicyRecordV1,
    resolution_material::CategoricalPythResolutionMaterialV1,
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
    material: Pubkey,
    manifest: Pubkey,
    substitute_material: Pubkey,
    substitute_manifest: Pubkey,
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
        "missing SBF artifact: {}",
        artifact.display()
    );
}

fn policy(target_time: i64, window: u32) -> CategoricalPythPolicyRecordV1 {
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
        upper_edges: [0; MAX_PRICE_CELLS],
        failure_outcome_index: 1,
    })
    .expect("failure-eligible policy")
}

fn account(lamports: u64, data: Vec<u8>, owner: Pubkey) -> Account {
    Account {
        lamports,
        data,
        owner,
        executable: false,
        rent_epoch: 0,
    }
}

fn manifest_bytes(policy_id: ContentId, fund_rent: u64) -> Vec<u8> {
    let entry = CapabilityEntryV1::new(
        ContentId::new([21; 32]).expect("kind"),
        ContentId::new([1; 32]).expect("Pyth release"),
        policy_id,
        ContentId::new([22; 32]).expect("capacity"),
        ContentId::new([23; 32]).expect("child schema"),
        ContentId::new([24; 32]).expect("derivation"),
        ActivationPolicy::RequiredAtFounding,
        0,
        0,
        [0; 16],
        FundingAmountsV1::new(fund_rent, 0, 0, PROVIDER_REIMBURSEMENT, BOUNTY, 0, 0)
            .expect("resolution quote"),
    )
    .expect("one required resolution capability");
    let mut bytes = vec![0; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&[entry], &mut bytes).expect("canonical manifest");
    bytes
}

fn fixture(target_time: i64, window: u32, underfunded: bool) -> Fixture {
    require_sbf_out_dir();
    let policy_record = policy(target_time, window);
    let feed = PythFeedProfileV1::new([4; 32], [5; 32], [6; 32]).expect("feed");
    let material = CategoricalPythResolutionMaterialV1::new(policy_record, feed).expect("material");
    let material_bytes = material.to_bytes();
    let fund_rent = Rent::default().minimum_balance(FUNDING_BYTES);
    let policy_id = ContentId::new(hash(&policy_record.to_bytes()).to_bytes()).expect("policy id");
    let manifest_bytes = manifest_bytes(policy_id, fund_rent);
    let manifest_id = ContentId::new(hash(&manifest_bytes).to_bytes()).expect("manifest id");
    let manifest = CapabilityManifestV1::decode(&manifest_bytes).expect("manifest decode");
    let selected = manifest
        .required_founding_entry_for_config(policy_id)
        .expect("selected entry");
    let fund_state =
        construct_required_resolution_funding(manifest_id, manifest, selected, fund_rent, 1)
            .expect("active raw funding state");
    let identity = MarketIdentity::new(
        ContentId::new([11; 32]).expect("realm"),
        ContentId::new([12; 32]).expect("terms"),
        ContentId::new([13; 32]).expect("basis"),
        policy_id,
        manifest_id,
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
    let material_key = Pubkey::new_from_array([83; 32]);
    let manifest_key = Pubkey::new_from_array([84; 32]);
    let substitute_material = Pubkey::new_from_array([85; 32]);
    let substitute_manifest = Pubkey::new_from_array([86; 32]);

    let mut root = MarketRoot::founding(identity, sponsor.to_bytes()).expect("founding root");
    root.transition_phase(GENERATION, Phase::Open)
        .expect("open");
    root.register_child(GENERATION, 0).expect("fund child");
    root.register_child(GENERATION, 1)
        .expect("surviving vault placeholder");
    let market_state = CategoricalMarketV1::<2>::new(
        root,
        101,
        [101, 101],
        CategoricalSettlementSummaryV1::empty(),
    )
    .expect("provider-neutral market state");
    let mut market_before =
        vec![0; CategoricalMarketV1::<2>::encoded_len().expect("market length")];
    market_state
        .encode(&mut market_before)
        .expect("market bytes");

    let minimum = required_resolution_minimum_balance(fund_state).expect("fund minimum");
    let fund_lamports = if underfunded {
        minimum.checked_sub(1).expect("positive funding")
    } else {
        minimum.checked_add(SPONSOR_EXCESS).expect("fund excess")
    };
    let fund_before = account(fund_lamports, fund_state.to_bytes().to_vec(), PROGRAM_ID);
    let wrong_material = CategoricalPythResolutionMaterialV1::new(policy(7, window), feed)
        .expect("substitution material")
        .to_bytes()
        .to_vec();
    let empty_manifest = CapabilityManifestV1::empty()
        .expect("empty manifest")
        .as_bytes()
        .to_vec();
    let mut test = ProgramTest::new("dclutch_sbf", PROGRAM_ID, None);
    test.prefer_bpf(true);
    test.add_account(
        market,
        account(1_000_000, market_before.clone(), PROGRAM_ID),
    );
    test.add_account(fund, fund_before.clone());
    test.add_account(
        material_key,
        account(1, material_bytes.to_vec(), PROGRAM_ID),
    );
    test.add_account(manifest_key, account(1, manifest_bytes, PROGRAM_ID));
    test.add_account(substitute_material, account(1, wrong_material, PROGRAM_ID));
    test.add_account(substitute_manifest, account(1, empty_manifest, PROGRAM_ID));
    test.add_account(bounty, account(BOUNTY_OPENING, vec![], system_program::ID));
    test.add_account(
        sponsor,
        account(SPONSOR_OPENING, vec![], system_program::ID),
    );
    Fixture {
        test,
        market,
        fund,
        material: material_key,
        manifest: manifest_key,
        substitute_material,
        substitute_manifest,
        bounty,
        sponsor,
        market_before,
        fund_before,
        bounty_before: BOUNTY_OPENING,
        sponsor_before: SPONSOR_OPENING,
    }
}

fn failure_instruction(fixture: &Fixture, material: Pubkey, manifest: Pubkey) -> Instruction {
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
            AccountMeta::new_readonly(material, false),
            AccountMeta::new_readonly(manifest, false),
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

async fn assert_rollback(
    context: &mut solana_program_test::ProgramTestContext,
    market_key: Pubkey,
    fund_key: Pubkey,
    bounty_key: Pubkey,
    sponsor_key: Pubkey,
    market_before: Vec<u8>,
    fund_before: Account,
    bounty_before: u64,
    sponsor_before: u64,
) {
    let market = context
        .banks_client
        .get_account(market_key)
        .await
        .expect("market query");
    let fund = context
        .banks_client
        .get_account(fund_key)
        .await
        .expect("fund query");
    let bounty = context
        .banks_client
        .get_account(bounty_key)
        .await
        .expect("bounty query");
    let sponsor = context
        .banks_client
        .get_account(sponsor_key)
        .await
        .expect("sponsor query");
    assert_eq!(market.expect("market remains").data, market_before);
    assert_eq!(fund, Some(fund_before));
    assert_eq!(bounty.expect("bounty remains").lamports, bounty_before);
    assert_eq!(sponsor.expect("sponsor remains").lamports, sponsor_before);
}

#[tokio::test]
async fn body_free_failure_resolves_closes_raw_fund_and_refuses_replay() {
    let fixture = fixture(-1_000_000, 1, false);
    let instruction = failure_instruction(&fixture, fixture.material, fixture.manifest);
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
    let state = CategoricalMarketV1::<2>::decode(&market.data).expect("resolved market");
    assert_eq!(state.root().phase(), Phase::Resolved);
    assert_eq!(state.root().outstanding_children(), 1);
    let resolution = state
        .settlement()
        .resolution()
        .expect("terminal settlement");
    assert_eq!(resolution.resolution_kind(), ResolutionKind::Failure);
    assert_eq!(resolution.winner(), 1);
    assert_eq!(state.hoard_atoms(), 101, "hoard principal is untouched");
    assert!(
        context
            .banks_client
            .get_account(fixture.fund)
            .await
            .expect("fund query")
            .is_none(),
        "real runtime purges the drained Fund"
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
async fn failure_refusals_preserve_market_raw_fund_and_payouts() {
    for (fixture, material, manifest) in [
        {
            let fixture = fixture(9_999_999_999, 1, false);
            (fixture, None, None)
        },
        {
            let fixture = fixture(-1_000_000, 1, true);
            (fixture, None, None)
        },
        {
            let fixture = fixture(-1_000_000, 1, false);
            let material = fixture.substitute_material;
            (fixture, Some(material), None)
        },
        {
            let fixture = fixture(-1_000_000, 1, false);
            let manifest = fixture.substitute_manifest;
            (fixture, None, Some(manifest))
        },
    ] {
        let instruction = failure_instruction(
            &fixture,
            material.unwrap_or(fixture.material),
            manifest.unwrap_or(fixture.manifest),
        );
        let market = fixture.market;
        let fund = fixture.fund;
        let bounty = fixture.bounty;
        let sponsor = fixture.sponsor;
        let market_before = fixture.market_before.clone();
        let fund_before = fixture.fund_before.clone();
        let bounty_before = fixture.bounty_before;
        let sponsor_before = fixture.sponsor_before;
        let mut context = fixture.test.start_with_context().await;
        assert!(
            submit(&mut context, instruction).await.is_err(),
            "invalid failure frame or funding must refuse atomically"
        );
        assert_rollback(
            &mut context,
            market,
            fund,
            bounty,
            sponsor,
            market_before,
            fund_before,
            bounty_before,
            sponsor_before,
        )
        .await;
    }
}
