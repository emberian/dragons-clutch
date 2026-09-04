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
    hot_v3::{
        DIRECT_HOT_HEAP_FRAME_BYTES_V1, HOT_CAPABILITY_SEAL_ACCOUNT_V3, HOT_FIXED_ACCOUNT_COUNT_V3,
    },
    set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
    v4::CapabilityProgramV4,
};
use dclutch_capability_seal_contract::{CAPABILITY_SEAL_BYTES_V1, SealedDescriptorClosureV1};
use dclutch_chain_bundle_builder::{
    WaistFactsV1,
    admitted::AdmittedAotInputV1,
    artifacts::{ArtifactSetV1, DerivedRecordV1, derive_record, digest},
    bundle::{BundleInputV1, FixedCorpusV1, ScenarioV1},
    frame::{
        BuiltAccountV1, SYSTEM_PROGRAM_BUILTIN_NAME_V1, data_account, external_with_view,
        program_with_view, system_program_builtin, vacant,
    },
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
        GENERAL_PRIMARY_STATE_ACCOUNT_V3, general_system_program_account_v3,
    },
};
use dclutch_general_codec::Action;
use dclutch_general_config_contract::{GENERAL_ROOT_BYTES_V2, GeneralRootV2};
use dclutch_market_core_codec::{
    CoreState, Identity as CoreIdentity, MarketCoreStateSeedsV2, MarketIdentity, Phase, Readiness,
    StateBumpsV1,
};
use dclutch_operator::capability_seal_v1::{
    CapabilitySealInstructionInputV1, capability_seal_instruction_v1,
};
use dclutch_operator::general_selected_release_v1::{
    GeneralConfigWindowsV1, GeneralDeploymentFactsV1, GeneralSelectedReleaseInputV1,
    GeneralSelectedReleaseV1, general_external_account_widths_v3,
    general_selected_entry_descriptor_v1, general_selected_release_v1,
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
use dclutch_record_contract::{ContentDigest, RecordKeyV1, RecordPdaSeedsV1, SchemaReleaseId};
use dclutch_release_set_contract::{ArtifactReleaseIdV1, CapabilityExecutionSelectionV1};
use dclutch_rent_contract::{
    RefundAuthority,
    lifecycle_v2::{
        LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleAccountIdV2, LifecycleRentCreditV2,
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

/// Both bumps derive through `RecordKeyV1`, the constructor the Record
/// contract exports for exactly this, rather than respelling the seed tuple
/// here. A test that spells the tuple becomes a second author for the address,
/// and the seam register's rule is that a NEW file restating an existing
/// domain is corrected rather than filed beside the existing debt.
fn record_bumps(schema: [u8; 32], content: [u8; 32]) -> (u8, u8) {
    let key = RecordKeyV1::new(
        SchemaReleaseId::new(schema).expect("schema release id"),
        ContentDigest::new(content).expect("content digest"),
    );
    let bump = |seeds: RecordPdaSeedsV1| {
        Pubkey::find_program_address(
            &[
                seeds.domain(),
                seeds.schema_release_id().as_bytes(),
                seeds.expected_digest().as_bytes(),
            ],
            &waist::REGISTRY_PROGRAM_ID,
        )
        .1
    };
    (
        bump(key.raw_record_pda_seeds()),
        bump(key.staging_cursor_pda_seeds()),
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
        // ONE AUTHOR, and this test used to be the only site that had it
        // right: it read the contracts while `general_market.rs`, the devnet
        // policy file and the operator's own fixture spelled the unit-test
        // literals, which is how cohort-14 founded a General market whose
        // `OpenBatch` names an `Exact(48)` RentCredit no producer can fill.
        external_widths: general_external_account_widths_v3(
            u32::try_from(product.basis.bytes.len()).expect("basis width"),
            u32::try_from(product.domain.bytes.len()).expect("domain width"),
        ),
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
            ..StateBumpsV1::UNRECORDED
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
    /// The Trading role's semantic release, a seed of this case's seal address.
    trading_semantic_release: [u8; 32],
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
    // THE ENTRY IS FOUNDED THE WAY A FOUNDING FOUNDS IT, not from the action
    // this harness happens to execute.
    //
    // This used to read `descriptor` -- the OpenBatch bundle's -- while
    // `tools/local-validator/bootstrap/successor/src/general_market.rs` read
    // `bundles.first()`, which is Consider. So the ladder picked the action and
    // derived the entry while the founding picked the entry and hoped, and until
    // they agreed a green run here said nothing about a founded market: devnet
    // found the wall first, `0x4015 DescriptorManifestEntry` after 128,724 CU.
    // Both now go through `general_selected_entry_descriptor_v1`, which refuses
    // a release whose fifteen descriptors disagree about what an entry holds.
    let entry_descriptor_bytes =
        general_selected_entry_descriptor_v1(&release).expect("family entry descriptor");
    let entry_descriptor =
        CapabilityProgramV4::decode(&entry_descriptor_bytes).expect("entry descriptor");
    // The two are different objects that agree on exactly the coordinates the
    // entry holds -- which is the property this harness is now exercising on the
    // founding's behalf, and is stated here rather than assumed by the run.
    assert_ne!(
        entry_descriptor_bytes, selected.descriptor,
        "the founding's descriptor and the executing action's must really differ"
    );
    assert_eq!(
        entry_descriptor.derivation_policy(),
        descriptor.derivation_policy(),
        "the entry the founding authors must bind the action this harness runs"
    );
    let clock_slot = substrate.bank_slot();
    let manifest = manifest_selection(&release, entry_descriptor, clock_slot);
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
    let trading_semantic_release = waist_facts.trading_semantic_release;
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
        // The System program itself, not an account owned by it. The commit
        // phase invokes System to allocate and assign the Batch state, and it
        // looks for the program among the profile-declared runtime accounts;
        // without this the route refuses `0x4005 Commit` at the first conjunct
        // of `apply_lifecycle_creates_v3`, which is what it did until
        // 2026-09-02.
        (
            usize::from(
                general_system_program_account_v3(Action::OpenBatch)
                    .expect("OpenBatch declares a System coordinate"),
            ),
            system_program_builtin(),
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
    // NO SPAN AND NO TRANSPORT SPAN. General's bank rides inline in the CPI
    // instruction data; the four input scratch pages it used to carry are gone
    // from the frame and there is no width for the builder to derive.
    assert!(built.bundle.span_counts.is_empty());
    assert_eq!(built.bundle.transport_span, None);
    HostCase {
        built,
        batch: request.batch,
        root: state.root.key,
        rent_credit: state.rent_credit.key,
        trading_semantic_release,
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

/// What one `OpenBatch` execution proved, for the callers that compare two.
struct OpenBatchRunV1 {
    invocations: u32,
    accounts: usize,
    compute_units: u64,
    /// The complete top-level account list, in order, exactly as signed.
    account_list: Vec<Pubkey>,
    /// The bank slot this execution actually ran at.
    executed_slot: u64,
}

async fn execute_open_batch(outcome_count: u32) -> (u32, usize, u64) {
    let run = execute_open_batch_at(outcome_count, None).await;
    (run.invocations, run.accounts, run.compute_units)
}

async fn execute_open_batch_at(outcome_count: u32, warp_to: Option<u64>) -> OpenBatchRunV1 {
    assert_eq!(DIRECT_HOT_HEAP_FRAME_BYTES_V1, 65_536);
    let substrate = waist::fixture_substrate();
    let elves = waist::elves();
    let accelerator_elf = load_accelerator_elf();
    let rent = Rent::default();
    // FIXED, NOT FRESH, and the two-slot proof is the reason. Both roles are
    // top-level account-list coordinates, so a `Keypair::new()` here makes
    // every pair of runs differ in the two entries the caller chose -- which is
    // not the property under test and would refute it whatever the seed did.
    // Each case runs in its own bank, so nothing collides.
    let payer = Keypair::new_from_array([0x11; 32]);
    let fee_payer = Keypair::new_from_array([0x12; 32]);
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
    // THE TWO COUNTS HAVE COME APART, which is the whole change. The
    // caller-authority span is still one account per accelerator invocation --
    // the output still chunks under `ChunkedBankV2` -- and the input page span
    // is empty. They were the same number, from the same return-data bound, and
    // reading one for the other is what made the input transport unbuildable.
    assert!(case.built.bundle.span_counts.is_empty());
    assert!(!case.built.admitted_authorities.entries.is_empty());
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
    eprintln!(
        "general-open-batch registers N={outcome_count} scalars={} identities={}",
        case.built.bundle.engine.input_scalars.len(),
        case.built.bundle.engine.input_identities.len(),
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
    // THE ONLY DIFFERENCE BETWEEN THE TWO RUNS THIS FUNCTION SERVES. Everything
    // above is byte-identical between them, including
    // `case.built.bundle.hot_instruction`, because the host derives the frame
    // from the founded Market and never from the clock.
    if let Some(slot) = warp_to {
        context.warp_to_slot(slot).expect("warp the bank");
    }
    let executed_slot = context
        .banks_client
        .get_sysvar::<solana_program::clock::Clock>()
        .await
        .expect("clock sysvar")
        .slot;
    // THE FRAME CONTROL, and the reason it is an assertion rather than a
    // diagnostic. `runtime_observations_digest` is a field of
    // `AdmittedInvocationContextV3`, and the host computes it over
    // `bundle.logical`'s modelled chain views while the chain computes it over
    // the accounts the bank actually holds. So every coordinate the host
    // mismodels is an invisible defect until the admitted route hashes the
    // frame, at which point it surfaces as `0x4018 AdmittedTransport` naming
    // no coordinate at all -- which is what General `OpenBatch` refused with
    // for the whole of 2026-09-02 because one binding claimed the System
    // program was a deployed upgradeable program.
    //
    // An account the transaction is about to CREATE is absent from the bank,
    // and the runtime presents an absent account to the program exactly as the
    // default System-owned empty account below, so that is not an exception to
    // the rule; it is the rule stated for a coordinate with no stored account.
    for coordinate in 0..case.built.bundle.logical.len() {
        let Some(built) = case.built.bundle.logical.get(coordinate) else {
            continue;
        };
        let view = built.chain_view();
        let observed = context
            .banks_client
            .get_account(built.key)
            .await
            .expect("bank query")
            .unwrap_or(Account {
                lamports: 0,
                data: Vec::new(),
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            });
        assert_eq!(
            (
                observed.owner,
                observed.lamports,
                observed.data.len(),
                observed.executable
            ),
            (view.owner, view.lamports, view.data.len(), view.executable),
            "logical coordinate {coordinate} ({}) is modelled by the host as something the bank does not hold",
            built.key,
        );
        assert_eq!(
            observed.data, view.data,
            "logical coordinate {coordinate} ({}) has the declared width and different bytes",
            built.key,
        );
    }
    assert_eq!(
        context
            .banks_client
            .get_account(system_program::ID)
            .await
            .expect("System program query")
            .expect("the bank holds the System program builtin")
            .data,
        SYSTEM_PROGRAM_BUILTIN_NAME_V1.as_bytes(),
        "the bank renamed its System builtin; `SYSTEM_PROGRAM_BUILTIN_NAME_V1` is the one author"
    );
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
    let submission = waist::submit_v0_observed(
        &mut context,
        &instructions,
        lookup_addresses,
        Some(&fee_payer),
        &[&payer],
    )
    .await;
    let execution = submission.expect("real Trading -> General accelerator OpenBatch");
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
    // THE HEADER'S BENEFICIARY IS THE CREDIT'S REFUND WALLET, NOT THE CREDIT
    // ACCOUNT. `apply_lifecycle_closes_v3` refuses unless
    // `authenticate_lifecycle_credit_v3(..).beneficiary` -- which is
    // `credit.refund_wallet()` -- equals the plan's beneficiary register, so
    // the wallet is what a close pays back and the wallet is what this header
    // has to carry. Read it off the credit account rather than restating it,
    // because a campaign that spells the beneficiary itself becomes a second
    // author for a fact the Rent record already owns.
    assert_eq!(
        local.header().beneficiary,
        LifecycleRentCreditV2::decode(&credit_after.data)
            .expect("RentCredit record")
            .refund_wallet()
            .to_bytes(),
    );
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
        "general-open-batch N={outcome_count} invocations={} accounts={} cu={} batch={} root_revision={}=>{}",
        case.built.admitted_authorities.entries.len(),
        case.built.bundle.hot_instruction.accounts.len(),
        execution.compute_units_consumed,
        case.batch,
        before.revision(),
        after.revision(),
    );
    OpenBatchRunV1 {
        invocations: u32::try_from(case.built.admitted_authorities.entries.len())
            .expect("invocation count"),
        accounts: case.built.bundle.hot_instruction.accounts.len(),
        compute_units: execution.compute_units_consumed,
        account_list: case
            .built
            .bundle
            .hot_instruction
            .accounts
            .iter()
            .map(|meta| meta.pubkey)
            .collect(),
        executed_slot,
    }
}

/// THE GENERAL MARKET'S CAPABILITY SEAL GETS A PRODUCER, AND IT IS NOT
/// DIRECT'S.
///
/// `devnet-general-session` reported cohort-14's seal at fixed coordinate 38 as
/// **producible and unproduced**: `process_capability_seal_v1` is
/// permissionless, anybody willing to pay the rent may call it, and the only
/// host builder for its request was
/// `direct_inline_route_v3::compile_direct_inline_capability_seal_plan_v3`,
/// which hard-codes `DirectExecutionActionV3::InlineOrdinary` and reads its
/// frame out of an authenticated Direct route. So the route had a reader, a
/// schema and a refusal, and only its failure path was ever exercised.
///
/// This drives `capability_seal_instruction_v1` -- the family-neutral producer
/// -- over a GENERAL descriptor, through the real Trading ELF, and requires the
/// seal to materialize at the address the builder derived. It shares nothing
/// with the admitted route but its fixed frame: the seal outer runs no hot
/// action, reads no register bank, and is deliberately checked here rather than
/// in a Direct suite, because "family-neutral" verified only against the one
/// family it came from is a claim rather than a measurement.
#[tokio::test]
async fn a_general_descriptor_seals_through_the_family_neutral_producer() {
    let substrate = waist::fixture_substrate();
    let elves = waist::elves();
    let accelerator_elf = load_accelerator_elf();
    let rent = Rent::default();
    let payer = Keypair::new();
    let fee_payer = Keypair::new();
    let seal_payer = Keypair::new();
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
        2,
        payer.pubkey(),
        &rent,
        substrate,
        &elves,
        releases,
        &accelerator_elf,
    );
    let fixed_frame = case
        .built
        .bundle
        .hot_instruction
        .accounts
        .iter()
        .take(HOT_FIXED_ACCOUNT_COUNT_V3)
        .map(|meta| meta.pubkey)
        .collect::<Vec<_>>();
    // THE FRAME'S COORDINATE 38 AND THE BUILDER'S DERIVATION ARE TWO
    // INDEPENDENT AUTHORS, and the builder refuses rather than trusting the
    // frame -- which is the whole reason it is a builder. The host derived this
    // frame from the founded Market; the builder derives the address from the
    // descriptor, the action, the Trading semantic release and the Registry.
    let composed = capability_seal_instruction_v1(CapabilitySealInstructionInputV1 {
        trading_program: waist::TRADING_PROGRAM_ID,
        registry_program: waist::REGISTRY_PROGRAM_ID,
        trading_semantic_release: case.trading_semantic_release,
        descriptor_digest: case.built.invocation_context.capability_program.to_bytes(),
        action: case.built.invocation_context.selected_action,
        fixed_frame: &fixed_frame,
        payer: seal_payer.pubkey(),
    })
    .expect("the General frame names the seal this builder derives");
    assert_eq!(
        composed.seal, fixed_frame[HOT_CAPABILITY_SEAL_ACCOUNT_V3],
        "the two authors must agree, and the builder must be the one that says so"
    );
    add_case_accounts(&mut test, &case, &payer, &fee_payer);
    test.add_account(
        seal_payer.pubkey(),
        Account {
            lamports: 10_000_000_000,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    // THE FIXTURE STAGES A MATERIALIZED SEAL, because the hot route reads one.
    // Overriding it back to vacant is what makes this a test of the PRODUCER
    // rather than of the fixture: a run that started from a sealed account would
    // pass whether or not the instruction did anything, which is the shape this
    // whole route was already in -- a reader, a schema and a refusal, with only
    // the failure path exercised.
    test.add_account(
        composed.seal,
        Account {
            lamports: 0,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    // THE HEAP FRAME IS PART OF THE PRODUCT, not a harness detail. The seal
    // outer declares the extended heap profile and `TradingSbfError::HeapFrame`
    // 0x4008 refuses a transaction that did not request it -- which is what this
    // test met on its first run, and the refusal names its own remedy, so it
    // cost one line instead of a bisect.
    let instructions = vec![
        ComputeBudgetInstruction::request_heap_frame(DIRECT_HOT_HEAP_FRAME_BYTES_V1),
        ComputeBudgetInstruction::set_compute_unit_limit(
            u32::try_from(waist::COMPUTE_LIMIT).expect("compute limit"),
        ),
        composed.instruction.clone(),
    ];
    let lookup_addresses = waist::canonical_lookup_addresses(&instructions, fee_payer.pubkey());
    waist::add_lookup_table(&mut test, &lookup_addresses);
    let mut context = waist::start_with_substrate(test, substrate).await;
    let before = context
        .banks_client
        .get_account(composed.seal)
        .await
        .expect("seal query");
    assert!(
        before
            .as_ref()
            .is_none_or(|account| account.owner == system_program::ID
                && account.data.is_empty()
                && account.lamports == 0),
        "the seal must be VACANT before this, or the run proves nothing"
    );
    let execution = waist::submit_v0_observed(
        &mut context,
        &instructions,
        lookup_addresses,
        Some(&fee_payer),
        &[&seal_payer],
    )
    .await
    .expect("the permissionless General seal");
    let sealed = context
        .banks_client
        .get_account(composed.seal)
        .await
        .expect("seal query")
        .expect("the seal materialized");
    assert_eq!(sealed.owner, waist::TRADING_PROGRAM_ID);
    assert_eq!(sealed.data.len(), CAPABILITY_SEAL_BYTES_V1);
    let closure = SealedDescriptorClosureV1::decode(&sealed.data).expect("sealed closure");
    // The verdict is the executing Program's, and the only thing checked here is
    // that it is filed under the coordinates this producer asked for: a host
    // that re-derived the closure body would be a second authority for a verdict
    // it did not compute.
    let key = closure.key().expect("sealed key");
    assert_eq!(key.action(), case.built.invocation_context.selected_action);
    assert_eq!(
        key.descriptor_digest(),
        case.built.invocation_context.capability_program.to_bytes()
    );
    assert_eq!(closure.bump().expect("sealed bump"), composed.bump);
    eprintln!(
        "general-capability-seal action={} seal={} bytes={} cu={}",
        case.built.invocation_context.selected_action,
        composed.seal,
        sealed.data.len(),
        execution.compute_units_consumed,
    );
}

/// ONE SIGNED ACCOUNT LIST, TWO EXECUTION SLOTS, AND IT COMMITS AT BOTH.
///
/// This is the property the caller-authority seed violated, and the reason
/// General could not be delivered on a real chain. Each of the four admitted
/// caller authorities was `find_program_address` over
/// `sha256(accelerator request header || inline register bank)`; `OpenBatch`'s
/// AccountProfile declares `TrustedEnvironmentV2::CurrentSlot`, so Trading
/// seeds `scalar::CURRENT_SLOT` from `Clock::get()` into that bank on every
/// execution; so each address was a function of the slot the transaction
/// executed in, while a signed transaction's account list is fixed when it is
/// signed. Trading refused `TradingSbfError::Release` 0x4001 in
/// `admitted_composition_v3.rs`, and there was no way to be right except to win
/// a slot lottery, once per action, for the whole lifecycle. See
/// `docs/design/GENERAL_CALLER_AUTHORITY_SLOT_BINDING_2026_09_03.md`.
///
/// The harness cannot sign a transaction at one slot and submit it at another
/// -- a blockhash expires -- so this asserts the thing that made that
/// impossible: the ACCOUNT LIST the host derives is byte-identical across two
/// executions at different slots, and both commit. Under the old seed the two
/// lists would differ in four of their fifty-five entries, and neither run
/// would have accepted the other's.
///
/// THE SLOTS DIFFERING IS THE POSITIVE CONTROL. `warp_to_slot` is asserted
/// through the executed `Clock`, not assumed: two runs that silently executed
/// at the same slot would pass this vacuously, and "nothing moved" and "my
/// instrument was disconnected" log identically.
#[tokio::test]
async fn one_signed_account_list_opens_the_same_batch_at_two_execution_slots() {
    let substrate = waist::fixture_substrate();
    // Far enough that the bank has demonstrably advanced, inside the manifest's
    // own activation deadline, which `manifest_selection` sets at
    // `clock_slot + 100`. A warp past it would refuse for activation reasons
    // and prove nothing about an address.
    let later = substrate.bank_slot() + 47;
    let first = execute_open_batch_at(2, None).await;
    let second = execute_open_batch_at(2, Some(later)).await;
    assert_eq!(first.executed_slot, substrate.bank_slot());
    assert_eq!(second.executed_slot, later);
    assert_ne!(
        first.executed_slot, second.executed_slot,
        "the two executions must be at different slots, or this proves nothing"
    );
    // NAMING THE COORDINATES, not only the fact. `assert_eq!` on two
    // fifty-five-entry vectors prints both in full and leaves the reader to
    // diff them, and this assertion's whole job is to say WHICH account is a
    // function of the slot -- the caller-authority span is four consecutive
    // entries and any other coordinate moving is a different defect entirely.
    let moved: Vec<(usize, Pubkey, Pubkey)> = first
        .account_list
        .iter()
        .zip(second.account_list.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(index, (a, b))| (index, *a, *b))
        .collect();
    assert_eq!(
        first.account_list.len(),
        second.account_list.len(),
        "the two executions did not present account lists of the same width"
    );
    assert!(
        moved.is_empty(),
        "the top-level account list moved with the executing slot; a signed \
         transaction cannot name it. Coordinates that moved: {moved:?}"
    );
    assert_eq!(
        (first.invocations, first.accounts),
        (second.invocations, second.accounts)
    );
    eprintln!(
        "general-open-batch two-slot slots={} and {} accounts={} identical=true cu={} and {}",
        first.executed_slot,
        second.executed_slot,
        first.accounts,
        first.compute_units,
        second.compute_units,
    );
}

#[tokio::test]
async fn real_elf_open_batch_commits_at_every_width_because_its_bank_does_not_grow() {
    // 258 IS REACHABLE NOW, AND FLATLY. `OpenBatch` declares a zero per-outcome
    // scalar stride -- Lean decides it in
    // `GeneralTransitionV3.actionItemScalarStride`, and the AccountProfile,
    // RequestProfile, transition and effect all carry the same zero -- so the
    // register bank is 151 scalars at every Product width. The scratch-page
    // count is derived from the bank width, so the page span does not move
    // either, and neither does the account frame.
    //
    // Before this, the Trading heap peaked at `59,376 + 528*(N - 2)` of 65,536:
    // N = 13 committed at 65,184 and N = 14 aborted needing 65,712. N = 258
    // refused earlier still, `0x4000 UnsupportedContent` at `hot_v3.rs:3986`,
    // because 151 + 6*258 = 1,699 scalars exceeded `MAX_HOT_SCALARS_V3` = 512.
    // That constant needs no lift: the count is 151 at every width now.
    let narrow = execute_open_batch(2).await;
    let middle = execute_open_batch(13).await;
    let widest = execute_open_batch(258).await;
    // THE FLATNESS IS THE ASSERTION, not a remark. Equal page spans and equal
    // account frames across a 129-fold width change is the observable form of
    // "the bank does not grow", and it is what would go red if any one of the
    // four artifacts started declaring a tail again.
    assert_eq!(
        (narrow.0, narrow.1),
        (widest.0, widest.1),
        "N=2 and N=258 must present the same invocation span and account frame"
    );
    assert_eq!((middle.0, middle.1), (widest.0, widest.1));
}
