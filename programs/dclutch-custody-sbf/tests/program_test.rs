//! Real-ELF ProgramTest campaign for canonical multiprogram Custody.

use std::{env, fs, path::PathBuf, vec::Vec};

use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CallerRoleV1, CompartmentV1, ContextV1, CustodyAuthoritySeedsV1, CustodyReplaySeedsV1,
    CustodyReplayV1, CustodyRequestV1, CustodyVaultSeedsV1, OperationV1,
};
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_PDA_DOMAIN, RealmV1, RealmV1Input,
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
use dclutch_token_svm::{
    LEGACY_TOKEN_PROGRAM_ID, PRODUCTION_ADAPTER_RELEASES, TOKEN_2022_PROGRAM_ID, TokenAccount,
};
use solana_account::Account;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_option::COption;
use solana_program_pack::Pack;
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::Signer;
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_transaction::Transaction;
use spl_token_interface::state::{Account as SplAccount, AccountState, Mint as SplMint};

const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc1; 32]);
const CALLER_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc2; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc3; 32]);
const MARKET: [u8; 32] = [0x41; 32];
const CONTEXT: [u8; 32] = MARKET;
const ACTOR: [u8; 32] = [0x42; 32];
const RECIPIENT: [u8; 32] = [0x43; 32];
const GENERATION: u64 = 7;
const DEPOSIT: u64 = 100;

#[derive(Clone, Copy, Debug)]
enum Profile {
    Legacy,
    Token2022,
}

impl Profile {
    const fn token_program(self) -> Pubkey {
        match self {
            Self::Legacy => Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID),
            Self::Token2022 => Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID),
        }
    }

    const fn release_index(self) -> usize {
        match self {
            Self::Legacy => 0,
            Self::Token2022 => 1,
        }
    }
}

struct Artifacts {
    custody: Vec<u8>,
    caller: Vec<u8>,
    registry: Vec<u8>,
}

struct Fixture {
    profile: Profile,
    release_set: [u8; 32],
    realm: [u8; 32],
    realm_key: Pubkey,
    activation_cache: Pubkey,
    caller_programdata: Pubkey,
    mint: Pubkey,
    replay: Pubkey,
    custody_authority: Pubkey,
    vault: Pubkey,
    external_source: Pubkey,
    external_destination: Pubkey,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Snapshot {
    replay: Account,
    vault: Account,
    source: Account,
    destination: Account,
}

fn artifacts() -> Artifacts {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    let custody_path = directory.join("dclutch_custody_sbf.so");
    let caller_path = directory.join("dclutch_custody_test_caller_sbf.so");
    let registry_path = directory.join("dclutch_registry_sbf.so");
    assert!(custody_path.is_file(), "missing real Custody ELF");
    assert!(caller_path.is_file(), "missing real test-caller ELF");
    assert!(registry_path.is_file(), "missing real Registry ELF");
    Artifacts {
        custody: fs::read(custody_path).expect("read Custody ELF"),
        caller: fs::read(caller_path).expect("read caller ELF"),
        registry: fs::read(registry_path).expect("read Registry ELF"),
    }
}

fn identity(key: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(key.to_bytes()).expect("nonzero program identity")
}

fn programdata_address(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn immutable_programdata(elf: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 45 + elf.len()];
    bytes
        .get_mut(0..4)
        .expect("loader state discriminator")
        .copy_from_slice(&3_u32.to_le_bytes());
    bytes
        .get_mut(4..12)
        .expect("loader deployment slot")
        .copy_from_slice(&0_u64.to_le_bytes());
    *bytes.get_mut(12).expect("loader upgrade authority option") = 0;
    bytes
        .get_mut(45..)
        .expect("loader program bytes")
        .copy_from_slice(elf);
    bytes
}

fn add_upgradeable_program(
    test: &mut ProgramTest,
    name: &'static str,
    program: Pubkey,
    elf: &[u8],
) {
    test.add_upgradeable_program_to_genesis(name, &program);
    let programdata = programdata_address(program);
    let data = immutable_programdata(elf);
    test.add_account(
        programdata,
        Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner: bpf_loader_upgradeable::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn release(program: Pubkey, semantic_seed: u8, elf: &[u8]) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        identity(program),
        identity(bpf_loader_upgradeable::ID),
        programdata_address(program).to_bytes(),
        ContentId::new([semantic_seed; 32]).expect("semantic release"),
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
    let observation = DeploymentObservationV1::new(
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
    .expect("deployment observation");
    ArtifactActivationInputV1::new(artifact_id(release), release, observation)
}

fn activation_cache(
    registry: ArtifactReleaseV1,
    caller: ArtifactReleaseV1,
    custody: ArtifactReleaseV1,
) -> ([u8; 32], Vec<u8>) {
    let caller_binding = binding(caller);
    let release_set = ExecutionReleaseSetV1::new(
        binding(registry),
        caller_binding,
        caller_binding,
        caller_binding,
        binding(custody),
    )
    .expect("release set");
    let release_set_id = hash(&release_set.to_bytes()).to_bytes();
    let content = ContentId::new(release_set_id).expect("release-set ID");
    let mut bytes = vec![0; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut bytes, content).expect("initialize cache");
    for (role, release) in [
        (ExecutionRoleV1::Core, registry),
        (ExecutionRoleV1::Claims, caller),
        (ExecutionRoleV1::Trading, caller),
        (ExecutionRoleV1::Resolution, caller),
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
    ActivatedExecutionReleaseSetV1::decode(&bytes).expect("complete activation cache");
    (release_set_id, bytes)
}

fn mint_data() -> Vec<u8> {
    let mut bytes = vec![0; SplMint::LEN];
    SplMint::pack(
        SplMint {
            mint_authority: COption::None,
            supply: 1_005,
            decimals: 6,
            is_initialized: true,
            freeze_authority: COption::None,
        },
        &mut bytes,
    )
    .expect("pack Mint");
    bytes
}

fn token_account_data(
    mint: Pubkey,
    owner: Pubkey,
    amount: u64,
    delegate: Option<Pubkey>,
    delegated_amount: u64,
) -> Vec<u8> {
    let mut bytes = vec![0; SplAccount::LEN];
    SplAccount::pack(
        SplAccount {
            mint,
            owner,
            amount,
            delegate: delegate.map_or(COption::None, COption::Some),
            state: AccountState::Initialized,
            is_native: COption::None,
            delegated_amount,
            close_authority: COption::None,
        },
        &mut bytes,
    )
    .expect("pack Account");
    bytes
}

fn add_protocol_account(test: &mut ProgramTest, key: Pubkey, owner: Pubkey, data: Vec<u8>) {
    test.add_account(
        key,
        Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn fixture(profile: Profile) -> (ProgramTest, Fixture) {
    let artifacts = artifacts();
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    add_upgradeable_program(
        &mut test,
        "dclutch_custody_sbf",
        CUSTODY_PROGRAM_ID,
        &artifacts.custody,
    );
    add_upgradeable_program(
        &mut test,
        "dclutch_custody_test_caller_sbf",
        CALLER_PROGRAM_ID,
        &artifacts.caller,
    );
    add_upgradeable_program(
        &mut test,
        "dclutch_registry_sbf",
        REGISTRY_PROGRAM_ID,
        &artifacts.registry,
    );

    let caller_release = release(CALLER_PROGRAM_ID, 0x51, &artifacts.caller);
    let custody_release = release(CUSTODY_PROGRAM_ID, 0x52, &artifacts.custody);
    let registry_release = release(REGISTRY_PROGRAM_ID, 0x53, &artifacts.registry);
    let (release_set, cache_data) =
        activation_cache(registry_release, caller_release, custody_release);
    let activation_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    add_protocol_account(&mut test, activation_cache, REGISTRY_PROGRAM_ID, cache_data);

    let mint = Pubkey::new_unique();
    let token_program = profile.token_program();
    let adapter = *PRODUCTION_ADAPTER_RELEASES
        .get(profile.release_index())
        .expect("supported adapter release");
    let realm_value = RealmV1::new(RealmV1Input {
        token_program: token_program.to_bytes(),
        collateral_mint: mint.to_bytes(),
        collateral_adapter_release_id: hash(&adapter.to_bytes()).to_bytes(),
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    })
    .expect("Realm");
    let realm_data = realm_value.to_bytes().to_vec();
    let realm = hash(&realm_data).to_bytes();
    let realm_key =
        Pubkey::find_program_address(&[REALM_PDA_DOMAIN, &realm], &REGISTRY_PROGRAM_ID).0;
    add_protocol_account(&mut test, realm_key, REGISTRY_PROGRAM_ID, realm_data);
    add_protocol_account(&mut test, mint, token_program, mint_data());

    let base_request = request_base(profile, release_set, realm, mint);
    let replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::from_request(base_request).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let custody_authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::from_request(base_request).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let mut vault_request = base_request;
    vault_request.destination_compartment = CompartmentV1::HoardPrincipal;
    vault_request.destination_vault_context = CONTEXT;
    let vault = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::from_request(vault_request, false).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let external_source = Pubkey::new_unique();
    let external_destination = Pubkey::new_unique();
    add_protocol_account(
        &mut test,
        external_source,
        token_program,
        token_account_data(
            mint,
            Pubkey::new_from_array(ACTOR),
            1_000,
            Some(custody_authority),
            1_000,
        ),
    );
    add_protocol_account(
        &mut test,
        external_destination,
        token_program,
        token_account_data(mint, Pubkey::new_from_array(RECIPIENT), 5, None, 0),
    );
    (
        test,
        Fixture {
            profile,
            release_set,
            realm,
            realm_key,
            activation_cache,
            caller_programdata: programdata_address(CALLER_PROGRAM_ID),
            mint,
            replay,
            custody_authority,
            vault,
            external_source,
            external_destination,
        },
    )
}

fn semantic(tag: u8) -> ContextV1 {
    ContextV1 {
        candidate: [0x61; 32],
        source_owner: [0; 32],
        destination_owner: [0; 32],
        order: [tag; 32],
        parent_request_digest: [tag.wrapping_add(1); 32],
        order_nonce: u64::from(tag),
        generation: GENERATION,
        page_index: 2,
        execution_index: 3,
        transfer_index: u16::from(tag),
    }
}

fn request_base(
    profile: Profile,
    release_set: [u8; 32],
    realm: [u8; 32],
    mint: Pubkey,
) -> CustodyRequestV1 {
    CustodyRequestV1 {
        operation: OperationV1::InitializeReplay,
        caller_role: CallerRoleV1::Trading,
        source_compartment: CompartmentV1::None,
        destination_compartment: CompartmentV1::None,
        release_set,
        market: MARKET,
        realm,
        context: CONTEXT,
        caller_program: CALLER_PROGRAM_ID.to_bytes(),
        semantic: semantic(1),
        source: [0; 32],
        destination: [0; 32],
        source_vault_context: [0; 32],
        destination_vault_context: [0; 32],
        mint: mint.to_bytes(),
        token_program: profile.token_program().to_bytes(),
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision: 0,
        resulting_revision: 1,
        amount: 0,
        rent_lamports: 0,
    }
}

fn initialize_request(fixture: &Fixture, payer: Pubkey) -> CustodyRequestV1 {
    let mut request = request_base(
        fixture.profile,
        fixture.release_set,
        fixture.realm,
        fixture.mint,
    );
    request.payer = payer.to_bytes();
    request.rent_refund = payer.to_bytes();
    request.mint = [0; 32];
    request.token_program = [0; 32];
    request.rent_lamports =
        Rent::default().minimum_balance(dclutch_custody_contract::CUSTODY_REPLAY_BYTES_V1);
    request
}

fn open_request(fixture: &Fixture, payer: Pubkey) -> CustodyRequestV1 {
    let mut request = request_base(
        fixture.profile,
        fixture.release_set,
        fixture.realm,
        fixture.mint,
    );
    request.operation = OperationV1::OpenVault;
    request.destination_compartment = CompartmentV1::HoardPrincipal;
    request.destination = fixture.vault.to_bytes();
    request.destination_vault_context = CONTEXT;
    request.payer = payer.to_bytes();
    request.rent_refund = payer.to_bytes();
    request.expected_revision = 1;
    request.resulting_revision = 2;
    request.rent_lamports = Rent::default().minimum_balance(dclutch_token_svm::ACCOUNT_BYTES);
    request.semantic = semantic(2);
    request
}

fn deposit_request(fixture: &Fixture, expected_revision: u64, amount: u64) -> CustodyRequestV1 {
    let mut request = request_base(
        fixture.profile,
        fixture.release_set,
        fixture.realm,
        fixture.mint,
    );
    request.operation = OperationV1::Transfer;
    request.source_compartment = CompartmentV1::External;
    request.destination_compartment = CompartmentV1::HoardPrincipal;
    request.source = fixture.external_source.to_bytes();
    request.destination = fixture.vault.to_bytes();
    request.destination_vault_context = CONTEXT;
    request.expected_revision = expected_revision;
    request.resulting_revision = expected_revision + 1;
    request.amount = amount;
    request.semantic = semantic(3);
    request.semantic.source_owner = ACTOR;
    request
}

fn external_transfer_request(fixture: &Fixture, expected_revision: u64) -> CustodyRequestV1 {
    let mut request = deposit_request(fixture, expected_revision, 5);
    request.destination_compartment = CompartmentV1::External;
    request.destination = fixture.external_destination.to_bytes();
    request.destination_vault_context = [0; 32];
    request.semantic = semantic(3);
    request.semantic.source_owner = ACTOR;
    request.semantic.destination_owner = RECIPIENT;
    request
}

fn withdraw_request(fixture: &Fixture, expected_revision: u64) -> CustodyRequestV1 {
    let mut request = deposit_request(fixture, expected_revision, DEPOSIT);
    request.source_compartment = CompartmentV1::HoardPrincipal;
    request.destination_compartment = CompartmentV1::External;
    request.source = fixture.vault.to_bytes();
    request.destination = fixture.external_destination.to_bytes();
    request.source_vault_context = CONTEXT;
    request.destination_vault_context = [0; 32];
    request.semantic = semantic(4);
    request.semantic.destination_owner = RECIPIENT;
    request
}

fn close_request(fixture: &Fixture, payer: Pubkey) -> CustodyRequestV1 {
    let mut request = request_base(
        fixture.profile,
        fixture.release_set,
        fixture.realm,
        fixture.mint,
    );
    request.operation = OperationV1::CloseVault;
    request.source_compartment = CompartmentV1::HoardPrincipal;
    request.source = fixture.vault.to_bytes();
    request.source_vault_context = CONTEXT;
    request.rent_refund = payer.to_bytes();
    request.expected_revision = 5;
    request.resulting_revision = 6;
    request.rent_lamports = Rent::default().minimum_balance(dclutch_token_svm::ACCOUNT_BYTES);
    request.semantic = semantic(5);
    request
}

fn common_metas(fixture: &Fixture, request: CustodyRequestV1) -> Vec<AccountMeta> {
    let request_bytes = request.to_bytes().expect("canonical request");
    let digest = hash(&request_bytes).to_bytes();
    let authority = Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::new(
            ContentId::new(request.release_set).expect("release set"),
            request.market,
            request.caller_role,
            request.context,
            digest,
        )
        .expect("caller seeds")
        .as_slices(),
        &CALLER_PROGRAM_ID,
    )
    .0;
    vec![
        AccountMeta::new_readonly(authority, false),
        AccountMeta::new_readonly(fixture.activation_cache, false),
        AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
        AccountMeta::new_readonly(CALLER_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.caller_programdata, false),
        AccountMeta::new_readonly(fixture.realm_key, false),
        AccountMeta::new(fixture.replay, false),
    ]
}

fn wrapper_instruction(
    fixture: &Fixture,
    request: CustodyRequestV1,
    payer: Pubkey,
    fail_after: bool,
) -> Instruction {
    let mut accounts = vec![AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false)];
    accounts.extend(common_metas(fixture, request));
    match request.operation {
        OperationV1::InitializeReplay => accounts.extend([
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ]),
        OperationV1::OpenVault => accounts.extend([
            AccountMeta::new_readonly(fixture.mint, false),
            AccountMeta::new(fixture.vault, false),
            AccountMeta::new_readonly(fixture.custody_authority, false),
            AccountMeta::new_readonly(fixture.profile.token_program(), false),
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ]),
        OperationV1::Transfer => accounts.extend([
            AccountMeta::new_readonly(fixture.mint, false),
            AccountMeta::new(Pubkey::new_from_array(request.source), false),
            AccountMeta::new(Pubkey::new_from_array(request.destination), false),
            AccountMeta::new_readonly(fixture.custody_authority, false),
            AccountMeta::new_readonly(fixture.profile.token_program(), false),
        ]),
        OperationV1::CloseVault => accounts.extend([
            AccountMeta::new_readonly(fixture.mint, false),
            AccountMeta::new(fixture.vault, false),
            AccountMeta::new_readonly(fixture.custody_authority, false),
            AccountMeta::new_readonly(fixture.profile.token_program(), false),
            AccountMeta::new(payer, false),
        ]),
    }
    let mut data = Vec::with_capacity(dclutch_custody_contract::CUSTODY_REQUEST_BYTES_V1 + 1);
    data.push(u8::from(fail_after));
    data.extend_from_slice(&request.to_bytes().expect("request bytes"));
    Instruction {
        program_id: CALLER_PROGRAM_ID,
        accounts,
        data,
    }
}

async fn submit(
    context: &mut ProgramTestContext,
    instruction: Instruction,
) -> Result<(bool, u64), BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await?;
    let units = processed
        .metadata
        .ok_or(BanksClientError::ClientError("missing metadata"))?
        .compute_units_consumed;
    Ok((processed.result.is_ok(), units))
}

async fn observed(context: &mut ProgramTestContext, key: Pubkey) -> Account {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account query")
        .expect("existing account")
}

async fn snapshot(context: &mut ProgramTestContext, fixture: &Fixture) -> Snapshot {
    Snapshot {
        replay: observed(context, fixture.replay).await,
        vault: observed(context, fixture.vault).await,
        source: observed(context, fixture.external_source).await,
        destination: observed(context, fixture.external_destination).await,
    }
}

fn token_amount(account: &Account) -> u64 {
    TokenAccount::parse(&account.data)
        .expect("token state")
        .amount
}

async fn campaign(profile: Profile) {
    let (test, fixture) = fixture(profile);
    let mut context = test.start_with_context().await;
    let payer = context.payer.pubkey();

    let (accepted, initialize_cu) = submit(
        &mut context,
        wrapper_instruction(&fixture, initialize_request(&fixture, payer), payer, false),
    )
    .await
    .expect("initialize transaction");
    assert!(accepted, "initialize replay");

    let (accepted, open_cu) = submit(
        &mut context,
        wrapper_instruction(&fixture, open_request(&fixture, payer), payer, false),
    )
    .await
    .expect("open transaction");
    assert!(accepted, "open vault");

    let (accepted, external_cu) = submit(
        &mut context,
        wrapper_instruction(
            &fixture,
            external_transfer_request(&fixture, 2),
            payer,
            false,
        ),
    )
    .await
    .expect("distinct-owner external transfer");
    assert!(accepted, "distinct-owner external transfer");
    let after_external = snapshot(&mut context, &fixture).await;
    assert_eq!(token_amount(&after_external.source), 995);
    assert_eq!(token_amount(&after_external.destination), 10);

    let mut wrong_delegate = external_transfer_request(&fixture, 3);
    wrong_delegate.source = fixture.external_destination.to_bytes();
    wrong_delegate.destination = fixture.external_source.to_bytes();
    wrong_delegate.semantic.source_owner = RECIPIENT;
    wrong_delegate.semantic.destination_owner = ACTOR;
    wrong_delegate.amount = 1;
    let (accepted, delegate_cu) = submit(
        &mut context,
        wrapper_instruction(&fixture, wrong_delegate, payer, false),
    )
    .await
    .expect("delegate-substitution transaction");
    assert!(
        !accepted,
        "external source without exact delegate must refuse"
    );
    assert_eq!(snapshot(&mut context, &fixture).await, after_external);

    let (accepted, deposit_cu) = submit(
        &mut context,
        wrapper_instruction(
            &fixture,
            deposit_request(&fixture, 3, DEPOSIT),
            payer,
            false,
        ),
    )
    .await
    .expect("deposit transaction");
    assert!(accepted, "deposit");
    let after_deposit = snapshot(&mut context, &fixture).await;
    assert_eq!(token_amount(&after_deposit.source), 895);
    assert_eq!(token_amount(&after_deposit.vault), DEPOSIT);
    assert_eq!(
        CustodyReplayV1::decode(&after_deposit.replay.data)
            .expect("replay")
            .next_revision,
        4
    );

    let before_late_failure = after_deposit.clone();
    let (accepted, late_failure_cu) = submit(
        &mut context,
        wrapper_instruction(&fixture, deposit_request(&fixture, 4, 7), payer, true),
    )
    .await
    .expect("late-failure transaction");
    assert!(!accepted, "caller late failure must refuse");
    assert_eq!(
        snapshot(&mut context, &fixture).await,
        before_late_failure,
        "token CPI and replay commit must roll back together"
    );

    let stale = deposit_request(&fixture, 3, 1);
    let (accepted, stale_cu) = submit(
        &mut context,
        wrapper_instruction(&fixture, stale, payer, false),
    )
    .await
    .expect("stale replay transaction");
    assert!(!accepted, "stale replay must refuse");
    assert_eq!(snapshot(&mut context, &fixture).await, before_late_failure);

    let mut substituted_source_owner = deposit_request(&fixture, 4, 1);
    substituted_source_owner.semantic.source_owner = [0x99; 32];
    let (accepted, source_owner_cu) = submit(
        &mut context,
        wrapper_instruction(&fixture, substituted_source_owner, payer, false),
    )
    .await
    .expect("source-owner-substitution transaction");
    assert!(!accepted, "external source-owner substitution must refuse");
    assert_eq!(snapshot(&mut context, &fixture).await, before_late_failure);

    let mut substituted_destination_owner = external_transfer_request(&fixture, 4);
    substituted_destination_owner.semantic.destination_owner = [0x99; 32];
    let (accepted, destination_owner_cu) = submit(
        &mut context,
        wrapper_instruction(&fixture, substituted_destination_owner, payer, false),
    )
    .await
    .expect("destination-owner-substitution transaction");
    assert!(
        !accepted,
        "external destination-owner substitution must refuse"
    );
    assert_eq!(snapshot(&mut context, &fixture).await, before_late_failure);

    let (accepted, withdraw_cu) = submit(
        &mut context,
        wrapper_instruction(&fixture, withdraw_request(&fixture, 4), payer, false),
    )
    .await
    .expect("withdraw transaction");
    assert!(accepted, "withdraw");
    let after_withdraw = snapshot(&mut context, &fixture).await;
    assert_eq!(token_amount(&after_withdraw.source), 895);
    assert_eq!(token_amount(&after_withdraw.vault), 0);
    assert_eq!(token_amount(&after_withdraw.destination), 110);

    let (accepted, close_cu) = submit(
        &mut context,
        wrapper_instruction(&fixture, close_request(&fixture, payer), payer, false),
    )
    .await
    .expect("close transaction");
    assert!(accepted, "close vault");
    assert!(
        context
            .banks_client
            .get_account(fixture.vault)
            .await
            .expect("closed-vault query")
            .is_none(),
        "closed vault must be reclaimed"
    );

    eprintln!(
        "Custody {profile:?} CU: initialize={initialize_cu}, open={open_cu}, external={external_cu}, delegate-refusal={delegate_cu}, deposit={deposit_cu}, late-rollback={late_failure_cu}, stale={stale_cu}, source-owner={source_owner_cu}, destination-owner={destination_owner_cu}, withdraw={withdraw_cu}, close={close_cu}"
    );
}

#[tokio::test]
async fn real_elf_legacy_custody_is_atomic_replay_safe_and_owner_bound() {
    campaign(Profile::Legacy).await;
}

#[tokio::test]
async fn real_elf_token_2022_custody_is_atomic_replay_safe_and_owner_bound() {
    campaign(Profile::Token2022).await;
}
