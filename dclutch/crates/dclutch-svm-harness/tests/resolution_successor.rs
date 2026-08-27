//! Real-SVM evidence for the successor Resolution controller.
//!
//! The fixture prepares pairwise-distinct compiled Registry, Core, Claims,
//! Trading, Resolution, Custody, and Rent ELFs plus the provenance-pinned
//! receiver/router ELFs and captured provider accounts. The joined route is
//! explicitly unavailable only until the canonical Core infrastructure init
//! and 31-account Found request land; this test therefore cannot yet claim
//! lifecycle execution evidence.

use std::{env, fs, path::PathBuf, str::FromStr};

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    CapabilityEntryV1, CapabilityFundingDerivationV1, CapabilityManifestV1, CompartmentFundingV1,
    FUNDING_STATE_BYTES, FundingAmountsV1, FundingCustodyObservationV1, FundingQuoteV1,
    FundingStateV1, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_core_contract::ContentId as CoreContentId;
use dclutch_market_core_codec::{
    CoreState, Identity as CoreIdentity, MarketCoreStateSeedsV2,
    MarketIdentity as CoreMarketIdentity, Phase as CorePhase, Readiness as CoreReadiness,
};
use dclutch_product_runtime_v2::{
    ContentId as RuntimeProductContentId, PortfolioInputV2, ResultDomainInputV2,
    compile_portfolio_v2, compile_result_domain_v2, portfolio_record_bytes,
    result_domain_record_bytes,
};
use dclutch_product_runtime_v2_admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_BYTES_V2, PRODUCT_RECORD_SCHEMA_ID_V2, ProductRecordV2,
    RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_pyth_svm::{FullPriceUpdateV2, local_validator_release_v1};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATION_PDA_DOMAIN_V1, ArtifactReleaseV1, ArtifactUpgradePolicyV1,
};
use dclutch_registry_svm::RegistryInstructionV1;
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1, PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1,
    ProgramIdentityV1, ProtocolInfrastructureProfileV1,
};
use dclutch_resolution_codec::{
    FundedTransitionActionV3, PRIMARY_CERTIFICATE_SEQUENCE_V3, PYTH_RELEASE_RECORD_SCHEMA_ID_V1,
    RESOLUTION_CERTIFICATE_BYTES_V2, RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
    RESOLUTION_CONTROLLER_RELEASE_ID_V4, ResolutionCertificateKindV2, ResolutionCertificateV2,
};
use dclutch_source_contract::{
    CapacityEnvelope, ContentId as SourceContentId, PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1,
    ProviderReleaseV1, PythAdapterConfigV1, RECOVERY_POLICY_SCHEMA_ID_V2, RecoveryAttemptV2,
    RecoveryPolicyV2, RoundingBoundary, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
    SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2, SourceAccessProfile, SourceCapacityProfileV1,
    SourceMaterialV2, SourceResolutionPhaseV1, SourceResolutionStateV2, SourceSpecV1,
    StatisticKind, StatisticSpecV1, WindowKind, WindowSpecV1,
};
use solana_account::Account;
use solana_program::{
    clock::Clock,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::Signer;
use solana_sdk_ids::{bpf_loader_upgradeable, system_program};
use solana_system_interface::instruction::transfer;
use solana_transaction::{InstructionError, Transaction, TransactionError};

const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x50; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x51; 32]);
const RESOLUTION_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x52; 32]);
const RECEIPT_CALLER_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x53; 32]);
const CLAIMS_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x54; 32]);
const TRADING_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x55; 32]);
const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x56; 32]);
const RENT_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x57; 32]);
const GENERATION: u64 = 1;
const PROVIDER_SLOT: u64 = 460_336_313;
const PUBLISH_TIME: i64 = 1_787_431_680;
const RECOVERY_TIME: i64 = PUBLISH_TIME + 11;
const EXHAUSTION_TIME: i64 = PUBLISH_TIME + 21;
const BOUNTY: u64 = 7;
const CERTIFICATE_DUST: u64 = 3;
const FEED: [u8; 32] = [0x2a; 32];
const PROTOCOL_UPGRADE_AUTHORITY: [u8; 32] = [0xee; 32];

const RECEIVER_ELF: &[u8] =
    include_bytes!("../../../fixtures/pyth/local-upgraded-2026-08-22/receiver.so");
const ROUTER_ELF: &[u8] =
    include_bytes!("../../../fixtures/pyth/local-upgraded-2026-08-22/router.so");
const RECEIVER_CONFIG: &[u8] =
    include_bytes!("../../../fixtures/pyth/local-upgraded-2026-08-22/receiver-config.account");
const PRICE_UPDATE: &[u8] =
    include_bytes!("../../../fixtures/pyth/local-upgraded-2026-08-22/price-update.account");

#[derive(Clone, Copy)]
struct MarketSpec {
    product_record_id: [u8; 32],
    product_id: [u8; 32],
    material_id: [u8; 32],
    capability_manifest_id: [u8; 32],
    release_set_id: [u8; 32],
    tag: u8,
}

#[derive(Clone, Copy)]
struct CaseAccounts {
    state: Pubkey,
    certificate: Pubkey,
}

#[derive(Clone, Copy)]
struct FundedStepAccounts {
    certificate: Pubkey,
    funding: Pubkey,
}

#[derive(Clone, Copy)]
struct LifecycleAccounts {
    state: Pubkey,
    worker: Pubkey,
    recovery: FundedStepAccounts,
    exhaustion: FundedStepAccounts,
    failure: FundedStepAccounts,
}

struct Fixture {
    test: Option<ProgramTest>,
    resolution_programdata: Pubkey,
    activation: Pubkey,
    product_record_id: [u8; 32],
    exhaust_allocation_id: [u8; 32],
    primary: CaseAccounts,
    underfunded: CaseAccounts,
    lifecycle: LifecycleAccounts,
    rollback: LifecycleAccounts,
    exact_funding_rent: u64,
    exact_certificate_rent: u64,
}

fn core_id(bytes: [u8; 32]) -> CoreContentId {
    CoreContentId::new(bytes).expect("nonzero Core content ID")
}

fn core_identity(bytes: [u8; 32]) -> CoreIdentity {
    CoreIdentity::new(bytes).expect("nonzero Core identity")
}

fn runtime_product_id(bytes: [u8; 32]) -> RuntimeProductContentId {
    RuntimeProductContentId::new(bytes).expect("nonzero Runtime Product content ID")
}

fn source_id(bytes: [u8; 32]) -> SourceContentId {
    SourceContentId::new(bytes).expect("nonzero Source content ID")
}

fn protocol_account(owner: Pubkey, data: Vec<u8>) -> Account {
    Account {
        lamports: Rent::default().minimum_balance(data.len()),
        data,
        owner,
        executable: false,
        rent_epoch: 0,
    }
}

fn program_identity(program: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(program.to_bytes()).expect("nonzero program")
}

fn programdata_address(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn loader_program_bytes(programdata: Pubkey) -> Vec<u8> {
    let mut bytes = vec![0_u8; 36];
    bytes[..4].copy_from_slice(&2_u32.to_le_bytes());
    bytes[4..36].copy_from_slice(programdata.as_ref());
    bytes
}

fn loader_programdata_bytes(
    deployment_slot: u64,
    upgrade_authority: [u8; 32],
    elf: &[u8],
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(45 + elf.len());
    bytes.extend_from_slice(&3_u32.to_le_bytes());
    bytes.extend_from_slice(&deployment_slot.to_le_bytes());
    bytes.push(1);
    bytes.extend_from_slice(&upgrade_authority);
    bytes.extend_from_slice(elf);
    bytes
}

fn immutable_loader_programdata_bytes(elf: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(45 + elf.len());
    bytes.extend_from_slice(&3_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u64.to_le_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&[0_u8; 32]);
    bytes.extend_from_slice(elf);
    bytes
}

fn add_upgradeable_program(
    test: &mut ProgramTest,
    program: Pubkey,
    elf: &[u8],
    slot: u64,
    authority: [u8; 32],
) {
    let rent = Rent::default();
    let programdata = programdata_address(program);
    let program_bytes = loader_program_bytes(programdata);
    let programdata_bytes = loader_programdata_bytes(slot, authority, elf);
    test.add_genesis_account(
        program,
        Account {
            lamports: rent.minimum_balance(program_bytes.len()),
            data: program_bytes,
            owner: bpf_loader_upgradeable::ID,
            executable: true,
            rent_epoch: u64::MAX,
        },
    );
    test.add_genesis_account(
        programdata,
        Account {
            lamports: rent.minimum_balance(programdata_bytes.len()),
            data: programdata_bytes,
            owner: bpf_loader_upgradeable::ID,
            executable: false,
            rent_epoch: u64::MAX,
        },
    );
}

fn add_immutable_program(test: &mut ProgramTest, program: Pubkey, elf: &[u8]) {
    let rent = Rent::default();
    let programdata = programdata_address(program);
    let program_bytes = loader_program_bytes(programdata);
    let programdata_bytes = immutable_loader_programdata_bytes(elf);
    test.add_genesis_account(
        program,
        Account {
            lamports: rent.minimum_balance(program_bytes.len()),
            data: program_bytes,
            owner: bpf_loader_upgradeable::ID,
            executable: true,
            rent_epoch: u64::MAX,
        },
    );
    test.add_genesis_account(
        programdata,
        Account {
            lamports: rent.minimum_balance(programdata_bytes.len()),
            data: programdata_bytes,
            owner: bpf_loader_upgradeable::ID,
            executable: false,
            rent_epoch: u64::MAX,
        },
    );
}

fn artifact(program: Pubkey, semantic_release: [u8; 32], elf: &[u8]) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        program_identity(program),
        program_identity(bpf_loader_upgradeable::ID),
        programdata_address(program).to_bytes(),
        core_id(semantic_release),
        hash(elf).to_bytes(),
        0,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("exact local artifact release")
}

fn artifact_id(release: ArtifactReleaseV1) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(hash(&release.to_bytes()).to_bytes()).expect("artifact ID")
}

fn required_semantic_release(variable: &str) -> [u8; 32] {
    let value = env::var(variable).unwrap_or_else(|_| panic!("{variable} is required"));
    assert_eq!(value.len(), 64, "{variable} must be 32-byte lowercase hex");
    assert!(
        value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "{variable} must be lowercase hexadecimal"
    );
    let mut output = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let pair = core::str::from_utf8(pair).expect("ASCII semantic release ID");
        output[index] = u8::from_str_radix(pair, 16).expect("hex semantic release ID");
    }
    assert_ne!(output, [0; 32], "{variable} must be nonzero");
    output
}

fn binding(release: ArtifactReleaseV1) -> ExecutionRoleBindingV1 {
    ExecutionRoleBindingV1::new(release.program(), artifact_id(release))
}

fn require_pairwise_distinct_role_programs(release_set: ExecutionReleaseSetV1) {
    let core = release_set.binding(ExecutionRoleV1::Core).program();
    let claims = release_set.binding(ExecutionRoleV1::Claims).program();
    let trading = release_set.binding(ExecutionRoleV1::Trading).program();
    let resolution = release_set.binding(ExecutionRoleV1::Resolution).program();
    let custody = release_set.binding(ExecutionRoleV1::Custody).program();
    for (left, right) in [
        (core, claims),
        (core, trading),
        (core, resolution),
        (core, custody),
        (claims, trading),
        (claims, resolution),
        (claims, custody),
        (trading, resolution),
        (trading, custody),
        (resolution, custody),
    ] {
        assert_ne!(
            left, right,
            "joined Resolution evidence requires five distinct real role programs"
        );
    }
}

fn add_record(test: &mut ProgramTest, schema: [u8; 32], digest: [u8; 32], data: Vec<u8>) {
    assert_eq!(hash(&data).to_bytes(), digest, "record digest");
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
    test.add_account(raw, protocol_account(REGISTRY_PROGRAM_ID, data));
    test.add_account(staging, Account::new(0, 0, &system_program::ID));
}

fn canonical_market(spec: MarketSpec) -> (Pubkey, Vec<u8>) {
    let provisional = CoreMarketIdentity {
        market_id: core_identity([0xc9; 32]),
        realm_id: core_identity([spec.tag; 32]),
        product_record: core_identity(spec.product_record_id),
        product_id: core_identity(spec.product_id),
        resolution_policy: core_identity(spec.material_id),
        capability_manifest: core_identity(spec.capability_manifest_id),
        selected_release_set: core_identity(spec.release_set_id),
        registry_program: core_identity(REGISTRY_PROGRAM_ID.to_bytes()),
        generation: GENERATION,
    };
    let market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(provisional).as_slices(),
        &CORE_PROGRAM_ID,
    )
    .0;
    let identity = CoreMarketIdentity {
        market_id: core_identity(market.to_bytes()),
        ..provisional
    };
    let bytes = CoreState {
        phase: CorePhase::Open,
        readiness: CoreReadiness::Consumed,
        terminal_winner: 0,
        identity,
        outstanding_capabilities: 0,
        rent_beneficiary: core_identity([0xc3; 32]),
        terminal_receipt: None,
    }
    .encode()
    .expect("open sparse Core state");
    (market, bytes.to_vec())
}

fn fresh_state(market: Pubkey, material_id: [u8; 32]) -> (Pubkey, SourceResolutionStateV2) {
    let (state, bump) = Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
            market.as_ref(),
            &GENERATION.to_le_bytes(),
        ],
        &RESOLUTION_PROGRAM_ID,
    );
    let value = SourceResolutionStateV2::fresh(
        market.to_bytes(),
        GENERATION,
        source_id(material_id),
        [0xd1; 32],
        bump,
        0,
        0,
    )
    .expect("fresh Source state")
    .state();
    (state, value)
}

fn certificate_key(state: Pubkey, kind: u8, sequence: u64) -> Pubkey {
    Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            state.as_ref(),
            &[kind],
            &sequence.to_le_bytes(),
        ],
        &RESOLUTION_PROGRAM_ID,
    )
    .0
}

fn add_case(
    test: &mut ProgramTest,
    spec: MarketSpec,
    certificate_kind: u8,
    certificate_sequence: u64,
) -> CaseAccounts {
    let (market, market_bytes) = canonical_market(spec);
    let (state, state_value) = fresh_state(market, spec.material_id);
    let certificate = certificate_key(state, certificate_kind, certificate_sequence);
    test.add_account(market, protocol_account(CORE_PROGRAM_ID, market_bytes));
    test.add_account(
        state,
        protocol_account(RESOLUTION_PROGRAM_ID, state_value.to_bytes().to_vec()),
    );
    CaseAccounts { state, certificate }
}

fn add_funding_account(
    test: &mut ProgramTest,
    market: Pubkey,
    capability_manifest_id: CoreContentId,
    capability_manifest: CapabilityManifestV1<'_>,
    entry_index: u16,
) -> Pubkey {
    let rent = Rent::default();
    let exact_rent = rent.minimum_balance(FUNDING_STATE_BYTES);
    let custody = FundingCustodyObservationV1::native_only(
        exact_rent
            .checked_mul(2)
            .and_then(|value| value.checked_add(BOUNTY))
            .expect("bounded funding custody"),
        exact_rent,
    )
    .expect("native funding custody");
    let mut funding_value = FundingStateV1::new(
        capability_manifest_id,
        capability_manifest,
        entry_index,
        custody,
    )
    .expect("pending funding");
    funding_value
        .activate(capability_manifest_id, capability_manifest, custody, 1)
        .expect("active funding");
    let derivation = CapabilityFundingDerivationV1::new(
        market.to_bytes(),
        GENERATION,
        capability_manifest_id,
        capability_manifest,
        funding_value,
    )
    .expect("funding derivation");
    let funding =
        Pubkey::find_program_address(&derivation.seed_components(), &RESOLUTION_PROGRAM_ID).0;
    test.add_account(
        funding,
        Account {
            lamports: exact_rent + BOUNTY,
            data: funding_value.to_bytes().to_vec(),
            owner: RESOLUTION_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    funding
}

fn add_lifecycle_case(
    test: &mut ProgramTest,
    spec: MarketSpec,
    capability_manifest: CapabilityManifestV1<'_>,
    occupied_failure: bool,
) -> LifecycleAccounts {
    let capability_manifest_id = core_id(spec.capability_manifest_id);
    let (market, market_bytes) = canonical_market(spec);
    let (state, state_value) = fresh_state(market, spec.material_id);
    let recovery_certificate = certificate_key(state, 2, 1);
    let exhaustion_certificate = certificate_key(state, 3, 2);
    let failure_certificate = certificate_key(state, 4, 3);
    test.add_account(market, protocol_account(CORE_PROGRAM_ID, market_bytes));
    test.add_account(
        state,
        protocol_account(RESOLUTION_PROGRAM_ID, state_value.to_bytes().to_vec()),
    );
    for (certificate, occupied) in [(failure_certificate, occupied_failure)] {
        if !occupied {
            continue;
        }
        test.add_account(
            certificate,
            protocol_account(
                RESOLUTION_PROGRAM_ID,
                vec![0xa5; RESOLUTION_CERTIFICATE_BYTES_V2],
            ),
        );
    }
    let worker = Pubkey::new_from_array([spec.tag.wrapping_add(0x40); 32]);
    test.add_account(worker, Account::new(101, 0, &system_program::ID));
    LifecycleAccounts {
        state,
        worker,
        recovery: FundedStepAccounts {
            certificate: recovery_certificate,
            funding: add_funding_account(
                test,
                market,
                capability_manifest_id,
                capability_manifest,
                0,
            ),
        },
        exhaustion: FundedStepAccounts {
            certificate: exhaustion_certificate,
            funding: add_funding_account(
                test,
                market,
                capability_manifest_id,
                capability_manifest,
                1,
            ),
        },
        failure: FundedStepAccounts {
            certificate: failure_certificate,
            funding: add_funding_account(
                test,
                market,
                capability_manifest_id,
                capability_manifest,
                2,
            ),
        },
    }
}

struct CompiledElves {
    registry: Vec<u8>,
    core: Vec<u8>,
    claims: Vec<u8>,
    trading: Vec<u8>,
    resolution: Vec<u8>,
    custody: Vec<u8>,
    rent: Vec<u8>,
    receipt_caller: Vec<u8>,
}

fn require_compiled_elves() -> CompiledElves {
    let directory = PathBuf::from(
        env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required for compiled Resolution evidence"),
    );
    let resolution = fs::read(directory.join("dclutch_resolution_proof_sbf.so"))
        .expect("read exact compiled Resolution ELF");
    let registry = fs::read(directory.join("dclutch_registry_sbf.so"))
        .expect("read exact compiled Registry ELF");
    let core =
        fs::read(directory.join("dclutch_core_sbf.so")).expect("read exact compiled Core ELF");
    let claims =
        fs::read(directory.join("dclutch_claims_sbf.so")).expect("read exact compiled Claims ELF");
    let trading = fs::read(directory.join("dclutch_trading_sbf.so"))
        .expect("read exact compiled Trading ELF");
    let custody = fs::read(directory.join("dclutch_custody_sbf.so"))
        .expect("read exact compiled Custody ELF");
    let rent =
        fs::read(directory.join("dclutch_rent_sbf.so")).expect("read exact compiled Rent ELF");
    let receipt_caller = fs::read(directory.join("dclutch_resolution_receipt_test_caller_sbf.so"))
        .expect("read exact compiled receipt-caller ELF");
    for (label, elf) in [
        ("Resolution", &resolution),
        ("Registry", &registry),
        ("Core", &core),
        ("Claims", &claims),
        ("Trading", &trading),
        ("Custody", &custody),
        ("Rent", &rent),
        ("Receipt caller", &receipt_caller),
    ] {
        assert_eq!(
            elf.get(..4),
            Some(&[0x7f, b'E', b'L', b'F'][..]),
            "{label} ELF"
        );
        eprintln!("{label} ELF SHA-256: {:?}", hash(elf).to_bytes());
    }
    CompiledElves {
        registry,
        core,
        claims,
        trading,
        resolution,
        custody,
        rent,
        receipt_caller,
    }
}

impl Fixture {
    fn new() -> Self {
        let elves = require_compiled_elves();
        assert_eq!(
            hash(RECEIVER_ELF).to_bytes(),
            [
                0xc5, 0x07, 0x95, 0x59, 0x86, 0x4f, 0xc3, 0x4d, 0xbd, 0x5f, 0xe8, 0x7b, 0x4a, 0xa9,
                0xfb, 0xa3, 0xa1, 0xed, 0x22, 0x69, 0x03, 0x63, 0xec, 0x49, 0x04, 0x49, 0xe8, 0x66,
                0x0e, 0x73, 0xaf, 0x64,
            ],
            "captured receiver ELF pin"
        );
        assert_eq!(
            hash(ROUTER_ELF).to_bytes(),
            [
                0xf9, 0x06, 0x1f, 0x03, 0xa8, 0x1b, 0x89, 0xdb, 0x29, 0xf4, 0x60, 0x36, 0x77, 0xe3,
                0xb3, 0xd8, 0x9b, 0x3b, 0xbf, 0x08, 0xd6, 0x78, 0x27, 0xb2, 0x83, 0x2f, 0x18, 0xa4,
                0xe2, 0xb6, 0x1a, 0xcb,
            ],
            "captured router ELF pin"
        );

        let mut test = ProgramTest::default();
        test.prefer_bpf(true);
        test.set_compute_max_units(1_400_000);
        for (program, elf) in [
            (REGISTRY_PROGRAM_ID, elves.registry.as_slice()),
            (CLAIMS_PROGRAM_ID, elves.claims.as_slice()),
            (TRADING_PROGRAM_ID, elves.trading.as_slice()),
            (RESOLUTION_PROGRAM_ID, elves.resolution.as_slice()),
            (CUSTODY_PROGRAM_ID, elves.custody.as_slice()),
            (RENT_PROGRAM_ID, elves.rent.as_slice()),
        ] {
            add_immutable_program(&mut test, program, elf);
        }
        add_upgradeable_program(
            &mut test,
            CORE_PROGRAM_ID,
            &elves.core,
            0,
            PROTOCOL_UPGRADE_AUTHORITY,
        );
        add_upgradeable_program(
            &mut test,
            RECEIPT_CALLER_PROGRAM_ID,
            &elves.receipt_caller,
            0,
            PROTOCOL_UPGRADE_AUTHORITY,
        );

        let registry_release = artifact(
            REGISTRY_PROGRAM_ID,
            required_semantic_release("DCLUTCH_REGISTRY_SEMANTIC_RELEASE_ID"),
            &elves.registry,
        );
        let core_release = artifact(
            CORE_PROGRAM_ID,
            required_semantic_release("DCLUTCH_CORE_SEMANTIC_RELEASE_ID"),
            &elves.core,
        );
        let claims_release = artifact(
            CLAIMS_PROGRAM_ID,
            required_semantic_release("DCLUTCH_CLAIMS_SEMANTIC_RELEASE_ID"),
            &elves.claims,
        );
        let trading_release = artifact(
            TRADING_PROGRAM_ID,
            required_semantic_release("DCLUTCH_TRADING_SEMANTIC_RELEASE_ID"),
            &elves.trading,
        );
        let resolution_release = artifact(
            RESOLUTION_PROGRAM_ID,
            RESOLUTION_CONTROLLER_RELEASE_ID_V4,
            &elves.resolution,
        );
        let custody_release = artifact(
            CUSTODY_PROGRAM_ID,
            required_semantic_release("DCLUTCH_CUSTODY_SEMANTIC_RELEASE_ID"),
            &elves.custody,
        );
        let rent_release = artifact(
            RENT_PROGRAM_ID,
            required_semantic_release("DCLUTCH_RENT_SEMANTIC_RELEASE_ID"),
            &elves.rent,
        );
        let infrastructure =
            ProtocolInfrastructureProfileV1::new(binding(registry_release), binding(rent_release))
                .expect("distinct Registry/Rent infrastructure profile");
        assert_eq!(
            infrastructure.to_bytes().len(),
            PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1
        );
        let infrastructure_address = Pubkey::find_program_address(
            &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
            &CORE_PROGRAM_ID,
        )
        .0;
        assert_ne!(infrastructure_address, Pubkey::default());
        let release_set = ExecutionReleaseSetV1::new(
            binding(core_release),
            binding(claims_release),
            binding(trading_release),
            binding(resolution_release),
            binding(custody_release),
        )
        .expect("pairwise-distinct release set");
        require_pairwise_distinct_role_programs(release_set);
        let release_set_id = core_id(hash(&release_set.to_bytes()).to_bytes());
        let activation = Pubkey::find_program_address(
            &[ACTIVATION_PDA_DOMAIN_V1, release_set_id.as_bytes()],
            &REGISTRY_PROGRAM_ID,
        )
        .0;

        let synthetic =
            local_validator_release_v1().expect("pinned local-validator Pyth release projection");
        let release = synthetic.release();
        let provider_release_bytes = release.to_bytes();
        let provider_release_id = hash(&provider_release_bytes).to_bytes();
        let receiver = Pubkey::new_from_array(release.receiver_program());
        let receiver_programdata = Pubkey::new_from_array(release.receiver_programdata());
        let receiver_config = Pubkey::new_from_array(release.receiver_config());
        let router = Pubkey::new_from_array(release.router_program());
        let router_programdata = Pubkey::new_from_array(release.router_programdata());
        assert_eq!(programdata_address(receiver), receiver_programdata);
        assert_eq!(programdata_address(router), router_programdata);
        assert_eq!(hash(RECEIVER_CONFIG).to_bytes(), release.config_digest());
        let captured_upgrade_authority =
            Pubkey::from_str("upg8KLALUN7ByDHiBu4wEbMDTC6UnSVFSYfTyGfXuzr")
                .expect("captured public upgrade authority")
                .to_bytes();
        add_upgradeable_program(
            &mut test,
            receiver,
            RECEIVER_ELF,
            0,
            captured_upgrade_authority,
        );
        add_upgradeable_program(&mut test, router, ROUTER_ELF, 0, captured_upgrade_authority);
        test.add_account(
            receiver_config,
            Account {
                lamports: Rent::default().minimum_balance(RECEIVER_CONFIG.len()),
                data: RECEIVER_CONFIG.to_vec(),
                owner: receiver,
                executable: false,
                rent_epoch: 0,
            },
        );
        let price_update = Pubkey::new_from_array([0x5a; 32]);
        test.add_account(
            price_update,
            Account {
                lamports: Rent::default().minimum_balance(PRICE_UPDATE.len()),
                data: PRICE_UPDATE.to_vec(),
                owner: receiver,
                executable: false,
                rent_epoch: 0,
            },
        );

        let coordinate_id = [0xa1; 32];
        let unit_id = [0xa2; 32];
        let product_id = [0xa3; 32];
        let claim_basis_id = [0xa4; 32];
        let liability_basis_id = [0xa5; 32];
        let representation_release_id = [0xa6; 32];
        let mapping_release_id = [0xa7; 32];
        let cuts = [0_i128];
        let coefficients = [1_u64, 1, 1];
        let mut domain_bytes =
            vec![0_u8; result_domain_record_bytes(cuts.len()).expect("runtime domain width")];
        compile_result_domain_v2(
            ResultDomainInputV2 {
                product_id: runtime_product_id(product_id),
                coordinate_domain_id: runtime_product_id(coordinate_id),
                result_unit_id: runtime_product_id(unit_id),
                liability_basis_id: runtime_product_id(liability_basis_id),
                representation_release_id: runtime_product_id(representation_release_id),
                mapping_release_id: runtime_product_id(mapping_release_id),
                cut_denominator: 1,
                cuts: &cuts,
            },
            &mut domain_bytes,
        )
        .expect("canonical runtime Product domain");
        let result_domain_id = hash(&domain_bytes).to_bytes();
        let mut portfolio_bytes = vec![
            0_u8;
            portfolio_record_bytes(coefficients.len())
                .expect("runtime portfolio width")
        ];
        compile_portfolio_v2(
            PortfolioInputV2 {
                product_id: runtime_product_id(product_id),
                result_domain_id: runtime_product_id(result_domain_id),
                claim_basis_id: runtime_product_id(claim_basis_id),
                liability_basis_id: runtime_product_id(liability_basis_id),
                representation_release_id: runtime_product_id(representation_release_id),
                denominator: 1,
                coefficients: &coefficients,
            },
            &mut portfolio_bytes,
        )
        .expect("canonical runtime Product portfolio");
        let portfolio_id = hash(&portfolio_bytes).to_bytes();
        let mut product_bytes = [0_u8; PRODUCT_RECORD_BYTES_V2];
        ProductRecordV2::new(
            runtime_product_id(product_id),
            runtime_product_id(result_domain_id),
            runtime_product_id(portfolio_id),
        )
        .encode_into(&mut product_bytes)
        .expect("canonical runtime Product root");
        let product_record_id = hash(&product_bytes).to_bytes();
        assert_eq!(coefficients.len(), cuts.len() + 2);

        let capacity = SourceCapacityProfileV1::new(
            CapacityEnvelope::Measured,
            1,
            1,
            source_id([0xb1; 32]),
            source_id([0xb2; 32]),
            256,
            0,
        )
        .expect("Source capacity");
        let capacity_id = source_id(hash(&capacity.to_bytes()).to_bytes());
        let provider = ProviderReleaseV1::new(
            source_id([0xb3; 32]),
            source_id(PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1),
            source_id(provider_release_id),
            source_id(release.price_update_codec_id()),
            source_id(release.adapter_id()),
        );
        let provider_id = source_id(hash(&provider.to_bytes()).to_bytes());
        let adapter = PythAdapterConfigV1::new(FEED, -8, 100).expect("Pyth adapter");
        let adapter_id = source_id(hash(&adapter.to_bytes()).to_bytes());
        let source = SourceSpecV1::new(
            source_id(coordinate_id),
            source_id(unit_id),
            provider_id,
            SourceAccessProfile::PythTerminalOneTransaction,
            adapter_id,
            capacity_id,
        );
        let source_spec_id = source_id(hash(&source.to_bytes()).to_bytes());
        // The market sells a closed period, not one exact second. The upper
        // bound stays at `PUBLISH_TIME` because `RECOVERY_TIME` and
        // `EXHAUSTION_TIME` are `end + max_age_seconds` offsets from it; the
        // false assumption this fixture carried was that the window has no
        // width at all, which is the shape no real provider cadence can answer.
        let window = WindowSpecV1::new(
            source_spec_id,
            WindowKind::Terminal,
            PUBLISH_TIME - 300,
            PUBLISH_TIME,
            10,
            2,
            source_id([0xb4; 32]),
        )
        .expect("terminal window");
        let window_id = source_id(hash(&window.to_bytes()).to_bytes());
        let statistic = StatisticSpecV1::new(
            source_id(unit_id),
            source_id(unit_id),
            StatisticKind::TerminalSample,
            RoundingBoundary::ExactRational,
            1,
            0,
            capacity_id,
            source_id([0xb5; 32]),
            capacity,
        )
        .expect("terminal statistic");
        let statistic_id = source_id(hash(&statistic.to_bytes()).to_bytes());
        let recovery_allocation_id = [0xd2; 32];
        let recovery_policy = RecoveryPolicyV2::new(
            capacity_id,
            [
                Some(
                    RecoveryAttemptV2::new(
                        source_spec_id,
                        provider_id,
                        PUBLISH_TIME + 20,
                        source_id(recovery_allocation_id),
                    )
                    .expect("positive recovery attempt"),
                ),
                None,
                None,
                None,
            ],
            1,
        )
        .expect("one ordered Product-free recovery");
        let recovery_policy_bytes = recovery_policy.to_bytes();
        let recovery_policy_id = source_id(hash(&recovery_policy_bytes).to_bytes());
        let exhaust_allocation_id = recovery_policy_id.to_bytes();
        let failure_policy_release = source_id([0xb6; 32]);
        let material = SourceMaterialV2::new(
            source_id(product_record_id),
            source_spec_id,
            window_id,
            statistic_id,
            Some(recovery_policy_id),
            failure_policy_release,
        );
        material
            .validate_source_graph(
                source_spec_id,
                source,
                window_id,
                window,
                statistic_id,
                statistic,
                Some(recovery_policy_id),
                failure_policy_release,
            )
            .expect("joined compact Source material graph");
        let material_bytes = material.to_bytes();
        let material_id = hash(&material_bytes).to_bytes();

        let exact_funding_rent = Rent::default().minimum_balance(FUNDING_STATE_BYTES);
        let exact_certificate_rent =
            Rent::default().minimum_balance(RESOLUTION_CERTIFICATE_BYTES_V2);
        let funding_quote = FundingQuoteV1::new(
            FundingAmountsV1::new(
                CompartmentFundingV1::native_lamports(exact_funding_rent).expect("funding rent"),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::native_lamports(BOUNTY).expect("worker bounty"),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
            )
            .expect("typed funding amounts"),
            None,
        )
        .expect("native funding quote");
        let entries = [
            CapabilityEntryV1::new(
                core_id([0xd3; 32]),
                core_id(RESOLUTION_CONTROLLER_RELEASE_ID_V4),
                core_id(recovery_allocation_id),
                core_id([0xd5; 32]),
                core_id([0xd6; 32]),
                core_id([0xd7; 32]),
                ActivationPolicy::RequiredAtFounding,
                0,
                0,
                [0; MAX_DEPENDENCIES_PER_CAPABILITY],
                funding_quote,
            )
            .expect("recovery funding entry"),
            CapabilityEntryV1::new(
                core_id([0xd4; 32]),
                core_id(RESOLUTION_CONTROLLER_RELEASE_ID_V4),
                core_id(exhaust_allocation_id),
                core_id([0xd5; 32]),
                core_id([0xd6; 32]),
                core_id([0xd7; 32]),
                ActivationPolicy::RequiredAtFounding,
                0,
                0,
                [0; MAX_DEPENDENCIES_PER_CAPABILITY],
                funding_quote,
            )
            .expect("exhaustion funding entry"),
            CapabilityEntryV1::new(
                core_id([0xd8; 32]),
                core_id(RESOLUTION_CONTROLLER_RELEASE_ID_V4),
                core_id(material_id),
                core_id([0xd5; 32]),
                core_id([0xd6; 32]),
                core_id([0xd7; 32]),
                ActivationPolicy::RequiredAtFounding,
                0,
                0,
                [0; MAX_DEPENDENCIES_PER_CAPABILITY],
                funding_quote,
            )
            .expect("failure funding entry"),
        ];
        let mut capability_manifest_bytes =
            vec![0_u8; MANIFEST_HEADER_BYTES + 3 * CAPABILITY_ENTRY_BYTES];
        CapabilityManifestV1::encode_into(&entries, &mut capability_manifest_bytes)
            .expect("capability manifest");
        let capability_manifest_id = hash(&capability_manifest_bytes).to_bytes();
        add_record(
            &mut test,
            SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
            material_id,
            material_bytes.to_vec(),
        );
        add_record(
            &mut test,
            RECOVERY_POLICY_SCHEMA_ID_V2,
            recovery_policy_id.to_bytes(),
            recovery_policy_bytes.to_vec(),
        );
        add_record(
            &mut test,
            PRODUCT_RECORD_SCHEMA_ID_V2,
            product_record_id,
            product_bytes.to_vec(),
        );
        add_record(
            &mut test,
            RESULT_DOMAIN_SCHEMA_ID_V2,
            result_domain_id,
            domain_bytes,
        );
        add_record(
            &mut test,
            PORTFOLIO_SCHEMA_ID_V2,
            portfolio_id,
            portfolio_bytes,
        );
        add_record(
            &mut test,
            PYTH_RELEASE_RECORD_SCHEMA_ID_V1,
            provider_release_id,
            provider_release_bytes.to_vec(),
        );
        add_record(
            &mut test,
            CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            capability_manifest_id,
            capability_manifest_bytes.clone(),
        );
        let manifest = CapabilityManifestV1::decode(&capability_manifest_bytes)
            .expect("capability manifest view");
        let primary = add_case(
            &mut test,
            MarketSpec {
                product_record_id,
                product_id,
                material_id,
                capability_manifest_id,
                release_set_id: release_set_id.to_bytes(),
                tag: 0xe1,
            },
            1,
            PRIMARY_CERTIFICATE_SEQUENCE_V3,
        );
        let lifecycle = add_lifecycle_case(
            &mut test,
            MarketSpec {
                product_record_id,
                product_id,
                material_id,
                capability_manifest_id,
                release_set_id: release_set_id.to_bytes(),
                tag: 0xe2,
            },
            manifest,
            false,
        );
        let rollback = add_lifecycle_case(
            &mut test,
            MarketSpec {
                product_record_id,
                product_id,
                material_id,
                capability_manifest_id,
                release_set_id: release_set_id.to_bytes(),
                tag: 0xe3,
            },
            manifest,
            false,
        );
        let underfunded = add_case(
            &mut test,
            MarketSpec {
                product_record_id,
                product_id,
                material_id,
                capability_manifest_id,
                release_set_id: release_set_id.to_bytes(),
                tag: 0xe4,
            },
            1,
            PRIMARY_CERTIFICATE_SEQUENCE_V3,
        );
        assert_ne!(
            lifecycle.recovery.certificate,
            lifecycle.exhaustion.certificate
        );
        assert_ne!(
            lifecycle.exhaustion.certificate,
            lifecycle.failure.certificate
        );

        Self {
            test: Some(test),
            resolution_programdata: programdata_address(RESOLUTION_PROGRAM_ID),
            activation,
            product_record_id,
            exhaust_allocation_id,
            primary,
            underfunded,
            lifecycle,
            rollback,
            exact_funding_rent,
            exact_certificate_rent,
        }
    }
}

fn primary_instruction(_fixture: &Fixture, _case: CaseAccounts) -> Instruction {
    panic!("Runtime V2 direct primary ABI has not been frozen")
}

fn funded_caller_instruction(
    _fixture: &Fixture,
    _case: LifecycleAccounts,
    _step: FundedStepAccounts,
    _action: FundedTransitionActionV3,
    _fail_after_receipt: bool,
) -> Instruction {
    panic!("Runtime V2 funded direct ABI and return receipt have not been frozen")
}

async fn submit(
    context: &mut ProgramTestContext,
    instruction: Instruction,
) -> Result<(), BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    context.banks_client.process_transaction(transaction).await
}

async fn prepay_certificate(
    context: &mut ProgramTestContext,
    certificate: Pubkey,
    exact_rent: u64,
) {
    prepay_certificate_amount(
        context,
        certificate,
        exact_rent
            .checked_add(CERTIFICATE_DUST)
            .expect("bounded certificate prepayment"),
    )
    .await;
}

async fn prepay_certificate_amount(
    context: &mut ProgramTestContext,
    certificate: Pubkey,
    amount: u64,
) {
    let payer = context.payer.pubkey();
    submit(context, transfer(&payer, &certificate, amount))
        .await
        .expect("prepay deterministic certificate PDA");
    let prepaid = observed(context, certificate).await;
    assert_eq!(prepaid.owner, system_program::ID);
    assert!(prepaid.data.is_empty());
    assert_eq!(prepaid.lamports, amount);
}

async fn observed(context: &mut ProgramTestContext, key: Pubkey) -> Account {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("read account")
        .expect("account exists")
}

async fn snapshot_funded(
    context: &mut ProgramTestContext,
    case: LifecycleAccounts,
    step: FundedStepAccounts,
) -> (Account, Account, Account, Account) {
    (
        observed(context, case.state).await,
        observed(context, step.certificate).await,
        observed(context, step.funding).await,
        observed(context, case.worker).await,
    )
}

fn set_clock(context: &mut ProgramTestContext, unix_timestamp: i64) {
    let clock = Clock {
        slot: PROVIDER_SLOT,
        epoch_start_timestamp: 0,
        epoch: 0,
        leader_schedule_epoch: 0,
        unix_timestamp,
    };
    context.set_sysvar(&clock);
}

fn assert_exact_bounty_compartments(account: &Account, exact_funding_rent: u64) {
    let funding = FundingStateV1::decode(&account.data).expect("canonical FundingState post-state");
    assert_eq!(funding.remaining().rent().amount(), 0);
    assert_eq!(funding.released().rent().amount(), exact_funding_rent);
    assert_eq!(funding.remaining().bounty().amount(), 0);
    assert_eq!(funding.released().bounty().amount(), BOUNTY);
}

#[tokio::test]
#[ignore = "requires canonical Core infrastructure init and 31-account Found request"]
async fn compiled_resolution_executes_primary_recovery_failure_and_atomic_refusal() {
    let mut fixture = Fixture::new();
    let mut context = fixture
        .test
        .take()
        .expect("ProgramTest")
        .start_with_context()
        .await;
    context
        .warp_to_slot(PROVIDER_SLOT)
        .expect("warp to the captured update's execution slot");
    set_clock(&mut context, PUBLISH_TIME);

    submit(
        &mut context,
        Instruction {
            program_id: REGISTRY_PROGRAM_ID,
            accounts: vec![
                AccountMeta::new_readonly(fixture.activation, false),
                AccountMeta::new_readonly(RESOLUTION_PROGRAM_ID, false),
                AccountMeta::new_readonly(fixture.resolution_programdata, false),
            ],
            data: RegistryInstructionV1::Reauthenticate(ExecutionRoleV1::Resolution)
                .to_bytes()
                .to_vec(),
        },
    )
    .await
    .expect("compiled Registry reauthenticates the exact Resolution ELF");

    prepay_certificate(
        &mut context,
        fixture.primary.certificate,
        fixture.exact_certificate_rent,
    )
    .await;
    submit(&mut context, primary_instruction(&fixture, fixture.primary))
        .await
        .expect("compiled Resolution admits the captured primary Pyth update");
    let primary_state =
        SourceResolutionStateV2::decode(&observed(&mut context, fixture.primary.state).await.data)
            .expect("primary Runtime V2 Source state");
    let primary_certificate = ResolutionCertificateV2::decode(
        &observed(&mut context, fixture.primary.certificate)
            .await
            .data,
    )
    .expect("primary certificate");
    let update = FullPriceUpdateV2::parse(PRICE_UPDATE).expect("captured update");
    assert_eq!(primary_state.phase(), SourceResolutionPhaseV1::Resolved);
    assert_eq!(
        primary_certificate.kind,
        ResolutionCertificateKindV2::ResolutionSuccess
    );
    assert_eq!(
        primary_certificate.product_record_digest,
        fixture.product_record_id
    );
    assert_eq!(
        primary_certificate.result_numerator,
        i128::from(update.price())
    );
    assert_eq!(primary_certificate.result_denominator, 1);
    assert_eq!(primary_certificate.observed_at, PUBLISH_TIME as u64);
    assert_eq!(
        primary_certificate.receipt_account,
        fixture.primary.certificate.to_bytes()
    );
    let primary_certificate_account = observed(&mut context, fixture.primary.certificate).await;
    assert_eq!(primary_certificate_account.owner, RESOLUTION_PROGRAM_ID);
    assert_eq!(
        primary_certificate_account.lamports,
        fixture.exact_certificate_rent + CERTIFICATE_DUST
    );
    let primary_replay_before = (
        observed(&mut context, fixture.primary.state).await,
        primary_certificate_account,
    );
    let primary_replay = submit(&mut context, primary_instruction(&fixture, fixture.primary)).await;
    assert!(
        matches!(
            primary_replay,
            Err(BanksClientError::TransactionError(
                TransactionError::InstructionError(0, InstructionError::Custom(12))
            ))
        ),
        "primary certificate replay refuses against terminal Source state: {primary_replay:?}"
    );
    assert_eq!(
        (
            observed(&mut context, fixture.primary.state).await,
            observed(&mut context, fixture.primary.certificate).await,
        ),
        primary_replay_before
    );

    let underfunded_amount = fixture
        .exact_certificate_rent
        .checked_sub(1)
        .expect("positive certificate rent");
    prepay_certificate_amount(
        &mut context,
        fixture.underfunded.certificate,
        underfunded_amount,
    )
    .await;
    let underfunded_before = (
        observed(&mut context, fixture.underfunded.state).await,
        observed(&mut context, fixture.underfunded.certificate).await,
    );
    let underfunded = submit(
        &mut context,
        primary_instruction(&fixture, fixture.underfunded),
    )
    .await;
    assert!(
        matches!(
            underfunded,
            Err(BanksClientError::TransactionError(
                TransactionError::InstructionError(0, InstructionError::Custom(2))
            ))
        ),
        "under-rent deterministic certificate refuses at the final output gate: {underfunded:?}"
    );
    assert_eq!(
        (
            observed(&mut context, fixture.underfunded.state).await,
            observed(&mut context, fixture.underfunded.certificate).await,
        ),
        underfunded_before,
        "under-rent refusal preserves Source and prepaid system account"
    );

    set_clock(&mut context, RECOVERY_TIME);
    prepay_certificate(
        &mut context,
        fixture.lifecycle.recovery.certificate,
        fixture.exact_certificate_rent,
    )
    .await;
    let recovery_funding_before = observed(&mut context, fixture.lifecycle.recovery.funding).await;
    let recovery_worker_before = observed(&mut context, fixture.lifecycle.worker).await;
    submit(
        &mut context,
        funded_caller_instruction(
            &fixture,
            fixture.lifecycle,
            fixture.lifecycle.recovery,
            FundedTransitionActionV3::FailNext,
            false,
        ),
    )
    .await
    .expect("compiled Resolution executes one funded ordered recovery");
    let recovery_state = SourceResolutionStateV2::decode(
        &observed(&mut context, fixture.lifecycle.state).await.data,
    )
    .expect("recovery Source state");
    let recovery_certificate = ResolutionCertificateV2::decode(
        &observed(&mut context, fixture.lifecycle.recovery.certificate)
            .await
            .data,
    )
    .expect("recovery certificate");
    let recovery_funding_after = observed(&mut context, fixture.lifecycle.recovery.funding).await;
    let recovery_worker_after = observed(&mut context, fixture.lifecycle.worker).await;
    assert_eq!(recovery_state.phase(), SourceResolutionPhaseV1::Recovery);
    assert_eq!(
        recovery_certificate.kind,
        ResolutionCertificateKindV2::RecoveryAdvanced
    );
    assert_eq!(
        recovery_certificate.product_record_digest,
        fixture.product_record_id
    );
    assert_eq!(recovery_certificate.attempt_index, 1);
    assert_eq!(
        recovery_certificate.receipt_account,
        fixture.lifecycle.recovery.certificate.to_bytes()
    );
    assert_eq!(recovery_certificate.work_paid, BOUNTY);
    assert_eq!(recovery_certificate.funding_remaining, 0);
    assert_eq!(
        recovery_funding_before.lamports - recovery_funding_after.lamports,
        BOUNTY
    );
    assert_eq!(recovery_funding_after.lamports, fixture.exact_funding_rent);
    assert_exact_bounty_compartments(&recovery_funding_after, fixture.exact_funding_rent);
    assert_eq!(
        recovery_worker_after.lamports - recovery_worker_before.lamports,
        BOUNTY
    );

    set_clock(&mut context, EXHAUSTION_TIME);
    prepay_certificate(
        &mut context,
        fixture.lifecycle.exhaustion.certificate,
        fixture.exact_certificate_rent,
    )
    .await;
    let exhaustion_funding_before =
        observed(&mut context, fixture.lifecycle.exhaustion.funding).await;
    let exhaustion_worker_before = observed(&mut context, fixture.lifecycle.worker).await;
    submit(
        &mut context,
        funded_caller_instruction(
            &fixture,
            fixture.lifecycle,
            fixture.lifecycle.exhaustion,
            FundedTransitionActionV3::Exhaust,
            false,
        ),
    )
    .await
    .expect("compiled Resolution executes exact funded exhaustion");
    let exhausted_state = SourceResolutionStateV2::decode(
        &observed(&mut context, fixture.lifecycle.state).await.data,
    )
    .expect("exhausted Source state");
    let exhaustion_certificate = ResolutionCertificateV2::decode(
        &observed(&mut context, fixture.lifecycle.exhaustion.certificate)
            .await
            .data,
    )
    .expect("exhaustion certificate");
    let exhaustion_funding_after =
        observed(&mut context, fixture.lifecycle.exhaustion.funding).await;
    let exhaustion_worker_after = observed(&mut context, fixture.lifecycle.worker).await;
    assert_eq!(exhausted_state.phase(), SourceResolutionPhaseV1::Exhausted);
    assert_eq!(
        exhaustion_certificate.kind,
        ResolutionCertificateKindV2::Exhausted
    );
    assert_eq!(
        exhaustion_certificate.product_record_digest,
        fixture.product_record_id
    );
    assert_ne!(exhaustion_certificate.route, [0; 32]);
    assert_eq!(exhaustion_certificate.attempt_index, 1);
    assert_eq!(exhaustion_certificate.selector, 0);
    assert_eq!(exhaustion_certificate.observed_at, EXHAUSTION_TIME as u64);
    assert_eq!(
        exhaustion_certificate.funding_allocation,
        fixture.exhaust_allocation_id
    );
    assert_eq!(
        exhaustion_certificate.receipt_account,
        fixture.lifecycle.exhaustion.certificate.to_bytes()
    );
    assert_eq!(exhaustion_certificate.work_paid, BOUNTY);
    assert_eq!(exhaustion_certificate.funding_remaining, 0);
    assert_eq!(
        exhaustion_funding_before.lamports - exhaustion_funding_after.lamports,
        BOUNTY
    );
    assert_eq!(
        exhaustion_funding_after.lamports,
        fixture.exact_funding_rent
    );
    assert_exact_bounty_compartments(&exhaustion_funding_after, fixture.exact_funding_rent);
    assert_eq!(
        exhaustion_worker_after.lamports - exhaustion_worker_before.lamports,
        BOUNTY
    );

    set_clock(&mut context, EXHAUSTION_TIME + 1);
    prepay_certificate(
        &mut context,
        fixture.lifecycle.failure.certificate,
        fixture.exact_certificate_rent,
    )
    .await;
    let failure_funding_before = observed(&mut context, fixture.lifecycle.failure.funding).await;
    let failure_worker_before = observed(&mut context, fixture.lifecycle.worker).await;
    submit(
        &mut context,
        funded_caller_instruction(
            &fixture,
            fixture.lifecycle,
            fixture.lifecycle.failure,
            FundedTransitionActionV3::CommitFailure,
            false,
        ),
    )
    .await
    .expect("compiled Resolution commits Product's explicit failure result");
    let failure_state = SourceResolutionStateV2::decode(
        &observed(&mut context, fixture.lifecycle.state).await.data,
    )
    .expect("failure Source state");
    let failure_certificate = ResolutionCertificateV2::decode(
        &observed(&mut context, fixture.lifecycle.failure.certificate)
            .await
            .data,
    )
    .expect("failure certificate");
    let failure_funding = observed(&mut context, fixture.lifecycle.failure.funding).await;
    let failure_worker = observed(&mut context, fixture.lifecycle.worker).await;
    assert_eq!(
        failure_state.phase(),
        SourceResolutionPhaseV1::FailureCommitted
    );
    assert_eq!(
        failure_certificate.kind,
        ResolutionCertificateKindV2::ResolutionFailure
    );
    assert_eq!(
        failure_certificate.product_record_digest,
        fixture.product_record_id
    );
    assert_eq!(failure_certificate.attempt_index, 1);
    assert_eq!(
        failure_certificate.receipt_account,
        fixture.lifecycle.failure.certificate.to_bytes()
    );
    assert_eq!(failure_certificate.selector, 2);
    assert_eq!(failure_certificate.route, [0; 32]);
    assert_eq!(failure_certificate.observed_at, 0);
    assert_eq!(failure_certificate.work_paid, BOUNTY);
    assert_eq!(failure_funding.lamports, fixture.exact_funding_rent);
    assert_exact_bounty_compartments(&failure_funding, fixture.exact_funding_rent);
    assert_eq!(
        failure_funding_before.lamports - failure_funding.lamports,
        BOUNTY
    );
    assert_eq!(
        failure_worker.lamports - failure_worker_before.lamports,
        BOUNTY
    );

    set_clock(&mut context, RECOVERY_TIME);
    prepay_certificate(
        &mut context,
        fixture.rollback.recovery.certificate,
        fixture.exact_certificate_rent,
    )
    .await;
    submit(
        &mut context,
        funded_caller_instruction(
            &fixture,
            fixture.rollback,
            fixture.rollback.recovery,
            FundedTransitionActionV3::FailNext,
            false,
        ),
    )
    .await
    .expect("rollback lineage enters funded recovery");
    set_clock(&mut context, EXHAUSTION_TIME);
    prepay_certificate(
        &mut context,
        fixture.rollback.exhaustion.certificate,
        fixture.exact_certificate_rent,
    )
    .await;
    submit(
        &mut context,
        funded_caller_instruction(
            &fixture,
            fixture.rollback,
            fixture.rollback.exhaustion,
            FundedTransitionActionV3::Exhaust,
            false,
        ),
    )
    .await
    .expect("rollback lineage reaches exhausted state");
    set_clock(&mut context, EXHAUSTION_TIME + 1);
    prepay_certificate(
        &mut context,
        fixture.rollback.failure.certificate,
        fixture.exact_certificate_rent,
    )
    .await;
    let rollback_before =
        snapshot_funded(&mut context, fixture.rollback, fixture.rollback.failure).await;
    let refusal = submit(
        &mut context,
        funded_caller_instruction(
            &fixture,
            fixture.rollback,
            fixture.rollback.failure,
            FundedTransitionActionV3::CommitFailure,
            true,
        ),
    )
    .await;
    assert!(
        matches!(
            refusal,
            Err(BanksClientError::TransactionError(
                TransactionError::InstructionError(0, InstructionError::Custom(5))
            ))
        ),
        "caller validates the complete funded receipt before deliberate refusal: {refusal:?}"
    );
    assert_eq!(
        snapshot_funded(&mut context, fixture.rollback, fixture.rollback.failure).await,
        rollback_before,
        "SVM rollback covers Source, certificate, funding, and worker state"
    );
}
