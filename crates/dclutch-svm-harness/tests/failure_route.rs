use std::{env, path::PathBuf};

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityFundingDerivationV1,
    CapabilityManifestV1, CompartmentFundingV1, FundingAmountsV1, FundingQuoteV1,
    MANIFEST_HEADER_BYTES,
};
use dclutch_core_contract::{ContentId, MarketIdentity, MarketRoot, Phase};
use dclutch_market_contract::market::{CategoricalMarketV1, CategoricalSettlementSummaryV1};
use dclutch_product_contract::{
    ContentId as ProductContentId,
    capacity::CapacityProfileId,
    product::{InstanceV1, InstanceV1Input},
    result_domain::{FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1, FiniteResultDomainV1},
    terminal::ResolutionKind,
};
use dclutch_pyth_contract::{
    funding::{
        FUNDING_BYTES, construct_required_resolution_funding, required_resolution_minimum_balance,
    },
    instruction::{RESOLVE_FAILURE_BYTES, ResolveCategoricalFailureV1},
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_rent_contract::{
    CreateRentCreditV1, RENT_CREDIT_BYTES_V1, RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority,
    RentCreditV1,
};
use dclutch_source_contract::{
    CapacityEnvelope as SourceCapacityEnvelope, ContentId as SourceContentId,
    PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1, ProviderReleaseV1, PythAdapterConfigV1,
    ResolutionPolicyV1, RoundingBoundary, SOURCE_MATERIAL_BYTES,
    SOURCE_MATERIAL_SCHEMA_RELEASE_PREIMAGE_V1, SourceAccessProfile, SourceCapacityProfileV1,
    SourceMaterialInputV1, SourceSpecV1, StatisticKind, StatisticSpecV1, WindowKind, WindowSpecV1,
    encode_source_material_into_v1,
};
use solana_account::Account;
use solana_program::{
    hash::{hash, hashv},
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
const MANIFEST_SCHEMA_LABEL: &[u8] = b"dclutch/schema/capability-manifest-profile-1-v1";

struct Fixture {
    test: ProgramTest,
    market: Pubkey,
    fund: Pubkey,
    material: Pubkey,
    material_cursor: Pubkey,
    manifest: Pubkey,
    manifest_cursor: Pubkey,
    substitute_material: Pubkey,
    substitute_material_cursor: Pubkey,
    substitute_manifest: Pubkey,
    substitute_manifest_cursor: Pubkey,
    bounty: Pubkey,
    sponsor: Pubkey,
    rent_credit: Pubkey,
    rent_credit_state: RentCreditV1,
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

fn product_id(bytes: [u8; 32]) -> ProductContentId {
    ProductContentId::new(bytes).expect("nonzero deterministic Product identity")
}

fn source_id(bytes: [u8; 32]) -> SourceContentId {
    SourceContentId::new(bytes).expect("nonzero deterministic Source identity")
}

fn source_material_bytes(terminal_time: i64) -> (Vec<u8>, [u8; 32], [u8; 32]) {
    let result_domain = FiniteResultDomainV1::new(product_id([5; 32]), product_id([6; 32]), 1, &[])
        .expect("canonical binary Product result domain");
    let result_domain_bytes = result_domain.to_bytes();
    let result_domain_digest = hashv(&[
        FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1,
        &[0],
        result_domain_bytes.as_slice(),
    ])
    .to_bytes();
    let claim_id = [13; 32];
    let instance = InstanceV1::new(InstanceV1Input {
        terms_id: product_id([12; 32]),
        occurrence_id: product_id([14; 32]),
        claim_basis_id: product_id(claim_id),
        result_domain_id: product_id(result_domain_digest),
        capacity_profile_id: CapacityProfileId::new(product_id([15; 32])),
        partition_cell_count: 2,
    })
    .expect("canonical Product instance");
    let instance_digest = hash(&instance.to_bytes()).to_bytes();
    let capacity = SourceCapacityProfileV1::new(
        SourceCapacityEnvelope::Measured,
        1,
        0,
        source_id([31; 32]),
        source_id([32; 32]),
        512,
        0,
    )
    .expect("canonical Source capacity");
    let capacity_id = source_id(hash(&capacity.to_bytes()).to_bytes());
    let provider = ProviderReleaseV1::new(
        source_id([33; 32]),
        source_id(PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1),
        source_id([1; 32]),
        source_id([34; 32]),
        source_id([35; 32]),
    );
    let provider_id = source_id(hash(&provider.to_bytes()).to_bytes());
    let adapter = PythAdapterConfigV1::new([4; 32], 0, 1).expect("canonical Pyth adapter config");
    let adapter_id = source_id(hash(&adapter.to_bytes()).to_bytes());
    let source = SourceSpecV1::new(
        source_id(result_domain.coordinate_domain_id().to_bytes()),
        source_id(result_domain.result_unit_id().to_bytes()),
        provider_id,
        SourceAccessProfile::PythTerminalOneTransaction,
        adapter_id,
        capacity_id,
    );
    let source_spec_id = source_id(hash(&source.to_bytes()).to_bytes());
    let window = WindowSpecV1::new(
        source_spec_id,
        WindowKind::Terminal,
        terminal_time,
        terminal_time,
        1,
        1,
        source_id([36; 32]),
    )
    .expect("canonical terminal window");
    let window_id = source_id(hash(&window.to_bytes()).to_bytes());
    let statistic = StatisticSpecV1::new(
        source_id(result_domain.result_unit_id().to_bytes()),
        source_id(result_domain.result_unit_id().to_bytes()),
        StatisticKind::TerminalSample,
        RoundingBoundary::ExactRational,
        1,
        0,
        capacity_id,
        source_id([37; 32]),
        capacity,
    )
    .expect("canonical terminal statistic");
    let statistic_id = source_id(hash(&statistic.to_bytes()).to_bytes());
    let policy = ResolutionPolicyV1::new(
        capacity_id,
        source_id(instance_digest),
        source_spec_id,
        window_id,
        statistic_id,
        source_id(result_domain_digest),
        None,
    );
    let mut material = vec![0; SOURCE_MATERIAL_BYTES];
    encode_source_material_into_v1(
        &mut material,
        SourceMaterialInputV1 {
            policy: &policy,
            capacity_profile_id: capacity_id,
            capacity_profile: &capacity,
            primary_source_id: source_spec_id,
            primary_source: &source,
            primary_provider_release_id: provider_id,
            primary_provider_release: &provider,
            primary_adapter_config: &adapter,
            window_id,
            window: &window,
            statistic_id,
            statistic: &statistic,
            product_instance_id: source_id(instance_digest),
            product_instance: &instance,
            result_domain: &result_domain,
            recovery: None,
        },
    )
    .expect("canonical Product-bound Source material");
    (material, instance_digest, claim_id)
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

fn record_addresses(schema_label: &[u8], exact_content: &[u8]) -> (Pubkey, Pubkey) {
    let schema = hash(schema_label).to_bytes();
    let digest = hash(exact_content).to_bytes();
    let (raw, _) = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, schema.as_slice(), digest.as_slice()],
        &PROGRAM_ID,
    );
    let (cursor, _) = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            schema.as_slice(),
            digest.as_slice(),
        ],
        &PROGRAM_ID,
    );
    (raw, cursor)
}

fn finalized_record_account(exact_content: Vec<u8>) -> Account {
    account(
        Rent::default().minimum_balance(exact_content.len()),
        exact_content,
        PROGRAM_ID,
    )
}

fn vacant_cursor() -> Account {
    Account::new(0, 0, &system_program::ID)
}

fn manifest_bytes(material_id: ContentId, fund_rent: u64) -> Vec<u8> {
    let entry = CapabilityEntryV1::new(
        ContentId::new([21; 32]).expect("kind"),
        ContentId::new(PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1).expect("Pyth Source extension"),
        material_id,
        ContentId::new([22; 32]).expect("capacity"),
        ContentId::new([23; 32]).expect("child schema"),
        ContentId::new([24; 32]).expect("derivation"),
        ActivationPolicy::RequiredAtFounding,
        0,
        0,
        [0; 16],
        FundingQuoteV1::new(
            FundingAmountsV1::new(
                CompartmentFundingV1::native_lamports(fund_rent)
                    .expect("Fund state rent is native lamports"),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::native_lamports(PROVIDER_REIMBURSEMENT)
                    .expect("provider reimbursement is native lamports"),
                CompartmentFundingV1::native_lamports(BOUNTY)
                    .expect("failure bounty is native lamports"),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
            )
            .expect("typed failure funding"),
            None,
        )
        .expect("native-only failure resolution quote"),
    )
    .expect("one required resolution capability");
    let mut bytes = vec![0; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&[entry], &mut bytes).expect("canonical manifest");
    bytes
}

fn fixture(terminal_time: i64, underfunded: bool) -> Fixture {
    require_sbf_out_dir();
    let (material_bytes, product_instance_id, claim_id) = source_material_bytes(terminal_time);
    let fund_rent = Rent::default().minimum_balance(FUNDING_BYTES);
    let material_id = ContentId::new(hash(&material_bytes).to_bytes()).expect("SourceMaterial ID");
    let manifest_bytes = manifest_bytes(material_id, fund_rent);
    let manifest_id = ContentId::new(hash(&manifest_bytes).to_bytes()).expect("manifest id");
    let manifest = CapabilityManifestV1::decode(&manifest_bytes).expect("manifest decode");
    let selected = manifest
        .required_founding_entry_for_config(material_id)
        .expect("selected entry");
    let fund_state =
        construct_required_resolution_funding(manifest_id, manifest, selected, fund_rent, 1)
            .expect("active raw funding state");
    let identity = MarketIdentity::new(
        ContentId::new([11; 32]).expect("realm"),
        ContentId::new(product_instance_id).expect("Product instance"),
        ContentId::new(claim_id).expect("claim basis"),
        material_id,
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
    let funding_derivation = CapabilityFundingDerivationV1::new(
        market.to_bytes(),
        GENERATION,
        manifest_id,
        manifest,
        fund_state,
    )
    .expect("canonical funding derivation");
    let (fund, _) =
        Pubkey::find_program_address(&funding_derivation.seed_components(), &PROGRAM_ID);
    let bounty = Pubkey::new_from_array([81; 32]);
    let sponsor = Pubkey::new_from_array([82; 32]);
    let authority = RefundAuthority::new(sponsor.to_bytes()).expect("refund authority");
    let authority_bytes = authority.to_bytes();
    let (rent_credit, rent_credit_bump) = Pubkey::find_program_address(
        &[RENT_CREDIT_PDA_DOMAIN_V1, authority_bytes.as_slice()],
        &PROGRAM_ID,
    );
    let rent_credit_state = RentCreditV1::new(authority, rent_credit_bump);
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
    let (wrong_material, wrong_product_instance_id, wrong_claim_id) = source_material_bytes(7);
    assert_eq!(wrong_product_instance_id, product_instance_id);
    assert_eq!(wrong_claim_id, claim_id);
    let empty_manifest = CapabilityManifestV1::empty()
        .expect("empty manifest")
        .as_bytes()
        .to_vec();
    let (material_key, material_cursor) =
        record_addresses(SOURCE_MATERIAL_SCHEMA_RELEASE_PREIMAGE_V1, &material_bytes);
    let (manifest_key, manifest_cursor) = record_addresses(MANIFEST_SCHEMA_LABEL, &manifest_bytes);
    let (substitute_material, substitute_material_cursor) =
        record_addresses(SOURCE_MATERIAL_SCHEMA_RELEASE_PREIMAGE_V1, &wrong_material);
    let (substitute_manifest, substitute_manifest_cursor) =
        record_addresses(MANIFEST_SCHEMA_LABEL, &empty_manifest);
    let mut test = ProgramTest::new("dclutch_sbf", PROGRAM_ID, None);
    test.prefer_bpf(true);
    test.add_account(
        market,
        account(1_000_000, market_before.clone(), PROGRAM_ID),
    );
    test.add_account(fund, fund_before.clone());
    test.add_account(material_key, finalized_record_account(material_bytes));
    test.add_account(material_cursor, vacant_cursor());
    test.add_account(manifest_key, finalized_record_account(manifest_bytes));
    test.add_account(manifest_cursor, vacant_cursor());
    test.add_account(
        substitute_material,
        finalized_record_account(wrong_material),
    );
    test.add_account(substitute_material_cursor, vacant_cursor());
    test.add_account(
        substitute_manifest,
        finalized_record_account(empty_manifest),
    );
    test.add_account(substitute_manifest_cursor, vacant_cursor());
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
        material_cursor,
        manifest: manifest_key,
        manifest_cursor,
        substitute_material,
        substitute_material_cursor,
        substitute_manifest,
        substitute_manifest_cursor,
        bounty,
        sponsor,
        rent_credit,
        rent_credit_state,
        market_before,
        fund_before,
        bounty_before: BOUNTY_OPENING,
        sponsor_before: SPONSOR_OPENING,
    }
}

fn failure_instruction(
    fixture: &Fixture,
    material: Pubkey,
    material_cursor: Pubkey,
    manifest: Pubkey,
    manifest_cursor: Pubkey,
) -> Instruction {
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
            AccountMeta::new(fixture.rent_credit, false),
            AccountMeta::new_readonly(material_cursor, false),
            AccountMeta::new_readonly(manifest_cursor, false),
            AccountMeta::new_readonly(solana_sdk_ids::sysvar::rent::ID, false),
        ],
        data: data.to_vec(),
    }
}

async fn create_rent_credit(
    context: &mut solana_program_test::ProgramTestContext,
    rent_credit: Pubkey,
    rent_credit_state: RentCreditV1,
) -> Account {
    let instruction = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(context.payer.pubkey(), true),
            AccountMeta::new(rent_credit, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(solana_sdk_ids::sysvar::rent::ID, false),
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
        .expect("route exact RentCredit creation through loaded ELF");
    let credit = context
        .banks_client
        .get_account(rent_credit)
        .await
        .expect("RentCredit query")
        .expect("RentCredit exists");
    assert_eq!(credit.owner, PROGRAM_ID);
    assert_eq!(
        credit.lamports,
        Rent::default().minimum_balance(RENT_CREDIT_BYTES_V1)
    );
    assert_eq!(RentCreditV1::decode(&credit.data), Ok(rent_credit_state));
    credit
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

struct RollbackSnapshot {
    market: Pubkey,
    fund: Pubkey,
    bounty: Pubkey,
    sponsor: Pubkey,
    rent_credit: Pubkey,
    market_before: Vec<u8>,
    fund_before: Account,
    bounty_before: u64,
    sponsor_before: u64,
    rent_credit_before: Account,
}

async fn assert_rollback(
    context: &mut solana_program_test::ProgramTestContext,
    expected: &RollbackSnapshot,
) {
    let market = context
        .banks_client
        .get_account(expected.market)
        .await
        .expect("market query");
    let fund = context
        .banks_client
        .get_account(expected.fund)
        .await
        .expect("fund query");
    let bounty = context
        .banks_client
        .get_account(expected.bounty)
        .await
        .expect("bounty query");
    let sponsor = context
        .banks_client
        .get_account(expected.sponsor)
        .await
        .expect("sponsor query");
    let rent_credit = context
        .banks_client
        .get_account(expected.rent_credit)
        .await
        .expect("RentCredit query");
    assert_eq!(market.expect("market remains").data, expected.market_before);
    assert_eq!(fund, Some(expected.fund_before.clone()));
    assert_eq!(
        bounty.expect("bounty remains").lamports,
        expected.bounty_before
    );
    assert_eq!(
        sponsor.expect("sponsor remains").lamports,
        expected.sponsor_before
    );
    assert_eq!(rent_credit, Some(expected.rent_credit_before.clone()));
}

#[tokio::test]
async fn body_free_failure_resolves_closes_raw_fund_and_refuses_replay() {
    let fixture = fixture(-1_000_000, false);
    let instruction = failure_instruction(
        &fixture,
        fixture.material,
        fixture.material_cursor,
        fixture.manifest,
        fixture.manifest_cursor,
    );
    let rent_credit = fixture.rent_credit;
    let rent_credit_state = fixture.rent_credit_state;
    let mut context = fixture.test.start_with_context().await;
    let rent_credit_before = create_rent_credit(&mut context, rent_credit, rent_credit_state).await;
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
    let rent_credit = context
        .banks_client
        .get_account(fixture.rent_credit)
        .await
        .expect("RentCredit query")
        .expect("RentCredit exists");
    let rent = Rent::default().minimum_balance(FUNDING_BYTES);
    assert_eq!(bounty.lamports, fixture.bounty_before + BOUNTY);
    assert_eq!(sponsor.lamports, fixture.sponsor_before);
    assert_eq!(
        rent_credit.lamports,
        rent_credit_before.lamports + rent + PROVIDER_REIMBURSEMENT + SPONSOR_EXCESS
    );
    assert_eq!(
        RentCreditV1::decode(&rent_credit.data),
        Ok(fixture.rent_credit_state)
    );
    let market_before_replay = market.clone();
    let bounty_before_replay = bounty.clone();
    let sponsor_before_replay = sponsor.clone();
    let credit_before_replay = rent_credit.clone();
    assert!(
        submit(&mut context, instruction).await.is_err(),
        "terminal market rejects replay"
    );
    assert_eq!(
        context
            .banks_client
            .get_account(fixture.market)
            .await
            .expect("market replay query"),
        Some(market_before_replay)
    );
    assert!(
        context
            .banks_client
            .get_account(fixture.fund)
            .await
            .expect("Fund replay query")
            .is_none(),
        "replay does not recreate the closed raw Fund"
    );
    assert_eq!(
        context
            .banks_client
            .get_account(fixture.bounty)
            .await
            .expect("bounty replay query"),
        Some(bounty_before_replay)
    );
    assert_eq!(
        context
            .banks_client
            .get_account(fixture.sponsor)
            .await
            .expect("sponsor replay query"),
        Some(sponsor_before_replay)
    );
    assert_eq!(
        context
            .banks_client
            .get_account(fixture.rent_credit)
            .await
            .expect("RentCredit replay query"),
        Some(credit_before_replay)
    );
}

#[tokio::test]
async fn failure_refusals_preserve_market_raw_fund_and_payouts() {
    for (fixture, material, material_cursor, manifest, manifest_cursor) in [
        {
            let fixture = fixture(9_999_999_999, false);
            (fixture, None, None, None, None)
        },
        {
            let fixture = fixture(-1_000_000, true);
            (fixture, None, None, None, None)
        },
        {
            let fixture = fixture(-1_000_000, false);
            let material = fixture.substitute_material;
            let cursor = fixture.substitute_material_cursor;
            (fixture, Some(material), Some(cursor), None, None)
        },
        {
            let fixture = fixture(-1_000_000, false);
            let manifest = fixture.substitute_manifest;
            let cursor = fixture.substitute_manifest_cursor;
            (fixture, None, None, Some(manifest), Some(cursor))
        },
    ] {
        let instruction = failure_instruction(
            &fixture,
            material.unwrap_or(fixture.material),
            material_cursor.unwrap_or(fixture.material_cursor),
            manifest.unwrap_or(fixture.manifest),
            manifest_cursor.unwrap_or(fixture.manifest_cursor),
        );
        let rent_credit = fixture.rent_credit;
        let rent_credit_state = fixture.rent_credit_state;
        let mut context = fixture.test.start_with_context().await;
        let rent_credit_before =
            create_rent_credit(&mut context, rent_credit, rent_credit_state).await;
        assert!(
            submit(&mut context, instruction).await.is_err(),
            "invalid failure frame or funding must refuse atomically"
        );
        assert_rollback(
            &mut context,
            &RollbackSnapshot {
                market: fixture.market,
                fund: fixture.fund,
                bounty: fixture.bounty,
                sponsor: fixture.sponsor,
                rent_credit,
                market_before: fixture.market_before.clone(),
                fund_before: fixture.fund_before.clone(),
                bounty_before: fixture.bounty_before,
                sponsor_before: fixture.sponsor_before,
                rent_credit_before,
            },
        )
        .await;
    }
}
