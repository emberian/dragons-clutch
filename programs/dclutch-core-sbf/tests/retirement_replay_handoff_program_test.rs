//! Real-ELF proof of the retirement-only Trading-to-Core Custody replay handoff.

use std::{env, fs, path::PathBuf, vec, vec::Vec};

use dclutch_claims_svm::liability_basis_state_v2::{
    LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_MARKET_SEED_V2,
    LiabilityBasisMarketInputV2, encode_liability_basis_market_into_v2,
};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CUSTODY_REPLAY_BYTES_V1, CompartmentV1, CustodyAuthoritySeedsV1, CustodyReplaySeedsV1,
    CustodyReplayV1, CustodyVaultSeedsV1, RetirementReplayHandoffRequestV1,
};
use dclutch_market_core_codec::{
    CoreState, Identity as CoreIdentity, MarketCoreStateSeedsV2, MarketIdentity, Phase, Readiness,
};
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1, RealmV1Input,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetV1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, activate_execution_role_into_v1,
    initialize_activation_cache_v1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, CallerAuthoritySeedsV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1,
    ExecutionRoleV1, ProgramIdentityV1,
};
use dclutch_rent_contract::{
    RefundAuthority,
    lifecycle_v2::{
        LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleAccountIdV2, LifecycleRentCreditV2,
    },
};
use dclutch_token_svm::{LEGACY_TOKEN_PROGRAM_ID, PRODUCTION_ADAPTER_RELEASES};
use solana_account::Account;
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_option::COption;
use solana_program_pack::Pack;
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
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
const REVISION: u64 = 7;
const CONTEXT: [u8; 32] = [0x44; 32];

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
        2_000_000_000,
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
        2_000_000_000,
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
        accounts: vec![
            AccountMeta::new(fixture.payer.pubkey(), true),
            AccountMeta::new_readonly(fixture.market, false),
            AccountMeta::new_readonly(fixture.cache, false),
            AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
            AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.core_programdata, false),
            AccountMeta::new_readonly(TRADING_PROGRAM_ID, false),
            AccountMeta::new_readonly(trading_programdata, false),
            AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.custody_programdata, false),
            AccountMeta::new_readonly(caller, false),
            AccountMeta::new_readonly(fixture.claims_aggregate, false),
            AccountMeta::new_readonly(fixture.realm_raw, false),
            AccountMeta::new_readonly(fixture.realm_staging, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
            AccountMeta::new(fixture.rent_credit, false),
            AccountMeta::new(fixture.trading_replay, false),
            AccountMeta::new(fixture.core_replay, false),
            AccountMeta::new_readonly(fixture.hoard, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(fixture.mint, false),
            AccountMeta::new_readonly(Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID), false),
            AccountMeta::new_readonly(fixture.authority, false),
        ],
        data: request_bytes.to_vec(),
    }
}

async fn submit(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    handoff: Instruction,
) -> Result<(), BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let transaction = Transaction::new_signed_with_payer(
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
            handoff,
        ],
        Some(&context.payer.pubkey()),
        &[&context.payer, &fixture.payer],
        blockhash,
    );
    context.banks_client.process_transaction(transaction).await
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
    submit(&mut context, &fixture, instruction(&fixture, Fault::None))
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
    assert!(
        submit(&mut context, &fixture, instruction(&fixture, Fault::None))
            .await
            .is_err(),
        "replay must refuse"
    );
    assert_eq!(snapshot(&mut context, &fixture).await, accepted);
}

#[tokio::test]
async fn hostile_substitution_replay_partial_rent_phase_and_release_refuse_with_rollback() {
    for fault in [
        Fault::Context,
        Fault::ReplayDigest,
        Fault::PartialCoreReplay,
        Fault::Rent,
        Fault::Phase,
        Fault::Release,
    ] {
        let (test, fixture) = fixture(fault);
        let mut context = test.start_with_context().await;
        let before = snapshot(&mut context, &fixture).await;
        assert!(
            submit(&mut context, &fixture, instruction(&fixture, fault))
                .await
                .is_err(),
            "{fault:?} must refuse"
        );
        assert_eq!(
            snapshot(&mut context, &fixture).await,
            before,
            "{fault:?} must roll back byte-for-byte"
        );
    }
}
