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
    BuilderError, WaistFactsV1,
    admitted::AdmittedAotInputV1,
    artifacts::{ArtifactSetV1, DerivedRecordV1, derive_record, digest},
    bundle::{BundleInputV1, FixedCorpusV1, ScenarioV1},
    frame::{
        BuiltAccountV1, SYSTEM_PROGRAM_BUILTIN_NAME_V1, data_account, external_with_view,
        program_with_view, system_program_builtin, vacant,
    },
    general::{
        GeneralActionPrestateV1, GeneralRequestEvidenceV1, GeneralRequestInputV1,
        build_general_action_bundle_v1, derive_general_request_v1,
        general_action_prestate_shape_v1,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_direct_hot_program_test_support::waist;
use dclutch_general_adapter_contract::{
    candidate_v1::{
        GeneralCandidateOpeningV1, GeneralCandidateStatusV1, GeneralCandidateV1,
        authenticate_candidate_identity_v1, general_candidate_identity_v1,
    },
    collection_v1::{
        BatchStatusV1, GeneralBatchOccurrenceTermsV1, GeneralBatchV1,
        authenticate_batch_candidate_v1,
    },
    local_state_v3::{GeneralLocalStateKindV3, GeneralLocalStateV3},
    runtime_width::{CandidateHeaderV2, CandidateV2, candidate_len},
    state_artifacts_v3::{
        GENERAL_PRIMARY_PAYER_ACCOUNT_V3, GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3,
        GENERAL_PRIMARY_STATE_ACCOUNT_V3, GeneralReadonlyEvidenceKindV3,
        general_readonly_evidence_v3, general_system_program_account_v3,
    },
};
use dclutch_general_codec::Action;
use dclutch_general_config_contract::v3::GeneralConfigV3;
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
use solana_program::{hash::hash, instruction::Instruction, pubkey::Pubkey, rent::Rent};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program};

const ACCELERATOR_PROGRAM: Pubkey = Pubkey::new_from_array([0xa1; 32]);
/// `TradingSbfError::Root`, derived from its REGISTERED BAND.
///
/// The enum itself is the better author and it is not reachable from this
/// workspace; see the `dclutch-refusal-registry` note in `Cargo.toml`. What
/// decision 0007 forbids either way is the third option, a bare `16386` that
/// keeps asserting an old number at a route that no longer raises it.
const TRADING_ROOT: u32 = dclutch_refusal_registry::TRADING_REFUSAL_BASE + 0x002;
const GENERATION: u64 = 9;
/// What genesis funds every wallet this campaign installs.
///
/// One author: `add_case_accounts` writes it and `genesis_prestate` models it,
/// and the frame control compares the two against the live bank.
const GENESIS_PAYER_LAMPORTS: u64 = 10_000_000_000;
const PRICE_SCALE: u64 = 1_000_000;
/// The solver that funds and endorses the campaign's one candidate.
const SOLVER: [u8; 32] = [0xc3; 32];
/// Revision the candidate's pages are pinned at.
const CANDIDATE_PAGE_REVISION: u64 = 11;
/// Lamports one verification crank pays out of the candidate's work escrow.
const CRANK_REWARD_LAMPORTS: u64 = 5_000;
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

/// Everything one founded General market fixes before any action executes.
///
/// A market is founded ONCE and the family's fifteen actions run against that
/// one founding: the same Product, the same selected release, the same manifest
/// entry -- which since `ae026955d` binds one FAMILY lifecycle policy rather
/// than one action's -- and the same Core Market, root and RentCredit.
///
/// Splitting this out of the per-action case is what makes more than one
/// General action on one market expressible at all. Until 2026-09-04 the
/// harness built the founding and the action together, so every run was a fresh
/// market and "the second action" had no meaning to express.
struct CampaignV1 {
    outcome_count: u32,
    payer: Pubkey,
    rent: Rent,
    substrate: waist::FixtureSubstrateV1,
    releases: waist::Releases,
    product: ProductRecords,
    release: GeneralSelectedReleaseV1,
    manifest: ManifestSelection,
    state: StateCorpus,
    waist_facts: WaistFactsV1,
    accelerator_artifact: Vec<u8>,
    accelerator_program: BuiltAccountV1,
    accelerator_programdata_account: BuiltAccountV1,
    externally_installed: [Pubkey; 2],
}

/// The bank state one action reads.
///
/// Genesis for the first action; READ BACK OUT OF THE BANK for every action
/// after it. A campaign that carried its own prediction forward instead would
/// be asserting against itself: the whole point of a second action is that the
/// first one's poststate is the second one's authority.
struct ChainPrestateV1 {
    market: BuiltAccountV1,
    root: BuiltAccountV1,
    rent_credit: BuiltAccountV1,
    /// The protocol payer, WITH ITS CURRENT BALANCE.
    ///
    /// It was a literal until 2026-09-04, and a literal was survivable only
    /// because no market ever ran a second action: `OpenBatch` moves one exact
    /// Batch principal out of this wallet, so from the second action onward the
    /// modelled balance is wrong by that amount and the admitted route refuses
    /// `0x4018 AdmittedTransport` naming no coordinate. The frame control found
    /// it by name on the first two-action run.
    payer: BuiltAccountV1,
    /// The live primary state this action operates on, or `None` where this
    /// execution is the one that creates it.
    primary_state: Option<BuiltAccountV1>,
}

struct HostCase {
    /// The action this case executes.
    action: Action,
    built: dclutch_chain_bundle_builder::bundle::BuiltAdmittedBundleV1,
    /// The primary state this action names: created by `OpenBatch`, read by
    /// every action after it.
    primary_state: Pubkey,
    root: Pubkey,
    rent_credit: Pubkey,
    /// The Trading role's semantic release, a seed of this case's seal address.
    trading_semantic_release: [u8; 32],
    /// The complete top-level instruction list, exactly as it will be signed.
    instructions: Vec<Instruction>,
    /// The canonical lookup addresses that instruction list resolves through.
    lookup_addresses: Vec<Pubkey>,
}

#[allow(clippy::too_many_arguments)]
fn build_campaign(
    outcome_count: u32,
    payer: Pubkey,
    rent: Rent,
    substrate: waist::FixtureSubstrateV1,
    elves: &waist::Elves,
    releases: waist::Releases,
    accelerator_elf: &[u8],
) -> CampaignV1 {
    build_campaign_with_entry(
        outcome_count,
        payer,
        rent,
        substrate,
        elves,
        releases,
        accelerator_elf,
        None,
    )
}

/// The same founding with an explicitly supplied manifest entry descriptor.
///
/// `None` is the founding a founding performs. `Some` is how a hostile founds a
/// market on an entry that does NOT bind this release's actions -- the cohort-15
/// shape -- and it is a parameter rather than a mutation because the entry is a
/// choice the founding makes, once, before any action exists.
#[allow(clippy::too_many_arguments)]
fn build_campaign_with_entry(
    outcome_count: u32,
    payer: Pubkey,
    rent: Rent,
    substrate: waist::FixtureSubstrateV1,
    elves: &waist::Elves,
    releases: waist::Releases,
    accelerator_elf: &[u8],
    foreign_entry: Option<Vec<u8>>,
) -> CampaignV1 {
    let product = build_product(outcome_count);
    let accelerator_artifact =
        waist::release_v2(ACCELERATOR_PROGRAM, 0x71, accelerator_elf, substrate);
    let release = selected_release(
        outcome_count,
        &product,
        waist::artifact_id(accelerator_artifact),
    );
    // THE ENTRY IS FOUNDED THE WAY A FOUNDING FOUNDS IT, not from whichever
    // action this harness happens to execute first.
    //
    // This used to read the OpenBatch bundle's descriptor while
    // `tools/local-validator/bootstrap/successor/src/general_market.rs` read
    // `bundles.first()`, which is Consider. So the ladder picked the action and
    // derived the entry while the founding picked the entry and hoped, and until
    // they agreed a green run here said nothing about a founded market: devnet
    // found the wall first, `0x4015 DescriptorManifestEntry` after 128,724 CU.
    // Both now go through `general_selected_entry_descriptor_v1`, which refuses
    // a release whose fifteen descriptors disagree about what an entry holds.
    //
    // THAT IT IS ACTION-FREE IS WHAT THIS CAMPAIGN RESTS ON. One entry binding
    // one family policy is exactly the property a multi-action run needs, and
    // it is stated here, at the founding, rather than per action.
    let entry_descriptor_bytes = foreign_entry.unwrap_or_else(|| {
        general_selected_entry_descriptor_v1(&release).expect("family entry descriptor")
    });
    let entry_descriptor =
        CapabilityProgramV4::decode(&entry_descriptor_bytes).expect("entry descriptor");
    let founds_this_release = release.bundles.iter().all(|bundle| {
        CapabilityProgramV4::decode(&bundle.descriptor)
            .expect("descriptor")
            .derivation_policy()
            == entry_descriptor.derivation_policy()
    });
    let mut distinct = 0_usize;
    for bundle in &release.bundles {
        if !founds_this_release {
            continue;
        }
        let descriptor = CapabilityProgramV4::decode(&bundle.descriptor).expect("descriptor");
        // Every one of the fifteen must be BOUND by the entry the founding
        // authors -- that is the property a multi-action campaign rests on, and
        // it is the property cohort-15 was founded without.
        assert_eq!(
            entry_descriptor.derivation_policy(),
            descriptor.derivation_policy(),
            "the entry the founding authors must bind {:?}",
            bundle.action
        );
        if entry_descriptor_bytes != bundle.descriptor {
            distinct += 1;
        }
    }
    // The entry descriptor IS one of the fifteen -- `bundles.first()`, which is
    // Consider -- so exactly fourteen of them are different objects that agree
    // on the coordinates an entry holds. Counting them is what makes the
    // agreement above a measurement rather than fifteen tautologies: if the
    // fifteen descriptors collapsed to one object the count would read zero and
    // this campaign would prove nothing about a founding.
    if founds_this_release {
        assert_eq!(
            distinct,
            release.bundles.len() - 1,
            "the founding's descriptor must really differ from every action's but its own"
        );
    }
    let clock_slot = substrate.bank_slot();
    let manifest = manifest_selection(&release, entry_descriptor, clock_slot);
    let state = state_corpus(
        &rent,
        payer,
        releases.release_set,
        &product,
        &manifest,
        &release,
    );
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
    CampaignV1 {
        outcome_count,
        payer,
        rent,
        substrate,
        releases,
        product,
        release,
        manifest,
        state,
        waist_facts,
        accelerator_artifact: accelerator_artifact.to_bytes().to_vec(),
        accelerator_program: program_with_view(ACCELERATOR_PROGRAM, accelerator_programdata),
        accelerator_programdata_account: external_with_view(
            accelerator_programdata,
            bpf_loader_upgradeable::ID,
            waist::programdata_v2(substrate, accelerator_elf),
        ),
        externally_installed: [ACCELERATOR_PROGRAM, accelerator_programdata],
    }
}

/// The founding's own prestate: what the bank holds before any action runs.
fn genesis_prestate(campaign: &CampaignV1) -> ChainPrestateV1 {
    ChainPrestateV1 {
        market: campaign.state.market.clone(),
        root: campaign.state.root.clone(),
        rent_credit: campaign.state.rent_credit.clone(),
        payer: vacant(campaign.payer).with_observed(Account {
            lamports: GENESIS_PAYER_LAMPORTS,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        }),
        primary_state: None,
    }
}

/// The coordinate this action's own profile declares for one evidence kind.
fn evidence_coordinate(action: Action, kind: GeneralReadonlyEvidenceKindV3) -> u16 {
    let mut index = 0_u16;
    loop {
        let evidence = general_readonly_evidence_v3(action, index)
            .expect("this action declares evidence of that kind");
        if evidence.kind == kind {
            return evidence.coordinate;
        }
        index = index.checked_add(1).expect("bounded evidence table");
    }
}

/// The authenticated records one action reads that are not its primary state.
///
/// Every entry is a record the bank really holds: one of them is read back out
/// of an earlier action's poststate and the rest are installed at genesis --
/// which is a debt with a name, not a shortcut. The two batch actions read
/// nothing but their primary state, so `OpenBatch` and `CloseBatch` pass the
/// default and the campaign below is unchanged for them.
///
/// It is ONE value for both ends of the execution: the request derivation and
/// the candidate projector are the same question -- which records does this
/// action read -- asked at the two ends, and a campaign that built two lists
/// could answer it differently in each.
#[derive(Clone, Default)]
struct EvidenceCorpusV1 {
    /// The closed Batch a candidate action names, exactly as the bank holds it.
    closed_batch: Option<BuiltAccountV1>,
    /// The immutable runtime-width candidate image, carrying its own digest.
    candidate_image: Option<BuiltAccountV1>,
    /// The exact submission record this execution writes.
    submitted_candidate: Option<BuiltAccountV1>,
}

impl EvidenceCorpusV1 {
    /// The records the REQUEST derivation reads, per action.
    fn request(&self) -> GeneralRequestEvidenceV1<'_> {
        GeneralRequestEvidenceV1 {
            candidate_image: self.candidate_image.as_ref().map(built_bytes),
            ..GeneralRequestEvidenceV1::default()
        }
    }

    /// The records the PROJECTOR reads, which is a strict superset.
    fn projector(&self) -> GeneralRequestEvidenceV1<'_> {
        GeneralRequestEvidenceV1 {
            batch_account: self.closed_batch.as_ref().map(built_bytes),
            submitted_candidate: self.submitted_candidate.as_ref().map(built_bytes),
            ..self.request()
        }
    }

    /// Bind each present record at the coordinate its action's profile declares.
    fn bindings(&self, action: Action) -> Vec<(usize, BuiltAccountV1)> {
        [
            (
                GeneralReadonlyEvidenceKindV3::ClosedBatch,
                &self.closed_batch,
            ),
            (
                GeneralReadonlyEvidenceKindV3::CandidateImage,
                &self.candidate_image,
            ),
            (
                GeneralReadonlyEvidenceKindV3::SubmittedCandidate,
                &self.submitted_candidate,
            ),
        ]
        .into_iter()
        .filter_map(|(kind, value)| {
            let account = value.as_ref()?;
            Some((
                usize::from(evidence_coordinate(action, kind)),
                account.clone(),
            ))
        })
        .collect()
    }
}

/// One immutable evidence record, installed at the address of its own digest.
///
/// CONTENT-ADDRESSED ON PURPOSE. The AccountProfile authenticates an evidence
/// coordinate by privileges, width and prestate and says nothing about its
/// owner or address -- the digest joins are inside the projector -- so the
/// campaign is free to choose, and the digest is the choice that cannot
/// silently drift: a substituted record is a DIFFERENT account rather than the
/// same account holding other bytes.
///
/// The owner is the Registry because these are content records with no other
/// author yet. That is a stand-in and it is the honest one: the two records
/// staged this way have real producers -- a solver publishes the candidate
/// image, the Effect writes the submission -- and neither producer has an
/// executable route in this campaign.
fn staged_record(rent: &Rent, bytes: Vec<u8>) -> BuiltAccountV1 {
    let key = Pubkey::new_from_array(digest(&bytes));
    data_account(rent, key, waist::REGISTRY_PROGRAM_ID, bytes)
}

/// One identity, short enough to read in a campaign row.
fn hex32(value: [u8; 32]) -> String {
    value
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Borrow one bound account's exact bytes, which is what both ends read.
fn built_bytes(account: &BuiltAccountV1) -> &[u8] {
    account.account.data.as_slice()
}

/// Build one action's complete admitted bundle against one chain prestate.
///
/// `clock_slot` is the slot this transaction will EXECUTE at, and it is a
/// parameter rather than a campaign constant because a same-bank campaign warps
/// between actions -- `close_is_permissionless` requires the collection window
/// to have elapsed. The host seeds `scalar::CURRENT_SLOT` from this and the
/// chain seeds it from `Clock::get()`; the campaign asserts the executed Clock
/// against it rather than assuming the warp landed.
fn build_action_case(
    campaign: &CampaignV1,
    action: Action,
    chain: &ChainPrestateV1,
    clock_slot: u64,
    fee_payer: Pubkey,
) -> HostCase {
    build_action_case_with_evidence(
        campaign,
        action,
        chain,
        clock_slot,
        fee_payer,
        &EvidenceCorpusV1::default(),
    )
    .expect("complete admitted General bundle")
}

#[allow(clippy::too_many_arguments)]
fn build_action_case_with_evidence(
    campaign: &CampaignV1,
    action: Action,
    chain: &ChainPrestateV1,
    clock_slot: u64,
    fee_payer: Pubkey,
    evidence: &EvidenceCorpusV1,
) -> Result<HostCase, dclutch_chain_bundle_builder::BuilderError> {
    let selected = campaign
        .release
        .bundles
        .iter()
        .find(|bundle| bundle.action == action)
        .expect("selected action bundle");
    let root_tail = GeneralRootV2::decode(
        chain
            .root
            .account
            .data
            .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .expect("root tail"),
    )
    .expect("General root");
    let request = derive_general_request_v1(GeneralRequestInputV1 {
        action,
        root: root_tail,
        root_address: chain.root.key,
        config: &campaign.release.config,
        outcome_count: campaign.outcome_count,
        product_id: campaign.product.product_id,
        trading_program: waist::TRADING_PROGRAM_ID,
        primary_state_account: chain
            .primary_state
            .as_ref()
            .map(|state| state.account.data.as_slice()),
        evidence: evidence.request(),
    })
    .expect("chain-derived General request");
    let mut bindings = vec![
        (
            usize::from(GENERAL_PRIMARY_PAYER_ACCOUNT_V3),
            chain.payer.clone(),
        ),
        (
            usize::from(GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3),
            chain.rent_credit.clone(),
        ),
        // The System program itself, not an account owned by it. The commit
        // phase invokes System to allocate and assign the state a lifecycle plan
        // creates, and it looks for the program among the profile-declared
        // runtime accounts; without this the route refuses `0x4005 Commit` at
        // the first conjunct of `apply_lifecycle_creates_v3`, which is what it
        // did until 2026-09-02.
        (
            usize::from(
                general_system_program_account_v3(action)
                    .expect("this action declares a System coordinate"),
            ),
            system_program_builtin(),
        ),
    ];
    bindings.extend(evidence.bindings(action));
    if let Some(primary) = chain.primary_state.as_ref() {
        // THE LIVE STATE IS A CORPUS BINDING, and the lifecycle preplan is an
        // independent author for its ADDRESS. `build_bundle` refuses when a
        // campaign-bound coordinate and the policy's derivation disagree, so
        // binding it here is a join rather than an assertion the harness makes
        // about itself.
        bindings.push((
            usize::from(GENERAL_PRIMARY_STATE_ACCOUNT_V3),
            primary.clone(),
        ));
    }
    let set = ArtifactSetV1 {
        descriptor: &selected.descriptor,
        account_profile: &selected.account_profile,
        request_profile: &selected.request_profile,
        transition: &selected.transition,
        effect: &selected.effect,
        lifecycle: &selected.lifecycle_policy,
        strategy: &selected.strategy,
        program_set: &campaign.release.program_set,
        manifest: &campaign.manifest.bytes,
        config: &campaign.release.config,
    };
    let input = BundleInputV1 {
        set,
        waist: campaign.waist_facts,
        scenario: ScenarioV1 {
            family_request: &request.request,
            tail_count: campaign.outcome_count,
            clock_slot,
            generation: GENERATION,
            ed25519_evidence: None,
            native_message_instruction_index: 2,
            externally_installed_extra: &campaign.externally_installed,
            payer: campaign.payer,
        },
        fixed: FixedCorpusV1 {
            market: chain.market.clone(),
            root: chain.root.clone(),
            product: campaign.product.product.clone(),
            result_domain: campaign.product.domain.clone(),
            portfolio: campaign.product.portfolio.clone(),
            linked_basis: campaign.product.basis.clone(),
            core_programdata: campaign.releases.core_programdata,
            trading_programdata: campaign.releases.trading_programdata,
        },
        bindings: &bindings,
        rent: &campaign.rent,
    };
    let built = build_general_action_bundle_v1(
        &input,
        AdmittedAotInputV1 {
            certificate: Some(&selected.certificate),
            admission: Some(&selected.admission),
            artifact_release: Some(&campaign.accelerator_artifact),
            accelerator_program: Some(&campaign.accelerator_program),
            accelerator_programdata: Some(&campaign.accelerator_programdata_account),
        },
        GeneralActionPrestateV1 {
            primary_state_account: chain
                .primary_state
                .as_ref()
                .map(|state| state.account.data.as_slice()),
            evidence: evidence.projector(),
        },
    )?;
    // NO SPAN AND NO TRANSPORT SPAN. General's bank rides inline in the CPI
    // instruction data; the four input scratch pages it used to carry are gone
    // from the frame and there is no width for the builder to derive.
    assert!(built.bundle.span_counts.is_empty());
    assert_eq!(built.bundle.transport_span, None);
    // The accelerator's admission joins the request's witnessed `STATE_BUMP` to
    // the lifecycle's `PRIMARY_CANONICAL_BUMP` and nothing else, so two
    // derivations that name DIFFERENT accounts still satisfy it whenever their
    // canonical bump bytes happen to agree. Join the addresses here, where both
    // are in hand, or a bump collision silently hides a lifecycle recipe reading
    // a register no artifact ever writes.
    assert_eq!(
        built
            .bundle
            .logical
            .get(usize::from(GENERAL_PRIMARY_STATE_ACCOUNT_V3))
            .expect("primary state coordinate")
            .key,
        request.primary_state,
        "the lifecycle-derived primary state is not the PDA the {action:?} request names"
    );
    let instructions = vec![
        ComputeBudgetInstruction::request_heap_frame(DIRECT_HOT_HEAP_FRAME_BYTES_V1),
        ComputeBudgetInstruction::set_compute_unit_limit(
            u32::try_from(waist::COMPUTE_LIMIT).expect("compute limit"),
        ),
        built.bundle.hot_instruction.clone(),
    ];
    let lookup_addresses = waist::canonical_lookup_addresses(&instructions, fee_payer);
    Ok(HostCase {
        action,
        built,
        primary_state: request.primary_state,
        root: chain.root.key,
        rent_credit: chain.rent_credit.key,
        trading_semantic_release: campaign.waist_facts.trading_semantic_release,
        instructions,
        lookup_addresses,
    })
}

/// The account the bank presents for a coordinate it holds nothing at.
///
/// The runtime presents an absent account to a program exactly as this, so a
/// coordinate the transaction is about to CREATE is not an exception to the
/// frame control; it is the rule stated for a coordinate with no stored
/// account.
fn absent_account() -> Account {
    Account {
        lamports: 0,
        data: Vec::new(),
        owner: system_program::ID,
        executable: false,
        rent_epoch: 0,
    }
}

/// Read one account exactly as the bank holds it, absent included.
async fn chain_account(
    context: &mut solana_program_test::ProgramTestContext,
    key: Pubkey,
) -> Account {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("bank query")
        .unwrap_or_else(absent_account)
}

/// One binding whose model IS what the bank holds.
async fn observed_binding(
    context: &mut solana_program_test::ProgramTestContext,
    key: Pubkey,
) -> BuiltAccountV1 {
    BuiltAccountV1 {
        key,
        account: chain_account(context, key).await,
        observed: None,
    }
}

/// THE FRAME CONTROL, and the reason it is an assertion rather than a
/// diagnostic.
///
/// `runtime_observations_digest` is a field of `AdmittedInvocationContextV3`,
/// and the host computes it over `bundle.logical`'s modelled chain views while
/// the chain computes it over the accounts the bank actually holds. So every
/// coordinate the host mismodels is an invisible defect until the admitted
/// route hashes the frame, at which point it surfaces as `0x4018
/// AdmittedTransport` naming no coordinate at all -- which is what General
/// `OpenBatch` refused with for the whole of 2026-09-02 because one binding
/// claimed the System program was a deployed upgradeable program.
///
/// IT RUNS BEFORE EVERY ACTION, not only the first. In a same-bank campaign the
/// second action's model is built from accounts read back out of the bank, and
/// this is what says so: a coordinate the campaign forgot to re-read, or one an
/// install silently clobbered back to its prestate, is caught here by name
/// instead of at the far end of a hash.
async fn assert_frame_control(
    context: &mut solana_program_test::ProgramTestContext,
    case: &HostCase,
) {
    for coordinate in 0..case.built.bundle.logical.len() {
        let Some(built) = case.built.bundle.logical.get(coordinate) else {
            continue;
        };
        let view = built.chain_view();
        let observed = chain_account(context, built.key).await;
        assert_eq!(
            (
                observed.owner,
                observed.lamports,
                observed.data.len(),
                observed.executable
            ),
            (view.owner, view.lamports, view.data.len(), view.executable),
            "{:?}: logical coordinate {coordinate} ({}) is modelled by the host as something the bank does not hold",
            case.action,
            built.key,
        );
        assert_eq!(
            observed.data, view.data,
            "{:?}: logical coordinate {coordinate} ({}) has the declared width and different bytes",
            case.action, built.key,
        );
    }
    assert_eq!(
        chain_account(context, system_program::ID).await.data,
        SYSTEM_PROGRAM_BUILTIN_NAME_V1.as_bytes(),
        "the bank renamed its System builtin; `SYSTEM_PROGRAM_BUILTIN_NAME_V1` is the one author"
    );
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
                lamports: GENESIS_PAYER_LAMPORTS,
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
    let campaign = build_campaign(
        outcome_count,
        payer.pubkey(),
        rent,
        substrate,
        &elves,
        releases,
        &accelerator_elf,
    );
    let case = build_action_case(
        &campaign,
        Action::OpenBatch,
        &genesis_prestate(&campaign),
        substrate.bank_slot(),
        fee_payer.pubkey(),
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
    add_case_accounts(&mut test, &case, &payer, &fee_payer);
    let instructions = case.instructions.clone();
    let lookup_addresses = case.lookup_addresses.clone();
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
    assert_frame_control(&mut context, &case).await;
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
            .get_account(case.primary_state)
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
        .get_account(case.primary_state)
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
        case.primary_state,
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
    let campaign = build_campaign(
        2,
        payer.pubkey(),
        rent,
        substrate,
        &elves,
        releases,
        &accelerator_elf,
    );
    let case = build_action_case(
        &campaign,
        Action::OpenBatch,
        &genesis_prestate(&campaign),
        substrate.bank_slot(),
        fee_payer.pubkey(),
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
            lamports: GENESIS_PAYER_LAMPORTS,
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

/// The refusal code one execution published, derived from the executing
/// Program's own enum by every caller of this.
fn refusal_code(error: &solana_program_test::BanksClientError) -> Option<u32> {
    let transaction = match error {
        solana_program_test::BanksClientError::TransactionError(value) => value,
        solana_program_test::BanksClientError::SimulationError { err, .. } => err,
        _ => return None,
    };
    match transaction {
        solana_sdk::transaction::TransactionError::InstructionError(
            _,
            solana_program::instruction::InstructionError::Custom(code),
        ) => Some(*code),
        _ => None,
    }
}

/// Install this action's accounts that the bank does not already hold.
///
/// THE BANK IS THE AUTHORITY FOR EVERYTHING IT HOLDS. A same-bank campaign's
/// second action derives a complete install list, and most of that list is the
/// founding's own state -- which the first action has since MUTATED. Writing
/// the model back over it would silently restore the prestate and make the
/// second action a second first action, which is the exact failure a green run
/// could not distinguish from a real sequence.
///
/// So this installs only coordinates the bank holds nothing at: this action's
/// own artifact records, which are inert Registry-owned content no execution
/// ever writes. Everything else is left alone and then CHECKED, coordinate by
/// coordinate, by `assert_frame_control`.
async fn install_absent(
    context: &mut solana_program_test::ProgramTestContext,
    case: &HostCase,
    skip: &[Pubkey],
) -> usize {
    let mut installed = 0;
    for install in &case.built.bundle.accounts {
        if case
            .built
            .bundle
            .externally_installed_keys
            .contains(&install.key)
            || skip.contains(&install.key)
        {
            continue;
        }
        // An install that is ITSELF the absent account writes nothing: the
        // runtime presents a stored empty System account and an absent one
        // identically, and every derived coordinate a bundle names -- the
        // caller authorities, the state it is about to create -- is one of
        // these. Counting them would make "records installed" a number about
        // the frame's width rather than about content.
        if install.account == absent_account() {
            continue;
        }
        let observed = chain_account(context, install.key).await;
        if observed != absent_account() {
            continue;
        }
        context.set_account(
            &install.key,
            &solana_account::AccountSharedData::from(install.account.clone()),
        );
        installed += 1;
    }
    installed
}

/// Produce one action's capability seal through the family-neutral producer.
///
/// PER ACTION, BY CONSTRUCTION: `CapabilitySealKeyV1`'s third seed is the action
/// selector, so a market that runs two actions needs two seals and the second
/// one has no producer until somebody calls this. In the single-action harness
/// the builder STAGED the seal into genesis, which exercises the reader and
/// never the writer -- the shape `devnet-general-session` reported as
/// "producible and unproduced". Here the seal starts vacant and the real
/// Trading ELF writes it.
async fn produce_seal(
    context: &mut solana_program_test::ProgramTestContext,
    case: &HostCase,
    seal_payer: &Keypair,
    fee_payer: &Keypair,
) -> (Pubkey, u64) {
    let fixed_frame = case
        .built
        .bundle
        .hot_instruction
        .accounts
        .iter()
        .take(HOT_FIXED_ACCOUNT_COUNT_V3)
        .map(|meta| meta.pubkey)
        .collect::<Vec<_>>();
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
    assert_eq!(
        composed.seal, case.built.bundle.artifacts.seal,
        "the seal the bundle stages and the seal the producer derives are one address"
    );
    let before = chain_account(context, composed.seal).await;
    assert_eq!(
        before,
        absent_account(),
        "the seal must be VACANT before this, or the run proves nothing"
    );
    let instructions = vec![
        ComputeBudgetInstruction::request_heap_frame(DIRECT_HOT_HEAP_FRAME_BYTES_V1),
        ComputeBudgetInstruction::set_compute_unit_limit(
            u32::try_from(waist::COMPUTE_LIMIT).expect("compute limit"),
        ),
        composed.instruction.clone(),
    ];
    let lookup_addresses = waist::canonical_lookup_addresses(&instructions, fee_payer.pubkey());
    waist::set_lookup_table(context, &lookup_addresses);
    let execution = waist::submit_v0_observed(
        context,
        &instructions,
        lookup_addresses,
        Some(fee_payer),
        &[seal_payer],
    )
    .await
    .expect("the permissionless General seal");
    let sealed = chain_account(context, composed.seal).await;
    assert_eq!(sealed.owner, waist::TRADING_PROGRAM_ID);
    assert_eq!(sealed.data.len(), CAPABILITY_SEAL_BYTES_V1);
    // TWO AUTHORS FOR ONE BODY. The host derived `seal_bytes` from the artifact
    // set alone; the Program wrote what it computed from the frame it was
    // handed. Comparing them is what makes the staged seal in every other run
    // of this harness a REPRODUCTION of the Program's verdict rather than a
    // fixture nobody ever checked.
    assert_eq!(
        sealed.data, case.built.bundle.artifacts.seal_bytes,
        "the Program's seal body and the builder's differ"
    );
    let closure = SealedDescriptorClosureV1::decode(&sealed.data).expect("sealed closure");
    let key = closure.key().expect("sealed key");
    assert_eq!(key.action(), case.built.invocation_context.selected_action);
    assert_eq!(
        key.descriptor_digest(),
        case.built.invocation_context.capability_program.to_bytes()
    );
    assert_eq!(closure.bump().expect("sealed bump"), composed.bump);
    (composed.seal, execution.compute_units_consumed)
}

/// Decode the live Batch envelope exactly as the bank holds it.
fn decode_batch(account: &Account) -> (GeneralLocalStateV3<'_>, GeneralBatchV1) {
    let envelope = GeneralLocalStateV3::decode(&account.data).expect("local Batch envelope");
    let batch = GeneralBatchV1::decode(envelope.body()).expect("Batch");
    (envelope, batch)
}

/// The General root tail one account carries.
fn root_tail_of(account: &Account) -> GeneralRootV2 {
    GeneralRootV2::decode(
        account
            .data
            .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .expect("root tail"),
    )
    .expect("General root")
}

/// TWO GENERAL ACTIONS, ONE FOUNDED MARKET, ONE BANK -- THE FIRST TIME IN ANY
/// HARNESS.
///
/// Every General run before this founded a market and executed exactly one
/// action against it, because the founding and the action were built together.
/// Cohort-15 measured why that mattered on devnet: a manifest entry holds ONE
/// `child_derivation_id`, so a market founded on a per-action lifecycle policy
/// could execute exactly one action and refused every other with `0x4015
/// DescriptorManifestEntry`. `ae026955d` made the fifteen actions share one
/// FAMILY policy; this is the first run that spends that -- one entry, two
/// actions, and the second one reading the first one's poststate.
///
/// THE SEQUENCE IS REAL AND ITS ORDER IS FORCED. `OpenBatch` creates the Batch
/// and consumes root revision 1; `CloseBatch` names the Batch by the identity
/// the CHAIN holds, consumes revision 2, and is admitted only once the
/// config-derived collection window has elapsed
/// (`GeneralBatchV1::close_is_permissionless`). Neither can be run first and
/// neither can be run twice, and both facts are executed below rather than
/// asserted in prose.
///
/// WHAT IS NOT HERE. The thirteen other actions need semantic corpus this
/// campaign does not build -- a signed order and its Claims/Custody escrow for
/// `PlaceOrder`, a submitted candidate for `SubmitCandidate`, a selection cursor
/// and a verified candidate for `Consider`, a settlement cursor for the five
/// settlement actions. `derive_general_request_v1` refuses each of them by name
/// (`BuilderError::UnsupportedRoute`) rather than building one wrong.
#[tokio::test]
async fn one_founded_market_opens_and_then_closes_its_batch_in_one_bank() {
    const OUTCOME_COUNT: u32 = 2;
    let substrate = waist::fixture_substrate();
    let elves = waist::elves();
    let accelerator_elf = load_accelerator_elf();
    let rent = Rent::default();
    let payer = Keypair::new_from_array([0x11; 32]);
    let fee_payer = Keypair::new_from_array([0x12; 32]);
    let seal_payer = Keypair::new_from_array([0x13; 32]);
    let mut test = waist::program_test_without_forced_budget(&elves);
    let releases = waist::add_release_waist_v2(&mut test, &elves, substrate);
    waist::add_program_v2(
        &mut test,
        "dclutch_general_accelerator_sbf",
        ACCELERATOR_PROGRAM,
        &accelerator_elf,
        substrate,
    );
    let campaign = build_campaign(
        OUTCOME_COUNT,
        payer.pubkey(),
        rent,
        substrate,
        &elves,
        releases,
        &accelerator_elf,
    );
    let open = build_action_case(
        &campaign,
        Action::OpenBatch,
        &genesis_prestate(&campaign),
        substrate.bank_slot(),
        fee_payer.pubkey(),
    );
    add_case_accounts(&mut test, &open, &payer, &fee_payer);
    test.add_account(
        seal_payer.pubkey(),
        Account {
            lamports: GENESIS_PAYER_LAMPORTS,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    waist::add_lookup_table(&mut test, &open.lookup_addresses);
    let mut context = waist::start_with_substrate(test, substrate).await;

    // ---- ACTION ONE: OpenBatch ------------------------------------------
    assert_frame_control(&mut context, &open).await;
    let payer_before_open = chain_account(&mut context, payer.pubkey()).await.lamports;
    let open_execution = waist::submit_v0_observed(
        &mut context,
        &open.instructions,
        open.lookup_addresses.clone(),
        Some(&fee_payer),
        &[&payer],
    )
    .await
    .expect("real Trading -> General accelerator OpenBatch");
    let opened_root = root_tail_of(&chain_account(&mut context, open.root).await);
    assert_eq!(opened_root.revision(), 2);
    assert_eq!(opened_root.open_batches(), 1);
    assert_eq!(opened_root.next_batch_sequence(), 1);

    // ---- THE POSTSTATE IS READ, NOT PREDICTED ---------------------------
    let batch_account = chain_account(&mut context, open.primary_state).await;
    let (envelope, opened_batch) = decode_batch(&batch_account);
    assert_eq!(envelope.header().kind, GeneralLocalStateKindV3::Batch);
    assert_eq!(opened_batch.state().order_count, 0);
    assert_eq!(opened_batch.state().opened_root_revision, 1);
    assert_eq!(opened_batch.state().closed_root_revision, 0);
    let collection_close_slot = opened_batch.opening().collection_close_slot;
    let chain = ChainPrestateV1 {
        market: observed_binding(&mut context, campaign.state.market.key).await,
        root: observed_binding(&mut context, open.root).await,
        rent_credit: observed_binding(&mut context, open.rent_credit).await,
        payer: observed_binding(&mut context, payer.pubkey()).await,
        primary_state: Some(observed_binding(&mut context, open.primary_state).await),
    };

    // THE WINDOW IS THE PROTOCOL'S, NOT THE HARNESS'S. `close_is_permissionless`
    // admits an early close only for a FULL batch; this one holds zero orders,
    // so the config-derived collection window has to elapse and the campaign
    // warps to exactly the slot the Batch itself names.
    assert!(collection_close_slot > substrate.bank_slot());
    context
        .warp_to_slot(collection_close_slot)
        .expect("warp the bank to the batch's own collection close slot");
    let executed_slot = context
        .banks_client
        .get_sysvar::<solana_program::clock::Clock>()
        .await
        .expect("clock sysvar")
        .slot;
    assert_eq!(
        executed_slot, collection_close_slot,
        "the warp is asserted through the executed Clock, not assumed"
    );

    // ---- ACTION TWO: CloseBatch -----------------------------------------
    let close = build_action_case(
        &campaign,
        Action::CloseBatch,
        &chain,
        collection_close_slot,
        fee_payer.pubkey(),
    );
    assert_eq!(
        close.primary_state, open.primary_state,
        "the second action must name the Batch the first one created"
    );
    assert_ne!(
        close.built.bundle.artifacts.seal, open.built.bundle.artifacts.seal,
        "a capability seal is keyed by action; two actions cannot share one"
    );
    let installed =
        install_absent(&mut context, &close, &[close.built.bundle.artifacts.seal]).await;
    let (seal, seal_cu) = produce_seal(&mut context, &close, &seal_payer, &fee_payer).await;
    assert_eq!(seal, close.built.bundle.artifacts.seal);
    waist::set_lookup_table(&mut context, &close.lookup_addresses);
    assert_frame_control(&mut context, &close).await;
    let payer_before_close = chain_account(&mut context, payer.pubkey()).await.lamports;
    let credit_before_close = chain_account(&mut context, open.rent_credit).await;
    let close_execution = waist::submit_v0_observed(
        &mut context,
        &close.instructions,
        close.lookup_addresses.clone(),
        Some(&fee_payer),
        &[&payer],
    )
    .await
    .expect("real Trading -> General accelerator CloseBatch on the same founded market");
    assert!(
        close_execution
            .logs
            .iter()
            .any(|line| line.contains(&format!("Program {ACCELERATOR_PROGRAM} invoke"))),
        "the success log proves the real accelerator CPI ran for the SECOND action"
    );

    // ---- THE TERMINAL STATE ---------------------------------------------
    let closed_batch_account = chain_account(&mut context, open.primary_state).await;
    let (_, closed_batch) = decode_batch(&closed_batch_account);
    assert_eq!(closed_batch.state().status, BatchStatusV1::Closed);
    assert_eq!(closed_batch.state().closed_root_revision, 3);
    assert_eq!(closed_batch.state().opened_root_revision, 1);
    assert_eq!(closed_batch.batch_id(), opened_batch.batch_id());
    assert_eq!(closed_batch.opening(), opened_batch.opening());
    assert_eq!(
        closed_batch_account.lamports, batch_account.lamports,
        "closing a batch's order window moves no principal"
    );
    assert_eq!(closed_batch_account.owner, waist::TRADING_PROGRAM_ID);
    let closed_root = root_tail_of(&chain_account(&mut context, open.root).await);
    assert_eq!(closed_root.revision(), 3);
    assert_eq!(closed_root.open_batches(), 0);
    assert_eq!(
        closed_root.next_batch_sequence(),
        1,
        "a close returns no sequence coordinate"
    );
    assert_eq!(
        chain_account(&mut context, payer.pubkey()).await.lamports,
        payer_before_close,
        "CloseBatch creates nothing and the protocol payer funds nothing"
    );
    assert_eq!(
        chain_account(&mut context, open.rent_credit).await,
        credit_before_close,
        "CloseBatch does not spend credit"
    );
    assert!(
        payer_before_open > payer_before_close,
        "the open funded the Batch principal and the close did not"
    );

    // ---- THE HOSTILE: THE SAME ACTION, ONE SLOT LATER --------------------
    //
    // AN ACTION OUT OF SEQUENCE, stated the only way a same-bank campaign can
    // state it. The host cannot BUILD a second `CloseBatch` -- the projector
    // decodes the batch the bank now holds and `GeneralBatchV1::close` refuses a
    // batch that is not `Collecting` -- so the out-of-order execution that
    // reaches the chain is this one: the exact bundle that just committed,
    // resubmitted against the poststate it produced. Its `expected_revision` is
    // 2 and the root now holds 3.
    //
    // The slot advances so the blockhash differs; a byte-identical transaction
    // at the same blockhash is refused for its SIGNATURE and would prove
    // nothing about a sequence.
    //
    // IT REFUSES AS `Root`, NOT AS `Transition`, and the difference is the
    // finding. The predicted code was `Transition` -- the request asks for
    // revision 2 and the root holds 3, so the candidate projection is where a
    // reader expects the join to fail. The chain refuses earlier and more
    // cheaply: `HotExecutionEnvelopeV3` carries the ROOT PRESTATE DIGEST the
    // bundle was built against, and Trading compares it to the account it was
    // handed before any artifact runs. So a General action executed out of
    // sequence is refused by the market's own state moving under it, which is a
    // stronger statement than an arithmetic mismatch and one no other harness
    // could have made -- it needs two actions on one root.
    context
        .warp_to_slot(collection_close_slot + 1)
        .expect("advance the bank for a distinct blockhash");
    // `Result::expect_err` is unavailable: `SuccessfulExecution` carries the
    // whole program log and is deliberately not `Debug`, so a failed
    // expectation could not print itself. This names the arm and reports what
    // an unexpected success actually cost, which is the one number worth having.
    let replay = match waist::submit_v0_observed(
        &mut context,
        &close.instructions,
        close.lookup_addresses.clone(),
        Some(&fee_payer),
        &[&payer],
    )
    .await
    {
        Ok(execution) => panic!(
            "a CloseBatch against its own poststate committed, at {} CU",
            execution.compute_units_consumed
        ),
        Err(refused) => refused,
    };
    assert_eq!(
        refusal_code(&replay.error),
        Some(TRADING_ROOT),
        "the out-of-sequence close refused with the wrong code: {:#?}",
        replay.logs,
    );
    assert_eq!(
        chain_account(&mut context, open.primary_state).await,
        closed_batch_account,
        "a refused close leaves the Batch byte-for-byte"
    );
    assert_eq!(
        root_tail_of(&chain_account(&mut context, open.root).await).revision(),
        3,
        "a refused close leaves the root revision where it was"
    );

    // ---- ACTION THREE: A SECOND BATCH ON THE SAME MARKET -----------------
    //
    // WHERE "ONE CALL AUCTION PER MARKET" IS TRUE AND WHERE IT IS NOT. The
    // BATCH half is already plural: `GENERAL_BATCH_STATE_RECIPE_V3` keys a
    // batch by (root, batch id) and the root carries a monotonic
    // `next_batch_sequence`, so the second occurrence is a different identity
    // at a different address and the market opens it after the first has
    // closed. That is measured here rather than argued.
    //
    // The SELECTION half WAS not, and this comment said so until `6ce8929ed`
    // landed: `GENERAL_SELECTION_STATE_RECIPE_V3` was keyed by the root ALONE,
    // nothing writes a frozen selection back to `Open`, and a market could
    // therefore open, fill and close as many batches as it liked and could
    // CLEAR in exactly one. The recipe carries the batch identity now, so "one
    // clearing per batch" is a property of an address. What this campaign still
    // cannot show is the second batch SELECTING: `Consider` needs a verified
    // certificate on the bank, which needs a submission, which needs an
    // escrowed order, and none of the three has an installed account here yet.
    // `a_second_batch_on_one_market_derives_its_own_selection_cursor` in the
    // bundle builder's suite is how far that is executed today -- two batches,
    // two cursor addresses, derived rather than argued -- and the on-chain half
    // is owed.
    // READ THE CLOCK, DO NOT PREDICT IT. `warp_to_slot` refuses a slot the bank
    // has already reached, and by this point four transactions and a warp have
    // advanced it by an amount this campaign does not own.
    let before_second = context
        .banks_client
        .get_sysvar::<solana_program::clock::Clock>()
        .await
        .expect("clock sysvar")
        .slot;
    context
        .warp_to_slot(before_second + 1)
        .expect("advance the bank for the second open");
    let second_chain = ChainPrestateV1 {
        market: observed_binding(&mut context, campaign.state.market.key).await,
        root: observed_binding(&mut context, open.root).await,
        rent_credit: observed_binding(&mut context, open.rent_credit).await,
        payer: observed_binding(&mut context, payer.pubkey()).await,
        primary_state: None,
    };
    let second_open = build_action_case(
        &campaign,
        Action::OpenBatch,
        &second_chain,
        before_second + 1,
        fee_payer.pubkey(),
    );
    assert_ne!(
        second_open.primary_state, open.primary_state,
        "the second batch must be a different account, or the root's sequence is not consumed"
    );
    let second_installed = install_absent(
        &mut context,
        &second_open,
        &[second_open.built.bundle.artifacts.seal],
    )
    .await;
    assert_eq!(
        second_installed, 0,
        "a second OpenBatch reuses the first one's artifact records and its seal exactly"
    );
    waist::set_lookup_table(&mut context, &second_open.lookup_addresses);
    assert_frame_control(&mut context, &second_open).await;
    let second_execution = waist::submit_v0_observed(
        &mut context,
        &second_open.instructions,
        second_open.lookup_addresses.clone(),
        Some(&fee_payer),
        &[&payer],
    )
    .await
    .expect("a second OpenBatch on the same founded market");
    let second_batch_account = chain_account(&mut context, second_open.primary_state).await;
    let (_, second_batch) = decode_batch(&second_batch_account);
    assert_eq!(second_batch.opening().sequence, 1);
    assert_eq!(second_batch.state().status, BatchStatusV1::Collecting);
    assert_eq!(second_batch.state().opened_root_revision, 3);
    assert_ne!(second_batch.batch_id(), opened_batch.batch_id());
    let after_second = root_tail_of(&chain_account(&mut context, open.root).await);
    assert_eq!(after_second.revision(), 4);
    assert_eq!(after_second.open_batches(), 1);
    assert_eq!(after_second.next_batch_sequence(), 2);
    // THE FIRST BATCH IS UNTOUCHED, which is what "two auctions" has to mean:
    // a second occurrence that mutated the first one's record would be one
    // auction wearing two names.
    assert_eq!(
        chain_account(&mut context, open.primary_state).await,
        closed_batch_account,
        "opening a second batch leaves the closed one byte-for-byte"
    );

    // ---- ACTION FOUR: A CANDIDATE SUBMITTED AGAINST THE CLOSED BATCH ----
    //
    // THE FIRST GENERAL ACTION THAT READS A RECORD ITS PRIMARY STATE DOES NOT
    // CARRY. `OpenBatch` and `CloseBatch` are the two of the fifteen whose
    // evidence table is empty; every other action names readonly evidence
    // coordinates its AccountProfile declares, and until this ran nothing in the
    // tree had ever bound one -- `build_general_action_bundle_v1` had exactly one
    // caller and it passed `GeneralRequestEvidenceV1::default()`.
    //
    // ONE OF THE THREE EVIDENCE RECORDS IS THIS CAMPAIGN'S OWN POSTSTATE. The
    // `ClosedBatch` coordinate is bound to the Batch account the CloseBatch two
    // actions ago wrote, read back out of the bank -- so the candidate is
    // submitted against a batch that was really opened, really filled with
    // nothing, and really closed, at an identity no line here types.
    //
    // THE OTHER TWO ARE STAGED, AND THAT IS A DEBT WITH A NAME. The candidate
    // image is a solver's immutable publication and the submission record is
    // what this execution writes; on a real chain a solver publishes the first
    // and the Effect produces the second. Here both are installed by
    // `install_absent` out of the bundle's own account list, which exercises
    // every reader and neither writer.
    let config = GeneralConfigV3::decode(&campaign.release.config).expect("General config");
    let submitted_slot = context
        .banks_client
        .get_sysvar::<solana_program::clock::Clock>()
        .await
        .expect("clock sysvar")
        .slot;
    // The submission window is the BATCH's, read off the record the chain wrote.
    assert!(submitted_slot >= closed_batch.opening().collection_close_slot);
    assert!(submitted_slot < closed_batch.opening().settlement_close_slot);

    // The candidate carries its OWN digest as its identity, so it is encoded
    // twice: once to fix every other byte, then again with the digest those
    // bytes produce. A literal here would be a candidate that could name any
    // identity at all, including one already verified under other prices.
    // THE PRICES ARE A SIMPLEX AND THE SCALE IS THE MARKET'S. `CandidateV2`
    // refuses `InvalidSimplex` unless they sum to exactly the config's price
    // scale, which is a million here and not the runtime width the accelerator's
    // own fixture uses -- so a price vector of ones, copied from that fixture,
    // refuses. Derived from the two numbers the founding already fixed.
    let outcomes = usize::try_from(OUTCOME_COUNT).expect("runtime width");
    let per_outcome = config.price_scale() / u64::from(OUTCOME_COUNT);
    let mut uniform_price = vec![per_outcome; outcomes];
    uniform_price[0] += config
        .price_scale()
        .checked_sub(per_outcome * u64::from(OUTCOME_COUNT))
        .expect("the split never exceeds the scale");
    assert_eq!(uniform_price.iter().sum::<u64>(), config.price_scale());
    let draft = CandidateHeaderV2 {
        outcome_count: OUTCOME_COUNT,
        page_count: 1,
        // The candidate's own ordinal among this batch's submissions, and the
        // coordinate a later `Consider` reads out of the certificate. One-based:
        // `CandidateV2` refuses `ZeroCoordinate`, which is how this line stopped
        // being a zero.
        candidate_coordinate: 1,
        price_scale: config.price_scale(),
        candidate_id: [0x7c; 32],
        product_id: campaign.product.product_id,
        batch_id: closed_batch.batch_id(),
    };
    let mut candidate_image = vec![0_u8; candidate_len(OUTCOME_COUNT).expect("candidate width")];
    CandidateV2::encode_into(draft, &uniform_price, &mut candidate_image).expect("draft candidate");
    let candidate_id = general_candidate_identity_v1(&candidate_image).expect("candidate identity");
    CandidateV2::encode_into(
        CandidateHeaderV2 {
            candidate_id,
            ..draft
        },
        &uniform_price,
        &mut candidate_image,
    )
    .expect("addressed candidate");
    let decoded_candidate = CandidateV2::decode(&candidate_image).expect("candidate");
    authenticate_candidate_identity_v1(decoded_candidate).expect("the candidate is its own digest");
    authenticate_batch_candidate_v1(closed_batch, decoded_candidate.header())
        .expect("the candidate authenticates against the batch this market closed");

    // The submission record the execution writes, produced by the protocol's own
    // verb rather than assembled here: it fixes the work capacity, and the
    // escrow is exact in both directions.
    let submission_opening = GeneralCandidateOpeningV1 {
        outcome_count: OUTCOME_COUNT,
        page_count: 1,
        page_revision: CANDIDATE_PAGE_REVISION,
        submitted_slot,
        candidate_id,
        batch_id: closed_batch.batch_id(),
        solver_id: SOLVER,
        row_count: 1,
        reward_rate_lamports: CRANK_REWARD_LAMPORTS,
    };
    let submission = GeneralCandidateV1::submit(
        closed_batch,
        decoded_candidate,
        CANDIDATE_PAGE_REVISION,
        1,
        CRANK_REWARD_LAMPORTS,
        SOLVER,
        submission_opening.work_capacity().expect("work capacity"),
        submitted_slot,
    )
    .expect("submit the candidate against the closed batch");
    assert_eq!(
        submission.state().status,
        GeneralCandidateStatusV1::Submitted
    );

    let evidence = EvidenceCorpusV1 {
        closed_batch: Some(observed_binding(&mut context, open.primary_state).await),
        candidate_image: Some(staged_record(&campaign.rent, candidate_image.clone())),
        submitted_candidate: Some(staged_record(
            &campaign.rent,
            submission.to_bytes().to_vec(),
        )),
    };
    let submit_chain = ChainPrestateV1 {
        market: observed_binding(&mut context, campaign.state.market.key).await,
        root: observed_binding(&mut context, open.root).await,
        rent_credit: observed_binding(&mut context, open.rent_credit).await,
        payer: observed_binding(&mut context, payer.pubkey()).await,
        primary_state: None,
    };

    // THE CORPUS IS THE SHAPE THE ACTION READS, and that is asserted FIRST so
    // the refusal below cannot be a malformed record wearing a register's name.
    // `general_action_prestate_shape_v1` is the same decode
    // `build_general_action_bundle_v1` runs before it builds anything: which
    // record reaches which projector parameter, for this action, with these
    // bytes.
    general_action_prestate_shape_v1(
        Action::SubmitCandidate,
        GeneralActionPrestateV1 {
            primary_state_account: None,
            evidence: evidence.projector(),
        },
    )
    .expect("the campaign's candidate corpus is the shape SubmitCandidate reads");

    // AND THE BUNDLE STILL CANNOT BE BUILT. This is the candidate half's wall,
    // and it is two registers wide.
    //
    // `project_general_submit_candidate_in_place_v3` requires the input bank to
    // carry `identity::CANDIDATE == candidate_id` and
    // `identity::PRIMARY_BENEFICIARY == solver_id`
    // (`hot_candidate_v3.rs:1107` and `:1115`, inside one forty-five-clause
    // conjunct that publishes a single `InvalidCoordinate`). SubmitCandidate's
    // own AccountProfile projects neither: its thirty-three operations
    // (`account_rules_v3.rs:628-795`) write `BEST_VERIFIED_DIGEST`, `ORDER`,
    // `SELECTION_POLICY`, `RESULT_BENEFICIARY_OBSERVATION`, `BENEFICIARY`,
    // `OWNER` and `PAYER`, and `identity::CANDIDATE` is projected only by
    // `PlaceOrder`, `CancelOrder` and `ReleaseOrder`. Measured, not inferred:
    // an instrumented replay of this exact bundle printed every coordinate the
    // conjunct reads, and thirty-seven of the thirty-nine matched -- the two
    // that did not were `CANDIDATE`, which was thirty-two zero bytes, and
    // `PRIMARY_BENEFICIARY`, which held the lifecycle's own beneficiary rather
    // than the solver.
    //
    // The first inference from the same refusal was WRONG and worth recording:
    // the conjunct's first clause checks the per-item `OUTCOME` column, so
    // "nothing projects the item outcomes" was the obvious reading. The probe
    // showed the bank carrying `[0, 1]` exactly as required. A forty-five-clause
    // conjunct behind one code is why that cost a measurement instead of a
    // glance.
    //
    // THE HARNESS IS THE MISSING PRODUCER TODAY. The accelerator's own
    // program-test executes SubmitCandidate on a real ELF because
    // `submit_candidate_bank` writes both registers by hand, so the projector
    // has never been asked for them by a route that assembles its own bank.
    // Nothing here works around it: when the two producers exist, this
    // assertion becomes the execution the campaign owes, and the corpus above
    // is already the one it needs.
    let wall = build_action_case_with_evidence(
        &campaign,
        Action::SubmitCandidate,
        &submit_chain,
        submitted_slot,
        fee_payer.pubkey(),
        &evidence,
    )
    .err();
    assert_eq!(
        wall,
        Some(BuilderError::Projection("general-submit-candidate")),
        "SubmitCandidate no longer refuses at the projector; the campaign owes its execution",
    );

    eprintln!(
        "general-campaign N={OUTCOME_COUNT} market={} root={} batch={}",
        campaign.state.market.key, open.root, open.primary_state,
    );
    eprintln!(
        "general-campaign open-batch cu={} accounts={} invocations={}",
        open_execution.compute_units_consumed,
        open.built.bundle.hot_instruction.accounts.len(),
        open.built.admitted_authorities.entries.len(),
    );
    eprintln!("general-campaign close-batch-seal cu={seal_cu} installed_records={installed}");
    eprintln!(
        "general-campaign close-batch cu={} accounts={} invocations={} slot={}",
        close_execution.compute_units_consumed,
        close.built.bundle.hot_instruction.accounts.len(),
        close.built.admitted_authorities.entries.len(),
        collection_close_slot,
    );
    eprintln!(
        "general-campaign second-open-batch cu={} batch={} sequence={} root_revision=3=>4",
        second_execution.compute_units_consumed,
        second_open.primary_state,
        second_batch.opening().sequence,
    );
    eprintln!(
        "general-campaign out-of-sequence-close cu={} code=0x{TRADING_ROOT:04X}",
        replay.compute_units_consumed,
    );
    eprintln!(
        "general-campaign submit-candidate-wall candidate={} batch={} refusal={:?} \
         missing=identity::CANDIDATE,identity::PRIMARY_BENEFICIARY",
        hex32(candidate_id),
        hex32(closed_batch.batch_id()),
        wall,
    );
}

/// `TradingSbfError::DescriptorManifestEntry`, derived from its REGISTERED BAND.
///
/// The variant `hot_v3` publishes when the selected descriptor and the entry the
/// Market was founded on disagree on one of the five coordinates an entry holds.
const TRADING_DESCRIPTOR_MANIFEST_ENTRY: u32 =
    dclutch_refusal_registry::TRADING_REFUSAL_BASE + 0x015;

/// THE COHORT-15 WALL, ON A REAL ELF, WITH THE ENTRY AS THE ONLY VARIABLE.
///
/// Cohort-15's General market activated under one action's lifecycle policy and
/// its `OpenBatch` refused `0x4015 DescriptorManifestEntry` after 128,724 CU on
/// devnet. That measurement cost a cohort and it has never been reproducible in
/// a harness, because the harness founded its entry from the action it was about
/// to run -- so the two could not disagree.
///
/// They can now: `build_campaign_with_entry` makes the entry a parameter. This
/// founds a market whose manifest entry carries the family policy of a release
/// compiled at ANOTHER Product width -- so its `child_derivation_id` is a real,
/// well-formed policy that simply is not this release's -- and runs the exact
/// `OpenBatch` bundle the campaign above commits. Everything else is byte for
/// byte the same founding.
///
/// THE POSITIVE CONTROL IS THE CAMPAIGN ITSELF: the same code path with the
/// family entry commits, four transactions deep, in the test above. Without that
/// pairing this would be a test that something refuses, which is a test of
/// nothing.
#[tokio::test]
async fn a_market_founded_on_a_foreign_entry_refuses_its_first_action_by_name() {
    const OUTCOME_COUNT: u32 = 2;
    /// A width whose external account widths, and therefore whose family
    /// lifecycle policy, genuinely differ from this market's.
    const FOREIGN_WIDTH: u32 = 13;
    let substrate = waist::fixture_substrate();
    let elves = waist::elves();
    let accelerator_elf = load_accelerator_elf();
    let rent = Rent::default();
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
    let accelerator_release = waist::artifact_id(waist::release_v2(
        ACCELERATOR_PROGRAM,
        0x71,
        &accelerator_elf,
        substrate,
    ));
    let foreign_release = selected_release(
        FOREIGN_WIDTH,
        &build_product(FOREIGN_WIDTH),
        accelerator_release,
    );
    let foreign_entry =
        general_selected_entry_descriptor_v1(&foreign_release).expect("foreign entry descriptor");
    let native_entry = general_selected_entry_descriptor_v1(&selected_release(
        OUTCOME_COUNT,
        &build_product(OUTCOME_COUNT),
        accelerator_release,
    ))
    .expect("family entry descriptor");
    // THE TWO ENTRIES MUST REALLY DIFFER, or this founds the same market twice
    // and refuses for a reason that has nothing to do with an entry.
    assert_ne!(
        CapabilityProgramV4::decode(&foreign_entry)
            .expect("foreign entry")
            .derivation_policy(),
        CapabilityProgramV4::decode(&native_entry)
            .expect("native entry")
            .derivation_policy(),
        "the foreign entry must name a lifecycle policy this release does not"
    );
    let campaign = build_campaign_with_entry(
        OUTCOME_COUNT,
        payer.pubkey(),
        rent,
        substrate,
        &elves,
        releases,
        &accelerator_elf,
        Some(foreign_entry),
    );
    let case = build_action_case(
        &campaign,
        Action::OpenBatch,
        &genesis_prestate(&campaign),
        substrate.bank_slot(),
        fee_payer.pubkey(),
    );
    add_case_accounts(&mut test, &case, &payer, &fee_payer);
    waist::add_lookup_table(&mut test, &case.lookup_addresses);
    let mut context = waist::start_with_substrate(test, substrate).await;
    assert_frame_control(&mut context, &case).await;
    let refused = match waist::submit_v0_observed(
        &mut context,
        &case.instructions,
        case.lookup_addresses.clone(),
        Some(&fee_payer),
        &[&payer],
    )
    .await
    {
        Ok(execution) => panic!(
            "a market founded on a foreign entry opened a batch, at {} CU",
            execution.compute_units_consumed
        ),
        Err(value) => value,
    };
    assert_eq!(
        refusal_code(&refused.error),
        Some(TRADING_DESCRIPTOR_MANIFEST_ENTRY),
        "the foreign entry refused with the wrong code: {:#?}",
        refused.logs,
    );
    assert!(
        chain_account(&mut context, case.primary_state)
            .await
            .data
            .is_empty(),
        "a refused OpenBatch materializes no Batch"
    );
    eprintln!(
        "general-campaign foreign-entry cu={} code=0x{TRADING_DESCRIPTOR_MANIFEST_ENTRY:04X}",
        refused.compute_units_consumed,
    );
}
