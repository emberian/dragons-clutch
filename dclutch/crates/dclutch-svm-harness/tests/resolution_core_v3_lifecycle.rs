// Real-SVM evidence for the current Resolution terminal-retirement waist.
//
// The fixture loads compiled Registry, Core, Resolution, and Custody ELFs. It
// executes exact Resolution funding creation/readiness into canonical Custody
// opening, and separately starts from an authenticated provider-produced
// terminal Source/certificate boundary to execute chain-derived terminal
// admission and closure. A deliberately stale retirement instruction proves
// rollback of the physical close across Core, Source, the subset ledger, the
// closure output, and the immutable RentCredit beneficiary.

use std::{env, fs, path::PathBuf};

#[path = "support/pyth_provider.rs"]
#[allow(dead_code)]
mod pyth_provider;

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    CapabilityEntryV1, CapabilityFundingLedgerDerivationV2, CapabilityManifestV1,
    CompartmentFundingV1, ContentId as CapabilityContentId, FUNDING_STATE_BYTES, FundingAmountsV1,
    FundingLedgerStatusV2, FundingLedgerV2, FundingQuoteV1, MANIFEST_HEADER_BYTES,
    MAX_DEPENDENCIES_PER_CAPABILITY, funding_ledger_bytes_v2,
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1, SelectedRecordBumpsV1,
};
use dclutch_core_contract::ContentId as CoreContentId;
use dclutch_custody_contract::{
    CallerRoleV1, CompartmentV1, ContextV1, CustodyAuthoritySeedsV1, CustodyReplaySeedsV1,
    CustodyReplayV1, CustodyRequestV1, CustodyVaultSeedsV1, OperationV1,
};
use dclutch_direct_codec::{
    execution_v3::DIRECT_SUCCESSOR_KIND_ID_V3,
    successor::{
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1, DIRECT_ROOT_SCHEMA_ID_V1, DIRECT_ROOT_STATE_BYTES_V1,
        DirectExecutionConfigV1, DirectRootStateV1,
    },
};
use dclutch_market_core_codec::{
    Action, CoreState, Identity as CoreIdentity, MarketCoreStateSeedsV2, MarketIdentity, Phase,
    REQUEST_BYTES, Readiness, Request, StateBumpsV1,
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
use dclutch_program_test_evidence::TransactionEvidence;
use dclutch_provider_transport_v3_operator::{
    ProviderExecuteDeploymentV3, ProviderExecuteIntentV3, ProviderExecuteSnapshotV3,
    ProviderReclaimDeploymentV3, ProviderSubmitDeploymentV3, ProviderSubmitIntentV3,
    ProviderSubmitSnapshotV3, ProviderTransportOperatorErrorV3, build_provider_abandon_v3,
    build_provider_execute_v3, build_provider_reclaim_v3, build_provider_submit_v3,
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
use dclutch_relay_contract::instruction::CommitDeadlineFailureInstructionV1;
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, CallerAuthoritySeedsV1, CapabilityExecutionSelectionV1,
    ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2, ProgramIdentityV1,
    ProtocolInfrastructureProfileV2,
};
use dclutch_resolution_codec::{
    FUNDING_ACTIVATION_RECEIPT_PDA_DOMAIN_V1, PROVIDER_UPDATE_LIFECYCLE_BYTES_V3,
    PYTH_RELEASE_RECORD_SCHEMA_ID_V1, ProviderUpdateLifecycleV3, ProviderUpdateStatusV3,
    RESOLUTION_CERTIFICATE_BYTES_V2, RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
    RESOLUTION_CONTROLLER_RELEASE_ID_V7, ResolutionCertificateKindV2, ResolutionCertificateV2,
    SOURCE_CLOSURE_RECEIPT_BYTES_V3, SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V3, SourceClosureReceiptV3,
};
use dclutch_resolution_core_v3_operator::{
    Finality, Observation, ObservedAccount, ResolutionActivateFundSnapshotV1,
    ResolutionAdmitTerminalSnapshotV3, ResolutionCloseFundSnapshotV3,
    ResolutionCoreOperatorErrorV3, ResolutionCreateFundSnapshotV3,
    ResolutionDirectCloseFundReportV1, ResolutionVerifyFundReadySnapshotV3,
    build_resolution_activate_fund_v1, build_resolution_admit_terminal_v3,
    build_resolution_create_fund_v3, build_resolution_direct_close_fund_v1,
    build_resolution_verify_fund_ready_v3, validate_resolution_create_fund_report_v3,
    validate_resolution_verify_fund_ready_report_v3,
};
use dclutch_resolution_proof_sbf::ResolutionError;
use dclutch_source_contract::{
    CapacityEnvelope, ContentId as SourceContentId, PROVIDER_RELEASE_SCHEMA_ID_V1,
    PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1, ProviderReleaseV1, PythAdapterConfigV1,
    RECOVERY_POLICY_SCHEMA_ID_V2, RecoveryAttemptV2, RecoveryPolicyV2, RoundingBoundary,
    SOURCE_FAILURE_POLICY_RELEASE_ID_V2, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
    SOURCE_RESOLUTION_STATE_BYTES_V2, SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
    SOURCE_SPEC_SCHEMA_ID_V1, STATISTIC_SPEC_SCHEMA_ID_V1, SourceAccessProfile,
    SourceCapacityProfileV1, SourceMaterialV3, SourceResolutionPhaseV1, SourceResolutionRouteV1,
    SourceResolutionStateV2, SourceSpecV1, StatisticKind, StatisticSpecV1,
    WINDOW_SPEC_SCHEMA_ID_V1, WindowKind, WindowSpecV1,
};
use dclutch_token_svm::{LEGACY_TOKEN_PROGRAM_ID, PRODUCTION_ADAPTER_RELEASES};
use solana_account::{Account, AccountSharedData};
use solana_address_lookup_table_interface::instruction::{
    create_lookup_table, extend_lookup_table, freeze_lookup_table,
};
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
use solana_transaction::{
    InstructionError, Transaction, TransactionError, versioned::VersionedTransaction,
};
use spl_token_interface::state::Mint as SplMint;

const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x71; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x72; 32]);
const RESOLUTION_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x73; 32]);
const RENT_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x74; 32]);
/// Fixture-local seed domain for the Market rent beneficiary; see its use below.
const RENT_BENEFICIARY_FIXTURE_DOMAIN: &[u8] = b"dclutch/test-rent-beneficiary";
const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x75; 32]);
/// The REAL Trading and Claims programs, distinct from every other role.
///
/// Until 2026-09-02 this campaign had neither. It satisfied the five-role
/// execution release set with THREE ELFs wearing five hats -- the Core program
/// activated in the Claims role and the Custody program in the Trading role --
/// which is enough to resolve a Core Market and is not enough to resolve a
/// market carrying a capability root, because a root is a PDA of the Trading
/// role and the walk authenticates the release-selected program per role.
const CLAIMS_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x78; 32]);
const TRADING_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x79; 32]);
const GENERATION: u64 = 7;
/// The Direct row's prepaid-lazy activation deadline. Never reached: nothing on
/// the Resolution walk reads it, which is the point.
const DIRECT_ACTIVATION_DEADLINE_SLOT: u64 = 1_000_000;
const DIRECT_PRICE_SCALE: u64 = 1_000;
const DIRECT_FEE_BASIS_POINTS: u16 = 25;
const TERMINAL_SEQUENCE: u64 = 1;
const TERMINAL_TIME: i64 = 1_787_431_680;
/// A wall clock strictly past the market's own primary deadline
/// (`window.end + max_age`), which is the only time a deadline walk exists at.
/// `exhaust_after_primary_deadline` refuses at or before it.
const FAILURE_TIME: i64 = TERMINAL_TIME + 20;
/// The window's liveness grace, and therefore the distance from the window's
/// closed upper bound to this market's primary deadline. Named once so the
/// walk campaign derives the deadline it stands on from the same number the
/// window record carries, instead of restating it.
const WINDOW_MAX_AGE_SECONDS: u32 = 10;
const BOUNTY: u64 = 7;

struct Elves {
    core: Vec<u8>,
    custody: Vec<u8>,
    registry: Vec<u8>,
    resolution: Vec<u8>,
    trading: Vec<u8>,
    claims: Vec<u8>,
}

#[derive(Clone, Copy)]
struct RecordPair {
    raw: Pubkey,
    staging: Pubkey,
    /// The canonical bumps of the pair, which is what an activation records
    /// into a capability root header so later readers derive instead of
    /// searching. Carried here so the Direct root this fixture plants states
    /// the same four bumps a real activation would have.
    raw_bump: u8,
    staging_bump: u8,
}

struct Fixture {
    test: Option<ProgramTest>,
    /// The Direct capability root: a PDA of the release-selected Trading
    /// program, owned by it, and read by nothing on the Resolution walk.
    direct_root: Pubkey,
    /// The manifest row the Direct capability occupies, discovered from the
    /// kind_id ordering rather than chosen.
    direct_capability_entry_index: u16,
    /// The three rows the Resolution funding subset selects: every row that is
    /// not the Direct one.
    resolution_entry_indices: [u16; 3],
    /// Those three rows as the ledger's own `selected_mask`.
    resolution_selected_mask: u16,
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
    trading_programdata: Pubkey,
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
    source_material_id: [u8; 32],
    window: RecordPair,
    /// A second, perfectly well-formed, finalized `WindowSpecV1` record whose
    /// only difference is a later closing bound. Record publication is
    /// permissionless, so this is a record anyone can put on chain; it exists
    /// so a hostile can TRY to move this market's window rather than merely
    /// assert that nothing does.
    widened_window: RecordPair,
    statistic: RecordPair,
    pyth_release: RecordPair,
    capability_manifest: RecordPair,
    recovery_policy: RecordPair,
    product: RecordPair,
    domain: RecordPair,
    portfolio: RecordPair,
    source: Pubkey,
    funding: Pubkey,
    activation_receipt: Pubkey,
    certificate: Pubkey,
    closure: Pubkey,
    rent_credit: Pubkey,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RetirementSnapshot {
    market: Option<Account>,
    source: Option<Account>,
    funding: Option<Account>,
    certificate: Option<Account>,
    closure: Option<Account>,
    rent_credit: Option<Account>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OpenRollbackSnapshot {
    market: Option<Account>,
    source: Option<Account>,
    funding: Option<Account>,
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
    funding: Option<Account>,
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
        trading: fs::read(directory.join("dclutch_trading_sbf.so")).expect("Trading ELF"),
        claims: fs::read(directory.join("dclutch_claims_sbf.so")).expect("Claims ELF"),
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

/// The activated five-role release set, with FIVE REAL PROGRAMS in it.
///
/// It used to be built from three: `binding(core)` stood in the Claims slot and
/// `binding(custody)` in the Trading slot, and the role loop activated the same
/// two substitutes. That is sound for a campaign that only resolves a Core
/// Market -- neither role is invoked -- and it is exactly what stopped this
/// campaign from resolving a market that carries a Direct capability root,
/// whose address is a PDA of the TRADING role and whose walk authenticates the
/// release-selected program for each role it enters.
fn activation(
    core: ArtifactReleaseV1,
    claims: ArtifactReleaseV1,
    trading: ArtifactReleaseV1,
    resolution: ArtifactReleaseV1,
    custody: ArtifactReleaseV1,
) -> ([u8; 32], Vec<u8>) {
    let release_set = ExecutionReleaseSetV1::new(
        binding(core),
        binding(claims),
        binding(trading),
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
        (ExecutionRoleV1::Claims, claims),
        (ExecutionRoleV1::Trading, trading),
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
    let (raw, raw_bump) = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
        &REGISTRY_PROGRAM_ID,
    );
    let (staging, staging_bump) = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
        &REGISTRY_PROGRAM_ID,
    );
    test.add_account(raw, protocol_account(REGISTRY_PROGRAM_ID, data));
    RecordPair {
        raw,
        staging,
        raw_bump,
        staging_bump,
    }
}

fn add_active_funding(
    test: &mut ProgramTest,
    market: Pubkey,
    manifest_id: CapabilityContentId,
    manifest: CapabilityManifestV1<'_>,
    entries: [u16; 3],
) -> Pubkey {
    let selected_mask = entries
        .into_iter()
        .fold(0_u16, |mask, entry| mask | (1_u16 << entry));
    let width = funding_ledger_bytes_v2(3).expect("three-row FundingLedgerV2 width");
    let rent = Rent::default().minimum_balance(width);
    let mut state = vec![0_u8; width];
    FundingLedgerV2::initialize(&mut state, manifest_id, manifest, selected_mask)
        .expect("pending FundingLedgerV2");
    for entry_index in entries {
        FundingLedgerV2::activate_in_place(&mut state, manifest_id, manifest, entry_index, 1)
            .expect("active FundingLedgerV2 row");
    }
    let authenticated = FundingLedgerV2::decode(&state)
        .and_then(|ledger| ledger.authenticate(manifest_id, manifest))
        .expect("authenticated active FundingLedgerV2");
    let remaining = authenticated
        .remaining_native_lamports_total()
        .expect("bounded aggregate native principal");
    let key = funding_key(market, manifest_id, manifest, selected_mask);
    test.add_account(
        key,
        Account {
            lamports: rent + remaining,
            data: state,
            owner: RESOLUTION_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    key
}

fn add_pending_funding(
    test: &mut ProgramTest,
    market: Pubkey,
    manifest_id: CapabilityContentId,
    manifest: CapabilityManifestV1<'_>,
    entries: [u16; 3],
) -> Pubkey {
    let selected_mask = entries
        .into_iter()
        .fold(0_u16, |mask, entry| mask | (1_u16 << entry));
    let width = funding_ledger_bytes_v2(3).expect("three-row FundingLedgerV2 width");
    let rent = Rent::default().minimum_balance(width);
    let mut state = vec![0_u8; width];
    FundingLedgerV2::initialize(&mut state, manifest_id, manifest, selected_mask)
        .expect("pre-Market Pending FundingLedgerV2");
    let authenticated = FundingLedgerV2::decode(&state)
        .and_then(|ledger| ledger.authenticate(manifest_id, manifest))
        .expect("authenticated pre-Market Pending FundingLedgerV2");
    let principal = authenticated
        .remaining_native_lamports_total()
        .expect("bounded aggregate native principal");
    let key = funding_key(market, manifest_id, manifest, selected_mask);
    test.add_account(
        key,
        Account {
            lamports: rent + principal,
            data: state,
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
    selected_mask: u16,
) -> Pubkey {
    let width = funding_ledger_bytes_v2(3).expect("three-row FundingLedgerV2 width");
    let mut state = vec![0_u8; width];
    FundingLedgerV2::initialize(&mut state, manifest_id, manifest, selected_mask)
        .expect("pending FundingLedgerV2");
    let ledger = FundingLedgerV2::decode(&state).expect("FundingLedgerV2");
    let derivation = CapabilityFundingLedgerDerivationV2::new(
        RESOLUTION_PROGRAM_ID.to_bytes(),
        market.to_bytes(),
        GENERATION,
        manifest_id,
        ledger,
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
    /// The same terminal shape, reached the other way: a market founded with
    /// NO recovery policy whose primary deadline passed with no answer, so the
    /// Source walked `Primary → Exhausted → FailureCommitted` and the
    /// certificate it minted is a `ResolutionFailure` at the Product's own
    /// pre-disclosed failure region, with no route and no provider evidence
    /// behind it.
    ///
    /// This is the prestate a funded deadline walk leaves. The no-recovery
    /// material is not a simplification: `exhaust_after_primary_deadline`
    /// refuses a market that still has recovery attempts owed to it, so a
    /// deadline walk only exists on this shape.
    TerminalFailure,
    /// The prestate `TerminalFailure` is the POSTSTATE of.
    ///
    /// `Open + Consumed`, one exact Pending subset ledger, no Source state and
    /// no certificate — the same shape as `AtomicallyFounded`, and the only
    /// difference is that the certificate address this fixture derives carries
    /// the `ResolutionFailure` kind tag, because that is the seat the walk will
    /// mint into.
    ///
    /// A seeded terminal proves the shape of an ending and nothing about how it
    /// is reached. This one is reached: the market is founded, funded and
    /// activated, then nobody answers, and the deadline walk carries it to the
    /// Product's own pre-disclosed failure region and pays whoever walked it.
    WalkableFailure,
}

impl MarketPrestateV1 {
    /// Whether the Market account starts `Open + Consumed` rather than
    /// `Founding + Prepaid`.
    const fn open(self) -> bool {
        matches!(
            self,
            Self::AtomicallyFounded
                | Self::Terminal
                | Self::TerminalFailure
                | Self::WalkableFailure
        )
    }

    /// Whether the Source state, its subset ledger and the terminal certificate
    /// are seeded as already-terminal rather than left to be created.
    const fn preload_terminal(self) -> bool {
        matches!(self, Self::Terminal | Self::TerminalFailure)
    }

    /// Whether this market's terminal is a deadline walk rather than a
    /// provider. It selects the certificate PDA's kind tag for every prestate,
    /// and where the terminal is preloaded it also selects the Source
    /// transition and the certificate kind, because they are one fact about one
    /// market. `WalkableFailure` preloads nothing and takes only the tag: the
    /// address the walk will mint into has to exist before the walk runs, and
    /// success and failure are different addresses for one Source at one
    /// sequence.
    const fn failure_terms(self) -> bool {
        matches!(self, Self::TerminalFailure | Self::WalkableFailure)
    }
}

fn fixture(prestate: MarketPrestateV1) -> Fixture {
    let preload_terminal = prestate.preload_terminal();
    let failure_terms = prestate.failure_terms();
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

    add_program(
        &mut test,
        "dclutch_claims_sbf",
        CLAIMS_PROGRAM_ID,
        &elves.claims,
    );
    add_program(
        &mut test,
        "dclutch_trading_sbf",
        TRADING_PROGRAM_ID,
        &elves.trading,
    );

    let core_release = release(CORE_PROGRAM_ID, [0x41; 32], &elves.core);
    let claims_release = release(CLAIMS_PROGRAM_ID, [0x47; 32], &elves.claims);
    let trading_release = release(TRADING_PROGRAM_ID, [0x48; 32], &elves.trading);
    let resolution_release = release(
        RESOLUTION_PROGRAM_ID,
        RESOLUTION_CONTROLLER_RELEASE_ID_V7,
        &elves.resolution,
    );
    let custody_release = release(CUSTODY_PROGRAM_ID, [0x42; 32], &elves.custody);
    let registry_release = release(REGISTRY_PROGRAM_ID, [0x43; 32], &elves.registry);
    let (release_set, activation_data) = activation(
        core_release,
        claims_release,
        trading_release,
        resolution_release,
        custody_release,
    );
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
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
        &CORE_PROGRAM_ID,
    )
    .0;
    let rent_binding = ExecutionRoleBindingV1::new(
        program_identity(sysvar::rent::ID),
        ArtifactReleaseIdV1::new([0x44; 32]).expect("rent artifact identity"),
    );
    // Registry moved across the succession and Rent did not: the predecessor
    // Registry id names the distinct release this profile succeeded, while
    // Rent holds the same id on both sides of it.
    let predecessor_registry_release = release(REGISTRY_PROGRAM_ID, [0xb3; 32], &elves.registry);
    let infrastructure_value = ProtocolInfrastructureProfileV2::new(
        binding(registry_release),
        rent_binding,
        artifact_id(predecessor_registry_release),
        rent_binding.artifact_release(),
    )
    .expect("infrastructure succession profile");
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
        WINDOW_MAX_AGE_SECONDS,
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
    let material_value = SourceMaterialV3::explicitly_unbounded(
        source_id(product_record_id),
        source_id(source_spec_id),
        source_id(window_id),
        source_id(statistic_id),
        // A funded deadline walk only exists where nothing else is owed: with a
        // recovery policy still unspent, `exhaust_after_primary_deadline`
        // refuses, because a market that has attempts left has not run out of
        // ways to be answered honestly.
        //
        // No prestate carries one any more. `SourceResolutionStateV2` has no
        // transition that advances a recovery attempt -- `funded.rs` plans the
        // whole walk as `Primary -> Exhausted -> FailureCommitted` -- so
        // `12d0deb5` welded `build_resolution_create_fund_v3` shut against
        // recovery-bearing material. A recovery-bearing prestate would
        // therefore assert a poststate no founding can reach.
        //
        // This comment used to add that the per-leg `FailNext` route "sits
        // under `cfg(any())` in the Resolution program's dispatch". It does
        // not: that block and the other thirteen were deleted, and the V1
        // ladder they gated has no definition anywhere. The weld's real and
        // checkable premise is the first clause -- there is no transition that
        // advances a recovery attempt -- which is why the sentence survives
        // without it.
        None,
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
            id(RESOLUTION_CONTROLLER_RELEASE_ID_V7),
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
    // The Direct capability, and the reason this manifest has four rows.
    //
    // Until 2026-09-02 this campaign resolved a market whose manifest was three
    // Resolution rows and nothing else -- a shape no Direct market has. A market
    // carrying a Direct capability ROOT carries the capability that owns the
    // root in its manifest, at a row the Resolution funding subset does not
    // select, prepaid-lazy and funded by the Trading role rather than by the
    // walk. The identities below are the REAL ones
    // (`dclutch-direct-codec`), not fixture bytes: `DIRECT_SUCCESSOR_KIND_ID_V3`
    // is what a Direct capability IS and `DIRECT_ROOT_SCHEMA_ID_V1` is what its
    // child account is, and stating them from their owning crate is what makes
    // the manifest a Direct market's manifest rather than a fourth arbitrary
    // row.
    let direct_config_bytes =
        DirectExecutionConfigV1::new(DIRECT_PRICE_SCALE, DIRECT_FEE_BASIS_POINTS, [0xd1; 32])
            .expect("Direct execution config")
            .encode()
            .to_vec();
    let direct_config = add_record(
        &mut test,
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
        direct_config_bytes.clone(),
    );
    let direct_config_id = hash(&direct_config_bytes).to_bytes();
    // The capability release the Direct row selects. Fixture-local ON PURPOSE
    // and said out loud: the ProgramSet record it would name is authenticated
    // by the TRADING role at founding and by the Direct lifecycle routes, and
    // by nothing on the Resolution walk -- Resolution decodes the manifest and
    // never opens a capability's release, its config, or its root. Authoring a
    // real one here would be a second authority for a fact this campaign does
    // not test.
    let direct_capability_release = [0xd2; 32];
    let direct_entry =
        CapabilityEntryV1::new(
            id(DIRECT_SUCCESSOR_KIND_ID_V3),
            id(direct_capability_release),
            id(direct_config_id),
            id([0xd3; 32]),
            id(DIRECT_ROOT_SCHEMA_ID_V1),
            id([0xd4; 32]),
            ActivationPolicy::PrepaidLazy,
            DIRECT_ACTIVATION_DEADLINE_SLOT,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            FundingQuoteV1::new(
                FundingAmountsV1::new(
                    // The exact rent of the root account this row owns, which is
                    // how `selected_manifest_entry_v1` quotes it in production.
                    CompartmentFundingV1::native_lamports(Rent::default().minimum_balance(
                        CAPABILITY_ROOT_HEADER_BYTES_V1 + DIRECT_ROOT_STATE_BYTES_V1,
                    ))
                    .expect("Direct root rent compartment"),
                    CompartmentFundingV1::not_applicable(),
                    CompartmentFundingV1::not_applicable(),
                    CompartmentFundingV1::not_applicable(),
                    CompartmentFundingV1::not_applicable(),
                    CompartmentFundingV1::not_applicable(),
                    CompartmentFundingV1::not_applicable(),
                )
                .expect("Direct typed funding"),
                None,
            )
            .expect("Direct funding quote"),
        )
        .expect("Direct capability entry");
    // Rows are kind_id-ASCENDING and `encode_into` refuses any other order, so
    // the Direct row's index is DISCOVERED rather than chosen: a real Direct
    // kind is a real digest and it lands wherever it lands among the fixture's
    // placeholder Resolution kinds. `merge_selected_manifest_v1` does exactly
    // this in production
    // (`tools/local-validator/bootstrap/successor/src/selected_capability.rs`),
    // which is the reason a fourth non-Resolution row is a shape the funding
    // walk already has to survive.
    let mut entries = vec![entries[0], entries[1], entries[2], direct_entry];
    entries.sort_by(|left, right| left.kind_id().to_bytes().cmp(&right.kind_id().to_bytes()));
    let direct_capability_entry_index = u16::try_from(
        entries
            .iter()
            .position(|entry| entry.kind_id().to_bytes() == DIRECT_SUCCESSOR_KIND_ID_V3)
            .expect("the Direct row is in the manifest"),
    )
    .expect("manifest row index");
    let resolution_entry_indices: [u16; 3] = core::array::from_fn(|slot| {
        let mut rows = (0..4_u16).filter(|row| *row != direct_capability_entry_index);
        rows.nth(slot).expect("three Resolution rows")
    });
    let resolution_selected_mask = resolution_entry_indices
        .into_iter()
        .fold(0_u16, |mask, entry| mask | (1_u16 << entry));
    let mut manifest_bytes = vec![0; MANIFEST_HEADER_BYTES + 4 * CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&entries, &mut manifest_bytes).expect("capability manifest");
    let manifest_id_bytes = hash(&manifest_bytes).to_bytes();
    let manifest = CapabilityManifestV1::decode(&manifest_bytes).expect("manifest view");

    let source_material = add_record(
        &mut test,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
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
    // The same window with its closing bound pushed an hour out, published as
    // its own finalized record. Nothing about it is malformed: same source
    // spec, same kind, same skew, same schedule identity. It is exactly what
    // an operator who wanted a later publication to count would author.
    let widened_window_bytes = WindowSpecV1::new(
        source_id(source_spec_id),
        WindowKind::Terminal,
        update_view.publish_time() - 300,
        update_view.publish_time() + 3_600,
        10,
        1,
        source_id([0x98; 32]),
    )
    .expect("widened terminal window")
    .to_bytes();
    assert_ne!(widened_window_bytes, window_bytes);
    let widened_window = add_record(
        &mut test,
        WINDOW_SPEC_SCHEMA_ID_V1,
        widened_window_bytes.to_vec(),
    );
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

    // The Market's rent beneficiary. Core compares this account BY KEY against
    // its own persisted `rent_beneficiary` and credits lamports to it; nothing
    // on any path under test decodes its bytes, and this test only ever reads
    // its lamports. It is deliberately not a `LifecycleRentCreditV2`: V2 is
    // keyed by [domain, market, generation], and this Market's own address is
    // derived from an identity that already carries the beneficiary, so a V2
    // credit cannot be the beneficiary of the Market it is keyed by in this
    // ordering. It is also no longer a `RentCreditV1` -- that record was
    // deleted with the last route that could create one.
    let (rent_credit, _) = Pubkey::find_program_address(
        &[RENT_BENEFICIARY_FIXTURE_DOMAIN, &[0xb1; 32]],
        &RENT_PROGRAM_ID,
    );
    test.add_account(
        rent_credit,
        protocol_account(RENT_PROGRAM_ID, std::vec![0_u8; 128]),
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
        principal_cap_sets: u64::MAX,
        rent_beneficiary: CoreIdentity::new(rent_credit.to_bytes()).expect("RentCredit"),
        terminal_receipt: None,
        bumps: StateBumpsV1::UNRECORDED,
    };
    test.add_account(
        market,
        protocol_account(
            CORE_PROGRAM_ID,
            state.encode().expect("Core state").to_vec(),
        ),
    );

    // The Direct capability ROOT: a PDA of the real Trading program, owned by
    // it, carrying the immutable activation header and the mutable `Open`
    // Direct tail. This is the account whose existence makes the Market above a
    // market carrying a Direct capability root rather than a plain Core Market,
    // and it is why the Trading role in this campaign's release set had to stop
    // being the Custody ELF: the address is derived under the release-selected
    // Trading program, so with a substituted Trading role it is a different
    // address entirely.
    //
    // Nothing on the Resolution walk reads it, and that is the invariant the
    // campaign now checks rather than assumes -- `an_atomically_founded_market_reaches_a_terminal_certificate`
    // proves the root is byte-identical and lamport-identical after the whole
    // resolution, so "Resolution does not touch a capability's root" is a
    // measurement here and not a reading of the source.
    let direct_root_header = CapabilityRootHeaderV1::new(
        CoreContentId::new(release_set).expect("release set identity"),
        market.to_bytes(),
        GENERATION,
        CapabilityExecutionSelectionV1::new(
            direct_capability_entry_index,
            CoreContentId::new(manifest_id_bytes).expect("manifest identity"),
            CoreContentId::new(DIRECT_SUCCESSOR_KIND_ID_V3).expect("Direct kind"),
            CoreContentId::new(direct_capability_release).expect("capability release"),
            CoreContentId::new(direct_config_id).expect("Direct config"),
        )
        .expect("Direct activation selection"),
        SelectedRecordBumpsV1::new(
            capability_manifest.raw_bump,
            capability_manifest.staging_bump,
            direct_config.raw_bump,
            direct_config.staging_bump,
        ),
    )
    .expect("immutable Direct root header");
    let mut direct_root_bytes =
        Vec::with_capacity(CAPABILITY_ROOT_HEADER_BYTES_V1 + DIRECT_ROOT_STATE_BYTES_V1);
    direct_root_bytes.extend_from_slice(&direct_root_header.to_bytes());
    direct_root_bytes.extend_from_slice(&DirectRootStateV1::new().encode());
    let direct_root =
        Pubkey::find_program_address(&direct_root_header.seeds().as_slices(), &TRADING_PROGRAM_ID)
            .0;
    test.add_account(
        direct_root,
        Account {
            lamports: Rent::default().minimum_balance(direct_root_bytes.len()),
            data: direct_root_bytes,
            owner: TRADING_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
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
        add_active_funding(
            &mut test,
            market,
            manifest_id,
            manifest,
            resolution_entry_indices,
        )
    } else {
        add_pending_funding(
            &mut test,
            market,
            manifest_id,
            manifest,
            resolution_entry_indices,
        )
    };
    let activation_receipt = Pubkey::find_program_address(
        &[
            FUNDING_ACTIVATION_RECEIPT_PDA_DOMAIN_V1,
            market.as_ref(),
            &GENERATION.to_le_bytes(),
        ],
        &RESOLUTION_PROGRAM_ID,
    )
    .0;
    test.add_account(
        activation_receipt,
        Account {
            lamports: 1,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    // Success and failure are different ADDRESSES for one Source at one
    // sequence, so a walked market's certificate can never occupy the seat a
    // provider-resolved one would have taken.
    let terminal_kind_tag: u8 = if failure_terms { 4 } else { 1 };
    let certificate = Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            source.as_ref(),
            &[terminal_kind_tag],
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
        let decision = if failure_terms {
            // The funded deadline walk's own transition, host-side. Nothing
            // about a provider is an input -- no record, no observation, no
            // evidence id -- because this is exactly the world where everyone
            // responsible stopped answering. Neither leg can be conjured
            // early: `exhaust_after_primary_deadline` refuses at or before the
            // market's own deadline, and `commit_failure_from_authenticated_domain`
            // refuses anywhere but Exhausted.
            source_value
                .exhaust_after_primary_deadline(
                    source_id(material_id),
                    material_value,
                    source_id(window_id),
                    window_value,
                    GENERATION,
                    FAILURE_TIME,
                )
                .expect("a primary deadline reached with no answer exhausts the Source");
            let decision = source_value
                .commit_failure_from_authenticated_domain(
                    source_id(material_id),
                    material_value,
                    source_id(product_record_id),
                    result_domain,
                    GENERATION,
                    FAILURE_TIME,
                    TERMINAL_SEQUENCE,
                )
                .expect("the exhausted Source commits its pre-disclosed failure terms");
            assert_eq!(
                decision.selector(),
                result_domain.failure_selector(),
                "the walk selects the Product's own explicit failure region, never a chosen value"
            );
            decision
        } else {
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
            decision
        };
        test.add_account(
            source,
            protocol_account(RESOLUTION_PROGRAM_ID, source_value.to_bytes().to_vec()),
        );
        // Every field that differs is FORCED by `validate_shape`'s
        // ResolutionFailure arm, not chosen for convenience: no route and no
        // provider evidence (nobody stood behind this terminal), a nonzero
        // funding_allocation naming the market's own Source material (what
        // makes the explicit-failure compartment identifiable at all), a
        // nonzero work_paid (a walk that could not be paid for could not have
        // encoded its own certificate either), and a zero result and
        // observed_at (there was no observation to record).
        let certificate_value = ResolutionCertificateV2 {
            kind: if failure_terms {
                ResolutionCertificateKindV2::ResolutionFailure
            } else {
                ResolutionCertificateKindV2::ResolutionSuccess
            },
            market: market.to_bytes(),
            route: if failure_terms { [0; 32] } else { [0xb4; 32] },
            source_material: material_id,
            product_record_digest: product_record_id,
            provider_evidence: if failure_terms { [0; 32] } else { [0xb3; 32] },
            funding_allocation: if failure_terms { material_id } else { [0; 32] },
            receipt_account: certificate.to_bytes(),
            generation: GENERATION,
            attempt_index: 0,
            schedule_index: 0,
            selector: decision.selector(),
            work_paid: if failure_terms { BOUNTY } else { 0 },
            funding_remaining: 0,
            result_numerator: if failure_terms { 0 } else { -1 },
            result_denominator: if failure_terms { 0 } else { 1 },
            observed_at: if failure_terms {
                0
            } else {
                u64::try_from(TERMINAL_TIME).expect("positive terminal time")
            },
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
            SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V3,
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
        direct_root,
        direct_capability_entry_index,
        resolution_entry_indices,
        resolution_selected_mask,
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
        trading_programdata: programdata(TRADING_PROGRAM_ID),
        resolution_programdata: programdata(RESOLUTION_PROGRAM_ID),
        realm,
        realm_record,
        mint,
        replay,
        vault,
        custody_authority,
        source_material,
        source_material_id: material_id,
        source_spec,
        provider_release,
        adapter_config,
        window,
        widened_window,
        statistic,
        pyth_release,
        capability_manifest,
        recovery_policy,
        product,
        domain,
        portfolio,
        source,
        funding,
        activation_receipt,
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

async fn assert_funding_ledger_status(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    expected: FundingLedgerStatusV2,
) {
    let ledger_account = observed(context, fixture.funding)
        .await
        .expect("Resolution FundingLedgerV2");
    let manifest_account = observed(context, fixture.capability_manifest.raw)
        .await
        .expect("capability manifest");
    let manifest = CapabilityManifestV1::decode(&manifest_account.data).expect("manifest");
    let manifest_id = CapabilityContentId::new(hash(&manifest_account.data).to_bytes())
        .expect("manifest content identity");
    let ledger = FundingLedgerV2::decode(&ledger_account.data)
        .and_then(|ledger| ledger.authenticate(manifest_id, manifest))
        .expect("authenticated FundingLedgerV2");
    assert_eq!(
        ledger.ledger().selected_mask(),
        fixture.resolution_selected_mask
    );
    assert_eq!(ledger.ledger().slot_count(), 3);
    assert_eq!(
        ledger.ledger().selected_mask() & (1 << fixture.direct_capability_entry_index),
        0,
        "the Resolution subset must not select the Direct capability's row"
    );
    for entry_index in fixture.resolution_entry_indices {
        assert_eq!(
            ledger.slot(entry_index).expect("selected row").status(),
            expected
        );
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

/// The extent of a signed legacy transaction on the wire.
///
/// It MEASURES and does not judge, and deliberately carries no copy of Solana's
/// 1,232-byte `PACKET_DATA_BYTES`. `solana-program-test` submits no packet and
/// cannot enforce that maximum itself, so the comparison belongs in
/// `tools/gauntlet/resolution-core-v3/witnesses.json`, where the campaign
/// cannot quietly satisfy it.
fn wire_extent(signatures: usize, message: &[u8]) -> usize {
    1 + signatures * 64 + message.len()
}

/// Solana's serialized transaction packet maximum.
///
/// Restated here because this harness resolves independently of the protocol
/// workspace and cannot link `dclutch_versioned_message_operator`, which owns
/// the constant: that crate pins `solana-hash =4.6.0` and
/// `solana-address-lookup-table-interface =3.2.0` against this harness's
/// `solana-message =4.4.1` and `=3.1.0`, and the two pin sets have no common
/// solution.
///
/// `market_retirement_v1_lifecycle.rs`, which `include!`s this file, states the
/// same number a third time as `SOLANA_PACKET_BYTES`. Retiring that one belongs
/// with converting the retirement checkpoint chain and is not done here: the
/// crate that binary needs (`dclutch-representation-composition-v3-kernel`) does
/// not compile at this moment, five files dirty under another lane mid-rename of
/// `generated_abi`, so an edit to that file could not be proved.
const PACKET_DATA_BYTES: usize = 1_232;

/// Addresses per table-extension transaction, bounded so the extension itself
/// stays a packet.
const EXTEND_ADDRESSES_PER_TRANSACTION_V1: usize = 20;

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
    /// The signed v0 message over the route's own derived frozen table.
    v0_bytes: usize,
    /// Addresses the v0 message still carries inline -- payer, program ids and
    /// signers, which no table can move.
    static_keys: usize,
    /// Addresses the table resolved.
    loaded_addresses: usize,
}

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
/// `PACKET_DATA_BYTES` above); the two agree by construction rather than by
/// discipline, because neither states the rule.
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
    let probe = solana_message::v0::Message::try_compile(
        &payer,
        instructions,
        &[solana_message::AddressLookupTableAccount {
            key: probe_key,
            addresses: candidates.clone(),
        }],
        solana_sdk::hash::Hash::default(),
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
/// Freezing is not tidiness. A mutable table is a second authority over which
/// addresses a submitted message actually resolves to, which is the
/// substitution the Pyth caller refuses by name; freezing makes the routing
/// data as fixed as the instruction bytes it routes. The rent is permanent and
/// intended.
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
    submit(context, &[create])
        .await
        .expect("create the route's lookup table");
    for chunk in addresses.chunks(EXTEND_ADDRESSES_PER_TRANSACTION_V1) {
        submit(
            context,
            &[extend_lookup_table(
                table,
                payer,
                Some(payer),
                chunk.to_vec(),
            )],
        )
        .await
        .expect("extend the route's lookup table");
    }
    submit(context, &[freeze_lookup_table(table, payer)])
        .await
        .expect("freeze the route's lookup table");
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

/// Submit one labelled step as a v0 message over a table frozen for exactly
/// this route, and report both extents.
///
/// The legacy extent is compiled from the identical instructions and thrown
/// away unsubmitted. It is the control: it is what this route used to be, and
/// without it a converted figure says nothing about whether the route moved or
/// the instrument did.
async fn submit_recorded_v0(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    signers: &[&Keypair],
    label: &str,
) -> (Result<(), BanksClientError>, PacketExtentV1) {
    let (table, addresses) = frozen_route_lookup_table(context, instructions).await;
    compile_submit_and_record_v0(context, instructions, signers, label, table, &addresses).await
}

/// The same over a table already frozen for a CHAIN of routes, asserting the
/// extent the route was expected to have.
///
/// A multi-transaction chain over one market's retirement takes ONE table, not
/// one per route: the frames share their coordinates, and a table per route is
/// a rent per route for a single act. Building it before the chain runs is also
/// what a real controller must do -- the addresses have to be finalized before
/// the first submission can resolve them.
async fn submit_recorded_over_table_v0(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    signers: &[&Keypair],
    label: &str,
    table: Pubkey,
    addresses: &[Pubkey],
    expected: PacketExtentV1,
) -> Result<(), BanksClientError> {
    let (outcome, extent) =
        compile_submit_and_record_v0(context, instructions, signers, label, table, addresses).await;
    assert_eq!(extent, expected, "{label}");
    assert!(
        expected.legacy_bytes > PACKET_DATA_BYTES,
        "{label}: a route that already fit needs no table"
    );
    assert!(
        expected.v0_bytes <= PACKET_DATA_BYTES,
        "{label}: the table did not close the overrun"
    );
    outcome
}

async fn compile_submit_and_record_v0(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    signers: &[&Keypair],
    label: &str,
    table: Pubkey,
    addresses: &[Pubkey],
) -> (Result<(), BanksClientError>, PacketExtentV1) {
    let addresses = addresses.to_vec();
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let payer = context.payer.pubkey();
    let mut all: Vec<&dyn Signer> = Vec::with_capacity(signers.len() + 1);
    all.push(&context.payer);
    all.extend(signers.iter().copied().map(|signer| signer as &dyn Signer));
    let legacy = Transaction::new_signed_with_payer(instructions, Some(&payer), &all, blockhash);
    let legacy_bytes = wire_extent(legacy.signatures.len(), &legacy.message.serialize());
    let compiled = solana_message::v0::Message::try_compile(
        &payer,
        instructions,
        &[solana_message::AddressLookupTableAccount {
            key: table,
            addresses,
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
    let transaction =
        VersionedTransaction::try_new(solana_message::VersionedMessage::V0(compiled), &all)
            .expect("signed v0 transaction");
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
    let failure = processed
        .result
        .clone()
        .err()
        .map(|error| format!("{error:?}"));
    let (logs, units) = processed
        .metadata
        .as_ref()
        .map(|metadata| {
            (
                metadata.log_messages.clone(),
                metadata.compute_units_consumed,
            )
        })
        .unwrap_or_default();
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
    (
        processed.result.map_err(BanksClientError::TransactionError),
        extent,
    )
}

/// Submit one labelled step and record the runtime's own account of it.
///
/// The evidence is emitted BEFORE the caller asserts anything, so a step that
/// fails its own assertion still leaves behind what the chain did. Only the
/// steps `tools/gauntlet/resolution-core-v3/bindings.json` names come through
/// here: this campaign submits well over fifty transactions and a campaign that
/// labelled every one of them would be claiming coverage no binding was written
/// for. Each label names exactly one transaction, which is what lets a binding
/// carry one outcome and one refusal code.
async fn submit_recorded(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    signers: &[&Keypair],
    label: &str,
) -> Result<(), BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let mut all: Vec<&dyn Signer> = Vec::with_capacity(signers.len() + 1);
    all.push(&context.payer);
    all.extend(signers.iter().copied().map(|signer| signer as &dyn Signer));
    let transaction = Transaction::new_signed_with_payer(
        instructions,
        Some(&context.payer.pubkey()),
        &all,
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
        .await?;
    let failure = processed
        .result
        .clone()
        .err()
        .map(|error| format!("{error:?}"));
    let (logs, units) = processed
        .metadata
        .as_ref()
        .map(|metadata| {
            (
                metadata.log_messages.clone(),
                metadata.compute_units_consumed,
            )
        })
        .unwrap_or_default();
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
    processed.result.map_err(BanksClientError::TransactionError)
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
    let update = fixture.update.pubkey();
    provider_execute_snapshot_for(context, fixture, lifecycle, update).await
}

/// The same observation against a named posted update, so a campaign can hold
/// more than one live provider submission at a time. First-valid semantics is
/// not observable with a single update account: the losing submission has to be
/// as real as the winning one, posted through the same Receiver ELF, or the
/// refusal under test is indistinguishable from a malformed-evidence refusal.
async fn provider_execute_snapshot_for(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    lifecycle: Pubkey,
    update: Pubkey,
) -> ProviderExecuteSnapshotV3 {
    ProviderExecuteSnapshotV3 {
        market: required_observed(context, fixture.market).await,
        source_state: required_observed(context, fixture.source).await,
        lifecycle: required_observed(context, lifecycle).await,
        update: required_observed(context, update).await,
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
        // The Trading slot of the shared Resolution frame, and the one position
        // the three-hat activation let this campaign get wrong for free: it used
        // to name `CUSTODY_PROGRAM_ID`, which was correct precisely because the
        // Trading role was activated as the Custody program. Against a release
        // set with a real Trading role the frame refuses `0x8019 ActivatedRole`.
        trading_program: TRADING_PROGRAM_ID,
        trading_programdata: fixture.trading_programdata,
        resolution_program: RESOLUTION_PROGRAM_ID,
        resolution_programdata: fixture.resolution_programdata,
        receiver_config: fixture.provider.config,
    }
}

/// The two optional-policy frame positions, resolved from the material itself.
///
/// The on-chain frame pins absent optional policy coordinates by repeating the
/// already-authenticated Source-material record pair, so every frame position
/// stays authenticated against exactly one expectation. Passing an unrelated
/// but valid policy record into a no-recovery frame is still a `Record`
/// refusal, which is why this reads the material rather than the prestate.
async fn optional_recovery_policy_pair(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
) -> (ObservedAccount, ObservedAccount) {
    let source_material = required_observed(context, fixture.source_material.raw).await;
    let source_material_staging = vacant_observed(fixture.source_material.staging);
    let material = SourceMaterialV3::decode(&source_material.data).expect("Source material");
    if material.recovery_policy().is_some() {
        (
            required_observed(context, fixture.recovery_policy.raw).await,
            vacant_observed(fixture.recovery_policy.staging),
        )
    } else {
        (source_material, source_material_staging)
    }
}

async fn create_snapshot(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
) -> ResolutionCreateFundSnapshotV3 {
    let (recovery_policy, recovery_policy_staging) =
        optional_recovery_policy_pair(context, fixture).await;
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
        funding_ledger: required_observed(context, fixture.funding).await,
        rent_sysvar: required_observed(context, sysvar::rent::ID).await,
        system_program: required_observed(context, system_program::ID).await,
        recovery_policy,
        recovery_policy_staging,
    }
}

async fn verify_snapshot(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
) -> ResolutionVerifyFundReadySnapshotV3 {
    let (recovery_policy, recovery_policy_staging) =
        optional_recovery_policy_pair(context, fixture).await;
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
        funding_ledger: required_observed(context, fixture.funding).await,
        beneficiary: required_observed(context, fixture.rent_credit).await,
        clock_sysvar: required_observed(context, sysvar::clock::ID).await,
        rent_sysvar: required_observed(context, sysvar::rent::ID).await,
        activation_receipt: observed_or_vacant(context, fixture.activation_receipt).await,
        recovery_policy,
        recovery_policy_staging,
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
        funding_ledger: required_observed(context, fixture.funding).await,
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

/// The 22-account funded deadline walk, aimed at this fixture's market.
///
/// This frame carries NO relay-family account — no relayed record, no relayer
/// key set, no adapter config. It is the Resolution program's own route and it
/// lives under the relay instruction magic only because that is where the
/// dispatcher put it, which is why a market resolved through the Pyth transport
/// can walk exactly the same way when its provider goes silent.
///
/// A caller supplies which market, when, and nothing else. The Source material
/// is read out of the Resolution-owned Source state and checked against the
/// Market's own resolution policy; the funding entry to debit is found by
/// matching each selected manifest entry's `config_id` against that same
/// material, never by an account position or an index the caller names.
fn deadline_failure_instruction(fixture: &Fixture, worker: Pubkey) -> Instruction {
    Instruction {
        program_id: RESOLUTION_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(worker, true),
            AccountMeta::new_readonly(fixture.market, false),
            AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.activation, false),
            AccountMeta::new(fixture.source, false),
            AccountMeta::new(fixture.certificate, false),
            AccountMeta::new_readonly(fixture.source_material.raw, false),
            AccountMeta::new_readonly(fixture.source_material.staging, false),
            AccountMeta::new_readonly(fixture.window.raw, false),
            AccountMeta::new_readonly(fixture.window.staging, false),
            AccountMeta::new_readonly(fixture.product.raw, false),
            AccountMeta::new_readonly(fixture.product.staging, false),
            AccountMeta::new_readonly(fixture.domain.raw, false),
            AccountMeta::new_readonly(fixture.domain.staging, false),
            AccountMeta::new_readonly(fixture.portfolio.raw, false),
            AccountMeta::new_readonly(fixture.portfolio.staging, false),
            AccountMeta::new_readonly(fixture.capability_manifest.raw, false),
            AccountMeta::new_readonly(fixture.capability_manifest.staging, false),
            AccountMeta::new(fixture.funding, false),
            AccountMeta::new_readonly(sysvar::clock::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: CommitDeadlineFailureInstructionV1::new(GENERATION, TERMINAL_SEQUENCE)
            .expect("deadline failure request")
            .to_bytes()
            .expect("deadline failure bytes")
            .to_vec(),
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
            AccountMeta::new(fixture.rent_credit, false),
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
    let source_material = required_observed(context, fixture.source_material.raw).await;
    let source_material_staging = vacant_observed(fixture.source_material.staging);
    let (recovery_policy, recovery_policy_staging) =
        optional_recovery_policy_pair(context, fixture).await;
    ResolutionCloseFundSnapshotV3 {
        market: required_observed(context, fixture.market).await,
        activation_cache: required_observed(context, fixture.activation).await,
        registry_program: required_observed(context, REGISTRY_PROGRAM_ID).await,
        core_program: required_observed(context, CORE_PROGRAM_ID).await,
        core_programdata: required_observed(context, fixture.core_programdata).await,
        resolution_program: required_observed(context, RESOLUTION_PROGRAM_ID).await,
        resolution_programdata: required_observed(context, fixture.resolution_programdata).await,
        source_material,
        source_material_staging,
        capability_manifest: required_observed(context, fixture.capability_manifest.raw).await,
        capability_manifest_staging: vacant_observed(fixture.capability_manifest.staging),
        source_state: required_observed(context, fixture.source).await,
        funding_ledger: required_observed(context, fixture.funding).await,
        certificate: required_observed(context, fixture.certificate).await,
        closure_destination: required_observed(context, fixture.closure).await,
        beneficiary: required_observed(context, fixture.rent_credit).await,
        clock_sysvar: required_observed(context, sysvar::clock::ID).await,
        rent_sysvar: required_observed(context, sysvar::rent::ID).await,
        system_program: required_observed(context, system_program::ID).await,
        recovery_policy,
        recovery_policy_staging,
    }
}

async fn retirement_snapshot(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
) -> RetirementSnapshot {
    RetirementSnapshot {
        market: observed(context, fixture.market).await,
        source: observed(context, fixture.source).await,
        funding: observed(context, fixture.funding).await,
        certificate: observed(context, fixture.certificate).await,
        closure: observed(context, fixture.closure).await,
        rent_credit: observed(context, fixture.rent_credit).await,
    }
}

fn assert_exhaustive_closure_receipt(
    receipt: SourceClosureReceiptV3,
    close: &ResolutionDirectCloseFundReportV1,
) {
    let facts = close.expected_retirement_facts;
    assert_eq!(
        receipt,
        SourceClosureReceiptV3 {
            market: facts.market,
            source_state: facts.source_state,
            source_material: facts.source_material,
            capability_manifest: facts.capability_manifest,
            terminal_certificate: facts.terminal_certificate,
            receipt_account: facts.resolution_closure_receipt,
            beneficiary: facts.beneficiary,
            source_state_digest: facts.source_state_digest,
            terminal_certificate_digest: facts.terminal_certificate_digest,
            funding_set_digest: facts.funding_set_digest,
            generation: facts.generation,
            terminal_sequence: facts.terminal_sequence,
            selector: facts.selector,
            source_refund_lamports: facts.source_refund_lamports,
            ledger_remaining_native_principal: facts.ledger_remaining_native_principal,
            ledger_rent_lamports: facts.ledger_rent_lamports,
            ledger_lamport_surplus: facts.ledger_lamport_surplus,
            refund_lamports: facts.refund_lamports,
            closed_at: facts.closed_at,
        },
        "the persisted V3 closure receipt must reproduce every chain-derived retirement fact"
    );
    assert_eq!(receipt.source_refund_lamports, facts.source_refund_lamports);
    assert_eq!(
        receipt.ledger_remaining_native_principal,
        facts.ledger_remaining_native_principal
    );
    assert_eq!(receipt.ledger_rent_lamports, facts.ledger_rent_lamports);
    assert_eq!(receipt.ledger_lamport_surplus, facts.ledger_lamport_surplus);
    assert_eq!(receipt.refund_lamports, facts.refund_lamports);
    assert_eq!(
        receipt
            .source_refund_lamports
            .checked_add(receipt.ledger_remaining_native_principal)
            .and_then(|value| value.checked_add(receipt.ledger_rent_lamports))
            .and_then(|value| value.checked_add(receipt.ledger_lamport_surplus)),
        Some(receipt.refund_lamports),
        "V3 exhaustively classifies every discharged Source and subset-ledger lamport"
    );
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
        funding: observed(context, fixture.funding).await,
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
        funding: observed(context, fixture.funding).await,
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
    // The three rows the operator derived from the ledger, named as the three
    // rows this Market's manifest actually gives Resolution rather than as the
    // literal `[0, 1, 2]`. Since the manifest carries the Direct capability at
    // the row its own kind_id sorts to, a literal here was a second author of
    // the layout and went stale the moment a fourth row existed.
    assert_eq!(
        create.funding_entry_indices,
        fixture.resolution_entry_indices
    );
    let pending_ledger_before = observed(&mut context, fixture.funding)
        .await
        .expect("pre-Market Resolution-owned Pending ledger");
    assert_eq!(pending_ledger_before.owner, RESOLUTION_PROGRAM_ID);

    let mut create_instructions = Vec::with_capacity(2);
    create_instructions.push(transfer(
        &payer,
        &fixture.source,
        create.source_top_up_lamports,
    ));
    create_instructions.push(create.instruction);
    let (created, create_extent) = submit_recorded_v0(
        &mut context,
        &create_instructions,
        &[],
        "core-v3: CreateFund creates the canonical Source funding",
    )
    .await;
    created.expect("prepay and create canonical Source funding");
    // 1,275 legacy is 43 over, which is one address: this route was over the
    // wall by a single account and nothing in a ProgramTest would ever have
    // said so. The prepay transfer rides in the same transaction, so the
    // measurement is of what is actually submitted, not of the CreateFund
    // instruction in isolation.
    assert_eq!(
        create_extent,
        PacketExtentV1 {
            legacy_bytes: 1_275,
            v0_bytes: 877,
            static_keys: 3,
            loaded_addresses: 14,
        }
    );
    assert!(create_extent.legacy_bytes > PACKET_DATA_BYTES);
    assert!(create_extent.v0_bytes <= PACKET_DATA_BYTES);

    let source = SourceResolutionStateV2::decode(
        &observed(&mut context, fixture.source)
            .await
            .expect("created Source")
            .data,
    )
    .expect("Source state");
    assert_eq!(source.phase(), SourceResolutionPhaseV1::Primary);
    assert_eq!(
        observed(&mut context, fixture.funding)
            .await
            .expect("unchanged pre-Market Resolution ledger"),
        pending_ledger_before,
        "CreateFund creates only Source state and leaves the existing Pending ledger bytes and lamports unchanged"
    );
    assert_funding_ledger_status(&mut context, &fixture, FundingLedgerStatusV2::Pending).await;

    let activation = build_resolution_activate_fund_v1(&ResolutionActivateFundSnapshotV1 {
        pending: verify_snapshot(&mut context, &fixture).await,
        system_program: required_observed(&mut context, system_program::ID).await,
    })
    .expect("chain-derived direct activation");
    let beneficiary_before = observed(&mut context, fixture.rent_credit)
        .await
        .expect("RentCredit")
        .lamports;
    let mut activation_instructions = Vec::with_capacity(2);
    if activation.receipt_top_up_lamports != 0 {
        activation_instructions.push(transfer(
            &payer,
            &fixture.activation_receipt,
            activation.receipt_top_up_lamports,
        ));
    }
    activation_instructions.push(activation.instruction);
    submit_recorded(
        &mut context,
        &activation_instructions,
        &[],
        "core-v3: activate the exact three-row Resolution funding ledger",
    )
    .await
    .expect("activate exact three-row Resolution funding ledger");
    assert_funding_ledger_status(&mut context, &fixture, FundingLedgerStatusV2::Active).await;
    assert_eq!(
        observed(&mut context, fixture.rent_credit)
            .await
            .expect("RentCredit")
            .lamports,
        beneficiary_before + activation.expected_beneficiary_credit_lamports
    );

    let after_activation = retirement_snapshot(&mut context, &fixture).await;
    let activation_receipt_after_activation = observed(&mut context, fixture.activation_receipt)
        .await
        .expect("durable activation receipt");
    let activation_replay = build_resolution_activate_fund_v1(&ResolutionActivateFundSnapshotV1 {
        pending: verify_snapshot(&mut context, &fixture).await,
        system_program: required_observed(&mut context, system_program::ID).await,
    })
    .expect("receipt-authenticated activation replay");
    assert_eq!(activation_replay.receipt_top_up_lamports, 0);
    assert_eq!(activation_replay.expected_beneficiary_credit_lamports, 0);
    assert_eq!(activation_replay.request_digest, activation.request_digest);
    submit_recorded(
        &mut context,
        &[activation_replay.instruction],
        &[],
        "core-v3: the completed activation replays without mutation",
    )
    .await
    .expect("completed activation replays without mutation");
    assert_eq!(
        retirement_snapshot(&mut context, &fixture).await,
        after_activation,
        "activation replay preserves every mutable lifecycle account"
    );
    assert_eq!(
        observed(&mut context, fixture.activation_receipt).await,
        Some(activation_receipt_after_activation),
        "activation replay preserves the immutable receipt"
    );

    let verify =
        build_resolution_verify_fund_ready_v3(&verify_snapshot(&mut context, &fixture).await)
            .expect("chain-derived no-CPI VerifyFundReady acceptance");
    validate_resolution_verify_fund_ready_report_v3(&verify).expect("exact VerifyFundReady report");
    let before_privilege_refusal = retirement_snapshot(&mut context, &fixture).await;
    let mut writable_beneficiary = verify.instruction.clone();
    writable_beneficiary
        .accounts
        .get_mut(14)
        .expect("beneficiary account")
        .is_writable = true;
    assert!(
        submit(&mut context, &[writable_beneficiary]).await.is_err(),
        "surplus writable beneficiary privilege must refuse"
    );
    assert_eq!(
        retirement_snapshot(&mut context, &fixture).await,
        before_privilege_refusal,
        "privilege refusal preserves Source, Funds, Core, and RentCredit"
    );

    submit(&mut context, &[verify.instruction])
        .await
        .expect("accept the durable activation receipt into Core readiness");
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
    assert_funding_ledger_status(&mut context, &fixture, FundingLedgerStatusV2::Active).await;

    let after_accept = retirement_snapshot(&mut context, &fixture).await;
    let activation_receipt_after_accept = observed(&mut context, fixture.activation_receipt)
        .await
        .expect("accepted activation receipt");
    let accept_replay =
        build_resolution_verify_fund_ready_v3(&verify_snapshot(&mut context, &fixture).await)
            .expect("Ready replay authenticates the Prepaid predecessor receipt");
    submit(&mut context, &[accept_replay.instruction])
        .await
        .expect("completed Core Accept replays without mutation");
    assert_eq!(
        retirement_snapshot(&mut context, &fixture).await,
        after_accept,
        "Core Accept replay preserves every mutable lifecycle account"
    );
    assert_eq!(
        observed(&mut context, fixture.activation_receipt).await,
        Some(activation_receipt_after_accept),
        "Core Accept replay preserves the immutable receipt"
    );

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
            &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
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
        "SourceMaterial substitution rolls back Market, Source, lifecycle, certificate, the subset ledger, Custody, update, and RentCredit"
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

    // FIRST-VALID, HALF ONE. A SECOND update, posted through the same real
    // Receiver ELF against the same VAA while this Source is still `Primary`.
    // Both submissions are unimpeachably valid provider evidence at the moment
    // they are made: same feed, same window, same release, same authenticated
    // record graph, each with its own program-owned `Submitted` lifecycle. The
    // only thing that will be different when the second one is consumed is that
    // the first one already won.
    let second_update = Keypair::new();
    let second_post_update_body = post_update_body.clone();
    let second_submit = build_provider_submit_v3(
        &provider_submit_snapshot(&mut context, &fixture, encoded_vaa).await,
        provider_submit_deployment(&fixture),
        &ProviderSubmitIntentV3 {
            submitter: payer,
            refund_recipient: fixture.rent_credit,
            update_account: second_update.pubkey(),
            reclaim_after_unix_seconds: TERMINAL_TIME + 20,
            post_update_body: second_post_update_body.clone(),
        },
    )
    .expect("chain-derived second real-provider submission");
    advance_provider_refusal_slot(&mut context).await;
    pyth_provider::submit(
        &mut context,
        &[
            transfer(&payer, &second_submit.lifecycle, provider_lifecycle_rent),
            second_submit.instruction,
        ],
        &[&second_update],
    )
    .await
    .expect("a Primary Source accepts a second real provider submission");
    assert_eq!(
        observed(&mut context, second_update.pubkey())
            .await
            .expect("second Receiver update")
            .owner,
        fixture.provider.receiver
    );
    assert_eq!(
        ProviderUpdateLifecycleV3::decode(
            &observed(&mut context, second_submit.lifecycle)
                .await
                .expect("second provider lifecycle")
                .data,
        )
        .expect("second provider lifecycle")
        .status,
        ProviderUpdateStatusV3::Submitted,
        "the second submission is live evidence awaiting consumption, not a rejected one"
    );

    // ABANDONMENT IS NOT AVAILABLE AGAINST A LIVE MARKET. Both submissions are
    // `Submitted` and this Source is still `Primary`, so either could still
    // become the answer. If abandonment needed only the submitter's deadline, a
    // stranger could delete a market's answer for a transaction fee -- the
    // `RecordStillConsumable` failure, one transport over. The builder refuses
    // to derive it at all, before a transaction exists.
    let live_market_deployment = ProviderReclaimDeploymentV3 {
        resolver: payer,
        registry_programdata: fixture.registry_programdata,
        resolution_program: RESOLUTION_PROGRAM_ID,
        resolution_programdata: fixture.resolution_programdata,
    };
    assert_eq!(
        SourceResolutionStateV2::decode(
            &observed(&mut context, fixture.source)
                .await
                .expect("live Source")
                .data,
        )
        .expect("live Source state")
        .phase(),
        SourceResolutionPhaseV1::Primary
    );
    assert_eq!(
        build_provider_abandon_v3(
            &required_observed(&mut context, second_submit.lifecycle).await,
            &required_observed(&mut context, fixture.source).await,
            &required_observed(&mut context, fixture.pyth_release.raw).await,
            live_market_deployment,
        ),
        Err(ProviderTransportOperatorErrorV3::State),
        "a submission a Primary Source could still consume is not abandoned"
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
    // FIRST-VALID, HALF TWO -- built here, submitted after the winner lands.
    // Both executions are derived from the SAME `Primary` Source observation,
    // so neither is privileged by construction; the host builder refuses to
    // derive against a resolved Source at all, which is why the order matters.
    let second_execute = build_provider_execute_v3(
        &provider_execute_snapshot_for(
            &mut context,
            &fixture,
            second_submit.lifecycle,
            second_update.pubkey(),
        )
        .await,
        provider_execute_deployment(&fixture),
        &ProviderExecuteIntentV3 {
            resolver: resolver.pubkey(),
            terminal_sequence: TERMINAL_SEQUENCE,
            post_update_body: second_post_update_body,
        },
    )
    .expect("chain-derived second provider execution against the same Primary Source");
    assert!(
        build_resolution_admit_terminal_v3(&admit_snapshot(&mut context, &fixture).await).is_err(),
        "before ExecuteProvider the Market is Open but Source is Primary and the standalone terminal-admission family must refuse"
    );

    // IMMUTABLE WINDOW. The market sold a question about a closed period, and
    // the only thing that fixes that period is a digest inside this market's
    // own `SourceMaterialV3` -- itself pinned by `identity.resolution_policy`
    // in the Core Market PDA, whose address is derived from that identity. So
    // there is no instruction anywhere that MOVES a window; the only attack
    // available is to publish a wider one and present it instead, and record
    // publication is permissionless, so that attack is always available.
    //
    // Both halves of the record pair are substituted, each derived correctly
    // from the widened record's own digest, so the frame is internally
    // consistent and only the join to the material can refuse it. That is the
    // point: the refusal must come from the market's own authority, not from a
    // mismatched staging cursor.
    let before_widened_window =
        provider_rollback_snapshot(&mut context, &fixture, provider_submit.lifecycle).await;
    let mut widened_window_frame = provider_execute.instruction.clone();
    widened_window_frame
        .accounts
        .get_mut(25)
        .expect("WindowSpec raw record")
        .pubkey = fixture.widened_window.raw;
    widened_window_frame
        .accounts
        .get_mut(26)
        .expect("WindowSpec staging cursor")
        .pubkey = fixture.widened_window.staging;
    advance_provider_refusal_slot(&mut context).await;
    let widened = pyth_provider::submit(&mut context, &[widened_window_frame], &[&resolver])
        .await
        .expect_err("a wider window must not be substitutable for the one the market sold");
    assert!(
        matches!(
            widened,
            BanksClientError::TransactionError(TransactionError::InstructionError(
                0,
                InstructionError::Custom(code)
            )) if code == ResolutionError::FinalizedRecord as u32
        ),
        "a substituted WindowSpec must refuse as Resolution FinalizedRecord, got {widened:?}"
    );
    assert_eq!(
        provider_rollback_snapshot(&mut context, &fixture, provider_submit.lifecycle).await,
        before_widened_window,
        "the widened-window refusal rolls back Market, Source, lifecycle, certificate, ledger and RentCredit"
    );

    let before_missing_caller_signature =
        provider_rollback_snapshot(&mut context, &fixture, provider_submit.lifecycle).await;
    let direct_resolution_without_core_caller = Instruction {
        program_id: RESOLUTION_PROGRAM_ID,
        accounts: provider_execute.instruction.accounts.clone(),
        data: provider_execute
            .instruction
            .data
            .get(REQUEST_BYTES..)
            .expect("provider body after Core request")
            .to_vec(),
    };
    assert!(
        pyth_provider::submit(
            &mut context,
            &[direct_resolution_without_core_caller],
            &[&resolver],
        )
        .await
        .is_err(),
        "a top-level Resolution invocation cannot substitute for Core's program-signed caller PDA"
    );
    assert_eq!(
        provider_rollback_snapshot(&mut context, &fixture, provider_submit.lifecycle).await,
        before_missing_caller_signature,
        "the missing program-signed caller PDA rolls back every provider and Market write"
    );

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
        "the late substitution rolls back Market, Source, lifecycle, certificate, the subset ledger, Custody, update, and RentCredit"
    );

    pyth_provider::submit(&mut context, &[provider_execute.instruction], &[&resolver])
        .await
        .expect(
            "Resolution consumes the authenticated provider result and persists its certificate",
        );
    let post_provider_market = CoreState::decode(
        &observed(&mut context, fixture.market)
            .await
            .expect("post-provider Market")
            .data,
    )
    .expect("post-provider Core state");
    assert_eq!(post_provider_market.phase, Phase::Open);
    assert!(post_provider_market.terminal_receipt.is_none());

    // FIRST-VALID, HALF TWO. The Market is still `Open + Consumed` and the
    // second lifecycle is still `Submitted`, so every frame, privilege,
    // release, deployment, record, product-domain, provider and freshness
    // check the winning execution passed still passes for this one. Exactly
    // one fact changed: the Source is `Resolved`, and
    // `SourceResolutionStateV2::resolve_primary_from_authenticated_domain`
    // refuses anywhere but `Primary`.
    //
    // Named by discriminant. A bare `is_err()` here would also pass on an
    // `AccountFrame`, `ProviderObservation` or `OutputState` refusal, and each
    // of those would prove the OPPOSITE of first-valid -- that the losing
    // submission was never admissible evidence in the first place, so nothing
    // was ever ordered. `Transition` is the only code that means "this was
    // good, and it lost".
    let before_first_valid =
        provider_rollback_snapshot(&mut context, &fixture, provider_submit.lifecycle).await;
    advance_provider_refusal_slot(&mut context).await;
    let later = pyth_provider::submit(&mut context, &[second_execute.instruction], &[&resolver])
        .await
        .expect_err("a second valid provider result must not overwrite the first");
    assert!(
        matches!(
            later,
            BanksClientError::TransactionError(TransactionError::InstructionError(
                0,
                InstructionError::Custom(code)
            )) if code == ResolutionError::Transition as u32
        ),
        "the later of two valid submissions must refuse as Resolution Transition, got {later:?}"
    );
    assert_eq!(
        provider_rollback_snapshot(&mut context, &fixture, provider_submit.lifecycle).await,
        before_first_valid,
        "the first-valid refusal leaves the winning Source, certificate, ledger and lifecycle byte-identical"
    );
    assert_eq!(
        ProviderUpdateLifecycleV3::decode(
            &observed(&mut context, second_submit.lifecycle)
                .await
                .expect("losing provider lifecycle")
                .data,
        )
        .expect("losing provider lifecycle")
        .status,
        ProviderUpdateStatusV3::Submitted,
        "the losing submission is not consumed by its own refusal; its reclaim route stays open"
    );

    let admit = build_resolution_admit_terminal_v3(&admit_snapshot(&mut context, &fixture).await)
        .expect("chain-derived no-CPI terminal certificate Accept");
    submit(&mut context, &[admit.instruction])
        .await
        .expect("Core accepts the durable Resolution certificate and commits terminal state");
    let terminal_market = CoreState::decode(
        &observed(&mut context, fixture.market)
            .await
            .expect("terminal Market")
            .data,
    )
    .expect("terminal Core state");
    assert_eq!(terminal_market.phase, Phase::Terminal);
    assert!(terminal_market.terminal_receipt.is_some());
    let after_terminal_accept = retirement_snapshot(&mut context, &fixture).await;
    let admit_replay =
        build_resolution_admit_terminal_v3(&admit_snapshot(&mut context, &fixture).await)
            .expect("terminal certificate Accept replay");
    submit(&mut context, &[admit_replay.instruction])
        .await
        .expect("terminal certificate Accept replays without mutation");
    assert_eq!(
        retirement_snapshot(&mut context, &fixture).await,
        after_terminal_accept,
        "terminal Accept replay preserves every mutable lifecycle account"
    );
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
    assert!(
        build_resolution_admit_terminal_v3(&admit_snapshot(&mut context, &fixture).await).is_ok(),
        "the exact terminal certificate Accept remains a reconstructable no-op after Core is Terminal"
    );
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
        .minimum_balance(SOURCE_CLOSURE_RECEIPT_BYTES_V3);
    submit(
        &mut context,
        &[transfer(&payer, &fixture.closure, closure_rent)],
    )
    .await
    .expect("prepay the same-lineage closure receipt");
    let close =
        build_resolution_direct_close_fund_v1(&close_snapshot(&mut context, &fixture).await)
            .expect("chain-derived same-lineage CloseFund");
    let (closed, close_extent) = submit_recorded_v0(
        &mut context,
        &[close.instruction.clone()],
        &[],
        "core-v3: CloseFund closes all three rows of the subset ledger",
    )
    .await;
    closed.expect("close all three rows in the provider-resolution subset ledger");
    // Five bytes over. A route this close to the wall is not "fine": one more
    // account, or the twelve-byte priority-fee instruction the house builder
    // pushes unconditionally, and it is unsubmittable with no code change to
    // blame.
    assert_eq!(
        close_extent,
        PacketExtentV1 {
            legacy_bytes: 1_237,
            v0_bytes: 715,
            static_keys: 2,
            loaded_addresses: 18,
        }
    );
    assert!(close_extent.legacy_bytes > PACKET_DATA_BYTES);
    assert!(close_extent.v0_bytes <= PACKET_DATA_BYTES);

    assert!(observed(&mut context, fixture.source).await.is_none());
    assert!(observed(&mut context, fixture.funding).await.is_none());
    let closure = SourceClosureReceiptV3::decode(
        &observed(&mut context, fixture.closure)
            .await
            .expect("same-lineage Source closure receipt")
            .data,
    )
    .expect("Source closure receipt");
    assert_exhaustive_closure_receipt(closure, &close);

    // RECLAIM. The Source fund is closed, but two provider-transport accounts
    // are still alive and still holding rent: each submission's program-owned
    // lifecycle PDA and the Receiver-owned `PriceUpdateV2` it posted. This is
    // where the route home for those lamports is measured -- once for the
    // submission that won, and once for the equally valid one that lost.
    let mut reclaim_clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("ProgramTest Clock");
    reclaim_clock.unix_timestamp = TERMINAL_TIME + 21;
    context.set_sysvar(&reclaim_clock);
    let reclaim_deployment = ProviderReclaimDeploymentV3 {
        resolver: resolver.pubkey(),
        registry_programdata: fixture.registry_programdata,
        resolution_program: RESOLUTION_PROGRAM_ID,
        resolution_programdata: fixture.resolution_programdata,
    };
    let winner_lifecycle_lamports = observed(&mut context, provider_submit.lifecycle)
        .await
        .expect("consumed winner lifecycle")
        .lamports;
    let winner_update_lamports = observed(&mut context, fixture.update.pubkey())
        .await
        .expect("winner posted update")
        .lamports;
    let rent_credit_before_reclaim = observed(&mut context, fixture.rent_credit)
        .await
        .expect("RentCredit")
        .lamports;
    let winner_reclaim = build_provider_reclaim_v3(
        &required_observed(&mut context, provider_submit.lifecycle).await,
        &required_observed(&mut context, fixture.pyth_release.raw).await,
        reclaim_deployment,
    )
    .expect("chain-derived permissionless reclaim of the consumed submission");
    pyth_provider::submit(&mut context, &[winner_reclaim.instruction], &[&resolver])
        .await
        .expect("a stranger reclaims the winning submission's provider rent");
    assert!(
        observed(&mut context, provider_submit.lifecycle)
            .await
            .is_none()
    );
    assert!(
        observed(&mut context, fixture.update.pubkey())
            .await
            .is_none()
    );
    assert_eq!(
        observed(&mut context, fixture.rent_credit)
            .await
            .expect("RentCredit")
            .lamports,
        rent_credit_before_reclaim + winner_lifecycle_lamports + winner_update_lamports,
        "every lamport the winning submission held returns to the persisted refund recipient"
    );

    // AND THE LOSER GOES HOME TOO. The consumed route cannot carry it:
    // `authenticate_reclaim_state`
    // (programs/dclutch-resolution-proof-sbf/src/provider_transport_v3.rs)
    // requires `Consumed` and a certificate carrying this lifecycle's own
    // `provider_evidence`, and a submission that lost the first-valid race has
    // neither. That gate did not move. `AbandonSubmission` is the other half of
    // the partition, and it proves the opposite fact: that consumption can
    // never happen.
    let stranded_lifecycle = observed(&mut context, second_submit.lifecycle)
        .await
        .expect("the losing submission's lifecycle survived the Source close");
    let stranded_update = observed(&mut context, second_update.pubkey())
        .await
        .expect("the losing submission's posted update survived the Source close");
    assert_eq!(stranded_lifecycle.owner, RESOLUTION_PROGRAM_ID);
    assert_eq!(stranded_update.owner, fixture.provider.receiver);
    assert_eq!(
        build_provider_reclaim_v3(
            &required_observed(&mut context, second_submit.lifecycle).await,
            &required_observed(&mut context, fixture.pyth_release.raw).await,
            reclaim_deployment,
        ),
        Err(ProviderTransportOperatorErrorV3::Lifecycle),
        "the consumed route still refuses a never-consumed lifecycle"
    );

    // The Source account is gone -- `CloseFund` discharged it above -- so the
    // frame presents its address as the vacant System account the runtime
    // materializes, which is the second of the two shapes that prove this
    // submission can never be consumed.
    let abandoned = build_provider_abandon_v3(
        &required_observed(&mut context, second_submit.lifecycle).await,
        &vacant_observed(fixture.source),
        &required_observed(&mut context, fixture.pyth_release.raw).await,
        reclaim_deployment,
    )
    .expect("chain-derived reclaim of the abandoned submission");

    // Early by one second. The submitter's own `reclaim_after_unix_seconds` is
    // the bound, and it is checked separately from the Source so that "not
    // yet" and "still live" are the same refusal only when they are the same
    // fact: both mean this submission is not abandoned.
    let mut early_clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("ProgramTest Clock");
    early_clock.unix_timestamp = TERMINAL_TIME + 19;
    context.set_sysvar(&early_clock);
    let before_early_abandon = (
        observed(&mut context, second_submit.lifecycle).await,
        observed(&mut context, second_update.pubkey()).await,
        observed(&mut context, fixture.rent_credit).await,
    );
    let early = submit_recorded(
        &mut context,
        &[abandoned.instruction.clone()],
        &[&resolver],
        "core-v3: abandon refuses before the submitter's own deadline",
    )
    .await
    .expect_err("abandonment before the submitter's own deadline must refuse");
    assert!(
        matches!(
            early,
            BanksClientError::TransactionError(TransactionError::InstructionError(
                0,
                InstructionError::Custom(code)
            )) if code == ResolutionError::SubmissionStillConsumable as u32
        ),
        "an early abandon must refuse as Resolution SubmissionStillConsumable, got {early:?}"
    );
    assert_eq!(
        (
            observed(&mut context, second_submit.lifecycle).await,
            observed(&mut context, second_update.pubkey()).await,
            observed(&mut context, fixture.rent_credit).await,
        ),
        before_early_abandon,
        "the early refusal leaves the abandoned submission and the beneficiary untouched"
    );

    // The stranger's reclaim carries the SAME instruction and the SAME signers
    // as the early refusal above, so the two are one transaction unless the
    // blockhash moves: the status cache is keyed on the signature, and an
    // identical message at an identical blockhash is `AlreadyProcessed`. This
    // campaign used to get that separation by accident, from slot progression
    // it did not ask for and could not point at. Ask for it.
    let reclaim_slot = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("ProgramTest Clock")
        .slot;
    context
        .warp_to_slot(reclaim_slot + 1)
        .expect("the reclaim is a later transaction than the refusal it repeats");
    let mut abandon_clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("ProgramTest Clock");
    abandon_clock.unix_timestamp = TERMINAL_TIME + 21;
    context.set_sysvar(&abandon_clock);
    let rent_credit_before_abandon = observed(&mut context, fixture.rent_credit)
        .await
        .expect("RentCredit")
        .lamports;
    submit_recorded(
        &mut context,
        &[abandoned.instruction],
        &[&resolver],
        "core-v3: a stranger reclaims the abandoned submission",
    )
    .await
    .expect("a stranger reclaims the losing submission's provider rent");
    assert!(
        observed(&mut context, second_submit.lifecycle)
            .await
            .is_none()
    );
    assert!(
        observed(&mut context, second_update.pubkey())
            .await
            .is_none()
    );
    assert_eq!(
        observed(&mut context, fixture.rent_credit)
            .await
            .expect("RentCredit")
            .lamports,
        rent_credit_before_abandon + stranded_lifecycle.lamports + stranded_update.lamports,
        "every lamport the LOSING submission held returns to the same persisted recipient"
    );
}

#[tokio::test]
async fn a_market_walked_to_failure_ends_terminal_on_its_pre_disclosed_terms() {
    // The other half of the funded failure walk, and the half nothing in this
    // tree had executed.
    //
    // `CommitDeadlineFailure` pays the walker and writes a `ResolutionFailure`
    // certificate, but it leaves the Market READONLY in its own frame. The
    // Market becoming terminal is this separate no-CPI Core route. Core's
    // failure arm is first-class rather than a fallback --
    // `build_resolution_admit_terminal_v3` derives the receipt kind, the
    // certificate kind and the PDA kind tag from the Source phase, and
    // `admit_terminal` has no branch on kind at all -- but until this test
    // every executed `AdmitTerminal` in the tree admitted a certificate a
    // provider stood behind, so the failure arm had never once been driven
    // against a real Core ELF.
    //
    // This is what a holder's exit at failure terms waits on: `terminal_winner`
    // has to be the Product's own failure region before any Claims redemption
    // can select it.
    let mut fixture = fixture(MarketPrestateV1::TerminalFailure);
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
    clock.unix_timestamp = FAILURE_TIME + 1;
    context.set_sysvar(&clock);

    let before = CoreState::decode(
        &observed(&mut context, fixture.market)
            .await
            .expect("Market")
            .data,
    )
    .expect("Core state");
    assert_eq!(before.phase, Phase::Open);
    assert_eq!(before.terminal_receipt, None);

    let admit = build_resolution_admit_terminal_v3(&admit_snapshot(&mut context, &fixture).await)
        .expect("chain-derived AdmitTerminal from a FailureCommitted Source");
    submit(&mut context, &[admit.instruction])
        .await
        .expect("a walked market's failure certificate terminalizes its Market");

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
        Some(fixture.certificate.to_bytes()),
        "the Market commits to the failure certificate's own address, which is a different \
         address from the one a provider-resolved terminal would have written"
    );

    let source = SourceResolutionStateV2::decode(
        &observed(&mut context, fixture.source)
            .await
            .expect("Source")
            .data,
    )
    .expect("Source state");
    assert_eq!(source.phase(), SourceResolutionPhaseV1::FailureCommitted);
    let terminal = source
        .terminal_projection()
        .expect("a FailureCommitted Source projects its terminal");
    assert_eq!(terminal.route(), SourceResolutionRouteV1::Failure);
    assert_eq!(
        admitted.terminal_winner,
        terminal.selector(),
        "the winner Core commits is the selector the Source's own failure decision carried, not \
         a value this route chose"
    );

    // Failure is a complete terminal route, not a certificate shape that
    // strands the Resolution fund.  Drive the same permissionless retirement
    // and DCLRFCQ1 close used by the provider-success arm.  This is deliberately
    // after Core's independent no-CPI Accept: closing Source before Core has
    // committed the certificate would destroy the fact Claims authenticates.
    submit(&mut context, &[begin_retiring_instruction(&fixture)])
        .await
        .expect("a failure-terminal Market begins authenticated retirement");
    let closure_rent = context
        .banks_client
        .get_rent()
        .await
        .expect("chain Rent")
        .minimum_balance(SOURCE_CLOSURE_RECEIPT_BYTES_V3);
    let payer = context.payer.pubkey();
    submit(
        &mut context,
        &[transfer(&payer, &fixture.closure, closure_rent)],
    )
    .await
    .expect("prepay the failure route's canonical closure receipt");

    let mut wrong_absent_policy_frame = close_snapshot(&mut context, &fixture).await;
    wrong_absent_policy_frame.recovery_policy =
        required_observed(&mut context, fixture.recovery_policy.raw).await;
    wrong_absent_policy_frame.recovery_policy_staging =
        vacant_observed(fixture.recovery_policy.staging);
    assert_eq!(
        build_resolution_direct_close_fund_v1(&wrong_absent_policy_frame)
            .expect_err("an absent optional policy cannot be replaced by an unrelated record"),
        ResolutionCoreOperatorErrorV3::Record
    );
    let close =
        build_resolution_direct_close_fund_v1(&close_snapshot(&mut context, &fixture).await)
            .expect("chain-derived failure-route CloseFund");

    let before_readonly_beneficiary = retirement_snapshot(&mut context, &fixture).await;
    let mut readonly_beneficiary = close.instruction.clone();
    readonly_beneficiary
        .accounts
        .get_mut(15)
        .expect("DCLRFCQ1 beneficiary coordinate")
        .is_writable = false;
    let error = submit(&mut context, &[readonly_beneficiary])
        .await
        .expect_err("a readonly refund beneficiary must refuse before close");
    assert!(
        matches!(
            error,
            BanksClientError::TransactionError(TransactionError::InstructionError(
                0,
                InstructionError::Custom(code)
            )) if code == ResolutionError::AccountFrame as u32
        ),
        "readonly beneficiary must refuse as Resolution AccountFrame, got {error:?}"
    );
    assert_eq!(
        retirement_snapshot(&mut context, &fixture).await,
        before_readonly_beneficiary,
        "the exact AccountFrame refusal rolls back Source, funding, closure prepayment, certificate, and beneficiary"
    );
    submit(&mut context, &[close.instruction.clone()])
        .await
        .expect("the failure route closes every Resolution funding row");
    assert!(observed(&mut context, fixture.source).await.is_none());
    assert!(observed(&mut context, fixture.funding).await.is_none());
    let closure = SourceClosureReceiptV3::decode(
        &observed(&mut context, fixture.closure)
            .await
            .expect("failure-route Source closure receipt")
            .data,
    )
    .expect("failure-route Source closure receipt");
    assert_exhaustive_closure_receipt(closure, &close);
}

/// The fallback life, WALKED rather than seeded, from founding through fund
/// close.
///
/// Its sibling above starts from `MarketPrestateV1::TerminalFailure` — a Source
/// already `FailureCommitted` and a certificate already minted — and proves what
/// a market does with that ending. It proves nothing about how a market GETS
/// there, and until this test the only real-ELF execution of the walk itself was
/// through the relay transport (`relayed_mainnet_state.rs`), against a market
/// whose evidence family is not Pyth.
///
/// So this drives it on the Pyth market, and the point is that no part of the
/// walk is Pyth-specific or relay-specific: the 22-account frame carries no
/// provider account and no relay account. A market founded to be answered by a
/// price feed, whose price feed then says nothing, must still terminate on the
/// terms it published before it opened. That is `MAINNET_STATE_RELAY.md` §4.8's
/// property — a silent provider cannot make a market unresolvable, only drive it
/// to a pre-disclosed outcome along a bounded, prepaid, permissionless path that
/// pays whoever walks it — and every clause of it is measured below.
#[tokio::test]
async fn a_silent_provider_cannot_strand_a_market_and_the_walker_is_paid() {
    let mut fixture = fixture(MarketPrestateV1::WalkableFailure);
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
    clock.slot = clock.slot.max(1);
    clock.unix_timestamp = TERMINAL_TIME;
    context.set_sysvar(&clock);
    let payer = context.payer.pubkey();

    // Found the Resolution fund exactly as the provider arc does. Nothing about
    // this prestate anticipates failure: it is an ordinary open market with an
    // ordinary prepaid ledger, and the only thing that will differ is that
    // nobody submits.
    let create = build_resolution_create_fund_v3(&create_snapshot(&mut context, &fixture).await)
        .expect("chain-derived CreateFund against an Open Market");
    submit(
        &mut context,
        &[
            transfer(&payer, &fixture.source, create.source_top_up_lamports),
            create.instruction,
        ],
    )
    .await
    .expect("an Open Market creates its Source against the pre-Market ledger");
    let activation = build_resolution_activate_fund_v1(&ResolutionActivateFundSnapshotV1 {
        pending: verify_snapshot(&mut context, &fixture).await,
        system_program: required_observed(&mut context, system_program::ID).await,
    })
    .expect("chain-derived activation");
    let mut activation_instructions = Vec::with_capacity(2);
    if activation.receipt_top_up_lamports != 0 {
        activation_instructions.push(transfer(
            &payer,
            &fixture.activation_receipt,
            activation.receipt_top_up_lamports,
        ));
    }
    activation_instructions.push(activation.instruction);
    submit(&mut context, &activation_instructions)
        .await
        .expect("activate the three-row Resolution funding ledger");
    assert_funding_ledger_status(&mut context, &fixture, FundingLedgerStatusV2::Active).await;
    let verify =
        build_resolution_verify_fund_ready_v3(&verify_snapshot(&mut context, &fixture).await)
            .expect("chain-derived VerifyFundReady");
    submit(&mut context, &[verify.instruction])
        .await
        .expect("VerifyFundReady rechecks the Active ledger");
    assert_eq!(
        SourceResolutionStateV2::decode(
            &observed(&mut context, fixture.source)
                .await
                .expect("created Source")
                .data,
        )
        .expect("Source state")
        .phase(),
        SourceResolutionPhaseV1::Primary,
        "the walk starts from a live market that could still have been answered"
    );

    // The walker is a stranger. It holds no role in this market, no capability,
    // no relationship to the manifest that will pay it, and it did not exist
    // when the market was founded.
    let walker = Keypair::new();
    let walker_rent = context
        .banks_client
        .get_rent()
        .await
        .expect("chain Rent")
        .minimum_balance(0);
    submit(
        &mut context,
        &[
            transfer(&payer, &walker.pubkey(), walker_rent),
            transfer(
                &payer,
                &fixture.certificate,
                Rent::default().minimum_balance(RESOLUTION_CERTIFICATE_BYTES_V2),
            ),
        ],
    )
    .await
    .expect("establish the stranger and prepay the failure certificate seat");

    // HOSTILE — the walk is not available while the market can still be
    // answered honestly. The deadline is the window's own closed upper bound
    // plus its liveness grace, and the comparison is strict, so the last second
    // an honest resolution may land and the first second a walk may run are
    // different seconds. Standing on the deadline itself must refuse.
    let deadline = TERMINAL_TIME + i64::from(WINDOW_MAX_AGE_SECONDS);
    let mut on_deadline = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("ProgramTest Clock");
    on_deadline.unix_timestamp = deadline;
    context.set_sysvar(&on_deadline);
    let before_early_walk = retirement_snapshot(&mut context, &fixture).await;
    let early = pyth_provider::submit(
        &mut context,
        &[deadline_failure_instruction(&fixture, walker.pubkey())],
        &[&walker],
    )
    .await
    .expect_err("a walk standing exactly on the deadline must refuse");
    assert!(
        matches!(
            early,
            BanksClientError::TransactionError(TransactionError::InstructionError(
                0,
                InstructionError::Custom(code)
            )) if code == ResolutionError::Transition as u32
        ),
        "an early walk must refuse as Resolution Transition, got {early:?}"
    );
    assert_eq!(
        retirement_snapshot(&mut context, &fixture).await,
        before_early_walk,
        "the early refusal leaves Source, ledger, certificate seat and RentCredit untouched"
    );

    // THE WALK. One second past the deadline, and one transition:
    // `Primary -> Exhausted -> FailureCommitted` with one debit from the
    // explicit-failure compartment and one `ResolutionFailure` certificate.
    let mut walk_clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("ProgramTest Clock");
    walk_clock.unix_timestamp = deadline + 1;
    context.set_sysvar(&walk_clock);
    let walker_before = observed(&mut context, walker.pubkey())
        .await
        .expect("walker")
        .lamports;
    let funding_before = observed(&mut context, fixture.funding)
        .await
        .expect("Active subset ledger")
        .lamports;
    pyth_provider::submit(
        &mut context,
        &[deadline_failure_instruction(&fixture, walker.pubkey())],
        &[&walker],
    )
    .await
    .expect("a stranger walks the silent market to its pre-disclosed terms");

    let walked = SourceResolutionStateV2::decode(
        &observed(&mut context, fixture.source)
            .await
            .expect("walked Source")
            .data,
    )
    .expect("walked Source state");
    assert_eq!(walked.phase(), SourceResolutionPhaseV1::FailureCommitted);
    let projection = walked
        .terminal_projection()
        .expect("a FailureCommitted Source projects its terminal");
    assert_eq!(projection.route(), SourceResolutionRouteV1::Failure);

    let certificate = ResolutionCertificateV2::decode(
        &observed(&mut context, fixture.certificate)
            .await
            .expect("minted failure certificate")
            .data,
    )
    .expect("failure certificate");
    assert_eq!(
        certificate.kind,
        ResolutionCertificateKindV2::ResolutionFailure
    );
    assert_eq!(certificate.market, fixture.market.to_bytes());
    assert_eq!(certificate.work_paid, BOUNTY);
    assert_eq!(
        certificate.provider_evidence, [0; 32],
        "nothing a provider said stands behind this terminal, and the certificate says so"
    );
    assert_eq!(
        certificate.route, [0; 32],
        "a walked terminal is attributable to no provider release"
    );
    assert_eq!(
        certificate.funding_allocation, fixture.source_material_id,
        "the compartment debited is the one whose manifest entry names this market's own material"
    );

    // The walker is paid exactly the bounty the market quoted before it opened,
    // and it comes out of the escrow, not out of thin air. `work_paid` and the
    // two lamport deltas are three independent statements of one number.
    assert_eq!(
        observed(&mut context, walker.pubkey())
            .await
            .expect("paid walker")
            .lamports,
        walker_before + BOUNTY,
        "the walker is paid the capability's own quoted bounty"
    );
    assert_eq!(
        observed(&mut context, fixture.funding)
            .await
            .expect("debited subset ledger")
            .lamports,
        funding_before - BOUNTY,
        "and every lamport of it comes out of the escrow that promised it"
    );

    // HOSTILE — the walk is paid once, and the ESCROW is what says so.
    //
    // I expected `Transition` here: the Source has left `Primary`, so
    // `exhaust_after_primary_deadline` must refuse. It does — but it never
    // runs. `plan_deadline_failure_v1` debits before it transitions, on purpose
    // ("a walk that cannot be paid for cannot move the market either"), so the
    // second walk dies one step earlier, in `release_in_place`, against a
    // Bounty compartment that is already empty. `Funding`, not `Transition`.
    //
    // That is a stronger fact than the one I assumed, and it is only visible
    // because the discriminant is named: the bound on how many times this walk
    // pays is not the state machine's monotonicity, it is that the market
    // escrowed exactly one bounty and it has been spent. A bare `is_err()`
    // would have shown me the refusal I predicted rather than the one there is.
    let before_replay = retirement_snapshot(&mut context, &fixture).await;
    let replay = pyth_provider::submit(
        &mut context,
        &[deadline_failure_instruction(&fixture, walker.pubkey())],
        &[&walker],
    )
    .await
    .expect_err("the walk cannot be run twice for two bounties");
    assert!(
        matches!(
            replay,
            BanksClientError::TransactionError(TransactionError::InstructionError(
                0,
                InstructionError::Custom(code)
            )) if code == ResolutionError::Funding as u32
        ),
        "a replayed walk must refuse as Resolution Funding against a spent bounty, got {replay:?}"
    );
    assert_eq!(
        retirement_snapshot(&mut context, &fixture).await,
        before_replay,
        "the replay pays no second bounty and moves nothing"
    );

    // Core admits the walked terminal on its own no-CPI route, and the winner
    // it commits is the Product's own failure region -- which is what a
    // holder's exit at failure terms waits on.
    let admit = build_resolution_admit_terminal_v3(&admit_snapshot(&mut context, &fixture).await)
        .expect("chain-derived AdmitTerminal from a walked Source");
    submit(&mut context, &[admit.instruction])
        .await
        .expect("Core accepts the walked market's failure certificate");
    let terminal = CoreState::decode(
        &observed(&mut context, fixture.market)
            .await
            .expect("terminal Market")
            .data,
    )
    .expect("terminal Core state");
    assert_eq!(terminal.phase, Phase::Terminal);
    assert_eq!(
        terminal.terminal_receipt.map(|value| value.to_bytes()),
        Some(fixture.certificate.to_bytes())
    );
    assert_eq!(
        terminal.terminal_winner,
        projection.selector(),
        "the winner Core commits is the selector the walk's own decision carried"
    );

    // And the fund closes. A market nobody answered is not a market whose money
    // is stuck: the same permissionless retirement and DCLRFCQ1 close the
    // provider-resolved arc runs, ending in the same exhaustive classification
    // of every remaining lamport.
    submit(&mut context, &[begin_retiring_instruction(&fixture)])
        .await
        .expect("a walked market begins authenticated retirement");
    let closure_rent = context
        .banks_client
        .get_rent()
        .await
        .expect("chain Rent")
        .minimum_balance(SOURCE_CLOSURE_RECEIPT_BYTES_V3);
    submit(
        &mut context,
        &[transfer(&payer, &fixture.closure, closure_rent)],
    )
    .await
    .expect("prepay the walked route's closure receipt");
    let close =
        build_resolution_direct_close_fund_v1(&close_snapshot(&mut context, &fixture).await)
            .expect("chain-derived walked-route CloseFund");
    submit(&mut context, &[close.instruction.clone()])
        .await
        .expect("the walked route closes every Resolution funding row");
    assert!(observed(&mut context, fixture.source).await.is_none());
    assert!(observed(&mut context, fixture.funding).await.is_none());
    let closure = SourceClosureReceiptV3::decode(
        &observed(&mut context, fixture.closure)
            .await
            .expect("walked-route Source closure receipt")
            .data,
    )
    .expect("walked-route closure receipt");
    assert_exhaustive_closure_receipt(closure, &close);
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

    let closure_rent = Rent::default().minimum_balance(SOURCE_CLOSURE_RECEIPT_BYTES_V3);
    let payer = context.payer.pubkey();
    submit(
        &mut context,
        &[transfer(&payer, &fixture.closure, closure_rent)],
    )
    .await
    .expect("prepay canonical closure receipt");
    let close =
        build_resolution_direct_close_fund_v1(&close_snapshot(&mut context, &fixture).await)
            .expect("chain-derived CloseFund");

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
        "SVM rollback restores Core, Source, the subset ledger, closure prepayment, certificate, and RentCredit"
    );

    submit(&mut context, &[close.instruction.clone()])
        .await
        .expect("Core -> Resolution physical close");
    assert!(observed(&mut context, fixture.source).await.is_none());
    assert!(observed(&mut context, fixture.funding).await.is_none());
    let closure = observed(&mut context, fixture.closure)
        .await
        .expect("Source closure receipt");
    assert_eq!(closure.owner, RESOLUTION_PROGRAM_ID);
    let closure = SourceClosureReceiptV3::decode(&closure.data).expect("closure receipt");
    assert_exhaustive_closure_receipt(closure, &close);
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
/// `MarketPrestateV1::AtomicallyFounded` is that poststate plus the Resolution
/// ledger initialized before Found: `Open + Consumed`, no terminal receipt or
/// Source state, one exact Pending subset ledger, and no certificate. The test
/// walks it to a real terminal certificate through the real Pyth transport.
#[tokio::test]
async fn an_atomically_founded_market_reaches_a_terminal_certificate() {
    let mut fixture = fixture(MarketPrestateV1::AtomicallyFounded);
    let mut context = fixture
        .test
        .take()
        .expect("unstarted ProgramTest")
        .start_with_context()
        .await;
    // The Direct capability root as it stands before any resolution runs.
    // Compared byte-for-byte and lamport-for-lamport at the end of this test:
    // Resolution must resolve a market that CARRIES a capability root without
    // reading, moving or funding the root itself, and until 2026-09-02 that was
    // a reading of the source rather than a measurement, because no campaign
    // that resolved anything had a root in the bank at all.
    let direct_root_before = observed(&mut context, fixture.direct_root)
        .await
        .expect("the Trading-owned Direct capability root");
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
    let pre_market_ledger = observed(&mut context, fixture.funding)
        .await
        .expect("pre-Market Resolution-owned subset ledger");
    assert_eq!(pre_market_ledger.owner, RESOLUTION_PROGRAM_ID);
    assert_funding_ledger_status(&mut context, &fixture, FundingLedgerStatusV2::Pending).await;

    let create = build_resolution_create_fund_v3(&create_snapshot(&mut context, &fixture).await)
        .expect("chain-derived CreateFund against an Open Market");
    validate_resolution_create_fund_report_v3(&create).expect("exact CreateFund report");
    // The three rows the operator derived from the ledger, named as the three
    // rows this Market's manifest actually gives Resolution rather than as the
    // literal `[0, 1, 2]`. Since the manifest carries the Direct capability at
    // the row its own kind_id sorts to, a literal here was a second author of
    // the layout and went stale the moment a fourth row existed.
    assert_eq!(
        create.funding_entry_indices,
        fixture.resolution_entry_indices
    );

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
    let mut substitution = vec![transfer(
        &payer,
        &fixture.source,
        create.source_top_up_lamports,
    )];
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

    // Hostile 2 — surplus funding. The pre-Market initializer owns exact
    // ledger custody; CreateFund must refuse even one extra lamport and the
    // enclosing transaction must roll that hostile transfer back.
    let before_surplus = retirement_snapshot(&mut context, &fixture).await;
    assert!(
        submit(
            &mut context,
            &[
                transfer(&payer, &fixture.source, create.source_top_up_lamports),
                transfer(&payer, &fixture.funding, 1),
                create.instruction.clone(),
            ],
        )
        .await
        .is_err(),
        "a surplus-funded Resolution ledger must refuse"
    );
    assert_eq!(
        retirement_snapshot(&mut context, &fixture).await,
        before_surplus,
        "surplus refusal rolls back Source, subset ledger, Market and RentCredit"
    );

    // The honest creation.
    submit(
        &mut context,
        &[
            transfer(&payer, &fixture.source, create.source_top_up_lamports),
            create.instruction.clone(),
        ],
    )
    .await
    .expect("an Open Market creates Source against its pre-Market Resolution ledger");
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
    assert_eq!(
        observed(&mut context, fixture.funding)
            .await
            .expect("existing Resolution ledger after CreateFund"),
        pre_market_ledger,
        "CreateFund must leave the initializer-owned Pending ledger bytes and lamports unchanged"
    );
    assert_funding_ledger_status(&mut context, &fixture, FundingLedgerStatusV2::Pending).await;

    // Hostile 3 — double create. The Source PDA is one per Market generation
    // and `require_prepaid_output` refuses anything that is not
    // System-owned and empty, so the second creation cannot overwrite the
    // first.
    let before_double = retirement_snapshot(&mut context, &fixture).await;
    assert!(
        submit(&mut context, &[create.instruction.clone()])
            .await
            .is_err(),
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
    // three active rows in one FundingLedgerV2, which is what `AdmitTerminal`
    // rechecks.
    //
    // Activation is its own transition, exactly as in the readiness ladder:
    // `CreateFund` leaves the subset ledger Pending, `ActivateFund` moves the
    // three rows to Active, and only then does `VerifyFundReady` have the
    // Active ledger its builder authenticates. This arc used to call
    // `VerifyFundReady` straight off a Pending ledger, which no builder
    // accepts; it was unreachable behind the recovery-material refusal above
    // and so had never run.
    let activation = build_resolution_activate_fund_v1(&ResolutionActivateFundSnapshotV1 {
        pending: verify_snapshot(&mut context, &fixture).await,
        system_program: required_observed(&mut context, system_program::ID).await,
    })
    .expect("chain-derived direct activation against an Open Market");
    let activation_beneficiary_before = observed(&mut context, fixture.rent_credit)
        .await
        .expect("RentCredit")
        .lamports;
    let mut activation_instructions = Vec::with_capacity(2);
    if activation.receipt_top_up_lamports != 0 {
        activation_instructions.push(transfer(
            &payer,
            &fixture.activation_receipt,
            activation.receipt_top_up_lamports,
        ));
    }
    activation_instructions.push(activation.instruction);
    submit(&mut context, &activation_instructions)
        .await
        .expect("activate the three-row Resolution funding ledger of an Open Market");
    assert_funding_ledger_status(&mut context, &fixture, FundingLedgerStatusV2::Active).await;
    assert_eq!(
        observed(&mut context, fixture.rent_credit)
            .await
            .expect("RentCredit")
            .lamports,
        activation_beneficiary_before + activation.expected_beneficiary_credit_lamports
    );

    let verify =
        build_resolution_verify_fund_ready_v3(&verify_snapshot(&mut context, &fixture).await)
            .expect("chain-derived VerifyFundReady against an Open Market");
    validate_resolution_verify_fund_ready_report_v3(&verify).expect("exact VerifyFundReady report");
    submit(&mut context, &[verify.instruction])
        .await
        .expect("VerifyFundReady rechecks the Active ledger of an Open Market");
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
    // The beneficiary is credited ONCE, at activation. `VerifyFundReady`
    // rechecks the same fact and adds nothing, so the base here is the
    // pre-activation balance -- exactly as the readiness ladder measures it.
    assert_eq!(
        observed(&mut context, fixture.rent_credit)
            .await
            .expect("RentCredit")
            .lamports,
        activation_beneficiary_before + verify.expected_beneficiary_credit_lamports
    );
    assert_funding_ledger_status(&mut context, &fixture, FundingLedgerStatusV2::Active).await;

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
    .expect("chain-derived direct Resolution provider execution");
    pyth_provider::submit(&mut context, &[provider_execute.instruction], &[&resolver])
        .await
        .expect("Resolution persists the atomically founded Market's terminal certificate");
    let admit = build_resolution_admit_terminal_v3(&admit_snapshot(&mut context, &fixture).await)
        .expect("chain-derived atomically founded terminal Accept");
    let (admitted, admit_extent) = submit_recorded_v0(
        &mut context,
        &[admit.instruction],
        &[],
        "core-v3: Core admits the terminal state without a child invocation",
    )
    .await;
    admitted.expect("Core accepts the terminal state of an atomically founded Market");
    // The widest of the three, 224 over. Core's outer terminal-admit frame is a
    // 22-account constant and the request carries the rest.
    assert_eq!(
        admit_extent,
        PacketExtentV1 {
            legacy_bytes: 1_456,
            v0_bytes: 841,
            static_keys: 2,
            loaded_addresses: 21,
        }
    );
    assert!(admit_extent.legacy_bytes > PACKET_DATA_BYTES);
    assert!(admit_extent.v0_bytes <= PACKET_DATA_BYTES);

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
    //
    // Named by discriminant, not by `is_err()`. This hostile had never
    // executed: every path to it panicked earlier on the recovery-material
    // refusal, and a bare `is_err()` would have passed on whichever conjunct
    // happened to refuse first even once it did run.
    assert_eq!(
        build_resolution_create_fund_v3(&create_snapshot(&mut context, &fixture).await),
        Err(ResolutionCoreOperatorErrorV3::Market),
        "a Market carrying a terminal receipt must not create a Resolution Fund"
    );

    // The whole resolution ran over a market carrying a Direct capability root,
    // and the root is exactly where it was. The address is a PDA of the
    // release-selected TRADING program, which is why this assertion could not
    // exist while the Trading role was the Custody ELF wearing a second hat.
    assert_eq!(
        observed(&mut context, fixture.direct_root).await,
        Some(direct_root_before),
        "the Resolution walk moved a capability root it must never read"
    );
}
