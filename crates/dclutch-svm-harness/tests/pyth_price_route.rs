//! Non-production executable evidence for the synthetic-local Pyth route.
//!
//! This campaign loads the locally compiled dClutch SBF ELF and the two
//! provenance-pinned upgraded provider ELFs.  It registers no native or mock
//! processor.  The signed observation is cryptographically real but names a
//! synthetic feed and is not devnet, provider-availability, production-release,
//! or mainnet evidence.

use std::{env, fs, path::PathBuf};

#[path = "support/pyth_provider.rs"]
mod pyth_provider;

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityFundingDerivationV1,
    CapabilityManifestV1, CompartmentFundingV1, FundingAmountsV1, FundingQuoteV1,
    MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_collateral_contract::{COMPACT_TERMINAL_MARKET_BYTES, CompactTerminalMarketV1};
use dclutch_core_contract::{ContentId, MarketIdentity, MarketRoot, Phase};
use dclutch_market_contract::market::{CategoricalMarketV1, CategoricalSettlementSummaryV1};
use dclutch_product_contract::{
    ContentId as ProductContentId,
    capacity::CapacityProfileId,
    product::{InstanceV1, InstanceV1Input},
    result_domain::{FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1, FiniteResultDomainV1},
};
use dclutch_pyth_contract::{
    funding::{
        FUNDING_BYTES, construct_required_resolution_funding, required_resolution_minimum_balance,
    },
    instruction::ResolveCategoricalPythV1,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_rent_contract::{RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority, RentCreditV1};
use dclutch_source_contract::{
    CapacityEnvelope as SourceCapacityEnvelope, ContentId as SourceContentId,
    PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1, ProviderReleaseV1, PythAdapterConfigV1,
    ResolutionPolicyV1, RoundingBoundary, SOURCE_MATERIAL_BYTES,
    SOURCE_MATERIAL_SCHEMA_RELEASE_PREIMAGE_V1, SourceAccessProfile, SourceCapacityProfileV1,
    SourceMaterialInputV1, SourceSpecV1, StatisticKind, StatisticSpecV1, WindowKind, WindowSpecV1,
    encode_source_material_into_v1,
};
use pyth_provider::{
    PUBLISH_TIME, ProviderAddresses, RECEIVER_POST_UPDATE, add_upgraded_provider_programs,
    assert_all_fixture_hashes, initialize_real_providers, prove_full_provider_update,
    set_fixture_clock, submit, synthetic_release_id,
};
use solana_account::Account;
use solana_program::{
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{system_program, sysvar};
use solana_transaction::{InstructionError, TransactionError};

const PROGRAM_ID: Pubkey = Pubkey::new_from_array([71; 32]);
const GENERATION: u64 = 73;
const OPEN_CHILD_COUNT: u64 = 2;
const PROVIDER_FEE: u64 = 1;
const SUCCESS_BOUNTY: u64 = 5;
const RESOLVER_OPENING: u64 = 20_000_000;
const MANIFEST_SCHEMA_LABEL: &[u8] = b"dclutch/schema/capability-manifest-profile-1-v1";

struct ResolutionFixture {
    test: Option<ProgramTest>,
    provider: ProviderAddresses,
    resolver: Keypair,
    update: Keypair,
    market: Pubkey,
    fund: Pubkey,
    material: Pubkey,
    manifest: Pubkey,
    rent_credit: Pubkey,
    material_cursor: Pubkey,
    manifest_cursor: Pubkey,
    market_before: Account,
    fund_before: Account,
    rent_credit_before: Account,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AtomicSnapshot {
    market: Option<Account>,
    fund: Option<Account>,
    rent_credit: Option<Account>,
    resolver: Option<Account>,
    treasury: Option<Account>,
    update: Option<Account>,
    encoded_vaa: Option<Account>,
    config: Option<Account>,
}

fn require_lab_sbf() {
    let directory = env::var("SBF_OUT_DIR").expect(
        "SBF_OUT_DIR is required; build the real adapter with `cargo build-sbf --manifest-path programs/dclutch-sbf/Cargo.toml --features non-production-real-pyth-lab`, then point SBF_OUT_DIR at its deploy directory",
    );
    let artifact = PathBuf::from(directory).join("dclutch_sbf.so");
    let bytes = fs::read(&artifact).unwrap_or_else(|error| {
        panic!(
            "cannot read the required compiled dClutch SBF ELF {}: {error}",
            artifact.display()
        )
    });
    assert_eq!(bytes.get(..4), Some(&[0x7f, b'E', b'L', b'F'][..]));
    assert!(
        bytes
            .windows(b"local-upgraded-2026-08-22".len())
            .any(|window| window == b"local-upgraded-2026-08-22"),
        "the compiled dClutch ELF lacks the explicit non-production-real-pyth-lab release; rebuild it with that feature instead of weakening release authentication"
    );
    eprintln!(
        "NON-PRODUCTION synthetic-local dClutch ELF SHA-256: {:?}",
        hash(&bytes).to_bytes()
    );
}

fn protocol_account(data: Vec<u8>) -> Account {
    Account {
        lamports: Rent::default().minimum_balance(data.len()),
        data,
        owner: PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    }
}

fn finalized_record(schema_label: &[u8], content: Vec<u8>) -> (Pubkey, Pubkey, Account) {
    let schema = hash(schema_label).to_bytes();
    let digest = hash(&content).to_bytes();
    let raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, schema.as_slice(), digest.as_slice()],
        &PROGRAM_ID,
    )
    .0;
    let cursor = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            schema.as_slice(),
            digest.as_slice(),
        ],
        &PROGRAM_ID,
    )
    .0;
    (raw, cursor, protocol_account(content))
}

fn product_id(bytes: [u8; 32]) -> ProductContentId {
    ProductContentId::new(bytes).expect("nonzero deterministic Product identity")
}

fn source_id(bytes: [u8; 32]) -> SourceContentId {
    SourceContentId::new(bytes).expect("nonzero deterministic Source identity")
}

fn source_material_bytes(release_id: [u8; 32]) -> (Vec<u8>, [u8; 32], [u8; 32]) {
    let result_domain =
        FiniteResultDomainV1::new(product_id([0xb1; 32]), product_id([0xb2; 32]), 1, &[])
            .expect("canonical binary Product result domain");
    let result_domain_bytes = result_domain.to_bytes();
    let result_domain_digest = hashv(&[
        FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1,
        &[0],
        result_domain_bytes.as_slice(),
    ])
    .to_bytes();
    let claim_id = [33; 32];
    let instance = InstanceV1::new(InstanceV1Input {
        terms_id: product_id([32; 32]),
        occurrence_id: product_id([34; 32]),
        claim_basis_id: product_id(claim_id),
        result_domain_id: product_id(result_domain_digest),
        capacity_profile_id: CapacityProfileId::new(product_id([35; 32])),
        partition_cell_count: 2,
    })
    .expect("canonical Product instance");
    let instance_digest = hash(&instance.to_bytes()).to_bytes();
    let capacity = SourceCapacityProfileV1::new(
        SourceCapacityEnvelope::Measured,
        1,
        0,
        source_id([36; 32]),
        source_id([37; 32]),
        512,
        0,
    )
    .expect("canonical Source capacity");
    let capacity_id = source_id(hash(&capacity.to_bytes()).to_bytes());
    let provider = ProviderReleaseV1::new(
        source_id([38; 32]),
        source_id(PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1),
        source_id(release_id),
        source_id([39; 32]),
        source_id([40; 32]),
    );
    let provider_id = source_id(hash(&provider.to_bytes()).to_bytes());
    let adapter =
        PythAdapterConfigV1::new([0x2a; 32], -8, 100).expect("pinned Pyth adapter config");
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
        PUBLISH_TIME,
        PUBLISH_TIME,
        60,
        1,
        source_id([41; 32]),
    )
    .expect("pinned terminal window");
    let window_id = source_id(hash(&window.to_bytes()).to_bytes());
    let statistic = StatisticSpecV1::new(
        source_id(result_domain.result_unit_id().to_bytes()),
        source_id(result_domain.result_unit_id().to_bytes()),
        StatisticKind::TerminalSample,
        RoundingBoundary::ExactRational,
        1,
        0,
        capacity_id,
        source_id([42; 32]),
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

fn resolution_fixture() -> ResolutionFixture {
    assert_all_fixture_hashes();
    require_lab_sbf();
    let provider = ProviderAddresses::pinned();
    let release_id = synthetic_release_id(provider);
    let (material_bytes, product_instance_id, claim_id) = source_material_bytes(release_id);
    let (material, material_cursor, material_account) =
        finalized_record(SOURCE_MATERIAL_SCHEMA_RELEASE_PREIMAGE_V1, material_bytes);

    let material_id =
        ContentId::new(hash(&material_account.data).to_bytes()).expect("SourceMaterial ID");
    let fund_rent = Rent::default().minimum_balance(FUNDING_BYTES);
    let entry = CapabilityEntryV1::new(
        ContentId::new([21; 32]).expect("capability kind"),
        ContentId::new(PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1)
            .expect("closed Pyth Source extension"),
        material_id,
        ContentId::new([22; 32]).expect("capacity profile"),
        ContentId::new([23; 32]).expect("fund schema"),
        ContentId::new([24; 32]).expect("fund derivation"),
        ActivationPolicy::RequiredAtFounding,
        0,
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        FundingQuoteV1::new(
            FundingAmountsV1::new(
                CompartmentFundingV1::native_lamports(fund_rent)
                    .expect("Fund state rent is native lamports"),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::native_lamports(PROVIDER_FEE)
                    .expect("receiver fee is native lamports"),
                CompartmentFundingV1::native_lamports(SUCCESS_BOUNTY)
                    .expect("resolver bounty is native lamports"),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
            )
            .expect("typed one-shot resolution funding"),
            None,
        )
        .expect("native-only one-shot resolution quote"),
    )
    .expect("canonical resolution capability");
    let mut manifest_bytes = vec![0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    let manifest_value = CapabilityManifestV1::encode_into(&[entry], &mut manifest_bytes)
        .expect("canonical manifest");
    let manifest_id =
        ContentId::new(hash(manifest_value.as_bytes()).to_bytes()).expect("manifest ID");
    let selected = manifest_value
        .required_founding_entry_for_config(material_id)
        .expect("one required resolution entry");
    let funding =
        construct_required_resolution_funding(manifest_id, manifest_value, selected, fund_rent, 1)
            .expect("active prepaid resolution funding");
    let refund_beneficiary = Pubkey::new_from_array([82; 32]);
    let refund_authority =
        RefundAuthority::new(refund_beneficiary.to_bytes()).expect("rent beneficiary");
    let refund_authority_bytes = refund_authority.to_bytes();
    let (rent_credit, rent_credit_bump) = Pubkey::find_program_address(
        &[RENT_CREDIT_PDA_DOMAIN_V1, refund_authority_bytes.as_slice()],
        &PROGRAM_ID,
    );
    let rent_credit_state = RentCreditV1::new(refund_authority, rent_credit_bump);
    let rent_credit_before = protocol_account(rent_credit_state.to_bytes().to_vec());

    let identity = MarketIdentity::new(
        ContentId::new([31; 32]).expect("Realm ID"),
        ContentId::new(product_instance_id).expect("Product Instance ID"),
        ContentId::new(claim_id).expect("Claim ID"),
        material_id,
        manifest_id,
        GENERATION,
    );
    let identity_digest = hash(&identity.to_bytes()).to_bytes();
    let market = Pubkey::find_program_address(
        &[b"dclutch/market-root/v1", identity_digest.as_slice()],
        &PROGRAM_ID,
    )
    .0;
    let mut root = MarketRoot::founding(identity, refund_beneficiary.to_bytes())
        .expect("canonical founding root");
    root.register_child(GENERATION, 0).expect("Fund child");
    root.register_child(GENERATION, 1).expect("custody child");
    root.transition_phase(GENERATION, Phase::Open)
        .expect("Open prerequisite state");
    let market_value =
        CategoricalMarketV1::<2>::new(root, 0, [0, 0], CategoricalSettlementSummaryV1::empty())
            .expect("provider-neutral Open Market");
    let mut market_bytes =
        vec![0_u8; CategoricalMarketV1::<2>::encoded_len().expect("binary Market width")];
    market_value
        .encode(&mut market_bytes)
        .expect("canonical Open Market bytes");
    let market_before = protocol_account(market_bytes);

    let fund_derivation = CapabilityFundingDerivationV1::new(
        market.to_bytes(),
        GENERATION,
        manifest_id,
        manifest_value,
        funding,
    )
    .expect("canonical Fund derivation");
    let fund = Pubkey::find_program_address(&fund_derivation.seed_components(), &PROGRAM_ID).0;
    let (manifest, manifest_cursor, manifest_account) =
        finalized_record(MANIFEST_SCHEMA_LABEL, manifest_bytes);
    let fund_before = Account {
        lamports: required_resolution_minimum_balance(funding).expect("exact Fund minimum"),
        data: funding.to_bytes().to_vec(),
        owner: PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };

    let resolver = Keypair::new();
    let update = Keypair::new();
    let mut test = ProgramTest::new("dclutch_sbf", PROGRAM_ID, None);
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    add_upgraded_provider_programs(&mut test, provider);
    test.add_account(
        resolver.pubkey(),
        Account::new(RESOLVER_OPENING, 0, &system_program::ID),
    );
    test.add_account(market, market_before.clone());
    test.add_account(fund, fund_before.clone());
    test.add_account(material, material_account);
    test.add_account(manifest, manifest_account);
    test.add_account(rent_credit, rent_credit_before.clone());
    test.add_account(material_cursor, Account::new(0, 0, &system_program::ID));
    test.add_account(manifest_cursor, Account::new(0, 0, &system_program::ID));

    ResolutionFixture {
        test: Some(test),
        provider,
        resolver,
        update,
        market,
        fund,
        material,
        manifest,
        rent_credit,
        material_cursor,
        manifest_cursor,
        market_before,
        fund_before,
        rent_credit_before,
    }
}

fn price_resolution_instruction(fixture: &ResolutionFixture, encoded_vaa: Pubkey) -> Instruction {
    assert_eq!(RECEIVER_POST_UPDATE.len(), 102);
    let body = &RECEIVER_POST_UPDATE[8..];
    let request = ResolveCategoricalPythV1::new(GENERATION, OPEN_CHILD_COUNT, body)
        .expect("nonempty exact provider body");
    let mut data = vec![0_u8; 40 + body.len()];
    request.encode(&mut data).expect("exact resolve wire");
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(fixture.resolver.pubkey(), true),
            AccountMeta::new(fixture.update.pubkey(), true),
            AccountMeta::new(fixture.market, false),
            AccountMeta::new(fixture.fund, false),
            AccountMeta::new_readonly(fixture.material, false),
            AccountMeta::new_readonly(fixture.manifest, false),
            AccountMeta::new(fixture.rent_credit, false),
            AccountMeta::new_readonly(fixture.provider.receiver, false),
            AccountMeta::new_readonly(fixture.provider.receiver_programdata, false),
            AccountMeta::new_readonly(fixture.provider.config, false),
            AccountMeta::new_readonly(encoded_vaa, false),
            AccountMeta::new_readonly(fixture.provider.router, false),
            AccountMeta::new_readonly(fixture.provider.router_programdata, false),
            AccountMeta::new(fixture.provider.treasury, false),
            AccountMeta::new_readonly(fixture.material_cursor, false),
            AccountMeta::new_readonly(fixture.manifest_cursor, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ],
        data,
    }
}

fn deliberately_late_refusal(fixture: &ResolutionFixture) -> Instruction {
    let mut data = [0_u8; COMPACT_TERMINAL_MARKET_BYTES];
    CompactTerminalMarketV1::new(GENERATION)
        .encode(&mut data)
        .expect("exact compaction wire");
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

async fn observed(context: &mut ProgramTestContext, address: Pubkey) -> Option<Account> {
    context
        .banks_client
        .get_account(address)
        .await
        .expect("bank account query")
}

async fn snapshot(
    context: &mut ProgramTestContext,
    fixture: &ResolutionFixture,
    encoded_vaa: Pubkey,
) -> AtomicSnapshot {
    AtomicSnapshot {
        market: observed(context, fixture.market).await,
        fund: observed(context, fixture.fund).await,
        rent_credit: observed(context, fixture.rent_credit).await,
        resolver: observed(context, fixture.resolver.pubkey()).await,
        treasury: observed(context, fixture.provider.treasury).await,
        update: observed(context, fixture.update.pubkey()).await,
        encoded_vaa: observed(context, encoded_vaa).await,
        config: observed(context, fixture.provider.config).await,
    }
}

#[tokio::test]
async fn captured_programs_verify_post_and_resolve_the_synthetic_price_through_real_elfs() {
    let mut fixture = resolution_fixture();
    let provider = fixture.provider;
    let mut context = fixture
        .test
        .take()
        .expect("unstarted real-program fixture")
        .start_with_context()
        .await;
    let encoded_vaa = initialize_real_providers(&mut context, provider).await;
    set_fixture_clock(&mut context).await;
    prove_full_provider_update(&mut context, provider, encoded_vaa).await;
    let treasury_before = observed(&mut context, provider.treasury)
        .await
        .expect("probe created treasury")
        .lamports;

    submit(
        &mut context,
        &[price_resolution_instruction(&fixture, encoded_vaa)],
        &[&fixture.resolver, &fixture.update],
    )
    .await
    .expect("real dClutch ELF atomically posts, resolves, reclaims, and closes funding");

    let market = observed(&mut context, fixture.market)
        .await
        .expect("resolved Market persists");
    let market = CategoricalMarketV1::<2>::decode(&market.data).expect("resolved Market bytes");
    let resolution = market
        .settlement()
        .resolution()
        .expect("terminal categorical truth");
    assert_eq!(resolution.winner(), 0);
    assert_eq!(market.root().phase(), Phase::Resolved);
    assert_eq!(market.root().outstanding_children(), 1);
    assert!(observed(&mut context, fixture.fund).await.is_none());
    assert!(
        observed(&mut context, fixture.update.pubkey())
            .await
            .is_none()
    );
    let resolver = observed(&mut context, fixture.resolver.pubkey())
        .await
        .expect("resolver persists");
    assert_eq!(resolver.lamports, RESOLVER_OPENING + SUCCESS_BOUNTY);
    let treasury = observed(&mut context, provider.treasury)
        .await
        .expect("provider treasury persists");
    assert_eq!(treasury.lamports, treasury_before + PROVIDER_FEE);
    let rent_credit = observed(&mut context, fixture.rent_credit)
        .await
        .expect("RentCredit persists");
    assert_eq!(
        rent_credit.lamports,
        fixture.rent_credit_before.lamports + Rent::default().minimum_balance(FUNDING_BYTES)
    );
}

#[tokio::test]
async fn late_dclutch_refusal_rolls_back_provider_and_protocol_writes_together() {
    let mut fixture = resolution_fixture();
    let provider = fixture.provider;
    let mut context = fixture
        .test
        .take()
        .expect("unstarted real-program fixture")
        .start_with_context()
        .await;
    let encoded_vaa = initialize_real_providers(&mut context, provider).await;
    set_fixture_clock(&mut context).await;
    prove_full_provider_update(&mut context, provider, encoded_vaa).await;
    let before = snapshot(&mut context, &fixture, encoded_vaa).await;
    assert_eq!(before.market, Some(fixture.market_before.clone()));
    assert_eq!(before.fund, Some(fixture.fund_before.clone()));
    assert_eq!(before.rent_credit, Some(fixture.rent_credit_before.clone()));
    assert!(before.treasury.is_some());
    assert!(before.update.is_none());

    let result = submit(
        &mut context,
        &[
            price_resolution_instruction(&fixture, encoded_vaa),
            // The first instruction has completed the provider post/reclaim,
            // Market resolution, and Fund close.  Compaction then refuses
            // because the live custody child makes this Market nonterminal.
            deliberately_late_refusal(&fixture),
        ],
        &[&fixture.resolver, &fixture.update],
    )
    .await;
    assert!(
        matches!(
            result,
            Err(BanksClientError::TransactionError(
                TransactionError::InstructionError(1, InstructionError::Custom(11))
            ))
        ),
        "instruction 1 must refuse with dClutch MarketTransition after Price18 completed"
    );
    assert_eq!(
        snapshot(&mut context, &fixture, encoded_vaa).await,
        before,
        "SVM transaction rollback must restore provider treasury/update state and every dClutch Market/Fund/RentCredit write"
    );
}
