//! One current-source General `OpenBatch` through real Trading and accelerator ELFs.

#![allow(clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

#[path = "open_batch/matched_trade.rs"]
mod matched_trade;
#[path = "open_batch/native_export.rs"]
mod native_export;
#[path = "open_batch/verify_continuation.rs"]
mod verify_continuation;

use std::{env, fs, path::PathBuf};

use dclutch_chain_bundle_builder::{
    WaistFactsV1,
    admitted::{AdmittedAotInputV1, create_admitted_output_page_v1},
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
use dclutch_claims::{
    frame_spec_v1::ClaimsFrameRoleV1,
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        LiabilityBasisMarketInputV2, LiabilityBasisMarketSeedsV2, LiabilityBasisMarketViewV2,
        LiabilityBasisPositionInputV2, LiabilityBasisPositionViewV2,
        encode_liability_basis_market_into_v2, encode_liability_basis_position_into_v2,
        put_liability_basis_market_bump_v2, put_liability_basis_position_bump_v2,
    },
    protocol_position_v2::{
        PROTOCOL_POSITION_ADMISSION_BYTES_V2, ProtocolPositionAdmissionV2, ProtocolPositionSeedsV2,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_custody::{
    CompartmentV1, CustodyAuthoritySeedsV1, CustodyFrameRoleV1, CustodyRequestLayoutV1,
    CustodyRequestV1, DELEGATED_CUSTODY_REQUEST_MAGIC_V2, DelegatedCustodyRequestV2, OperationV1,
    token_svm::{PRODUCTION_ADAPTER_RELEASES, TOKEN_2022_PROGRAM_ID, state::TokenAccountLayoutV1},
};
use dclutch_direct_hot_program_test_support::waist;
use dclutch_market::capability_manifest::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityManifestV1,
    FundingQuoteV1, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
    funding::{CompartmentFundingV1, FundingAmountsV1},
};
use dclutch_market::capability_program::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1, SelectedRecordBumpsV1,
    hot_v3::{
        DIRECT_HOT_HEAP_FRAME_BYTES_V1, GENERAL_HOT_HEAP_FRAME_BYTES_V3,
        HOT_CAPABILITY_SEAL_ACCOUNT_V3, HOT_FIXED_ACCOUNT_COUNT_V3,
    },
    set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
    v4::CapabilityProgramV4,
};
use dclutch_market::execution_strategy::v2::{
    AcceleratorTransportProfileV2, ExecutionStrategyProgramV2,
};
use dclutch_market::realm::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1, RealmV1Input,
};
use dclutch_market::rent::{
    RefundAuthority,
    lifecycle_v2::{
        LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleAccountIdV2, LifecycleRentCreditV2,
    },
};
use dclutch_market::{
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
use dclutch_operator::general_session_v1::{
    GeneralEscrowChildrenV1, GeneralFrameSourceV1, general_frame_sources_v1,
};
use dclutch_product::admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_BYTES_V2, PRODUCT_RECORD_SCHEMA_ID_V2,
    RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_product::payoff::{
    registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
    runtime_v3::{
        BasisInputV3, BasisKindV3, basis_record_bytes_v3, compile_basis_v3, semantic_basis_id_v3,
    },
};
use dclutch_product::{
    ContentId as ProductContentId, portfolio_record_bytes, result_domain_record_bytes,
};
use dclutch_product_runtime_v2_operator::{ProductCompilationInputV2, compile_product_records_v2};
use dclutch_registry::record::{ContentDigest, RecordKeyV1, RecordPdaSeedsV1, SchemaReleaseId};
use dclutch_registry::release_set::{ArtifactReleaseIdV1, CapabilityExecutionSelectionV1};
use dclutch_trading::general::{
    candidate_v1::{
        GeneralCandidateOpeningV1, GeneralCandidateStatusV1, GeneralCandidateV1,
        authenticate_candidate_identity_v1, general_candidate_identity_v1,
    },
    collection_v1::{
        BatchStatusV1, GeneralBatchOccurrenceTermsV1, GeneralBatchV2, GeneralOrderHeaderV2,
        GeneralOrderPhaseV1, GeneralOrderStateV1, GeneralOrderV2, authenticate_batch_candidate_v1,
        general_order_len_v2, general_signed_order_terms_len_v2,
    },
    hot_candidate_v3::general_hot_candidate_bank_len_v3,
    local_state_v3::{GeneralLocalStateKindV3, GeneralLocalStateV3},
    runtime_verify::OrderSideV2,
    runtime_width::{CandidateHeaderV2, CandidateV2, candidate_len},
    state_artifacts_v3::{
        GENERAL_PRIMARY_PAYER_ACCOUNT_V3, GENERAL_PRIMARY_STATE_ACCOUNT_V3,
        GENERAL_TERMINAL_STATE_ACCOUNT_V3, GeneralReadonlyEvidenceKindV3,
        general_create_payer_account_v3, general_readonly_evidence_v3,
        general_rent_credit_account_v3, general_system_program_account_v3,
    },
};
use dclutch_trading::general_codec::Action;
use dclutch_trading::general_config::v3::GeneralConfigV3;
use dclutch_trading::general_config::{GENERAL_ROOT_BYTES_V2, GeneralRootV2};
use dclutch_vm::capability_seal::{CAPABILITY_SEAL_BYTES_V1, SealedDescriptorClosureV1};
use dclutch_vm::effect::v2::FixedRole;
use solana_account::Account;
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_program::{hash::hash, instruction::Instruction, pubkey::Pubkey, rent::Rent};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_transaction::Transaction;

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
/// The Product's CLAIM basis, which is NOT what the General config binds.
///
/// It is a free identity here and it is deliberately not the liability basis:
/// the Portfolio carries both, one field apart (`claim_basis_id` @96,
/// `liability_basis_id` @128), and until 2026-09-06 the General AccountProfile
/// projected @96 into `SEMANTIC_BASIS_ID` while the operator filled the config
/// from `semantic_basis_id_v3(linked_basis)` @128. Every fixture agreed with
/// itself because every fixture wrote BOTH sides from this one literal. Keeping
/// the two distinct is what makes `require_market` say anything at all.
const CLAIM_BASIS: [u8; 32] = [0x56; 32];
const GENERAL_TOKEN_PROGRAM: Pubkey = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);
const GENERAL_COLLATERAL_MINT: Pubkey = Pubkey::new_from_array([0xd2; 32]);

#[derive(Clone)]
struct ProductRecords {
    product_id: [u8; 32],
    product: DerivedRecordV1,
    domain: DerivedRecordV1,
    portfolio: DerivedRecordV1,
    basis: DerivedRecordV1,
    /// The semantic basis this founding derived: the Portfolio's
    /// `liability_basis_id` @128, and the value the General config binds.
    semantic_basis: [u8; 32],
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
    // ONE AUTHOR. `semantic_basis_id_v3` is the tree's sole spelling of this
    // domain-separated hash; the operator's `general_market_derivation_v1`
    // reaches the same function for the config's field, so the config and this
    // Portfolio cannot disagree about what the semantic basis IS -- only about
    // which record it was taken from, which is the join `require_market` makes.
    let semantic_basis = semantic_basis_id_v3(&provisional).expect("semantic basis identity");
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
        semantic_basis,
    }
}

fn load_accelerator_elf() -> Vec<u8> {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR"));
    let path = env::var_os("DCLUTCH_GENERAL_ACCELERATOR_ELF_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| directory.join("dclutch_accelerator_sbf.so"));
    fs::read(path).expect("current General accelerator ELF")
}

fn load_token_2022_elf() -> Vec<u8> {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR"));
    fs::read(directory.join("spl_token_2022.so")).expect("current Token-2022 ELF")
}

fn load_rent_elf() -> Vec<u8> {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR"));
    fs::read(directory.join("dclutch_rent_sbf.so")).expect("current Rent ELF")
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
    config_basis: [u8; 32],
) -> GeneralSelectedReleaseV1 {
    general_selected_release_v1(GeneralSelectedReleaseInputV1 {
        capacity_profile: [0x41; 32],
        // DERIVED FROM THE FOUNDING, not written beside it -- and a PARAMETER,
        // so a hostile can found the same market on the other basis. The
        // field's name is the misnomer devnet convicted on 2026-09-06: the
        // operator fills it from `semantic_basis_id_v3(linked_basis)` -- the
        // Portfolio's LIABILITY basis -- and `require_market` compares it
        // against the register the General AccountProfile projects out of that
        // same Portfolio. Spelling `CLAIM_BASIS` here made both sides one
        // literal and made the join vacuous.
        claim_basis: config_basis,
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

#[test]
fn every_general_action_joins_its_own_funding_profile() {
    use dclutch_vm::account_profile::{lifecycle_v3::StateLifecyclePolicyV5, v3::AccountProfileV3};
    let product = build_product(2);
    let release = selected_release(
        2,
        &product,
        ArtifactReleaseIdV1::new([0x71; 32]).unwrap(),
        product.semantic_basis,
    );
    assert_eq!(release.bundles.len(), 15);
    let mut failures = Vec::new();
    for bundle in &release.bundles {
        let descriptor = CapabilityProgramV4::decode(&bundle.descriptor).unwrap();
        let selected = descriptor.lifecycle().program().to_bytes();
        let lifecycle =
            StateLifecyclePolicyV5::decode_selected(selected, selected, &bundle.lifecycle_policy)
                .unwrap();
        let profile = AccountProfileV3::decode(&bundle.account_profile).unwrap();
        if let Err(cause) = lifecycle
            .validate_account_profile_with_external_funding_join_for_action(
                profile,
                u32::from(bundle.action as u8),
            )
        {
            failures.push(format!("{:?}: {cause:?}", bundle.action));
        }
        let strategy = ExecutionStrategyProgramV2::decode(&bundle.strategy).unwrap();
        if strategy.transport_profile() != Ok(AcceleratorTransportProfileV2::OutputPageV3) {
            failures.push(format!(
                "{:?}: General must select OutputPageV3, got {:?}",
                bundle.action,
                strategy.transport_profile(),
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "General action profile joins failed:\n{}",
        failures.join("\n"),
    );
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
        dclutch_market::capability_manifest::ContentId::new(descriptor.kind().to_bytes())
            .expect("kind"),
        dclutch_market::capability_manifest::ContentId::new(digest(&release.program_set))
            .expect("ProgramSet"),
        dclutch_market::capability_manifest::ContentId::new(digest(&release.config))
            .expect("config"),
        dclutch_market::capability_manifest::ContentId::new(
            descriptor.capacity_profile().to_bytes(),
        )
        .expect("capacity"),
        dclutch_market::capability_manifest::ContentId::new(descriptor.root_schema().to_bytes())
            .expect("root schema"),
        dclutch_market::capability_manifest::ContentId::new(
            descriptor.derivation_policy().to_bytes(),
        )
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
        dclutch_market::capability_manifest::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
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
    realm_id: [u8; 32],
) -> StateCorpus {
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
    realm: DerivedRecordV1,
    state: StateCorpus,
    waist_facts: WaistFactsV1,
    accelerator_artifact: Vec<u8>,
    accelerator_program: BuiltAccountV1,
    accelerator_programdata_account: BuiltAccountV1,
    externally_installed: [Pubkey; 3],
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
    /// The one caller-funded accelerator page, read back from the bank before
    /// each action and reused for the whole market session.
    output_page: BuiltAccountV1,
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
    config_basis: Option<[u8; 32]>,
) -> CampaignV1 {
    let product = build_product(outcome_count);
    let accelerator_artifact =
        waist::release_v2(ACCELERATOR_PROGRAM, 0x71, accelerator_elf, substrate);
    let release = selected_release(
        outcome_count,
        &product,
        waist::artifact_id(accelerator_artifact),
        // `None` is what a founding founds: the semantic basis this Product's
        // own linked basis derives, which is the Portfolio's `liability_basis_id`
        // @128. `Some` is a hostile founding a market whose config binds some
        // other identity -- a choice made once, before any action exists, and
        // therefore a parameter and not a mutation.
        config_basis.unwrap_or(product.semantic_basis),
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
    let realm = derive_record(
        waist::REGISTRY_PROGRAM_ID,
        REALM_SCHEMA_RELEASE_ID_V1,
        &RealmV1::new(RealmV1Input {
            token_program: GENERAL_TOKEN_PROGRAM.to_bytes(),
            collateral_mint: GENERAL_COLLATERAL_MINT.to_bytes(),
            collateral_adapter_release_id: hash(&PRODUCTION_ADAPTER_RELEASES[1].to_bytes())
                .to_bytes(),
            mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
            freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
        })
        .expect("Token-2022 Realm")
        .to_bytes(),
    );
    let state = state_corpus(
        &rent,
        payer,
        releases.release_set,
        &product,
        &manifest,
        &release,
        realm.digest,
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
        rent: rent.clone(),
        substrate,
        releases,
        product,
        release,
        manifest,
        realm,
        state,
        waist_facts,
        accelerator_artifact: accelerator_artifact.to_bytes().to_vec(),
        accelerator_program: observed_rent_funded(
            &rent,
            program_with_view(ACCELERATOR_PROGRAM, accelerator_programdata),
        ),
        accelerator_programdata_account: observed_rent_funded(
            &rent,
            external_with_view(
                accelerator_programdata,
                bpf_loader_upgradeable::ID,
                waist::programdata_v2(substrate, accelerator_elf),
            ),
        ),
        externally_installed: [
            ACCELERATOR_PROGRAM,
            accelerator_programdata,
            GENERAL_TOKEN_PROGRAM,
        ],
    }
}

/// The founding's own prestate: what the bank holds before any action runs.
fn output_page_width(campaign: &CampaignV1) -> usize {
    dclutch_trading::general::release_v3::GENERAL_ACTIONS_V3
        .into_iter()
        .map(|action| {
            general_hot_candidate_bank_len_v3(action, campaign.outcome_count)
                .expect("General output-page width")
        })
        .max()
        .expect("General has actions")
}

fn expected_output_page(campaign: &CampaignV1, key: Pubkey) -> BuiltAccountV1 {
    data_account(
        &campaign.rent,
        key,
        ACCELERATOR_PROGRAM,
        vec![0; output_page_width(campaign)],
    )
}

fn genesis_prestate(campaign: &CampaignV1, output_page: Pubkey) -> ChainPrestateV1 {
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
        output_page: expected_output_page(campaign, output_page),
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
    /// The exact immutable bytes a maker signed (`PlaceOrder`'s sole evidence).
    order_terms: Option<BuiltAccountV1>,
    /// Solver-published canonical page carrying the next execution row.
    candidate_page: Option<BuiltAccountV1>,
    /// Live Order created by this campaign's PlaceOrder.
    order_account: Option<BuiltAccountV1>,
    /// Exact manifest chunk produced by replaying that row.
    settlement_manifest: Option<BuiltAccountV1>,
}

impl EvidenceCorpusV1 {
    /// The records the REQUEST derivation reads, per action.
    fn request(&self) -> GeneralRequestEvidenceV1<'_> {
        GeneralRequestEvidenceV1 {
            candidate_image: self.candidate_image.as_ref().map(built_bytes),
            signed_order_terms: self.order_terms.as_ref().map(built_bytes),
            candidate_page: self.candidate_page.as_ref().map(built_bytes),
            order_account: self.order_account.as_ref().map(built_bytes),
            settlement_manifest: self.settlement_manifest.as_ref().map(built_bytes),
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
            (GeneralReadonlyEvidenceKindV3::OrderTerms, &self.order_terms),
            (
                GeneralReadonlyEvidenceKindV3::CandidatePage,
                &self.candidate_page,
            ),
            (
                GeneralReadonlyEvidenceKindV3::EscrowedOrder,
                &self.order_account,
            ),
            (
                GeneralReadonlyEvidenceKindV3::SettlementManifest,
                &self.settlement_manifest,
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

/// Canonical, founded child state a live General order admission consumes.
///
/// The action owns four lifecycle-created children (the escrow Position and
/// admission, plus the Custody replay and vault); they are therefore vacant at
/// the exact PDAs below.  The maker Position, Claims aggregate, Realm, mint,
/// and source token account predate the order and are installed as real
/// records.  Filling this with width-only placeholders would get through the
/// profile builder and fail at the first child CPI, so the corpus is assembled
/// from the child contracts' own encoders and seed constructors.
struct PlaceOrderCorpusV1 {
    claims_market: BuiltAccountV1,
    maker_position: BuiltAccountV1,
    realm: BuiltAccountV1,
    realm_staging: BuiltAccountV1,
    mint: BuiltAccountV1,
    maker_token: BuiltAccountV1,
    escrow_position: BuiltAccountV1,
    escrow_admission: BuiltAccountV1,
    escrow_replay: BuiltAccountV1,
    escrow_vault: BuiltAccountV1,
    custody_authority: BuiltAccountV1,
    order_identity: BuiltAccountV1,
    order_owner: BuiltAccountV1,
}

fn token_mint_bytes(supply: u64) -> Vec<u8> {
    let mut bytes = vec![0_u8; 82];
    bytes[36..44].copy_from_slice(&supply.to_le_bytes());
    bytes[45] = 1;
    bytes
}

fn token_account_bytes(
    mint: Pubkey,
    owner: Pubkey,
    amount: u64,
    delegate: Pubkey,
    delegated_amount: u64,
) -> Vec<u8> {
    let mut bytes = vec![0_u8; 165];
    bytes[..32].copy_from_slice(mint.as_ref());
    bytes[32..64].copy_from_slice(owner.as_ref());
    bytes[64..72].copy_from_slice(&amount.to_le_bytes());
    bytes[TokenAccountLayoutV1::DELEGATE..TokenAccountLayoutV1::DELEGATE + 4]
        .copy_from_slice(&1_u32.to_le_bytes());
    bytes[TokenAccountLayoutV1::DELEGATE + 4..TokenAccountLayoutV1::DELEGATE + 36]
        .copy_from_slice(delegate.as_ref());
    bytes[TokenAccountLayoutV1::DELEGATED_AMOUNT..TokenAccountLayoutV1::DELEGATED_AMOUNT + 8]
        .copy_from_slice(&delegated_amount.to_le_bytes());
    bytes[108] = 1;
    bytes
}

fn token_account_amount(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(
        bytes
            .get(64..72)
            .expect("canonical token account amount")
            .try_into()
            .expect("u64 token amount"),
    )
}

fn token_account_delegate_tag(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(
        bytes
            .get(TokenAccountLayoutV1::DELEGATE..TokenAccountLayoutV1::DELEGATE + 4)
            .expect("canonical token delegate tag")
            .try_into()
            .expect("u32 delegate tag"),
    )
}

fn token_account_delegated_amount(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(
        bytes
            .get(TokenAccountLayoutV1::DELEGATED_AMOUNT..TokenAccountLayoutV1::DELEGATED_AMOUNT + 8)
            .expect("canonical token delegated amount")
            .try_into()
            .expect("u64 delegated amount"),
    )
}

/// A child-owned account that has been prepaid but remains vacant until its
/// owner allocates it. Claims admission deliberately requires this floor while
/// still requiring the System owner and an empty body: no caller can allocate
/// the off-curve PDA before the Claims program does.
fn prepaid_vacant(rent: &Rent, key: Pubkey, eventual_bytes: usize) -> BuiltAccountV1 {
    let mut account = vacant(key);
    account.account.lamports = rent.minimum_balance(eventual_bytes);
    account
}

/// The loader, sysvar, and Registry records this frame reads are already
/// installed by ProgramTest.  Their chain view carries the current rent floor,
/// even though the harness keeps an inert external installation placeholder.
fn observed_rent_funded(rent: &Rent, account: BuiltAccountV1) -> BuiltAccountV1 {
    let mut observed = account.chain_view().clone();
    if !observed.data.is_empty() {
        observed.lamports = rent.minimum_balance(observed.data.len());
    }
    account.with_observed(observed)
}

fn place_order_corpus(
    campaign: &CampaignV1,
    chain: &ChainPrestateV1,
    order_id: [u8; 32],
    quote_reserve: u64,
) -> PlaceOrderCorpusV1 {
    let claims_market = Pubkey::find_program_address(
        &LiabilityBasisMarketSeedsV2::new(chain.market.key.to_bytes())
            .expect("General Claims aggregate seeds")
            .as_slices(),
        &waist::CLAIMS_PROGRAM_ID,
    )
    .0;
    let mut claims_bytes =
        vec![
            0_u8;
            LIABILITY_BASIS_MARKET_HEADER_BYTES_V2
                + usize::try_from(campaign.outcome_count).expect("outcome width") * 8
        ];
    encode_liability_basis_market_into_v2(
        LiabilityBasisMarketInputV2 {
            revision: 1,
            logical_market: chain.market.key.to_bytes(),
            release_set: campaign.releases.release_set,
            registry_program: waist::REGISTRY_PROGRAM_ID.to_bytes(),
            product_instance_id: campaign.product.product_id,
            basis_id: campaign.product.semantic_basis,
            realm_id: campaign.realm.digest,
            custody_context: [0xd3; 32],
            generation: GENERATION,
        },
        &vec![4_u64; usize::try_from(campaign.outcome_count).expect("outcome width")],
        &mut claims_bytes,
    )
    .expect("founded General Claims aggregate");
    let claims_bump = Pubkey::find_program_address(
        &LiabilityBasisMarketSeedsV2::new(chain.market.key.to_bytes())
            .expect("General Claims aggregate seeds")
            .as_slices(),
        &waist::CLAIMS_PROGRAM_ID,
    )
    .1;
    put_liability_basis_market_bump_v2(&mut claims_bytes, claims_bump)
        .expect("Claims aggregate bump");

    let maker = chain.payer.key;
    let maker_position = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(claims_market.to_bytes(), maker.to_bytes())
            .expect("maker Position seeds")
            .as_slices(),
        &waist::CLAIMS_PROGRAM_ID,
    );
    let mut maker_position_bytes =
        vec![
            0_u8;
            LIABILITY_BASIS_POSITION_HEADER_BYTES_V2
                + usize::try_from(campaign.outcome_count).expect("outcome width") * 8
        ];
    encode_liability_basis_position_into_v2(
        LiabilityBasisPositionInputV2 {
            revision: 1,
            market_account: claims_market.to_bytes(),
            owner: maker.to_bytes(),
            basis_id: campaign.product.semantic_basis,
        },
        &vec![4_u64; usize::try_from(campaign.outcome_count).expect("outcome width")],
        &mut maker_position_bytes,
    )
    .expect("maker Claims Position");
    put_liability_basis_position_bump_v2(&mut maker_position_bytes, maker_position.1)
        .expect("maker Position bump");

    let order_seeds = dclutch_trading::general::state_seeds_v3::GeneralStateAddressSeedsV3::order(
        chain.root.key.to_bytes(),
        order_id,
    )
    .expect("Order lifecycle seeds");
    let order_seed_slices = order_seeds.as_slices().expect("Order seed slices");
    let order_owner =
        Pubkey::find_program_address(order_seed_slices.as_slice(), &waist::TRADING_PROGRAM_ID).0;
    let children = GeneralEscrowChildrenV1::derive(
        waist::CLAIMS_PROGRAM_ID,
        waist::CUSTODY_PROGRAM_ID,
        claims_market,
        chain.market.key.to_bytes(),
        campaign.releases.release_set,
        order_id,
        order_owner,
    )
    .expect("order escrow children");
    let custody_authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::new(chain.market.key.to_bytes(), campaign.releases.release_set)
            .as_slices(),
        &waist::CUSTODY_PROGRAM_ID,
    )
    .0;
    let maker_token = Pubkey::find_program_address(
        &[b"general-place-maker-token", maker.as_ref()],
        &GENERAL_TOKEN_PROGRAM,
    )
    .0;
    PlaceOrderCorpusV1 {
        claims_market: data_account(
            &campaign.rent,
            claims_market,
            waist::CLAIMS_PROGRAM_ID,
            claims_bytes,
        ),
        maker_position: data_account(
            &campaign.rent,
            maker_position.0,
            waist::CLAIMS_PROGRAM_ID,
            maker_position_bytes,
        ),
        realm: data_account(
            &campaign.rent,
            campaign.realm.raw,
            waist::REGISTRY_PROGRAM_ID,
            campaign.realm.bytes.clone(),
        ),
        realm_staging: vacant(campaign.realm.staging),
        mint: data_account(
            &campaign.rent,
            GENERAL_COLLATERAL_MINT,
            GENERAL_TOKEN_PROGRAM,
            token_mint_bytes(quote_reserve),
        ),
        maker_token: data_account(
            &campaign.rent,
            maker_token,
            GENERAL_TOKEN_PROGRAM,
            token_account_bytes(
                GENERAL_COLLATERAL_MINT,
                maker,
                quote_reserve,
                custody_authority,
                quote_reserve,
            ),
        ),
        // Claims admission has no payer account. Its Position and admission
        // PDAs must therefore arrive as System-owned, data-empty accounts
        // prepaid to the exact widths it will allocate. Keeping them at zero
        // made the host encode an underfunded child request that Claims
        // correctly refused before the CPI could create either account.
        escrow_position: prepaid_vacant(
            &campaign.rent,
            children.position,
            LIABILITY_BASIS_POSITION_HEADER_BYTES_V2
                + usize::try_from(campaign.outcome_count).expect("outcome width") * 8,
        ),
        escrow_admission: prepaid_vacant(
            &campaign.rent,
            children.admission,
            PROTOCOL_POSITION_ADMISSION_BYTES_V2,
        ),
        escrow_replay: vacant(children.replay),
        escrow_vault: vacant(children.vault),
        custody_authority: vacant(custody_authority),
        order_identity: vacant(Pubkey::new_from_array(order_id)),
        order_owner: vacant(order_owner),
    }
}

fn place_order_child_bindings(
    campaign: &CampaignV1,
    chain: &ChainPrestateV1,
    corpus: &PlaceOrderCorpusV1,
    order_terms: &BuiltAccountV1,
    rent_sysvar: &BuiltAccountV1,
) -> Vec<(usize, BuiltAccountV1)> {
    let program = |key| {
        observed_rent_funded(
            &campaign.rent,
            program_with_view(key, waist::programdata(key)),
        )
    };
    // ProgramTest owns the Rent sysvar body.  Its current wire uses the
    // post-SIMD-0194 layout while `solana_program::Rent::default()` has the
    // legacy-but-economically-equivalent representation, so only the bank
    // observation is authoritative for this admitted transcript.
    let rent_sysvar = || rent_sysvar.clone();
    // The activation cache is an already-finalized Registry record.  Unlike a
    // sysvar or builtin, it carries its serialized account rent at execution;
    // model it with the same rent-funded record constructor the bank used when
    // release waist installed it.  Treating the exact body as an external
    // one-lamport view changed the admitted observation transcript before the
    // Custody InitializeReplay CPI.
    let activation = || {
        data_account(
            &campaign.rent,
            campaign.releases.activation,
            waist::REGISTRY_PROGRAM_ID,
            campaign.releases.activation_data.to_vec(),
        )
    };
    // The outer General Profile names the two affine positions by their
    // semantic roles: maker, then the escrow Position that the immediately
    // preceding Claims Admit creates.  Trading's typed Claims adapter rewrites
    // that private child plan and gathers this pair in canonical PDA-key order
    // just before its CPI; sorting here would make the outer escrow alias
    // key-dependent and erase the lifecycle/create cross-check.
    let semantic_positions = [
        corpus.maker_position.clone(),
        corpus.escrow_position.clone(),
    ];
    general_frame_sources_v1(Action::PlaceOrder)
        .expect("PlaceOrder frame sources")
        .into_iter()
        .filter_map(|(coordinate, source)| {
            let account = match source {
                GeneralFrameSourceV1::Claims(_, role) => match role {
                    ClaimsFrameRoleV1::CallerAuthority => return None,
                    ClaimsFrameRoleV1::ClaimsMarket => corpus.claims_market.clone(),
                    ClaimsFrameRoleV1::ProtocolPosition => corpus.escrow_position.clone(),
                    ClaimsFrameRoleV1::ProtocolPositionAdmission => corpus.escrow_admission.clone(),
                    ClaimsFrameRoleV1::BasisRecord
                    | ClaimsFrameRoleV1::ProductRecord
                    | ClaimsFrameRoleV1::PortfolioRecord => return None,
                    ClaimsFrameRoleV1::BasisStaging => vacant(campaign.product.basis.staging),
                    ClaimsFrameRoleV1::ProductStaging => vacant(campaign.product.product.staging),
                    ClaimsFrameRoleV1::ResultDomainRecord => data_account(
                        &campaign.rent,
                        campaign.product.domain.raw,
                        waist::REGISTRY_PROGRAM_ID,
                        campaign.product.domain.bytes.clone(),
                    ),
                    ClaimsFrameRoleV1::ResultDomainStaging => {
                        vacant(campaign.product.domain.staging)
                    }
                    ClaimsFrameRoleV1::PortfolioStaging => {
                        vacant(campaign.product.portfolio.staging)
                    }
                    ClaimsFrameRoleV1::RentSysvar => rent_sysvar(),
                    ClaimsFrameRoleV1::SystemProgram => system_program_builtin(),
                    ClaimsFrameRoleV1::CoreMarket => chain.market.clone(),
                    ClaimsFrameRoleV1::ActivationCache => activation(),
                    ClaimsFrameRoleV1::RegistryProgram => program(waist::REGISTRY_PROGRAM_ID),
                    ClaimsFrameRoleV1::TradingProgram | ClaimsFrameRoleV1::CallerProgram => {
                        program(waist::TRADING_PROGRAM_ID)
                    }
                    ClaimsFrameRoleV1::TradingProgramData
                    | ClaimsFrameRoleV1::CallerProgramData => observed_rent_funded(
                        &campaign.rent,
                        external_with_view(
                            campaign.releases.trading_programdata,
                            bpf_loader_upgradeable::ID,
                            waist::programdata_v2(campaign.substrate, &waist::elves().trading),
                        ),
                    ),
                    ClaimsFrameRoleV1::ClaimsProgram => program(waist::CLAIMS_PROGRAM_ID),
                    ClaimsFrameRoleV1::ClaimsProgramData => observed_rent_funded(
                        &campaign.rent,
                        external_with_view(
                            campaign.releases.claims_programdata,
                            bpf_loader_upgradeable::ID,
                            waist::programdata_v2(campaign.substrate, &waist::elves().claims),
                        ),
                    ),
                    ClaimsFrameRoleV1::CoreProgram => program(waist::CORE_PROGRAM_ID),
                    ClaimsFrameRoleV1::CoreProgramData => observed_rent_funded(
                        &campaign.rent,
                        external_with_view(
                            campaign.releases.core_programdata,
                            bpf_loader_upgradeable::ID,
                            waist::programdata_v2(campaign.substrate, &waist::elves().core),
                        ),
                    ),
                    ClaimsFrameRoleV1::PositionOwnerIdentity => corpus.order_owner.clone(),
                    ClaimsFrameRoleV1::RentCredit => chain.rent_credit.clone(),
                    ClaimsFrameRoleV1::RentProgram => program(waist::RENT_PROGRAM_ID),
                    ClaimsFrameRoleV1::AffinePosition(index) => semantic_positions
                        .get(usize::from(index))
                        .expect("PlaceOrder affine Position")
                        .clone(),
                    ClaimsFrameRoleV1::SignedDeltaPosition(_)
                    | ClaimsFrameRoleV1::SparseSourcePosition
                    | ClaimsFrameRoleV1::SparseDestinationPosition => {
                        panic!("PlaceOrder has no sparse or signed-delta Claims route")
                    }
                },
                GeneralFrameSourceV1::Custody(_, role) => match role {
                    CustodyFrameRoleV1::CallerAuthority => return None,
                    CustodyFrameRoleV1::CoreMarket => chain.market.clone(),
                    CustodyFrameRoleV1::ActivationCache => activation(),
                    CustodyFrameRoleV1::RegistryProgram => program(waist::REGISTRY_PROGRAM_ID),
                    CustodyFrameRoleV1::CallerProgram => program(waist::TRADING_PROGRAM_ID),
                    CustodyFrameRoleV1::CallerProgramData => observed_rent_funded(
                        &campaign.rent,
                        external_with_view(
                            campaign.releases.trading_programdata,
                            bpf_loader_upgradeable::ID,
                            waist::programdata_v2(campaign.substrate, &waist::elves().trading),
                        ),
                    ),
                    CustodyFrameRoleV1::RealmRecord => corpus.realm.clone(),
                    CustodyFrameRoleV1::RealmStaging => corpus.realm_staging.clone(),
                    CustodyFrameRoleV1::Replay => corpus.escrow_replay.clone(),
                    CustodyFrameRoleV1::Payer => chain.payer.clone(),
                    CustodyFrameRoleV1::SystemProgram => system_program_builtin(),
                    CustodyFrameRoleV1::RentSysvar => rent_sysvar(),
                    CustodyFrameRoleV1::Mint => corpus.mint.clone(),
                    CustodyFrameRoleV1::Vault => corpus.escrow_vault.clone(),
                    CustodyFrameRoleV1::CustodyAuthority => corpus.custody_authority.clone(),
                    CustodyFrameRoleV1::TokenProgram => program(GENERAL_TOKEN_PROGRAM),
                    CustodyFrameRoleV1::TransferSource => corpus.maker_token.clone(),
                    CustodyFrameRoleV1::TransferDestination => corpus.escrow_vault.clone(),
                    CustodyFrameRoleV1::RentRefund => chain.payer.clone(),
                },
                GeneralFrameSourceV1::CustodyCallee => program(waist::CUSTODY_PROGRAM_ID),
                _ => return None,
            };
            Some((usize::from(coordinate), account))
        })
        .collect()
}

/// The child frame does not install its selected programs, loader records, or
/// sysvars.  Their account facts belong to ProgramTest's bank.  Re-read those
/// exact facts after `OpenBatch` rather than predicting them from a local
/// `Rent` default: the profile still authenticates the selected addresses and
/// the child adapters still authenticate the bodies they consume.
async fn observe_existing_place_order_child_bindings(
    context: &mut solana_program_test::ProgramTestContext,
    bindings: Vec<(usize, BuiltAccountV1)>,
) -> Vec<(usize, BuiltAccountV1)> {
    let sources = general_frame_sources_v1(Action::PlaceOrder).expect("PlaceOrder frame sources");
    let is_bank_owned = |source: GeneralFrameSourceV1| match source {
        GeneralFrameSourceV1::SystemProgram | GeneralFrameSourceV1::CustodyCallee => true,
        GeneralFrameSourceV1::Claims(_, role) => matches!(
            role,
            ClaimsFrameRoleV1::RentSysvar
                | ClaimsFrameRoleV1::SystemProgram
                | ClaimsFrameRoleV1::RegistryProgram
                | ClaimsFrameRoleV1::TradingProgram
                | ClaimsFrameRoleV1::CallerProgram
                | ClaimsFrameRoleV1::TradingProgramData
                | ClaimsFrameRoleV1::CallerProgramData
                | ClaimsFrameRoleV1::ClaimsProgram
                | ClaimsFrameRoleV1::ClaimsProgramData
                | ClaimsFrameRoleV1::CoreProgram
                | ClaimsFrameRoleV1::CoreProgramData
                | ClaimsFrameRoleV1::RentProgram
        ),
        GeneralFrameSourceV1::Custody(_, role) => matches!(
            role,
            CustodyFrameRoleV1::RegistryProgram
                | CustodyFrameRoleV1::CallerProgram
                | CustodyFrameRoleV1::CallerProgramData
                | CustodyFrameRoleV1::SystemProgram
                | CustodyFrameRoleV1::RentSysvar
                | CustodyFrameRoleV1::TokenProgram
        ),
        _ => false,
    };
    let mut observed = Vec::with_capacity(bindings.len());
    for (coordinate, binding) in bindings {
        let source = sources
            .iter()
            .find_map(|(source_coordinate, source)| {
                (*source_coordinate == u16::try_from(coordinate).expect("frame coordinate"))
                    .then_some(*source)
            })
            .expect("every PlaceOrder child binding has a typed source");
        if is_bank_owned(source) {
            observed.push((coordinate, observed_binding(context, binding.key).await));
        } else {
            observed.push((coordinate, binding));
        }
    }
    observed
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

/// The explicit Hot heap frame this diagnostic asks the runtime to grant.
///
/// General's canonical profile is 128 KiB. A test may select the former 64 KiB
/// profile only to retain the measured named refusal; Direct remains separately
/// owned at 64 KiB.
fn general_hot_heap_frame_bytes_v1(action: Action) -> u32 {
    const DIAGNOSTIC_HEAP_ENV_V1: &str = "DCLUTCH_GENERAL_HOT_DIAGNOSTIC_HEAP_BYTES";
    if action != Action::PlaceOrder {
        return GENERAL_HOT_HEAP_FRAME_BYTES_V3;
    }
    match env::var(DIAGNOSTIC_HEAP_ENV_V1) {
        Err(env::VarError::NotPresent) => GENERAL_HOT_HEAP_FRAME_BYTES_V3,
        Ok(value) => match value.parse::<u32>() {
            Ok(frame @ (65_536 | GENERAL_HOT_HEAP_FRAME_BYTES_V3)) => frame,
            _ => panic!("{DIAGNOSTIC_HEAP_ENV_V1} must be 65536 or 131072"),
        },
        Err(error) => panic!("cannot read {DIAGNOSTIC_HEAP_ENV_V1}: {error}"),
    }
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
    build_action_case_with_evidence_and_bindings(
        campaign,
        action,
        chain,
        clock_slot,
        fee_payer,
        evidence,
        &[],
    )
}

#[allow(clippy::too_many_arguments)]
fn build_action_case_with_evidence_and_bindings(
    campaign: &CampaignV1,
    action: Action,
    chain: &ChainPrestateV1,
    clock_slot: u64,
    fee_payer: Pubkey,
    evidence: &EvidenceCorpusV1,
    additional_bindings: &[(usize, BuiltAccountV1)],
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
    // THE PAYER AND THE CREDIT SIT WHERE THE ACTION PUTS THEM, NOT AT 6 AND 7.
    //
    // Those two literals are the primary shape's coordinates and are right for
    // nine of the fifteen actions. `PlaceOrder`, `CancelOrder` and `Close` put
    // the payer at 7 and the credit at 8; `VerifyCandidateRow` puts them at 8
    // and 9. Bound at the literals, `PlaceOrder` presented a 128-byte RentCredit
    // record at coordinate 7 -- whose rule for that action declares nothing --
    // and its order state at 6, and the account-projection kernel refused
    // `DataLengthMismatch` over a fifty-coordinate frame. `state_artifacts_v3`
    // is the one author of both coordinates and the emitted plans now take
    // theirs from the same functions.
    //
    // `CloseCandidate` alone declares no create payer, and its profile still
    // names a payer ACCOUNT at the primary coordinate for the Effect to read,
    // so the fallback is that coordinate rather than no binding.
    let mut bindings = vec![
        (
            usize::from(
                general_create_payer_account_v3(action).unwrap_or(GENERAL_PRIMARY_PAYER_ACCOUNT_V3),
            ),
            chain.payer.clone(),
        ),
        (
            usize::from(general_rent_credit_account_v3(action)),
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
    bindings.extend(additional_bindings.iter().cloned());
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
            output_page: Some(&chain.output_page),
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
        ComputeBudgetInstruction::request_heap_frame(general_hot_heap_frame_bytes_v1(action)),
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

/// Create the page through the same signed System instruction a real caller
/// uses, then return the bank's account body. The bundle never installs this
/// account as a fixture.
async fn provision_output_page(
    context: &mut solana_program_test::ProgramTestContext,
    campaign: &CampaignV1,
    payer: &Keypair,
    fee_payer: &Keypair,
    page: &Keypair,
) -> BuiltAccountV1 {
    let instruction = create_admitted_output_page_v1(
        &payer.pubkey(),
        &page.pubkey(),
        &ACCELERATOR_PROGRAM,
        output_page_width(campaign),
        &campaign.rent,
    )
    .expect("real caller-funded output-page creation instruction");
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("output-page creation blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&fee_payer.pubkey()),
        &[fee_payer, payer, page],
        blockhash,
    );
    context
        .banks_client
        .process_transaction(transaction)
        .await
        .expect("System created the reusable accelerator output page");
    let observed = observed_binding(context, page.pubkey()).await;
    assert_eq!(observed.account.owner, ACCELERATOR_PROGRAM);
    assert!(!observed.account.executable);
    assert_eq!(observed.account.data.len(), output_page_width(campaign));
    assert_eq!(
        observed.account.lamports,
        campaign.rent.minimum_balance(output_page_width(campaign)),
        "the caller pays exactly the page's reusable scratch rent"
    );
    observed
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
    let sources = general_frame_sources_v1(case.action).expect("General frame sources");
    let mut mismatches = Vec::new();
    for coordinate in 0..case.built.bundle.logical.len() {
        let Some(built) = case.built.bundle.logical.get(coordinate) else {
            continue;
        };
        let view = built.chain_view();
        let observed = chain_account(context, built.key).await;
        if observed.owner != view.owner
            || observed.lamports != view.lamports
            || observed.data != view.data
            || observed.executable != view.executable
        {
            let source = sources.iter().find_map(|(source_coordinate, source)| {
                (usize::from(*source_coordinate) == coordinate).then_some(*source)
            });
            let first_byte = observed
                .data
                .iter()
                .zip(view.data.iter())
                .position(|(actual, modelled)| actual != modelled)
                .map_or_else(
                    || {
                        if observed.data.len() == view.data.len() {
                            "none".to_owned()
                        } else {
                            "past shared prefix".to_owned()
                        }
                    },
                    |index| {
                        format!(
                            "{index}: bank={:#04x} host={:#04x}",
                            observed.data[index], view.data[index]
                        )
                    },
                );
            mismatches.push(format!(
                "coordinate {coordinate} source={source:?} key={} bank=({},{},{},{}) host=({},{},{},{}) first_data_difference={first_byte}",
                built.key,
                observed.owner,
                observed.lamports,
                observed.data.len(),
                observed.executable,
                view.owner,
                view.lamports,
                view.data.len(),
                view.executable,
            ));
        }
    }
    assert!(
        mismatches.is_empty(),
        "{:?}: host frame differs from bank:\n{}",
        case.action,
        mismatches.join("\n")
    );
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
    let output_page = case.built.admitted_authorities.output_page;
    for install in &case.built.bundle.accounts {
        if !case
            .built
            .bundle
            .externally_installed_keys
            .contains(&install.key)
            && Some(install.key) != output_page
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
    let output_page_signer = Keypair::new_from_array([0x13; 32]);
    let mut test = waist::program_test_without_forced_budget(&elves);
    let releases = waist::add_release_waist_v2(&mut test, &elves, substrate);
    waist::add_program_v2(
        &mut test,
        "dclutch_accelerator_sbf",
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
    let mut chain = genesis_prestate(&campaign, output_page_signer.pubkey());
    let fixture_case = build_action_case(
        &campaign,
        Action::OpenBatch,
        &chain,
        substrate.bank_slot(),
        fee_payer.pubkey(),
    );
    // General selects one accelerator-owned output page for its complete
    // authenticated bank. The page route has one caller authority whatever the
    // bank width, while input remains inline and owns no transport span.
    assert!(fixture_case.built.bundle.span_counts.is_empty());
    assert_eq!(fixture_case.built.admitted_authorities.entries.len(), 1);
    let output_page = fixture_case
        .built
        .admitted_authorities
        .output_page
        .expect("the selected OutputPageV3 strategy derives one page");
    assert!(
        fixture_case.built.bundle.hot_instruction.accounts.len() <= 100,
        "the exact ALT route stays under the v0 account ceiling"
    );
    eprintln!(
        "general-open-batch geometry N={outcome_count} instruction_accounts={} logical={} span_counts={:?} page_authorities={} output_page={} runtime_start={}",
        fixture_case.built.bundle.hot_instruction.accounts.len(),
        fixture_case.built.bundle.logical.len(),
        fixture_case.built.bundle.span_counts,
        fixture_case.built.admitted_authorities.entries.len(),
        output_page,
        47 + fixture_case.built.admitted_authorities.entries.len() + 1,
    );
    eprintln!(
        "general-open-batch registers N={outcome_count} scalars={} identities={}",
        fixture_case.built.bundle.engine.input_scalars.len(),
        fixture_case.built.bundle.engine.input_identities.len(),
    );
    add_case_accounts(&mut test, &fixture_case, &payer, &fee_payer);
    let lookup_addresses = fixture_case.lookup_addresses.clone();
    waist::add_lookup_table(&mut test, &lookup_addresses);
    let mut context = waist::start_with_substrate(test, substrate).await;
    chain.output_page = provision_output_page(
        &mut context,
        &campaign,
        &payer,
        &fee_payer,
        &output_page_signer,
    )
    .await;
    chain.payer = observed_binding(&mut context, payer.pubkey()).await;
    let case = build_action_case(
        &campaign,
        Action::OpenBatch,
        &chain,
        substrate.bank_slot(),
        fee_payer.pubkey(),
    );
    let instructions = case.instructions.clone();
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
    let page_before = chain_account(&mut context, output_page).await;
    assert_eq!(page_before.owner, ACCELERATOR_PROGRAM);
    assert!(!page_before.executable);
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
    assert_eq!(
        execution
            .logs
            .iter()
            .filter(|line| line.contains(&format!("Program {ACCELERATOR_PROGRAM} invoke")))
            .count(),
        1,
        "OutputPageV3 invokes the accelerator once for the whole bank"
    );
    let page_after = chain_account(&mut context, output_page).await;
    assert_eq!(page_after.owner, ACCELERATOR_PROGRAM);
    assert_eq!(
        page_after.lamports, page_before.lamports,
        "OpenBatch writes its accelerator page without lifecycle funding"
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
    let decoded_batch = GeneralBatchV2::decode(local.body()).expect("Batch");
    let occurrence = GeneralBatchOccurrenceTermsV1::new(decoded_batch.opening())
        .expect("Batch occurrence")
        .occurrence_id();
    assert_eq!(
        local.header().bump,
        Pubkey::find_program_address(
            dclutch_trading::general::state_seeds_v3::GeneralStateAddressSeedsV3::batch(
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
    let output_page_signer = Keypair::new();
    let mut test = waist::program_test_without_forced_budget(&elves);
    let releases = waist::add_release_waist_v2(&mut test, &elves, substrate);
    waist::add_program_v2(
        &mut test,
        "dclutch_accelerator_sbf",
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
        &genesis_prestate(&campaign, output_page_signer.pubkey()),
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
fn decode_batch(account: &Account) -> (GeneralLocalStateV3<'_>, GeneralBatchV2) {
    let envelope = GeneralLocalStateV3::decode(&account.data).expect("local Batch envelope");
    let batch = GeneralBatchV2::decode(envelope.body()).expect("Batch");
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
/// (`GeneralBatchV2::close_is_permissionless`). Neither can be run first and
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
    let outcome_count: u32 = env::var("DCLUTCH_GENERAL_OUTCOMES")
        .map(|value| value.parse().expect("outcome count is a u32"))
        .unwrap_or(2);
    let participant_seed: u8 = env::var("DCLUTCH_GENERAL_PARTICIPANT_SEED")
        .map(|value| value.parse().expect("participant seed is a u8"))
        .unwrap_or(0);
    let place_only = env::var("DCLUTCH_GENERAL_PLACE_ONLY").as_deref() == Ok("1");
    let order_side = match env::var("DCLUTCH_GENERAL_ORDER_SIDE").as_deref() {
        Ok("Sell") => OrderSideV2::Sell,
        Ok("Buy") | Err(_) => OrderSideV2::Buy,
        Ok(_) => panic!("order side must be Buy or Sell"),
    };
    assert!(
        order_side == OrderSideV2::Buy || place_only,
        "the standalone Sell control stops after actual Place poststates; the continued solver corpus is a Buy"
    );
    let participant = |tag: u8| {
        Keypair::new_from_array(
            [tag.checked_add(participant_seed)
                .expect("participant seed plus role tag fits u8"); 32],
        )
    };
    eprintln!(
        "general-campaign parameters outcomes={outcome_count} side={order_side:?} participant_seed={participant_seed} place_only={place_only}"
    );
    let substrate = waist::fixture_substrate();
    let elves = waist::elves();
    let accelerator_elf = load_accelerator_elf();
    let token_2022_elf = load_token_2022_elf();
    let rent_elf = load_rent_elf();
    let rent = Rent::default();
    let payer = participant(0x11);
    let fee_payer = participant(0x12);
    let seal_payer = participant(0x13);
    // THE SOLVER IS NOT THE MARKET'S SPONSOR, AND THAT IS THE CONTROL.
    //
    // `payer` is the wallet the founding wrote into the RentCredit's
    // `RefundAuthority`, so a candidate submitted BY `payer` would satisfy
    // `identity::PRIMARY_BENEFICIARY == solver_id` under either refund
    // declaration -- decision 0021 records exactly that coincidence ("it passed
    // for as long as the campaign staged that credit with the LP owner's own
    // key") as the shape that hid the Dealer defect for a family. A fourth
    // keypair makes the two answers different values, so this execution
    // distinguishes `Payer` from `Credit` instead of agreeing with both.
    let solver = participant(0x14);
    let output_page_signer = participant(0x15);
    let mut test = waist::program_test_without_forced_budget(&elves);
    waist::add_program_v2(
        &mut test,
        "spl_token_2022",
        GENERAL_TOKEN_PROGRAM,
        &token_2022_elf,
        substrate,
    );
    waist::add_program_v2(
        &mut test,
        "dclutch_rent_sbf",
        waist::RENT_PROGRAM_ID,
        &rent_elf,
        substrate,
    );
    let releases = waist::add_release_waist_v2(&mut test, &elves, substrate);
    waist::add_program_v2(
        &mut test,
        "dclutch_accelerator_sbf",
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
    let mut opening_chain = genesis_prestate(&campaign, output_page_signer.pubkey());
    let fixture_open = build_action_case(
        &campaign,
        Action::OpenBatch,
        &opening_chain,
        substrate.bank_slot(),
        fee_payer.pubkey(),
    );
    let output_page = fixture_open
        .built
        .admitted_authorities
        .output_page
        .expect("General's selected OutputPageV3 strategy derives its one page");
    assert_eq!(fixture_open.built.admitted_authorities.entries.len(), 1);
    add_case_accounts(&mut test, &fixture_open, &payer, &fee_payer);
    for funded in [&seal_payer, &solver] {
        test.add_account(
            funded.pubkey(),
            Account {
                lamports: GENESIS_PAYER_LAMPORTS,
                data: Vec::new(),
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            },
        );
    }
    waist::add_lookup_table(&mut test, &fixture_open.lookup_addresses);
    let mut context = waist::start_with_substrate(test, substrate).await;
    opening_chain.output_page = provision_output_page(
        &mut context,
        &campaign,
        &payer,
        &fee_payer,
        &output_page_signer,
    )
    .await;
    opening_chain.payer = observed_binding(&mut context, payer.pubkey()).await;
    let open = build_action_case(
        &campaign,
        Action::OpenBatch,
        &opening_chain,
        substrate.bank_slot(),
        fee_payer.pubkey(),
    );
    assert_eq!(
        open.lookup_addresses, fixture_open.lookup_addresses,
        "finalized page observation must not change the routed account set"
    );
    let output_page_before_open = chain_account(&mut context, output_page).await;
    assert_eq!(output_page_before_open.owner, ACCELERATOR_PROGRAM);
    assert!(!output_page_before_open.executable);

    // ---- ACTION ONE: OpenBatch ------------------------------------------
    assert_frame_control(&mut context, &open).await;
    native_export::export_before_submit(&mut context, &campaign, &open, fee_payer.pubkey()).await;
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
    assert_eq!(
        open_execution
            .logs
            .iter()
            .filter(|line| line.contains(&format!("Program {ACCELERATOR_PROGRAM} invoke")))
            .count(),
        1,
        "OpenBatch sends its complete bank to the one selected output page"
    );
    let output_page_after_open = chain_account(&mut context, output_page).await;
    assert_eq!(output_page_after_open.owner, ACCELERATOR_PROGRAM);
    assert_eq!(
        output_page_after_open.lamports, output_page_before_open.lamports,
        "OpenBatch neither funds nor closes the durable accelerator page"
    );
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
    let mut chain = ChainPrestateV1 {
        market: observed_binding(&mut context, campaign.state.market.key).await,
        root: observed_binding(&mut context, open.root).await,
        rent_credit: observed_binding(&mut context, open.rent_credit).await,
        payer: observed_binding(&mut context, payer.pubkey()).await,
        output_page: observed_binding(&mut context, output_page).await,
        primary_state: Some(observed_binding(&mut context, open.primary_state).await),
    };

    // ---- THE ORDER HALF, PROBED WHILE THE BATCH IS STILL COLLECTING ----
    //
    // `PlaceOrder` is the one action whose readonly-evidence table holds a
    // single record -- `OrderTerms`, the exact immutable bytes a maker signs --
    // and it is the only action reachable in a Collecting batch, so this is
    // where the escrow half of the fifteen begins. The terms are PROJECTED out
    // of a canonical order record rather than typed: `encode_signed_terms_into`
    // omits the mutable escrow window and re-derives the identity, so a record
    // and the bytes its maker signed cannot disagree here by construction.
    //
    // The maker is the campaign's own `payer`, because the transition carries
    // `identity_eq(PAYER, OWNER)`: whoever signs the placement IS the maker the
    // record names, exactly as SubmitCandidate's solver is the account that
    // funds the candidate.
    // ONE CLAIM AT ONE OUTCOME, ON ONE SIDE. Cohort-18's joint arm makes an
    // order an INTERVAL: it moves `claims_per_lot` claims at every outcome of
    // `[outcome_lo, outcome_hi]` and nothing off it, and the record's per-lot
    // vectors must be the ones that shape derives (`RowsDisagreeWithShape`).
    // A single-outcome buy of one claim per lot is the smallest order
    // `GeneralOrderV2::decode` admits: `validate_shape` refuses a
    // `claims_per_lot` of zero, so a record that moves no claim in either
    // direction cannot be encoded at all.
    let order_header = GeneralOrderHeaderV2 {
        outcome_count,
        nonce: 1,
        owner_id: payer.pubkey().to_bytes(),
        market: campaign.state.market.key.to_bytes(),
        batch_id: opened_batch.batch_id(),
        generation: GENERATION,
        max_lots: 1,
        // Quote atoms per lot: one unit claim has at most one atom of payoff.
        max_quote_debit_per_lot: u64::from(order_side == OrderSideV2::Buy),
        min_quote_credit_per_lot: u64::from(order_side == OrderSideV2::Sell),
        valid_until_slot: opened_batch.opening().settlement_close_slot,
        side: order_side,
        outcome_lo: 0,
        outcome_hi: 0,
        claims_per_lot: 1,
    };
    let derived_rows: Vec<(u64, u64)> = (0..outcome_count)
        .map(|outcome| order_header.derived_row(outcome))
        .collect();
    let receive_per_lot: Vec<u64> = derived_rows.iter().map(|row| row.0).collect();
    let deliver_per_lot: Vec<u64> = derived_rows.iter().map(|row| row.1).collect();
    let mut order_bytes = vec![0_u8; general_order_len_v2(outcome_count).expect("order width")];
    GeneralOrderV2::encode_into(
        order_header,
        &receive_per_lot,
        &deliver_per_lot,
        GeneralOrderStateV1 {
            phase: GeneralOrderPhaseV1::Placed,
            admitted_slot: substrate.bank_slot(),
            released_slot: 0,
        },
        &mut order_bytes,
    )
    .expect("canonical maker order against the open batch");
    let order_record = GeneralOrderV2::decode(&order_bytes).expect("order record");
    let mut signed_terms =
        vec![0_u8; general_signed_order_terms_len_v2(outcome_count).expect("terms width")];
    order_record
        .encode_signed_terms_into(&mut signed_terms)
        .expect("the signed projection keeps the record identity");

    let place_evidence = EvidenceCorpusV1 {
        order_terms: Some(staged_record(&campaign.rent, signed_terms.clone())),
        ..EvidenceCorpusV1::default()
    };
    // THE CORPUS IS THE SHAPE THE ACTION READS, asserted before the build, so a
    // refusal below cannot be a malformed record wearing a register's name.
    general_action_prestate_shape_v1(
        Action::PlaceOrder,
        GeneralActionPrestateV1 {
            primary_state_account: Some(built_bytes(
                chain.primary_state.as_ref().expect("the open batch"),
            )),
            evidence: place_evidence.projector(),
        },
    )
    .expect("the campaign's order corpus is the shape PlaceOrder reads");

    // ---- ACTION TWO: PlaceOrder -----------------------------------------
    //
    // The additional bindings are the canonical Claims and Custody corpus for
    // this order.  They are not width-shaped stand-ins: the existing market
    // aggregate, maker position, Realm, mint, and token account are encoded by
    // their owners, while the escrow Position, admission, replay, and vault are
    // vacant at the child contracts' PDAs.  The action therefore has to create
    // the exact child state it later spends.
    let quote_reserve = order_record.quote_reserve().expect("exact quote reserve");
    let place_corpus =
        place_order_corpus(&campaign, &chain, order_record.order_id(), quote_reserve);
    assert_eq!(
        place_corpus.order_identity.key.to_bytes(),
        order_record.order_id(),
        "the Custody escrow context is the signed order identity"
    );
    let expected_position_rent = campaign.rent.minimum_balance(
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2
            + usize::try_from(campaign.outcome_count).expect("outcome width") * 8,
    );
    let expected_admission_rent = campaign
        .rent
        .minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2);
    for (name, account, rent_floor) in [
        (
            "Claims escrow Position",
            &place_corpus.escrow_position,
            expected_position_rent,
        ),
        (
            "Claims escrow admission",
            &place_corpus.escrow_admission,
            expected_admission_rent,
        ),
    ] {
        assert_eq!(
            account.account.owner,
            system_program::ID,
            "{name} is vacant"
        );
        assert!(
            account.account.data.is_empty(),
            "{name} has no preallocated body"
        );
        assert_eq!(
            account.account.lamports, rent_floor,
            "{name} is prepaid to the exact Claims allocation width"
        );
    }
    let observed_rent = observed_binding(&mut context, sysvar::rent::ID).await;
    let place_bindings = observe_existing_place_order_child_bindings(
        &mut context,
        place_order_child_bindings(
            &campaign,
            &chain,
            &place_corpus,
            place_evidence
                .order_terms
                .as_ref()
                .expect("signed order terms"),
            &observed_rent,
        ),
    )
    .await;
    let place = build_action_case_with_evidence_and_bindings(
        &campaign,
        Action::PlaceOrder,
        &chain,
        substrate.bank_slot(),
        fee_payer.pubkey(),
        &place_evidence,
        &place_bindings,
    )
    .expect("PlaceOrder assembles against its Claims and Custody corpus");
    assert_eq!(
        place.built.admitted_authorities.output_page,
        Some(output_page),
        "PlaceOrder reuses the founded market's accelerator page"
    );
    assert_eq!(place.built.admitted_authorities.entries.len(), 1);
    for invocation in &place.built.bundle.engine.invocations {
        if invocation.resolved.role == FixedRole::Claims
            && invocation.request.get(..8)
                == Some(
                    dclutch_claims::protocol_position_v2::PROTOCOL_POSITION_REQUEST_MAGIC_V2
                        .as_slice(),
                )
        {
            let request = dclutch_claims::protocol_position_v2::ProtocolPositionRequestV2::decode(
                &invocation.request,
            )
            .expect("canonical projected Claims admission");
            let observed =
                LiabilityBasisMarketViewV2::decode(&place_corpus.claims_market.account.data)
                    .expect("actual nonzero Claims revision");
            assert_ne!(
                observed.revision, 0,
                "this control catches default-zero projection"
            );
            assert_eq!(request.expected_market_revision, observed.revision);
        }
        if invocation.resolved.role == FixedRole::Custody {
            let delegated = (invocation.request.get(..8)
                == Some(DELEGATED_CUSTODY_REQUEST_MAGIC_V2.as_slice()))
            .then(|| {
                DelegatedCustodyRequestV2::decode(&invocation.request)
                    .expect("PlaceOrder delegated Custody request")
            });
            let request = delegated.map(|value| value.custody).unwrap_or_else(|| {
                CustodyRequestV1::decode(&invocation.request).unwrap_or_else(|error| {
                let zero_required = [
                    ("release-set", CustodyRequestLayoutV1::RELEASE_SET),
                    ("market", CustodyRequestLayoutV1::MARKET),
                    ("realm", CustodyRequestLayoutV1::REALM),
                    ("context", CustodyRequestLayoutV1::CONTEXT),
                    ("caller-program", CustodyRequestLayoutV1::CALLER_PROGRAM),
                    (
                        "parent-request-digest",
                        CustodyRequestLayoutV1::PARENT_REQUEST_DIGEST,
                    ),
                ]
                .into_iter()
                .filter_map(|(name, offset)| {
                    invocation
                        .request
                        .get(offset..offset + 32)
                        .is_some_and(|bytes| bytes.iter().all(|byte| *byte == 0))
                        .then_some(name)
                })
                .collect::<Vec<_>>();
                let byte = |offset| invocation.request.get(offset).copied();
                let u64_at = |offset| {
                    invocation
                        .request
                        .get(offset..offset + 8)
                        .and_then(|bytes| bytes.try_into().ok())
                        .map(u64::from_le_bytes)
                };
                let zero_identity = |offset| {
                    invocation
                        .request
                        .get(offset..offset + 32)
                        .is_some_and(|bytes| bytes.iter().all(|byte| *byte == 0))
                };
                panic!(
                    "PlaceOrder projected a noncanonical Custody request at route {}: {error:?}; zero required identities: {zero_required:?}; operation={:?} source-compartment={:?} destination-compartment={:?} expected={:?} resulting={:?} amount={:?} rent={:?} zero source-owner={} destination-owner={} source={} destination={} source-context={} destination-context={} mint={} token-program={} payer={} rent-refund={}",
                    invocation.route,
                    byte(CustodyRequestLayoutV1::OPERATION),
                    byte(CustodyRequestLayoutV1::SOURCE_COMPARTMENT),
                    byte(CustodyRequestLayoutV1::DESTINATION_COMPARTMENT),
                    u64_at(CustodyRequestLayoutV1::EXPECTED_REVISION),
                    u64_at(CustodyRequestLayoutV1::RESULTING_REVISION),
                    u64_at(CustodyRequestLayoutV1::AMOUNT),
                    u64_at(CustodyRequestLayoutV1::RENT_LAMPORTS),
                    zero_identity(CustodyRequestLayoutV1::SOURCE_OWNER),
                    zero_identity(CustodyRequestLayoutV1::DESTINATION_OWNER),
                    zero_identity(CustodyRequestLayoutV1::SOURCE),
                    zero_identity(CustodyRequestLayoutV1::DESTINATION),
                    zero_identity(CustodyRequestLayoutV1::SOURCE_VAULT_CONTEXT),
                    zero_identity(CustodyRequestLayoutV1::DESTINATION_VAULT_CONTEXT),
                    zero_identity(CustodyRequestLayoutV1::MINT),
                    zero_identity(CustodyRequestLayoutV1::TOKEN_PROGRAM),
                    zero_identity(CustodyRequestLayoutV1::PAYER),
                    zero_identity(CustodyRequestLayoutV1::RENT_REFUND),
                )
                })
            });
            assert_eq!(
                request.realm, campaign.realm.digest,
                "every Custody child carries the authenticated Realm content identity"
            );
            assert_eq!(
                request.context,
                place_corpus.order_identity.key.to_bytes(),
                "every PlaceOrder Custody child is keyed by the signed order identity"
            );
            if request.operation == OperationV1::OpenVault {
                assert_eq!(
                    request.destination_vault_context,
                    place_corpus.order_identity.key.to_bytes(),
                    "the opened Settlement vault shares the order context"
                );
            }
            if request.source_compartment == CompartmentV1::External {
                let delegated = delegated.expect("an external debit uses delegated Custody");
                assert!(delegated.starts_atomic_debit);
                assert!(delegated.terminal);
                assert_eq!(
                    delegated.delegate_before,
                    place_corpus.custody_authority.key.to_bytes()
                );
                assert_eq!(delegated.delegate_after, [0; 32]);
                assert_eq!(delegated.total_debit, quote_reserve);
                assert_eq!(delegated.allowance_before, quote_reserve);
                assert_eq!(delegated.allowance_after, 0);
            }
        }
    }
    let place_installed =
        install_absent(&mut context, &place, &[place.built.bundle.artifacts.seal]).await;
    let (place_seal, place_seal_cu) =
        produce_seal(&mut context, &place, &seal_payer, &fee_payer).await;
    assert_eq!(place_seal, place.built.bundle.artifacts.seal);
    waist::set_lookup_table(&mut context, &place.lookup_addresses);
    assert_frame_control(&mut context, &place).await;
    let place_execution = waist::submit_v0_observed(
        &mut context,
        &place.instructions,
        place.lookup_addresses.clone(),
        Some(&fee_payer),
        &[&payer],
    )
    .await
    .expect("real Trading -> General accelerator PlaceOrder");
    eprintln!(
        "general-campaign place-order cu={} headroom={} outcomes={} side={:?} lots={} quote_reserve={} participant_seed={participant_seed}",
        place_execution.compute_units_consumed,
        waist::COMPUTE_LIMIT
            .checked_sub(place_execution.compute_units_consumed)
            .expect("chain CU limit"),
        outcome_count,
        order_side,
        order_record.header().max_lots,
        quote_reserve,
    );
    assert!(
        place_execution
            .logs
            .iter()
            .any(|line| line.contains(&format!("Program {ACCELERATOR_PROGRAM} invoke"))),
        "the real accelerator CPI ran for PlaceOrder"
    );
    assert_eq!(
        place_execution
            .logs
            .iter()
            .filter(|line| line.contains(&format!("Program {ACCELERATOR_PROGRAM} invoke")))
            .count(),
        1,
        "PlaceOrder invokes the accelerator once for its whole bank"
    );
    let output_page_after_place = chain_account(&mut context, output_page).await;
    assert_eq!(output_page_after_place.owner, ACCELERATOR_PROGRAM);
    assert_eq!(
        output_page_after_place.lamports, output_page_before_open.lamports,
        "PlaceOrder writes the existing page without charging its lifecycle credit"
    );
    let placed_batch_account = chain_account(&mut context, open.primary_state).await;
    let (_, placed_batch) = decode_batch(&placed_batch_account);
    assert_eq!(placed_batch.state().status, BatchStatusV1::Collecting);
    assert_eq!(placed_batch.state().order_count, 1);
    assert_eq!(placed_batch.live_order_count(), 1);
    let order_state = place
        .built
        .bundle
        .logical
        .get(usize::from(GENERAL_TERMINAL_STATE_ACCOUNT_V3))
        .expect("PlaceOrder terminal state coordinate")
        .key;
    let placed_order_account = chain_account(&mut context, order_state).await;
    assert_eq!(
        root_tail_of(&chain_account(&mut context, open.root).await).revision(),
        opened_root.revision(),
        "PlaceOrder admits into the Batch without advancing the root revision"
    );
    let order_envelope = GeneralLocalStateV3::decode(&placed_order_account.data)
        .expect("the Placement materialized its General Order");
    assert_eq!(order_envelope.header().kind, GeneralLocalStateKindV3::Order);
    let placed_order = GeneralOrderV2::decode(order_envelope.body())
        .expect("the terminal local-state body is the signed General Order");
    assert_eq!(placed_order.header(), order_record.header());
    assert_eq!(placed_order.state(), order_record.state());
    assert_eq!(placed_order.order_id(), order_record.order_id());
    for (name, account) in [
        ("Claims escrow Position", &place_corpus.escrow_position),
        ("Claims escrow admission", &place_corpus.escrow_admission),
        ("Custody replay", &place_corpus.escrow_replay),
        ("Custody vault", &place_corpus.escrow_vault),
    ] {
        assert_ne!(
            chain_account(&mut context, account.key).await,
            absent_account(),
            "PlaceOrder did not create {name}"
        );
    }
    let aggregate_account = chain_account(&mut context, place_corpus.claims_market.key).await;
    let aggregate = LiabilityBasisMarketViewV2::decode(&aggregate_account.data)
        .expect("PlaceOrder preserves the canonical Claims aggregate");
    let claim_transfer = u64::from(order_side == OrderSideV2::Sell);
    assert_eq!(aggregate.revision, 1 + claim_transfer);
    for outcome in 0..outcome_count {
        assert_eq!(
            aggregate
                .supply(&aggregate_account.data, outcome)
                .expect("aggregate supply"),
            4,
            "admission preserves aggregate claim supply"
        );
    }
    let maker_position_account = chain_account(&mut context, place_corpus.maker_position.key).await;
    let maker_position = LiabilityBasisPositionViewV2::decode(&maker_position_account.data)
        .expect("canonical maker Position");
    assert_eq!(maker_position.revision, 1 + claim_transfer);
    for outcome in 0..outcome_count {
        assert_eq!(
            maker_position
                .balance(&maker_position_account.data, outcome)
                .expect("maker balance"),
            4 - order_header.derived_row(outcome).1,
            "admission escrows the exact signed delivery from the maker"
        );
    }
    let escrow_position_account =
        chain_account(&mut context, place_corpus.escrow_position.key).await;
    let escrow_position = LiabilityBasisPositionViewV2::decode(&escrow_position_account.data)
        .expect("canonical admitted escrow Position");
    assert_eq!(escrow_position.revision, claim_transfer);
    assert_eq!(escrow_position.owner, order_state.to_bytes());
    for outcome in 0..outcome_count {
        assert_eq!(
            escrow_position
                .balance(&escrow_position_account.data, outcome)
                .expect("escrow balance"),
            order_header.derived_row(outcome).1,
            "admission escrows the exact signed delivery"
        );
    }
    let admission_account = chain_account(&mut context, place_corpus.escrow_admission.key).await;
    let admission = ProtocolPositionAdmissionV2::decode(&admission_account.data)
        .expect("canonical persisted escrow admission");
    assert_eq!(admission.position_owner(), order_state.to_bytes());
    assert_eq!(
        admission.market_revision(),
        1,
        "admission binds the pre-transfer Market revision"
    );
    assert_eq!(admission.outcome_count(), outcome_count);
    assert_eq!(
        token_account_amount(
            &chain_account(&mut context, place_corpus.escrow_vault.key)
                .await
                .data,
        ),
        quote_reserve,
        "Custody holds the exact signed quote reserve"
    );
    let maker_token_after = chain_account(&mut context, place_corpus.maker_token.key).await;
    assert_eq!(
        token_account_amount(&maker_token_after.data),
        0,
        "the one funded maker source paid exactly the quote reserve"
    );
    assert_eq!(
        token_account_delegate_tag(&maker_token_after.data),
        u32::from(quote_reserve == 0),
        "a terminal external debit revokes its delegate; an inactive debit leaves it untouched"
    );
    assert_eq!(
        token_account_delegated_amount(&maker_token_after.data),
        0,
        "the terminal external debit exhausts its exact allowance"
    );
    eprintln!(
        "general-campaign place-poststates verified outcomes={outcome_count} side={order_side:?} participant_seed={participant_seed}"
    );
    if place_only {
        return;
    }
    chain = ChainPrestateV1 {
        market: observed_binding(&mut context, campaign.state.market.key).await,
        root: observed_binding(&mut context, open.root).await,
        rent_credit: observed_binding(&mut context, open.rent_credit).await,
        payer: observed_binding(&mut context, payer.pubkey()).await,
        output_page: observed_binding(&mut context, output_page).await,
        primary_state: Some(observed_binding(&mut context, open.primary_state).await),
    };

    // THE WINDOW IS THE PROTOCOL'S, NOT THE HARNESS'S. The order leaves this
    // batch live but not full, so the config-derived collection window has to
    // elapse and the campaign warps to exactly the slot the Batch itself names.
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

    // ---- ACTION THREE: CloseBatch ---------------------------------------
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
    assert_eq!(
        close.built.admitted_authorities.output_page,
        Some(output_page),
        "CloseBatch uses the same durable accelerator page"
    );
    assert_eq!(close.built.admitted_authorities.entries.len(), 1);
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
    assert_eq!(
        close_execution
            .logs
            .iter()
            .filter(|line| line.contains(&format!("Program {ACCELERATOR_PROGRAM} invoke")))
            .count(),
        1,
        "CloseBatch invokes the accelerator once for its whole bank"
    );
    let output_page_after_close = chain_account(&mut context, output_page).await;
    assert_eq!(output_page_after_close.owner, ACCELERATOR_PROGRAM);
    assert_eq!(
        output_page_after_close.lamports, output_page_before_open.lamports,
        "CloseBatch neither refunds nor closes the durable accelerator page"
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
    // decodes the batch the bank now holds and `GeneralBatchV2::close` refuses a
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

    // ---- ACTION FOUR: A SECOND BATCH ON THE SAME MARKET ------------------
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
        output_page: observed_binding(&mut context, output_page).await,
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

    // ---- ACTION FIVE: A CANDIDATE SUBMITTED AGAINST THE CLOSED BATCH ----
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
    // submitted against a batch that was opened, received this campaign's
    // escrowed order, and closed, at an identity no line here types.
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
    // With one Buy and no seller, this candidate enumerates the order at zero
    // fill and its marginal limit. Prices sum to the market's exact scale;
    // the native control refuses the former uniform-price candidate because
    // it would ration an order strictly inside its limit.
    let prices = verify_continuation::single_buy_zero_fill_prices(
        order_envelope.body(),
        config.price_scale(),
    );
    assert_eq!(prices.iter().sum::<u64>(), config.price_scale());
    let draft = CandidateHeaderV2 {
        outcome_count,
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
        // The batch's own count, never a literal: `authenticate_batch_candidate_v1`
        // refuses a candidate that disagrees with the batch about how many
        // orders are live, and this batch closed holding the one order this
        // walk placed.
        live_order_count: closed_batch.live_order_count(),
    };
    let mut candidate_image = vec![0_u8; candidate_len(outcome_count).expect("candidate width")];
    CandidateV2::encode_into(draft, &prices, &mut candidate_image).expect("draft candidate");
    let candidate_id = general_candidate_identity_v1(&candidate_image).expect("candidate identity");
    CandidateV2::encode_into(
        CandidateHeaderV2 {
            candidate_id,
            ..draft
        },
        &prices,
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
        outcome_count,
        page_count: 1,
        page_revision: CANDIDATE_PAGE_REVISION,
        submitted_slot,
        candidate_id,
        batch_id: closed_batch.batch_id(),
        // THE SOLVER IS THE ACCOUNT THAT PAYS, and this line is the whole of
        // that law on the campaign's side. The transition carries
        // `identity_eq(PAYER, OWNER)` and the projector joins the lifecycle's
        // beneficiary to the record's solver; both are satisfied by the payer
        // this action is bound to, and by nothing else.
        solver_id: solver.pubkey().to_bytes(),
        row_count: 1,
        reward_rate_lamports: CRANK_REWARD_LAMPORTS,
    };
    let submission = GeneralCandidateV1::submit(
        closed_batch,
        decoded_candidate,
        CANDIDATE_PAGE_REVISION,
        1,
        CRANK_REWARD_LAMPORTS,
        solver.pubkey().to_bytes(),
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
        ..EvidenceCorpusV1::default()
    };
    let submit_chain = ChainPrestateV1 {
        market: observed_binding(&mut context, campaign.state.market.key).await,
        root: observed_binding(&mut context, open.root).await,
        rent_credit: observed_binding(&mut context, open.rent_credit).await,
        // The account the profile debits for this action is the SOLVER, not the
        // campaign's sponsor. `GENERAL_PRIMARY_PAYER_ACCOUNT_V3` is a per-action
        // binding, so an action whose state belongs to one participant binds
        // that participant here.
        payer: observed_binding(&mut context, solver.pubkey()).await,
        output_page: observed_binding(&mut context, output_page).await,
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

    // ---- AND IT EXECUTES. THE WALL WAS TWO REGISTERS AND ONE PLAN BYTE ----
    //
    // `44c0ccf19` measured the refusal and named the two coordinates the input
    // bank did not carry: `identity::CANDIDATE`, which was thirty-two zero
    // bytes, and `identity::PRIMARY_BENEFICIARY`, which "held the lifecycle's
    // own beneficiary rather than the solver". They have DIFFERENT producers
    // and only one of them is the AccountProfile's, which is why the pair could
    // not be closed by one edit:
    //
    // * `identity::CANDIDATE` is a profile projection and now is one --
    //   `account_rules_v3.rs`, SubmitCandidate operation 33, out of the
    //   authenticated CandidateImage. It is the register
    //   `GENERAL_CANDIDATE_STATE_RECIPE_V3` seeds the state address on, so
    //   until it had a source every candidate under one root derived the SAME
    //   address from thirty-two zero bytes.
    // * `identity::PRIMARY_BENEFICIARY` is a lifecycle PROTECTED OUTPUT, and
    //   `validate_protected_outputs_against_profile` refuses `ProfileMismatch`
    //   for any policy whose profile writes one. The AccountProfile is
    //   forbidden from projecting it; the lifecycle preplan writes it, and the
    //   value is decided by decision 0021's byte five. `Credit` writes the
    //   market's RentCredit wallet, which is why the register held what it
    //   held. The Candidate recipe declares `Payer` now, so the preplan writes
    //   `*payer.key`, and the projector's conjunct becomes the check it reads
    //   as: the account that funded this candidate IS the solver the record
    //   names.
    let submit = build_action_case_with_evidence(
        &campaign,
        Action::SubmitCandidate,
        &submit_chain,
        submitted_slot,
        fee_payer.pubkey(),
        &evidence,
    )
    .expect("the fifth General action assembles against the corpus this campaign wrote");
    assert_ne!(
        submit.built.bundle.artifacts.seal, close.built.bundle.artifacts.seal,
        "a capability seal is keyed by action; SubmitCandidate needs its own"
    );
    let submit_installed =
        install_absent(&mut context, &submit, &[submit.built.bundle.artifacts.seal]).await;
    let (submit_seal, submit_seal_cu) =
        produce_seal(&mut context, &submit, &seal_payer, &fee_payer).await;
    assert_eq!(submit_seal, submit.built.bundle.artifacts.seal);
    waist::set_lookup_table(&mut context, &submit.lookup_addresses);
    assert_frame_control(&mut context, &submit).await;
    let solver_before = chain_account(&mut context, solver.pubkey()).await.lamports;
    let sponsor_before_submit = chain_account(&mut context, payer.pubkey()).await.lamports;
    let submit_execution = waist::submit_v0_observed(
        &mut context,
        &submit.instructions,
        submit.lookup_addresses.clone(),
        Some(&fee_payer),
        &[&solver],
    )
    .await
    .expect("real Trading -> General accelerator SubmitCandidate with funded work escrow");
    assert!(
        submit_execution
            .logs
            .iter()
            .any(|line| line.contains(&format!("Program {ACCELERATOR_PROGRAM} invoke"))),
        "the real accelerator CPI ran for SubmitCandidate"
    );

    // The candidate is Trading-owned local state, created only after the
    // funding plan transfers its exact work capacity through System. Decode the
    // bank record rather than relying on the staged submission corpus: this is
    // the poststate that later Consider and verification actions consume.
    let candidate_account = chain_account(&mut context, submit.primary_state).await;
    assert_eq!(candidate_account.owner, waist::TRADING_PROGRAM_ID);
    let candidate_state = GeneralLocalStateV3::decode(&candidate_account.data)
        .expect("SubmitCandidate materialized a local Candidate");
    assert_eq!(
        candidate_state.header().kind,
        GeneralLocalStateKindV3::Candidate
    );
    let observed_submission = GeneralCandidateV1::decode(candidate_state.body())
        .expect("the Candidate local-state body is a submitted candidate");
    assert_eq!(observed_submission, submission);
    assert!(
        chain_account(&mut context, solver.pubkey()).await.lamports < solver_before,
        "SubmitCandidate funds the candidate work escrow from its solver"
    );
    assert_eq!(
        chain_account(&mut context, payer.pubkey()).await.lamports,
        sponsor_before_submit,
        "SubmitCandidate does not debit the market sponsor"
    );

    eprintln!(
        "general-campaign N={outcome_count} market={} root={} batch={}",
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
        "general-campaign submit-candidate cu={} candidate={} batch={} solver={} \
         installed_records={submit_installed}",
        submit_execution.compute_units_consumed,
        hex32(candidate_id),
        hex32(closed_batch.batch_id()),
        hex32(solver.pubkey().to_bytes()),
    );
    eprintln!("general-campaign submit-candidate-seal cu={submit_seal_cu}");
    verify_continuation::verify_first_row(
        &mut context,
        &campaign,
        &submit,
        open.primary_state,
        order_state,
        output_page,
        &payer,
        &fee_payer,
        &seal_payer,
        &candidate_image,
    )
    .await;
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
    let output_page_signer = Keypair::new();
    let mut test = waist::program_test_without_forced_budget(&elves);
    let releases = waist::add_release_waist_v2(&mut test, &elves, substrate);
    waist::add_program_v2(
        &mut test,
        "dclutch_accelerator_sbf",
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
    let foreign_product = build_product(FOREIGN_WIDTH);
    let foreign_release = selected_release(
        FOREIGN_WIDTH,
        &foreign_product,
        accelerator_release,
        foreign_product.semantic_basis,
    );
    let foreign_entry =
        general_selected_entry_descriptor_v1(&foreign_release).expect("foreign entry descriptor");
    let native_product = build_product(OUTCOME_COUNT);
    let native_entry = general_selected_entry_descriptor_v1(&selected_release(
        OUTCOME_COUNT,
        &native_product,
        accelerator_release,
        native_product.semantic_basis,
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
        None,
    );
    let mut chain = genesis_prestate(&campaign, output_page_signer.pubkey());
    let fixture_case = build_action_case(
        &campaign,
        Action::OpenBatch,
        &chain,
        substrate.bank_slot(),
        fee_payer.pubkey(),
    );
    add_case_accounts(&mut test, &fixture_case, &payer, &fee_payer);
    waist::add_lookup_table(&mut test, &fixture_case.lookup_addresses);
    let mut context = waist::start_with_substrate(test, substrate).await;
    chain.output_page = provision_output_page(
        &mut context,
        &campaign,
        &payer,
        &fee_payer,
        &output_page_signer,
    )
    .await;
    chain.payer = observed_binding(&mut context, payer.pubkey()).await;
    let case = build_action_case(
        &campaign,
        Action::OpenBatch,
        &chain,
        substrate.bank_slot(),
        fee_payer.pubkey(),
    );
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

/// `TradingSbfError::Transition`, derived from its REGISTERED BAND.
const TRADING_TRANSITION: u32 = dclutch_refusal_registry::TRADING_REFUSAL_BASE + 0x004;

/// THE DEVNET WALL OF 2026-09-06, ON REAL ELFS, WITH THE BASIS AS THE ONLY
/// VARIABLE.
///
/// Cohort-16F drove the first `OpenBatch` ever to reach the accelerator on any
/// chain. It refused `0x4004 Transition` after 361,538 CU on the ack of a CPI
/// that had itself succeeded, and the accelerator's own `sol_log_data` printed
/// both sides of the conjunct it failed: `require_market` compared the config's
/// `ed7bfc75...` against the bank's `48a66eee...`, which are one live
/// Portfolio's `liability_basis_id` @128 and `claim_basis_id` @96. The General
/// AccountProfile projected @96; the operator's config derivation is @128.
///
/// This founds a market whose config binds the Portfolio's CLAIM basis -- which
/// is EXACTLY what every General fixture in this tree spelled before today,
/// including this one -- and it must now refuse. It is therefore both the
/// hostile and the standing red-proof: revert
/// `general_account_profile_operation_v3`'s basis operation to
/// `PORTFOLIO_CLAIM_BASIS_ID_OFFSET` and this test goes green while the
/// campaign above goes red, which is the state devnet found.
///
/// THE POSITIVE CONTROL IS THE CAMPAIGN ITSELF: the identical code path with
/// the config bound to the founding's semantic basis commits, four transactions
/// deep, in `one_founded_market_opens_and_then_closes_its_batch_in_one_bank`.
#[tokio::test]
async fn a_config_bound_to_the_portfolios_claim_basis_refuses_the_accelerators_market_conjunct() {
    const OUTCOME_COUNT: u32 = 2;
    let substrate = waist::fixture_substrate();
    let elves = waist::elves();
    let accelerator_elf = load_accelerator_elf();
    let rent = Rent::default();
    let payer = Keypair::new_from_array([0x11; 32]);
    let fee_payer = Keypair::new_from_array([0x12; 32]);
    let output_page_signer = Keypair::new();
    let mut test = waist::program_test_without_forced_budget(&elves);
    let releases = waist::add_release_waist_v2(&mut test, &elves, substrate);
    waist::add_program_v2(
        &mut test,
        "dclutch_accelerator_sbf",
        ACCELERATOR_PROGRAM,
        &accelerator_elf,
        substrate,
    );
    // THE TWO FIELDS MUST REALLY DIFFER on this founding, or the hostile founds
    // the market the campaign founds and refuses for some other reason.
    let product = build_product(OUTCOME_COUNT);
    assert_ne!(
        product.semantic_basis, CLAIM_BASIS,
        "this Product's liability and claim bases coincide, so nothing is under test"
    );
    let campaign = build_campaign_with_entry(
        OUTCOME_COUNT,
        payer.pubkey(),
        rent,
        substrate,
        &elves,
        releases,
        &accelerator_elf,
        None,
        Some(CLAIM_BASIS),
    );
    let mut chain = genesis_prestate(&campaign, output_page_signer.pubkey());
    let fixture_case = build_action_case(
        &campaign,
        Action::OpenBatch,
        &chain,
        substrate.bank_slot(),
        fee_payer.pubkey(),
    );
    add_case_accounts(&mut test, &fixture_case, &payer, &fee_payer);
    waist::add_lookup_table(&mut test, &fixture_case.lookup_addresses);
    let mut context = waist::start_with_substrate(test, substrate).await;
    chain.output_page = provision_output_page(
        &mut context,
        &campaign,
        &payer,
        &fee_payer,
        &output_page_signer,
    )
    .await;
    chain.payer = observed_binding(&mut context, payer.pubkey()).await;
    let case = build_action_case(
        &campaign,
        Action::OpenBatch,
        &chain,
        substrate.bank_slot(),
        fee_payer.pubkey(),
    );
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
            "a config bound to the claim basis opened a batch, at {} CU",
            execution.compute_units_consumed
        ),
        Err(value) => value,
    };
    assert_eq!(
        refusal_code(&refused.error),
        Some(TRADING_TRANSITION),
        "the disagreeing basis refused with the wrong code: {:#?}",
        refused.logs,
    );
    // THE CODE IS ONE ACCUSATION OVER MANY CONJUNCTS, so the code alone does
    // not name this cause. The accelerator prints both sides before it refuses
    // and the CPI itself SUCCEEDS -- Trading refuses on the ack -- which is the
    // exact phase devnet reported. Both are asserted, or `0x4004` raised
    // anywhere earlier in the transition would pass for this.
    assert!(
        refused
            .logs
            .iter()
            .any(|line| line.contains("general: config/bank semantic basis")),
        "the accelerator did not reach `require_market`: {:#?}",
        refused.logs,
    );
    assert!(
        refused
            .logs
            .iter()
            .any(|line| line.contains(&format!("Program {ACCELERATOR_PROGRAM} success"))),
        "the refusal is not the accelerator's ack: {:#?}",
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
        "general-campaign claim-basis-config cu={} code=0x{TRADING_TRANSITION:04X}",
        refused.compute_units_consumed,
    );
}
