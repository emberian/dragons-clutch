//! Real-SVM continuity from pre-Market Resolution funding through CreateFund.
//!
//! This focused fixture keeps the three authority boundaries separate:
//! a real Trading caller signs the universal CallerAuthority PDA, the current
//! Resolution deployment projects Core's exact ProjectFound36 frame and creates its
//! own Pending subset ledger, and ordinary Core Found later creates the Market.
//! V6 CreateFund must then create only Source state while preserving the
//! initializer-owned ledger byte-for-byte and lamport-for-lamport.

use std::{env, fs, path::PathBuf};

use dclutch_market::capability_manifest::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    CapabilityEntryV1, CapabilityFundingLedgerDerivationV2, CapabilityManifestV1,
    CompartmentFundingV1, ContentId as CapabilityContentId,
    ControllerFundingCheckpointDerivationV1, ControllerFundingCheckpointInputV1,
    ControllerFundingCheckpointV1, FUNDING_STATE_BYTES, FundingAmountsV1, FundingLedgerStatusV2,
    FundingLedgerV2, FundingQuoteV1, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
    derive_funded_rent_rate_v2, funding_ledger_bytes_v2,
};
use dclutch_core_contract::ContentId as CoreContentId;
use dclutch_market::{
    Action, CoreState, FOUND_RENT_SYSVAR_INDEX_V3, Identity as CoreIdentity,
    MarketCoreStateSeedsV2, MarketIdentity, PROJECT_FOUND_ACCOUNT_COUNT_V2,
    PROJECT_FOUND_RECEIPT_BYTES_V2, Phase, ProjectFoundReceiptV2, ProjectFoundRequestV2, Readiness,
    Request,
};
use dclutch_product::payoff::{
    registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
    runtime_v3::{
        BasisInputV3, BasisKindV3, SEMANTIC_BASIS_CONTENT_DOMAIN_V3, basis_record_bytes_v3,
        compile_basis_v3, semantic_basis_preimage_v3,
    },
};
use dclutch_product::{
    ContentId as ProductContentId, portfolio_record_bytes, result_domain_record_bytes,
};
use dclutch_product::admission::PRODUCT_RECORD_BYTES_V2;
use dclutch_product_runtime_v2_operator::{ProductCompilationInputV2, compile_product_records_v2};
use dclutch_program_test_evidence::TransactionEvidence;
use dclutch_market::realm::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1, RealmV1Input,
};
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, activate_execution_role_into_v1,
    initialize_activation_cache_v1,
};
use dclutch_registry::release_set::CallerAuthoritySeedsV1;
use dclutch_registry::release_set::{
    ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2, ProgramIdentityV1,
    ProtocolInfrastructureProfileV2,
};
use dclutch_market::rent::{
    RefundAuthority,
    lifecycle_v2::{
        LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleAccountIdV2, LifecycleRentCreditV2,
    },
};
use dclutch_source::resolution::{
    PRE_MARKET_FUNDING_ABORT_RECEIPT_BYTES_V1, PRE_MARKET_FUNDING_RECEIPT_BYTES_V2,
    PreMarketFundingAbortReceiptV1, PreMarketFundingAbortRequestV1,
    RESOLUTION_CONTROLLER_RELEASE_ID_V7, pre_market_funding_ledger_account_digest_v1,
};
use dclutch_resolution_core_v3_operator::{
    Finality, Observation, ObservedAccount, ResolutionCreateFundSnapshotV3,
    build_resolution_create_fund_v3,
    pre_market_funding_v1::{
        PRE_MARKET_FUNDING_ACCOUNT_COUNT_V1, PRE_MARKET_PROJECT_FOUND_ACCOUNT_COUNT_V1,
        PreMarketFundingSnapshotV2, authenticate_pre_market_funding_receipt_v2,
        build_pre_market_funding_v2,
    },
    validate_resolution_create_fund_report_v3,
};
use dclutch_resolution_proof_sbf::ResolutionError;
use dclutch_source::{
    CapacityEnvelope, ContentId as SourceContentId, RECOVERY_POLICY_SCHEMA_ID_V2,
    RecoveryAttemptV2, RecoveryPolicyV2, SOURCE_CAPACITY_PROFILE_SCHEMA_ID_V1,
    SOURCE_FAILURE_POLICY_RELEASE_ID_V2, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
    SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2, SOURCE_SPEC_SCHEMA_ID_V1, SourceAccessProfile,
    SourceCapacityProfileV1, SourceMaterialV3, SourceResolutionPhaseV1, SourceResolutionStateV2,
    SourceSpecV1,
};
use solana_account::{Account, AccountSharedData};
use solana_address_lookup_table_interface::instruction::{
    create_lookup_table, extend_lookup_table, freeze_lookup_table,
};
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_program::{
    clock::Clock,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::hash::Hash;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_system_interface::instruction::transfer;
use solana_transaction::{
    InstructionError, Transaction, TransactionError, versioned::VersionedTransaction,
};

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

/// The exemption-scaled rent rate this bank charges, which is what a founding
/// here records in its FundingLedgerV2 header. Every account these fixtures
/// fund is priced with the same `Rent::default()`, so a ledger's own
/// `validate_recorded_native_custody` has to agree with this figure.
fn funded_rent_rate(account_bytes: usize) -> u32 {
    let rent = Rent::default();
    derive_funded_rent_rate_v2(
        rent.minimum_balance(0),
        account_bytes,
        rent.minimum_balance(account_bytes),
    )
    .expect("Rent::default() is affine in the account length")
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
        // Exempt by proof: degree 0 and 1 need no price gate,
        // and a digest offered alongside one is refused.
        price_gate_certificate_digest: [0_u8; 32],
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
            content(RESOLUTION_CONTROLLER_RELEASE_ID_V7),
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
        RESOLUTION_CONTROLLER_RELEASE_ID_V7,
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
        // The no-recovery material. `SourceResolutionStateV2` has no transition
        // that advances a recovery attempt -- `funded.rs` plans the whole walk
        // as `Primary -> Exhausted -> FailureCommitted` -- so a material that
        // named a recovery policy would found a market no route can terminalize
        // and `build_resolution_create_fund_v3` refuses it at `12d0deb5`'s weld.
        None,
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
            &dclutch_source::MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1,
            &absent,
        ],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    let absent_floor_staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &dclutch_source::MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1,
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
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
        &CORE_PROGRAM_ID,
    )
    .0;
    // Registry moved across the succession and Rent did not: the predecessor
    // Registry id names the distinct release this profile succeeded, while
    // Rent holds the same id on both sides of it.
    let predecessor_registry_release = release(REGISTRY_PROGRAM_ID, [0xb2; 32], &elves.registry);
    let infrastructure_value = ProtocolInfrastructureProfileV2::new(
        binding(registry_release),
        binding(rent_release),
        artifact_id(predecessor_registry_release),
        artifact_id(rent_release),
    )
    .expect("infrastructure succession profile");
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
    let pending_rate = funded_rent_rate(width);
    FundingLedgerV2::initialize(
        &mut pending,
        manifest_id,
        manifest_view,
        0b111,
        pending_rate,
    )
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
    let keys = project_found_accounts(fixture)
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

fn project_found_accounts(fixture: &Fixture) -> Vec<AccountMeta> {
    let mut accounts = found_accounts(fixture, false);
    accounts.remove(FOUND_RENT_SYSVAR_INDEX_V3);
    assert_eq!(accounts.len(), PROJECT_FOUND_ACCOUNT_COUNT_V2);
    accounts
}

/// The extent of a signed legacy transaction on the wire.
///
/// It MEASURES and does not judge, and deliberately carries no copy of Solana's
/// 1,232-byte `PACKET_DATA_BYTES`. `solana-program-test` submits no packet and
/// cannot enforce that maximum itself, so the comparison belongs in
/// `tools/gauntlet/resolution-pre-market-funding/witnesses.json`.
fn wire_extent(signatures: usize, message: &[u8]) -> usize {
    1 + signatures * 64 + message.len()
}

/// What one route costs on the wire, measured both ways.
///
/// A conversion that reports only the number it arrived at is not a
/// measurement, it is an assertion: the reader cannot tell whether the route
/// moved or the instrument did. Both extents are built from the SAME
/// instruction bytes and the SAME account set, so the only difference between
/// them is the envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PacketExtentV1 {
    /// The signed legacy message: every address inline. This is the number the
    /// route could not be submitted at.
    legacy_bytes: usize,
    /// The signed v0 message over the route's own derived table.
    v0_bytes: usize,
    /// Addresses the v0 message still carries inline -- payer, program ids and
    /// signers, which no table can move.
    static_keys: usize,
    /// Addresses the table resolved.
    loaded_addresses: usize,
}

/// Solana's serialized transaction packet maximum.
///
/// Restated here because this harness resolves independently of the protocol
/// workspace and cannot link `dclutch_versioned_message_operator`, which owns
/// the constant: that crate pins `solana-hash =4.6.0` and
/// `solana-address-lookup-table-interface =3.2.0` against this harness's
/// `solana-message =4.4.1` and `=3.1.0`, and the two pin sets have no common
/// solution. Named, not typed at a call site, so there is one copy to retire
/// when the workspaces converge.
const PACKET_DATA_BYTES: usize = 1_232;

/// Addresses per table-extension transaction, bounded so the extension itself
/// stays a packet.
const EXTEND_ADDRESSES_PER_TRANSACTION_V1: usize = 20;

/// The addresses this route's table must carry, decided by the message compiler
/// rather than by a filter written here.
///
/// Two classes of address can never be looked up -- an instruction's program id
/// has to resolve before the tables load, and a signer is authenticated by its
/// position in the static header -- and a campaign that states that rule in its
/// own words acquires a second author for it. So this states nothing: it offers
/// the compiler every address the route names and keeps the ones the compiler
/// resolved through a table. A table entry the runtime declines to use is
/// ignored in silence and costs permanent rent, which is exactly the failure a
/// hand-written filter produces and nothing catches.
///
/// `dclutch_versioned_message_operator::canonical_route_lookup_addresses_v1` is
/// the same probe for the protocol workspace. This harness cannot link it (see
/// `PACKET_DATA_BYTES` above), and the two agree by construction rather than by
/// discipline: neither states the rule, both ask the compiler for it.
fn route_lookup_addresses(payer: Pubkey, instructions: &[Instruction]) -> Vec<Pubkey> {
    let mut candidates: Vec<Pubkey> = Vec::new();
    for instruction in instructions {
        for account in &instruction.accounts {
            if !candidates.contains(&account.pubkey) {
                candidates.push(account.pubkey);
            }
        }
    }
    let probe_key = Pubkey::new_from_array([0xff; 32]);
    assert!(
        !candidates.contains(&probe_key) && payer != probe_key,
        "the probe table's key must not be one of the route's own coordinates"
    );
    let probe = v0::Message::try_compile(
        &payer,
        instructions,
        &[AddressLookupTableAccount {
            key: probe_key,
            addresses: candidates.clone(),
        }],
        Hash::default(),
    )
    .expect("the route compiles as a message");
    let mut eligible = Vec::new();
    for lookup in &probe.address_table_lookups {
        for index in lookup
            .writable_indexes
            .iter()
            .chain(lookup.readonly_indexes.iter())
        {
            eligible.push(candidates[usize::from(*index)]);
        }
    }
    eligible.sort_unstable_by_key(Pubkey::to_bytes);
    eligible.dedup();
    assert!(
        !eligible.is_empty(),
        "a route with nothing a table can carry does not need one"
    );
    eligible
}

/// Create, extend and FREEZE this route's own lookup table, then wait out the
/// slot its addresses need to become resolvable.
///
/// The address list is derived, never written here:
/// `canonical_route_lookup_addresses_v1` reads the route's own account metas,
/// so the table cannot name a coordinate the instruction does not, and a
/// campaign that mis-states the frame gets a compile failure rather than a
/// quietly different transaction.
///
/// Freezing is not tidiness. A mutable table is a second authority over which
/// addresses a submitted message actually resolves to, which is the substitution
/// the Pyth caller refuses by name; freezing makes the routing data as fixed as
/// the instruction bytes it routes. The rent is permanent and intended.
async fn frozen_route_lookup_table(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
) -> (Pubkey, Vec<Pubkey>) {
    let payer = context.payer.pubkey();
    let addresses = route_lookup_addresses(payer, instructions);
    let clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("pre-derivation Clock");
    context
        .warp_to_slot(clock.slot + 1)
        .expect("the derivation slot must be strictly recent");
    let (create, table) = create_lookup_table(payer, payer, clock.slot);
    submit(context, &[create], &[]).await;
    for chunk in addresses.chunks(EXTEND_ADDRESSES_PER_TRANSACTION_V1) {
        submit(
            context,
            &[extend_lookup_table(
                table,
                payer,
                Some(payer),
                chunk.to_vec(),
            )],
            &[],
        )
        .await;
    }
    submit(context, &[freeze_lookup_table(table, payer)], &[]).await;
    let extended = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("post-extension Clock");
    context
        .warp_to_slot(extended.slot + 1)
        .expect("appended addresses resolve only after the slot they landed in");
    (table, addresses)
}

/// Submit one labelled step as a v0 message over a frozen table, and record
/// both extents.
///
/// The legacy extent is computed from the identical instructions and thrown
/// away unsubmitted -- it is the control, and it is what this route used to be.
async fn process_recorded_v0(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    table: Pubkey,
    addresses: &[Pubkey],
    label: &str,
) -> (
    Result<(), TransactionError>,
    Option<(Pubkey, Vec<u8>)>,
    PacketExtentV1,
) {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let payer = context.payer.pubkey();
    let legacy = Transaction::new_signed_with_payer(
        instructions,
        Some(&payer),
        &[&context.payer],
        blockhash,
    );
    let legacy_bytes = wire_extent(legacy.signatures.len(), &legacy.message.serialize());
    let compiled = v0::Message::try_compile(
        &payer,
        instructions,
        &[AddressLookupTableAccount {
            key: table,
            addresses: addresses.to_vec(),
        }],
        blockhash,
    )
    .expect("the route compiles as v0 over its own frozen table");
    let static_keys = compiled.account_keys.len();
    let loaded_addresses: usize = compiled
        .address_table_lookups
        .iter()
        .map(|lookup| lookup.writable_indexes.len() + lookup.readonly_indexes.len())
        .sum();
    let message = VersionedMessage::V0(compiled);
    let payer_signer = context.payer.insecure_clone();
    let transaction =
        VersionedTransaction::try_new(message, &[&payer_signer]).expect("signed v0 transaction");
    let v0_bytes = wire_extent(
        transaction.signatures.len(),
        &transaction.message.serialize(),
    );
    let extent = PacketExtentV1 {
        legacy_bytes,
        v0_bytes,
        static_keys,
        loaded_addresses,
    };
    let signature = transaction
        .signatures
        .first()
        .expect("signed transaction")
        .to_string();
    let slot = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .map_or(0, |clock| clock.slot);
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("Banks RPC");
    let outcome = processed.result.clone();
    let failure = outcome.clone().err().map(|error| format!("{error:?}"));
    let metadata = processed.metadata;
    let logs = metadata
        .as_ref()
        .map(|value| value.log_messages.clone())
        .unwrap_or_default();
    let units = metadata
        .as_ref()
        .map_or(0, |value| value.compute_units_consumed);
    let returned = metadata
        .and_then(|value| value.return_data)
        .map(|value| (value.program_id, value.data));
    dclutch_program_test_evidence::record(&TransactionEvidence {
        label,
        signature: &signature,
        slot,
        error: failure.as_deref(),
        logs: &logs,
        compute_units_consumed: Some(units),
        wire_bytes: Some(v0_bytes),
    })
    .expect("campaign evidence must be writable when the gauntlet asked for it");
    (outcome, returned, extent)
}

/// Submit one labelled step, record what the chain did, and return the result
/// together with the program-return data the caller inspects.
///
/// The evidence is emitted BEFORE the caller asserts anything, so a step that
/// fails its own assertion still leaves behind what the chain did. Only the
/// steps `tools/gauntlet/resolution-pre-market-funding/bindings.json` names come
/// through here, and each label names exactly one transaction -- which is what
/// lets a binding carry one outcome and one refusal code.
async fn process_recorded(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    label: &str,
) -> (Result<(), TransactionError>, Option<(Pubkey, Vec<u8>)>) {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = Transaction::new_signed_with_payer(
        instructions,
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    let signature = transaction
        .signatures
        .first()
        .expect("signed transaction")
        .to_string();
    let extent = wire_extent(
        transaction.signatures.len(),
        &transaction.message.serialize(),
    );
    let slot = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .map_or(0, |clock| clock.slot);
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("Banks RPC");
    let outcome = processed.result.clone();
    let failure = outcome.clone().err().map(|error| format!("{error:?}"));
    let metadata = processed.metadata;
    let logs = metadata
        .as_ref()
        .map(|value| value.log_messages.clone())
        .unwrap_or_default();
    let units = metadata
        .as_ref()
        .map_or(0, |value| value.compute_units_consumed);
    let returned = metadata
        .and_then(|value| value.return_data)
        .map(|value| (value.program_id, value.data));
    dclutch_program_test_evidence::record(&TransactionEvidence {
        label,
        signature: &signature,
        slot,
        error: failure.as_deref(),
        logs: &logs,
        compute_units_consumed: Some(units),
        wire_bytes: Some(extent),
    })
    .expect("campaign evidence must be writable when the gauntlet asked for it");
    (outcome, returned)
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

async fn project_found_receipt(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    request: ProjectFoundRequestV2,
) -> ProjectFoundReceiptV2 {
    let instruction = Instruction {
        program_id: CORE_PROGRAM_ID,
        accounts: project_found_accounts(fixture),
        data: request.encode().expect("ProjectFound request").to_vec(),
    };
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("Banks RPC");
    assert!(processed.result.is_ok(), "ProjectFound projection succeeds");
    let returned = processed
        .metadata
        .expect("projection metadata")
        .return_data
        .expect("ProjectFound receipt");
    assert_eq!(returned.program_id, CORE_PROGRAM_ID);
    assert_eq!(returned.data.len(), PROJECT_FOUND_RECEIPT_BYTES_V2);
    ProjectFoundReceiptV2::decode(&returned.data).expect("canonical ProjectFound receipt")
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
    let expected_project_found = project_found_receipt(&mut context, &fixture, found_request).await;
    let transaction_payer = context.payer.pubkey();
    let initializer = build_pre_market_funding_v2(
        &PreMarketFundingSnapshotV2 {
            resolution_program: required(&mut context, RESOLUTION_PROGRAM_ID).await,
            caller_program: required(&mut context, TRADING_CALLER_PROGRAM_ID).await,
            caller_programdata: required(&mut context, fixture.caller_programdata).await,
            resolution_programdata: required(&mut context, fixture.resolution_programdata).await,
            funding_source: required(&mut context, transaction_payer).await,
            ledger: vacant(fixture.ledger),
            rent: required(&mut context, sysvar::rent::ID).await,
            project_found_accounts: found_snapshot(&mut context, &fixture).await,
        },
        found_request,
        expected_project_found,
    )
    .expect("chain-derived 43-account initializer");
    assert_eq!(
        initializer.instruction.accounts.len(),
        PRE_MARKET_FUNDING_ACCOUNT_COUNT_V1
    );
    assert_eq!(PRE_MARKET_FUNDING_ACCOUNT_COUNT_V1, 43);
    for (index, account) in initializer.instruction.accounts.iter().enumerate() {
        for other in initializer.instruction.accounts.iter().skip(index + 1) {
            assert_ne!(account.pubkey, other.pubkey, "initializer address alias");
        }
    }
    assert_ne!(fixture.ledger, fixture.source);
    assert_eq!(initializer.selected_mask, 0b111);
    // The initializer and both hostiles are 43-meta frames over the SAME
    // address set -- each hostile aliases one coordinate onto another already in
    // it, which is what makes them hostile -- so one table derived from the
    // initializer's own metas routes all three, and a hostile cannot quietly
    // acquire an address the honest route does not name.
    let honest_initializer = Instruction {
        program_id: TRADING_CALLER_PROGRAM_ID,
        accounts: {
            let mut accounts = initializer.instruction.accounts.clone();
            accounts[0].is_signer = false;
            accounts
        },
        data: initializer.instruction.data.clone(),
    };
    let (route_table, route_addresses) =
        frozen_route_lookup_table(&mut context, std::slice::from_ref(&honest_initializer)).await;
    assert_eq!(
        route_addresses.len(),
        41,
        "43 metas less the transaction payer and the invoked caller program"
    );
    assert_eq!(
        initializer.expected_receipt.ledger,
        fixture.ledger.to_bytes()
    );
    assert_eq!(
        initializer.expected_receipt.rent_credit,
        fixture.rent_credit.to_bytes()
    );

    let found_start = PRE_MARKET_FUNDING_ACCOUNT_COUNT_V1
        .checked_sub(PRE_MARKET_PROJECT_FOUND_ACCOUNT_COUNT_V1)
        .expect("initializer prefix width");
    let mut source_aliased_accounts = initializer.instruction.accounts.clone();
    source_aliased_accounts[0].is_signer = false;
    source_aliased_accounts[found_start].pubkey = source_aliased_accounts[5].pubkey;
    let source_aliased_instruction = Instruction {
        program_id: TRADING_CALLER_PROGRAM_ID,
        accounts: source_aliased_accounts,
        data: initializer.instruction.data.clone(),
    };
    let (source_aliased, _, source_aliased_extent) = process_recorded_v0(
        &mut context,
        &[source_aliased_instruction],
        route_table,
        &route_addresses,
        "pre-market funding: refuses a funding source aliased into ProjectFound36",
    )
    .await;
    assert!(
        source_aliased_extent.legacy_bytes > PACKET_DATA_BYTES
            && source_aliased_extent.v0_bytes <= PACKET_DATA_BYTES,
        "a hostile nobody could submit refuses nothing: {source_aliased_extent:?}"
    );
    assert!(
        matches!(
            source_aliased,
            Err(TransactionError::InstructionError(
                0,
                InstructionError::Custom(code)
            )) if code == ResolutionError::AccountFrame as u32
        ),
        "a funding source aliased into ProjectFound36 must refuse as Resolution \
         AccountFrame, got {source_aliased:?}"
    );
    assert_eq!(
        observed(&mut context, fixture.ledger).await,
        None,
        "the refused funding-source alias leaves the Resolution ledger vacant"
    );

    let mut aliased_accounts = initializer.instruction.accounts.clone();
    aliased_accounts[0].is_signer = false;
    aliased_accounts[found_start + 4].pubkey = aliased_accounts[found_start + 5].pubkey;
    let aliased_instruction = Instruction {
        program_id: TRADING_CALLER_PROGRAM_ID,
        accounts: aliased_accounts,
        data: initializer.instruction.data.clone(),
    };
    let (aliased, _, aliased_extent) = process_recorded_v0(
        &mut context,
        &[aliased_instruction],
        route_table,
        &route_addresses,
        "pre-market funding: refuses an internal ProjectFound36 alias",
    )
    .await;
    assert!(
        aliased_extent.legacy_bytes > PACKET_DATA_BYTES
            && aliased_extent.v0_bytes <= PACKET_DATA_BYTES,
        "a hostile nobody could submit refuses nothing: {aliased_extent:?}"
    );
    // Core, not Resolution, catches this one: the alias is internal to the
    // ProjectFound36 frame Core authenticates, and the observed refusal is
    // `core/CoreSbfError::AccountFrame` raised at depth three and re-reported
    // by Resolution and the caller on the way out. This workspace takes no
    // dependency on the Core program crate, so the exact code is asserted where
    // it can be derived rather than typed --
    // `tools/gauntlet/resolution-pre-market-funding/bindings.json`, which the
    // census checks against the inventory AND against which program raised it.
    // What is derivable here is the half that discriminates: the refusing band
    // is not Resolution's, so this hostile is not being caught one frame early.
    assert!(
        matches!(
            aliased,
            Err(TransactionError::InstructionError(
                0,
                InstructionError::Custom(code)
            )) if code >> 12 != (ResolutionError::AccountFrame as u32) >> 12
        ),
        "an internal ProjectFound36 alias must refuse outside Resolution's band, \
         got {aliased:?}"
    );
    assert_eq!(
        observed(&mut context, fixture.ledger).await,
        None,
        "the refused projection leaves the Resolution ledger vacant"
    );

    let (outcome, returned, extent) = process_recorded_v0(
        &mut context,
        std::slice::from_ref(&honest_initializer),
        route_table,
        &route_addresses,
        "pre-market funding: initialize the Resolution-owned Pending ledger",
    )
    .await;
    assert!(outcome.is_ok(), "initializer transaction commits");
    // The whole point of this route, as a number. 1,797 is what a legacy message
    // costs and it is 565 over Solana's maximum, so the initializer had never
    // been submittable anywhere -- ProgramTest submits no packet, which is
    // exactly why it survived. Forty-one of its forty-three coordinates are
    // ordinary accounts and each becomes a one-byte index: 41 x 31 = 1,271, less
    // the 36 the lookup entry itself costs (the table key, its two short-vector
    // prefixes, the lookups short-vector and the version byte), which is the
    // 1,235 bytes between the two figures. Only two addresses stay inline, and
    // neither could have moved: the transaction payer and the invoked caller
    // program. Nothing about the instruction changed -- same data, same account
    // set, same privileges, same program, same 43 metas. Only the envelope.
    assert_eq!(
        extent,
        PacketExtentV1 {
            legacy_bytes: 1_797,
            v0_bytes: 562,
            static_keys: 2,
            loaded_addresses: 41,
        }
    );
    assert!(extent.legacy_bytes > PACKET_DATA_BYTES);
    assert!(extent.v0_bytes <= PACKET_DATA_BYTES);
    let returned = returned.expect("initializer receipt");
    assert_eq!(returned.0, TRADING_CALLER_PROGRAM_ID);
    assert_eq!(returned.1.len(), PRE_MARKET_FUNDING_RECEIPT_BYTES_V2);
    let receipt =
        authenticate_pre_market_funding_receipt_v2(&returned.1, initializer.expected_receipt)
            .expect("exact initializer receipt");
    assert_eq!(receipt, initializer.expected_receipt);

    let initialized_ledger = observed(&mut context, fixture.ledger)
        .await
        .expect("initialized Resolution ledger");
    assert_eq!(initialized_ledger.owner, RESOLUTION_PROGRAM_ID);
    assert_eq!(initialized_ledger.lamports, initializer.exact_post_lamports);
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
        // The no-recovery frame rule: with no policy record to authenticate,
        // the two policy positions must re-present the already-authenticated
        // Source-material pair, so every frame position stays authenticated
        // against exactly one expectation.
        recovery_policy: required(&mut context, fixture.material.raw).await,
        recovery_policy_staging: vacant(fixture.material.staging),
    })
    .expect("V6 CreateFund against initializer ledger");
    validate_resolution_create_fund_report_v3(&create).expect("exact CreateFund report");
    let create_manifest =
        CapabilityManifestV1::decode(&fixture.manifest.data).expect("CreateFund manifest");
    // The no-recovery selection is a derivation, not a choice: the failure
    // compartment is the unique Resolution-controller entry configured by this
    // market's own Source material, and it is returned LAST; the recovery and
    // exhaustion compartments are exactly the two other Resolution-controller
    // entries in manifest order. `manifest()` orders entries by `kind_id`,
    // which is `hash(config)`, so the expectation is derived here from the two
    // other configs rather than read back out of the manifest under test.
    let mut other_configs = [fixture.recovery_allocation, fixture.recovery.digest];
    other_configs.sort_by_key(|config| hash(config).to_bytes());
    for (index, expected_config) in create.funding_entry_indices.into_iter().zip([
        other_configs[0],
        other_configs[1],
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

#[tokio::test]
async fn expired_prepared_checkpoint_refunds_and_closes_resolution_ledger() {
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
    let projected = project_found_receipt(&mut context, &fixture, found_request).await;
    let payer = context.payer.pubkey();
    let initializer = build_pre_market_funding_v2(
        &PreMarketFundingSnapshotV2 {
            resolution_program: required(&mut context, RESOLUTION_PROGRAM_ID).await,
            caller_program: required(&mut context, TRADING_CALLER_PROGRAM_ID).await,
            caller_programdata: required(&mut context, fixture.caller_programdata).await,
            resolution_programdata: required(&mut context, fixture.resolution_programdata).await,
            funding_source: required(&mut context, payer).await,
            ledger: vacant(fixture.ledger),
            rent: required(&mut context, sysvar::rent::ID).await,
            project_found_accounts: found_snapshot(&mut context, &fixture).await,
        },
        found_request,
        projected,
    )
    .expect("initializer");
    let mut create_accounts = initializer.instruction.accounts.clone();
    create_accounts[0].is_signer = false;
    submit(
        &mut context,
        &[Instruction {
            program_id: TRADING_CALLER_PROGRAM_ID,
            accounts: create_accounts,
            data: initializer.instruction.data.clone(),
        }],
        &[],
    )
    .await;

    let ledger_before = observed(&mut context, fixture.ledger)
        .await
        .expect("initialized ledger");
    let rent_credit_before = observed(&mut context, fixture.rent_credit)
        .await
        .expect("RentCredit");
    let funding_list = [0x76; 32];
    let trading_ledger = [0x77; 32];
    let checkpoint_input = ControllerFundingCheckpointInputV1 {
        release_set: projected.release_set.to_bytes(),
        market: fixture.market.to_bytes(),
        generation: GENERATION,
        manifest: fixture.manifest.digest,
        funding_list,
        found_request_digest: initializer.expected_receipt.found_request_digest,
        project_found_receipt_digest: initializer.expected_receipt.project_found_receipt_digest,
        resolution_ledger: fixture.ledger.to_bytes(),
        resolution_ledger_digest: hash(&ledger_before.data).to_bytes(),
        trading_ledger,
        trading_ledger_digest: [0x78; 32],
        funding_source: payer.to_bytes(),
        rent_credit: fixture.rent_credit.to_bytes(),
        lock_request_digest: [0x79; 32],
        expiry_slot: 2,
        prepared_slot: 1,
        resolution_mask: initializer.selected_mask,
        trading_mask: 0b1_000,
    };
    let checkpoint =
        ControllerFundingCheckpointV1::prepared(checkpoint_input).expect("Prepared checkpoint");
    let checkpoint_data = checkpoint.encode();
    let checkpoint_key = Pubkey::find_program_address(
        &ControllerFundingCheckpointDerivationV1::new(
            checkpoint_input.release_set,
            checkpoint_input.market,
            checkpoint_input.generation,
            checkpoint_input.manifest,
            checkpoint_input.funding_list,
        )
        .expect("checkpoint derivation")
        .seed_components(),
        &TRADING_CALLER_PROGRAM_ID,
    )
    .0;
    context.set_account(
        &checkpoint_key,
        &AccountSharedData::from(Account {
            lamports: Rent::default().minimum_balance(checkpoint_data.len()),
            data: checkpoint_data.to_vec(),
            owner: TRADING_CALLER_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        }),
    );
    context.warp_to_slot(3).expect("past checkpoint expiry");
    let ledger_account_digest = pre_market_funding_ledger_account_digest_v1(
        fixture.ledger.to_bytes(),
        RESOLUTION_PROGRAM_ID.to_bytes(),
        ledger_before.lamports,
        &ledger_before.data,
    );
    let abort = PreMarketFundingAbortRequestV1 {
        checkpoint_phase: checkpoint.phase() as u8,
        checkpoint_revision: checkpoint.revision(),
        release_set: checkpoint_input.release_set,
        checkpoint: checkpoint_key.to_bytes(),
        checkpoint_digest: hash(&checkpoint_data).to_bytes(),
        market: checkpoint_input.market,
        generation: checkpoint_input.generation,
        manifest: checkpoint_input.manifest,
        funding_list: checkpoint_input.funding_list,
        selected_mask: checkpoint_input.resolution_mask,
        ledger: checkpoint_input.resolution_ledger,
        ledger_account_digest,
        funding_source: checkpoint_input.funding_source,
        rent_credit: checkpoint_input.rent_credit,
        expiry_slot: checkpoint_input.expiry_slot,
    };
    let abort_bytes = abort.encode().expect("abort request");
    let authority_seeds = CallerAuthoritySeedsV1::from_bytes(
        abort.release_set,
        abort.market,
        ExecutionRoleV1::Trading,
        abort.manifest,
        hash(&abort_bytes).to_bytes(),
    )
    .expect("abort authority");
    let authority =
        Pubkey::find_program_address(&authority_seeds.as_slices(), &TRADING_CALLER_PROGRAM_ID).0;
    let abort_accounts = vec![
        AccountMeta::new_readonly(authority, false),
        AccountMeta::new_readonly(TRADING_CALLER_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.caller_programdata, false),
        AccountMeta::new_readonly(RESOLUTION_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.resolution_programdata, false),
        AccountMeta::new_readonly(checkpoint_key, false),
        AccountMeta::new(fixture.ledger, false),
        AccountMeta::new(payer, false),
        AccountMeta::new(fixture.rent_credit, false),
        AccountMeta::new_readonly(fixture.activation, false),
        AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.manifest.raw, false),
        AccountMeta::new_readonly(fixture.manifest.staging, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(sysvar::clock::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
    ];
    let mut dusted = ledger_before.clone();
    dusted.lamports = dusted
        .lamports
        .checked_add(1)
        .expect("one hostile dust lamport");
    context.set_account(&fixture.ledger, &AccountSharedData::from(dusted.clone()));
    let dusted_abort = PreMarketFundingAbortRequestV1 {
        ledger_account_digest: pre_market_funding_ledger_account_digest_v1(
            fixture.ledger.to_bytes(),
            RESOLUTION_PROGRAM_ID.to_bytes(),
            dusted.lamports,
            &dusted.data,
        ),
        ..abort
    };
    let dusted_bytes = dusted_abort.encode().expect("dusted abort request");
    let dusted_seeds = CallerAuthoritySeedsV1::from_bytes(
        dusted_abort.release_set,
        dusted_abort.market,
        ExecutionRoleV1::Trading,
        dusted_abort.manifest,
        hash(&dusted_bytes).to_bytes(),
    )
    .expect("dusted abort authority");
    let mut dusted_accounts = abort_accounts.clone();
    dusted_accounts[0].pubkey =
        Pubkey::find_program_address(&dusted_seeds.as_slices(), &TRADING_CALLER_PROGRAM_ID).0;
    let (dusted_result, _) = process_recorded(
        &mut context,
        &[Instruction {
            program_id: TRADING_CALLER_PROGRAM_ID,
            accounts: dusted_accounts,
            data: dusted_bytes.to_vec(),
        }],
        "pre-market funding abort: surplus ledger dust refuses",
    )
    .await;
    assert!(
        matches!(
            dusted_result,
            Err(TransactionError::InstructionError(
                0,
                InstructionError::Custom(code)
            )) if code == ResolutionError::Funding as u32
        ),
        "surplus ledger dust must refuse as Resolution Funding, got {dusted_result:?}"
    );
    let after_refusal = observed(&mut context, fixture.ledger)
        .await
        .expect("refused ledger remains");
    assert_eq!(after_refusal, dusted, "dust refusal rolls back");
    context.set_account(
        &fixture.ledger,
        &AccountSharedData::from(ledger_before.clone()),
    );
    let source_before = observed(&mut context, payer)
        .await
        .expect("post-refusal funding source");
    let (outcome, returned) = process_recorded(
        &mut context,
        &[Instruction {
            program_id: TRADING_CALLER_PROGRAM_ID,
            accounts: abort_accounts,
            data: abort_bytes.to_vec(),
        }],
        "pre-market funding abort: the expired checkpoint refunds and closes the ledger",
    )
    .await;
    assert!(outcome.is_ok(), "expiry close succeeds");
    let returned = returned.expect("abort receipt");
    assert_eq!(returned.0, TRADING_CALLER_PROGRAM_ID);
    assert_eq!(returned.1.len(), PRE_MARKET_FUNDING_ABORT_RECEIPT_BYTES_V1);
    let receipt =
        PreMarketFundingAbortReceiptV1::decode(&returned.1).expect("canonical abort receipt");
    assert_eq!(receipt.ledger, fixture.ledger.to_bytes());
    assert_eq!(receipt.total_refund_lamports, ledger_before.lamports);
    assert!(observed(&mut context, fixture.ledger).await.is_none());
    let source_after = observed(&mut context, payer).await.expect("funding source");
    let rent_credit_after = observed(&mut context, fixture.rent_credit)
        .await
        .expect("RentCredit");
    assert_eq!(
        source_after.lamports,
        source_before
            .lamports
            .checked_add(receipt.native_principal_refund_lamports)
            .expect("source refund")
            .checked_sub(5_000)
            .expect("transaction fee")
    );
    assert_eq!(
        rent_credit_after.lamports,
        rent_credit_before
            .lamports
            .checked_add(receipt.rent_refund_lamports)
            .expect("rent refund")
    );
}

#[tokio::test]
async fn initializer_reconciles_system_owned_dust_below_and_above_target() {
    for excess in [false, true] {
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
        let projected = project_found_receipt(&mut context, &fixture, found_request).await;
        let funding_keypair = Keypair::new();
        let funding_source = funding_keypair.pubkey();
        context.set_account(
            &funding_source,
            &AccountSharedData::from(Account {
                lamports: 10_000_000_000,
                data: Vec::new(),
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            }),
        );
        let zero = build_pre_market_funding_v2(
            &PreMarketFundingSnapshotV2 {
                resolution_program: required(&mut context, RESOLUTION_PROGRAM_ID).await,
                caller_program: required(&mut context, TRADING_CALLER_PROGRAM_ID).await,
                caller_programdata: required(&mut context, fixture.caller_programdata).await,
                resolution_programdata: required(&mut context, fixture.resolution_programdata)
                    .await,
                funding_source: required(&mut context, funding_source).await,
                ledger: vacant(fixture.ledger),
                rent: required(&mut context, sysvar::rent::ID).await,
                project_found_accounts: found_snapshot(&mut context, &fixture).await,
            },
            found_request,
            projected,
        )
        .expect("zero-dust projection");
        let target = zero.exact_post_lamports;
        let dust = if excess {
            target.checked_add(1).expect("one excess lamport")
        } else {
            target.checked_sub(1).expect("one lamport short")
        };
        context.set_account(
            &fixture.ledger,
            &AccountSharedData::from(Account {
                lamports: dust,
                data: Vec::new(),
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            }),
        );
        let initializer = build_pre_market_funding_v2(
            &PreMarketFundingSnapshotV2 {
                resolution_program: required(&mut context, RESOLUTION_PROGRAM_ID).await,
                caller_program: required(&mut context, TRADING_CALLER_PROGRAM_ID).await,
                caller_programdata: required(&mut context, fixture.caller_programdata).await,
                resolution_programdata: required(&mut context, fixture.resolution_programdata)
                    .await,
                funding_source: required(&mut context, funding_source).await,
                ledger: required(&mut context, fixture.ledger).await,
                rent: required(&mut context, sysvar::rent::ID).await,
                project_found_accounts: found_snapshot(&mut context, &fixture).await,
            },
            found_request,
            projected,
        )
        .expect("dust-bound projection");
        let source_before = observed(&mut context, funding_source)
            .await
            .expect("funding source");
        let credit_before = observed(&mut context, fixture.rent_credit)
            .await
            .expect("RentCredit");
        let mut caller_accounts = initializer.instruction.accounts.clone();
        caller_accounts[0].is_signer = false;
        submit(
            &mut context,
            &[Instruction {
                program_id: TRADING_CALLER_PROGRAM_ID,
                accounts: caller_accounts,
                data: initializer.instruction.data.clone(),
            }],
            &[&funding_keypair],
        )
        .await;
        let ledger = observed(&mut context, fixture.ledger)
            .await
            .expect("initialized ledger");
        let source_after = observed(&mut context, funding_source)
            .await
            .expect("funding source");
        let credit_after = observed(&mut context, fixture.rent_credit)
            .await
            .expect("RentCredit");
        assert_eq!(ledger.owner, RESOLUTION_PROGRAM_ID);
        assert_eq!(ledger.lamports, target);
        assert_eq!(initializer.observed_dust_lamports, dust);
        assert_eq!(
            source_after.lamports,
            source_before
                .lamports
                .checked_sub(initializer.top_up_lamports)
                .expect("exact top-up")
        );
        assert_eq!(
            credit_after.lamports,
            credit_before
                .lamports
                .checked_add(initializer.refund_lamports)
                .expect("exact dust refund")
        );
        assert_eq!(initializer.top_up_lamports, u64::from(!excess));
        assert_eq!(initializer.refund_lamports, u64::from(excess));
    }
}
