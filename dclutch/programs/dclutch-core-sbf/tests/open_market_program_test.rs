//! Real-ELF Core -> Custody Market-open composition.

use std::{env, fs, path::PathBuf, vec, vec::Vec};

use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CallerRoleV1, CompartmentV1, ContextV1, CustodyAuthoritySeedsV1, CustodyReplaySeedsV1,
    CustodyReplayV1, CustodyRequestV1, CustodyVaultSeedsV1, OperationV1,
};
use dclutch_market_core_codec::{
    Action, CoreState, Identity as CoreIdentity, MarketCoreStateSeedsV1, MarketIdentity, Phase,
    Readiness, Request,
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
use dclutch_token_svm::{LEGACY_TOKEN_PROGRAM_ID, PRODUCTION_ADAPTER_RELEASES};
use solana_account::Account;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_option::COption;
use solana_program_pack::Pack;
use solana_program_test::ProgramTest;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_transaction::Transaction;
use spl_token_interface::state::Mint as SplMint;

const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xd1; 32]);
const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xd2; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xd3; 32]);
const GENERATION: u64 = 9;

struct Artifacts {
    core: Vec<u8>,
    custody: Vec<u8>,
    registry: Vec<u8>,
}

struct Fixture {
    release_set: [u8; 32],
    market: Pubkey,
    realm: [u8; 32],
    realm_raw: Pubkey,
    realm_staging: Pubkey,
    cache: Pubkey,
    core_programdata: Pubkey,
    custody_programdata: Pubkey,
    replay: Pubkey,
    vault: Pubkey,
    authority: Pubkey,
    mint: Pubkey,
    payer: Keypair,
}

fn artifacts() -> Artifacts {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    Artifacts {
        core: fs::read(directory.join("dclutch_core_sbf.so")).expect("Core ELF"),
        custody: fs::read(directory.join("dclutch_custody_sbf.so")).expect("Custody ELF"),
        registry: fs::read(directory.join("dclutch_registry_sbf.so")).expect("Registry ELF"),
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
    bytes
        .get_mut(0..4)
        .expect("tag")
        .copy_from_slice(&3_u32.to_le_bytes());
    bytes
        .get_mut(4..12)
        .expect("slot")
        .copy_from_slice(&0_u64.to_le_bytes());
    *bytes.get_mut(12).expect("authority option") = 0;
    bytes.get_mut(45..).expect("ELF").copy_from_slice(elf);
    bytes
}

fn add_upgradeable_program(
    test: &mut ProgramTest,
    name: &'static str,
    program: Pubkey,
    elf: &[u8],
) {
    test.add_upgradeable_program_to_genesis(name, &program);
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

fn release(program: Pubkey, seed: u8, elf: &[u8]) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        program_identity(program),
        program_identity(bpf_loader_upgradeable::ID),
        programdata_address(program).to_bytes(),
        ContentId::new([seed; 32]).expect("semantic release"),
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

fn activation(core: ArtifactReleaseV1, custody: ArtifactReleaseV1) -> ([u8; 32], Vec<u8>) {
    let core_binding = binding(core);
    let release_set = ExecutionReleaseSetV1::new(
        core_binding,
        core_binding,
        core_binding,
        core_binding,
        binding(custody),
    )
    .expect("release set");
    let release_set_id = hash(&release_set.to_bytes()).to_bytes();
    let content = ContentId::new(release_set_id).expect("release set ID");
    let mut bytes = vec![0; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut bytes, content).expect("cache");
    for (role, release) in [
        (ExecutionRoleV1::Core, core),
        (ExecutionRoleV1::Claims, core),
        (ExecutionRoleV1::Trading, core),
        (ExecutionRoleV1::Resolution, core),
        (ExecutionRoleV1::Custody, custody),
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

fn add_account(test: &mut ProgramTest, key: Pubkey, owner: Pubkey, data: Vec<u8>) {
    test.add_account(
        key,
        Account {
            lamports: Rent::default().minimum_balance(data.len()).max(1),
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    );
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
    .expect("Mint");
    bytes
}

fn fixture() -> (ProgramTest, Fixture) {
    let artifacts = artifacts();
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    add_upgradeable_program(
        &mut test,
        "dclutch_core_sbf",
        CORE_PROGRAM_ID,
        &artifacts.core,
    );
    add_upgradeable_program(
        &mut test,
        "dclutch_custody_sbf",
        CUSTODY_PROGRAM_ID,
        &artifacts.custody,
    );
    add_upgradeable_program(
        &mut test,
        "dclutch_registry_sbf",
        REGISTRY_PROGRAM_ID,
        &artifacts.registry,
    );
    let (release_set, cache_data) = activation(
        release(CORE_PROGRAM_ID, 0x51, &artifacts.core),
        release(CUSTODY_PROGRAM_ID, 0x52, &artifacts.custody),
    );
    let cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    add_account(&mut test, cache, REGISTRY_PROGRAM_ID, cache_data);

    let mint = Pubkey::new_unique();
    let adapter = PRODUCTION_ADAPTER_RELEASES
        .first()
        .copied()
        .expect("production adapter profile");
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
    test.add_account(
        payer.pubkey(),
        Account {
            lamports: 10_000_000_000,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    let mut identity = MarketIdentity {
        market_id: CoreIdentity::new([0xff; 32]).expect("placeholder"),
        realm_id: CoreIdentity::new(realm).expect("Realm ID"),
        product_id: CoreIdentity::new([0x61; 32]).expect("Product"),
        result_domain: CoreIdentity::new([0x62; 32]).expect("domain"),
        resolution_policy: CoreIdentity::new([0x63; 32]).expect("resolution"),
        capability_manifest: CoreIdentity::new([0x64; 32]).expect("manifest"),
        selected_release_set: CoreIdentity::new(release_set).expect("release"),
        registry_program: CoreIdentity::new(REGISTRY_PROGRAM_ID.to_bytes()).expect("Registry"),
        generation: GENERATION,
    };
    let market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV1::new(identity).as_slices(),
        &CORE_PROGRAM_ID,
    )
    .0;
    identity.market_id = CoreIdentity::new(market.to_bytes()).expect("Market");
    let state = CoreState {
        phase: Phase::Founding,
        readiness: Readiness::Ready,
        terminal_winner: 0,
        identity,
        outstanding_capabilities: 0,
        rent_beneficiary: CoreIdentity::new(payer.pubkey().to_bytes()).expect("beneficiary"),
        terminal_receipt: None,
    };
    add_account(
        &mut test,
        market,
        CORE_PROGRAM_ID,
        state.encode().expect("Market state").to_vec(),
    );
    let base = custody_request(
        release_set,
        market,
        realm,
        mint,
        payer.pubkey(),
        OperationV1::InitializeReplay,
    );
    let replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::from_request(base).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::from_request(base).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let mut vault_request = base;
    vault_request.destination_compartment = CompartmentV1::HoardPrincipal;
    vault_request.destination_vault_context = market.to_bytes();
    let vault = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::from_request(vault_request, false).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    (
        test,
        Fixture {
            release_set,
            market,
            realm,
            realm_raw,
            realm_staging,
            cache,
            core_programdata: programdata_address(CORE_PROGRAM_ID),
            custody_programdata: programdata_address(CUSTODY_PROGRAM_ID),
            replay,
            vault,
            authority,
            mint,
            payer,
        },
    )
}

fn custody_request(
    release_set: [u8; 32],
    market: Pubkey,
    realm: [u8; 32],
    mint: Pubkey,
    payer: Pubkey,
    operation: OperationV1,
) -> CustodyRequestV1 {
    let core_request = Request::administrative(
        Action::OpenMarket,
        GENERATION,
        CoreIdentity::new(market.to_bytes()).expect("Market"),
    )
    .encode()
    .expect("Core request");
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
        rent_refund: payer.to_bytes(),
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

fn core_instruction(fixture: &Fixture, operation: OperationV1) -> Instruction {
    let custody = custody_request(
        fixture.release_set,
        fixture.market,
        fixture.realm,
        fixture.mint,
        fixture.payer.pubkey(),
        operation,
    );
    let custody_bytes = custody.to_bytes().expect("Custody request");
    let authority = Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::new(
            ContentId::new(fixture.release_set).expect("release"),
            fixture.market.to_bytes(),
            ExecutionRoleV1::Core,
            fixture.market.to_bytes(),
            hash(&custody_bytes).to_bytes(),
        )
        .expect("caller seeds")
        .as_slices(),
        &CORE_PROGRAM_ID,
    )
    .0;
    let mut accounts = vec![
        AccountMeta::new_readonly(authority, false),
        AccountMeta::new(fixture.market, false),
        AccountMeta::new_readonly(fixture.cache, false),
        AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
        AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.core_programdata, false),
        AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.custody_programdata, false),
        AccountMeta::new_readonly(fixture.realm_raw, false),
        AccountMeta::new_readonly(fixture.realm_staging, false),
        AccountMeta::new(fixture.replay, false),
    ];
    match operation {
        OperationV1::InitializeReplay => accounts.extend([
            AccountMeta::new(fixture.payer.pubkey(), true),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ]),
        OperationV1::OpenVault => accounts.extend([
            AccountMeta::new_readonly(fixture.mint, false),
            AccountMeta::new(fixture.vault, false),
            AccountMeta::new_readonly(fixture.authority, false),
            AccountMeta::new_readonly(Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID), false),
            AccountMeta::new(fixture.payer.pubkey(), true),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ]),
        OperationV1::Transfer | OperationV1::CloseVault | OperationV1::CloseReplay => {
            accounts.clear();
        }
    }
    let core = Request::administrative(
        Action::OpenMarket,
        GENERATION,
        CoreIdentity::new(fixture.market.to_bytes()).expect("Market"),
    )
    .encode()
    .expect("Core request");
    let mut data = Vec::with_capacity(core.len() + custody_bytes.len());
    data.extend_from_slice(&core);
    data.extend_from_slice(&custody_bytes);
    Instruction {
        program_id: CORE_PROGRAM_ID,
        accounts,
        data,
    }
}

#[tokio::test]
async fn real_core_opens_exact_registry_realm_custody_and_commits_last() {
    let (test, fixture) = fixture();
    let context = test.start_with_context().await;
    let payer_before = context
        .banks_client
        .get_balance(fixture.payer.pubkey())
        .await
        .expect("payer balance");
    for operation in [OperationV1::InitializeReplay, OperationV1::OpenVault] {
        let blockhash = context
            .banks_client
            .get_latest_blockhash()
            .await
            .expect("blockhash");
        let transaction = Transaction::new_signed_with_payer(
            &[core_instruction(&fixture, operation)],
            Some(&context.payer.pubkey()),
            &[&context.payer, &fixture.payer],
            blockhash,
        );
        context
            .banks_client
            .process_transaction(transaction)
            .await
            .expect("Core/Custody effect");
    }
    let market = context
        .banks_client
        .get_account(fixture.market)
        .await
        .expect("Market query")
        .expect("Market");
    let state = CoreState::decode(&market.data).expect("CoreState");
    assert_eq!(state.phase, Phase::Open);
    assert_eq!(state.readiness, Readiness::Consumed);
    let replay = context
        .banks_client
        .get_account(fixture.replay)
        .await
        .expect("replay query")
        .expect("replay");
    let replay = CustodyReplayV1::decode(&replay.data).expect("Custody replay");
    assert_eq!(replay.next_revision, 2);
    assert_eq!(replay.open_vault_count, 1);
    let vault = context
        .banks_client
        .get_account(fixture.vault)
        .await
        .expect("Vault query")
        .expect("Vault");
    let profile = PRODUCTION_ADAPTER_RELEASES
        .first()
        .expect("production adapter profile")
        .profile();
    let token = profile
        .check_custody_account(
            LEGACY_TOKEN_PROGRAM_ID,
            &vault.data,
            fixture.mint.to_bytes(),
            fixture.authority.to_bytes(),
        )
        .expect("empty Hoard Vault");
    assert_eq!(token.amount, 0);
    let payer_after = context
        .banks_client
        .get_balance(fixture.payer.pubkey())
        .await
        .expect("payer balance");
    assert_eq!(
        payer_before.checked_sub(payer_after),
        Some(
            Rent::default().minimum_balance(dclutch_custody_contract::CUSTODY_REPLAY_BYTES_V1)
                + Rent::default().minimum_balance(dclutch_token_svm::ACCOUNT_BYTES)
        )
    );
}
