//! Real-ELF Core -> Custody Market-open composition.

use std::{env, fs, path::PathBuf, vec, vec::Vec};

use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CallerRoleV1, CompartmentV1, ContextV1, CustodyAuthoritySeedsV1, CustodyReplaySeedsV1,
    CustodyReplayV1, CustodyRequestV1, CustodyVaultSeedsV1, OperationV1,
};
use dclutch_market_core_codec::{
    Action, CoreState, Identity as CoreIdentity, MarketCoreStateSeedsV2, MarketIdentity, Phase,
    Readiness, Request, StateBumpsV1,
};
use dclutch_market_open_v1_operator::{
    REGISTRY_OPEN_MARKET_CONTINUATION_PREFIX_ACCOUNTS_V1, RegistryOpenMarketContinuationStateV1,
    build_registry_open_market_continuation_v1,
};
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1, RealmV1Input,
};
use dclutch_record_contract::{
    BeginRecordV1, CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1, ContentDigest, RAW_RECORD_PDA_SEED_V1,
    RecordKeyV1, STAGING_CURSOR_BYTES_V1, STAGING_CURSOR_PDA_SEED_V1, SchemaReleaseId,
};
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
use dclutch_resolution_core_v3_operator::{Finality, Observation, ObservedAccount};
use dclutch_token_svm::{LEGACY_TOKEN_PROGRAM_ID, PRODUCTION_ADAPTER_RELEASES};
use solana_account::Account;
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
use spl_token_interface::state::Mint as SplMint;

/// Core frame coordinates of the replay-initialization tail, from
/// `open_market.rs`'s own INITIALIZE_* constants.
const INITIALIZE_PAYER: usize = 11;
const INITIALIZE_REFUND: usize = 14;
/// `CoreSbfError::AccountFrame` and `CoreSbfError::Reference`.
const CORE_ACCOUNT_FRAME: u32 = 0x3001;
const CORE_REFERENCE: u32 = 0x3003;

const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xd1; 32]);
const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xd2; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xd3; 32]);
const RENT_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xd4; 32]);
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
    rent_credit: Pubkey,
    payer: Keypair,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OpeningSnapshot {
    market: Account,
    replay: Option<Account>,
    vault: Option<Account>,
    payer_lamports: u64,
    rent_credit_lamports: u64,
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
    let refund_wallet = RefundAuthority::new([0xd5; 32]).expect("refund wallet");
    let (rent_credit, rent_credit_bump) = Pubkey::find_program_address(
        &[
            LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
            market.as_ref(),
            &GENERATION.to_le_bytes(),
        ],
        &RENT_PROGRAM_ID,
    );
    let rent_credit_value = LifecycleRentCreditV2::new(
        refund_wallet,
        LifecycleAccountIdV2::new(market.to_bytes()).expect("Market identity"),
        LifecycleAccountIdV2::new(release_set).expect("release-set identity"),
        GENERATION,
        rent_credit_bump,
    )
    .expect("lifecycle RentCredit");
    add_account(
        &mut test,
        rent_credit,
        RENT_PROGRAM_ID,
        rent_credit_value.to_bytes().to_vec(),
    );
    let state = CoreState {
        phase: Phase::Founding,
        readiness: Readiness::Ready,
        terminal_winner: 0,
        identity,
        outstanding_capabilities: 0,
        principal_cap_sets: u64::MAX,
        rent_beneficiary: CoreIdentity::new(rent_credit.to_bytes()).expect("RentCredit"),
        terminal_receipt: None,
        bumps: StateBumpsV1::UNRECORDED,
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
        rent_credit,
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
            rent_credit,
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
    rent_refund: Pubkey,
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

fn core_instruction_with_coordinates(
    fixture: &Fixture,
    operation: OperationV1,
    request_payer: Pubkey,
    rent_refund: Pubkey,
    outer_payer: Pubkey,
) -> Instruction {
    let custody = custody_request(
        fixture.release_set,
        fixture.market,
        fixture.realm,
        fixture.mint,
        request_payer,
        rent_refund,
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
            AccountMeta::new(outer_payer, true),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
            AccountMeta::new(rent_refund, false),
        ]),
        OperationV1::OpenVault => accounts.extend([
            AccountMeta::new_readonly(fixture.mint, false),
            AccountMeta::new(fixture.vault, false),
            AccountMeta::new_readonly(fixture.authority, false),
            AccountMeta::new_readonly(Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID), false),
            AccountMeta::new(outer_payer, true),
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

fn core_instruction(fixture: &Fixture, operation: OperationV1) -> Instruction {
    core_instruction_with_coordinates(
        fixture,
        operation,
        fixture.payer.pubkey(),
        fixture.rent_credit,
        fixture.payer.pubkey(),
    )
}

/// Same-finalized Registry and Loader facts the continuation is derived from.
async fn continuation_state(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
) -> RegistryOpenMarketContinuationStateV1 {
    RegistryOpenMarketContinuationStateV1 {
        registry_program: required_observed(context, REGISTRY_PROGRAM_ID).await,
        activation_cache: required_observed(context, fixture.cache).await,
        core_program: required_observed(context, CORE_PROGRAM_ID).await,
        core_programdata: required_observed(context, fixture.core_programdata).await,
        custody_program: required_observed(context, CUSTODY_PROGRAM_ID).await,
        custody_programdata: required_observed(context, fixture.custody_programdata).await,
    }
}

/// Wrap one Core frame in the Registry continuation that alone can admit it.
///
/// `2dc53776` moved market opening behind the Registry: the last account of a
/// Core `OpenMarket` frame is an invocation-scoped admission PDA derived under
/// the Registry program, and Core requires it to be a SIGNER. Nothing outside
/// a Registry `invoke_signed` can produce that, so the route is not callable
/// top level and this file drove a frame the program had stopped accepting --
/// one account short, refused on length at 5,100 compute units, with every
/// hostile case in the test passing for that reason instead of its own.
///
/// The operator is the same one `crates/dclutch-svm-harness` uses and the same
/// one the successor bootstrap sends, so the frame under test is the frame
/// that ships rather than a second author's idea of it.
async fn continuation(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    operation: OperationV1,
) -> Instruction {
    let core = core_instruction(fixture, operation);
    let state = continuation_state(context, fixture).await;
    build_registry_open_market_continuation_v1(&state, &core)
        .expect("chain-derived Registry continuation over the Core frame")
        .instruction
}

/// Borrow one nested Core account meta out of the outer Registry frame.
///
/// The continuation carries a fixed Registry prefix and then the Core frame
/// verbatim, so a Core index addresses the same account in both.
fn nested_meta(instruction: &mut Instruction, core_index: usize) -> &mut AccountMeta {
    instruction
        .accounts
        .get_mut(REGISTRY_OPEN_MARKET_CONTINUATION_PREFIX_ACCOUNTS_V1 + core_index)
        .expect("nested Core account")
}

/// The exact custom refusal a submission produced, or `None` if it succeeded.
///
/// The four hostile cases here asserted only `is_err()` until 2026-08-30, and
/// every one of them had been passing on a frame-length refusal none of them
/// was about -- so the code is now named.
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

async fn required_observed(context: &mut ProgramTestContext, key: Pubkey) -> ObservedAccount {
    let account = context
        .banks_client
        .get_account(key)
        .await
        .expect("account query")
        .expect("required account exists");
    ObservedAccount {
        observation: Observation {
            slot: 1,
            unix_timestamp: 0,
            finality: Finality::Finalized,
        },
        key,
        owner: account.owner,
        lamports: account.lamports,
        executable: account.executable,
        data: account.data,
    }
}

async fn submit(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    instruction: Instruction,
    sponsor_signs: bool,
) -> Result<(), BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let transaction = if sponsor_signs {
        Transaction::new_signed_with_payer(
            &[instruction],
            Some(&context.payer.pubkey()),
            &[&context.payer, &fixture.payer],
            blockhash,
        )
    } else {
        Transaction::new_signed_with_payer(
            &[instruction],
            Some(&context.payer.pubkey()),
            &[&context.payer],
            blockhash,
        )
    };
    context.banks_client.process_transaction(transaction).await
}

async fn opening_snapshot(context: &mut ProgramTestContext, fixture: &Fixture) -> OpeningSnapshot {
    OpeningSnapshot {
        market: context
            .banks_client
            .get_account(fixture.market)
            .await
            .expect("Market query")
            .expect("Market"),
        replay: context
            .banks_client
            .get_account(fixture.replay)
            .await
            .expect("replay query"),
        vault: context
            .banks_client
            .get_account(fixture.vault)
            .await
            .expect("Vault query"),
        payer_lamports: context
            .banks_client
            .get_balance(fixture.payer.pubkey())
            .await
            .expect("payer balance"),
        rent_credit_lamports: context
            .banks_client
            .get_balance(fixture.rent_credit)
            .await
            .expect("RentCredit balance"),
    }
}

#[tokio::test]
async fn real_core_opens_exact_registry_realm_custody_and_commits_last() {
    let (test, fixture) = fixture();
    let mut context = test.start_with_context().await;
    let before_hostile = opening_snapshot(&mut context, &fixture).await;

    // Every hostile case below substitutes an ACCOUNT and leaves the request
    // bytes alone, because the continuation digest -- and therefore the
    // admission address -- covers the Core instruction's data. A hostility
    // that rewrote the data would move the admission and be refused for that
    // instead, which is a refusal about the test rather than about the route.
    let honest = continuation(&mut context, &fixture, OperationV1::InitializeReplay).await;

    let mut missing_signer = honest.clone();
    nested_meta(&mut missing_signer, INITIALIZE_PAYER).is_signer = false;
    assert_eq!(
        refusal(submit(&mut context, &fixture, missing_signer, false).await),
        Some(CORE_ACCOUNT_FRAME),
        "an unsigned sponsor is a frame refusal"
    );

    let mut read_only_sponsor = honest.clone();
    nested_meta(&mut read_only_sponsor, INITIALIZE_PAYER).is_writable = false;
    assert_eq!(
        refusal(submit(&mut context, &fixture, read_only_sponsor, true).await),
        Some(CORE_ACCOUNT_FRAME),
        "a read-only sponsor is a frame refusal"
    );

    // A writable non-signer, because the frame's own privilege conjuncts run
    // first: substituting a signer here refuses as a frame before the request
    // coordinate is ever compared, which is a different assertion.
    let mut substituted_refund = honest.clone();
    nested_meta(&mut substituted_refund, INITIALIZE_REFUND).pubkey = Pubkey::new_unique();
    assert_eq!(
        refusal(submit(&mut context, &fixture, substituted_refund, true).await),
        Some(CORE_REFERENCE),
        "the refund account must be the one the request names"
    );

    // The one hostility the honest builder will not even emit: a request whose
    // payer coordinate disagrees with the account the frame carries. Asserted
    // where it is refused rather than restated as a program refusal it never
    // reaches.
    let wrong_payer = core_instruction_with_coordinates(
        &fixture,
        OperationV1::InitializeReplay,
        context.payer.pubkey(),
        fixture.rent_credit,
        fixture.payer.pubkey(),
    );
    assert!(
        build_registry_open_market_continuation_v1(
            &continuation_state(&mut context, &fixture).await,
            &wrong_payer,
        )
        .is_err(),
        "a payer coordinate that disagrees with the frame is not admissible"
    );

    assert_eq!(
        opening_snapshot(&mut context, &fixture).await,
        before_hostile,
        "payer/refund/privilege refusals leave Market, Custody, sponsor, and RentCredit unchanged"
    );
    let payer_before = context
        .banks_client
        .get_balance(fixture.payer.pubkey())
        .await
        .expect("payer balance");
    for operation in [OperationV1::InitializeReplay, OperationV1::OpenVault] {
        let instruction = continuation(&mut context, &fixture, operation).await;
        submit(&mut context, &fixture, instruction, true)
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
    assert_eq!(replay.rent_refund, fixture.rent_credit.to_bytes());
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

// ---------------------------------------------------------------------------
// Decision 0017 §7's tripwire, on the FOUNDING continuation
// ---------------------------------------------------------------------------

/// One `Program <id> invoke [<depth>]` line, as the runtime writes it.
fn invoked_depths(logs: &[String], program: Pubkey) -> Vec<usize> {
    let prefix = format!("Program {program} invoke [");
    logs.iter()
        .filter_map(|line| line.strip_prefix(&prefix))
        .filter_map(|tail| tail.strip_suffix(']'))
        .filter_map(|depth| depth.parse().ok())
        .collect()
}

/// The depth the Registry occupies when it enters a continuation.
///
/// This constant is the whole of the reentrancy rule, and it is a fact about
/// the STACK rather than a threshold: the Registry has a live frame at one, so
/// ANY program running deeper that invokes it is reentering that frame and the
/// runtime kills the transaction with `ReentrancyNotAllowed`. It is not
/// "depth three or deeper" -- `registry_hot_continuation.rs` uses three
/// because in the Direct Hot topology Trading sits at two and the children it
/// walks sit at three. Here Core itself sits at two, and two is already past
/// the Registry, so Core is exposed to the wall exactly as its own children
/// are. Stating the rule as "deeper than the Registry" is what makes both
/// topologies instances of one thing.
const REGISTRY_CONTINUATION_DEPTH: usize = 1;

/// Submit one continuation and keep the runtime's own invoke log.
///
/// `submit` above throws the log away, which is fine for a test about account
/// state and useless for a test about the call stack.
async fn submit_observed(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    instruction: Instruction,
) -> Vec<String> {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer, &fixture.payer],
        blockhash,
    );
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("execution completed");
    let logs = processed
        .metadata
        .map_or_else(Vec::new, |metadata| metadata.log_messages);
    if let Err(error) = processed.result {
        panic!(
            "a child under a REAL Registry continuation must EXECUTE, and this \
             one refused with {error:?}. If that is ReentrancyNotAllowed, some \
             role program has re-acquired a RegistryInstructionV1 CPI and \
             decision 0017's wall is down -- read the invoke log for which \
             family died below the Registry. Logs: {logs:#?}"
        );
    }
    logs
}

/// Core and Custody run below a live Registry frame, and the market still opens.
///
/// # Which continuation this is, because it matters
///
/// This is the **founding** continuation -- the Registry-invoked Core
/// `OpenMarket` introduced by `2dc53776`, whose admission PDA is derived under
/// the Registry program and required by Core to be a SIGNER, so nothing but a
/// Registry `invoke_signed` can produce it. It is **load-bearing production**:
/// the 2026-08-30 decision packet's §4 ruling demoted the *Hot trade*
/// continuation to harness-only and carved this one out explicitly
/// (CORESTATE-3, *"the founding continuation is load-bearing since 2dc53776
/// and is untouched by this ruling"*).
///
/// That distinction is the reason this test exists at all.
/// `registry_hot_continuation.rs`'s dynamic tripwire rides the Direct Hot
/// bundle, which the same ruling demoted; this one rides the route markets are
/// actually founded through.
///
/// # What it catches
///
/// Decision 0017 ratified "children read the activation cache instead of
/// invoking the Registry", and §7 attached one condition to the ratification:
///
/// > a test that exercises a child under a real continuation for each family,
/// > so the wall has a tripwire and not only a comment.
///
/// The enforcement is subtractive -- 0017 §3, *"The illegal call is not
/// refused; it is unwriteable without re-adding an import"* -- so a
/// contributor who re-adds `RegistryInstructionV1::Reauthenticate` to Core or
/// Custody gets code that passes every top-level test and dies only here.
///
/// # The family gap this closes, stated against the record that opened it
///
/// CACHEREAD measured its own dynamic half at two families of five and named
/// the other three as unbuilt: *"Core is never invoked at all on the Direct
/// Hot route, nor are Dealer or Rent, which have no continuation fixture
/// anywhere in the tree"* (0017 §9). That is true of the Hot route and false
/// of the tree: this file has driven a real Registry continuation into Core
/// and Custody since `2dc53776`. What it never did was ASSERT the stack, so
/// the founding continuation was exercising the wall without anybody able to
/// point at a test that would go red when the wall fell. Core is now the third
/// family covered dynamically. Dealer and Rent remain uncovered and remain
/// sized in 0017 §9 rather than assumed.
///
/// # Why the depths are asserted and not assumed
///
/// Without the Registry-at-one assertion this case could pass on a transaction
/// that never entered through the continuation, which would make it a test
/// about market opening wearing a tripwire's name. The depths are read out of
/// the runtime's own log on this very execution.
#[tokio::test]
async fn core_and_custody_execute_as_children_under_the_founding_continuation() {
    let (test, fixture) = fixture();
    let mut context = test.start_with_context().await;

    for operation in [OperationV1::InitializeReplay, OperationV1::OpenVault] {
        let instruction = continuation(&mut context, &fixture, operation).await;
        let logs = submit_observed(&mut context, &fixture, instruction).await;

        assert_eq!(
            invoked_depths(&logs, REGISTRY_PROGRAM_ID),
            vec![REGISTRY_CONTINUATION_DEPTH],
            "{operation:?} must be a REAL continuation: the Registry enters at \
             depth one exactly once, and it is the live Registry frame that \
             makes a Registry CPI from below it reentrancy. Logs: {logs:#?}",
        );

        for (family, program) in [("Core", CORE_PROGRAM_ID), ("Custody", CUSTODY_PROGRAM_ID)] {
            let depths = invoked_depths(&logs, program);
            assert!(
                !depths.is_empty(),
                "{family} never executed on {operation:?}, so this transaction \
                 proves nothing about {family}'s wall. Logs: {logs:#?}",
            );
            assert!(
                depths
                    .iter()
                    .all(|depth| *depth > REGISTRY_CONTINUATION_DEPTH),
                "{family} ran at depths {depths:?} on {operation:?}, and a \
                 Registry CPI is only reentrancy from deeper than \
                 {REGISTRY_CONTINUATION_DEPTH}. This fixture has stopped \
                 exercising the wall.",
            );
        }
    }

    let market = context
        .banks_client
        .get_account(fixture.market)
        .await
        .expect("Market query")
        .expect("Market");
    let state = CoreState::decode(&market.data).expect("CoreState");
    assert_eq!(
        state.phase,
        Phase::Open,
        "the wall is only interesting if the continuation it guards completes",
    );
}

// ---------------------------------------------------------------------------
// TRUST_RATCHET_V1 §7.3's S-3 tripwire
// ---------------------------------------------------------------------------

/// `RegistryError::Record`, the one code every record-route refusal carries.
///
/// Because it is shared by every conjunct of `authenticate_begin`, this code
/// alone cannot say WHICH conjunct refused. That is what the control in
/// `a_finalized_record_refuses_re_begin_at_its_own_coordinate` is for.
const REGISTRY_RECORD: u32 = 0x100C;

/// The two record PDAs one `(schema, digest)` coordinate owns.
fn record_addresses(schema: [u8; 32], digest: [u8; 32]) -> (Pubkey, Pubkey) {
    (
        Pubkey::find_program_address(
            &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
            &REGISTRY_PROGRAM_ID,
        )
        .0,
        Pubkey::find_program_address(
            &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
            &REGISTRY_PROGRAM_ID,
        )
        .0,
    )
}

/// One canonical permissionless `Begin` at the coordinate `digest` names.
///
/// The frame is the six accounts `build_begin` in
/// `crates/dclutch-product-runtime-v2-operator/src/publication.rs` emits, in
/// its order, so this is the shipped Begin rather than a second author's idea
/// of one.
fn begin_instruction(
    fixture: &Fixture,
    digest: [u8; 32],
    exact_length: u64,
    clock_slot: u64,
) -> Instruction {
    let profile = CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1;
    let cursor_rent = Rent::default().minimum_balance(STAGING_CURSOR_BYTES_V1);
    let key = RecordKeyV1::new(
        SchemaReleaseId::new(REALM_SCHEMA_RELEASE_ID_V1).expect("Realm schema"),
        ContentDigest::new(digest).expect("content digest"),
    );
    let request = BeginRecordV1::new(
        key,
        exact_length,
        profile.page_envelope().expect("canonical page envelope"),
        profile
            .staging_liveness_policy(cursor_rent)
            .expect("canonical liveness policy")
            .policy_id(),
        clock_slot
            .checked_add(profile.maximum_staging_lifetime_slots())
            .expect("expiry slot"),
        cursor_rent,
    )
    .expect("canonical Begin request");
    let (raw, cursor) = record_addresses(REALM_SCHEMA_RELEASE_ID_V1, digest);
    Instruction {
        program_id: REGISTRY_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(fixture.payer.pubkey(), true),
            AccountMeta::new(raw, false),
            AccountMeta::new(cursor, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
            AccountMeta::new_readonly(sysvar::clock::ID, false),
        ],
        data: request.to_bytes().to_vec(),
    }
}

/// A finalized record cannot be re-staged at its own coordinate.
///
/// # What rests on this, and why it needed a test rather than a comment
///
/// `TRUST_RATCHET_V1.md` §7 works out that Trading's shipped capability seal
/// carries exactly one proposition about an account it does not hold in its
/// frame: *"at seal time, the canonical staging cursor for this
/// `(schema, digest)` was vacant and System-owned."* `borrow_sealed_record`
/// cannot re-check it, because the staging cursor is precisely what the seal
/// removed from the frame. The proposition is sound only while a finalized
/// record cannot be returned to a mid-build state, and §7.1 traces that to a
/// single line -- `require_prefunded_vacant(frame.raw)` at
/// `programs/dclutch-registry-sbf/src/record_v1.rs:342`:
///
/// > What stops a re-`Begin` at the same canonical key is `authenticate_begin`
/// > calling `require_prefunded_vacant(frame.raw)` [...] The cursor's own
/// > `require_prefunded_vacant` on the next line is satisfied by a finalized
/// > record and refuses nothing here.
///
/// That asymmetry is the whole point and it is why this test is not obvious.
/// After finalization the cursor IS vacant, so the check a reader would expect
/// to be load-bearing passes. The refusal comes from the raw account, and §9.3
/// asks for it to be tripwired because *"nobody has been told [it] is
/// load-bearing for anything but `Begin`'s own hygiene"* -- a future
/// record-reclamation route, which P-006's rent argument actively invites, is
/// written by someone reading that line as hygiene.
///
/// # §7.1 names one line; there are two, and building this measured it
///
/// Deleting `require_prefunded_vacant(frame.raw)` at `record_v1.rs:342` does
/// NOT make this re-`Begin` succeed. `process_begin` reaches
/// `create_or_allocate_prefunded_pda`, whose first act is
/// `if !is_prefunded_vacant(created) { return Err(record_error()) }`
/// (`record_v1.rs:886`) -- the SAME predicate, re-consulted at the point of
/// allocation. Measured on this fixture: the honest refusal costs **2,589 CU**,
/// and with `:342` deleted the route runs to **20,420 CU** and refuses at the
/// allocation guard instead, carrying the same `RegistryError::Record`.
///
/// So §7.1's *"what stops a re-`Begin` at the same canonical key is
/// `authenticate_begin` calling `require_prefunded_vacant(frame.raw)`"* is true
/// and incomplete: what stops it is the PREDICATE `is_prefunded_vacant` applied
/// to the raw account, at an authentication gate and again at the point of use.
/// That is a stronger position than the ratchet claims, and it is worth knowing
/// before someone writes a reclamation route believing one edit clears the way.
///
/// It also fixes what this test can honestly gate. A single deletion of `:342`
/// leaves it green, so `:342` alone is not what it watches; the shared
/// predicate is. Proved by mutation: `is_prefunded_vacant` stubbed to `true`
/// turns the refusal below from `Some(0x100C)` into `Some(0)` and this case
/// goes red.
///
/// # The control, and why the refusal is unreadable without it
///
/// Every conjunct of `authenticate_begin` refuses with the same
/// `RegistryError::Record`, so asserting the code proves the route said no and
/// not what it said no ABOUT. This file has been bitten by exactly that: its
/// four hostile cases spent four days passing on a frame-length refusal none
/// of them was about (`refusal`'s own doc comment, `67e96e5b`).
///
/// So the two submissions here differ in ONE field. Same schema, same exact
/// length, same envelope, same liveness policy, same expiry, same sponsor,
/// same frame shape -- only the digest differs, and the digest is what selects
/// which raw account the frame carries. The control names a coordinate nothing
/// has ever staged and must SUCCEED; the case names the fixture's finalized
/// Realm record and must refuse. A refusal that survived the control is a
/// refusal about raw-account vacancy, because nothing else moved.
///
/// # The boundary this does not cross
///
/// This is a tripwire on the condition, not on the sequence §7.2 describes.
/// No reclamation route exists to call, so no test can drive
/// reclaim-and-restage today. What goes red here is the day a finalized raw
/// account starts reading as vacant to the Registry -- which is the one thing
/// §7.3 says must be reasoned about, and precisely what a reclamation route
/// would have to arrange in order to work.
#[tokio::test]
async fn a_finalized_record_refuses_re_begin_at_its_own_coordinate() {
    let (test, fixture) = fixture();
    let mut context = test.start_with_context().await;

    // The fixture stands the Realm up in the post-finalization shape: the raw
    // record Registry-owned and exactly its content wide, the staging cursor
    // System-owned and empty. Read the width off the chain rather than
    // recomputing it, so the request describes the account that is there.
    let finalized = context
        .banks_client
        .get_account(fixture.realm_raw)
        .await
        .expect("Realm raw query")
        .expect("finalized Realm raw record");
    assert_eq!(
        finalized.owner, REGISTRY_PROGRAM_ID,
        "the case is only about a FINALIZED record, which is a Registry-owned \
         raw account",
    );
    let staging = context
        .banks_client
        .get_account(fixture.realm_staging)
        .await
        .expect("Realm staging query");
    assert!(
        staging
            .is_none_or(|account| account.owner == system_program::ID && account.data.is_empty()),
        "finalization destroys the cursor, so the conjunct a reader expects to \
         refuse here is satisfied -- that is what makes the raw check the \
         load-bearing one",
    );
    let exact_length = u64::try_from(finalized.data.len()).expect("record width");
    let clock_slot = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("Clock sysvar")
        .slot;

    // Control: the identical request at a coordinate nothing has staged.
    let unstaged = hash(b"a Realm this fixture has never published").to_bytes();
    assert_eq!(
        refusal(
            submit(
                &mut context,
                &fixture,
                begin_instruction(&fixture, unstaged, exact_length, clock_slot),
                true,
            )
            .await
        ),
        None,
        "the control must be ADMITTED, or the case below refuses for a reason \
         that has nothing to do with finalization and this test proves nothing",
    );

    // The case: the same request, aimed at the finalized record's coordinate.
    assert_eq!(
        refusal(
            submit(
                &mut context,
                &fixture,
                begin_instruction(&fixture, fixture.realm, exact_length, clock_slot),
                true,
            )
            .await
        ),
        Some(REGISTRY_RECORD),
        "a finalized record must refuse re-staging at its own coordinate. If \
         this went green, `require_prefunded_vacant(frame.raw)` no longer \
         refuses a finalized raw account, and every Trading capability seal \
         naming a record is now a durable claim about a state the chain can \
         leave -- TRUST_RATCHET_V1 §7.2's window is open.",
    );
}
