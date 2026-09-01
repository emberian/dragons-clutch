//! One current-source General `OpenBatch` through real Trading and accelerator ELFs.

#![allow(clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

use std::{env, fs, path::PathBuf};

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityManifestV1,
    FundingQuoteV1, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
    funding::{CompartmentFundingV1, FundingAmountsV1},
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1, SelectedRecordBumpsV1,
    hot_v3::DIRECT_HOT_HEAP_FRAME_BYTES_V1, set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
    v4::CapabilityProgramV4,
};
use dclutch_chain_bundle_builder::{
    WaistFactsV1,
    admitted::AdmittedAotInputV1,
    artifacts::{ArtifactSetV1, DerivedRecordV1, derive_record, digest},
    bundle::{BundleInputV1, FixedCorpusV1, ScenarioV1},
    frame::{BuiltAccountV1, data_account, external_with_view, program_with_view, vacant},
    general::{
        GeneralOpenBatchRequestInputV1, build_general_open_batch_bundle_v1,
        derive_general_open_batch_request_v1,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_direct_hot_program_test_support::waist;
use dclutch_general_adapter_contract::{
    account_rules_v3::GeneralExternalAccountWidthsV3,
    collection_v1::{GeneralBatchOccurrenceTermsV1, GeneralBatchV1},
    local_state_v3::GeneralLocalStateV3,
    state_artifacts_v3::{
        GENERAL_PRIMARY_PAYER_ACCOUNT_V3, GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3,
        GENERAL_PRIMARY_STATE_ACCOUNT_V3,
    },
};
use dclutch_general_codec::Action;
use dclutch_general_config_contract::{GENERAL_ROOT_BYTES_V2, GeneralRootV2};
use dclutch_market_core_codec::{
    CoreState, Identity as CoreIdentity, MarketCoreStateSeedsV2, MarketIdentity, Phase, Readiness,
    STATE_BYTES as CORE_STATE_BYTES, StateBumpsV1,
};
use dclutch_operator::general_selected_release_v1::{
    GeneralConfigWindowsV1, GeneralDeploymentFactsV1, GeneralSelectedReleaseInputV1,
    GeneralSelectedReleaseV1, general_selected_release_v1,
};
use dclutch_product_payoff_v2_codec::{
    registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
    runtime_v3::{
        BasisInputV3, BasisKindV3, SEMANTIC_BASIS_CONTENT_DOMAIN_V3, basis_record_bytes_v3,
        compile_basis_v3, semantic_basis_preimage_v3,
    },
};
use dclutch_product_runtime_v2::{
    ContentId as ProductContentId, portfolio_record_bytes, result_domain_record_bytes,
};
use dclutch_product_runtime_v2_admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_BYTES_V2, PRODUCT_RECORD_SCHEMA_ID_V2,
    RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_product_runtime_v2_operator::{ProductCompilationInputV2, compile_product_records_v2};
use dclutch_realm_contract::{REALM_BYTES, REALM_SCHEMA_RELEASE_ID_V1};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1;
use dclutch_release_set_contract::{ArtifactReleaseIdV1, CapabilityExecutionSelectionV1};
use dclutch_rent_contract::{
    RefundAuthority,
    lifecycle_v2::{
        LIFECYCLE_RENT_CREDIT_BYTES_V2, LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleAccountIdV2,
        LifecycleRentCreditV2,
    },
};
use solana_account::Account;
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_program::{hash::hash, pubkey::Pubkey, rent::Rent};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program};

const ACCELERATOR_PROGRAM: Pubkey = Pubkey::new_from_array([0xa1; 32]);
const GENERATION: u64 = 9;
const PRICE_SCALE: u64 = 1_000_000;
const CLAIM_BASIS: [u8; 32] = [0x56; 32];

#[derive(Clone)]
struct ProductRecords {
    product_id: [u8; 32],
    product: DerivedRecordV1,
    domain: DerivedRecordV1,
    portfolio: DerivedRecordV1,
    basis: DerivedRecordV1,
}

fn product_content(value: [u8; 32]) -> ProductContentId {
    ProductContentId::new(value).expect("nonzero Product identity")
}

fn build_product(outcome_count: u32) -> ProductRecords {
    let registry = waist::REGISTRY_PROGRAM_ID;
    let product_id = product_content([0x51; 32]);
    let coordinate_domain = product_content([0x52; 32]);
    let result_unit = product_content([0x53; 32]);
    let provisional_input = BasisInputV3 {
        kind: BasisKindV3::CategoricalQ1,
        product_id: product_id.to_bytes(),
        result_domain_id: [0x54; 32],
        coordinate_domain_id: coordinate_domain.to_bytes(),
        result_unit_id: result_unit.to_bytes(),
        evaluator_release_id: [0x55; 32],
        basis_width: outcome_count,
        payout_scale: 1,
        knot_denominator: 1,
        knots: &[],
        terms: &[],
        failure_payouts: &[],
        price_gate_certificate_digest: [0; 32],
    };
    let outcomes = usize::try_from(outcome_count).expect("outcome width");
    let basis_bytes =
        basis_record_bytes_v3(BasisKindV3::CategoricalQ1, outcomes, 0, 0).expect("basis width");
    let mut provisional = vec![0; basis_bytes];
    compile_basis_v3(provisional_input, &mut provisional).expect("provisional basis");
    let semantic = semantic_basis_preimage_v3(&provisional).expect("semantic basis preimage");
    let semantic_basis = solana_program::hash::hashv(&[
        SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
        semantic.prefix(),
        semantic.suffix(),
    ])
    .to_bytes();
    let cut_count = outcome_count.checked_sub(2).expect("categorical width");
    let cuts = (0..i128::from(cut_count)).collect::<Vec<_>>();
    let coefficients = vec![1; outcomes];
    let mut product_bytes = vec![0; PRODUCT_RECORD_BYTES_V2];
    let mut domain_bytes = vec![0; result_domain_record_bytes(cuts.len()).expect("domain width")];
    let mut portfolio_bytes =
        vec![0; portfolio_record_bytes(coefficients.len()).expect("portfolio width")];
    compile_product_records_v2(
        registry,
        ProductCompilationInputV2 {
            product_id,
            coordinate_domain_id: coordinate_domain,
            result_unit_id: result_unit,
            claim_basis_id: product_content(CLAIM_BASIS),
            liability_basis_id: product_content(semantic_basis),
            representation_release_id: product_content([0x57; 32]),
            mapping_release_id: product_content([0x58; 32]),
            cut_denominator: 1,
            cuts: &cuts,
            portfolio_denominator: 1,
            coefficients: &coefficients,
        },
        &mut product_bytes,
        &mut domain_bytes,
        &mut portfolio_bytes,
    )
    .expect("Product compiler");
    let product = derive_record(registry, PRODUCT_RECORD_SCHEMA_ID_V2, &product_bytes);
    let domain = derive_record(registry, RESULT_DOMAIN_SCHEMA_ID_V2, &domain_bytes);
    let portfolio = derive_record(registry, PORTFOLIO_SCHEMA_ID_V2, &portfolio_bytes);
    let mut linked_basis = vec![0; basis_bytes];
    compile_basis_v3(
        BasisInputV3 {
            result_domain_id: domain.digest,
            ..provisional_input
        },
        &mut linked_basis,
    )
    .expect("linked basis");
    let basis = derive_record(registry, GRADED_BASIS_RECORD_SCHEMA_ID_V3, &linked_basis);
    ProductRecords {
        product_id: product_id.to_bytes(),
        product,
        domain,
        portfolio,
        basis,
    }
}

fn load_accelerator_elf() -> Vec<u8> {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR"));
    let path = env::var_os("DCLUTCH_GENERAL_ACCELERATOR_ELF_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| directory.join("dclutch_general_accelerator_sbf.so"));
    fs::read(path).expect("current General accelerator ELF")
}

fn record_bumps(schema: [u8; 32], content: [u8; 32]) -> (u8, u8) {
    (
        Pubkey::find_program_address(
            &[RAW_RECORD_PDA_SEED_V1, &schema, &content],
            &waist::REGISTRY_PROGRAM_ID,
        )
        .1,
        Pubkey::find_program_address(
            &[STAGING_CURSOR_PDA_SEED_V1, &schema, &content],
            &waist::REGISTRY_PROGRAM_ID,
        )
        .1,
    )
}

fn core_identity(value: [u8; 32]) -> CoreIdentity {
    CoreIdentity::new(value).expect("nonzero Core identity")
}

fn content(value: [u8; 32]) -> ContentId {
    ContentId::new(value).expect("nonzero identity")
}

fn selected_release(
    outcome_count: u32,
    product: &ProductRecords,
    accelerator_release: ArtifactReleaseIdV1,
) -> GeneralSelectedReleaseV1 {
    general_selected_release_v1(GeneralSelectedReleaseInputV1 {
        capacity_profile: [0x41; 32],
        claim_basis: CLAIM_BASIS,
        selection_policy: [0x43; 32],
        quote_surplus_beneficiary: [0x44; 32],
        generation: GENERATION,
        price_scale: PRICE_SCALE,
        windows: GeneralConfigWindowsV1 {
            collection_slots: 16,
            selection_slots: 16,
            settlement_slots: 64,
            max_orders_per_candidate: 32,
            max_pages_per_candidate: 32,
            continuation_reward_lamports: 1,
        },
        outcome_count,
        external_widths: GeneralExternalAccountWidthsV3 {
            linked_basis_prefix: u32::try_from(product.basis.bytes.len()).expect("basis width"),
            result_domain: u32::try_from(product.domain.bytes.len()).expect("domain width"),
            rent_sysvar: 17,
            core_market: u32::try_from(CORE_STATE_BYTES).expect("Core width"),
            activation_cache: u32::try_from(ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1)
                .expect("activation width"),
            upgradeable_program: 36,
            trading_programdata_prefix: 45,
            claims_programdata_prefix: 45,
            core_programdata_prefix: 45,
            realm_record: u32::try_from(REALM_BYTES).expect("Realm width"),
            rent_credit: u32::try_from(LIFECYCLE_RENT_CREDIT_BYTES_V2).expect("RentCredit width"),
        },
        token_account_bytes: 165,
        deployment: GeneralDeploymentFactsV1 {
            accelerator_artifact_release: accelerator_release.to_bytes(),
            compiler_release: [0x52; 32],
            toolchain: [0x53; 32],
            translation_validation: [0x54; 32],
        },
    })
    .expect("current General release")
}

struct ManifestSelection {
    bytes: Vec<u8>,
    selection: CapabilityExecutionSelectionV1,
    record_bumps: SelectedRecordBumpsV1,
}

fn manifest_selection(
    release: &GeneralSelectedReleaseV1,
    descriptor: CapabilityProgramV4,
    clock_slot: u64,
) -> ManifestSelection {
    let amounts = FundingAmountsV1::new(
        CompartmentFundingV1::native_lamports(1).expect("native funding"),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
    )
    .expect("funding compartments");
    let entry = CapabilityEntryV1::new(
        dclutch_capability_contract::ContentId::new(descriptor.kind().to_bytes()).expect("kind"),
        dclutch_capability_contract::ContentId::new(digest(&release.program_set))
            .expect("ProgramSet"),
        dclutch_capability_contract::ContentId::new(digest(&release.config)).expect("config"),
        dclutch_capability_contract::ContentId::new(descriptor.capacity_profile().to_bytes())
            .expect("capacity"),
        dclutch_capability_contract::ContentId::new(descriptor.root_schema().to_bytes())
            .expect("root schema"),
        dclutch_capability_contract::ContentId::new(descriptor.derivation_policy().to_bytes())
            .expect("lifecycle"),
        ActivationPolicy::PrepaidLazy,
        clock_slot.checked_add(100).expect("activation deadline"),
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        FundingQuoteV1::new(amounts, None).expect("funding quote"),
    )
    .expect("manifest entry");
    let mut bytes = vec![0; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&[entry], &mut bytes).expect("manifest");
    let manifest_digest = digest(&bytes);
    let program_set_digest = digest(&release.program_set);
    let config_digest = digest(&release.config);
    let program_set_bumps = record_bumps(
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        program_set_digest,
    );
    let manifest_bumps = record_bumps(
        dclutch_capability_contract::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        manifest_digest,
    );
    let config_bumps = record_bumps(descriptor.config_schema().to_bytes(), config_digest);
    let selection = CapabilityExecutionSelectionV1::new(
        0,
        content(manifest_digest),
        content(descriptor.kind().to_bytes()),
        content(program_set_digest),
        content(config_digest),
    )
    .expect("selection")
    .with_capability_release_record_bumps(program_set_bumps.0, program_set_bumps.1);
    ManifestSelection {
        bytes,
        selection,
        record_bumps: SelectedRecordBumpsV1::new(
            manifest_bumps.0,
            manifest_bumps.1,
            config_bumps.0,
            config_bumps.1,
        ),
    }
}

struct StateCorpus {
    market: BuiltAccountV1,
    root: BuiltAccountV1,
    rent_credit: BuiltAccountV1,
}

fn state_corpus(
    rent: &Rent,
    payer: Pubkey,
    release_set: [u8; 32],
    product: &ProductRecords,
    manifest: &ManifestSelection,
    release: &GeneralSelectedReleaseV1,
) -> StateCorpus {
    let realm_id = hash(&[0x77; REALM_BYTES]).to_bytes();
    let provisional = MarketIdentity {
        market_id: core_identity([0x61; 32]),
        realm_id: core_identity(realm_id),
        product_record: core_identity(product.product.digest),
        product_id: core_identity(product.product_id),
        resolution_policy: core_identity([0x62; 32]),
        capability_manifest: core_identity(digest(&manifest.bytes)),
        selected_release_set: core_identity(release_set),
        registry_program: core_identity(waist::REGISTRY_PROGRAM_ID.to_bytes()),
        generation: GENERATION,
    };
    let market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(provisional).as_slices(),
        &waist::CORE_PROGRAM_ID,
    )
    .0;
    let identity = MarketIdentity {
        market_id: core_identity(market.to_bytes()),
        ..provisional
    };
    let (rederived_market, market_bump) = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(identity).as_slices(),
        &waist::CORE_PROGRAM_ID,
    );
    assert_eq!(rederived_market, market);
    let realm_bumps = record_bumps(REALM_SCHEMA_RELEASE_ID_V1, realm_id);
    let (rent_credit, rent_credit_bump) = Pubkey::find_program_address(
        &[
            LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
            market.as_ref(),
            &GENERATION.to_le_bytes(),
        ],
        &waist::RENT_PROGRAM_ID,
    );
    let credit = LifecycleRentCreditV2::new(
        RefundAuthority::new(payer.to_bytes()).expect("refund wallet"),
        LifecycleAccountIdV2::new(market.to_bytes()).expect("Market"),
        LifecycleAccountIdV2::new(release_set).expect("release set"),
        GENERATION,
        rent_credit_bump,
    )
    .expect("RentCredit");
    let market_bytes = CoreState {
        phase: Phase::Open,
        readiness: Readiness::Consumed,
        terminal_winner: 0,
        identity,
        outstanding_capabilities: 1,
        principal_cap_sets: u64::MAX,
        rent_beneficiary: core_identity(rent_credit.to_bytes()),
        terminal_receipt: None,
        bumps: StateBumpsV1 {
            market: StateBumpsV1::record(market_bump),
            realm_raw_record: StateBumpsV1::record(realm_bumps.0),
            realm_staging_record: StateBumpsV1::record(realm_bumps.1),
        },
    }
    .encode()
    .expect("Core Market")
    .to_vec();
    let header = CapabilityRootHeaderV1::new(
        content(release_set),
        market.to_bytes(),
        GENERATION,
        manifest.selection,
        manifest.record_bumps,
    )
    .expect("root header");
    let root =
        Pubkey::find_program_address(&header.seeds().as_slices(), &waist::TRADING_PROGRAM_ID).0;
    let root_tail = GeneralRootV2::active(market.to_bytes(), digest(&release.config), GENERATION)
        .expect("active General root");
    let mut root_bytes =
        Vec::with_capacity(CAPABILITY_ROOT_HEADER_BYTES_V1 + GENERAL_ROOT_BYTES_V2);
    root_bytes.extend_from_slice(&header.to_bytes());
    root_bytes.extend_from_slice(&root_tail.to_bytes());
    StateCorpus {
        market: data_account(rent, market, waist::CORE_PROGRAM_ID, market_bytes),
        root: data_account(rent, root, waist::TRADING_PROGRAM_ID, root_bytes),
        rent_credit: data_account(
            rent,
            rent_credit,
            waist::RENT_PROGRAM_ID,
            credit.to_bytes().to_vec(),
        ),
    }
}

struct HostCase {
    built: dclutch_chain_bundle_builder::bundle::BuiltAdmittedBundleV1,
    batch: Pubkey,
    root: Pubkey,
    rent_credit: Pubkey,
}

#[allow(clippy::too_many_arguments)]
fn build_host_case(
    outcome_count: u32,
    payer: Pubkey,
    rent: &Rent,
    substrate: waist::FixtureSubstrateV1,
    elves: &waist::Elves,
    releases: waist::Releases,
    accelerator_elf: &[u8],
) -> HostCase {
    let product = build_product(outcome_count);
    let accelerator_artifact =
        waist::release_v2(ACCELERATOR_PROGRAM, 0x71, accelerator_elf, substrate);
    let release = selected_release(
        outcome_count,
        &product,
        waist::artifact_id(accelerator_artifact),
    );
    let selected = release
        .bundles
        .iter()
        .find(|bundle| bundle.action == Action::OpenBatch)
        .expect("OpenBatch bundle");
    let descriptor = CapabilityProgramV4::decode(&selected.descriptor).expect("descriptor");
    let clock_slot = substrate.bank_slot();
    let manifest = manifest_selection(&release, descriptor, clock_slot);
    let state = state_corpus(
        rent,
        payer,
        releases.release_set,
        &product,
        &manifest,
        &release,
    );
    let root_tail = GeneralRootV2::decode(
        state
            .root
            .account
            .data
            .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .expect("root tail"),
    )
    .expect("General root");
    let request = derive_general_open_batch_request_v1(GeneralOpenBatchRequestInputV1 {
        root: root_tail,
        root_address: state.root.key,
        config: &release.config,
        outcome_count,
        product_id: product.product_id,
        trading_program: waist::TRADING_PROGRAM_ID,
    })
    .expect("chain-derived OpenBatch request");
    let trading_release =
        waist::release_v2(waist::TRADING_PROGRAM_ID, 0x33, &elves.trading, substrate);
    let waist_facts = WaistFactsV1 {
        registry_program: waist::REGISTRY_PROGRAM_ID,
        trading_program: waist::TRADING_PROGRAM_ID,
        core_program: waist::CORE_PROGRAM_ID,
        claims_program: waist::CLAIMS_PROGRAM_ID,
        custody_program: waist::CUSTODY_PROGRAM_ID,
        release_set: releases.release_set,
        activation_cache: releases.activation,
        trading_semantic_release: trading_release.semantic_release_id().to_bytes(),
    };
    let accelerator_programdata = waist::programdata(ACCELERATOR_PROGRAM);
    let accelerator_program = program_with_view(ACCELERATOR_PROGRAM, accelerator_programdata);
    let accelerator_programdata_account = external_with_view(
        accelerator_programdata,
        bpf_loader_upgradeable::ID,
        waist::programdata_v2(substrate, accelerator_elf),
    );
    let payer_binding = vacant(payer).with_observed(Account {
        lamports: 10_000_000_000,
        data: Vec::new(),
        owner: system_program::ID,
        executable: false,
        rent_epoch: 0,
    });
    let bindings = vec![
        (usize::from(GENERAL_PRIMARY_PAYER_ACCOUNT_V3), payer_binding),
        (
            usize::from(GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3),
            state.rent_credit.clone(),
        ),
    ];
    let set = ArtifactSetV1 {
        descriptor: &selected.descriptor,
        account_profile: &selected.account_profile,
        request_profile: &selected.request_profile,
        transition: &selected.transition,
        effect: &selected.effect,
        lifecycle: &selected.lifecycle_policy,
        strategy: &selected.strategy,
        program_set: &release.program_set,
        manifest: &manifest.bytes,
        config: &release.config,
    };
    let externally_installed = [ACCELERATOR_PROGRAM, accelerator_programdata];
    let input = BundleInputV1 {
        set,
        waist: waist_facts,
        scenario: ScenarioV1 {
            family_request: &request.request,
            tail_count: outcome_count,
            clock_slot,
            generation: GENERATION,
            ed25519_evidence: None,
            native_message_instruction_index: 2,
            externally_installed_extra: &externally_installed,
            payer,
        },
        fixed: FixedCorpusV1 {
            market: state.market,
            root: state.root.clone(),
            product: product.product,
            result_domain: product.domain,
            portfolio: product.portfolio,
            linked_basis: product.basis,
            core_programdata: releases.core_programdata,
            trading_programdata: releases.trading_programdata,
        },
        bindings: &bindings,
        rent,
    };
    let built = build_general_open_batch_bundle_v1(
        &input,
        AdmittedAotInputV1 {
            certificate: Some(&selected.certificate),
            admission: Some(&selected.admission),
            artifact_release: Some(&accelerator_artifact.to_bytes()),
            accelerator_program: Some(&accelerator_program),
            accelerator_programdata: Some(&accelerator_programdata_account),
        },
    )
    .expect("complete admitted OpenBatch bundle");
    assert_eq!(built.bundle.span_counts.len(), 1);
    assert_eq!(built.bundle.transport_span, Some(0));
    HostCase {
        built,
        batch: request.batch,
        root: state.root.key,
        rent_credit: state.rent_credit.key,
    }
}

fn add_case_accounts(
    test: &mut solana_program_test::ProgramTest,
    case: &HostCase,
    payer: &Keypair,
    fee_payer: &Keypair,
) {
    for install in &case.built.bundle.accounts {
        if !case
            .built
            .bundle
            .externally_installed_keys
            .contains(&install.key)
        {
            test.add_account(install.key, install.account.clone());
        }
    }
    for signer in [payer, fee_payer] {
        test.add_account(
            signer.pubkey(),
            Account {
                lamports: 10_000_000_000,
                data: Vec::new(),
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            },
        );
    }
}

async fn execute_open_batch(outcome_count: u32) {
    assert_eq!(DIRECT_HOT_HEAP_FRAME_BYTES_V1, 65_536);
    let substrate = waist::fixture_substrate();
    let elves = waist::elves();
    let accelerator_elf = load_accelerator_elf();
    let rent = Rent::default();
    let payer = Keypair::new();
    let fee_payer = Keypair::new();
    let mut test = waist::program_test_without_forced_budget(&elves);
    let releases = waist::add_release_waist_v2(&mut test, &elves, substrate);
    waist::add_program_v2(
        &mut test,
        "dclutch_general_accelerator_sbf",
        ACCELERATOR_PROGRAM,
        &accelerator_elf,
        substrate,
    );
    let case = build_host_case(
        outcome_count,
        payer.pubkey(),
        &rent,
        substrate,
        &elves,
        releases,
        &accelerator_elf,
    );
    assert_eq!(
        case.built.bundle.span_counts,
        vec![
            u32::try_from(case.built.admitted_authorities.entries.len())
                .expect("scratch page count")
        ]
    );
    assert!(
        case.built.bundle.hot_instruction.accounts.len() <= 100,
        "the exact ALT route stays under the v0 account ceiling"
    );
    eprintln!(
        "general-open-batch geometry N={outcome_count} instruction_accounts={} logical={} span_counts={:?} chunk_authorities={} runtime_start={}",
        case.built.bundle.hot_instruction.accounts.len(),
        case.built.bundle.logical.len(),
        case.built.bundle.span_counts,
        case.built.admitted_authorities.entries.len(),
        47 + case.built.admitted_authorities.entries.len(),
    );
    // The accelerator's OpenBatch admission joins the request's witnessed
    // `STATE_BUMP` to the lifecycle's `PRIMARY_CANONICAL_BUMP` and nothing
    // else, so two derivations that name DIFFERENT accounts still satisfy it
    // whenever their canonical bump bytes happen to agree. Join the addresses
    // here, where both are in hand, or a bump collision silently hides a
    // lifecycle recipe reading a register no artifact ever writes.
    assert_eq!(
        case.built
            .bundle
            .logical
            .get(usize::from(GENERAL_PRIMARY_STATE_ACCOUNT_V3))
            .expect("primary state coordinate")
            .key,
        case.batch,
        "the lifecycle-derived primary state is not the Batch PDA the request names"
    );
    add_case_accounts(&mut test, &case, &payer, &fee_payer);
    let instructions = vec![
        ComputeBudgetInstruction::request_heap_frame(DIRECT_HOT_HEAP_FRAME_BYTES_V1),
        ComputeBudgetInstruction::set_compute_unit_limit(
            u32::try_from(waist::COMPUTE_LIMIT).expect("compute limit"),
        ),
        case.built.bundle.hot_instruction.clone(),
    ];
    let lookup_addresses = waist::canonical_lookup_addresses(&instructions, fee_payer.pubkey());
    waist::add_lookup_table(&mut test, &lookup_addresses);
    let mut context = waist::start_with_substrate(test, substrate).await;
    let payer_before = context
        .banks_client
        .get_account(payer.pubkey())
        .await
        .expect("payer query")
        .expect("payer account");
    let root_before = context
        .banks_client
        .get_account(case.root)
        .await
        .expect("root query")
        .expect("root account");
    let credit_before = context
        .banks_client
        .get_account(case.rent_credit)
        .await
        .expect("RentCredit query")
        .expect("RentCredit account");
    assert!(
        context
            .banks_client
            .get_account(case.batch)
            .await
            .expect("Batch query")
            .is_none()
    );
    let execution = waist::submit_v0_observed(
        &mut context,
        &instructions,
        lookup_addresses,
        Some(&fee_payer),
        &[&payer],
    )
    .await
    .expect("real Trading -> General accelerator OpenBatch");
    assert!(
        execution
            .logs
            .iter()
            .any(|line| line.contains(&format!("Program {ACCELERATOR_PROGRAM} invoke"))),
        "the success log proves the real accelerator CPI ran"
    );
    let root_after = context
        .banks_client
        .get_account(case.root)
        .await
        .expect("root query")
        .expect("root account");
    let batch_after = context
        .banks_client
        .get_account(case.batch)
        .await
        .expect("Batch query")
        .expect("materialized Batch");
    let payer_after = context
        .banks_client
        .get_account(payer.pubkey())
        .await
        .expect("payer query")
        .expect("payer account");
    let credit_after = context
        .banks_client
        .get_account(case.rent_credit)
        .await
        .expect("RentCredit query")
        .expect("RentCredit account");
    assert_eq!(
        credit_after, credit_before,
        "OpenBatch does not spend credit"
    );
    assert_eq!(batch_after.owner, waist::TRADING_PROGRAM_ID);
    assert_eq!(
        payer_before.lamports - payer_after.lamports,
        batch_after.lamports,
        "the isolated protocol payer funds exactly the new Batch principal"
    );
    let local = GeneralLocalStateV3::decode(&batch_after.data).expect("local Batch envelope");
    let decoded_batch = GeneralBatchV1::decode(local.body()).expect("Batch");
    let occurrence = GeneralBatchOccurrenceTermsV1::new(decoded_batch.opening())
        .expect("Batch occurrence")
        .occurrence_id();
    assert_eq!(
        local.header().bump,
        Pubkey::find_program_address(
            dclutch_general_adapter_contract::state_seeds_v3::GeneralStateAddressSeedsV3::batch(
                case.root.to_bytes(),
                occurrence,
            )
            .expect("Batch seeds")
            .as_slices()
            .expect("Batch seed slices")
            .as_slice(),
            &waist::TRADING_PROGRAM_ID,
        )
        .1
    );
    assert_eq!(local.header().rent_principal, batch_after.lamports);
    assert_eq!(local.header().beneficiary, case.rent_credit.to_bytes());
    let batch = decoded_batch;
    assert_eq!(batch.opening().outcome_count, outcome_count);
    assert_eq!(batch.opening().generation, GENERATION);
    assert_eq!(
        batch.opening().market,
        case.built.invocation_context.market.to_bytes()
    );
    assert_eq!(batch.opening().price_scale, PRICE_SCALE);
    assert_eq!(batch.opening().max_orders, 32);
    assert!(batch.opening().collection_close_slot > substrate.bank_slot());
    assert!(batch.opening().settlement_close_slot > batch.opening().collection_close_slot);
    assert_eq!(batch.state().order_count, 0);
    assert_eq!(batch.state().opened_root_revision, 1);
    let root_tail = |account: &Account| {
        GeneralRootV2::decode(
            account
                .data
                .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
                .expect("root tail"),
        )
        .expect("General root")
    };
    let before = root_tail(&root_before);
    let after = root_tail(&root_after);
    assert_eq!(before.revision(), 1);
    assert_eq!(before.next_batch_sequence(), 0);
    assert_eq!(before.open_batches(), 0);
    assert_eq!(after.revision(), 2);
    assert_eq!(after.next_batch_sequence(), 1);
    assert_eq!(after.open_batches(), 1);
    assert_eq!(
        root_before.lamports + payer_before.lamports,
        root_after.lamports + payer_after.lamports + batch_after.lamports,
        "root principal is conserved and one exact Batch principal moves from payer"
    );
    eprintln!(
        "general-open-batch N={outcome_count} pages={} accounts={} cu={} batch={} root_revision={}=>{}",
        case.built.bundle.span_counts[0],
        case.built.bundle.hot_instruction.accounts.len(),
        execution.compute_units_consumed,
        case.batch,
        before.revision(),
        after.revision(),
    );
}

#[tokio::test]
async fn real_elf_open_batch_runs_at_small_and_maximum_width() {
    execute_open_batch(2).await;
    execute_open_batch(258).await;
}
