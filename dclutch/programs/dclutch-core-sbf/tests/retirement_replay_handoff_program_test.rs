//! Real-ELF proof of the retirement-only Trading-to-Core Custody replay handoff.

use std::{env, fs, path::PathBuf, vec, vec::Vec};

use dclutch_claims::liability_basis_state_v2::{
    LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_MARKET_SEED_V2,
    LiabilityBasisMarketInputV2, encode_liability_basis_market_into_v2,
};
use dclutch_core_contract::ContentId;
use dclutch_custody::token_svm::{LEGACY_TOKEN_PROGRAM_ID, PRODUCTION_ADAPTER_RELEASES};
use dclutch_custody::{
    CUSTODY_REPLAY_BYTES_V1, CompartmentV1, CustodyAuthoritySeedsV1, CustodyReplaySeedsV1,
    CustodyReplayV1, CustodyVaultSeedsV1, RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1,
    RetirementReplayHandoffAccountLayoutV1 as Layout, RetirementReplayHandoffRequestV1,
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
use dclutch_program_test_evidence::TransactionEvidence;
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry::release_set::{
    ArtifactReleaseIdV1, CallerAuthoritySeedsV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1,
    ExecutionRoleV1, ProgramIdentityV1,
};
use dclutch_registry::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetV1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, activate_execution_role_into_v1,
    initialize_activation_cache_v1,
};
use solana_account::Account;
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_message::Message;
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
use solana_sdk::{
    instruction::InstructionError,
    signature::{Keypair, Signer},
    transaction::TransactionError,
};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_transaction::Transaction;
use spl_token_interface::state::{Account as SplAccount, AccountState as SplAccountState, Mint};

const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xd1; 32]);
const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xd2; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xd3; 32]);
const TRADING_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xd4; 32]);
const CLAIMS_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xd5; 32]);
const RENT_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xd6; 32]);
const GENERATION: u64 = 9;
/// The frame PAYER role's funded balance, and the one the honest request
/// commits when nothing debits that account for the message.
const PAYER_LAMPORTS: u64 = 2_000_000_000;
const REVISION: u64 = 7;
const CONTEXT: [u8; 32] = [0x44; 32];

/// `CoreSbfError::Reference`, `::Release`, `::Market` and `::Creation`.
const CORE_REFERENCE: u32 = 0x3003;
const CORE_RELEASE: u32 = 0x3004;
const CORE_MARKET: u32 = 0x3005;
const CORE_CREATION: u32 = 0x3007;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Fault {
    None,
    Context,
    ReplayDigest,
    PartialCoreReplay,
    Rent,
    Phase,
    Release,
}

struct Artifacts {
    core: Vec<u8>,
    custody: Vec<u8>,
}

struct Fixture {
    release_set: [u8; 32],
    market: Pubkey,
    realm_raw: Pubkey,
    realm_staging: Pubkey,
    cache: Pubkey,
    claims_aggregate: Pubkey,
    core_programdata: Pubkey,
    trading_programdata: Pubkey,
    custody_programdata: Pubkey,
    substituted_programdata: Pubkey,
    trading_replay: Pubkey,
    core_replay: Pubkey,
    hoard: Pubkey,
    authority: Pubkey,
    mint: Pubkey,
    rent_credit: Pubkey,
    payer: Keypair,
    request: RetirementReplayHandoffRequestV1,
    source: CustodyReplayV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Snapshot {
    market: Account,
    trading_replay: Option<Account>,
    core_replay: Option<Account>,
    hoard: Account,
    rent_credit: Account,
    payer: Account,
}

fn artifacts() -> Artifacts {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    Artifacts {
        core: fs::read(directory.join("dclutch_core_sbf.so")).expect("Core ELF"),
        custody: fs::read(directory.join("dclutch_custody_sbf.so")).expect("Custody ELF"),
    }
}

fn program_identity(key: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(key.to_bytes()).expect("program identity")
}

fn programdata_address(program: Pubkey) -> Pubkey {
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

fn add_upgradeable_program(
    test: &mut ProgramTest,
    artifact_name: &'static str,
    program: Pubkey,
    elf: &[u8],
) {
    test.add_upgradeable_program_to_genesis(artifact_name, &program);
    let data = immutable_programdata(elf);
    test.add_account(
        programdata_address(program),
        Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner: bpf_loader_upgradeable::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn release(program: Pubkey, semantic: u8, elf: &[u8]) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        program_identity(program),
        program_identity(bpf_loader_upgradeable::ID),
        programdata_address(program).to_bytes(),
        ContentId::new([semantic; 32]).expect("semantic release"),
        hash(elf).to_bytes(),
        0,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("artifact release")
}

fn artifact_id(release: ArtifactReleaseV1) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(hash(&release.to_bytes()).to_bytes()).expect("artifact ID")
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
        .expect("deployment"),
    )
}

fn activation(artifacts: &Artifacts) -> ([u8; 32], Vec<u8>) {
    let releases = [
        release(CORE_PROGRAM_ID, 0x51, &artifacts.core),
        release(CLAIMS_PROGRAM_ID, 0x52, &artifacts.core),
        release(TRADING_PROGRAM_ID, 0x53, &artifacts.core),
        release(CORE_PROGRAM_ID, 0x51, &artifacts.core),
        release(CUSTODY_PROGRAM_ID, 0x54, &artifacts.custody),
    ];
    let release_set = ExecutionReleaseSetV1::new(
        binding(releases[0]),
        binding(releases[1]),
        binding(releases[2]),
        binding(releases[3]),
        binding(releases[4]),
    )
    .expect("release set");
    let release_set_id = hash(&release_set.to_bytes()).to_bytes();
    let content = ContentId::new(release_set_id).expect("release set ID");
    let mut bytes = vec![0; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut bytes, content).expect("cache");
    for (role, release) in [
        (ExecutionRoleV1::Core, releases[0]),
        (ExecutionRoleV1::Claims, releases[1]),
        (ExecutionRoleV1::Trading, releases[2]),
        (ExecutionRoleV1::Resolution, releases[3]),
        (ExecutionRoleV1::Custody, releases[4]),
    ] {
        activate_execution_role_into_v1(
            &mut bytes,
            content,
            &release_set,
            role,
            &activation_input(release),
        )
        .expect("activate role");
    }
    ActivatedExecutionReleaseSetV1::decode(&bytes).expect("complete cache");
    (release_set_id, bytes)
}

fn add_account_with_lamports(
    test: &mut ProgramTest,
    key: Pubkey,
    owner: Pubkey,
    data: Vec<u8>,
    lamports: u64,
) {
    test.add_account(
        key,
        Account {
            lamports,
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn add_account(test: &mut ProgramTest, key: Pubkey, owner: Pubkey, data: Vec<u8>) {
    let lamports = Rent::default().minimum_balance(data.len()).max(1);
    add_account_with_lamports(test, key, owner, data, lamports);
}

fn mint_data() -> Vec<u8> {
    let mut bytes = vec![0; Mint::LEN];
    Mint::pack(
        Mint {
            mint_authority: COption::None,
            supply: 500_000,
            decimals: 6,
            is_initialized: true,
            freeze_authority: COption::None,
        },
        &mut bytes,
    )
    .expect("Mint");
    bytes
}

fn token_data(mint: Pubkey, authority: Pubkey) -> Vec<u8> {
    let mut bytes = vec![0; SplAccount::LEN];
    SplAccount::pack(
        SplAccount {
            mint,
            owner: authority,
            amount: 500_000,
            delegate: COption::None,
            state: SplAccountState::Initialized,
            is_native: COption::None,
            delegated_amount: 0,
            close_authority: COption::None,
        },
        &mut bytes,
    )
    .expect("token Account");
    bytes
}

fn fixture(fault: Fault) -> (ProgramTest, Fixture) {
    let artifacts = artifacts();
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    for (name, program, elf) in [
        (
            "dclutch_core_sbf",
            CORE_PROGRAM_ID,
            artifacts.core.as_slice(),
        ),
        (
            "dclutch_core_sbf",
            TRADING_PROGRAM_ID,
            artifacts.core.as_slice(),
        ),
        (
            "dclutch_core_sbf",
            CLAIMS_PROGRAM_ID,
            artifacts.core.as_slice(),
        ),
        (
            "dclutch_core_sbf",
            REGISTRY_PROGRAM_ID,
            artifacts.core.as_slice(),
        ),
        (
            "dclutch_custody_sbf",
            CUSTODY_PROGRAM_ID,
            artifacts.custody.as_slice(),
        ),
    ] {
        add_upgradeable_program(&mut test, name, program, elf);
    }
    let (release_set, cache_data) = activation(&artifacts);
    let cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    add_account(&mut test, cache, REGISTRY_PROGRAM_ID, cache_data);

    let mint = Pubkey::new_unique();
    let adapter = PRODUCTION_ADAPTER_RELEASES[0];
    let realm_value = RealmV1::new(RealmV1Input {
        token_program: LEGACY_TOKEN_PROGRAM_ID,
        collateral_mint: mint.to_bytes(),
        collateral_adapter_release_id: hash(&adapter.to_bytes()).to_bytes(),
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    })
    .expect("Realm");
    let realm_data = realm_value.to_bytes().to_vec();
    let realm = hash(&realm_data).to_bytes();
    let realm_raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &REALM_SCHEMA_RELEASE_ID_V1, &realm],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    let realm_staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &REALM_SCHEMA_RELEASE_ID_V1,
            &realm,
        ],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    add_account(&mut test, realm_raw, REGISTRY_PROGRAM_ID, realm_data);
    add_account(&mut test, realm_staging, system_program::ID, Vec::new());
    add_account(
        &mut test,
        mint,
        Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID),
        mint_data(),
    );

    let payer = Keypair::new();
    add_account_with_lamports(
        &mut test,
        payer.pubkey(),
        system_program::ID,
        Vec::new(),
        PAYER_LAMPORTS,
    );
    let mut identity = MarketIdentity {
        market_id: CoreIdentity::new([0xff; 32]).expect("placeholder"),
        realm_id: CoreIdentity::new(realm).expect("Realm ID"),
        product_record: CoreIdentity::new([0x61; 32]).expect("Product record"),
        product_id: CoreIdentity::new([0x62; 32]).expect("Product"),
        resolution_policy: CoreIdentity::new([0x63; 32]).expect("resolution"),
        capability_manifest: CoreIdentity::new([0x64; 32]).expect("manifest"),
        selected_release_set: CoreIdentity::new(release_set).expect("release"),
        registry_program: CoreIdentity::new(REGISTRY_PROGRAM_ID.to_bytes()).expect("Registry"),
        generation: GENERATION,
    };
    let market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(identity).as_slices(),
        &CORE_PROGRAM_ID,
    )
    .0;
    identity.market_id = CoreIdentity::new(market.to_bytes()).expect("Market");
    let (rent_credit, rent_credit_bump) = Pubkey::find_program_address(
        &[
            LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
            market.as_ref(),
            &GENERATION.to_le_bytes(),
        ],
        &RENT_PROGRAM_ID,
    );
    let rent_credit_value = LifecycleRentCreditV2::new(
        RefundAuthority::new([0x71; 32]).expect("refund wallet"),
        LifecycleAccountIdV2::new(market.to_bytes()).expect("Market identity"),
        LifecycleAccountIdV2::new(release_set).expect("release identity"),
        GENERATION,
        rent_credit_bump,
    )
    .expect("RentCredit");
    let rent_credit_data = rent_credit_value.to_bytes().to_vec();
    let rent_credit_lamports = Rent::default()
        .minimum_balance(rent_credit_data.len())
        .saturating_add(50_000);
    add_account_with_lamports(
        &mut test,
        rent_credit,
        RENT_PROGRAM_ID,
        rent_credit_data,
        rent_credit_lamports,
    );
    let (phase, terminal_receipt) = if fault == Fault::Phase {
        (Phase::Open, None)
    } else {
        (
            Phase::Retiring,
            Some(CoreIdentity::new([0x72; 32]).expect("terminal receipt")),
        )
    };
    let state = CoreState {
        phase,
        readiness: Readiness::Consumed,
        terminal_winner: if phase == Phase::Open { 0 } else { 1 },
        identity,
        outstanding_capabilities: 0,
        principal_cap_sets: u64::MAX,
        rent_beneficiary: CoreIdentity::new(rent_credit.to_bytes()).expect("RentCredit"),
        terminal_receipt,
        bumps: StateBumpsV1::UNRECORDED,
    };
    add_account(
        &mut test,
        market,
        CORE_PROGRAM_ID,
        state.encode().expect("Market state").to_vec(),
    );

    let claims_aggregate = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, market.as_ref()],
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    let mut claims_data = vec![0; LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + 8];
    encode_liability_basis_market_into_v2(
        LiabilityBasisMarketInputV2 {
            revision: 3,
            logical_market: market.to_bytes(),
            release_set,
            registry_program: REGISTRY_PROGRAM_ID.to_bytes(),
            product_instance_id: identity.product_id.to_bytes(),
            basis_id: [0x73; 32],
            realm_id: realm,
            custody_context: CONTEXT,
            generation: GENERATION,
        },
        &[500_000],
        &mut claims_data,
    )
    .expect("Claims aggregate");
    add_account(&mut test, claims_aggregate, CLAIMS_PROGRAM_ID, claims_data);

    let trading_replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::new(
            market.to_bytes(),
            release_set,
            ExecutionRoleV1::Trading,
            CONTEXT,
        )
        .as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let core_replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::new(
            market.to_bytes(),
            release_set,
            ExecutionRoleV1::Core,
            CONTEXT,
        )
        .as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let source = CustodyReplayV1 {
        caller_role: ExecutionRoleV1::Trading,
        release_set,
        market: market.to_bytes(),
        realm,
        context: CONTEXT,
        caller_program: TRADING_PROGRAM_ID.to_bytes(),
        rent_refund: rent_credit.to_bytes(),
        open_vault_count: 1,
        next_revision: REVISION,
        generation: GENERATION,
        last_request_digest: [0x74; 32],
        last_poststate_commitment: [0x75; 32],
    };
    let source_data = source.to_bytes().expect("replay").to_vec();
    let source_lamports = Rent::default()
        .minimum_balance(CUSTODY_REPLAY_BYTES_V1)
        .saturating_add(111);
    add_account_with_lamports(
        &mut test,
        trading_replay,
        CUSTODY_PROGRAM_ID,
        source_data.clone(),
        source_lamports,
    );
    if fault == Fault::PartialCoreReplay {
        add_account_with_lamports(&mut test, core_replay, system_program::ID, Vec::new(), 1);
    }
    let authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::new(market.to_bytes(), release_set).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let hoard = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            market.to_bytes(),
            release_set,
            CONTEXT,
            CompartmentV1::HoardPrincipal,
        )
        .as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let hoard_data = token_data(mint, authority);
    let hoard_lamports = Rent::default()
        .minimum_balance(hoard_data.len())
        .saturating_add(222);
    add_account_with_lamports(
        &mut test,
        hoard,
        Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID),
        hoard_data.clone(),
        hoard_lamports,
    );

    let substituted_programdata = Pubkey::new_unique();
    add_account(
        &mut test,
        substituted_programdata,
        bpf_loader_upgradeable::ID,
        immutable_programdata(&artifacts.core),
    );
    let mut request_context = CONTEXT;
    let mut replay_digest = hash(&source_data).to_bytes();
    let mut core_rent = Rent::default().minimum_balance(CUSTODY_REPLAY_BYTES_V1);
    if fault == Fault::Context {
        request_context = [0x76; 32];
    }
    if fault == Fault::ReplayDigest {
        replay_digest = [0x77; 32];
    }
    if fault == Fault::Rent {
        core_rent = core_rent.saturating_add(1);
    }
    let request = RetirementReplayHandoffRequestV1::new(
        market.to_bytes(),
        request_context,
        replay_digest,
        hash(&hoard_data).to_bytes(),
        GENERATION,
        REVISION,
        source_lamports,
        core_rent,
        hoard_lamports,
        rent_credit_lamports,
        PAYER_LAMPORTS,
    )
    .expect("handoff request");
    (
        test,
        Fixture {
            release_set,
            market,
            realm_raw,
            realm_staging,
            cache,
            claims_aggregate,
            core_programdata: programdata_address(CORE_PROGRAM_ID),
            trading_programdata: programdata_address(TRADING_PROGRAM_ID),
            custody_programdata: programdata_address(CUSTODY_PROGRAM_ID),
            substituted_programdata,
            trading_replay,
            core_replay,
            hoard,
            authority,
            mint,
            rent_credit,
            payer,
            request,
            source,
        },
    )
}

fn instruction(fixture: &Fixture, fault: Fault) -> Instruction {
    let request_bytes = fixture.request.to_bytes();
    let caller = Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::from_bytes(
            fixture.release_set,
            fixture.market.to_bytes(),
            ExecutionRoleV1::Core,
            fixture.request.context(),
            hash(&request_bytes).to_bytes(),
        )
        .expect("caller seeds")
        .as_slices(),
        &CORE_PROGRAM_ID,
    )
    .0;
    let trading_programdata = if fault == Fault::Release {
        fixture.substituted_programdata
    } else {
        fixture.trading_programdata
    };
    Instruction {
        program_id: CORE_PROGRAM_ID,
        accounts: frame([
            (
                Layout::PAYER,
                AccountMeta::new(fixture.payer.pubkey(), true),
            ),
            (
                Layout::MARKET,
                AccountMeta::new_readonly(fixture.market, false),
            ),
            (
                Layout::CACHE,
                AccountMeta::new_readonly(fixture.cache, false),
            ),
            (
                Layout::REGISTRY,
                AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
            ),
            (
                Layout::CORE_PROGRAM,
                AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
            ),
            (
                Layout::CORE_PROGRAMDATA,
                AccountMeta::new_readonly(fixture.core_programdata, false),
            ),
            (
                Layout::TRADING_PROGRAM,
                AccountMeta::new_readonly(TRADING_PROGRAM_ID, false),
            ),
            (
                Layout::TRADING_PROGRAMDATA,
                AccountMeta::new_readonly(trading_programdata, false),
            ),
            (
                Layout::CUSTODY_PROGRAM,
                AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false),
            ),
            (
                Layout::CUSTODY_PROGRAMDATA,
                AccountMeta::new_readonly(fixture.custody_programdata, false),
            ),
            (
                Layout::CALLER_AUTHORITY,
                AccountMeta::new_readonly(caller, false),
            ),
            (
                Layout::CLAIMS_AGGREGATE,
                AccountMeta::new_readonly(fixture.claims_aggregate, false),
            ),
            (
                Layout::REALM,
                AccountMeta::new_readonly(fixture.realm_raw, false),
            ),
            (
                Layout::REALM_STAGING,
                AccountMeta::new_readonly(fixture.realm_staging, false),
            ),
            (
                Layout::RENT,
                AccountMeta::new_readonly(sysvar::rent::ID, false),
            ),
            (
                Layout::RENT_CREDIT,
                AccountMeta::new(fixture.rent_credit, false),
            ),
            (
                Layout::TRADING_REPLAY,
                AccountMeta::new(fixture.trading_replay, false),
            ),
            (
                Layout::CORE_REPLAY,
                AccountMeta::new(fixture.core_replay, false),
            ),
            (
                Layout::HOARD,
                AccountMeta::new_readonly(fixture.hoard, false),
            ),
            (
                Layout::SYSTEM,
                AccountMeta::new_readonly(system_program::ID, false),
            ),
            (Layout::MINT, AccountMeta::new_readonly(fixture.mint, false)),
            (
                Layout::TOKEN_PROGRAM,
                AccountMeta::new_readonly(Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID), false),
            ),
            (
                Layout::CUSTODY_AUTHORITY,
                AccountMeta::new_readonly(fixture.authority, false),
            ),
        ]),
        data: request_bytes.to_vec(),
    }
}

/// Order one frame by the ordinals `RetirementReplayHandoffAccountLayoutV1`
/// owns, rather than by the order they happen to be written here.
///
/// Core parses this route as `&[AccountInfo; 23]` and then indexes it by those
/// same constants, so a positional list agrees with the program only until
/// somebody renumbers the layout -- at which point every coordinate is quietly
/// mislabelled and the test still passes on some other refusal. `67e96e5b` is
/// the same defect one layer out: a literal frame that had stopped matching
/// the route it was submitting to.
fn frame(
    entries: [(usize, AccountMeta); RETIREMENT_REPLAY_HANDOFF_ACCOUNT_COUNT_V1],
) -> Vec<AccountMeta> {
    let mut ordered = entries;
    ordered.sort_by_key(|(ordinal, _)| *ordinal);
    for (index, (ordinal, _)) in ordered.iter().enumerate() {
        assert_eq!(
            index, *ordinal,
            "the handoff layout owns no ordinal {index}: this frame is not the frame Core parses"
        );
    }
    ordered.into_iter().map(|(_, meta)| meta).collect()
}

/// The exact custom refusal a submission produced, or `None` if it succeeded.
///
/// All six hostilities below and the honest test's replay asserted only
/// `is_err()` until 2026-08-30 -- the shape `67e96e5b` caught passing on a
/// refusal none of its cases was about.
fn refusal(result: Result<(), BanksClientError>) -> Option<u32> {
    match result {
        Ok(()) => None,
        Err(BanksClientError::TransactionError(TransactionError::InstructionError(
            _,
            InstructionError::Custom(code),
        ))) => Some(code),
        Err(other) => panic!("expected a program refusal, got {other:?}"),
    }
}

/// Submit one handoff and record it for the census.
///
/// The campaign has driven both `core/retirement_replay_handoff_v1::process`
/// and, by CPI, `custody/retirement_replay_handoff_v1::process` against real
/// ELFs since it landed, and both read NEVER-EXECUTED in the route register the
/// whole time: it emitted no evidence, so no binding could be corroborated
/// against it. `record()` is a no-op unless a runner sets
/// `DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR`, so this stays an ordinary test.
///
/// The label carries the FAULT, not just the act. Six hostiles share this one
/// call site and they raise four different codes; one label across all six
/// would let a binding read the code off whichever ran first, which is the
/// exact defect the census refuses.
async fn submit(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    handoff: Instruction,
    label: &str,
) -> Result<(), BanksClientError> {
    submit_paid_by(context, fixture, handoff, label, FeePayer::Harness).await
}

/// Which account Solana debits for the message.
///
/// Not a detail of the harness: `accounts[PAYER].lamports()` is a conjunct of
/// this route, and the fee is taken from the fee payer while the transaction
/// LOADS, so a route that reads the fee payer's balance reads it net of the
/// fee. Every case above pays from the harness's own mint account, which leaves
/// the frame's PAYER role never debited and its two balances the same number --
/// so none of them could have caught the operator committing the wrong one.
/// `FeePayer::Frame` is the devnet shape, where the frame's PAYER role and the
/// message's fee payer are one account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FeePayer {
    Harness,
    Frame,
}

async fn submit_paid_by(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    handoff: Instruction,
    label: &str,
    fee_payer: FeePayer,
) -> Result<(), BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let instructions = [
        ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
        handoff,
    ];
    let transaction = match fee_payer {
        FeePayer::Harness => Transaction::new_signed_with_payer(
            &instructions,
            Some(&context.payer.pubkey()),
            &[&context.payer, &fixture.payer],
            blockhash,
        ),
        FeePayer::Frame => Transaction::new_signed_with_payer(
            &instructions,
            Some(&fixture.payer.pubkey()),
            &[&fixture.payer],
            blockhash,
        ),
    };
    let signature = transaction
        .signatures
        .first()
        .expect("a submitted transaction carries its own signature")
        .to_string();
    let wire_bytes = 1 + transaction.signatures.len() * 64 + transaction.message_data().len();
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
        wire_bytes: Some(wire_bytes),
    })
    .expect("campaign evidence must be writable when the gauntlet asked for it");
    processed.result.map_err(BanksClientError::TransactionError)
}

async fn snapshot(context: &mut ProgramTestContext, fixture: &Fixture) -> Snapshot {
    Snapshot {
        market: context
            .banks_client
            .get_account(fixture.market)
            .await
            .expect("market read")
            .expect("Market"),
        trading_replay: context
            .banks_client
            .get_account(fixture.trading_replay)
            .await
            .expect("Trading replay read"),
        core_replay: context
            .banks_client
            .get_account(fixture.core_replay)
            .await
            .expect("Core replay read"),
        hoard: context
            .banks_client
            .get_account(fixture.hoard)
            .await
            .expect("Hoard read")
            .expect("Hoard"),
        rent_credit: context
            .banks_client
            .get_account(fixture.rent_credit)
            .await
            .expect("RentCredit read")
            .expect("RentCredit"),
        payer: context
            .banks_client
            .get_account(fixture.payer.pubkey())
            .await
            .expect("payer read")
            .expect("payer"),
    }
}

#[tokio::test]
async fn real_sbf_handoff_preserves_lineage_principal_and_exact_rent_arithmetic() {
    let (test, fixture) = fixture(Fault::None);
    let mut context = test.start_with_context().await;
    let before = snapshot(&mut context, &fixture).await;
    submit(
        &mut context,
        &fixture,
        instruction(&fixture, Fault::None),
        "core replay handoff: the Trading-role replay becomes a Core-role replay",
    )
    .await
    .expect("handoff succeeds");
    let after = snapshot(&mut context, &fixture).await;
    assert_eq!(after.market, before.market);
    assert_eq!(after.hoard, before.hoard);
    assert!(after.trading_replay.is_none());
    let core = after.core_replay.as_ref().expect("Core replay created");
    let decoded = CustodyReplayV1::decode(&core.data).expect("Core replay bytes");
    assert_eq!(
        decoded,
        CustodyReplayV1 {
            caller_role: ExecutionRoleV1::Core,
            caller_program: CORE_PROGRAM_ID.to_bytes(),
            ..fixture.source
        }
    );
    assert_eq!(
        before.payer.lamports - after.payer.lamports,
        fixture.request.core_replay_rent_lamports()
    );
    assert_eq!(
        after.rent_credit.lamports - before.rent_credit.lamports,
        fixture.request.trading_replay_lamports()
    );

    let accepted = after.clone();
    // The Trading replay the first pass closed is now a vacant system account,
    // so `authenticate_prestate` refuses it on owner and width -- the same
    // conjunct, and the same code, that the digest and partial-creation
    // hostilities below reach. Naming it is what distinguishes "the route is
    // idempotent" from "the second submission failed somehow".
    assert_eq!(
        refusal(
            submit(
                &mut context,
                &fixture,
                instruction(&fixture, Fault::None),
                "core replay handoff: replayed against the prestate it no longer has",
            )
            .await
        ),
        Some(CORE_REFERENCE),
        "replay must refuse at the prestate it no longer has"
    );
    assert_eq!(snapshot(&mut context, &fixture).await, accepted);
}

#[tokio::test]
async fn hostile_substitution_replay_partial_rent_phase_and_release_refuse_with_rollback() {
    // Each code is the conjunct the case is actually about, and three of them
    // are NOT the ones a reader would guess:
    //
    //   Context           the Claims aggregate owns the retirement context, and
    //                     Core compares it before it ever reaches the replay,
    //                     so a divergent request context is a Reference.
    //   ReplayDigest      the Trading replay's own digest conjunct, Reference.
    //   PartialCoreReplay Reference, not Creation: Core authenticates the
    //                     vacant Core replay in the PRESTATE (owner, zero
    //                     lamports, empty data) and never reaches creation.
    //   Rent              Creation, and the earliest refusal here at 8.4k CU:
    //                     the request's own rent figure is compared against
    //                     the Rent sysvar before the Market is read at all.
    //   Phase             Market. Release  the activation join.
    for (fault, expected) in [
        (Fault::Context, CORE_REFERENCE),
        (Fault::ReplayDigest, CORE_REFERENCE),
        (Fault::PartialCoreReplay, CORE_REFERENCE),
        (Fault::Rent, CORE_CREATION),
        (Fault::Phase, CORE_MARKET),
        (Fault::Release, CORE_RELEASE),
    ] {
        let (test, fixture) = fixture(fault);
        let mut context = test.start_with_context().await;
        let before = snapshot(&mut context, &fixture).await;
        assert_eq!(
            refusal(
                submit(
                    &mut context,
                    &fixture,
                    instruction(&fixture, fault),
                    &format!("core replay handoff: hostile {fault:?}"),
                )
                .await
            ),
            Some(expected),
            "{fault:?} must refuse with its own code"
        );
        assert_eq!(
            snapshot(&mut context, &fixture).await,
            before,
            "{fault:?} must roll back byte-for-byte"
        );
    }
}

/// One author for the payer's lamports, and it is the message's own fee that
/// separates the two candidates.
fn request_with_payer_lamports(
    request: RetirementReplayHandoffRequestV1,
    payer_lamports: u64,
) -> RetirementReplayHandoffRequestV1 {
    RetirementReplayHandoffRequestV1::new(
        request.market(),
        request.context(),
        request.trading_replay_digest(),
        request.hoard_data_digest(),
        request.generation(),
        request.revision(),
        request.trading_replay_lamports(),
        request.core_replay_rent_lamports(),
        request.hoard_lamports(),
        request.rent_credit_lamports(),
        payer_lamports,
    )
    .expect("handoff request")
}

/// `getFeeForMessage` for the exact message this case signs. The harness owns
/// the number; the test never writes one.
async fn frame_payer_message_fee(context: &mut ProgramTestContext, fixture: &Fixture) -> u64 {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let message = Message::new_with_blockhash(
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
            instruction(fixture, Fault::None),
        ],
        Some(&fixture.payer.pubkey()),
        &blockhash,
    );
    context
        .banks_client
        .get_fee_for_message(message)
        .await
        .expect("fee quote")
        .expect("the harness quoted a fee for this message")
}

async fn payer_lamports(context: &mut ProgramTestContext, fixture: &Fixture) -> u64 {
    context
        .banks_client
        .get_account(fixture.payer.pubkey())
        .await
        .expect("payer read")
        .expect("payer")
        .lamports
}

/// THE FRAME'S PAYER ROLE PAYING FOR ITS OWN MESSAGE, WHICH IS THE DEVNET SHAPE.
///
/// Core compares `accounts[PAYER].lamports()` against `request.payer_lamports()`
/// and refuses `Reference`. Solana debits the fee payer while the transaction
/// LOADS, so when those two accounts are one account the route reads a balance
/// the message has already reduced. Every other case in this file pays from the
/// harness's mint keypair, so the frame's PAYER role was never debited and the
/// observed and read balances were the same number -- which is why a devnet
/// packet refused `0x3003` at 61,946 CU (cohort-17D) against a green file.
///
/// Both halves run against the SAME code and differ in eight bytes, so the code
/// is what the red half convicts. `Reference` is one code over many conjuncts;
/// what makes this case about the payer conjunct is not the code, it is that
/// the identical packet with those eight bytes reduced by the quoted fee lands.
#[tokio::test]
async fn a_request_built_from_the_pre_fee_balance_refuses_and_the_net_one_lands() {
    let (test, mut fixture) = fixture(Fault::None);
    let mut context = test.start_with_context().await;
    let before = snapshot(&mut context, &fixture).await;
    assert_eq!(before.payer.lamports, PAYER_LAMPORTS);

    let fee = frame_payer_message_fee(&mut context, &fixture).await;
    assert!(
        fee > 0,
        "a zero-fee harness cannot tell the two requests apart, and this case is only about their difference"
    );

    // RED. The fixture's request commits `PAYER_LAMPORTS`, the balance an
    // observer reads before the fee -- exactly what the operator committed.
    assert_eq!(
        refusal(
            submit_paid_by(
                &mut context,
                &fixture,
                instruction(&fixture, Fault::None),
                "core replay handoff: the request commits the balance before its own fee",
                FeePayer::Frame,
            )
            .await
        ),
        Some(CORE_REFERENCE),
        "a request built from the pre-fee balance must refuse at the payer conjunct"
    );
    let refused = snapshot(&mut context, &fixture).await;
    assert_eq!(
        refused.payer.lamports,
        PAYER_LAMPORTS - fee,
        "the refused message still paid its fee, which is the whole mechanism"
    );
    assert_eq!(
        Snapshot {
            payer: before.payer.clone(),
            ..refused.clone()
        },
        before,
        "apart from the fee the refusal must roll back byte for byte"
    );

    // GREEN. Re-quote for the message this half actually signs: the new request
    // moves the digest and therefore the caller-authority key, and moves
    // nothing that prices a message. The equality is the assertion.
    let observed = payer_lamports(&mut context, &fixture).await;
    assert_eq!(observed, PAYER_LAMPORTS - fee);
    fixture.request = request_with_payer_lamports(fixture.request, observed - fee);
    let requoted = frame_payer_message_fee(&mut context, &fixture).await;
    assert_eq!(
        requoted, fee,
        "changing the committed balance must not change what the message costs"
    );

    submit_paid_by(
        &mut context,
        &fixture,
        instruction(&fixture, Fault::None),
        "core replay handoff: the request commits the balance the route reads",
        FeePayer::Frame,
    )
    .await
    .expect("the request that commits the balance at load lands");

    let after = snapshot(&mut context, &fixture).await;
    assert!(after.trading_replay.is_none());
    assert_eq!(
        after.payer.lamports,
        observed - fee - fixture.request.core_replay_rent_lamports(),
        "the payer pays the fee and the Core replay's rent, and nothing else"
    );
    assert_eq!(
        after.rent_credit.lamports - refused.rent_credit.lamports,
        fixture.request.trading_replay_lamports()
    );
}
