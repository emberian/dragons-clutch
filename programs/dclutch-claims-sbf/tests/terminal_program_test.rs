//! Real-ELF ProgramTest campaign for terminal representation redemption.
//!
//! Production Claims burns permissioned Token-2022 receipts, mutates the sole
//! Claims economic owner, and invokes canonical Custody for the collateral
//! payout. A real SBF wrapper then deliberately fails after that complete CPI
//! graph returns to prove transaction rollback across every mutable owner.

use std::{env, fs, path::PathBuf, vec::Vec};

use dclutch_claims_representation_codec::{
    ActionV1, ClaimsReleaseAdmission, DescriptorV1, EconomicPhase as RepresentationEconomicPhase,
    StateV1, prepare,
};
use dclutch_claims_svm::{ClaimsAggregateSeedsV1, ClaimsPositionSeedsV1};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CallerRoleV1, CompartmentV1, ContextV1, CustodyAuthoritySeedsV1, CustodyReplaySeedsV1,
    CustodyReplayV1, CustodyRequestV1, CustodyVaultSeedsV1, OperationV1,
};
use dclutch_economic_slice_kernel::{
    BasketAction, BasketFrame, MARKET_HEADER_BYTES, POSITION_HEADER_BYTES, Phase as EconomicPhase,
    SCALAR_BYTES, execute_basket, initialize_market, initialize_position, market_phase,
    market_revision, position_materialized, position_revision,
};
use dclutch_market_core_codec::{
    CoreState, Identity, MarketCoreStateSeedsV1, MarketIdentity, Phase as CorePhase, Readiness,
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
use dclutch_token_svm::{PRODUCTION_ADAPTER_RELEASES, TOKEN_2022_PROGRAM_ID, TokenAccount};
use solana_account::Account;
use solana_address_lookup_table_interface::instruction::{
    create_lookup_table, extend_lookup_table,
};
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_program::{
    clock::Clock,
    hash::{Hash, hash},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_option::COption;
use solana_program_pack::Pack;
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program};
use solana_transaction::{Transaction, versioned::VersionedTransaction};
use spl_token_2022_interface::extension::ExtensionType;
use spl_token_interface::state::{Account as SplAccount, AccountState, Mint as SplMint};

const CLAIMS_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xd1; 32]);
const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xd2; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xd3; 32]);
const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xd4; 32]);
const TEST_CALLER_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xd5; 32]);
const GENERATION: u64 = 19;
const OUTCOME_COUNT: u32 = 2;
const WINNER: u32 = 1;
const CLAIM_ATOMS: [u64; 2] = [3, 3];
const TERMINAL_PAYOUT: u64 = 3;
const RECEIPT_UNITS_PER_LOT: u64 = 10;
const INITIAL_RECIPIENT_ATOMS: u64 = 5;
const CUSTODY_EXPECTED_REVISION: u64 = 8;
const DESCRIPTOR_HEADER_BYTES: usize = 224;
const REPRESENTATION_MINT_BYTES: usize = 238;
const TERMINAL_ACCOUNT_METAS: usize = 26;
const PACKET_LIMIT: usize = 1_232;
const TOKEN_2022_V11_ELF_DIGEST: [u8; 32] = [
    0x49, 0x5e, 0x9d, 0x76, 0x80, 0xdd, 0x55, 0x5c, 0xb1, 0x26, 0xa6, 0xa8, 0xe5, 0x46, 0x4a, 0xf5,
    0xbe, 0x9b, 0x01, 0xf0, 0x2f, 0x2c, 0xd7, 0x06, 0x34, 0x35, 0x27, 0x22, 0xd2, 0x2e, 0x3c, 0xad,
];

struct Artifacts {
    claims: Vec<u8>,
    custody: Vec<u8>,
    registry: Vec<u8>,
    core: Vec<u8>,
    token_2022: Vec<u8>,
    test_caller: Vec<u8>,
}

struct Fixture {
    claimant: Keypair,
    descriptor: Pubkey,
    state: Pubkey,
    market: Pubkey,
    claimant_position: Pubkey,
    wrapper_position: Pubkey,
    activation_cache: Pubkey,
    claims_programdata: Pubkey,
    core_market: Pubkey,
    core_programdata: Pubkey,
    custody_programdata: Pubkey,
    realm_key: Pubkey,
    receipt_mint: Pubkey,
    holder: Pubkey,
    collateral_mint: Pubkey,
    replay: Pubkey,
    hoard: Pubkey,
    recipient: Pubkey,
    custody_authority: Pubkey,
    custody_request: CustodyRequestV1,
    action: ActionV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Snapshot {
    state: Account,
    market: Account,
    claimant_position: Account,
    wrapper_position: Account,
    receipt_mint: Account,
    holder: Account,
    replay: Account,
    hoard: Account,
    recipient: Account,
}

struct Submission {
    accepted: bool,
    compute_units: u64,
    wire_bytes: usize,
    logs: Vec<String>,
}

fn artifacts() -> Artifacts {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    let read = |name: &str| {
        let path = directory.join(name);
        assert!(path.is_file(), "missing real ELF: {}", path.display());
        fs::read(path).expect("read real ELF")
    };
    let token_2022 = read("spl_token_2022.so");
    assert_eq!(
        hash(&token_2022).to_bytes(),
        TOKEN_2022_V11_ELF_DIGEST,
        "the real Token-2022 v11 PermissionedBurn runtime is required"
    );
    Artifacts {
        claims: read("dclutch_claims_sbf.so"),
        custody: read("dclutch_custody_sbf.so"),
        registry: read("dclutch_registry_sbf.so"),
        core: read("dclutch_core_sbf.so"),
        token_2022,
        test_caller: read("dclutch_claims_terminal_test_caller_sbf.so"),
    }
}

fn identity(key: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(key.to_bytes()).expect("nonzero program identity")
}

fn semantic_identity(value: [u8; 32]) -> Identity {
    Identity::new(value).expect("nonzero semantic identity")
}

fn programdata_address(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn immutable_programdata(elf: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 45 + elf.len()];
    put(&mut bytes, 0, &3_u32.to_le_bytes());
    put(&mut bytes, 4, &0_u64.to_le_bytes());
    *bytes.get_mut(12).expect("ProgramData authority option") = 0;
    put(&mut bytes, 45, elf);
    bytes
}

fn put(output: &mut [u8], offset: usize, input: &[u8]) {
    let end = offset.checked_add(input.len()).expect("fixture offset");
    output
        .get_mut(offset..end)
        .expect("fixture field")
        .copy_from_slice(input);
}

fn add_upgradeable_program(
    test: &mut ProgramTest,
    name: &'static str,
    program: Pubkey,
    elf: &[u8],
) {
    test.add_upgradeable_program_to_genesis(name, &program);
    let data = immutable_programdata(elf);
    add_account(
        test,
        programdata_address(program),
        bpf_loader_upgradeable::ID,
        data,
    );
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

fn activation_cache(artifacts: &Artifacts) -> ([u8; 32], Vec<u8>) {
    let core = release(CORE_PROGRAM_ID, 0x52, &artifacts.core);
    let claims = release(CLAIMS_PROGRAM_ID, 0x53, &artifacts.claims);
    let custody = release(CUSTODY_PROGRAM_ID, 0x54, &artifacts.custody);
    let release_set = ExecutionReleaseSetV1::new(
        binding(core),
        binding(claims),
        binding(claims),
        binding(claims),
        binding(custody),
    )
    .expect("release set");
    let release_set_id = hash(&release_set.to_bytes()).to_bytes();
    let content = ContentId::new(release_set_id).expect("release-set ID");
    let mut bytes = vec![0; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut bytes, content).expect("initialize cache");
    for (role, artifact) in [
        (ExecutionRoleV1::Core, core),
        (ExecutionRoleV1::Claims, claims),
        (ExecutionRoleV1::Trading, claims),
        (ExecutionRoleV1::Resolution, claims),
        (ExecutionRoleV1::Custody, custody),
    ] {
        activate_execution_role_into_v1(
            &mut bytes,
            content,
            &release_set,
            role,
            &activation_input(artifact),
        )
        .expect("activate role");
    }
    ActivatedExecutionReleaseSetV1::decode(&bytes).expect("complete cache");
    (release_set_id, bytes)
}

fn descriptor_data(
    descriptor: Pubkey,
    market: Pubkey,
    product: [u8; 32],
    result_domain: [u8; 32],
    receipt_mint: Pubkey,
    release_set: [u8; 32],
) -> Vec<u8> {
    let mut bytes = vec![0_u8; DESCRIPTOR_HEADER_BYTES + CLAIM_ATOMS.len() * 8];
    put(&mut bytes, 0, b"DCLWRPD1");
    put(&mut bytes, 8, &1_u16.to_le_bytes());
    put(&mut bytes, 16, descriptor.as_ref());
    put(&mut bytes, 48, market.as_ref());
    put(&mut bytes, 80, &product);
    put(&mut bytes, 112, &result_domain);
    put(&mut bytes, 144, receipt_mint.as_ref());
    put(&mut bytes, 176, &release_set);
    put(&mut bytes, 208, &OUTCOME_COUNT.to_le_bytes());
    put(&mut bytes, 216, &RECEIPT_UNITS_PER_LOT.to_le_bytes());
    for (index, quantity) in CLAIM_ATOMS.into_iter().enumerate() {
        let start = DESCRIPTOR_HEADER_BYTES + index * 8;
        put(&mut bytes, start, &quantity.to_le_bytes());
    }
    DescriptorV1::decode(&bytes).expect("canonical descriptor");
    bytes
}

fn base_mint_data(
    mint_authority: COption<Pubkey>,
    supply: u64,
    decimals: u8,
) -> [u8; SplMint::LEN] {
    let mut bytes = [0_u8; SplMint::LEN];
    SplMint::pack(
        SplMint {
            mint_authority,
            supply,
            decimals,
            is_initialized: true,
            freeze_authority: COption::None,
        },
        &mut bytes,
    )
    .expect("pack Mint");
    bytes
}

fn representation_mint_data(state: Pubkey) -> Vec<u8> {
    let mut bytes = vec![0_u8; REPRESENTATION_MINT_BYTES];
    put(
        &mut bytes,
        0,
        &base_mint_data(COption::Some(state), RECEIPT_UNITS_PER_LOT, 0),
    );
    *bytes.get_mut(165).expect("Mint account type") = 1;
    put(
        &mut bytes,
        166,
        &(ExtensionType::MintCloseAuthority as u16).to_le_bytes(),
    );
    put(&mut bytes, 168, &32_u16.to_le_bytes());
    put(&mut bytes, 170, state.as_ref());
    put(
        &mut bytes,
        202,
        &(ExtensionType::PermissionedBurn as u16).to_le_bytes(),
    );
    put(&mut bytes, 204, &32_u16.to_le_bytes());
    put(&mut bytes, 206, state.as_ref());
    bytes
}

fn token_account_data(
    mint: Pubkey,
    owner: Pubkey,
    amount: u64,
    delegate: COption<Pubkey>,
    delegated_amount: u64,
) -> Vec<u8> {
    let mut bytes = vec![0_u8; SplAccount::LEN];
    SplAccount::pack(
        SplAccount {
            mint,
            owner,
            amount,
            delegate,
            state: AccountState::Initialized,
            is_native: COption::None,
            delegated_amount,
            close_authority: COption::None,
        },
        &mut bytes,
    )
    .expect("pack token account");
    bytes
}

fn terminal_economic_data(
    market: Pubkey,
    release_set: [u8; 32],
    claimant: Pubkey,
    state: Pubkey,
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let outcome_count = usize::try_from(OUTCOME_COUNT).expect("small outcome width");
    let mut aggregate = vec![0_u8; MARKET_HEADER_BYTES + outcome_count * 3 * SCALAR_BYTES];
    let mut claimant_position =
        vec![0_u8; POSITION_HEADER_BYTES + outcome_count * 2 * SCALAR_BYTES];
    let mut wrapper_position = vec![0_u8; POSITION_HEADER_BYTES + outcome_count * 2 * SCALAR_BYTES];
    initialize_market(
        &mut aggregate,
        market.to_bytes(),
        release_set,
        REGISTRY_PROGRAM_ID.to_bytes(),
        OUTCOME_COUNT,
        EconomicPhase::Open,
        0,
    )
    .expect("Claims Market");
    initialize_position(
        &mut claimant_position,
        market.to_bytes(),
        claimant.to_bytes(),
        OUTCOME_COUNT,
    )
    .expect("claimant Position");
    initialize_position(
        &mut wrapper_position,
        market.to_bytes(),
        state.to_bytes(),
        OUTCOME_COUNT,
    )
    .expect("wrapper Position");
    let quantities = CLAIM_ATOMS
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect::<Vec<_>>();
    execute_basket(
        &mut aggregate,
        None,
        Some(&mut claimant_position),
        BasketFrame {
            expected_market_revision: 0,
            expected_source_revision: None,
            expected_destination_revision: Some(0),
            action: BasketAction::MintCompleteSet,
            quantities: &quantities,
            quantity_multiplier: 1,
        },
    )
    .expect("founding complete set");
    execute_basket(
        &mut aggregate,
        Some(&mut claimant_position),
        Some(&mut wrapper_position),
        BasketFrame {
            expected_market_revision: 1,
            expected_source_revision: Some(1),
            expected_destination_revision: Some(0),
            action: BasketAction::Materialize,
            quantities: &quantities,
            quantity_multiplier: 1,
        },
    )
    .expect("materialize representation");
    // Economic ABI phase tag and winner are fixed generated-header fields.
    *aggregate.get_mut(10).expect("Economic phase tag") = 1;
    put(&mut aggregate, 20, &WINNER.to_le_bytes());
    assert_eq!(
        market_phase(&aggregate),
        Ok(EconomicPhase::Terminal(WINNER))
    );
    (aggregate, claimant_position, wrapper_position)
}

fn core_market(
    release_set: [u8; 32],
    realm: [u8; 32],
    product: [u8; 32],
    result_domain: [u8; 32],
) -> (Pubkey, Vec<u8>) {
    let mut identity = MarketIdentity {
        market_id: semantic_identity([1; 32]),
        realm_id: semantic_identity(realm),
        product_id: semantic_identity(product),
        result_domain: semantic_identity(result_domain),
        resolution_policy: semantic_identity([0x71; 32]),
        capability_manifest: semantic_identity([0x72; 32]),
        selected_release_set: semantic_identity(release_set),
        registry_program: semantic_identity(REGISTRY_PROGRAM_ID.to_bytes()),
        generation: GENERATION,
    };
    let market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV1::new(identity).as_slices(),
        &CORE_PROGRAM_ID,
    )
    .0;
    identity.market_id = semantic_identity(market.to_bytes());
    let state = CoreState {
        phase: CorePhase::Terminal,
        readiness: Readiness::Consumed,
        terminal_winner: WINNER,
        identity,
        outstanding_capabilities: 1,
        rent_beneficiary: semantic_identity([0x73; 32]),
        terminal_receipt: Some(semantic_identity([0x74; 32])),
    };
    (
        market,
        state.encode().expect("terminal Core state").to_vec(),
    )
}

fn terminal_request(
    release_set: [u8; 32],
    market: Pubkey,
    realm: [u8; 32],
    descriptor: Pubkey,
    action: ActionV1,
    collateral_mint: Pubkey,
    recipient: Pubkey,
) -> (CustodyRequestV1, Pubkey, Pubkey, Pubkey) {
    let action_bytes = action.encode().expect("action bytes");
    let mut request = CustodyRequestV1 {
        operation: OperationV1::Transfer,
        caller_role: CallerRoleV1::Claims,
        source_compartment: CompartmentV1::HoardPrincipal,
        destination_compartment: CompartmentV1::External,
        release_set,
        market: market.to_bytes(),
        realm,
        context: descriptor.to_bytes(),
        caller_program: CLAIMS_PROGRAM_ID.to_bytes(),
        semantic: ContextV1 {
            candidate: [0; 32],
            actor: action.claimant,
            order: [0; 32],
            parent_request_digest: hash(&action_bytes).to_bytes(),
            order_nonce: action.expected_next_nonce,
            generation: GENERATION,
            page_index: 0,
            execution_index: 0,
            transfer_index: 0,
        },
        source: [0x91; 32],
        destination: recipient.to_bytes(),
        source_vault_context: market.to_bytes(),
        destination_vault_context: [0; 32],
        mint: collateral_mint.to_bytes(),
        token_program: TOKEN_2022_PROGRAM_ID,
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision: CUSTODY_EXPECTED_REVISION,
        resulting_revision: CUSTODY_EXPECTED_REVISION + 1,
        amount: TERMINAL_PAYOUT,
        rent_lamports: 0,
    };
    let custody_authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::from_request(request).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let hoard = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::from_request(request, true).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    request.source = hoard.to_bytes();
    request.to_bytes().expect("canonical terminal request");
    let replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::from_request(request).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    (request, replay, hoard, custody_authority)
}

fn fixture() -> (ProgramTest, Fixture) {
    let artifacts = artifacts();
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    add_upgradeable_program(
        &mut test,
        "dclutch_claims_sbf",
        CLAIMS_PROGRAM_ID,
        &artifacts.claims,
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
    add_upgradeable_program(
        &mut test,
        "dclutch_core_sbf",
        CORE_PROGRAM_ID,
        &artifacts.core,
    );
    add_upgradeable_program(
        &mut test,
        "dclutch_claims_terminal_test_caller_sbf",
        TEST_CALLER_PROGRAM_ID,
        &artifacts.test_caller,
    );
    add_upgradeable_program(
        &mut test,
        "spl_token_2022",
        Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID),
        &artifacts.token_2022,
    );

    let claimant = Keypair::new_from_array([0xa6; 32]);
    add_account(&mut test, claimant.pubkey(), system_program::ID, Vec::new());
    let (release_set, cache_data) = activation_cache(&artifacts);
    let activation_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    add_account(&mut test, activation_cache, REGISTRY_PROGRAM_ID, cache_data);

    let receipt_mint = Pubkey::new_from_array([0xa1; 32]);
    let collateral_mint = Pubkey::new_from_array([0xa2; 32]);
    let adapter = PRODUCTION_ADAPTER_RELEASES
        .get(1)
        .copied()
        .expect("Token-2022 production adapter");
    let realm_value = RealmV1::new(RealmV1Input {
        token_program: TOKEN_2022_PROGRAM_ID,
        collateral_mint: collateral_mint.to_bytes(),
        collateral_adapter_release_id: hash(&adapter.to_bytes()).to_bytes(),
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    })
    .expect("Realm");
    let realm_data = realm_value.to_bytes().to_vec();
    let realm = hash(&realm_data).to_bytes();
    let realm_key = Pubkey::find_program_address(&[REALM_PDA_DOMAIN, &realm], &CORE_PROGRAM_ID).0;
    add_account(&mut test, realm_key, CORE_PROGRAM_ID, realm_data);

    let product = [0x61; 32];
    let result_domain = [0x62; 32];
    let (core_market, core_data) = core_market(release_set, realm, product, result_domain);
    add_account(&mut test, core_market, CORE_PROGRAM_ID, core_data);
    let descriptor = Pubkey::new_from_array([0xa3; 32]);
    let state = Pubkey::find_program_address(
        &[b"dclutch:representation:v1", descriptor.as_ref()],
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    let descriptor_data = descriptor_data(
        descriptor,
        core_market,
        product,
        result_domain,
        receipt_mint,
        release_set,
    );
    let state_data = StateV1 {
        descriptor_id: descriptor.to_bytes(),
        next_nonce: 4,
        issued_lots: 1,
        retired: false,
    }
    .encode()
    .expect("representation state")
    .to_vec();
    add_account(
        &mut test,
        descriptor,
        CLAIMS_PROGRAM_ID,
        descriptor_data.clone(),
    );
    add_account(&mut test, state, CLAIMS_PROGRAM_ID, state_data.clone());

    let market = Pubkey::find_program_address(
        &ClaimsAggregateSeedsV1::new(core_market.to_bytes())
            .expect("aggregate seeds")
            .as_slices(),
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    let claimant_position = Pubkey::find_program_address(
        &ClaimsPositionSeedsV1::new(core_market.to_bytes(), claimant.pubkey().to_bytes())
            .expect("claimant seeds")
            .as_slices(),
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    let wrapper_position = Pubkey::find_program_address(
        &ClaimsPositionSeedsV1::new(core_market.to_bytes(), state.to_bytes())
            .expect("wrapper seeds")
            .as_slices(),
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    let (market_data, claimant_data, wrapper_data) =
        terminal_economic_data(core_market, release_set, claimant.pubkey(), state);
    add_account(&mut test, market, CLAIMS_PROGRAM_ID, market_data);
    add_account(
        &mut test,
        claimant_position,
        CLAIMS_PROGRAM_ID,
        claimant_data,
    );
    add_account(&mut test, wrapper_position, CLAIMS_PROGRAM_ID, wrapper_data);

    let holder = Pubkey::new_from_array([0xa4; 32]);
    add_account(
        &mut test,
        receipt_mint,
        Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID),
        representation_mint_data(state),
    );
    add_account(
        &mut test,
        holder,
        Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID),
        token_account_data(
            receipt_mint,
            claimant.pubkey(),
            RECEIPT_UNITS_PER_LOT,
            COption::None,
            0,
        ),
    );
    add_account(
        &mut test,
        collateral_mint,
        Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID),
        base_mint_data(COption::None, INITIAL_RECIPIENT_ATOMS + TERMINAL_PAYOUT, 6).to_vec(),
    );

    let recipient = Pubkey::new_from_array([0xa5; 32]);
    let action = ActionV1 {
        tag: 3,
        descriptor_id: descriptor.to_bytes(),
        expected_release_set_id: release_set,
        claimant: claimant.pubkey().to_bytes(),
        expected_next_nonce: 4,
        expected_issued_lots: 1,
        lots: 1,
    };
    prepare(
        DescriptorV1::decode(&descriptor_data).expect("fixture descriptor"),
        StateV1::decode(&state_data).expect("fixture representation state"),
        action,
        RepresentationEconomicPhase::Terminal,
        ClaimsReleaseAdmission {
            selected_release_set_id: release_set,
            receipt_release_set_id: release_set,
            registry_authenticated: true,
            claims_role_authenticated: true,
            activation_cache_authenticated: true,
            current_deployment_reauthenticated: true,
        },
    )
    .expect("pure terminal representation transition");
    let (custody_request, replay, hoard, custody_authority) = terminal_request(
        release_set,
        core_market,
        realm,
        descriptor,
        action,
        collateral_mint,
        recipient,
    );
    let replay_state = CustodyReplayV1 {
        caller_role: CallerRoleV1::Claims,
        release_set,
        market: core_market.to_bytes(),
        realm,
        context: descriptor.to_bytes(),
        caller_program: CLAIMS_PROGRAM_ID.to_bytes(),
        rent_refund: claimant.pubkey().to_bytes(),
        next_revision: CUSTODY_EXPECTED_REVISION,
        generation: GENERATION,
        last_request_digest: [0xa1; 32],
        last_poststate_commitment: [0xa2; 32],
    };
    replay_state
        .advance(
            custody_request,
            hash(
                &custody_request
                    .to_bytes()
                    .expect("Custody request replay bytes"),
            )
            .to_bytes(),
            [0xa3; 32],
        )
        .expect("fixture replay advances under canonical contract");
    add_account(
        &mut test,
        replay,
        CUSTODY_PROGRAM_ID,
        replay_state.to_bytes().expect("Custody replay").to_vec(),
    );
    add_account(
        &mut test,
        hoard,
        Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID),
        token_account_data(
            collateral_mint,
            custody_authority,
            TERMINAL_PAYOUT,
            COption::None,
            0,
        ),
    );
    add_account(
        &mut test,
        recipient,
        Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID),
        token_account_data(
            collateral_mint,
            claimant.pubkey(),
            INITIAL_RECIPIENT_ATOMS,
            COption::None,
            0,
        ),
    );

    (
        test,
        Fixture {
            claimant,
            descriptor,
            state,
            market,
            claimant_position,
            wrapper_position,
            activation_cache,
            claims_programdata: programdata_address(CLAIMS_PROGRAM_ID),
            core_market,
            core_programdata: programdata_address(CORE_PROGRAM_ID),
            custody_programdata: programdata_address(CUSTODY_PROGRAM_ID),
            realm_key,
            receipt_mint,
            holder,
            collateral_mint,
            replay,
            hoard,
            recipient,
            custody_authority,
            custody_request,
            action,
        },
    )
}

fn custody_caller_authority(request: CustodyRequestV1) -> Pubkey {
    let request_bytes = request.to_bytes().expect("Custody request bytes");
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(request.release_set).expect("release set"),
        request.market,
        ExecutionRoleV1::Claims,
        request.context,
        hash(&request_bytes).to_bytes(),
    )
    .expect("Claims caller seeds");
    Pubkey::find_program_address(&seeds.as_slices(), &CLAIMS_PROGRAM_ID).0
}

fn terminal_accounts(fixture: &Fixture) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new_readonly(fixture.claimant.pubkey(), true),
        AccountMeta::new_readonly(fixture.descriptor, false),
        AccountMeta::new(fixture.state, false),
        AccountMeta::new(fixture.market, false),
        AccountMeta::new(fixture.claimant_position, false),
        AccountMeta::new(fixture.wrapper_position, false),
        AccountMeta::new_readonly(fixture.activation_cache, false),
        AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.claims_programdata, false),
        AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
        AccountMeta::new(fixture.receipt_mint, false),
        AccountMeta::new(fixture.holder, false),
        AccountMeta::new_readonly(Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID), false),
        AccountMeta::new_readonly(fixture.core_market, false),
        AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.core_programdata, false),
        AccountMeta::new_readonly(custody_caller_authority(fixture.custody_request), false),
        AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.custody_programdata, false),
        AccountMeta::new_readonly(fixture.realm_key, false),
        AccountMeta::new(fixture.replay, false),
        AccountMeta::new_readonly(fixture.collateral_mint, false),
        AccountMeta::new(fixture.hoard, false),
        AccountMeta::new(fixture.recipient, false),
        AccountMeta::new_readonly(fixture.custody_authority, false),
        AccountMeta::new_readonly(Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID), false),
    ]
}

fn terminal_data(fixture: &Fixture) -> Vec<u8> {
    let mut data = fixture.action.encode().expect("action bytes").to_vec();
    data.extend_from_slice(
        &fixture
            .custody_request
            .to_bytes()
            .expect("Custody request bytes"),
    );
    assert_eq!(data.len(), 776);
    data
}

fn claims_instruction(fixture: &Fixture) -> Instruction {
    let accounts = terminal_accounts(fixture);
    assert_eq!(accounts.len(), TERMINAL_ACCOUNT_METAS);
    Instruction {
        program_id: CLAIMS_PROGRAM_ID,
        accounts,
        data: terminal_data(fixture),
    }
}

fn late_wrapper_instruction(fixture: &Fixture) -> Instruction {
    let mut accounts = vec![AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false)];
    accounts.extend(terminal_accounts(fixture));
    let mut data = Vec::with_capacity(777);
    data.push(1);
    data.extend_from_slice(&terminal_data(fixture));
    Instruction {
        program_id: TEST_CALLER_PROGRAM_ID,
        accounts,
        data,
    }
}

fn unique_account_count(instruction: &Instruction) -> usize {
    let mut addresses = vec![instruction.program_id];
    for account in &instruction.accounts {
        if !addresses.contains(&account.pubkey) {
            addresses.push(account.pubkey);
        }
    }
    addresses.len()
}

fn legacy_wire_bytes(payer: Pubkey, instruction: Instruction, _hash: Hash) -> usize {
    let message = solana_message::legacy::Message::new(&[instruction], Some(&payer));
    1 + usize::from(message.header.num_required_signatures) * 64 + message.serialize().len()
}

fn no_lookup_v0_wire_bytes(payer: Pubkey, instruction: Instruction, hash: Hash) -> usize {
    let message = VersionedMessage::V0(
        v0::Message::try_compile(&payer, &[instruction], &[], hash).expect("uncompressed v0"),
    );
    1 + 2 * 64 + message.serialize().len()
}

fn live_lookup_v0_wire_bytes(
    payer: Pubkey,
    instruction: Instruction,
    hash: Hash,
    table: Pubkey,
    addresses: Vec<Pubkey>,
) -> usize {
    let message = VersionedMessage::V0(
        v0::Message::try_compile(
            &payer,
            &[instruction],
            &[AddressLookupTableAccount {
                key: table,
                addresses,
            }],
            hash,
        )
        .expect("compressed v0"),
    );
    1 + 2 * 64 + message.serialize().len()
}

fn lookup_addresses(payer: Pubkey, fixture: &Fixture, instructions: &[Instruction]) -> Vec<Pubkey> {
    let claimant = fixture.claimant.pubkey();
    let mut addresses = Vec::new();
    for instruction in instructions {
        if instruction.program_id != payer
            && instruction.program_id != claimant
            && !addresses.contains(&instruction.program_id)
        {
            addresses.push(instruction.program_id);
        }
        for account in &instruction.accounts {
            if account.pubkey != payer
                && account.pubkey != claimant
                && !addresses.contains(&account.pubkey)
            {
                addresses.push(account.pubkey);
            }
        }
    }
    addresses
}

async fn process_legacy(context: &mut ProgramTestContext, instruction: Instruction) -> u64 {
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
        .expect("ALT lifecycle processing");
    assert!(processed.result.is_ok(), "ALT lifecycle must commit");
    processed
        .metadata
        .map_or(0, |metadata| metadata.compute_units_consumed)
}

async fn create_live_lookup_table(
    context: &mut ProgramTestContext,
    addresses: &[Pubkey],
) -> (Pubkey, [u64; 3]) {
    let clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("Clock sysvar");
    context
        .warp_to_slot(clock.slot + 1)
        .expect("make lookup-table slot recent");
    let payer = context.payer.pubkey();
    let (create, table) = create_lookup_table(payer, payer, clock.slot);
    let create_cu = process_legacy(context, create).await;
    let first = addresses.get(..addresses.len().min(20)).unwrap_or_default();
    let remaining = addresses.get(first.len()..).unwrap_or_default();
    let first_cu = process_legacy(
        context,
        extend_lookup_table(table, payer, Some(payer), first.to_vec()),
    )
    .await;
    let second_cu = if remaining.is_empty() {
        0
    } else {
        process_legacy(
            context,
            extend_lookup_table(table, payer, Some(payer), remaining.to_vec()),
        )
        .await
    };
    let extension_clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("post-extension Clock");
    context
        .warp_to_slot(extension_clock.slot + 1)
        .expect("activate lookup addresses");
    (table, [create_cu, first_cu, second_cu])
}

async fn submit_v0(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    instruction: Instruction,
    table: Pubkey,
    addresses: Vec<Pubkey>,
) -> Result<Submission, BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let lookup = AddressLookupTableAccount {
        key: table,
        addresses,
    };
    let message = VersionedMessage::V0(
        v0::Message::try_compile(
            &context.payer.pubkey(),
            &[instruction],
            &[lookup],
            blockhash,
        )
        .expect("v0 message"),
    );
    let transaction = VersionedTransaction::try_new(message, &[&context.payer, &fixture.claimant])
        .expect("signed v0 transaction");
    let wire_bytes = 1 + transaction.signatures.len() * 64 + transaction.message.serialize().len();
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await?;
    let (compute_units, logs) = processed
        .metadata
        .map(|metadata| (metadata.compute_units_consumed, metadata.log_messages))
        .unwrap_or_default();
    Ok(Submission {
        accepted: processed.result.is_ok(),
        compute_units,
        wire_bytes,
        logs,
    })
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
        state: observed(context, fixture.state).await,
        market: observed(context, fixture.market).await,
        claimant_position: observed(context, fixture.claimant_position).await,
        wrapper_position: observed(context, fixture.wrapper_position).await,
        receipt_mint: observed(context, fixture.receipt_mint).await,
        holder: observed(context, fixture.holder).await,
        replay: observed(context, fixture.replay).await,
        hoard: observed(context, fixture.hoard).await,
        recipient: observed(context, fixture.recipient).await,
    }
}

fn token_amount(account: &Account) -> u64 {
    TokenAccount::parse(&account.data)
        .expect("token account")
        .amount
}

fn receipt_supply(account: &Account) -> u64 {
    SplMint::unpack(account.data.get(..SplMint::LEN).expect("Mint base"))
        .expect("Mint")
        .supply
}

fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    let value: [u8; 8] = bytes
        .get(offset..offset + 8)
        .expect("u64 field")
        .try_into()
        .expect("u64 width");
    u64::from_le_bytes(value)
}

#[tokio::test]
async fn direct_terminal_representation_is_real_atomic_and_alt_bounded() {
    let (test, fixture) = fixture();
    let mut context = test.start_with_context().await;
    let payer = context.payer.pubkey();
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let direct = claims_instruction(&fixture);
    let late = late_wrapper_instruction(&fixture);
    let legacy_bytes = legacy_wire_bytes(payer, direct.clone(), blockhash);
    let no_lookup_v0_bytes = no_lookup_v0_wire_bytes(payer, direct.clone(), blockhash);
    assert!(legacy_bytes > PACKET_LIMIT, "legacy must honestly overflow");
    assert!(
        no_lookup_v0_bytes > PACKET_LIMIT,
        "v0 without an ALT must honestly overflow"
    );
    assert_eq!(direct.accounts.len(), TERMINAL_ACCOUNT_METAS);
    let direct_unique_accounts = unique_account_count(&direct);

    let addresses = lookup_addresses(payer, &fixture, &[direct.clone(), late.clone()]);
    let (table, lookup_cu) = create_live_lookup_table(&mut context, &addresses).await;
    let direct_live_v0_bytes = live_lookup_v0_wire_bytes(
        payer,
        direct.clone(),
        context
            .banks_client
            .get_latest_blockhash()
            .await
            .expect("post-ALT blockhash"),
        table,
        addresses.clone(),
    );
    assert!(direct_live_v0_bytes <= PACKET_LIMIT, "positive v0 overflow");
    eprintln!(
        "Claims terminal packet preflight: data=776, metas={}, unique={}, legacy={}, v0-no-ALT={}, v0-live-ALT={}, ALT-create/extend-CU={lookup_cu:?}",
        TERMINAL_ACCOUNT_METAS,
        direct_unique_accounts,
        legacy_bytes,
        no_lookup_v0_bytes,
        direct_live_v0_bytes,
    );
    let before = snapshot(&mut context, &fixture).await;
    let late_result = submit_v0(&mut context, &fixture, late, table, addresses.clone())
        .await
        .expect("late rollback transaction");
    eprintln!(
        "Claims terminal late wrapper: v0={}, CU={}, accepted={}",
        late_result.wire_bytes, late_result.compute_units, late_result.accepted,
    );
    assert!(
        !late_result.accepted,
        "late wrapper must deliberately refuse"
    );
    assert!(late_result.wire_bytes <= PACKET_LIMIT, "late v0 overflow");
    let custody_success = format!("Program {CUSTODY_PROGRAM_ID} success");
    let claims_success = format!("Program {CLAIMS_PROGRAM_ID} success");
    assert!(
        late_result.logs.iter().any(|log| log == &custody_success),
        "Custody must return successfully before the deliberate refusal"
    );
    assert!(
        late_result.logs.iter().any(|log| log == &claims_success),
        "Claims must return successfully before the deliberate refusal"
    );
    assert_eq!(
        snapshot(&mut context, &fixture).await,
        before,
        "late refusal must roll back Claims, Token-2022, and Custody together"
    );

    let positive = submit_v0(&mut context, &fixture, direct, table, addresses)
        .await
        .expect("positive terminal transaction");
    assert!(positive.accepted, "terminal composition must commit");
    assert!(positive.wire_bytes <= PACKET_LIMIT, "positive v0 overflow");
    let after = snapshot(&mut context, &fixture).await;
    let state = StateV1::decode(&after.state.data).expect("post representation state");
    assert_eq!(state.next_nonce, 5);
    assert_eq!(state.issued_lots, 0);
    assert!(!state.retired);
    assert_eq!(market_revision(&after.market.data), Ok(3));
    assert_eq!(u64_at(&after.market.data, 32), 0, "Claims Hoard projection");
    assert_eq!(
        position_revision(&after.claimant_position.data, OUTCOME_COUNT),
        Ok(2)
    );
    assert_eq!(
        position_revision(&after.wrapper_position.data, OUTCOME_COUNT),
        Ok(2)
    );
    for outcome in 0..OUTCOME_COUNT {
        assert_eq!(
            position_materialized(&after.wrapper_position.data, OUTCOME_COUNT, outcome),
            Ok(0)
        );
    }
    assert_eq!(receipt_supply(&after.receipt_mint), 0);
    assert_eq!(token_amount(&after.holder), 0);
    assert_eq!(
        CustodyReplayV1::decode(&after.replay.data)
            .expect("post Custody replay")
            .next_revision,
        CUSTODY_EXPECTED_REVISION + 1
    );
    assert_eq!(token_amount(&after.hoard), 0);
    assert_eq!(
        token_amount(&after.recipient),
        INITIAL_RECIPIENT_ATOMS + TERMINAL_PAYOUT
    );

    eprintln!(
        "Claims terminal: data=776, metas={}, unique={}, legacy={}, v0-no-ALT={}, v0-live-ALT={}, CU={}, late-v0={}, late-CU={}, ALT-create/extend-CU={lookup_cu:?}",
        TERMINAL_ACCOUNT_METAS,
        direct_unique_accounts,
        legacy_bytes,
        no_lookup_v0_bytes,
        positive.wire_bytes,
        positive.compute_units,
        late_result.wire_bytes,
        late_result.compute_units,
    );
}
