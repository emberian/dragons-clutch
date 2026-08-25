//! Real-SBF Series action-3 evidence for the canonical capability-template
//! projection.  A reusable template is finalized first; each occurrence then
//! binds its full SourceMaterial digest into one separately finalized manifest.
//!
//! No legacy Pyth-shaped resolution-material wrapper, provisional manifest
//! digest, or native dClutch processor is used here.

use std::{env, path::PathBuf};

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    CAPABILITY_TEMPLATE_ENTRY_BYTES, CAPABILITY_TEMPLATE_SCHEMA_RELEASE_ID_V1,
    CapabilityConfigProjectionV1, CapabilityEntryV1, CapabilityFundingDerivationV1,
    CapabilityManifestV1, CapabilityTemplateEntryV1, CapabilityTemplateV1, CompartmentFundingV1,
    FundingAmountsV1, FundingQuoteV1, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_core_contract::ContentId as CoreContentId;
use dclutch_market_contract::market::CategoricalMarketV1;
use dclutch_product_contract::{
    ContentId as ProductContentId,
    capacity::{CapacityEnvelope, CapacityProfileId, CapacityProfileV1, CapacityProfileV1Input},
    claim::{CategoricalUnitV1, CategoricalUnitV1Input},
    product::{InstanceV1, InstanceV1Input},
    result_domain::{FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1, FiniteResultDomainV1},
};
use dclutch_pyth_contract::funding::{
    FUNDING_BYTES, construct_required_resolution_funding, required_resolution_minimum_balance,
};
use dclutch_realm_contract::{FreezeAuthorityPolicy, MintAuthorityPolicy, RealmV1, RealmV1Input};
use dclutch_record_contract::{
    AppendPageV1, BeginRecordV1, ContentDigest, FinalizeRecordV1, PageEnvelopeKindV1,
    PageEnvelopeV1, RAW_RECORD_PDA_SEED_V1, RecordKeyV1, STAGING_CURSOR_PDA_SEED_V1,
    SchemaReleaseId,
};
use dclutch_rent_contract::{
    CreateRentCreditV1, RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority, RentCreditV1,
};
use dclutch_series_contract::{
    CAPABILITY_DERIVATION_RELEASE_ID_V1, CapitalizationAggregateV1, ConsumeTicketV1,
    CreateSeriesV1, IdentityV1, InstantiateNextV1, MARKET_DERIVATION_RELEASE_ID_V1,
    OCCURRENCE_DERIVATION_RELEASE_ID_V1, OccurrenceCapitalizationV1,
    PRODUCT_COMPILER_RELEASE_ID_V1, SERIES_ESCROW_PDA_DOMAIN_V1, SERIES_REPLAY_GUARD_PDA_DOMAIN_V1,
    SERIES_ROOT_PDA_DOMAIN_V1, SERIES_TICKET_PDA_DOMAIN_V1, SOURCE_DERIVATION_RELEASE_ID_V1,
    SeriesRecipeV1, SeriesRootV1, authenticate_occurrence_capability_manifest_v1,
    authenticate_occurrence_source_material_v1, derive_occurrence_product_v1, derive_occurrence_v1,
    source_schedule_identity_v1,
};
use dclutch_source_contract::{
    CapacityEnvelope as SourceCapacityEnvelope, ContentId as SourceContentId,
    PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1, ProviderReleaseV1, PythAdapterConfigV1,
    ResolutionPolicyV1, RoundingBoundary, SOURCE_MATERIAL_BYTES,
    SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1, SourceAccessProfile, SourceCapacityProfileV1,
    SourceMaterialInputV1, SourceSpecV1, StatisticKind, StatisticSpecV1, WindowKind, WindowSpecV1,
    encode_source_material_into_v1,
};
use solana_account::Account;
use solana_program::{
    clock::Clock,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{system_program, sysvar};
use solana_transaction::Transaction;

const PROGRAM_ID: Pubkey = Pubkey::new_from_array([71; 32]);
const OUTCOME_COUNT: u8 = 2;
const GENERATION: u64 = 61;
const PROVIDER_FEE: u64 = 5;
const SUCCESS_BOUNTY: u64 = 7;
const PAGE_BYTES: usize = 768;
const STAGING_LIFETIME_SLOTS: u64 = 216_000;
const ROOT_DUST: u64 = 3;
const ESCROW_DUST: u64 = 5;
const GUARD_DUST: u64 = 7;
const TICKET_DUST: u64 = 11;
const MARKET_DUST: u64 = 13;
const FUND_DUST: u64 = 17;

const PAGE_RELEASE_LABEL: &[u8] = b"dclutch/sbf-record-page-envelope/provisional-v1";
const STAGING_RELEASE_LABEL: &[u8] = b"dclutch/sbf-record-staging-liveness/v1";
const REALM_SCHEMA_LABEL: &[u8] = b"dclutch/schema/realm-v1";
const INSTANCE_SCHEMA_LABEL: &[u8] = b"dclutch/schema/product-instance-v1";
const CLAIM_SCHEMA_LABEL: &[u8] = b"dclutch/schema/categorical-unit-claim-v1";
const CAPACITY_SCHEMA_LABEL: &[u8] = b"dclutch/schema/product-capacity-profile-v1";
const RECIPE_SCHEMA_LABEL: &[u8] = b"dclutch/schema/series-recipe-v3";
const AGGREGATE_SCHEMA_LABEL: &[u8] = b"dclutch/schema/series-capitalization-aggregate-v1";
const DERIVED_SCHEMA_LABEL: &[u8] = b"dclutch/schema/series-derived-occurrence-v1";
const CAPITALIZATION_SCHEMA_LABEL: &[u8] = b"dclutch/schema/series-occurrence-capitalization-v1";

#[derive(Clone, Debug, Eq, PartialEq)]
struct Snapshot {
    sponsor: Option<Account>,
    root: Option<Account>,
    escrow: Option<Account>,
    ticket: Option<Account>,
    market: Option<Account>,
    fund: Option<Account>,
    credit: Option<Account>,
}

struct Fixture {
    test: Option<ProgramTest>,
    sponsor: Keypair,
    credit: Pubkey,
    credit_state: RentCreditV1,
    template: Record,
    manifest: Record,
    substituted_manifest: Record,
    root: Pubkey,
    escrow: Pubkey,
    guard: Pubkey,
    ticket: Pubkey,
    market: Pubkey,
    fund: Pubkey,
    realm: Record,
    instance: Record,
    claim: Record,
    capacity: Record,
    source: Record,
    recipe: Record,
    aggregate: Record,
    derived: Record,
    capitalization: Record,
    create: CreateSeriesV1,
    instantiate: InstantiateNextV1,
    consume: ConsumeTicketV1,
}

#[derive(Clone)]
struct Record {
    raw: Pubkey,
    cursor: Pubkey,
    bytes: Vec<u8>,
}

fn require_sbf() {
    let output = env::var("SBF_OUT_DIR")
        .expect("SBF_OUT_DIR is required for the real dClutch ELF Series harness");
    assert!(
        PathBuf::from(output).join("dclutch_sbf.so").is_file(),
        "SBF_OUT_DIR must name an exact built dclutch_sbf.so"
    );
}

fn core_id(bytes: [u8; 32]) -> CoreContentId {
    CoreContentId::new(bytes).expect("nonzero Core content ID")
}

fn product_id(bytes: [u8; 32]) -> ProductContentId {
    ProductContentId::new(bytes).expect("nonzero Product content ID")
}

fn source_id(bytes: [u8; 32]) -> SourceContentId {
    SourceContentId::new(bytes).expect("nonzero Source content ID")
}

fn identity(bytes: [u8; 32]) -> IdentityV1 {
    IdentityV1::new(bytes).expect("nonzero Series identity")
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

fn record(schema: [u8; 32], bytes: Vec<u8>) -> Record {
    let digest = hash(&bytes).to_bytes();
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
    Record { raw, cursor, bytes }
}

fn add_finalized(test: &mut ProgramTest, record: &Record) {
    test.add_account(record.raw, protocol_account(record.bytes.clone()));
    test.add_account(record.cursor, Account::new(0, 0, &system_program::ID));
}

fn add_vacant_record(test: &mut ProgramTest, record: &Record) {
    test.add_account(record.raw, Account::new(0, 0, &system_program::ID));
    test.add_account(record.cursor, Account::new(0, 0, &system_program::ID));
}

fn resolution_quote(fund_rent: u64) -> FundingQuoteV1 {
    FundingQuoteV1::new(
        FundingAmountsV1::new(
            CompartmentFundingV1::native_lamports(fund_rent).expect("Fund rent"),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::native_lamports(PROVIDER_FEE).expect("provider fee"),
            CompartmentFundingV1::native_lamports(SUCCESS_BOUNTY).expect("success bounty"),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
        )
        .expect("one-shot resolution funding"),
        None,
    )
    .expect("native-only resolution quote")
}

fn template_bytes(fund_rent: u64) -> Vec<u8> {
    let entry = CapabilityTemplateEntryV1::new(
        core_id([21; 32]),
        core_id(PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1),
        CapabilityConfigProjectionV1::OccurrenceResolutionMaterial,
        core_id([22; 32]),
        core_id([23; 32]),
        core_id([24; 32]),
        ActivationPolicy::RequiredAtFounding,
        0,
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        resolution_quote(fund_rent),
    )
    .expect("one dynamic required template entry");
    let mut bytes = vec![0; MANIFEST_HEADER_BYTES + CAPABILITY_TEMPLATE_ENTRY_BYTES];
    CapabilityTemplateV1::encode_into(core::slice::from_ref(&entry), &mut bytes)
        .expect("canonical template");
    bytes
}

fn source_material_bytes(
    instance: InstanceV1,
    instance_id: [u8; 32],
    domain: FiniteResultDomainV1,
) -> Vec<u8> {
    let capacity = SourceCapacityProfileV1::new(
        SourceCapacityEnvelope::Measured,
        1,
        0,
        source_id([31; 32]),
        source_id([32; 32]),
        512,
        0,
    )
    .expect("Source capacity");
    let capacity_id = source_id(hash(&capacity.to_bytes()).to_bytes());
    let provider = ProviderReleaseV1::new(
        source_id([33; 32]),
        source_id(PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1),
        source_id([34; 32]),
        source_id([35; 32]),
        source_id([36; 32]),
    );
    let provider_id = source_id(hash(&provider.to_bytes()).to_bytes());
    let adapter = PythAdapterConfigV1::new([37; 32], -8, 10_000).expect("Pyth adapter");
    let adapter_id = source_id(hash(&adapter.to_bytes()).to_bytes());
    let source = SourceSpecV1::new(
        source_id(domain.coordinate_domain_id().to_bytes()),
        source_id(domain.result_unit_id().to_bytes()),
        provider_id,
        SourceAccessProfile::PythTerminalOneTransaction,
        adapter_id,
        capacity_id,
    );
    let source_spec_id = source_id(hash(&source.to_bytes()).to_bytes());
    let window = WindowSpecV1::new(
        source_spec_id,
        WindowKind::Terminal,
        0,
        0,
        1,
        1,
        source_id([38; 32]),
    )
    .expect("Source terminal window");
    let window_id = source_id(hash(&window.to_bytes()).to_bytes());
    let statistic = StatisticSpecV1::new(
        source_id(domain.result_unit_id().to_bytes()),
        source_id(domain.result_unit_id().to_bytes()),
        StatisticKind::TerminalSample,
        RoundingBoundary::ExactRational,
        1,
        0,
        capacity_id,
        source_id([39; 32]),
        capacity,
    )
    .expect("Source terminal statistic");
    let statistic_id = source_id(hash(&statistic.to_bytes()).to_bytes());
    let domain_bytes = domain.to_bytes();
    let domain_id = source_id(
        hashv(&[
            FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1,
            &[0],
            domain_bytes.as_slice(),
        ])
        .to_bytes(),
    );
    let policy = ResolutionPolicyV1::new(
        capacity_id,
        source_id(instance_id),
        source_spec_id,
        window_id,
        statistic_id,
        domain_id,
        None,
    );
    let mut bytes = vec![0; SOURCE_MATERIAL_BYTES];
    encode_source_material_into_v1(
        &mut bytes,
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
            product_instance_id: source_id(instance_id),
            product_instance: &instance,
            result_domain: &domain,
            recovery: None,
        },
    )
    .expect("Product-bound Source material");
    bytes
}

fn fixture() -> Fixture {
    require_sbf();
    let sponsor = Keypair::new();
    let fund_rent = Rent::default().minimum_balance(FUNDING_BYTES);

    let realm_value = RealmV1::new(RealmV1Input {
        token_program: [2; 32],
        collateral_mint: [3; 32],
        collateral_adapter_release_id: [4; 32],
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    })
    .expect("Realm");
    let realm_bytes = realm_value.to_bytes().to_vec();
    let realm = record(hash(REALM_SCHEMA_LABEL).to_bytes(), realm_bytes);
    let realm_id = identity(hash(&realm.bytes).to_bytes());

    let capacity_value = CapacityProfileV1::new(CapacityProfileV1Input {
        envelope: CapacityEnvelope::Measured,
        verifier_release_id: product_id([5; 32]),
        envelope_basis_id: product_id([6; 32]),
        max_artifact_bytes: 104,
        page_payload_bytes: 104,
        max_pages: 1,
        max_partition_cells: u32::from(OUTCOME_COUNT),
    })
    .expect("Series capacity");
    let capacity = record(
        hash(CAPACITY_SCHEMA_LABEL).to_bytes(),
        capacity_value.to_bytes().to_vec(),
    );
    let capacity_id = identity(hash(&capacity.bytes).to_bytes());
    let product_capacity = CapacityProfileId::new(product_id(capacity_id.to_bytes()));
    let claim_value = CategoricalUnitV1::new(
        CategoricalUnitV1Input {
            capacity_profile_id: product_capacity,
            outcome_count: u32::from(OUTCOME_COUNT),
        },
        capacity_value,
    )
    .expect("two-outcome categorical claim");
    let claim = record(
        hash(CLAIM_SCHEMA_LABEL).to_bytes(),
        claim_value.to_bytes().to_vec(),
    );
    let claim_id = identity(hash(&claim.bytes).to_bytes());

    let domain = FiniteResultDomainV1::new(product_id([7; 32]), product_id([8; 32]), 1, &[])
        .expect("two-outcome finite result domain");
    let domain_bytes = domain.to_bytes();
    let domain_id = identity(
        hashv(&[
            FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1,
            &[0],
            domain_bytes.as_slice(),
        ])
        .to_bytes(),
    );
    let template_instance = InstanceV1::new(InstanceV1Input {
        terms_id: product_id([9; 32]),
        occurrence_id: product_id([10; 32]),
        claim_basis_id: product_id(claim_id.to_bytes()),
        result_domain_id: product_id(domain_id.to_bytes()),
        capacity_profile_id: product_capacity,
        partition_cell_count: u32::from(OUTCOME_COUNT),
    })
    .expect("template Product");
    let template_instance_id = hash(&template_instance.to_bytes()).to_bytes();
    let source_schedule = source_schedule_identity_v1(&source_material_bytes(
        template_instance,
        template_instance_id,
        domain,
    ))
    .expect("reusable Source schedule");

    let template_bytes = template_bytes(fund_rent);
    let template_id = identity(hash(&template_bytes).to_bytes());
    let recipe_value = SeriesRecipeV1 {
        realm_id,
        terms_id: identity([9; 32]),
        claim_basis_id: claim_id,
        result_domain_id: domain_id,
        capacity_profile_id: capacity_id,
        compiler_release_id: identity(PRODUCT_COMPILER_RELEASE_ID_V1),
        occurrence_schedule_id: identity([11; 32]),
        source_schedule_id: source_schedule,
        capability_template_id: template_id,
        occurrence_derivation_release_id: identity(OCCURRENCE_DERIVATION_RELEASE_ID_V1),
        source_derivation_release_id: identity(SOURCE_DERIVATION_RELEASE_ID_V1),
        capability_derivation_release_id: identity(CAPABILITY_DERIVATION_RELEASE_ID_V1),
        market_derivation_release_id: identity(MARKET_DERIVATION_RELEASE_ID_V1),
        capitalization_schedule_id: identity([12; 32]),
        first_occurrence_time: 0,
        cadence_seconds: 1,
        occurrence_count: 1,
        first_generation: GENERATION,
        outcome_count: u16::from(OUTCOME_COUNT),
    };
    recipe_value.validate().expect("Series recipe");
    let recipe_id = identity(hash(&recipe_value.to_bytes()).to_bytes());
    let occurrence = derive_occurrence_product_v1(recipe_id, &recipe_value, 0)
        .expect("canonical occurrence Product");
    let instance = record(
        hash(INSTANCE_SCHEMA_LABEL).to_bytes(),
        occurrence.product_instance.to_bytes().to_vec(),
    );
    assert_eq!(
        hash(&instance.bytes).to_bytes(),
        occurrence.product_instance_id.to_bytes()
    );
    let source = record(
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
        source_material_bytes(
            occurrence.product_instance,
            occurrence.product_instance_id.to_bytes(),
            domain,
        ),
    );
    let source_id = identity(hash(&source.bytes).to_bytes());
    let source_facts = authenticate_occurrence_source_material_v1(source_id, &source.bytes)
        .expect("occurrence Source material");

    let template = CapabilityTemplateV1::decode(&template_bytes).expect("template decode");
    let projection = template
        .project_for_resolution_material(core_id(source_id.to_bytes()))
        .expect("Source-bound manifest projection");
    let (manifest_bytes, manifest_id, manifest_facts, funding) = {
        let mut encoded = vec![0; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        let manifest_view = projection
            .encode_into(&mut encoded)
            .expect("realized manifest");
        projection
            .validate_manifest(manifest_view)
            .expect("projection equality");
        let manifest_id = identity(hash(manifest_view.as_bytes()).to_bytes());
        let manifest_facts = authenticate_occurrence_capability_manifest_v1(
            template_id,
            &template_bytes,
            source_id,
            manifest_id,
            manifest_view.as_bytes(),
        )
        .expect("Source-bound manifest authentication");
        let selected = manifest_view
            .required_founding_entry_for_config(core_id(source_id.to_bytes()))
            .expect("one Found resolution entry");
        let funding = construct_required_resolution_funding(
            core_id(manifest_id.to_bytes()),
            manifest_view,
            selected,
            fund_rent,
            0,
        )
        .expect("required Found funding");
        (encoded, manifest_id, manifest_facts, funding)
    };
    let market_rent = Rent::default()
        .minimum_balance(CategoricalMarketV1::<2>::encoded_len().expect("Market width"));
    let market_principal = market_rent
        .checked_add(required_resolution_minimum_balance(funding).expect("Fund balance"))
        .expect("market principal");
    let ticket_rent =
        Rent::default().minimum_balance(dclutch_series_contract::OCCURRENCE_TICKET_BYTES_V1);
    let capitalization_value = OccurrenceCapitalizationV1 {
        recipe_id,
        capitalization_schedule_id: recipe_value.capitalization_schedule_id,
        occurrence_index: 0,
        market_principal,
        ticket_rent,
        total_principal: market_principal
            .checked_add(ticket_rent)
            .expect("total principal"),
        next_capitalization_id: None,
    };
    let capitalization = record(
        hash(CAPITALIZATION_SCHEMA_LABEL).to_bytes(),
        capitalization_value.to_bytes().to_vec(),
    );
    let capitalization_id = identity(hash(&capitalization.bytes).to_bytes());
    let aggregate_value = CapitalizationAggregateV1 {
        recipe_id,
        capitalization_schedule_id: recipe_value.capitalization_schedule_id,
        occurrence_count: 1,
        total_principal: capitalization_value.total_principal,
        first_capitalization_id: capitalization_id,
    };
    let aggregate = record(
        hash(AGGREGATE_SCHEMA_LABEL).to_bytes(),
        aggregate_value.to_bytes().to_vec(),
    );
    let aggregate_id = identity(hash(&aggregate.bytes).to_bytes());
    let derived_value = derive_occurrence_v1(
        recipe_id,
        &recipe_value,
        0,
        &capitalization_value,
        source_facts,
        manifest_facts,
    )
    .expect("canonical derived occurrence");
    let derived = record(
        hash(DERIVED_SCHEMA_LABEL).to_bytes(),
        derived_value.to_bytes().to_vec(),
    );
    let recipe = record(
        hash(RECIPE_SCHEMA_LABEL).to_bytes(),
        recipe_value.to_bytes().to_vec(),
    );
    assert_eq!(identity(hash(&recipe.bytes).to_bytes()), recipe_id);

    let mut substituted_bytes = vec![0; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    let substituted_entry = CapabilityEntryV1::new(
        core_id([21; 32]),
        core_id(PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1),
        core_id([91; 32]),
        core_id([22; 32]),
        core_id([23; 32]),
        core_id([24; 32]),
        ActivationPolicy::RequiredAtFounding,
        0,
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        resolution_quote(fund_rent),
    )
    .expect("substituted canonical manifest entry");
    CapabilityManifestV1::encode_into(&[substituted_entry], &mut substituted_bytes)
        .expect("substituted canonical manifest");

    let refund = RefundAuthority::new(sponsor.pubkey().to_bytes()).expect("refund authority");
    let (credit, credit_bump) = Pubkey::find_program_address(
        &[RENT_CREDIT_PDA_DOMAIN_V1, refund.to_bytes().as_slice()],
        &PROGRAM_ID,
    );
    let credit_state = RentCreditV1::new(refund, credit_bump);
    let (root, root_bump) = Pubkey::find_program_address(
        &[
            SERIES_ROOT_PDA_DOMAIN_V1,
            recipe_id.to_bytes().as_slice(),
            aggregate_id.to_bytes().as_slice(),
            sponsor.pubkey().as_ref(),
        ],
        &PROGRAM_ID,
    );
    let (escrow, escrow_bump) =
        Pubkey::find_program_address(&[SERIES_ESCROW_PDA_DOMAIN_V1, root.as_ref()], &PROGRAM_ID);
    let (guard, guard_bump) = Pubkey::find_program_address(
        &[SERIES_REPLAY_GUARD_PDA_DOMAIN_V1, root.as_ref()],
        &PROGRAM_ID,
    );
    let (ticket, ticket_bump) = Pubkey::find_program_address(
        &[
            SERIES_TICKET_PDA_DOMAIN_V1,
            root.as_ref(),
            &0_u64.to_le_bytes(),
        ],
        &PROGRAM_ID,
    );
    let market_identity_digest = derived_value.market_identity_id.to_bytes();
    let market = Pubkey::find_program_address(
        &[b"dclutch/market-root/v1", market_identity_digest.as_slice()],
        &PROGRAM_ID,
    )
    .0;
    let manifest_view =
        CapabilityManifestV1::decode(&manifest_bytes).expect("realized manifest decode");
    let funding_derivation = CapabilityFundingDerivationV1::new(
        market.to_bytes(),
        GENERATION,
        core_id(manifest_id.to_bytes()),
        manifest_view,
        funding,
    )
    .expect("canonical Fund derivation");
    let fund = Pubkey::find_program_address(&funding_derivation.seed_components(), &PROGRAM_ID).0;
    let manifest = record(CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, manifest_bytes);
    let substituted_manifest = record(CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, substituted_bytes);
    let template = record(CAPABILITY_TEMPLATE_SCHEMA_RELEASE_ID_V1, template_bytes);

    let mut test = ProgramTest::new("dclutch_sbf", PROGRAM_ID, None);
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    test.add_account(
        sponsor.pubkey(),
        Account::new(500_000_000, 0, &system_program::ID),
    );
    for final_record in [
        &realm,
        &instance,
        &claim,
        &capacity,
        &source,
        &recipe,
        &aggregate,
        &derived,
        &capitalization,
    ] {
        add_finalized(&mut test, final_record);
    }
    for staged_record in [&template, &manifest, &substituted_manifest] {
        add_vacant_record(&mut test, staged_record);
    }
    for (address, lamports) in [
        (root, ROOT_DUST),
        (escrow, ESCROW_DUST),
        (guard, GUARD_DUST),
        (ticket, TICKET_DUST),
        (market, MARKET_DUST),
        (fund, FUND_DUST),
    ] {
        test.add_account(address, Account::new(lamports, 0, &system_program::ID));
    }

    let sponsor_identity = identity(sponsor.pubkey().to_bytes());
    Fixture {
        test: Some(test),
        sponsor,
        credit,
        credit_state,
        template,
        manifest,
        substituted_manifest,
        root,
        escrow,
        guard,
        ticket,
        market,
        fund,
        realm,
        instance,
        claim,
        capacity,
        source,
        recipe,
        aggregate,
        derived,
        capitalization,
        create: CreateSeriesV1 {
            refund_authority: sponsor_identity,
            root_bump,
            escrow_bump,
            replay_guard_bump: guard_bump,
        },
        instantiate: InstantiateNextV1 {
            expected_index: 0,
            expected_time: recipe_value.first_occurrence_time,
            ticket_bump,
        },
        consume: ConsumeTicketV1 { expected_index: 0 },
    }
}

async fn submit(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    signers: &[&Keypair],
) -> Result<(), BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let mut all = vec![&context.payer];
    all.extend_from_slice(signers);
    context
        .banks_client
        .process_transaction(Transaction::new_signed_with_payer(
            instructions,
            Some(&context.payer.pubkey()),
            &all,
            blockhash,
        ))
        .await
}

async fn submit_with_cu(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    signers: &[&Keypair],
) -> Result<u64, BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let mut all = vec![&context.payer];
    all.extend_from_slice(signers);
    let processed = context
        .banks_client
        .process_transaction_with_metadata(Transaction::new_signed_with_payer(
            instructions,
            Some(&context.payer.pubkey()),
            &all,
            blockhash,
        ))
        .await?;
    processed.result?;
    processed
        .metadata
        .map(|metadata| metadata.compute_units_consumed)
        .ok_or(BanksClientError::ClientError(
            "missing transaction metadata",
        ))
}

async fn account(context: &mut ProgramTestContext, address: Pubkey) -> Option<Account> {
    context
        .banks_client
        .get_account(address)
        .await
        .expect("account query")
}

async fn snapshot(context: &mut ProgramTestContext, fixture: &Fixture) -> Snapshot {
    Snapshot {
        sponsor: account(context, fixture.sponsor.pubkey()).await,
        root: account(context, fixture.root).await,
        escrow: account(context, fixture.escrow).await,
        ticket: account(context, fixture.ticket).await,
        market: account(context, fixture.market).await,
        fund: account(context, fixture.fund).await,
        credit: account(context, fixture.credit).await,
    }
}

async fn create_credit(context: &mut ProgramTestContext, fixture: &Fixture) {
    submit(
        context,
        &[Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(fixture.sponsor.pubkey(), true),
                AccountMeta::new(fixture.credit, false),
                AccountMeta::new_readonly(system_program::ID, false),
                AccountMeta::new_readonly(sysvar::rent::ID, false),
            ],
            data: CreateRentCreditV1::new(
                fixture.credit_state.refund_authority(),
                fixture.credit_state.pda_bump(),
            )
            .to_bytes()
            .to_vec(),
        }],
        &[&fixture.sponsor],
    )
    .await
    .expect("routed RentCredit creation");
}

async fn finalize_routed_record(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    record: &Record,
    schema: [u8; 32],
) {
    let key = RecordKeyV1::new(
        SchemaReleaseId::new(schema).expect("schema release"),
        ContentDigest::new(hash(&record.bytes).to_bytes()).expect("record digest"),
    );
    let clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("ProgramTest Clock");
    let cursor_rent =
        Rent::default().minimum_balance(dclutch_record_contract::STAGING_CURSOR_BYTES_V1);
    let envelope = PageEnvelopeV1::new(
        PageEnvelopeKindV1::Provisional,
        u32::try_from(PAGE_BYTES).expect("bounded page width"),
        SchemaReleaseId::new(hash(PAGE_RELEASE_LABEL).to_bytes()).expect("page release"),
    )
    .expect("exact page envelope");
    let begin = BeginRecordV1::new(
        key,
        u64::try_from(record.bytes.len()).expect("record length"),
        envelope,
        SchemaReleaseId::new(hash(STAGING_RELEASE_LABEL).to_bytes()).expect("staging release"),
        clock
            .slot
            .checked_add(STAGING_LIFETIME_SLOTS)
            .expect("expiry slot"),
        cursor_rent,
    )
    .expect("routed record begin");
    submit(
        context,
        &[Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(fixture.sponsor.pubkey(), true),
                AccountMeta::new(record.raw, false),
                AccountMeta::new(record.cursor, false),
                AccountMeta::new_readonly(fixture.credit, false),
                AccountMeta::new_readonly(system_program::ID, false),
                AccountMeta::new_readonly(sysvar::rent::ID, false),
                AccountMeta::new_readonly(sysvar::clock::ID, false),
            ],
            data: begin.to_bytes().to_vec(),
        }],
        &[&fixture.sponsor],
    )
    .await
    .expect("routed record begin");
    for (index, chunk) in record.bytes.chunks(PAGE_BYTES).enumerate() {
        let page = AppendPageV1::new(
            u64::try_from(index).expect("page index"),
            u64::try_from(index.checked_mul(PAGE_BYTES).expect("page offset")).expect("offset"),
            chunk,
        )
        .expect("routed record append");
        let mut data = vec![0; page.encoded_len().expect("append width")];
        page.encode(&mut data).expect("append data");
        submit(
            context,
            &[Instruction {
                program_id: PROGRAM_ID,
                accounts: vec![
                    AccountMeta::new_readonly(fixture.sponsor.pubkey(), true),
                    AccountMeta::new(record.raw, false),
                    AccountMeta::new(record.cursor, false),
                ],
                data,
            }],
            &[&fixture.sponsor],
        )
        .await
        .expect("routed record append");
    }
    submit(
        context,
        &[Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new_readonly(record.raw, false),
                AccountMeta::new(record.cursor, false),
                AccountMeta::new(fixture.credit, false),
            ],
            data: FinalizeRecordV1.to_bytes().to_vec(),
        }],
        &[],
    )
    .await
    .expect("routed record finalization");
    let raw = account(context, record.raw)
        .await
        .expect("final raw record");
    let cursor = account(context, record.cursor)
        .await
        .expect("final cursor vacancy");
    assert_eq!(raw.owner, PROGRAM_ID);
    assert_eq!(raw.data, record.bytes);
    assert_eq!(cursor.owner, system_program::ID);
    assert_eq!(cursor.lamports, 0);
    assert!(cursor.data.is_empty());
}

fn create_accounts(fixture: &Fixture) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new(fixture.sponsor.pubkey(), true),
        AccountMeta::new_readonly(fixture.recipe.raw, false),
        AccountMeta::new_readonly(fixture.aggregate.raw, false),
        AccountMeta::new_readonly(fixture.capacity.raw, false),
        AccountMeta::new(fixture.root, false),
        AccountMeta::new(fixture.escrow, false),
        AccountMeta::new(fixture.guard, false),
        AccountMeta::new_readonly(fixture.credit, false),
        AccountMeta::new_readonly(fixture.recipe.cursor, false),
        AccountMeta::new_readonly(fixture.aggregate.cursor, false),
        AccountMeta::new_readonly(fixture.capacity.cursor, false),
        AccountMeta::new_readonly(fixture.template.raw, false),
        AccountMeta::new_readonly(fixture.template.cursor, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
    ]
}

fn instantiate_accounts(
    fixture: &Fixture,
    manifest: Pubkey,
    manifest_cursor: Pubkey,
) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new(fixture.sponsor.pubkey(), true),
        AccountMeta::new(fixture.root, false),
        AccountMeta::new_readonly(fixture.recipe.raw, false),
        AccountMeta::new_readonly(fixture.aggregate.raw, false),
        AccountMeta::new_readonly(fixture.capacity.raw, false),
        AccountMeta::new_readonly(fixture.derived.raw, false),
        AccountMeta::new_readonly(fixture.capitalization.raw, false),
        AccountMeta::new(fixture.escrow, false),
        AccountMeta::new(fixture.ticket, false),
        AccountMeta::new_readonly(fixture.recipe.cursor, false),
        AccountMeta::new_readonly(fixture.aggregate.cursor, false),
        AccountMeta::new_readonly(fixture.capacity.cursor, false),
        AccountMeta::new_readonly(fixture.derived.cursor, false),
        AccountMeta::new_readonly(fixture.capitalization.cursor, false),
        AccountMeta::new_readonly(fixture.source.raw, false),
        AccountMeta::new_readonly(fixture.source.cursor, false),
        AccountMeta::new_readonly(fixture.template.raw, false),
        AccountMeta::new_readonly(fixture.template.cursor, false),
        AccountMeta::new_readonly(manifest, false),
        AccountMeta::new_readonly(manifest_cursor, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
    ]
}

fn consume_accounts(
    fixture: &Fixture,
    manifest: Pubkey,
    manifest_cursor: Pubkey,
) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new(fixture.sponsor.pubkey(), true),
        AccountMeta::new(fixture.market, false),
        AccountMeta::new(fixture.fund, false),
        AccountMeta::new(fixture.credit, false),
        AccountMeta::new_readonly(fixture.realm.raw, false),
        AccountMeta::new_readonly(fixture.instance.raw, false),
        AccountMeta::new_readonly(fixture.claim.raw, false),
        AccountMeta::new_readonly(fixture.capacity.raw, false),
        AccountMeta::new_readonly(fixture.source.raw, false),
        AccountMeta::new_readonly(manifest, false),
        AccountMeta::new_readonly(fixture.realm.cursor, false),
        AccountMeta::new_readonly(fixture.instance.cursor, false),
        AccountMeta::new_readonly(fixture.claim.cursor, false),
        AccountMeta::new_readonly(fixture.capacity.cursor, false),
        AccountMeta::new_readonly(fixture.source.cursor, false),
        AccountMeta::new_readonly(manifest_cursor, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new(fixture.root, false),
        AccountMeta::new_readonly(fixture.recipe.raw, false),
        AccountMeta::new_readonly(fixture.aggregate.raw, false),
        AccountMeta::new_readonly(fixture.derived.raw, false),
        AccountMeta::new_readonly(fixture.capitalization.raw, false),
        AccountMeta::new(fixture.ticket, false),
        AccountMeta::new_readonly(fixture.recipe.cursor, false),
        AccountMeta::new_readonly(fixture.aggregate.cursor, false),
        AccountMeta::new_readonly(fixture.derived.cursor, false),
        AccountMeta::new_readonly(fixture.capitalization.cursor, false),
        AccountMeta::new_readonly(fixture.template.raw, false),
        AccountMeta::new_readonly(fixture.template.cursor, false),
    ]
}

fn series_instruction(accounts: Vec<AccountMeta>, data: Vec<u8>) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts,
        data,
    }
}

#[tokio::test]
async fn series_template_projection_is_routed_and_ticket_consume_is_atomic() {
    let mut fixture = fixture();
    let mut context = fixture
        .test
        .take()
        .expect("unstarted Series fixture")
        .start_with_context()
        .await;
    create_credit(&mut context, &fixture).await;
    finalize_routed_record(
        &mut context,
        &fixture,
        &fixture.template,
        CAPABILITY_TEMPLATE_SCHEMA_RELEASE_ID_V1,
    )
    .await;
    finalize_routed_record(
        &mut context,
        &fixture,
        &fixture.manifest,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    )
    .await;
    finalize_routed_record(
        &mut context,
        &fixture,
        &fixture.substituted_manifest,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    )
    .await;
    submit(
        &mut context,
        &[series_instruction(
            create_accounts(&fixture),
            fixture.create.to_bytes().to_vec(),
        )],
        &[&fixture.sponsor],
    )
    .await
    .expect("Series Create15");

    let before_substitution = snapshot(&mut context, &fixture).await;
    assert!(
        submit(
            &mut context,
            &[series_instruction(
                instantiate_accounts(
                    &fixture,
                    fixture.substituted_manifest.raw,
                    fixture.substituted_manifest.cursor,
                ),
                fixture.instantiate.to_bytes().to_vec(),
            )],
            &[&fixture.sponsor],
        )
        .await
        .is_err()
    );
    assert_eq!(snapshot(&mut context, &fixture).await, before_substitution);

    submit(
        &mut context,
        &[series_instruction(
            instantiate_accounts(&fixture, fixture.manifest.raw, fixture.manifest.cursor),
            fixture.instantiate.to_bytes().to_vec(),
        )],
        &[&fixture.sponsor],
    )
    .await
    .expect("Series Instantiate22");
    let before_consume_substitution = snapshot(&mut context, &fixture).await;
    assert!(
        submit(
            &mut context,
            &[series_instruction(
                consume_accounts(
                    &fixture,
                    fixture.substituted_manifest.raw,
                    fixture.substituted_manifest.cursor,
                ),
                fixture.consume.to_bytes().to_vec(),
            )],
            &[&fixture.sponsor],
        )
        .await
        .is_err()
    );
    assert_eq!(
        snapshot(&mut context, &fixture).await,
        before_consume_substitution,
        "Consume30 refuses a separately finalized but non-projected manifest before ticket mutation"
    );
    let before_atomic_replay = snapshot(&mut context, &fixture).await;
    let consume = series_instruction(
        consume_accounts(&fixture, fixture.manifest.raw, fixture.manifest.cursor),
        fixture.consume.to_bytes().to_vec(),
    );
    assert!(
        submit(
            &mut context,
            &[consume.clone(), consume.clone()],
            &[&fixture.sponsor],
        )
        .await
        .is_err()
    );
    assert_eq!(
        snapshot(&mut context, &fixture).await,
        before_atomic_replay,
        "second action-3 replay rolls back the first Found and ticket consumption"
    );

    let credit_before = account(&mut context, fixture.credit)
        .await
        .expect("RentCredit");
    let consume_cu = submit_with_cu(
        &mut context,
        std::slice::from_ref(&consume),
        &[&fixture.sponsor],
    )
    .await
    .expect("Series Consume30");
    assert!(consume_cu > 0);
    eprintln!("series template Consume30 CU: {consume_cu}");
    let after = snapshot(&mut context, &fixture).await;
    assert_eq!(
        after.sponsor, before_atomic_replay.sponsor,
        "the permissionless actor keeps no temporary Series funding credit"
    );
    let root = SeriesRootV1::decode(&after.root.as_ref().expect("root").data).expect("Series root");
    assert_eq!(root.outstanding_tickets, 0);
    assert_eq!(
        after.ticket.as_ref().expect("ticket").owner,
        system_program::ID
    );
    assert_eq!(after.ticket.as_ref().expect("ticket").lamports, 0);
    assert!(after.ticket.as_ref().expect("ticket").data.is_empty());
    assert_eq!(after.market.as_ref().expect("Market").owner, PROGRAM_ID);
    assert_eq!(after.fund.as_ref().expect("Fund").owner, PROGRAM_ID);
    let credit = after.credit.as_ref().expect("RentCredit");
    assert!(credit.lamports > credit_before.lamports);
    assert_eq!(RentCreditV1::decode(&credit.data), Ok(fixture.credit_state));
    assert!(
        submit(&mut context, &[consume], &[&fixture.sponsor])
            .await
            .is_err()
    );
    assert_eq!(snapshot(&mut context, &fixture).await, after);
}
