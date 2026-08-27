// Real-SVM evidence for the current Resolution terminal-retirement waist.
//
// The fixture loads compiled Registry, Core, Resolution, and Custody ELFs. It
// executes exact Resolution funding creation/readiness into canonical Custody
// opening, and separately starts from an authenticated provider-produced
// terminal Source/certificate boundary to execute chain-derived terminal
// admission and closure. A deliberately stale retirement instruction proves
// rollback of the physical close across Core, Source, all three Funds, the
// closure output, and the immutable RentCredit beneficiary.

use std::{env, fs, path::PathBuf};

#[path = "support/pyth_provider.rs"]
#[allow(dead_code)]
mod pyth_provider;

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    CapabilityEntryV1, CapabilityFundingDerivationV1, CapabilityManifestV1, CompartmentFundingV1,
    ContentId as CapabilityContentId, FUNDING_STATE_BYTES, FundingAmountsV1,
    FundingCustodyObservationV1, FundingQuoteV1, FundingStateV1, FundingStatus,
    MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_core_contract::ContentId as CoreContentId;
use dclutch_custody_contract::{
    CallerRoleV1, CompartmentV1, ContextV1, CustodyAuthoritySeedsV1, CustodyReplaySeedsV1,
    CustodyReplayV1, CustodyRequestV1, CustodyVaultSeedsV1, OperationV1,
};
use dclutch_market_core_codec::{
    Action, CoreState, Identity as CoreIdentity, MarketCoreStateSeedsV2, MarketIdentity, Phase,
    Readiness, Request,
};
use dclutch_market_open_v1_operator::{
    RegistryOpenMarketContinuationStateV1, build_registry_open_market_continuation_v1,
};
use dclutch_product_runtime_v2::{
    ContentId as ProductContentId, PortfolioInputV2, ResultDomainInputV2, ResultDomainV2,
    compile_portfolio_v2, compile_result_domain_v2, portfolio_record_bytes,
    result_domain_record_bytes,
};
use dclutch_product_runtime_v2_admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_BYTES_V2, PRODUCT_RECORD_SCHEMA_ID_V2, ProductRecordV2,
    RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_provider_transport_v3_operator::{
    ProviderExecuteDeploymentV3, ProviderExecuteIntentV3, ProviderExecuteSnapshotV3,
    ProviderSubmitDeploymentV3, ProviderSubmitIntentV3, ProviderSubmitSnapshotV3,
    build_provider_execute_v3, build_provider_submit_v3,
};
use dclutch_pyth_svm::{
    FullPriceUpdateV2, PYTH_RELEASE_V1_ENCODED_LEN, PythReleaseV1, VerifiedEncodedVaaV1,
};
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1, RealmV1Input,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ARTIFACT_RELEASE_SCHEMA_ID_V1, ActivatedExecutionReleaseSetV1, ArtifactActivationInputV1,
    ArtifactReleaseV1, ArtifactUpgradePolicyV1, DeploymentObservationV1,
    activate_execution_role_into_v1, initialize_activation_cache_v1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, CallerAuthoritySeedsV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1,
    ExecutionRoleV1, PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1, ProgramIdentityV1,
    ProtocolInfrastructureProfileV1,
};
use dclutch_rent_contract::{RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority, RentCreditV1};
use dclutch_resolution_codec::{
    PROVIDER_UPDATE_LIFECYCLE_BYTES_V3, PYTH_RELEASE_RECORD_SCHEMA_ID_V1,
    ProviderUpdateLifecycleV3, ProviderUpdateStatusV3, RESOLUTION_CERTIFICATE_BYTES_V2,
    RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3, RESOLUTION_CONTROLLER_RELEASE_ID_V4,
    ResolutionCertificateKindV2, ResolutionCertificateV2, SOURCE_CLOSURE_RECEIPT_BYTES_V2,
    SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V2, SourceClosureReceiptV2,
};
use dclutch_resolution_core_v3_operator::{
    Finality, Observation, ObservedAccount, ResolutionAdmitTerminalSnapshotV3,
    ResolutionCloseFundSnapshotV3, ResolutionCreateFundSnapshotV3,
    ResolutionVerifyFundReadySnapshotV3, build_resolution_admit_terminal_v3,
    build_resolution_close_fund_v3, build_resolution_create_fund_v3,
    build_resolution_verify_fund_ready_v3, validate_resolution_close_fund_report_v3,
    validate_resolution_create_fund_report_v3, validate_resolution_verify_fund_ready_report_v3,
};
use dclutch_source_contract::{
    CapacityEnvelope, ContentId as SourceContentId, PROVIDER_RELEASE_SCHEMA_ID_V1,
    PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1, ProviderReleaseV1, PythAdapterConfigV1,
    RECOVERY_POLICY_SCHEMA_ID_V2, RecoveryAttemptV2, RecoveryPolicyV2, RoundingBoundary,
    SOURCE_FAILURE_POLICY_RELEASE_ID_V2, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
    SOURCE_RESOLUTION_STATE_BYTES_V2, SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
    SOURCE_SPEC_SCHEMA_ID_V1, STATISTIC_SPEC_SCHEMA_ID_V1, SourceAccessProfile,
    SourceCapacityProfileV1, SourceMaterialV2, SourceResolutionPhaseV1, SourceResolutionStateV2,
    SourceSpecV1, StatisticKind, StatisticSpecV1, WINDOW_SPEC_SCHEMA_ID_V1, WindowKind,
    WindowSpecV1,
};
use dclutch_token_svm::{LEGACY_TOKEN_PROGRAM_ID, PRODUCTION_ADAPTER_RELEASES};
use solana_account::{Account, AccountSharedData};
use solana_program::{
    clock::Clock,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_option::COption;
use solana_program_pack::Pack;
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, system_program, sysvar};
use solana_system_interface::instruction::transfer;
use solana_transaction::Transaction;
use spl_token_interface::state::Mint as SplMint;

const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x71; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x72; 32]);
const RESOLUTION_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x73; 32]);
const RENT_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x74; 32]);
const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x75; 32]);
const GENERATION: u64 = 7;
const TERMINAL_SEQUENCE: u64 = 1;
const TERMINAL_TIME: i64 = 1_787_431_680;
const BOUNTY: u64 = 7;

struct Elves {
    core: Vec<u8>,
    custody: Vec<u8>,
    registry: Vec<u8>,
    resolution: Vec<u8>,
}

#[derive(Clone, Copy)]
struct RecordPair {
    raw: Pubkey,
    staging: Pubkey,
}

struct Fixture {
    test: Option<ProgramTest>,
    provider: pyth_provider::ProviderAddresses,
    update: Keypair,
    release_set: [u8; 32],
    market: Pubkey,
    activation: Pubkey,
    infrastructure: Pubkey,
    registry_programdata: Pubkey,
    registry_artifact: RecordPair,
    core_programdata: Pubkey,
    custody_programdata: Pubkey,
    resolution_programdata: Pubkey,
    realm: [u8; 32],
    realm_record: RecordPair,
    mint: Pubkey,
    replay: Pubkey,
    vault: Pubkey,
    custody_authority: Pubkey,
    source_material: RecordPair,
    source_spec: RecordPair,
    provider_release: RecordPair,
    adapter_config: RecordPair,
    window: RecordPair,
    statistic: RecordPair,
    pyth_release: RecordPair,
    capability_manifest: RecordPair,
    recovery_policy: RecordPair,
    product: RecordPair,
    domain: RecordPair,
    portfolio: RecordPair,
    source: Pubkey,
    funding: [Pubkey; 3],
    certificate: Pubkey,
    closure: Pubkey,
    rent_credit: Pubkey,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RetirementSnapshot {
    market: Option<Account>,
    source: Option<Account>,
    funding: [Option<Account>; 3],
    certificate: Option<Account>,
    closure: Option<Account>,
    rent_credit: Option<Account>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OpenRollbackSnapshot {
    market: Option<Account>,
    source: Option<Account>,
    funding: [Option<Account>; 3],
    replay: Option<Account>,
    vault: Option<Account>,
    rent_credit: Option<Account>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProviderRollbackSnapshot {
    market: Option<Account>,
    source: Option<Account>,
    lifecycle: Option<Account>,
    update: Option<Account>,
    funding: [Option<Account>; 3],
    certificate: Option<Account>,
    replay: Option<Account>,
    vault: Option<Account>,
    rent_credit: Option<Account>,
    treasury: Option<Account>,
}

fn id(bytes: [u8; 32]) -> CoreContentId {
    CoreContentId::new(bytes).expect("nonzero content identity")
}

fn product_id(bytes: [u8; 32]) -> ProductContentId {
    ProductContentId::new(bytes).expect("nonzero Product identity")
}

fn source_id(bytes: [u8; 32]) -> SourceContentId {
    SourceContentId::new(bytes).expect("nonzero Source identity")
}

fn artifacts() -> Elves {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    Elves {
        core: fs::read(directory.join("dclutch_core_sbf.so")).expect("Core ELF"),
        custody: fs::read(directory.join("dclutch_custody_sbf.so")).expect("Custody ELF"),
        registry: fs::read(directory.join("dclutch_registry_sbf.so")).expect("Registry ELF"),
        resolution: fs::read(directory.join("dclutch_resolution_proof_sbf.so"))
            .expect("Resolution ELF"),
    }
}

fn programdata(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn immutable_programdata(elf: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 45 + elf.len()];
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

fn program_identity(program: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(program.to_bytes()).expect("nonzero program")
}

fn release(program: Pubkey, semantic: [u8; 32], elf: &[u8]) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        program_identity(program),
        program_identity(bpf_loader_upgradeable::ID),
        programdata(program).to_bytes(),
        id(semantic),
        hash(elf).to_bytes(),
        0,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("immutable artifact release")
}

fn artifact_id(release: ArtifactReleaseV1) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(hash(&release.to_bytes()).to_bytes()).expect("artifact identity")
}

fn binding(release: ArtifactReleaseV1) -> ExecutionRoleBindingV1 {
    ExecutionRoleBindingV1::new(release.program(), artifact_id(release))
}

fn activation_input(release: ArtifactReleaseV1) -> ArtifactActivationInputV1 {
    ArtifactActivationInputV1::new(
        artifact_id(release),
        release,
        DeploymentObservationV1::new(
            release.program().to_bytes(),
            bpf_loader_upgradeable::ID.to_bytes(),
            true,
            release.programdata(),
            bpf_loader_upgradeable::ID.to_bytes(),
            false,
            release.programdata(),
            bpf_loader_upgradeable::ID.to_bytes(),
            release.deployment_slot(),
            release.elf_digest(),
            release.upgrade_authority(),
        )
        .expect("current deployment observation"),
    )
}

fn activation(
    core: ArtifactReleaseV1,
    resolution: ArtifactReleaseV1,
    custody: ArtifactReleaseV1,
) -> ([u8; 32], Vec<u8>) {
    let release_set = ExecutionReleaseSetV1::new(
        binding(core),
        binding(core),
        binding(custody),
        binding(resolution),
        binding(custody),
    )
    .expect("execution release set");
    let release_set_id = hash(&release_set.to_bytes()).to_bytes();
    let content = id(release_set_id);
    let mut bytes = vec![0; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut bytes, content).expect("activation cache");
    for (role, selected) in [
        (ExecutionRoleV1::Core, core),
        (ExecutionRoleV1::Claims, core),
        (ExecutionRoleV1::Trading, custody),
        (ExecutionRoleV1::Resolution, resolution),
        (ExecutionRoleV1::Custody, custody),
    ] {
        activate_execution_role_into_v1(
            &mut bytes,
            content,
            &release_set,
            role,
            &activation_input(selected),
        )
        .expect("activate execution role");
    }
    ActivatedExecutionReleaseSetV1::decode(&bytes).expect("complete activation cache");
    (release_set_id, bytes)
}

fn protocol_account(owner: Pubkey, data: Vec<u8>) -> Account {
    Account {
        lamports: Rent::default().minimum_balance(data.len()).max(1),
        data,
        owner,
        executable: false,
        rent_epoch: 0,
    }
}

fn mint_data() -> Vec<u8> {
    let mut bytes = vec![0; SplMint::LEN];
    SplMint::pack(
        SplMint {
            mint_authority: COption::None,
            supply: 0,
            decimals: 6,
            is_initialized: true,
            freeze_authority: COption::None,
        },
        &mut bytes,
    )
    .expect("production collateral Mint");
    bytes
}

fn add_record(test: &mut ProgramTest, schema: [u8; 32], data: Vec<u8>) -> RecordPair {
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
    test.add_account(raw, protocol_account(REGISTRY_PROGRAM_ID, data));
    RecordPair { raw, staging }
}

fn add_active_funding(
    test: &mut ProgramTest,
    market: Pubkey,
    manifest_id: CapabilityContentId,
    manifest: CapabilityManifestV1<'_>,
    entry_index: u16,
) -> Pubkey {
    let rent = Rent::default().minimum_balance(FUNDING_STATE_BYTES);
    let custody = FundingCustodyObservationV1::native_only(
        rent.checked_mul(2)
            .and_then(|value| value.checked_add(BOUNTY))
            .expect("bounded funding custody"),
        rent,
    )
    .expect("native funding custody");
    let mut state = FundingStateV1::new(manifest_id, manifest, entry_index, custody)
        .expect("pending FundingState");
    state
        .activate(manifest_id, manifest, custody, 1)
        .expect("active FundingState");
    let key = funding_key(market, manifest_id, manifest, entry_index);
    test.add_account(
        key,
        Account {
            lamports: rent + BOUNTY,
            data: state.to_bytes().to_vec(),
            owner: RESOLUTION_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    key
}

fn funding_key(
    market: Pubkey,
    manifest_id: CapabilityContentId,
    manifest: CapabilityManifestV1<'_>,
    entry_index: u16,
) -> Pubkey {
    let rent = Rent::default().minimum_balance(FUNDING_STATE_BYTES);
    let custody = FundingCustodyObservationV1::native_only(
        rent.checked_mul(2)
            .and_then(|value| value.checked_add(BOUNTY))
            .expect("bounded funding custody"),
        rent,
    )
    .expect("native funding custody");
    let state = FundingStateV1::new(manifest_id, manifest, entry_index, custody)
        .expect("pending FundingState");
    let derivation = CapabilityFundingDerivationV1::new(
        market.to_bytes(),
        GENERATION,
        manifest_id,
        manifest,
        state,
    )
    .expect("funding derivation");
    Pubkey::find_program_address(&derivation.seed_components(), &RESOLUTION_PROGRAM_ID).0
}

fn custody_request(
    release_set: [u8; 32],
    market: Pubkey,
    realm: [u8; 32],
    mint: Pubkey,
    payer: Pubkey,
    rent_refund: Pubkey,
    operation: OperationV1,
) -> CustodyRequestV1 {
    let core_request = Request::administrative(
        Action::OpenMarket,
        GENERATION,
        CoreIdentity::new(market.to_bytes()).expect("Market"),
    )
    .encode()
    .expect("Core open request");
    let mut request = CustodyRequestV1 {
        operation: OperationV1::InitializeReplay,
        caller_role: CallerRoleV1::Core,
        source_compartment: CompartmentV1::None,
        destination_compartment: CompartmentV1::None,
        release_set,
        market: market.to_bytes(),
        realm,
        context: market.to_bytes(),
        caller_program: CORE_PROGRAM_ID.to_bytes(),
        semantic: ContextV1 {
            candidate: [0; 32],
            source_owner: [0; 32],
            destination_owner: [0; 32],
            order: [0; 32],
            parent_request_digest: hash(&core_request).to_bytes(),
            order_nonce: 0,
            generation: GENERATION,
            page_index: 0,
            execution_index: 0,
            transfer_index: 0,
        },
        source: [0; 32],
        destination: [0; 32],
        source_vault_context: [0; 32],
        destination_vault_context: [0; 32],
        mint: [0; 32],
        token_program: [0; 32],
        payer: payer.to_bytes(),
        rent_refund: rent_refund.to_bytes(),
        expected_revision: 0,
        resulting_revision: 1,
        amount: 0,
        rent_lamports: Rent::default()
            .minimum_balance(dclutch_custody_contract::CUSTODY_REPLAY_BYTES_V1),
    };
    if operation == OperationV1::OpenVault {
        request.operation = operation;
        request.destination_compartment = CompartmentV1::HoardPrincipal;
        request.destination_vault_context = market.to_bytes();
        request.mint = mint.to_bytes();
        request.token_program = LEGACY_TOKEN_PROGRAM_ID;
        request.expected_revision = 1;
        request.resulting_revision = 2;
        request.rent_lamports = Rent::default().minimum_balance(dclutch_token_svm::ACCOUNT_BYTES);
        request.destination = Pubkey::find_program_address(
            &CustodyVaultSeedsV1::from_request(request, false).as_slices(),
            &CUSTODY_PROGRAM_ID,
        )
        .0
        .to_bytes();
    }
    request
}

/// Which founding route left the Market this fixture starts from.
///
/// The three variants are three distinct real prestates, not three degrees of
/// convenience.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MarketPrestateV1 {
    /// `Founding + Prepaid`: what Core's canonical Found31 leaves. The Market
    /// still owes its whole readiness ladder — `CreateFund`,
    /// `VerifyFundReady`, then a separate `OpenMarket`.
    ReadinessLadder,
    /// `Open + Consumed` with no Resolution Fund of any kind: exactly what
    /// `DCLTGMF1`'s commit-last `open_series_market` leaves. This Market is
    /// open and tradeable and has never had a `SourceResolutionStateV2`, and
    /// before the Fund admission existed it could never acquire one.
    AtomicallyFounded,
    /// `Open + Consumed` with an already-resolved Source, three active Funds
    /// and a minted certificate — the prestate of terminal admission and
    /// retirement.
    Terminal,
}

impl MarketPrestateV1 {
    /// Whether the Market account starts `Open + Consumed` rather than
    /// `Founding + Prepaid`.
    const fn open(self) -> bool {
        matches!(self, Self::AtomicallyFounded | Self::Terminal)
    }

    /// Whether the Source state, its three Funds and the terminal certificate
    /// are seeded as already-terminal rather than left to be created.
    const fn preload_terminal(self) -> bool {
        matches!(self, Self::Terminal)
    }
}

fn fixture(prestate: MarketPrestateV1) -> Fixture {
    let preload_terminal = prestate.preload_terminal();
    let elves = artifacts();
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    add_program(
        &mut test,
        "dclutch_registry_sbf",
        REGISTRY_PROGRAM_ID,
        &elves.registry,
    );
    add_program(&mut test, "dclutch_core_sbf", CORE_PROGRAM_ID, &elves.core);
    add_program(
        &mut test,
        "dclutch_custody_sbf",
        CUSTODY_PROGRAM_ID,
        &elves.custody,
    );
    add_program(
        &mut test,
        "dclutch_resolution_proof_sbf",
        RESOLUTION_PROGRAM_ID,
        &elves.resolution,
    );
    let provider = pyth_provider::ProviderAddresses::pinned();
    pyth_provider::assert_all_fixture_hashes();
    pyth_provider::add_upgraded_provider_programs(&mut test, provider);

    let core_release = release(CORE_PROGRAM_ID, [0x41; 32], &elves.core);
    let resolution_release = release(
        RESOLUTION_PROGRAM_ID,
        RESOLUTION_CONTROLLER_RELEASE_ID_V4,
        &elves.resolution,
    );
    let custody_release = release(CUSTODY_PROGRAM_ID, [0x42; 32], &elves.custody);
    let registry_release = release(REGISTRY_PROGRAM_ID, [0x43; 32], &elves.registry);
    let (release_set, activation_data) =
        activation(core_release, resolution_release, custody_release);
    let activation = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    test.add_account(
        activation,
        protocol_account(REGISTRY_PROGRAM_ID, activation_data),
    );
    let registry_artifact = add_record(
        &mut test,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        registry_release.to_bytes().to_vec(),
    );
    let infrastructure = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
        &CORE_PROGRAM_ID,
    )
    .0;
    let rent_binding = ExecutionRoleBindingV1::new(
        program_identity(sysvar::rent::ID),
        ArtifactReleaseIdV1::new([0x44; 32]).expect("rent artifact identity"),
    );
    let infrastructure_value =
        ProtocolInfrastructureProfileV1::new(binding(registry_release), rent_binding)
            .expect("immutable infrastructure profile");
    test.add_account(
        infrastructure,
        protocol_account(CORE_PROGRAM_ID, infrastructure_value.to_bytes().to_vec()),
    );

    let mint = Pubkey::new_unique();
    let adapter = PRODUCTION_ADAPTER_RELEASES
        .first()
        .copied()
        .expect("production collateral adapter");
    let realm_value = RealmV1::new(RealmV1Input {
        token_program: LEGACY_TOKEN_PROGRAM_ID,
        collateral_mint: mint.to_bytes(),
        collateral_adapter_release_id: hash(&adapter.to_bytes()).to_bytes(),
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    })
    .expect("immutable Realm");
    let realm_bytes = realm_value.to_bytes().to_vec();
    let realm = hash(&realm_bytes).to_bytes();
    let realm_record = add_record(&mut test, REALM_SCHEMA_RELEASE_ID_V1, realm_bytes);
    test.add_account(
        mint,
        protocol_account(Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID), mint_data()),
    );

    let coordinate_id = [0x81; 32];
    let unit_id = [0x82; 32];
    let product_identity = [0x83; 32];
    let liability_id = [0x84; 32];
    let representation_release = [0x85; 32];
    let mapping_release = [0x86; 32];
    let cuts = [0_i128];
    let coefficients = [1_u64, 1, 1];
    let mut domain_bytes = vec![0; result_domain_record_bytes(cuts.len()).expect("domain width")];
    compile_result_domain_v2(
        ResultDomainInputV2 {
            product_id: product_id(product_identity),
            coordinate_domain_id: product_id(coordinate_id),
            result_unit_id: product_id(unit_id),
            liability_basis_id: product_id(liability_id),
            representation_release_id: product_id(representation_release),
            mapping_release_id: product_id(mapping_release),
            cut_denominator: 1,
            cuts: &cuts,
        },
        &mut domain_bytes,
    )
    .expect("Product result domain");
    let domain_id = hash(&domain_bytes).to_bytes();
    let mut portfolio_bytes =
        vec![0; portfolio_record_bytes(coefficients.len()).expect("portfolio width")];
    compile_portfolio_v2(
        PortfolioInputV2 {
            product_id: product_id(product_identity),
            result_domain_id: product_id(domain_id),
            claim_basis_id: product_id([0x87; 32]),
            liability_basis_id: product_id(liability_id),
            representation_release_id: product_id(representation_release),
            denominator: 1,
            coefficients: &coefficients,
        },
        &mut portfolio_bytes,
    )
    .expect("Product portfolio");
    let portfolio_id = hash(&portfolio_bytes).to_bytes();
    let mut product_bytes = [0; PRODUCT_RECORD_BYTES_V2];
    ProductRecordV2::new(
        product_id(product_identity),
        product_id(domain_id),
        product_id(portfolio_id),
    )
    .encode_into(&mut product_bytes)
    .expect("Product root");
    let product_record_id = hash(&product_bytes).to_bytes();

    let capacity = SourceCapacityProfileV1::new(
        CapacityEnvelope::Measured,
        1,
        1,
        source_id([0x91; 32]),
        source_id([0x92; 32]),
        256,
        0,
    )
    .expect("Source capacity");
    let capacity_id = source_id(hash(&capacity.to_bytes()).to_bytes());
    let recovery_allocation = source_id([0x93; 32]);
    let recovery_policy_value = RecoveryPolicyV2::new(
        capacity_id,
        [
            Some(
                RecoveryAttemptV2::new(
                    source_id([0x94; 32]),
                    source_id([0x95; 32]),
                    TERMINAL_TIME + 20,
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
    let recovery_policy_bytes = recovery_policy_value.to_bytes();
    let recovery_policy_id = hash(&recovery_policy_bytes).to_bytes();
    let update_view =
        FullPriceUpdateV2::parse(pyth_provider::PRICE_UPDATE).expect("captured full Pyth update");
    let pyth_release_bytes = pyth_provider::synthetic_release_bytes(provider);
    assert_eq!(pyth_release_bytes.len(), PYTH_RELEASE_V1_ENCODED_LEN);
    let pyth_release_value =
        PythReleaseV1::decode(&pyth_release_bytes).expect("pinned Pyth release");
    let pyth_release_id = hash(&pyth_release_bytes).to_bytes();
    let provider_release_value = ProviderReleaseV1::new(
        source_id([0x96; 32]),
        source_id(pyth_release_value.adapter_id()),
        source_id(pyth_release_id),
        source_id(pyth_release_value.price_update_codec_id()),
        source_id(pyth_release_value.router_abi_id()),
    );
    let provider_release_bytes = provider_release_value.to_bytes();
    let provider_release_id = hash(&provider_release_bytes).to_bytes();
    let adapter_config_value =
        PythAdapterConfigV1::new(update_view.feed_id(), update_view.exponent(), 10_000)
            .expect("captured Pyth adapter configuration");
    let adapter_config_bytes = adapter_config_value.to_bytes();
    let adapter_config_id = hash(&adapter_config_bytes).to_bytes();
    let source_unit = source_id([0x97; 32]);
    let source_spec_value = SourceSpecV1::new(
        source_id(coordinate_id),
        source_unit,
        source_id(provider_release_id),
        SourceAccessProfile::PythTerminalOneTransaction,
        source_id(adapter_config_id),
        capacity_id,
    );
    let source_spec_bytes = source_spec_value.to_bytes();
    let source_spec_id = hash(&source_spec_bytes).to_bytes();
    // A closed period ending at the captured publication, rather than a window
    // pinned to that publication at both ends. The upper bound is load-bearing
    // for every `TERMINAL_TIME + n` deadline in this file; the width is what
    // the fixture was wrong about.
    let window_value = WindowSpecV1::new(
        source_id(source_spec_id),
        WindowKind::Terminal,
        update_view.publish_time() - 300,
        update_view.publish_time(),
        10,
        1,
        source_id([0x98; 32]),
    )
    .expect("captured terminal window");
    let window_bytes = window_value.to_bytes();
    let window_id = hash(&window_bytes).to_bytes();
    let statistic_value = StatisticSpecV1::new(
        source_unit,
        source_id(unit_id),
        StatisticKind::TerminalSample,
        RoundingBoundary::ExactRational,
        1,
        0,
        capacity_id,
        source_id([0x99; 32]),
        capacity,
    )
    .expect("captured terminal statistic");
    let statistic_bytes = statistic_value.to_bytes();
    let statistic_id = hash(&statistic_bytes).to_bytes();
    let material_value = SourceMaterialV2::new(
        source_id(product_record_id),
        source_id(source_spec_id),
        source_id(window_id),
        source_id(statistic_id),
        Some(source_id(recovery_policy_id)),
        source_id(SOURCE_FAILURE_POLICY_RELEASE_ID_V2),
    );
    let material_bytes = material_value.to_bytes();
    let material_id = hash(&material_bytes).to_bytes();

    let funding_rent = Rent::default().minimum_balance(FUNDING_STATE_BYTES);
    let quote = FundingQuoteV1::new(
        FundingAmountsV1::new(
            CompartmentFundingV1::native_lamports(funding_rent).expect("funding rent"),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::native_lamports(BOUNTY).expect("worker bounty"),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
        )
        .expect("typed funding"),
        None,
    )
    .expect("funding quote");
    let entries = [
        (0xa1, recovery_allocation.to_bytes()),
        (0xa2, recovery_policy_id),
        (0xa3, material_id),
    ]
    .map(|(seed, config)| {
        CapabilityEntryV1::new(
            id([seed; 32]),
            id(RESOLUTION_CONTROLLER_RELEASE_ID_V4),
            id(config),
            id([0xa4; 32]),
            id([0xa5; 32]),
            id([0xa6; 32]),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            quote,
        )
        .expect("Resolution funding entry")
    });
    let mut manifest_bytes = vec![0; MANIFEST_HEADER_BYTES + 3 * CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&entries, &mut manifest_bytes).expect("capability manifest");
    let manifest_id_bytes = hash(&manifest_bytes).to_bytes();
    let manifest = CapabilityManifestV1::decode(&manifest_bytes).expect("manifest view");

    let source_material = add_record(
        &mut test,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
        material_bytes.to_vec(),
    );
    let source_spec = add_record(
        &mut test,
        SOURCE_SPEC_SCHEMA_ID_V1,
        source_spec_bytes.to_vec(),
    );
    let provider_release = add_record(
        &mut test,
        PROVIDER_RELEASE_SCHEMA_ID_V1,
        provider_release_bytes.to_vec(),
    );
    let adapter_config = add_record(
        &mut test,
        PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1,
        adapter_config_bytes.to_vec(),
    );
    let window = add_record(&mut test, WINDOW_SPEC_SCHEMA_ID_V1, window_bytes.to_vec());
    let statistic = add_record(
        &mut test,
        STATISTIC_SPEC_SCHEMA_ID_V1,
        statistic_bytes.to_vec(),
    );
    let pyth_release = add_record(
        &mut test,
        PYTH_RELEASE_RECORD_SCHEMA_ID_V1,
        pyth_release_bytes.to_vec(),
    );
    let capability_manifest = add_record(
        &mut test,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        manifest_bytes.clone(),
    );
    let recovery_policy = add_record(
        &mut test,
        RECOVERY_POLICY_SCHEMA_ID_V2,
        recovery_policy_bytes.to_vec(),
    );
    let product = add_record(
        &mut test,
        PRODUCT_RECORD_SCHEMA_ID_V2,
        product_bytes.to_vec(),
    );
    let domain = add_record(&mut test, RESULT_DOMAIN_SCHEMA_ID_V2, domain_bytes.clone());
    let portfolio = add_record(&mut test, PORTFOLIO_SCHEMA_ID_V2, portfolio_bytes);

    let refund_authority = RefundAuthority::new([0xb1; 32]).expect("rent refund authority");
    let (rent_credit, rent_credit_bump) = Pubkey::find_program_address(
        &[RENT_CREDIT_PDA_DOMAIN_V1, &refund_authority.to_bytes()],
        &RENT_PROGRAM_ID,
    );
    let rent_credit_value = RentCreditV1::new(refund_authority, rent_credit_bump);
    test.add_account(
        rent_credit,
        protocol_account(RENT_PROGRAM_ID, rent_credit_value.to_bytes().to_vec()),
    );

    let mut identity = MarketIdentity {
        market_id: CoreIdentity::new([0xff; 32]).expect("placeholder Market"),
        realm_id: CoreIdentity::new(realm).expect("Realm"),
        product_record: CoreIdentity::new(product_record_id).expect("Product record"),
        product_id: CoreIdentity::new(product_identity).expect("Product"),
        resolution_policy: CoreIdentity::new(material_id).expect("Source material"),
        capability_manifest: CoreIdentity::new(manifest_id_bytes).expect("manifest"),
        selected_release_set: CoreIdentity::new(release_set).expect("release set"),
        registry_program: CoreIdentity::new(REGISTRY_PROGRAM_ID.to_bytes()).expect("Registry"),
        generation: GENERATION,
    };
    let market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(identity).as_slices(),
        &CORE_PROGRAM_ID,
    )
    .0;
    identity.market_id = CoreIdentity::new(market.to_bytes()).expect("Market");
    let state = CoreState {
        phase: if prestate.open() {
            Phase::Open
        } else {
            Phase::Founding
        },
        readiness: if prestate.open() {
            Readiness::Consumed
        } else {
            Readiness::Prepaid
        },
        terminal_winner: 0,
        identity,
        outstanding_capabilities: 0,
        rent_beneficiary: CoreIdentity::new(rent_credit.to_bytes()).expect("RentCredit"),
        terminal_receipt: None,
    };
    test.add_account(
        market,
        protocol_account(
            CORE_PROGRAM_ID,
            state.encode().expect("Core state").to_vec(),
        ),
    );

    let (source, source_bump) = Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
            market.as_ref(),
            &GENERATION.to_le_bytes(),
        ],
        &RESOLUTION_PROGRAM_ID,
    );
    if !preload_terminal {
        test.add_account(
            source,
            Account {
                lamports: 1,
                data: Vec::new(),
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            },
        );
    }
    let manifest_id = CapabilityContentId::new(manifest_id_bytes).expect("manifest identity");
    let funding = if preload_terminal {
        [0_u16, 1, 2]
            .map(|entry| add_active_funding(&mut test, market, manifest_id, manifest, entry))
    } else {
        [0_u16, 1, 2].map(|entry| funding_key(market, manifest_id, manifest, entry))
    };
    let certificate = Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            source.as_ref(),
            &[1],
            &TERMINAL_SEQUENCE.to_le_bytes(),
        ],
        &RESOLUTION_PROGRAM_ID,
    )
    .0;
    if preload_terminal {
        let mut source_value = SourceResolutionStateV2::fresh(
            market.to_bytes(),
            GENERATION,
            source_id(material_id),
            rent_credit.to_bytes(),
            source_bump,
            0,
            0,
        )
        .expect("fresh Source")
        .state();
        let result_domain = ResultDomainV2::decode(&domain_bytes).expect("ResultDomain view");
        let decision = source_value
            .resolve_primary_from_authenticated_domain(
                source_id(material_id),
                material_value,
                source_id(product_record_id),
                result_domain,
                source_id([0xb3; 32]),
                -1,
                1,
                GENERATION,
                TERMINAL_TIME,
                TERMINAL_SEQUENCE,
            )
            .expect("provider-authenticated terminal Source projection");
        assert_eq!(decision.selector(), 0);
        test.add_account(
            source,
            protocol_account(RESOLUTION_PROGRAM_ID, source_value.to_bytes().to_vec()),
        );
        let certificate_value = ResolutionCertificateV2 {
            kind: ResolutionCertificateKindV2::ResolutionSuccess,
            market: market.to_bytes(),
            route: [0xb4; 32],
            source_material: material_id,
            product_record_digest: product_record_id,
            provider_evidence: [0xb3; 32],
            funding_allocation: [0; 32],
            receipt_account: certificate.to_bytes(),
            generation: GENERATION,
            attempt_index: 0,
            schedule_index: 0,
            selector: decision.selector(),
            work_paid: 0,
            funding_remaining: 0,
            result_numerator: -1,
            result_denominator: 1,
            observed_at: u64::try_from(TERMINAL_TIME).expect("positive terminal time"),
        };
        test.add_account(
            certificate,
            protocol_account(
                RESOLUTION_PROGRAM_ID,
                certificate_value
                    .to_bytes()
                    .expect("terminal certificate")
                    .to_vec(),
            ),
        );
    }
    let closure = Pubkey::find_program_address(
        &[
            SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V2,
            source.as_ref(),
            &(TERMINAL_SEQUENCE + 1).to_le_bytes(),
        ],
        &RESOLUTION_PROGRAM_ID,
    )
    .0;
    let replay_request = custody_request(
        release_set,
        market,
        realm,
        mint,
        system_program::ID,
        rent_credit,
        OperationV1::InitializeReplay,
    );
    let replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::from_request(replay_request).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let custody_authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::from_request(replay_request).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let vault_request = custody_request(
        release_set,
        market,
        realm,
        mint,
        system_program::ID,
        rent_credit,
        OperationV1::OpenVault,
    );
    let vault = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::from_request(vault_request, false).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;

    Fixture {
        test: Some(test),
        provider,
        update: Keypair::new(),
        release_set,
        market,
        activation,
        infrastructure,
        registry_programdata: programdata(REGISTRY_PROGRAM_ID),
        registry_artifact,
        core_programdata: programdata(CORE_PROGRAM_ID),
        custody_programdata: programdata(CUSTODY_PROGRAM_ID),
        resolution_programdata: programdata(RESOLUTION_PROGRAM_ID),
        realm,
        realm_record,
        mint,
        replay,
        vault,
        custody_authority,
        source_material,
        source_spec,
        provider_release,
        adapter_config,
        window,
        statistic,
        pyth_release,
        capability_manifest,
        recovery_policy,
        product,
        domain,
        portfolio,
        source,
        funding,
        certificate,
        closure,
        rent_credit,
    }
}

fn finality() -> Observation {
    Observation {
        slot: 1,
        unix_timestamp: TERMINAL_TIME,
        finality: Finality::Finalized,
    }
}

fn into_observed(key: Pubkey, account: Account) -> ObservedAccount {
    ObservedAccount {
        observation: finality(),
        key,
        owner: account.owner,
        lamports: account.lamports,
        executable: account.executable,
        data: account.data,
    }
}

async fn observed(context: &mut ProgramTestContext, key: Pubkey) -> Option<Account> {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account query")
}

async fn required_observed(context: &mut ProgramTestContext, key: Pubkey) -> ObservedAccount {
    into_observed(
        key,
        observed(context, key)
            .await
            .expect("required account exists"),
    )
}

async fn observed_or_vacant(context: &mut ProgramTestContext, key: Pubkey) -> ObservedAccount {
    match observed(context, key).await {
        Some(account) => into_observed(key, account),
        None => vacant_observed(key),
    }
}

fn vacant_observed(key: Pubkey) -> ObservedAccount {
    ObservedAccount {
        observation: finality(),
        key,
        owner: system_program::ID,
        lamports: 0,
        executable: false,
        data: Vec::new(),
    }
}

async fn submit(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
) -> Result<(), BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let transaction = Transaction::new_signed_with_payer(
        instructions,
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    context.banks_client.process_transaction(transaction).await
}

async fn provider_submit_snapshot(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    encoded_vaa: Pubkey,
) -> ProviderSubmitSnapshotV3 {
    ProviderSubmitSnapshotV3 {
        market: required_observed(context, fixture.market).await,
        source_state: required_observed(context, fixture.source).await,
        source_material: required_observed(context, fixture.source_material.raw).await,
        source_spec: required_observed(context, fixture.source_spec.raw).await,
        source_provider_release: required_observed(context, fixture.provider_release.raw).await,
        pyth_release: required_observed(context, fixture.pyth_release.raw).await,
        window: required_observed(context, fixture.window.raw).await,
        encoded_vaa: required_observed(context, encoded_vaa).await,
    }
}

fn provider_submit_deployment(fixture: &Fixture) -> ProviderSubmitDeploymentV3 {
    ProviderSubmitDeploymentV3 {
        infrastructure: fixture.infrastructure,
        registry_programdata: fixture.registry_programdata,
        registry_artifact: fixture.registry_artifact.raw,
        registry_artifact_staging: fixture.registry_artifact.staging,
        core_programdata: fixture.core_programdata,
        resolution_program: RESOLUTION_PROGRAM_ID,
        resolution_programdata: fixture.resolution_programdata,
        receiver_config: fixture.provider.config,
        guardian_set: fixture.provider.guardian_set,
    }
}

async fn provider_execute_snapshot(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    lifecycle: Pubkey,
) -> ProviderExecuteSnapshotV3 {
    ProviderExecuteSnapshotV3 {
        market: required_observed(context, fixture.market).await,
        source_state: required_observed(context, fixture.source).await,
        lifecycle: required_observed(context, lifecycle).await,
        update: required_observed(context, fixture.update.pubkey()).await,
        source_material: required_observed(context, fixture.source_material.raw).await,
        source_spec: required_observed(context, fixture.source_spec.raw).await,
        source_provider_release: required_observed(context, fixture.provider_release.raw).await,
        adapter_config: required_observed(context, fixture.adapter_config.raw).await,
        window: required_observed(context, fixture.window.raw).await,
        statistic: required_observed(context, fixture.statistic.raw).await,
        pyth_release: required_observed(context, fixture.pyth_release.raw).await,
        product: required_observed(context, fixture.product.raw).await,
        result_domain: required_observed(context, fixture.domain.raw).await,
        portfolio: required_observed(context, fixture.portfolio.raw).await,
    }
}

fn provider_execute_deployment(fixture: &Fixture) -> ProviderExecuteDeploymentV3 {
    ProviderExecuteDeploymentV3 {
        registry_programdata: fixture.registry_programdata,
        registry_artifact: fixture.registry_artifact.raw,
        registry_artifact_staging: fixture.registry_artifact.staging,
        core_programdata: fixture.core_programdata,
        trading_program: CUSTODY_PROGRAM_ID,
        trading_programdata: fixture.custody_programdata,
        resolution_program: RESOLUTION_PROGRAM_ID,
        resolution_programdata: fixture.resolution_programdata,
        receiver_config: fixture.provider.config,
    }
}

async fn create_snapshot(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
) -> ResolutionCreateFundSnapshotV3 {
    ResolutionCreateFundSnapshotV3 {
        market: required_observed(context, fixture.market).await,
        activation_cache: required_observed(context, fixture.activation).await,
        registry_program: required_observed(context, REGISTRY_PROGRAM_ID).await,
        core_program: required_observed(context, CORE_PROGRAM_ID).await,
        core_programdata: required_observed(context, fixture.core_programdata).await,
        resolution_program: required_observed(context, RESOLUTION_PROGRAM_ID).await,
        resolution_programdata: required_observed(context, fixture.resolution_programdata).await,
        source_material: required_observed(context, fixture.source_material.raw).await,
        source_material_staging: vacant_observed(fixture.source_material.staging),
        capability_manifest: required_observed(context, fixture.capability_manifest.raw).await,
        capability_manifest_staging: vacant_observed(fixture.capability_manifest.staging),
        source_destination: observed_or_vacant(context, fixture.source).await,
        recovery_destination: observed_or_vacant(context, fixture.funding[0]).await,
        exhaustion_destination: observed_or_vacant(context, fixture.funding[1]).await,
        failure_destination: observed_or_vacant(context, fixture.funding[2]).await,
        rent_sysvar: required_observed(context, sysvar::rent::ID).await,
        system_program: required_observed(context, system_program::ID).await,
        recovery_policy: required_observed(context, fixture.recovery_policy.raw).await,
        recovery_policy_staging: vacant_observed(fixture.recovery_policy.staging),
    }
}

async fn verify_snapshot(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
) -> ResolutionVerifyFundReadySnapshotV3 {
    ResolutionVerifyFundReadySnapshotV3 {
        market: required_observed(context, fixture.market).await,
        activation_cache: required_observed(context, fixture.activation).await,
        registry_program: required_observed(context, REGISTRY_PROGRAM_ID).await,
        core_program: required_observed(context, CORE_PROGRAM_ID).await,
        core_programdata: required_observed(context, fixture.core_programdata).await,
        resolution_program: required_observed(context, RESOLUTION_PROGRAM_ID).await,
        resolution_programdata: required_observed(context, fixture.resolution_programdata).await,
        source_material: required_observed(context, fixture.source_material.raw).await,
        source_material_staging: vacant_observed(fixture.source_material.staging),
        capability_manifest: required_observed(context, fixture.capability_manifest.raw).await,
        capability_manifest_staging: vacant_observed(fixture.capability_manifest.staging),
        source_state: required_observed(context, fixture.source).await,
        recovery_funding: required_observed(context, fixture.funding[0]).await,
        exhaustion_funding: required_observed(context, fixture.funding[1]).await,
        failure_funding: required_observed(context, fixture.funding[2]).await,
        beneficiary: required_observed(context, fixture.rent_credit).await,
        clock_sysvar: required_observed(context, sysvar::clock::ID).await,
        rent_sysvar: required_observed(context, sysvar::rent::ID).await,
        recovery_policy: required_observed(context, fixture.recovery_policy.raw).await,
        recovery_policy_staging: vacant_observed(fixture.recovery_policy.staging),
    }
}

async fn admit_snapshot(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
) -> ResolutionAdmitTerminalSnapshotV3 {
    ResolutionAdmitTerminalSnapshotV3 {
        market: required_observed(context, fixture.market).await,
        activation_cache: required_observed(context, fixture.activation).await,
        registry_program: required_observed(context, REGISTRY_PROGRAM_ID).await,
        core_program: required_observed(context, CORE_PROGRAM_ID).await,
        core_programdata: required_observed(context, fixture.core_programdata).await,
        resolution_program: required_observed(context, RESOLUTION_PROGRAM_ID).await,
        resolution_programdata: required_observed(context, fixture.resolution_programdata).await,
        source_material: required_observed(context, fixture.source_material.raw).await,
        source_material_staging: vacant_observed(fixture.source_material.staging),
        capability_manifest: required_observed(context, fixture.capability_manifest.raw).await,
        capability_manifest_staging: vacant_observed(fixture.capability_manifest.staging),
        source_state: required_observed(context, fixture.source).await,
        recovery_funding: required_observed(context, fixture.funding[0]).await,
        exhaustion_funding: required_observed(context, fixture.funding[1]).await,
        failure_funding: required_observed(context, fixture.funding[2]).await,
        certificate: required_observed(context, fixture.certificate).await,
        rent_sysvar: required_observed(context, sysvar::rent::ID).await,
        product_raw: required_observed(context, fixture.product.raw).await,
        product_staging: vacant_observed(fixture.product.staging),
        result_domain_raw: required_observed(context, fixture.domain.raw).await,
        result_domain_staging: vacant_observed(fixture.domain.staging),
        portfolio_raw: required_observed(context, fixture.portfolio.raw).await,
        portfolio_staging: vacant_observed(fixture.portfolio.staging),
    }
}

fn begin_retiring_instruction(fixture: &Fixture) -> Instruction {
    let request = Request::administrative(
        Action::BeginRetiring,
        GENERATION,
        CoreIdentity::new(fixture.market.to_bytes()).expect("Market"),
    );
    Instruction {
        program_id: CORE_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(fixture.market, false),
            AccountMeta::new_readonly(fixture.activation, false),
            AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
            AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.core_programdata, false),
        ],
        data: request.encode().expect("BeginRetiring request").to_vec(),
    }
}

fn core_open_instruction(fixture: &Fixture, payer: Pubkey, operation: OperationV1) -> Instruction {
    let custody = custody_request(
        fixture.release_set,
        fixture.market,
        fixture.realm,
        fixture.mint,
        payer,
        fixture.rent_credit,
        operation,
    );
    let custody_bytes = custody.to_bytes().expect("Custody request");
    let authority = Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::new(
            CoreContentId::new(custody.release_set).expect("release set"),
            fixture.market.to_bytes(),
            ExecutionRoleV1::Core,
            fixture.market.to_bytes(),
            hash(&custody_bytes).to_bytes(),
        )
        .expect("Core caller seeds")
        .as_slices(),
        &CORE_PROGRAM_ID,
    )
    .0;
    let mut accounts = vec![
        AccountMeta::new_readonly(authority, false),
        AccountMeta::new(fixture.market, false),
        AccountMeta::new_readonly(fixture.activation, false),
        AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
        AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.core_programdata, false),
        AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.custody_programdata, false),
        AccountMeta::new_readonly(fixture.realm_record.raw, false),
        AccountMeta::new_readonly(fixture.realm_record.staging, false),
        AccountMeta::new(fixture.replay, false),
    ];
    match operation {
        OperationV1::InitializeReplay => accounts.extend([
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ]),
        OperationV1::OpenVault => accounts.extend([
            AccountMeta::new_readonly(fixture.mint, false),
            AccountMeta::new(fixture.vault, false),
            AccountMeta::new_readonly(fixture.custody_authority, false),
            AccountMeta::new_readonly(Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID), false),
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ]),
        OperationV1::Transfer | OperationV1::CloseVault | OperationV1::CloseReplay => {
            unreachable!("market opening uses only replay initialization and Hoard vault creation")
        }
    }
    let core = Request::administrative(
        Action::OpenMarket,
        GENERATION,
        CoreIdentity::new(fixture.market.to_bytes()).expect("Market"),
    )
    .encode()
    .expect("Core open request");
    let mut data = Vec::with_capacity(core.len() + custody_bytes.len());
    data.extend_from_slice(&core);
    data.extend_from_slice(&custody_bytes);
    Instruction {
        program_id: CORE_PROGRAM_ID,
        accounts,
        data,
    }
}

async fn open_instruction(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    payer: Pubkey,
    operation: OperationV1,
) -> Instruction {
    let core = core_open_instruction(fixture, payer, operation);
    let state = RegistryOpenMarketContinuationStateV1 {
        registry_program: required_observed(context, REGISTRY_PROGRAM_ID).await,
        activation_cache: required_observed(context, fixture.activation).await,
        core_program: required_observed(context, CORE_PROGRAM_ID).await,
        core_programdata: required_observed(context, fixture.core_programdata).await,
        custody_program: required_observed(context, CUSTODY_PROGRAM_ID).await,
        custody_programdata: required_observed(context, fixture.custody_programdata).await,
    };
    build_registry_open_market_continuation_v1(&state, &core)
        .expect("chain-derived authenticated Core market opening")
        .instruction
}

async fn close_snapshot(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
) -> ResolutionCloseFundSnapshotV3 {
    ResolutionCloseFundSnapshotV3 {
        market: required_observed(context, fixture.market).await,
        activation_cache: required_observed(context, fixture.activation).await,
        registry_program: required_observed(context, REGISTRY_PROGRAM_ID).await,
        core_program: required_observed(context, CORE_PROGRAM_ID).await,
        core_programdata: required_observed(context, fixture.core_programdata).await,
        resolution_program: required_observed(context, RESOLUTION_PROGRAM_ID).await,
        resolution_programdata: required_observed(context, fixture.resolution_programdata).await,
        source_material: required_observed(context, fixture.source_material.raw).await,
        source_material_staging: vacant_observed(fixture.source_material.staging),
        capability_manifest: required_observed(context, fixture.capability_manifest.raw).await,
        capability_manifest_staging: vacant_observed(fixture.capability_manifest.staging),
        source_state: required_observed(context, fixture.source).await,
        recovery_funding: required_observed(context, fixture.funding[0]).await,
        exhaustion_funding: required_observed(context, fixture.funding[1]).await,
        failure_funding: required_observed(context, fixture.funding[2]).await,
        certificate: required_observed(context, fixture.certificate).await,
        closure_destination: required_observed(context, fixture.closure).await,
        beneficiary: required_observed(context, fixture.rent_credit).await,
        clock_sysvar: required_observed(context, sysvar::clock::ID).await,
        rent_sysvar: required_observed(context, sysvar::rent::ID).await,
        system_program: required_observed(context, system_program::ID).await,
        recovery_policy: required_observed(context, fixture.recovery_policy.raw).await,
        recovery_policy_staging: vacant_observed(fixture.recovery_policy.staging),
    }
}

async fn retirement_snapshot(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
) -> RetirementSnapshot {
    RetirementSnapshot {
        market: observed(context, fixture.market).await,
        source: observed(context, fixture.source).await,
        funding: [
            observed(context, fixture.funding[0]).await,
            observed(context, fixture.funding[1]).await,
            observed(context, fixture.funding[2]).await,
        ],
        certificate: observed(context, fixture.certificate).await,
        closure: observed(context, fixture.closure).await,
        rent_credit: observed(context, fixture.rent_credit).await,
    }
}

async fn provider_rollback_snapshot(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    lifecycle: Pubkey,
) -> ProviderRollbackSnapshot {
    ProviderRollbackSnapshot {
        market: observed(context, fixture.market).await,
        source: observed(context, fixture.source).await,
        lifecycle: observed(context, lifecycle).await,
        update: observed(context, fixture.update.pubkey()).await,
        funding: [
            observed(context, fixture.funding[0]).await,
            observed(context, fixture.funding[1]).await,
            observed(context, fixture.funding[2]).await,
        ],
        certificate: observed(context, fixture.certificate).await,
        replay: observed(context, fixture.replay).await,
        vault: observed(context, fixture.vault).await,
        rent_credit: observed(context, fixture.rent_credit).await,
        treasury: observed(context, fixture.provider.treasury).await,
    }
}

async fn advance_provider_refusal_slot(context: &mut ProgramTestContext) {
    let current = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("ProgramTest Clock");
    context
        .warp_to_slot(current.slot.checked_add(1).expect("bounded fixture slot"))
        .expect("advance hostile provider transaction blockhash");
    let mut advanced = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("advanced ProgramTest Clock");
    advanced.unix_timestamp = TERMINAL_TIME;
    context.set_sysvar(&advanced);
}

async fn open_rollback_snapshot(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
) -> OpenRollbackSnapshot {
    OpenRollbackSnapshot {
        market: observed(context, fixture.market).await,
        source: observed(context, fixture.source).await,
        funding: [
            observed(context, fixture.funding[0]).await,
            observed(context, fixture.funding[1]).await,
            observed(context, fixture.funding[2]).await,
        ],
        replay: observed(context, fixture.replay).await,
        vault: observed(context, fixture.vault).await,
        rent_credit: observed(context, fixture.rent_credit).await,
    }
}

#[tokio::test]
async fn current_resolution_creates_and_activates_exact_funding() {
    let mut fixture = fixture(MarketPrestateV1::ReadinessLadder);
    let mut context = fixture
        .test
        .take()
        .expect("unstarted ProgramTest")
        .start_with_context()
        .await;
    let encoded_vaa =
        pyth_provider::initialize_real_providers(&mut context, fixture.provider).await;
    let mut clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("ProgramTest Clock");
    clock.slot = clock.slot.max(1);
    clock.unix_timestamp = TERMINAL_TIME;
    context.set_sysvar(&clock);

    let payer = context.payer.pubkey();
    let create_snapshot = create_snapshot(&mut context, &fixture).await;
    assert_eq!(create_snapshot.system_program.key, system_program::ID);
    assert_eq!(create_snapshot.system_program.owner, native_loader::ID);
    assert!(create_snapshot.system_program.executable);
    assert!(!create_snapshot.system_program.data.is_empty());
    let create =
        build_resolution_create_fund_v3(&create_snapshot).expect("chain-derived CreateFund");
    validate_resolution_create_fund_report_v3(&create).expect("exact CreateFund report");
    assert!(create.source_top_up_lamports > 0);
    assert!(
        create
            .funding_top_up_lamports
            .iter()
            .all(|value| *value > 0)
    );

    let mut create_instructions = Vec::with_capacity(5);
    create_instructions.push(transfer(
        &payer,
        &fixture.source,
        create.source_top_up_lamports,
    ));
    for (funding, top_up) in fixture
        .funding
        .into_iter()
        .zip(create.funding_top_up_lamports)
    {
        create_instructions.push(transfer(&payer, &funding, top_up));
    }
    create_instructions.push(create.instruction);
    submit(&mut context, &create_instructions)
        .await
        .expect("prepay and create canonical Source funding");

    let source = SourceResolutionStateV2::decode(
        &observed(&mut context, fixture.source)
            .await
            .expect("created Source")
            .data,
    )
    .expect("Source state");
    assert_eq!(source.phase(), SourceResolutionPhaseV1::Primary);
    for funding in fixture.funding {
        let state = FundingStateV1::decode(
            &observed(&mut context, funding)
                .await
                .expect("created Funding")
                .data,
        )
        .expect("Funding state");
        assert_eq!(state.status(), FundingStatus::Pending);
    }

    let verify =
        build_resolution_verify_fund_ready_v3(&verify_snapshot(&mut context, &fixture).await)
            .expect("chain-derived VerifyFundReady");
    validate_resolution_verify_fund_ready_report_v3(&verify).expect("exact VerifyFundReady report");
    let before_privilege_refusal = retirement_snapshot(&mut context, &fixture).await;
    let mut read_only_beneficiary = verify.instruction.clone();
    read_only_beneficiary
        .accounts
        .get_mut(16)
        .expect("beneficiary account")
        .is_writable = false;
    assert!(
        submit(&mut context, &[read_only_beneficiary])
            .await
            .is_err(),
        "read-only beneficiary privilege must refuse"
    );
    assert_eq!(
        retirement_snapshot(&mut context, &fixture).await,
        before_privilege_refusal,
        "privilege refusal preserves Source, Funds, Core, and RentCredit"
    );

    let beneficiary_before = observed(&mut context, fixture.rent_credit)
        .await
        .expect("RentCredit")
        .lamports;
    submit(&mut context, &[verify.instruction])
        .await
        .expect("activate exact three-ledger Resolution funding");
    let ready = CoreState::decode(
        &observed(&mut context, fixture.market)
            .await
            .expect("Market")
            .data,
    )
    .expect("Core state");
    assert_eq!(ready.phase, Phase::Founding);
    assert_eq!(ready.readiness, Readiness::Ready);
    assert_eq!(
        observed(&mut context, fixture.rent_credit)
            .await
            .expect("RentCredit")
            .lamports,
        beneficiary_before + verify.expected_beneficiary_credit_lamports
    );
    for funding in fixture.funding {
        let state = FundingStateV1::decode(
            &observed(&mut context, funding)
                .await
                .expect("active Funding")
                .data,
        )
        .expect("Funding state");
        assert_eq!(state.status(), FundingStatus::Active);
    }

    let beneficiary_after_readiness = observed(&mut context, fixture.rent_credit)
        .await
        .expect("RentCredit after readiness")
        .lamports;
    let before_open_refusals = open_rollback_snapshot(&mut context, &fixture).await;
    let mut reordered =
        open_instruction(&mut context, &fixture, payer, OperationV1::InitializeReplay).await;
    reordered.accounts.swap(1, 3);
    assert!(
        submit(&mut context, &[reordered]).await.is_err(),
        "swapped Core/Custody role deployments must refuse"
    );
    assert_eq!(
        open_rollback_snapshot(&mut context, &fixture).await,
        before_open_refusals,
        "Registry role-order refusal preserves Market, Source, Funds, Custody, and RentCredit"
    );
    let mut substituted =
        open_instruction(&mut context, &fixture, payer, OperationV1::InitializeReplay).await;
    substituted
        .accounts
        .last_mut()
        .expect("nested Registry admission")
        .pubkey = Pubkey::new_unique();
    assert!(
        submit(&mut context, &[substituted]).await.is_err(),
        "substituted invocation admission must refuse"
    );
    assert_eq!(
        open_rollback_snapshot(&mut context, &fixture).await,
        before_open_refusals,
        "admission substitution rolls back Market, Source, Funds, Custody, and RentCredit"
    );
    for operation in [OperationV1::InitializeReplay, OperationV1::OpenVault] {
        let instruction = open_instruction(&mut context, &fixture, payer, operation).await;
        submit(&mut context, &[instruction])
            .await
            .expect("Core opens canonical Custody replay and Hoard vault");
    }
    let open = CoreState::decode(
        &observed(&mut context, fixture.market)
            .await
            .expect("open Market")
            .data,
    )
    .expect("Core state");
    assert_eq!(open.phase, Phase::Open);
    assert_eq!(open.readiness, Readiness::Consumed);
    let replay = CustodyReplayV1::decode(
        &observed(&mut context, fixture.replay)
            .await
            .expect("Custody replay")
            .data,
    )
    .expect("Custody replay state");
    assert_eq!(replay.next_revision, 2);
    assert_eq!(replay.open_vault_count, 1);
    assert_eq!(replay.rent_refund, fixture.rent_credit.to_bytes());
    assert_eq!(
        observed(&mut context, fixture.rent_credit)
            .await
            .expect("RentCredit after opening")
            .lamports,
        beneficiary_after_readiness,
        "the sponsor pays creation rent without debiting or rewriting the immutable beneficiary"
    );
    let vault = observed(&mut context, fixture.vault)
        .await
        .expect("Hoard vault");
    let profile = PRODUCTION_ADAPTER_RELEASES
        .first()
        .expect("production collateral adapter")
        .profile();
    let token = profile
        .check_custody_account(
            LEGACY_TOKEN_PROGRAM_ID,
            &vault.data,
            fixture.mint.to_bytes(),
            fixture.custody_authority.to_bytes(),
        )
        .expect("empty Hoard vault");
    assert_eq!(token.amount, 0);

    let post_update_body = pyth_provider::RECEIVER_POST_UPDATE
        .get(8..)
        .expect("Receiver PostUpdate body")
        .to_vec();
    let submit_intent = ProviderSubmitIntentV3 {
        submitter: payer,
        refund_recipient: fixture.rent_credit,
        update_account: fixture.update.pubkey(),
        reclaim_after_unix_seconds: TERMINAL_TIME + 20,
        post_update_body: post_update_body.clone(),
    };
    let submit_snapshot = provider_submit_snapshot(&mut context, &fixture, encoded_vaa).await;
    let submit_deployment = provider_submit_deployment(&fixture);
    let pyth_release = PythReleaseV1::decode(&submit_snapshot.pyth_release.data)
        .expect("authenticated Pyth release");
    let encoded = VerifiedEncodedVaaV1::parse(&submit_snapshot.encoded_vaa.data)
        .expect("verified encoded VAA");
    let expected_guardian = Pubkey::find_program_address(
        &[b"GuardianSet", &encoded.guardian_set_index().to_be_bytes()],
        &Pubkey::new_from_array(pyth_release.router_program()),
    )
    .0;
    assert_eq!(submit_deployment.guardian_set, expected_guardian);
    assert_eq!(
        submit_deployment.receiver_config,
        Pubkey::new_from_array(pyth_release.receiver_config())
    );
    assert_eq!(
        submit_deployment.infrastructure,
        Pubkey::find_program_address(
            &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
            &submit_snapshot.market.owner,
        )
        .0
    );
    let provider_submit =
        build_provider_submit_v3(&submit_snapshot, submit_deployment, &submit_intent)
            .expect("chain-derived real-provider submission");
    let provider_lifecycle_rent = context
        .banks_client
        .get_rent()
        .await
        .expect("chain Rent")
        .minimum_balance(PROVIDER_UPDATE_LIFECYCLE_BYTES_V3);
    let prepay_provider_lifecycle =
        transfer(&payer, &provider_submit.lifecycle, provider_lifecycle_rent);
    let before_material_substitution =
        provider_rollback_snapshot(&mut context, &fixture, provider_submit.lifecycle).await;
    let mut substituted_material = provider_submit.instruction.clone();
    substituted_material
        .accounts
        .get_mut(17)
        .expect("SourceMaterial raw record")
        .pubkey = fixture.product.raw;
    assert!(
        pyth_provider::submit(
            &mut context,
            &[prepay_provider_lifecycle.clone(), substituted_material],
            &[&fixture.update],
        )
        .await
        .is_err(),
        "a finalized record from the wrong schema/content identity must refuse"
    );
    assert_eq!(
        provider_rollback_snapshot(&mut context, &fixture, provider_submit.lifecycle).await,
        before_material_substitution,
        "SourceMaterial substitution rolls back Market, Source, lifecycle, certificate, all three Funds, Custody, update, and RentCredit"
    );

    let before_treasury_key_substitution =
        provider_rollback_snapshot(&mut context, &fixture, provider_submit.lifecycle).await;
    let mut substituted_treasury = provider_submit.instruction.clone();
    substituted_treasury
        .accounts
        .get_mut(34)
        .expect("Receiver treasury")
        .pubkey = Pubkey::new_unique();
    assert!(
        pyth_provider::submit(
            &mut context,
            &[prepay_provider_lifecycle.clone(), substituted_treasury],
            &[&fixture.update],
        )
        .await
        .is_err(),
        "a substituted Receiver treasury PDA must refuse"
    );
    assert_eq!(
        provider_rollback_snapshot(&mut context, &fixture, provider_submit.lifecycle).await,
        before_treasury_key_substitution,
        "treasury-key substitution rolls back every provider and Market write"
    );

    let original_treasury = observed(&mut context, fixture.provider.treasury).await;
    for (label, owner, data, executable) in [
        (
            "Receiver-owned treasury",
            fixture.provider.receiver,
            Vec::new(),
            false,
        ),
        ("data-bearing treasury", system_program::ID, vec![1], false),
        ("executable treasury", system_program::ID, Vec::new(), true),
    ] {
        advance_provider_refusal_slot(&mut context).await;
        let hostile = Account {
            lamports: Rent::default().minimum_balance(data.len()).max(1),
            data,
            owner,
            executable,
            rent_epoch: 0,
        };
        context.set_account(
            &fixture.provider.treasury,
            &AccountSharedData::from(hostile),
        );
        let before =
            provider_rollback_snapshot(&mut context, &fixture, provider_submit.lifecycle).await;
        assert!(
            pyth_provider::submit(
                &mut context,
                &[
                    prepay_provider_lifecycle.clone(),
                    provider_submit.instruction.clone(),
                ],
                &[&fixture.update],
            )
            .await
            .is_err(),
            "{label} must refuse"
        );
        assert_eq!(
            provider_rollback_snapshot(&mut context, &fixture, provider_submit.lifecycle).await,
            before,
            "{label} refusal rolls back every provider and Market write"
        );
    }
    let restored_treasury = original_treasury.unwrap_or(Account {
        lamports: 0,
        data: Vec::new(),
        owner: system_program::ID,
        executable: false,
        rent_epoch: 0,
    });
    context.set_account(
        &fixture.provider.treasury,
        &AccountSharedData::from(restored_treasury),
    );
    advance_provider_refusal_slot(&mut context).await;
    pyth_provider::submit(
        &mut context,
        &[prepay_provider_lifecycle, provider_submit.instruction],
        &[&fixture.update],
    )
    .await
    .expect("Resolution submits one update through the real Receiver ELF");
    assert_eq!(
        observed(&mut context, fixture.update.pubkey())
            .await
            .expect("Receiver update")
            .owner,
        fixture.provider.receiver
    );

    let resolver = Keypair::new();
    let resolver_rent = context
        .banks_client
        .get_rent()
        .await
        .expect("chain Rent")
        .minimum_balance(0);
    submit(
        &mut context,
        &[
            transfer(
                &payer,
                &fixture.certificate,
                Rent::default().minimum_balance(RESOLUTION_CERTIFICATE_BYTES_V2),
            ),
            transfer(&payer, &resolver.pubkey(), resolver_rent),
        ],
    )
    .await
    .expect("prepay the terminal certificate and establish the distinct resolver");
    let execute_intent = ProviderExecuteIntentV3 {
        resolver: resolver.pubkey(),
        terminal_sequence: TERMINAL_SEQUENCE,
        post_update_body,
    };
    let provider_execute = build_provider_execute_v3(
        &provider_execute_snapshot(&mut context, &fixture, provider_submit.lifecycle).await,
        provider_execute_deployment(&fixture),
        &execute_intent,
    )
    .expect("chain-derived Core provider execution");

    let before_late_substitution =
        provider_rollback_snapshot(&mut context, &fixture, provider_submit.lifecycle).await;
    let late_substitution = Instruction {
        program_id: CORE_PROGRAM_ID,
        accounts: vec![AccountMeta::new(fixture.source, false)],
        data: Request::administrative(
            Action::BeginRetiring,
            GENERATION,
            CoreIdentity::new(fixture.market.to_bytes()).expect("Market"),
        )
        .encode()
        .expect("late hostile Core request")
        .to_vec(),
    };
    assert!(
        pyth_provider::submit(
            &mut context,
            &[provider_execute.instruction.clone(), late_substitution],
            &[&resolver],
        )
        .await
        .is_err(),
        "a Market-to-Source substitution after provider terminalization must refuse"
    );
    assert_eq!(
        provider_rollback_snapshot(&mut context, &fixture, provider_submit.lifecycle).await,
        before_late_substitution,
        "the late substitution rolls back Market, Source, lifecycle, certificate, all three Funds, Custody, update, and RentCredit"
    );

    pyth_provider::submit(&mut context, &[provider_execute.instruction], &[&resolver])
        .await
        .expect("Core consumes the authenticated provider result and admits terminal Market state");
    let terminal_market = CoreState::decode(
        &observed(&mut context, fixture.market)
            .await
            .expect("terminal Market")
            .data,
    )
    .expect("terminal Core state");
    assert_eq!(terminal_market.phase, Phase::Terminal);
    assert!(terminal_market.terminal_receipt.is_some());
    let resolved_source = SourceResolutionStateV2::decode(
        &observed(&mut context, fixture.source)
            .await
            .expect("resolved Source")
            .data,
    )
    .expect("resolved Source state");
    assert_eq!(resolved_source.phase(), SourceResolutionPhaseV1::Resolved);
    let lifecycle = ProviderUpdateLifecycleV3::decode(
        &observed(&mut context, provider_submit.lifecycle)
            .await
            .expect("consumed provider lifecycle")
            .data,
    )
    .expect("provider lifecycle");
    assert_eq!(lifecycle.status, ProviderUpdateStatusV3::Consumed);
    assert_eq!(lifecycle.terminal_sequence, TERMINAL_SEQUENCE);
    assert_eq!(
        observed(&mut context, provider_submit.lifecycle)
            .await
            .expect("rent-exempt lifecycle")
            .data
            .len(),
        PROVIDER_UPDATE_LIFECYCLE_BYTES_V3
    );
    let certificate = ResolutionCertificateV2::decode(
        &observed(&mut context, fixture.certificate)
            .await
            .expect("terminal certificate")
            .data,
    )
    .expect("terminal Resolution certificate");
    assert_eq!(
        certificate.kind,
        ResolutionCertificateKindV2::ResolutionSuccess
    );
    assert_eq!(certificate.market, fixture.market.to_bytes());

    let after_success =
        provider_rollback_snapshot(&mut context, &fixture, provider_submit.lifecycle).await;
    assert_eq!(after_success.funding, before_late_substitution.funding);
    assert_eq!(after_success.replay, before_late_substitution.replay);
    assert_eq!(after_success.vault, before_late_substitution.vault);
    assert_eq!(
        after_success.rent_credit,
        before_late_substitution.rent_credit
    );

    submit(&mut context, &[begin_retiring_instruction(&fixture)])
        .await
        .expect("the same provider-resolved Market begins authenticated retirement");
    let retiring_market = CoreState::decode(
        &observed(&mut context, fixture.market)
            .await
            .expect("retiring Market")
            .data,
    )
    .expect("retiring Core state");
    assert_eq!(retiring_market.phase, Phase::Retiring);

    let closure_rent = context
        .banks_client
        .get_rent()
        .await
        .expect("chain Rent")
        .minimum_balance(SOURCE_CLOSURE_RECEIPT_BYTES_V2);
    submit(
        &mut context,
        &[transfer(&payer, &fixture.closure, closure_rent)],
    )
    .await
    .expect("prepay the same-lineage closure receipt");
    let close = build_resolution_close_fund_v3(&close_snapshot(&mut context, &fixture).await)
        .expect("chain-derived same-lineage CloseFund");
    validate_resolution_close_fund_report_v3(&close).expect("exact CloseFund report");
    submit(&mut context, &[close.instruction])
        .await
        .expect("close all three provider-resolution funding compartments");

    assert!(observed(&mut context, fixture.source).await.is_none());
    for funding in fixture.funding {
        assert!(observed(&mut context, funding).await.is_none());
    }
    let closure = SourceClosureReceiptV2::decode(
        &observed(&mut context, fixture.closure)
            .await
            .expect("same-lineage Source closure receipt")
            .data,
    )
    .expect("Source closure receipt");
    assert_eq!(closure.market, fixture.market.to_bytes());
    assert_eq!(closure.terminal_certificate, fixture.certificate.to_bytes());
    assert_eq!(closure.beneficiary, fixture.rent_credit.to_bytes());
}

#[tokio::test]
async fn current_resolution_admits_retires_closes_and_rolls_back_late_refusal() {
    let mut fixture = fixture(MarketPrestateV1::Terminal);
    let mut context = fixture
        .test
        .take()
        .expect("unstarted ProgramTest")
        .start_with_context()
        .await;
    let mut clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("ProgramTest Clock");
    clock.unix_timestamp = TERMINAL_TIME + 1;
    context.set_sysvar(&clock);

    let admit = build_resolution_admit_terminal_v3(&admit_snapshot(&mut context, &fixture).await)
        .expect("chain-derived AdmitTerminal");
    submit(&mut context, &[admit.instruction])
        .await
        .expect("Core -> Resolution terminal admission");
    let admitted = CoreState::decode(
        &observed(&mut context, fixture.market)
            .await
            .expect("Market")
            .data,
    )
    .expect("Core state");
    assert_eq!(admitted.phase, Phase::Terminal);
    assert_eq!(
        admitted.terminal_receipt.map(|value| value.to_bytes()),
        Some(fixture.certificate.to_bytes())
    );

    submit(&mut context, &[begin_retiring_instruction(&fixture)])
        .await
        .expect("permissionless authenticated BeginRetiring");
    let retiring = CoreState::decode(
        &observed(&mut context, fixture.market)
            .await
            .expect("Market")
            .data,
    )
    .expect("Core state");
    assert_eq!(retiring.phase, Phase::Retiring);

    let closure_rent = Rent::default().minimum_balance(SOURCE_CLOSURE_RECEIPT_BYTES_V2);
    let payer = context.payer.pubkey();
    submit(
        &mut context,
        &[transfer(&payer, &fixture.closure, closure_rent)],
    )
    .await
    .expect("prepay canonical closure receipt");
    let close = build_resolution_close_fund_v3(&close_snapshot(&mut context, &fixture).await)
        .expect("chain-derived CloseFund");
    validate_resolution_close_fund_report_v3(&close).expect("exact CloseFund report");

    let before_refusal = retirement_snapshot(&mut context, &fixture).await;
    let mut substituted_release = begin_retiring_instruction(&fixture);
    substituted_release
        .accounts
        .get_mut(4)
        .expect("Core ProgramData account")
        .pubkey = fixture.resolution_programdata;
    let refusal = submit(
        &mut context,
        &[
            close.instruction.clone(),
            // CloseFund has completed every physical write when this hostile
            // substitution refuses current Core release authentication.
            substituted_release,
        ],
    )
    .await;
    assert!(refusal.is_err(), "substituted Core release must refuse");
    assert_eq!(
        retirement_snapshot(&mut context, &fixture).await,
        before_refusal,
        "SVM rollback restores Core, Source, all three Funds, closure prepayment, certificate, and RentCredit"
    );

    submit(&mut context, &[close.instruction])
        .await
        .expect("Core -> Resolution physical close");
    assert!(observed(&mut context, fixture.source).await.is_none());
    for funding in fixture.funding {
        assert!(observed(&mut context, funding).await.is_none());
    }
    let closure = observed(&mut context, fixture.closure)
        .await
        .expect("Source closure receipt");
    assert_eq!(closure.owner, RESOLUTION_PROGRAM_ID);
    let closure = SourceClosureReceiptV2::decode(&closure.data).expect("closure receipt");
    assert_eq!(closure.market, fixture.market.to_bytes());
    assert_eq!(closure.terminal_certificate, fixture.certificate.to_bytes());
    assert_eq!(closure.beneficiary, fixture.rent_credit.to_bytes());
    let terminal =
        SourceResolutionStateV2::decode(&before_refusal.source.expect("Source prestate").data)
            .expect("terminal Source");
    assert_eq!(terminal.phase(), SourceResolutionPhaseV1::Resolved);
    let system = required_observed(&mut context, system_program::ID).await;
    assert_eq!(system.owner, native_loader::ID);
    assert!(system.executable);
    assert_eq!(
        CoreState::decode(
            &observed(&mut context, fixture.market)
                .await
                .expect("Market")
                .data,
        )
        .expect("Core state")
        .phase,
        Phase::Retiring,
        "CloseFund is a receipt-producing retirement component, not the full Claims/Custody retirement join"
    );
}

/// The whole point of this campaign: **a Market founded atomically can be
/// resolved.**
///
/// `DCLTGMF1` runs Lock, Found, Realize, Claims and a commit-last Open in one
/// rollback domain, and that Open is `open_series_market`, which moves the
/// Market from `Founding + Prepaid` straight to `Open + Consumed`. It never
/// passes the readiness ladder, so it never runs `CreateFund` — and
/// `CreateFund` is the only thing in the tree that creates a
/// `SourceResolutionStateV2`, which every terminal-certificate route consumes.
/// Before the Fund admission landed, this exact prestate was a permanently
/// unresolvable Market: open, tradeable, and with no reachable outcome.
///
/// `MarketPrestateV1::AtomicallyFounded` is that poststate and nothing else —
/// `Open + Consumed`, no terminal receipt, and no Source state, Fund or
/// certificate anywhere. The test walks it to a real terminal certificate
/// through the real Pyth transport, and refuses four hostile inputs on the way.
#[tokio::test]
async fn an_atomically_founded_market_reaches_a_terminal_certificate() {
    let mut fixture = fixture(MarketPrestateV1::AtomicallyFounded);
    let mut context = fixture
        .test
        .take()
        .expect("unstarted ProgramTest")
        .start_with_context()
        .await;
    let encoded_vaa =
        pyth_provider::initialize_real_providers(&mut context, fixture.provider).await;
    let mut clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("ProgramTest Clock");
    clock.slot = clock.slot.max(1);
    clock.unix_timestamp = TERMINAL_TIME;
    context.set_sysvar(&clock);
    let payer = context.payer.pubkey();

    // The prestate, asserted rather than assumed. This is what the atomic
    // founding leaves behind and it is the whole of what JRNY-1 found.
    let founded = CoreState::decode(
        &observed(&mut context, fixture.market)
            .await
            .expect("atomically founded Market")
            .data,
    )
    .expect("Core state");
    assert_eq!(founded.phase, Phase::Open);
    assert_eq!(founded.readiness, Readiness::Consumed);
    assert!(founded.terminal_receipt.is_none());
    assert_eq!(
        observed(&mut context, fixture.source)
            .await
            .expect("prepaid Source destination")
            .owner,
        system_program::ID,
        "an atomically founded Market has no Source resolution state at all"
    );
    for funding in fixture.funding {
        assert!(
            observed(&mut context, funding).await.is_none(),
            "an atomically founded Market has no Resolution Fund at all"
        );
    }

    let create = build_resolution_create_fund_v3(&create_snapshot(&mut context, &fixture).await)
        .expect("chain-derived CreateFund against an Open Market");
    validate_resolution_create_fund_report_v3(&create).expect("exact CreateFund report");
    let prepay = |top_ups: [u64; 3], source: u64| {
        let mut instructions = Vec::with_capacity(4);
        instructions.push(transfer(&payer, &fixture.source, source));
        for (funding, top_up) in fixture.funding.into_iter().zip(top_ups) {
            instructions.push(transfer(&payer, &funding, top_up));
        }
        instructions
    };
    let exact_top_ups = create.funding_top_up_lamports;

    // Hostile 1 — Source resolution-state substitution. The one account whose
    // address is not pinned by a manifest entry is the Source state, so its
    // derivation is the only thing standing between this Market and a state
    // bound to some other Market's material. Point the output slot at another
    // Resolution-program PDA of this same Market and it must refuse.
    let before_substitution = retirement_snapshot(&mut context, &fixture).await;
    let mut substituted_source = create.instruction.clone();
    substituted_source
        .accounts
        .get_mut(12)
        .expect("Source output account")
        .pubkey = fixture.closure;
    let mut substitution = prepay(exact_top_ups, create.source_top_up_lamports);
    substitution.push(transfer(
        &payer,
        &fixture.closure,
        Rent::default().minimum_balance(SOURCE_RESOLUTION_STATE_BYTES_V2),
    ));
    substitution.push(substituted_source);
    assert!(
        submit(&mut context, &substitution).await.is_err(),
        "a substituted Source resolution state must refuse"
    );
    assert_eq!(
        retirement_snapshot(&mut context, &fixture).await,
        before_substitution,
        "the substitution rolls back every prepayment with it"
    );

    // Hostile 2 — wrong-capability funding, under and over. Each Fund's
    // lamports must equal rent plus exactly the native principal its manifest
    // entry quotes; a Fund that is not the manifest's Fund is not this
    // Market's Fund. Both directions refuse, which is deliberate: over-funding
    // is not a donation the Fund may keep.
    for (label, delta) in [("under-funded", -1_i64), ("over-funded", 1_i64)] {
        let mut skewed = exact_top_ups;
        let first = skewed.first_mut().expect("recovery Fund top-up");
        *first = first
            .checked_add_signed(delta)
            .expect("bounded hostile top-up");
        let before = retirement_snapshot(&mut context, &fixture).await;
        let mut hostile = prepay(skewed, create.source_top_up_lamports);
        hostile.push(create.instruction.clone());
        assert!(
            submit(&mut context, &hostile).await.is_err(),
            "a {label} recovery compartment must refuse"
        );
        assert_eq!(
            retirement_snapshot(&mut context, &fixture).await,
            before,
            "the {label} refusal rolls back Source, all three Funds, Market and RentCredit"
        );
    }

    // The honest creation.
    let mut creation = prepay(exact_top_ups, create.source_top_up_lamports);
    creation.push(create.instruction.clone());
    submit(&mut context, &creation)
        .await
        .expect("an Open Market prepays and creates its own Resolution Fund");
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
    for funding in fixture.funding {
        assert_eq!(
            FundingStateV1::decode(
                &observed(&mut context, funding)
                    .await
                    .expect("created Funding")
                    .data,
            )
            .expect("Funding state")
            .status(),
            FundingStatus::Pending
        );
    }

    // Hostile 3 — double create. The Source PDA is one per Market generation
    // and `require_prepaid_output` refuses anything that is not
    // System-owned and empty, so the second creation cannot overwrite the
    // first.
    let before_double = retirement_snapshot(&mut context, &fixture).await;
    let mut double = prepay(exact_top_ups, create.source_top_up_lamports);
    double.push(create.instruction.clone());
    assert!(
        submit(&mut context, &double).await.is_err(),
        "a second CreateFund on the same Market generation must refuse"
    );
    assert_eq!(
        retirement_snapshot(&mut context, &fixture).await,
        before_double,
        "the double-create refusal leaves the first Fund byte-identical"
    );

    // Activation. Core stays `Open + Consumed`: `Readiness::Ready` is the
    // Founding lane's record of this same fact, and this Market consumed its
    // readiness at the commit-last Open. The activation itself lives in the
    // three FundingState accounts, which is what `AdmitTerminal` rechecks.
    let verify =
        build_resolution_verify_fund_ready_v3(&verify_snapshot(&mut context, &fixture).await)
            .expect("chain-derived VerifyFundReady against an Open Market");
    validate_resolution_verify_fund_ready_report_v3(&verify).expect("exact VerifyFundReady report");
    let beneficiary_before = observed(&mut context, fixture.rent_credit)
        .await
        .expect("RentCredit")
        .lamports;
    submit(&mut context, &[verify.instruction])
        .await
        .expect("activate the three-ledger Resolution funding of an Open Market");
    let activated = CoreState::decode(
        &observed(&mut context, fixture.market)
            .await
            .expect("Market")
            .data,
    )
    .expect("Core state");
    assert_eq!(activated.phase, Phase::Open);
    assert_eq!(
        activated.readiness,
        Readiness::Consumed,
        "an Open Market has already consumed its readiness and must not be rewritten to Ready"
    );
    assert_eq!(
        observed(&mut context, fixture.rent_credit)
            .await
            .expect("RentCredit")
            .lamports,
        beneficiary_before + verify.expected_beneficiary_credit_lamports
    );
    for funding in fixture.funding {
        assert_eq!(
            FundingStateV1::decode(
                &observed(&mut context, funding)
                    .await
                    .expect("active Funding")
                    .data,
            )
            .expect("Funding state")
            .status(),
            FundingStatus::Active
        );
    }

    // The real provider transport, unchanged from the ladder campaign: one
    // Pyth update posted through the real Receiver ELF, then one Core-driven
    // execution that mints the terminal certificate.
    let post_update_body = pyth_provider::RECEIVER_POST_UPDATE
        .get(8..)
        .expect("Receiver PostUpdate body")
        .to_vec();
    let submit_intent = ProviderSubmitIntentV3 {
        submitter: payer,
        refund_recipient: fixture.rent_credit,
        update_account: fixture.update.pubkey(),
        reclaim_after_unix_seconds: TERMINAL_TIME + 20,
        post_update_body: post_update_body.clone(),
    };
    let provider_submit = build_provider_submit_v3(
        &provider_submit_snapshot(&mut context, &fixture, encoded_vaa).await,
        provider_submit_deployment(&fixture),
        &submit_intent,
    )
    .expect("chain-derived real-provider submission");
    let provider_lifecycle_rent = context
        .banks_client
        .get_rent()
        .await
        .expect("chain Rent")
        .minimum_balance(PROVIDER_UPDATE_LIFECYCLE_BYTES_V3);
    pyth_provider::submit(
        &mut context,
        &[
            transfer(&payer, &provider_submit.lifecycle, provider_lifecycle_rent),
            provider_submit.instruction,
        ],
        &[&fixture.update],
    )
    .await
    .expect("Resolution submits one update through the real Receiver ELF");

    let resolver = Keypair::new();
    let resolver_rent = context
        .banks_client
        .get_rent()
        .await
        .expect("chain Rent")
        .minimum_balance(0);
    submit(
        &mut context,
        &[
            transfer(
                &payer,
                &fixture.certificate,
                Rent::default().minimum_balance(RESOLUTION_CERTIFICATE_BYTES_V2),
            ),
            transfer(&payer, &resolver.pubkey(), resolver_rent),
        ],
    )
    .await
    .expect("prepay the terminal certificate and establish the distinct resolver");
    let provider_execute = build_provider_execute_v3(
        &provider_execute_snapshot(&mut context, &fixture, provider_submit.lifecycle).await,
        provider_execute_deployment(&fixture),
        &ProviderExecuteIntentV3 {
            resolver: resolver.pubkey(),
            terminal_sequence: TERMINAL_SEQUENCE,
            post_update_body,
        },
    )
    .expect("chain-derived Core provider execution");
    pyth_provider::submit(&mut context, &[provider_execute.instruction], &[&resolver])
        .await
        .expect("Core admits the terminal state of an atomically founded Market");

    let terminal = CoreState::decode(
        &observed(&mut context, fixture.market)
            .await
            .expect("terminal Market")
            .data,
    )
    .expect("terminal Core state");
    assert_eq!(terminal.phase, Phase::Terminal);
    assert_eq!(
        terminal.terminal_receipt,
        Some(CoreIdentity::new(fixture.certificate.to_bytes()).expect("certificate identity")),
    );
    assert_eq!(
        SourceResolutionStateV2::decode(
            &observed(&mut context, fixture.source)
                .await
                .expect("resolved Source")
                .data,
        )
        .expect("resolved Source state")
        .phase(),
        SourceResolutionPhaseV1::Resolved
    );
    let certificate = ResolutionCertificateV2::decode(
        &observed(&mut context, fixture.certificate)
            .await
            .expect("terminal certificate")
            .data,
    )
    .expect("terminal Resolution certificate");
    assert_eq!(
        certificate.kind,
        ResolutionCertificateKindV2::ResolutionSuccess
    );
    assert_eq!(certificate.market, fixture.market.to_bytes());
    assert_eq!(certificate.generation, GENERATION);
    assert_eq!(certificate.selector, terminal.terminal_winner);

    // Hostile 4 — the admission stops at the terminal receipt. A Market that
    // has resolved may not create a second Fund, which is the conjunct that
    // keeps `Terminal`, `Retiring` and `Retired` out of the admission even
    // though this test's Market is still the same account it always was.
    assert!(
        build_resolution_create_fund_v3(&create_snapshot(&mut context, &fixture).await).is_err(),
        "a Market carrying a terminal receipt must not create a Resolution Fund"
    );
}
