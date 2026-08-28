//! Real-SVM continuity from pre-Market Resolution funding through CreateFund.
//!
//! This focused fixture keeps the three authority boundaries separate:
//! a real Trading caller signs the universal CallerAuthority PDA, the current
//! Resolution deployment projects Core's exact Found37 frame and creates its
//! own Pending subset ledger, and ordinary Core Found later creates the Market.
//! V5 CreateFund must then create only Source state while preserving the
//! initializer-owned ledger byte-for-byte and lamport-for-lamport.

use std::{env, fs, path::PathBuf};

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    CapabilityEntryV1, CapabilityFundingLedgerDerivationV2, CapabilityManifestV1,
    CompartmentFundingV1, ContentId as CapabilityContentId, FUNDING_STATE_BYTES, FundingAmountsV1,
    FundingLedgerStatusV2, FundingLedgerV2, FundingQuoteV1, MANIFEST_HEADER_BYTES,
    MAX_DEPENDENCIES_PER_CAPABILITY, funding_ledger_bytes_v2,
};
use dclutch_core_contract::ContentId as CoreContentId;
use dclutch_market_core_codec::{
    Action, CoreState, Identity as CoreIdentity, MarketCoreStateSeedsV2, MarketIdentity, Phase,
    ProjectFoundRequestV2, Readiness, Request,
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
use dclutch_product_runtime_v2_admission::PRODUCT_RECORD_BYTES_V2;
use dclutch_product_runtime_v2_operator::{ProductCompilationInputV2, compile_product_records_v2};
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1, RealmV1Input,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, activate_execution_role_into_v1,
    initialize_activation_cache_v1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1, ProgramIdentityV1,
    ProtocolInfrastructureProfileV1,
};
use dclutch_rent_contract::{
    RefundAuthority,
    lifecycle_v2::{
        LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleAccountIdV2, LifecycleRentCreditV2,
    },
};
use dclutch_resolution_codec::{
    PRE_MARKET_FUNDING_RECEIPT_BYTES_V1, RESOLUTION_CONTROLLER_RELEASE_ID_V5,
};
use dclutch_resolution_core_v3_operator::{
    Finality, Observation, ObservedAccount, ResolutionCreateFundSnapshotV3,
    build_resolution_create_fund_v3,
    pre_market_funding_v1::{
        PRE_MARKET_FUNDING_ACCOUNT_COUNT_V1, PreMarketFundingSnapshotV1,
        authenticate_pre_market_funding_receipt_v1, build_pre_market_funding_v1,
    },
    validate_resolution_create_fund_report_v3,
};
use dclutch_source_contract::{
    CapacityEnvelope, ContentId as SourceContentId, RECOVERY_POLICY_SCHEMA_ID_V2,
    RecoveryAttemptV2, RecoveryPolicyV2, SOURCE_CAPACITY_PROFILE_SCHEMA_ID_V1,
    SOURCE_FAILURE_POLICY_RELEASE_ID_V2, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
    SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2, SOURCE_SPEC_SCHEMA_ID_V1, SourceAccessProfile,
    SourceCapacityProfileV1, SourceMaterialV3, SourceResolutionPhaseV1, SourceResolutionStateV2,
    SourceSpecV1,
};
use solana_account::Account;
use solana_program::{
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_system_interface::instruction::transfer;
use solana_transaction::Transaction;

const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x31; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x32; 32]);
const RENT_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x33; 32]);
const RESOLUTION_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x34; 32]);
const TRADING_CALLER_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x35; 32]);
const GENERATION: u64 = 11;
const BOUNTY: u64 = 7;

struct Elves {
    core: Vec<u8>,
    registry: Vec<u8>,
    rent: Vec<u8>,
    resolution: Vec<u8>,
    caller: Vec<u8>,
}

#[derive(Clone)]
struct Record {
    raw: Pubkey,
    staging: Pubkey,
    digest: [u8; 32],
    data: Vec<u8>,
}

struct Fixture {
    test: Option<ProgramTest>,
    found_payer: Keypair,
    market: Pubkey,
    rent_credit: Pubkey,
    realm: Record,
    product: Record,
    domain: Record,
    portfolio: Record,
    linked_basis: Record,
    material: Record,
    source_spec: Record,
    capacity: Record,
    recovery_allocation: [u8; 32],
    recovery: Record,
    manifest: Record,
    absent_floor_raw: Pubkey,
    absent_floor_staging: Pubkey,
    activation: Pubkey,
    core_programdata: Pubkey,
    registry_programdata: Pubkey,
    rent_programdata: Pubkey,
    resolution_programdata: Pubkey,
    caller_programdata: Pubkey,
    infrastructure: Pubkey,
    registry_artifact: Record,
    rent_artifact: Record,
    ledger: Pubkey,
    source: Pubkey,
}

fn content(bytes: [u8; 32]) -> CoreContentId {
    CoreContentId::new(bytes).expect("nonzero content identity")
}

fn product_id(byte: u8) -> ProductContentId {
    ProductContentId::new([byte; 32]).expect("Product identity")
}

fn source_id(byte: u8) -> SourceContentId {
    SourceContentId::new([byte; 32]).expect("Source identity")
}

fn program_identity(program: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(program.to_bytes()).expect("program identity")
}

fn artifacts() -> Elves {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    Elves {
        core: fs::read(directory.join("dclutch_core_sbf.so")).expect("Core ELF"),
        registry: fs::read(directory.join("dclutch_registry_sbf.so")).expect("Registry ELF"),
        rent: fs::read(directory.join("dclutch_rent_sbf.so")).expect("Rent ELF"),
        resolution: fs::read(directory.join("dclutch_resolution_proof_sbf.so"))
            .expect("Resolution ELF"),
        caller: fs::read(directory.join("dclutch_pre_market_funding_test_caller_sbf.so"))
            .expect("Trading caller ELF"),
    }
}

fn programdata(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn immutable_programdata(elf: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0_u8; 45 + elf.len()];
    bytes[0..4].copy_from_slice(&3_u32.to_le_bytes());
    bytes[4..12].copy_from_slice(&0_u64.to_le_bytes());
    bytes[12] = 0;
    bytes[45..].copy_from_slice(elf);
    bytes
}

fn add_program(test: &mut ProgramTest, name: &'static str, program: Pubkey, elf: &[u8]) {
    test.add_upgradeable_program_to_genesis(name, &program);
    let data = immutable_programdata(elf);
    test.add_account(
        programdata(program),
        Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner: bpf_loader_upgradeable::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn release(program: Pubkey, semantic: [u8; 32], elf: &[u8]) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        program_identity(program),
        program_identity(bpf_loader_upgradeable::ID),
        programdata(program).to_bytes(),
        content(semantic),
        hash(elf).to_bytes(),
        0,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("immutable release")
}

fn artifact_id(value: ArtifactReleaseV1) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(hash(&value.to_bytes()).to_bytes()).expect("artifact identity")
}

fn binding(value: ArtifactReleaseV1) -> ExecutionRoleBindingV1 {
    ExecutionRoleBindingV1::new(value.program(), artifact_id(value))
}

fn activation_input(value: ArtifactReleaseV1) -> ArtifactActivationInputV1 {
    ArtifactActivationInputV1::new(
        artifact_id(value),
        value,
        DeploymentObservationV1::new(
            value.program().to_bytes(),
            bpf_loader_upgradeable::ID.to_bytes(),
            true,
            value.programdata(),
            bpf_loader_upgradeable::ID.to_bytes(),
            false,
            value.programdata(),
            bpf_loader_upgradeable::ID.to_bytes(),
            value.deployment_slot(),
            value.elf_digest(),
            value.upgrade_authority(),
        )
        .expect("deployment observation"),
    )
}

impl Record {
    fn new(schema: [u8; 32], data: Vec<u8>) -> Self {
        let digest = hash(&data).to_bytes();
        let raw = Pubkey::find_program_address(
            &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
            &REGISTRY_PROGRAM_ID,
        )
        .0;
        let staging = Pubkey::find_program_address(
            &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
            &REGISTRY_PROGRAM_ID,
        )
        .0;
        Self {
            raw,
            staging,
            digest,
            data,
        }
    }

    fn add(&self, test: &mut ProgramTest) {
        test.add_account(
            self.raw,
            Account {
                lamports: Rent::default().minimum_balance(self.data.len()),
                data: self.data.clone(),
                owner: REGISTRY_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        );
    }
}

fn product_graph() -> (Record, Record, Record, Record, [u8; 32]) {
    let provisional = BasisInputV3 {
        kind: BasisKindV3::CategoricalQ1,
        product_id: product_id(1).to_bytes(),
        result_domain_id: [0x42; 32],
        coordinate_domain_id: product_id(2).to_bytes(),
        result_unit_id: product_id(3).to_bytes(),
        evaluator_release_id: [0x43; 32],
        basis_width: 258,
        payout_scale: 1,
        knot_denominator: 1,
        knots: &[],
        terms: &[],
        failure_payouts: &[],
    };
    let basis_width =
        basis_record_bytes_v3(BasisKindV3::CategoricalQ1, 258, 0, 0).expect("basis width");
    let mut provisional_bytes = vec![0_u8; basis_width];
    compile_basis_v3(provisional, &mut provisional_bytes).expect("provisional basis");
    let semantic = semantic_basis_preimage_v3(&provisional_bytes).expect("basis semantic");
    let liability_basis_id = ProductContentId::new(
        hashv(&[
            SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
            semantic.prefix(),
            semantic.suffix(),
        ])
        .to_bytes(),
    )
    .expect("liability basis identity");
    let cuts: Vec<i128> = (-128_i128..128).collect();
    let coefficients = vec![7_u64; cuts.len() + 2];
    let mut product = [0_u8; PRODUCT_RECORD_BYTES_V2];
    let mut domain = vec![0_u8; result_domain_record_bytes(cuts.len()).expect("domain width")];
    let mut portfolio =
        vec![0_u8; portfolio_record_bytes(coefficients.len()).expect("portfolio width")];
    let report = compile_product_records_v2(
        REGISTRY_PROGRAM_ID,
        ProductCompilationInputV2 {
            product_id: product_id(1),
            coordinate_domain_id: product_id(2),
            result_unit_id: product_id(3),
            claim_basis_id: product_id(4),
            liability_basis_id,
            representation_release_id: product_id(6),
            mapping_release_id: product_id(7),
            cut_denominator: 1,
            cuts: &cuts,
            portfolio_denominator: 9,
            coefficients: &coefficients,
        },
        &mut product,
        &mut domain,
        &mut portfolio,
    )
    .expect("Product graph");
    let mut linked_basis = vec![0_u8; basis_width];
    compile_basis_v3(
        BasisInputV3 {
            result_domain_id: report.receipt.result_domain.content_digest.to_bytes(),
            ..provisional
        },
        &mut linked_basis,
    )
    .expect("linked basis");
    (
        Record::new(
            report.receipt.product.schema_id.to_bytes(),
            product.to_vec(),
        ),
        Record::new(report.receipt.result_domain.schema_id.to_bytes(), domain),
        Record::new(report.receipt.portfolio.schema_id.to_bytes(), portfolio),
        Record::new(GRADED_BASIS_RECORD_SCHEMA_ID_V3, linked_basis),
        product_id(1).to_bytes(),
    )
}

fn manifest(recovery_allocation: [u8; 32], recovery: [u8; 32], material: [u8; 32]) -> Record {
    let none = CompartmentFundingV1::not_applicable();
    let rent = Rent::default().minimum_balance(FUNDING_STATE_BYTES);
    let quote = FundingQuoteV1::new(
        FundingAmountsV1::new(
            CompartmentFundingV1::native_lamports(rent).expect("funding rent"),
            none,
            none,
            none,
            CompartmentFundingV1::native_lamports(BOUNTY).expect("worker bounty"),
            none,
            none,
        )
        .expect("funding amounts"),
        None,
    )
    .expect("funding quote");
    let mut entries = [recovery_allocation, recovery, material].map(|config| {
        CapabilityEntryV1::new(
            content(hash(&config).to_bytes()),
            content(RESOLUTION_CONTROLLER_RELEASE_ID_V5),
            content(config),
            content([0x51; 32]),
            content([0x52; 32]),
            content([0x53; 32]),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            quote,
        )
        .expect("Resolution capability entry")
    });
    entries.sort_by_key(|entry| entry.kind_id().to_bytes());
    let mut bytes = vec![0_u8; MANIFEST_HEADER_BYTES + 3 * CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&entries, &mut bytes).expect("manifest");
    Record::new(CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, bytes)
}

fn fixture() -> Fixture {
    let elves = artifacts();
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    add_program(&mut test, "dclutch_core_sbf", CORE_PROGRAM_ID, &elves.core);
    add_program(
        &mut test,
        "dclutch_registry_sbf",
        REGISTRY_PROGRAM_ID,
        &elves.registry,
    );
    add_program(&mut test, "dclutch_rent_sbf", RENT_PROGRAM_ID, &elves.rent);
    add_program(
        &mut test,
        "dclutch_resolution_proof_sbf",
        RESOLUTION_PROGRAM_ID,
        &elves.resolution,
    );
    add_program(
        &mut test,
        "dclutch_pre_market_funding_test_caller_sbf",
        TRADING_CALLER_PROGRAM_ID,
        &elves.caller,
    );

    let core_release = release(CORE_PROGRAM_ID, [0x61; 32], &elves.core);
    let registry_release = release(REGISTRY_PROGRAM_ID, [0x62; 32], &elves.registry);
    let rent_release = release(RENT_PROGRAM_ID, [0x63; 32], &elves.rent);
    let caller_release = release(TRADING_CALLER_PROGRAM_ID, [0x64; 32], &elves.caller);
    let resolution_release = release(
        RESOLUTION_PROGRAM_ID,
        RESOLUTION_CONTROLLER_RELEASE_ID_V5,
        &elves.resolution,
    );
    let release_set = ExecutionReleaseSetV1::new(
        binding(core_release),
        binding(core_release),
        binding(caller_release),
        binding(resolution_release),
        binding(core_release),
    )
    .expect("release set");
    let release_set_id = hash(&release_set.to_bytes()).to_bytes();
    let mut activation_data = vec![0_u8; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut activation_data, content(release_set_id))
        .expect("activation cache");
    for (role, selected) in [
        (ExecutionRoleV1::Core, core_release),
        (ExecutionRoleV1::Claims, core_release),
        (ExecutionRoleV1::Trading, caller_release),
        (ExecutionRoleV1::Resolution, resolution_release),
        (ExecutionRoleV1::Custody, core_release),
    ] {
        activate_execution_role_into_v1(
            &mut activation_data,
            content(release_set_id),
            &release_set,
            role,
            &activation_input(selected),
        )
        .expect("activate role");
    }
    let activation = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set_id],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    test.add_account(
        activation,
        Account {
            lamports: Rent::default().minimum_balance(activation_data.len()),
            data: activation_data,
            owner: REGISTRY_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let (product, domain, portfolio, linked_basis, stable_product_id) = product_graph();
    let realm_value = RealmV1::new(RealmV1Input {
        token_program: [0x71; 32],
        collateral_mint: [0x72; 32],
        collateral_adapter_release_id: [0x73; 32],
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    })
    .expect("Realm");
    let realm = Record::new(REALM_SCHEMA_RELEASE_ID_V1, realm_value.to_bytes().to_vec());
    let capacity_value = SourceCapacityProfileV1::new(
        CapacityEnvelope::Measured,
        1,
        1,
        source_id(0x74),
        source_id(0x75),
        256,
        0,
    )
    .and_then(|profile| profile.bounding_principal(1, 1))
    .expect("Source capacity with explicit principal capacity");
    let capacity = Record::new(
        SOURCE_CAPACITY_PROFILE_SCHEMA_ID_V1,
        capacity_value.to_bytes().to_vec(),
    );
    let source_spec_value = SourceSpecV1::new(
        source_id(0x76),
        source_id(0x77),
        source_id(0x78),
        SourceAccessProfile::PythTerminalOneTransaction,
        source_id(0x79),
        SourceContentId::new(capacity.digest).expect("capacity identity"),
    );
    let source_spec = Record::new(
        SOURCE_SPEC_SCHEMA_ID_V1,
        source_spec_value.to_bytes().to_vec(),
    );
    let recovery_allocation = source_id(0x7a);
    let recovery_value = RecoveryPolicyV2::new(
        SourceContentId::new(capacity.digest).expect("capacity identity"),
        [
            Some(
                RecoveryAttemptV2::new(
                    source_id(0x7b),
                    source_id(0x7c),
                    1_900_000_000,
                    recovery_allocation,
                )
                .expect("recovery attempt"),
            ),
            None,
            None,
            None,
        ],
        1,
    )
    .expect("recovery policy");
    let recovery = Record::new(
        RECOVERY_POLICY_SCHEMA_ID_V2,
        recovery_value.to_bytes().to_vec(),
    );
    let material_value = SourceMaterialV3::explicitly_unbounded(
        SourceContentId::new(product.digest).expect("Product root"),
        SourceContentId::new(source_spec.digest).expect("SourceSpec"),
        source_id(0x7d),
        source_id(0x7e),
        Some(SourceContentId::new(recovery.digest).expect("recovery identity")),
        SourceContentId::new(SOURCE_FAILURE_POLICY_RELEASE_ID_V2).expect("failure release"),
    );
    let material = Record::new(
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        material_value.to_bytes().to_vec(),
    );
    let manifest = manifest(
        recovery_allocation.to_bytes(),
        recovery.digest,
        material.digest,
    );
    for record in [
        &realm,
        &product,
        &domain,
        &portfolio,
        &linked_basis,
        &material,
        &source_spec,
        &capacity,
        &recovery,
        &manifest,
    ] {
        record.add(&mut test);
    }
    let absent = [0_u8; 32];
    let absent_floor_raw = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &dclutch_source_contract::MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1,
            &absent,
        ],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    let absent_floor_staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &dclutch_source_contract::MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1,
            &absent,
        ],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    for key in [absent_floor_raw, absent_floor_staging] {
        test.add_account(
            key,
            Account {
                lamports: 1,
                data: Vec::new(),
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            },
        );
    }

    let registry_artifact = Record::new(
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        registry_release.to_bytes().to_vec(),
    );
    let rent_artifact = Record::new(
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        rent_release.to_bytes().to_vec(),
    );
    registry_artifact.add(&mut test);
    rent_artifact.add(&mut test);
    let infrastructure = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
        &CORE_PROGRAM_ID,
    )
    .0;
    let infrastructure_value =
        ProtocolInfrastructureProfileV1::new(binding(registry_release), binding(rent_release))
            .expect("infrastructure profile");
    test.add_account(
        infrastructure,
        Account {
            lamports: Rent::default().minimum_balance(infrastructure_value.to_bytes().len()),
            data: infrastructure_value.to_bytes().to_vec(),
            owner: CORE_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let identity = MarketIdentity {
        market_id: CoreIdentity::new([0xff; 32]).expect("placeholder Market"),
        realm_id: CoreIdentity::new(realm.digest).expect("Realm"),
        product_record: CoreIdentity::new(product.digest).expect("Product record"),
        product_id: CoreIdentity::new(stable_product_id).expect("Product"),
        resolution_policy: CoreIdentity::new(material.digest).expect("Source material"),
        capability_manifest: CoreIdentity::new(manifest.digest).expect("manifest"),
        selected_release_set: CoreIdentity::new(release_set_id).expect("release set"),
        registry_program: CoreIdentity::new(REGISTRY_PROGRAM_ID.to_bytes()).expect("Registry"),
        generation: GENERATION,
    };
    let market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(identity).as_slices(),
        &CORE_PROGRAM_ID,
    )
    .0;
    let found_payer = Keypair::new();
    test.add_account(
        found_payer.pubkey(),
        Account {
            lamports: 10_000_000_000,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    let (rent_credit, rent_bump) = Pubkey::find_program_address(
        &[
            LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
            market.as_ref(),
            &GENERATION.to_le_bytes(),
        ],
        &RENT_PROGRAM_ID,
    );
    let rent_credit_value = LifecycleRentCreditV2::new(
        RefundAuthority::new(found_payer.pubkey().to_bytes()).expect("refund authority"),
        LifecycleAccountIdV2::new(market.to_bytes()).expect("Market"),
        LifecycleAccountIdV2::new(release_set_id).expect("release set"),
        GENERATION,
        rent_bump,
    )
    .expect("RentCredit");
    test.add_account(
        rent_credit,
        Account {
            lamports: Rent::default().minimum_balance(rent_credit_value.to_bytes().len()),
            data: rent_credit_value.to_bytes().to_vec(),
            owner: RENT_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let manifest_view = CapabilityManifestV1::decode(&manifest.data).expect("manifest view");
    let manifest_id = CapabilityContentId::new(manifest.digest).expect("manifest identity");
    let width = funding_ledger_bytes_v2(3).expect("Resolution ledger width");
    let mut pending = vec![0_u8; width];
    FundingLedgerV2::initialize(&mut pending, manifest_id, manifest_view, 0b111)
        .expect("pending ledger projection");
    let ledger_view = FundingLedgerV2::decode(&pending).expect("pending ledger");
    let derivation = CapabilityFundingLedgerDerivationV2::new(
        RESOLUTION_PROGRAM_ID.to_bytes(),
        market.to_bytes(),
        GENERATION,
        manifest_id,
        ledger_view,
    )
    .expect("ledger derivation");
    let ledger =
        Pubkey::find_program_address(&derivation.seed_components(), &RESOLUTION_PROGRAM_ID).0;
    let source = Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
            market.as_ref(),
            &GENERATION.to_le_bytes(),
        ],
        &RESOLUTION_PROGRAM_ID,
    )
    .0;

    Fixture {
        test: Some(test),
        found_payer,
        market,
        rent_credit,
        realm,
        product,
        domain,
        portfolio,
        linked_basis,
        material,
        source_spec,
        capacity,
        recovery_allocation: recovery_allocation.to_bytes(),
        recovery,
        manifest,
        absent_floor_raw,
        absent_floor_staging,
        activation,
        core_programdata: programdata(CORE_PROGRAM_ID),
        registry_programdata: programdata(REGISTRY_PROGRAM_ID),
        rent_programdata: programdata(RENT_PROGRAM_ID),
        resolution_programdata: programdata(RESOLUTION_PROGRAM_ID),
        caller_programdata: programdata(TRADING_CALLER_PROGRAM_ID),
        infrastructure,
        registry_artifact,
        rent_artifact,
        ledger,
        source,
    }
}

fn found_accounts(fixture: &Fixture, writable: bool) -> Vec<AccountMeta> {
    let mut accounts = vec![
        AccountMeta::new_readonly(fixture.found_payer.pubkey(), false),
        AccountMeta::new_readonly(fixture.market, false),
        AccountMeta::new_readonly(fixture.rent_credit, false),
        AccountMeta::new_readonly(RENT_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.realm.raw, false),
        AccountMeta::new_readonly(fixture.realm.staging, false),
        AccountMeta::new_readonly(fixture.product.raw, false),
        AccountMeta::new_readonly(fixture.product.staging, false),
        AccountMeta::new_readonly(fixture.domain.raw, false),
        AccountMeta::new_readonly(fixture.domain.staging, false),
        AccountMeta::new_readonly(fixture.portfolio.raw, false),
        AccountMeta::new_readonly(fixture.portfolio.staging, false),
        AccountMeta::new_readonly(fixture.linked_basis.raw, false),
        AccountMeta::new_readonly(fixture.linked_basis.staging, false),
        AccountMeta::new_readonly(fixture.material.raw, false),
        AccountMeta::new_readonly(fixture.material.staging, false),
        AccountMeta::new_readonly(fixture.source_spec.raw, false),
        AccountMeta::new_readonly(fixture.source_spec.staging, false),
        AccountMeta::new_readonly(fixture.capacity.raw, false),
        AccountMeta::new_readonly(fixture.capacity.staging, false),
        AccountMeta::new_readonly(fixture.absent_floor_raw, false),
        AccountMeta::new_readonly(fixture.absent_floor_staging, false),
        AccountMeta::new_readonly(fixture.manifest.raw, false),
        AccountMeta::new_readonly(fixture.manifest.staging, false),
        AccountMeta::new_readonly(fixture.activation, false),
        AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.core_programdata, false),
        AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(fixture.infrastructure, false),
        AccountMeta::new_readonly(fixture.registry_artifact.raw, false),
        AccountMeta::new_readonly(fixture.registry_artifact.staging, false),
        AccountMeta::new_readonly(fixture.registry_programdata, false),
        AccountMeta::new_readonly(fixture.rent_artifact.raw, false),
        AccountMeta::new_readonly(fixture.rent_artifact.staging, false),
        AccountMeta::new_readonly(fixture.rent_programdata, false),
    ];
    assert_eq!(accounts.len(), 37);
    if writable {
        accounts[0] = AccountMeta::new(fixture.found_payer.pubkey(), true);
        accounts[1] = AccountMeta::new(fixture.market, false);
    }
    accounts
}

fn observation() -> Observation {
    Observation {
        slot: 1,
        unix_timestamp: 1_800_000_000,
        finality: Finality::Finalized,
    }
}

fn into_observed(key: Pubkey, account: Account) -> ObservedAccount {
    ObservedAccount {
        observation: observation(),
        key,
        owner: account.owner,
        lamports: account.lamports,
        executable: account.executable,
        data: account.data,
    }
}

fn vacant(key: Pubkey) -> ObservedAccount {
    ObservedAccount {
        observation: observation(),
        key,
        owner: system_program::ID,
        lamports: 0,
        executable: false,
        data: Vec::new(),
    }
}

async fn observed(context: &mut ProgramTestContext, key: Pubkey) -> Option<Account> {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account query")
}

async fn required(context: &mut ProgramTestContext, key: Pubkey) -> ObservedAccount {
    into_observed(key, observed(context, key).await.expect("required account"))
}

async fn found_snapshot(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
) -> Vec<ObservedAccount> {
    let keys = found_accounts(fixture, false)
        .into_iter()
        .map(|meta| meta.pubkey)
        .collect::<Vec<_>>();
    let mut output = Vec::with_capacity(keys.len());
    for key in keys {
        output.push(match observed(context, key).await {
            Some(account) => into_observed(key, account),
            None => vacant(key),
        });
    }
    output
}

async fn submit(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    signers: &[&Keypair],
) {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let mut all = vec![&context.payer];
    all.extend_from_slice(signers);
    let transaction = Transaction::new_signed_with_payer(
        instructions,
        Some(&context.payer.pubkey()),
        &all,
        blockhash,
    );
    context
        .banks_client
        .process_transaction(transaction)
        .await
        .expect("transaction commits");
}

#[tokio::test]
async fn initializer_found_and_create_preserve_the_resolution_ledger() {
    let mut fixture = fixture();
    let mut context = fixture
        .test
        .take()
        .expect("unstarted ProgramTest")
        .start_with_context()
        .await;
    let found_request = ProjectFoundRequestV2::new(Request::administrative(
        Action::Found,
        GENERATION,
        CoreIdentity::new(fixture.market.to_bytes()).expect("Market"),
    ))
    .expect("ProjectFound");
    let transaction_payer = context.payer.pubkey();
    let initializer = build_pre_market_funding_v1(
        &PreMarketFundingSnapshotV1 {
            resolution_program: required(&mut context, RESOLUTION_PROGRAM_ID).await,
            caller_program: required(&mut context, TRADING_CALLER_PROGRAM_ID).await,
            caller_programdata: required(&mut context, fixture.caller_programdata).await,
            resolution_programdata: required(&mut context, fixture.resolution_programdata).await,
            funding_source: required(&mut context, transaction_payer).await,
            ledger: vacant(fixture.ledger),
            project_found_accounts: found_snapshot(&mut context, &fixture).await,
        },
        found_request,
    )
    .expect("chain-derived 44-account initializer");
    assert_eq!(
        initializer.instruction.accounts.len(),
        PRE_MARKET_FUNDING_ACCOUNT_COUNT_V1
    );
    assert_eq!(PRE_MARKET_FUNDING_ACCOUNT_COUNT_V1, 44);
    for (index, account) in initializer.instruction.accounts.iter().enumerate() {
        for other in initializer.instruction.accounts.iter().skip(index + 1) {
            assert_ne!(account.pubkey, other.pubkey, "initializer address alias");
        }
    }
    assert_ne!(fixture.ledger, fixture.source);
    assert_eq!(initializer.selected_mask, 0b111);
    assert_eq!(
        initializer.expected_receipt.ledger,
        fixture.ledger.to_bytes()
    );
    assert_eq!(
        initializer.expected_receipt.rent_credit,
        fixture.rent_credit.to_bytes()
    );

    let mut caller_accounts = initializer.instruction.accounts.clone();
    caller_accounts[0].is_signer = false;
    let caller_instruction = Instruction {
        program_id: TRADING_CALLER_PROGRAM_ID,
        accounts: caller_accounts,
        data: initializer.instruction.data.clone(),
    };
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[caller_instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("Banks RPC");
    assert!(processed.result.is_ok(), "initializer transaction commits");
    let returned = processed
        .metadata
        .expect("initializer metadata")
        .return_data
        .expect("initializer receipt");
    assert_eq!(returned.program_id, TRADING_CALLER_PROGRAM_ID);
    assert_eq!(returned.data.len(), PRE_MARKET_FUNDING_RECEIPT_BYTES_V1);
    let receipt =
        authenticate_pre_market_funding_receipt_v1(&returned.data, initializer.expected_receipt)
            .expect("exact initializer receipt");
    assert_eq!(receipt, initializer.expected_receipt);

    let initialized_ledger = observed(&mut context, fixture.ledger)
        .await
        .expect("initialized Resolution ledger");
    assert_eq!(initialized_ledger.owner, RESOLUTION_PROGRAM_ID);
    assert_eq!(
        initialized_ledger.lamports,
        initializer.exact_funding_lamports
    );
    let classified_lamports = receipt
        .exact_rent_lamports
        .checked_add(receipt.exact_native_principal)
        .expect("classified initializer lamports");
    assert_eq!(initialized_ledger.lamports, classified_lamports);
    assert_eq!(
        hash(&initialized_ledger.data).to_bytes(),
        receipt.poststate_digest
    );
    let manifest_view = CapabilityManifestV1::decode(&fixture.manifest.data).expect("manifest");
    let manifest_id = CapabilityContentId::new(fixture.manifest.digest).expect("manifest ID");
    let authenticated = FundingLedgerV2::decode(&initialized_ledger.data)
        .and_then(|ledger| ledger.authenticate(manifest_id, manifest_view))
        .expect("authenticated initialized ledger");
    for index in [0_u16, 1, 2] {
        assert_eq!(
            authenticated.slot(index).expect("selected row").status(),
            FundingLedgerStatusV2::Pending
        );
    }
    let classified_native_principal = authenticated
        .remaining_native_lamports_total()
        .expect("classified native principal");
    assert_eq!(classified_native_principal, receipt.exact_native_principal);
    let unsolicited_surplus = initialized_ledger
        .lamports
        .checked_sub(receipt.exact_rent_lamports)
        .and_then(|remainder| remainder.checked_sub(classified_native_principal))
        .expect("ledger classifications are bounded by its lamports");
    assert_eq!(unsolicited_surplus, 0);

    submit(
        &mut context,
        &[Instruction {
            program_id: CORE_PROGRAM_ID,
            accounts: found_accounts(&fixture, true),
            data: found_request
                .found
                .encode()
                .expect("Found request")
                .to_vec(),
        }],
        &[&fixture.found_payer],
    )
    .await;
    let market = CoreState::decode(
        &observed(&mut context, fixture.market)
            .await
            .expect("founded Market")
            .data,
    )
    .expect("Core state");
    assert_eq!(market.phase, Phase::Founding);
    assert_eq!(market.readiness, Readiness::Prepaid);

    let create = build_resolution_create_fund_v3(&ResolutionCreateFundSnapshotV3 {
        market: required(&mut context, fixture.market).await,
        activation_cache: required(&mut context, fixture.activation).await,
        registry_program: required(&mut context, REGISTRY_PROGRAM_ID).await,
        core_program: required(&mut context, CORE_PROGRAM_ID).await,
        core_programdata: required(&mut context, fixture.core_programdata).await,
        resolution_program: required(&mut context, RESOLUTION_PROGRAM_ID).await,
        resolution_programdata: required(&mut context, fixture.resolution_programdata).await,
        source_material: required(&mut context, fixture.material.raw).await,
        source_material_staging: vacant(fixture.material.staging),
        capability_manifest: required(&mut context, fixture.manifest.raw).await,
        capability_manifest_staging: vacant(fixture.manifest.staging),
        source_destination: vacant(fixture.source),
        funding_ledger: required(&mut context, fixture.ledger).await,
        rent_sysvar: required(&mut context, sysvar::rent::ID).await,
        system_program: required(&mut context, system_program::ID).await,
        recovery_policy: required(&mut context, fixture.recovery.raw).await,
        recovery_policy_staging: vacant(fixture.recovery.staging),
    })
    .expect("V5 CreateFund against initializer ledger");
    validate_resolution_create_fund_report_v3(&create).expect("exact CreateFund report");
    let create_manifest =
        CapabilityManifestV1::decode(&fixture.manifest.data).expect("CreateFund manifest");
    for (index, expected_config) in create.funding_entry_indices.into_iter().zip([
        fixture.recovery_allocation,
        fixture.recovery.digest,
        fixture.material.digest,
    ]) {
        assert_eq!(
            create_manifest
                .entry(index)
                .expect("CreateFund funding row")
                .config_id()
                .to_bytes(),
            expected_config
        );
    }
    let mut exhaustive_indices = create.funding_entry_indices;
    exhaustive_indices.sort_unstable();
    assert_eq!(exhaustive_indices, [0, 1, 2]);
    submit(
        &mut context,
        &[
            transfer(
                &transaction_payer,
                &fixture.source,
                create.source_top_up_lamports,
            ),
            create.instruction,
        ],
        &[],
    )
    .await;
    assert_eq!(
        observed(&mut context, fixture.ledger)
            .await
            .expect("ledger after CreateFund"),
        initialized_ledger,
        "CreateFund preserves the initializer-owned ledger bytes and lamports"
    );
    let source = SourceResolutionStateV2::decode(
        &observed(&mut context, fixture.source)
            .await
            .expect("created Source")
            .data,
    )
    .expect("Source state");
    assert_eq!(source.phase(), SourceResolutionPhaseV1::Primary);
    assert_eq!(source.market(), fixture.market.to_bytes());
    assert_eq!(source.generation(), GENERATION);
}
