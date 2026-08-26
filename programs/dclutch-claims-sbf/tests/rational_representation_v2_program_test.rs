//! Real-ELF ProgramTest evidence for RationalRepresentationV2 composition.
//!
//! The campaign executes immutable Registry records, Claims economics, real
//! Token-2022 v11, and canonical Custody. A test-only SBF caller deliberately
//! refuses after the complete child graph returns to prove transaction-level
//! rollback across every mutable semantic owner.

use std::{env, fs, path::PathBuf, vec::Vec};

use dclutch_claims_sbf::liability_basis_v2::{
    LIABILITY_BASIS_CANDIDATE_DIGEST_DOMAIN_V2, LIABILITY_BASIS_MARKET_SEED_V2,
    LIABILITY_BASIS_SCHEMA_RELEASE_ID_V2, LiabilityBasisMarketInputV2,
    LiabilityBasisPositionInputV2, TERMINAL_COORDINATE_SCHEMA_RELEASE_ID_V2,
    encode_liability_basis_market_v2, encode_liability_basis_position_v2,
    encode_terminal_coordinate_v2,
};
use dclutch_claims_svm::protocol_position_v2::ProtocolPositionSeedsV2;
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CallerRoleV1 as CustodyCallerRoleV1, CompartmentV1, ContextV1, CustodyAuthoritySeedsV1,
    CustodyReplaySeedsV1, CustodyReplayV1, CustodyRequestV1, CustodyVaultSeedsV1, OperationV1,
};
use dclutch_liability_basis_v2_kernel::product_claims::{
    CAPPED_RAMP_BASIS_BYTES_V2, CappedRampBasisInputV2, ContentIdV2,
    LINKED_CAPPED_RAMP_BASIS_BYTES_V2, encode_capped_ramp_basis_v2, encode_linked_basis_record_v2,
};
use dclutch_market_core_codec::{
    CoreState, Identity, MarketCoreStateSeedsV2, MarketIdentity, Phase as CorePhase, Readiness,
};
use dclutch_product_runtime_v2::{
    ContentId as RuntimeContentId, PortfolioInputV2, ResultDomainInputV2, compile_portfolio_v2,
    compile_result_domain_v2, portfolio_record_bytes, result_domain_record_bytes,
};
use dclutch_product_runtime_v2_admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_BYTES_V2, PRODUCT_RECORD_SCHEMA_ID_V2, ProductRecordV2,
    RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_rational_representation_v2_contract::{
    ABSENT_REVISION, ASSET_BYTES_V2, AssetV2, CallerRoleV2, RATIONAL_ASSET_ACCOUNT_COUNT_V2,
    RATIONAL_BASE_ACCOUNT_COUNT_V2, RATIONAL_CLAIMS_CUSTODY_OWNER_SEED_V2,
    RATIONAL_REPLAY_BYTES_V2, RATIONAL_REPLAY_MAGIC_V2, RATIONAL_REPLAY_SEED_V2,
    RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2, RATIONAL_SHARD_MINT_SEED_V2,
    RATIONAL_TERMINAL_ACCOUNT_COUNT_V2, REQUEST_HEADER_BYTES_V2, RepresentationActionV2,
    RepresentationRequestHeaderV2, RepresentationRequestV2,
};
use dclutch_rational_representation_v2_kernel::{
    DESCRIPTOR_COEFFICIENT_BYTES, DESCRIPTOR_HEADER_BYTES, DESCRIPTOR_MAGIC_V3, GRAPH_EDGE_BYTES,
    GRAPH_HEADER_BYTES, GRAPH_MAGIC_V2, GRAPH_NODE_BYTES,
    REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3, REPRESENTATION_GRAPH_SCHEMA_RELEASE_ID_V2,
    SCALAR_BYTES, SCHEMA_VERSION_V2,
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
use dclutch_token_svm::{PRODUCTION_ADAPTER_RELEASES, TOKEN_2022_PROGRAM_ID, TokenAccount};
use solana_account::Account;
use solana_address_lookup_table_interface::instruction::{
    create_lookup_table, extend_lookup_table,
};
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_program::{
    clock::Clock,
    hash::{Hash, hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_option::COption;
use solana_program_pack::Pack;
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_transaction::{Transaction, versioned::VersionedTransaction};
use spl_associated_token_account_interface::address::get_associated_token_address_with_program_id;
use spl_token_interface::state::{Account as SplAccount, AccountState, Mint as SplMint};

const CLAIMS_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xe1; 32]);
const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xe2; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xe3; 32]);
const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xe4; 32]);
const TEST_CALLER_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xe5; 32]);
const TOKEN_PROGRAM_ID: Pubkey = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);
const GENERATION: u64 = 29;
const OUTCOME_COUNT: u32 = 2;
const WINNER: u32 = 1;
const DENOMINATOR: u64 = 10;
const RECEIPT_SUPPLY: u64 = 7;
const COEFFICIENTS: [u64; 2] = [3, 7];
const SHARD_SUPPLIES: [u64; 2] = [30, 70];
const ACTOR_SHARDS: [u64; 2] = [9, 21];
const STRUCTURED_SHARDS: [u64; 2] = [21, 49];
const CUSTODY_EXPECTED_REVISION: u64 = 8;
const INITIAL_RECIPIENT_ATOMS: u64 = 5;
const INITIAL_HOARD_ATOMS: u64 = 9;
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
    caller: Vec<u8>,
}

#[derive(Clone, Copy)]
struct AssetFixture {
    custody_owner: Pubkey,
    position: Pubkey,
    mint: Pubkey,
    actor_token: Pubkey,
    structured_token: Pubkey,
}

#[derive(Clone, Copy)]
struct TerminalFixture {
    coordinate_raw: Pubkey,
    coordinate_staging: Pubkey,
    realm_raw: Pubkey,
    realm_staging: Pubkey,
    custody_caller: Pubkey,
    custody_replay: Pubkey,
    collateral_mint: Pubkey,
    hoard: Pubkey,
    recipient: Pubkey,
    custody_authority: Pubkey,
}

struct Fixture {
    actor: Keypair,
    release_set: [u8; 32],
    realm_id: [u8; 32],
    parent_context: [u8; 32],
    market: Pubkey,
    aggregate: Pubkey,
    actor_position: Pubkey,
    activation_cache: Pubkey,
    claims_programdata: Pubkey,
    custody_programdata: Pubkey,
    core_programdata: Pubkey,
    caller_programdata: Pubkey,
    representation_authority: Pubkey,
    descriptor_id: [u8; 32],
    descriptor_raw: Pubkey,
    descriptor_staging: Pubkey,
    alternate_descriptor_raw: Pubkey,
    alternate_descriptor_staging: Pubkey,
    graph_id: [u8; 32],
    graph_raw: Pubkey,
    graph_staging: Pubkey,
    alternate_graph_raw: Pubkey,
    alternate_graph_staging: Pubkey,
    linked_basis_record: Pubkey,
    linked_basis_staging: Pubkey,
    product_record: Pubkey,
    product_staging: Pubkey,
    result_domain_record: Pubkey,
    result_domain_staging: Pubkey,
    portfolio_record: Pubkey,
    portfolio_staging: Pubkey,
    representation_replay: Pubkey,
    receipt_mint: Pubkey,
    actor_receipt: Pubkey,
    assets: [AssetFixture; 2],
    terminal_accounts: Option<TerminalFixture>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Snapshot {
    replay: Account,
    aggregate: Account,
    actor_position: Account,
    positions: [Account; 2],
    receipt_mint: Account,
    actor_receipt: Account,
    shard_mints: [Account; 2],
    actor_shards: [Account; 2],
    structured_shards: [Account; 2],
    custody_replay: Option<Account>,
    hoard: Option<Account>,
    recipient: Option<Account>,
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
        "the matching real Token-2022 v11 runtime is required"
    );
    Artifacts {
        claims: read("dclutch_claims_sbf.so"),
        custody: read("dclutch_custody_sbf.so"),
        registry: read("dclutch_registry_sbf.so"),
        core: read("dclutch_core_sbf.so"),
        token_2022,
        caller: read("dclutch_rational_v2_test_caller_sbf.so"),
    }
}

fn put(output: &mut [u8], offset: usize, input: &[u8]) {
    let end = offset.checked_add(input.len()).expect("fixture offset");
    output
        .get_mut(offset..end)
        .expect("fixture field")
        .copy_from_slice(input);
}

fn put_u32(output: &mut [u8], offset: usize, value: u32) {
    put(output, offset, &value.to_le_bytes());
}

fn put_u64(output: &mut [u8], offset: usize, value: u64) {
    put(output, offset, &value.to_le_bytes());
}

fn identity(key: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(key.to_bytes()).expect("nonzero program identity")
}

fn semantic_identity(bytes: [u8; 32]) -> Identity {
    Identity::new(bytes).expect("nonzero semantic identity")
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

fn add_funded_empty(test: &mut ProgramTest, key: Pubkey, required_bytes: usize) {
    test.add_account(
        key,
        Account {
            lamports: Rent::default().minimum_balance(required_bytes).max(1),
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn add_upgradeable_program(
    test: &mut ProgramTest,
    name: &'static str,
    program: Pubkey,
    elf: &[u8],
) {
    test.add_upgradeable_program_to_genesis(name, &program);
    add_account(
        test,
        programdata_address(program),
        bpf_loader_upgradeable::ID,
        immutable_programdata(elf),
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
    let core = release(CORE_PROGRAM_ID, 0x41, &artifacts.core);
    let claims = release(CLAIMS_PROGRAM_ID, 0x42, &artifacts.claims);
    let custody = release(CUSTODY_PROGRAM_ID, 0x43, &artifacts.custody);
    let trading = release(TEST_CALLER_PROGRAM_ID, 0x44, &artifacts.caller);
    let release_set = ExecutionReleaseSetV1::new(
        binding(core),
        binding(claims),
        binding(trading),
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
        (ExecutionRoleV1::Trading, trading),
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

fn finalized_record_keys(schema: [u8; 32], digest: [u8; 32]) -> (Pubkey, Pubkey) {
    let raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, schema.as_slice(), digest.as_slice()],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    let staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            schema.as_slice(),
            digest.as_slice(),
        ],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    (raw, staging)
}

fn add_finalized_record(
    test: &mut ProgramTest,
    schema: [u8; 32],
    bytes: &[u8],
) -> (Pubkey, Pubkey, [u8; 32]) {
    let digest = hash(bytes).to_bytes();
    let (raw, staging) = finalized_record_keys(schema, digest);
    add_account(test, raw, REGISTRY_PROGRAM_ID, bytes.to_vec());
    add_account(test, staging, system_program::ID, Vec::new());
    (raw, staging, digest)
}

#[derive(Clone, Copy)]
struct GraphNode {
    id: [u8; 32],
    rank: u32,
    first_edge: u32,
    edge_count: u32,
    kind: u8,
    parameter: u64,
    exposure: [u64; 2],
}

#[derive(Clone, Copy)]
struct GraphEdge {
    child_id: [u8; 32],
    child_index: u32,
    multiplicity: u64,
}

fn graph_bytes(changed: bool) -> Vec<u8> {
    let graph_id = [0x31; 32];
    let root = [0x45; 32];
    let final_multiplicities = if changed { [4, 6] } else { [3, 7] };
    let first_multiplicity = *final_multiplicities.first().expect("first multiplicity");
    let second_multiplicity = *final_multiplicities.get(1).expect("second multiplicity");
    let nodes = [
        GraphNode {
            id: [0x41; 32],
            rank: 0,
            first_edge: 0,
            edge_count: 0,
            kind: 0,
            parameter: 0,
            exposure: [100, 0],
        },
        GraphNode {
            id: [0x42; 32],
            rank: 0,
            first_edge: 0,
            edge_count: 0,
            kind: 0,
            parameter: 1,
            exposure: [0, 100],
        },
        GraphNode {
            id: [0x43; 32],
            rank: 1,
            first_edge: 0,
            edge_count: 1,
            kind: 1,
            parameter: DENOMINATOR,
            exposure: [10, 0],
        },
        GraphNode {
            id: [0x44; 32],
            rank: 1,
            first_edge: 1,
            edge_count: 1,
            kind: 1,
            parameter: DENOMINATOR,
            exposure: [0, 10],
        },
        GraphNode {
            id: root,
            rank: 2,
            first_edge: 2,
            edge_count: 2,
            kind: 2,
            parameter: 0,
            exposure: [first_multiplicity * 10, second_multiplicity * 10],
        },
    ];
    let edges = [
        GraphEdge {
            child_id: [0x41; 32],
            child_index: 0,
            multiplicity: 1,
        },
        GraphEdge {
            child_id: [0x42; 32],
            child_index: 1,
            multiplicity: 1,
        },
        GraphEdge {
            child_id: [0x43; 32],
            child_index: 2,
            multiplicity: first_multiplicity,
        },
        GraphEdge {
            child_id: [0x44; 32],
            child_index: 3,
            multiplicity: second_multiplicity,
        },
    ];
    let mut bytes = vec![
        0;
        GRAPH_HEADER_BYTES
            + nodes.len() * GRAPH_NODE_BYTES
            + edges.len() * GRAPH_EDGE_BYTES
            + nodes.len() * 2 * SCALAR_BYTES
    ];
    put(&mut bytes, 0, &GRAPH_MAGIC_V2);
    put(&mut bytes, 8, &SCHEMA_VERSION_V2.to_le_bytes());
    put(&mut bytes, 16, &graph_id);
    put(&mut bytes, 48, &root);
    put_u32(&mut bytes, 80, OUTCOME_COUNT);
    put_u32(
        &mut bytes,
        84,
        u32::try_from(nodes.len()).expect("node count"),
    );
    put_u32(
        &mut bytes,
        88,
        u32::try_from(edges.len()).expect("edge count"),
    );
    put_u64(&mut bytes, 96, 100);
    for (index, node) in nodes.iter().enumerate() {
        let offset = GRAPH_HEADER_BYTES + index * GRAPH_NODE_BYTES;
        put(&mut bytes, offset, &node.id);
        put_u32(&mut bytes, offset + 32, node.rank);
        put_u32(&mut bytes, offset + 36, node.first_edge);
        put_u32(&mut bytes, offset + 40, node.edge_count);
        *bytes.get_mut(offset + 44).expect("node kind") = node.kind;
        put_u64(&mut bytes, offset + 48, node.parameter);
    }
    let edge_start = GRAPH_HEADER_BYTES + nodes.len() * GRAPH_NODE_BYTES;
    for (index, edge) in edges.iter().enumerate() {
        let offset = edge_start + index * GRAPH_EDGE_BYTES;
        put(&mut bytes, offset, &edge.child_id);
        put_u32(&mut bytes, offset + 32, edge.child_index);
        put_u64(&mut bytes, offset + 40, edge.multiplicity);
    }
    let exposure_start = edge_start + edges.len() * GRAPH_EDGE_BYTES;
    for (node_index, node) in nodes.iter().enumerate() {
        for (outcome, value) in node.exposure.iter().enumerate() {
            put_u64(
                &mut bytes,
                exposure_start + (node_index * 2 + outcome) * SCALAR_BYTES,
                *value,
            );
        }
    }
    bytes
}

fn descriptor_bytes(
    graph_digest: [u8; 32],
    market: Pubkey,
    release_set: [u8; 32],
    receipt_mint: Pubkey,
    coefficients: [u64; 2],
) -> Vec<u8> {
    let mut bytes = vec![0; DESCRIPTOR_HEADER_BYTES + 2 * DESCRIPTOR_COEFFICIENT_BYTES];
    put(&mut bytes, 0, &DESCRIPTOR_MAGIC_V3);
    put(&mut bytes, 8, &3_u16.to_le_bytes());
    put(&mut bytes, 16, &[0x31; 32]);
    put(&mut bytes, 48, &graph_digest);
    put(&mut bytes, 80, &[0x45; 32]);
    put(&mut bytes, 112, market.as_ref());
    put(&mut bytes, 144, &release_set);
    put(&mut bytes, 176, receipt_mint.as_ref());
    put(&mut bytes, 208, &TOKEN_2022_PROGRAM_ID);
    put_u32(&mut bytes, 240, OUTCOME_COUNT);
    put_u64(&mut bytes, 248, DENOMINATOR);
    for (index, coefficient) in coefficients.iter().enumerate() {
        put_u64(
            &mut bytes,
            DESCRIPTOR_HEADER_BYTES + index * DESCRIPTOR_COEFFICIENT_BYTES,
            *coefficient,
        );
    }
    bytes
}

fn core_market(
    release_set: [u8; 32],
    realm_id: [u8; 32],
    product_record: [u8; 32],
    product_id: [u8; 32],
    terminal_receipt: Option<[u8; 32]>,
) -> (Pubkey, Vec<u8>) {
    let mut identity = MarketIdentity {
        market_id: semantic_identity([1; 32]),
        realm_id: semantic_identity(realm_id),
        product_record: semantic_identity(product_record),
        product_id: semantic_identity(product_id),
        resolution_policy: semantic_identity([0x63; 32]),
        capability_manifest: semantic_identity([0x64; 32]),
        selected_release_set: semantic_identity(release_set),
        registry_program: semantic_identity(REGISTRY_PROGRAM_ID.to_bytes()),
        generation: GENERATION,
    };
    let market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(identity).as_slices(),
        &CORE_PROGRAM_ID,
    )
    .0;
    identity.market_id = semantic_identity(market.to_bytes());
    let state = CoreState {
        phase: if terminal_receipt.is_some() {
            CorePhase::Terminal
        } else {
            CorePhase::Open
        },
        readiness: Readiness::Consumed,
        terminal_winner: if terminal_receipt.is_some() {
            WINNER
        } else {
            0
        },
        identity,
        outstanding_capabilities: 1,
        rent_beneficiary: semantic_identity([0x65; 32]),
        terminal_receipt: terminal_receipt.map(semantic_identity),
    };
    (market, state.encode().expect("Core state").to_vec())
}

struct ProductClaimsFixture {
    product_id: [u8; 32],
    product_digest: [u8; 32],
    basis_id: [u8; 32],
    linked_basis_record: Pubkey,
    linked_basis_staging: Pubkey,
    product_record: Pubkey,
    product_staging: Pubkey,
    result_domain_record: Pubkey,
    result_domain_staging: Pubkey,
    portfolio_record: Pubkey,
    portfolio_staging: Pubkey,
}

fn add_core_finalized_record(
    test: &mut ProgramTest,
    schema: [u8; 32],
    bytes: &[u8],
) -> (Pubkey, Pubkey, [u8; 32]) {
    let digest = hash(bytes).to_bytes();
    let raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
        &CORE_PROGRAM_ID,
    )
    .0;
    let staging = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
        &CORE_PROGRAM_ID,
    )
    .0;
    add_account(test, raw, CORE_PROGRAM_ID, bytes.to_vec());
    add_account(test, staging, system_program::ID, Vec::new());
    (raw, staging, digest)
}

fn runtime_id(value: [u8; 32]) -> RuntimeContentId {
    RuntimeContentId::new(value).expect("runtime identity")
}

fn add_product_claims(test: &mut ProgramTest) -> ProductClaimsFixture {
    let stable_product = [0x61; 32];
    let product_for_basis = ContentIdV2::new(stable_product).expect("basis Product");
    let mut embedded = [0_u8; CAPPED_RAMP_BASIS_BYTES_V2];
    encode_capped_ramp_basis_v2(
        CappedRampBasisInputV2 {
            product_instance_id: product_for_basis,
            knot_denominator: 1,
            left_numerator: 0,
            right_numerator: 1,
            scale: 1,
        },
        &mut embedded,
    )
    .expect("basis");
    let basis_id = hashv(&[
        b"dclutch/lbv2/semantic-id/v2",
        embedded.get(..32).expect("basis prefix"),
        embedded.get(64..).expect("basis suffix"),
    ])
    .to_bytes();
    let mut linked = [0_u8; LINKED_CAPPED_RAMP_BASIS_BYTES_V2];
    encode_linked_basis_record_v2(
        product_for_basis,
        ContentIdV2::new(basis_id).expect("basis ID"),
        &embedded,
        &mut linked,
    )
    .expect("linked basis");
    let (linked_basis_record, linked_basis_staging, _) =
        add_core_finalized_record(test, LIABILITY_BASIS_SCHEMA_RELEASE_ID_V2, &linked);

    let cuts: [i128; 0] = [];
    let mut domain = vec![0_u8; result_domain_record_bytes(0).expect("domain width")];
    compile_result_domain_v2(
        ResultDomainInputV2 {
            product_id: runtime_id(stable_product),
            coordinate_domain_id: runtime_id([0x62; 32]),
            result_unit_id: runtime_id([0x63; 32]),
            liability_basis_id: runtime_id(basis_id),
            representation_release_id: runtime_id([0x64; 32]),
            mapping_release_id: runtime_id([0x65; 32]),
            cut_denominator: 1,
            cuts: &cuts,
        },
        &mut domain,
    )
    .expect("domain");
    let (result_domain_record, result_domain_staging, domain_digest) =
        add_finalized_record(test, RESULT_DOMAIN_SCHEMA_ID_V2, &domain);
    let coefficients = [1_u64, 1];
    let mut portfolio = vec![0_u8; portfolio_record_bytes(2).expect("portfolio width")];
    compile_portfolio_v2(
        PortfolioInputV2 {
            product_id: runtime_id(stable_product),
            result_domain_id: runtime_id(domain_digest),
            claim_basis_id: runtime_id([0x66; 32]),
            liability_basis_id: runtime_id(basis_id),
            representation_release_id: runtime_id([0x64; 32]),
            denominator: 1,
            coefficients: &coefficients,
        },
        &mut portfolio,
    )
    .expect("portfolio");
    let (portfolio_record, portfolio_staging, portfolio_digest) =
        add_finalized_record(test, PORTFOLIO_SCHEMA_ID_V2, &portfolio);
    let mut product = [0_u8; PRODUCT_RECORD_BYTES_V2];
    ProductRecordV2::new(
        runtime_id(stable_product),
        runtime_id(domain_digest),
        runtime_id(portfolio_digest),
    )
    .encode_into(&mut product)
    .expect("Product root");
    let (product_record, product_staging, product_digest) =
        add_finalized_record(test, PRODUCT_RECORD_SCHEMA_ID_V2, &product);
    ProductClaimsFixture {
        product_id: stable_product,
        product_digest,
        basis_id,
        linked_basis_record,
        linked_basis_staging,
        product_record,
        product_staging,
        result_domain_record,
        result_domain_staging,
        portfolio_record,
        portfolio_staging,
    }
}

fn mint_data(authority: COption<Pubkey>, supply: u64, decimals: u8) -> Vec<u8> {
    let mut bytes = vec![0; SplMint::LEN];
    SplMint::pack(
        SplMint {
            mint_authority: authority,
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

fn token_account_data(mint: Pubkey, owner: Pubkey, amount: u64) -> Vec<u8> {
    let mut bytes = vec![0; SplAccount::LEN];
    SplAccount::pack(
        SplAccount {
            mint,
            owner,
            amount,
            delegate: COption::None,
            state: AccountState::Initialized,
            is_native: COption::None,
            delegated_amount: 0,
            close_authority: COption::None,
        },
        &mut bytes,
    )
    .expect("pack Token Account");
    bytes
}

#[allow(clippy::too_many_arguments)]
fn request_bytes_from(
    action: RepresentationActionV2,
    release_set: [u8; 32],
    market: Pubkey,
    graph_id: [u8; 32],
    descriptor_id: [u8; 32],
    parent_context: [u8; 32],
    actor: Pubkey,
    receipt_mint: Pubkey,
    actor_receipt: Pubkey,
    representation_authority: Pubkey,
    realm_id: [u8; 32],
    recipient: Option<Pubkey>,
    representation_revision: u64,
    receipt_supply: u64,
    actor_balances: [u64; 2],
    structured_balances: [u64; 2],
    assets: [AssetFixture; 2],
) -> Vec<u8> {
    let structured = matches!(
        action,
        RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured
    );
    let terminal = action == RepresentationActionV2::RedeemTerminal;
    let selected_action = action.selected_outcome();
    let selected = if selected_action { WINNER } else { u32::MAX };
    let asset_count = if selected_action { 1 } else { OUTCOME_COUNT };
    let mut rows = vec![0; usize::try_from(asset_count).expect("asset width") * ASSET_BYTES_V2];
    let requested = if selected_action {
        vec![(WINNER, *assets.get(1).expect("terminal outcome asset"))]
    } else {
        vec![
            (0, *assets.first().expect("first asset")),
            (1, *assets.get(1).expect("second asset")),
        ]
    };
    for (row, (outcome, asset)) in requested.into_iter().enumerate() {
        let index = usize::try_from(outcome).expect("outcome index");
        AssetV2 {
            shard_mint: asset.mint.to_bytes(),
            actor_shard_account: asset.actor_token.to_bytes(),
            structured_custody_account: asset.structured_token.to_bytes(),
            claims_custody_owner: asset.custody_owner.to_bytes(),
            coefficient: *COEFFICIENTS.get(index).expect("coefficient"),
            expected_shard_supply: SHARD_SUPPLIES
                .get(index)
                .copied()
                .expect("shard supply")
                .checked_add(
                    if action == RepresentationActionV2::Reconstitute && outcome == WINNER {
                        DENOMINATOR
                    } else {
                        0
                    },
                )
                .expect("fixture supply"),
            expected_actor_shards: *actor_balances.get(index).expect("actor balance"),
            expected_structured_shards: *structured_balances
                .get(index)
                .expect("structured balance"),
        }
        .encode_into(
            rows.get_mut(row * ASSET_BYTES_V2..(row + 1) * ASSET_BYTES_V2)
                .expect("asset row"),
        )
        .expect("encode asset");
    }
    let request = RepresentationRequestV2::new(
        RepresentationRequestHeaderV2 {
            action,
            caller_role: CallerRoleV2::Trading,
            release_set,
            market: market.to_bytes(),
            graph_id,
            descriptor_id,
            parent_context,
            actor: actor.to_bytes(),
            receipt_mint: receipt_mint.to_bytes(),
            receipt_account: if structured {
                actor_receipt.to_bytes()
            } else {
                [0; 32]
            },
            representation_authority: representation_authority.to_bytes(),
            token_program: TOKEN_2022_PROGRAM_ID,
            realm: if terminal { realm_id } else { [0; 32] },
            collateral_recipient: recipient.map_or([0; 32], |value| value.to_bytes()),
            expected_representation_revision: representation_revision,
            expected_claims_market_revision: match action {
                RepresentationActionV2::Denominate => 0,
                RepresentationActionV2::Reconstitute => 1,
                RepresentationActionV2::RedeemTerminal => 0,
                RepresentationActionV2::IssueStructured
                | RepresentationActionV2::UnwrapStructured => ABSENT_REVISION,
            },
            expected_actor_position_revision: match action {
                RepresentationActionV2::Denominate => 0,
                RepresentationActionV2::Reconstitute => 1,
                RepresentationActionV2::IssueStructured
                | RepresentationActionV2::UnwrapStructured
                | RepresentationActionV2::RedeemTerminal => ABSENT_REVISION,
            },
            expected_custody_position_revision: match action {
                RepresentationActionV2::Denominate => 0,
                RepresentationActionV2::Reconstitute => 1,
                RepresentationActionV2::RedeemTerminal => 0,
                RepresentationActionV2::IssueStructured
                | RepresentationActionV2::UnwrapStructured => ABSENT_REVISION,
            },
            expected_custody_replay_revision: if terminal {
                CUSTODY_EXPECTED_REVISION
            } else {
                ABSENT_REVISION
            },
            generation: GENERATION,
            quantity: 1,
            denominator: DENOMINATOR,
            expected_receipt_supply: receipt_supply,
            outcome_count: OUTCOME_COUNT,
            selected_outcome: selected,
            asset_count,
        },
        &rows,
    )
    .expect("canonical representation request");
    let mut output = vec![0; REQUEST_HEADER_BYTES_V2 + rows.len()];
    request
        .encode_into(&mut output)
        .expect("encode representation request");
    output
}

fn request_bytes(
    fixture: &Fixture,
    action: RepresentationActionV2,
    representation_revision: u64,
) -> Vec<u8> {
    let issued = representation_revision == 1 && action == RepresentationActionV2::UnwrapStructured;
    let denominated = action == RepresentationActionV2::Reconstitute;
    request_bytes_from(
        action,
        fixture.release_set,
        fixture.market,
        fixture.graph_id,
        fixture.descriptor_id,
        fixture.parent_context,
        fixture.actor.pubkey(),
        fixture.receipt_mint,
        fixture.actor_receipt,
        fixture.representation_authority,
        fixture.realm_id,
        fixture.terminal_accounts.map(|value| value.recipient),
        representation_revision,
        if issued {
            RECEIPT_SUPPLY + 1
        } else {
            RECEIPT_SUPPLY
        },
        if issued {
            [6, 14]
        } else if denominated {
            [9, 31]
        } else {
            ACTOR_SHARDS
        },
        if issued { [24, 56] } else { STRUCTURED_SHARDS },
        fixture.assets,
    )
}

fn outer_caller_authority(request_bytes: &[u8], market: Pubkey, release_set: [u8; 32]) -> Pubkey {
    let request = RepresentationRequestV2::decode(request_bytes).expect("canonical request");
    let header = request.header();
    assert_eq!(header.market, market.to_bytes());
    assert_eq!(header.release_set, release_set);
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(release_set).expect("release set"),
        market.to_bytes(),
        ExecutionRoleV1::Trading,
        header.parent_context,
        hash(request_bytes).to_bytes(),
    )
    .expect("Trading caller seeds");
    Pubkey::find_program_address(&seeds.as_slices(), &TEST_CALLER_PROGRAM_ID).0
}

#[allow(clippy::too_many_arguments)]
fn terminal_custody(
    request_bytes: &[u8],
    release_set: [u8; 32],
    market: Pubkey,
    realm_id: [u8; 32],
    parent_context: [u8; 32],
    actor: Pubkey,
    collateral_mint: Pubkey,
    recipient: Pubkey,
    candidate_digest: [u8; 32],
) -> (CustodyRequestV1, Pubkey, Pubkey, Pubkey, Pubkey) {
    let mut request = CustodyRequestV1 {
        operation: OperationV1::Transfer,
        caller_role: CustodyCallerRoleV1::Claims,
        source_compartment: CompartmentV1::HoardPrincipal,
        destination_compartment: CompartmentV1::External,
        release_set,
        market: market.to_bytes(),
        realm: realm_id,
        context: parent_context,
        caller_program: CLAIMS_PROGRAM_ID.to_bytes(),
        semantic: ContextV1 {
            candidate: candidate_digest,
            source_owner: [0; 32],
            destination_owner: actor.to_bytes(),
            order: [0; 32],
            parent_request_digest: hash(request_bytes).to_bytes(),
            order_nonce: 0,
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
        amount: 1,
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
    let request_bytes = request.to_bytes().expect("canonical Custody request");
    let replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::from_request(request).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let caller_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(release_set).expect("release set"),
        market.to_bytes(),
        ExecutionRoleV1::Claims,
        parent_context,
        hash(&request_bytes).to_bytes(),
    )
    .expect("Claims caller seeds");
    let caller = Pubkey::find_program_address(&caller_seeds.as_slices(), &CLAIMS_PROGRAM_ID).0;
    (request, caller, replay, hoard, custody_authority)
}

fn fixture(terminal: bool) -> (ProgramTest, Fixture) {
    let artifacts = artifacts();
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    for (name, program, elf) in [
        (
            "dclutch_claims_sbf",
            CLAIMS_PROGRAM_ID,
            artifacts.claims.as_slice(),
        ),
        (
            "dclutch_custody_sbf",
            CUSTODY_PROGRAM_ID,
            artifacts.custody.as_slice(),
        ),
        (
            "dclutch_registry_sbf",
            REGISTRY_PROGRAM_ID,
            artifacts.registry.as_slice(),
        ),
        (
            "dclutch_core_sbf",
            CORE_PROGRAM_ID,
            artifacts.core.as_slice(),
        ),
        (
            "dclutch_rational_v2_test_caller_sbf",
            TEST_CALLER_PROGRAM_ID,
            artifacts.caller.as_slice(),
        ),
        (
            "spl_token_2022",
            TOKEN_PROGRAM_ID,
            artifacts.token_2022.as_slice(),
        ),
    ] {
        add_upgradeable_program(&mut test, name, program, elf);
    }

    let actor = Keypair::new_from_array(if terminal { [0x72; 32] } else { [0x71; 32] });
    add_account(&mut test, actor.pubkey(), system_program::ID, Vec::new());
    let (release_set, cache_data) = activation_cache(&artifacts);
    let activation_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    add_account(&mut test, activation_cache, REGISTRY_PROGRAM_ID, cache_data);

    let collateral_mint = Pubkey::new_from_array(if terminal { [0x74; 32] } else { [0x73; 32] });
    let adapter = PRODUCTION_ADAPTER_RELEASES
        .get(1)
        .copied()
        .expect("Token-2022 production adapter");
    let realm = RealmV1::new(RealmV1Input {
        token_program: TOKEN_2022_PROGRAM_ID,
        collateral_mint: collateral_mint.to_bytes(),
        collateral_adapter_release_id: hash(&adapter.to_bytes()).to_bytes(),
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    })
    .expect("Realm");
    let realm_bytes = realm.to_bytes();
    let (realm_raw, realm_staging, realm_id) =
        add_finalized_record(&mut test, REALM_SCHEMA_RELEASE_ID_V1, &realm_bytes);

    let product_claims = add_product_claims(&mut test);
    let terminal_coordinate = terminal.then(|| {
        let bytes = encode_terminal_coordinate_v2(0, 1).expect("terminal coordinate");
        add_core_finalized_record(&mut test, TERMINAL_COORDINATE_SCHEMA_RELEASE_ID_V2, &bytes)
    });
    let (market, core_data) = core_market(
        release_set,
        realm_id,
        product_claims.product_digest,
        product_claims.product_id,
        terminal_coordinate.map(|(_, _, digest)| digest),
    );
    add_account(&mut test, market, CORE_PROGRAM_ID, core_data);
    let aggregate = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, market.as_ref()],
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    let receipt_mint = Pubkey::new_from_array(if terminal { [0x76; 32] } else { [0x75; 32] });
    let actor_receipt = Pubkey::new_from_array(if terminal { [0x78; 32] } else { [0x77; 32] });
    let graph = graph_bytes(false);
    let (graph_raw, graph_staging, graph_digest) =
        add_finalized_record(&mut test, REPRESENTATION_GRAPH_SCHEMA_RELEASE_ID_V2, &graph);
    let alternate_graph = graph_bytes(true);
    let (alternate_graph_raw, alternate_graph_staging, alternate_graph_digest) =
        add_finalized_record(
            &mut test,
            REPRESENTATION_GRAPH_SCHEMA_RELEASE_ID_V2,
            &alternate_graph,
        );
    assert_ne!(graph_digest, alternate_graph_digest);
    let descriptor = descriptor_bytes(
        graph_digest,
        market,
        release_set,
        receipt_mint,
        COEFFICIENTS,
    );
    let (descriptor_raw, descriptor_staging, descriptor_id) = add_finalized_record(
        &mut test,
        REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
        &descriptor,
    );
    let alternate_descriptor =
        descriptor_bytes(graph_digest, market, release_set, receipt_mint, [4, 6]);
    let (alternate_descriptor_raw, alternate_descriptor_staging, alternate_descriptor_id) =
        add_finalized_record(
            &mut test,
            REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
            &alternate_descriptor,
        );
    assert_ne!(descriptor_id, alternate_descriptor_id);

    let representation_authority = Pubkey::find_program_address(
        &[RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2, &descriptor_id],
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    add_account(
        &mut test,
        representation_authority,
        system_program::ID,
        Vec::new(),
    );
    let assets = std::array::from_fn(|index| {
        let outcome = u32::try_from(index).expect("outcome");
        let outcome_bytes = outcome.to_le_bytes();
        let mint = Pubkey::find_program_address(
            &[RATIONAL_SHARD_MINT_SEED_V2, &descriptor_id, &outcome_bytes],
            &CLAIMS_PROGRAM_ID,
        )
        .0;
        let custody_owner = Pubkey::find_program_address(
            &[
                RATIONAL_CLAIMS_CUSTODY_OWNER_SEED_V2,
                &descriptor_id,
                &outcome_bytes,
            ],
            &CLAIMS_PROGRAM_ID,
        )
        .0;
        let position_seeds =
            ProtocolPositionSeedsV2::new(aggregate.to_bytes(), custody_owner.to_bytes())
                .expect("custody Position seeds");
        let position =
            Pubkey::find_program_address(&position_seeds.as_slices(), &CLAIMS_PROGRAM_ID).0;
        let actor_token = Pubkey::new_from_array(if index == 0 { [0x81; 32] } else { [0x82; 32] });
        let structured_token = get_associated_token_address_with_program_id(
            &representation_authority,
            &mint,
            &TOKEN_PROGRAM_ID,
        );
        AssetFixture {
            custody_owner,
            position,
            mint,
            actor_token,
            structured_token,
        }
    });

    let actor_position = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(aggregate.to_bytes(), actor.pubkey().to_bytes())
            .expect("actor Position seeds")
            .as_slices(),
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    let market_input = LiabilityBasisMarketInputV2 {
        revision: 0,
        logical_market: market.to_bytes(),
        release_set,
        registry_program: REGISTRY_PROGRAM_ID.to_bytes(),
        product_instance_id: product_claims.product_id,
        basis_id: product_claims.basis_id,
        realm_id,
        custody_context: [0x68; 32],
        generation: GENERATION,
    };
    let aggregate_data =
        encode_liability_basis_market_v2(market_input, &[3, 9]).expect("LBV2 Claims aggregate");
    add_account(&mut test, aggregate, CLAIMS_PROGRAM_ID, aggregate_data);
    add_account(
        &mut test,
        actor_position,
        CLAIMS_PROGRAM_ID,
        encode_liability_basis_position_v2(
            LiabilityBasisPositionInputV2 {
                revision: 0,
                market_account: aggregate.to_bytes(),
                owner: actor.pubkey().to_bytes(),
                basis_id: product_claims.basis_id,
            },
            &[0, 2],
        )
        .expect("actor Position"),
    );
    for (index, asset) in assets.iter().enumerate() {
        let claims = if index == 0 { [3, 0] } else { [0, 7] };
        add_account(
            &mut test,
            asset.position,
            CLAIMS_PROGRAM_ID,
            encode_liability_basis_position_v2(
                LiabilityBasisPositionInputV2 {
                    revision: 0,
                    market_account: aggregate.to_bytes(),
                    owner: asset.custody_owner.to_bytes(),
                    basis_id: product_claims.basis_id,
                },
                &claims,
            )
            .expect("custody Position"),
        );
        add_account(
            &mut test,
            asset.mint,
            TOKEN_PROGRAM_ID,
            mint_data(
                COption::Some(representation_authority),
                *SHARD_SUPPLIES.get(index).expect("shard supply"),
                0,
            ),
        );
        add_account(
            &mut test,
            asset.actor_token,
            TOKEN_PROGRAM_ID,
            token_account_data(
                asset.mint,
                actor.pubkey(),
                *ACTOR_SHARDS.get(index).expect("actor shards"),
            ),
        );
        add_account(
            &mut test,
            asset.structured_token,
            TOKEN_PROGRAM_ID,
            token_account_data(
                asset.mint,
                representation_authority,
                *STRUCTURED_SHARDS.get(index).expect("structured shards"),
            ),
        );
    }
    add_account(
        &mut test,
        receipt_mint,
        TOKEN_PROGRAM_ID,
        mint_data(COption::Some(representation_authority), RECEIPT_SUPPLY, 0),
    );
    add_account(
        &mut test,
        actor_receipt,
        TOKEN_PROGRAM_ID,
        token_account_data(receipt_mint, actor.pubkey(), 0),
    );
    let representation_replay = Pubkey::find_program_address(
        &[
            RATIONAL_REPLAY_SEED_V2,
            descriptor_id.as_slice(),
            actor.pubkey().as_ref(),
        ],
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    add_funded_empty(&mut test, representation_replay, RATIONAL_REPLAY_BYTES_V2);

    let parent_context = [0x68; 32];
    let fixture_stub = Fixture {
        actor,
        release_set,
        realm_id,
        parent_context,
        market,
        aggregate,
        actor_position,
        activation_cache,
        claims_programdata: programdata_address(CLAIMS_PROGRAM_ID),
        custody_programdata: programdata_address(CUSTODY_PROGRAM_ID),
        core_programdata: programdata_address(CORE_PROGRAM_ID),
        caller_programdata: programdata_address(TEST_CALLER_PROGRAM_ID),
        representation_authority,
        descriptor_id,
        descriptor_raw,
        descriptor_staging,
        alternate_descriptor_raw,
        alternate_descriptor_staging,
        graph_id: [0x31; 32],
        graph_raw,
        graph_staging,
        alternate_graph_raw,
        alternate_graph_staging,
        linked_basis_record: product_claims.linked_basis_record,
        linked_basis_staging: product_claims.linked_basis_staging,
        product_record: product_claims.product_record,
        product_staging: product_claims.product_staging,
        result_domain_record: product_claims.result_domain_record,
        result_domain_staging: product_claims.result_domain_staging,
        portfolio_record: product_claims.portfolio_record,
        portfolio_staging: product_claims.portfolio_staging,
        representation_replay,
        receipt_mint,
        actor_receipt,
        assets,
        terminal_accounts: None,
    };

    let mut fixture = fixture_stub;
    if terminal {
        let recipient = Pubkey::new_from_array([0x85; 32]);
        let terminal_request = request_bytes_from(
            RepresentationActionV2::RedeemTerminal,
            fixture.release_set,
            fixture.market,
            fixture.graph_id,
            fixture.descriptor_id,
            fixture.parent_context,
            fixture.actor.pubkey(),
            fixture.receipt_mint,
            fixture.actor_receipt,
            fixture.representation_authority,
            fixture.realm_id,
            Some(recipient),
            0,
            RECEIPT_SUPPLY,
            ACTOR_SHARDS,
            STRUCTURED_SHARDS,
            fixture.assets,
        );
        let (custody_request, custody_caller, custody_replay, hoard, custody_authority) =
            terminal_custody(
                &terminal_request,
                fixture.release_set,
                fixture.market,
                fixture.realm_id,
                fixture.parent_context,
                fixture.actor.pubkey(),
                collateral_mint,
                recipient,
                hashv(&[
                    &LIABILITY_BASIS_CANDIDATE_DIGEST_DOMAIN_V2,
                    &encode_liability_basis_market_v2(
                        LiabilityBasisMarketInputV2 {
                            revision: 1,
                            ..market_input
                        },
                        &[3, 8],
                    )
                    .expect("terminal aggregate candidate"),
                    &encode_liability_basis_position_v2(
                        LiabilityBasisPositionInputV2 {
                            revision: 1,
                            market_account: aggregate.to_bytes(),
                            owner: fixture.assets[1].custody_owner.to_bytes(),
                            basis_id: product_claims.basis_id,
                        },
                        &[0, 6],
                    )
                    .expect("terminal Position candidate"),
                ])
                .to_bytes(),
            );
        let replay_state = CustodyReplayV1 {
            caller_role: CustodyCallerRoleV1::Claims,
            release_set: fixture.release_set,
            market: fixture.market.to_bytes(),
            realm: fixture.realm_id,
            context: fixture.parent_context,
            caller_program: CLAIMS_PROGRAM_ID.to_bytes(),
            rent_refund: fixture.actor.pubkey().to_bytes(),
            open_vault_count: 1,
            next_revision: CUSTODY_EXPECTED_REVISION,
            generation: GENERATION,
            last_request_digest: [0x92; 32],
            last_poststate_commitment: [0x93; 32],
        };
        replay_state
            .advance(
                custody_request,
                hash(&custody_request.to_bytes().expect("Custody request")).to_bytes(),
                [0x94; 32],
            )
            .expect("Custody replay admits terminal transfer");
        add_account(
            &mut test,
            custody_replay,
            CUSTODY_PROGRAM_ID,
            replay_state.to_bytes().expect("Custody replay").to_vec(),
        );
        add_account(
            &mut test,
            collateral_mint,
            TOKEN_PROGRAM_ID,
            mint_data(
                COption::None,
                INITIAL_RECIPIENT_ATOMS + INITIAL_HOARD_ATOMS,
                6,
            ),
        );
        add_account(
            &mut test,
            hoard,
            TOKEN_PROGRAM_ID,
            token_account_data(collateral_mint, custody_authority, INITIAL_HOARD_ATOMS),
        );
        add_account(
            &mut test,
            recipient,
            TOKEN_PROGRAM_ID,
            token_account_data(
                collateral_mint,
                fixture.actor.pubkey(),
                INITIAL_RECIPIENT_ATOMS,
            ),
        );
        add_account(&mut test, custody_caller, system_program::ID, Vec::new());
        fixture.terminal_accounts = Some(TerminalFixture {
            coordinate_raw: terminal_coordinate.expect("terminal coordinate").0,
            coordinate_staging: terminal_coordinate.expect("terminal coordinate").1,
            realm_raw,
            realm_staging,
            custody_caller,
            custody_replay,
            collateral_mint,
            hoard,
            recipient,
            custody_authority,
        });
        assert_eq!(
            request_bytes(&fixture, RepresentationActionV2::RedeemTerminal, 0),
            terminal_request
        );
        let outer = outer_caller_authority(&terminal_request, fixture.market, fixture.release_set);
        add_account(&mut test, outer, system_program::ID, Vec::new());
    } else {
        for (action, revision) in [
            (RepresentationActionV2::IssueStructured, 0),
            (RepresentationActionV2::UnwrapStructured, 1),
            (RepresentationActionV2::Denominate, 2),
            (RepresentationActionV2::Reconstitute, 3),
        ] {
            let bytes = request_bytes(&fixture, action, revision);
            let outer = outer_caller_authority(&bytes, fixture.market, fixture.release_set);
            add_account(&mut test, outer, system_program::ID, Vec::new());
        }
    }
    (test, fixture)
}

fn claims_accounts(
    fixture: &Fixture,
    action: RepresentationActionV2,
    representation_revision: u64,
    descriptor_records: Option<(Pubkey, Pubkey)>,
    graph_records: Option<(Pubkey, Pubkey)>,
) -> Vec<AccountMeta> {
    let request = request_bytes(fixture, action, representation_revision);
    let decoded_request =
        RepresentationRequestV2::decode(&request).expect("canonical fixture request");
    let caller = outer_caller_authority(&request, fixture.market, fixture.release_set);
    let structured = matches!(
        action,
        RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured
    );
    let terminal = action == RepresentationActionV2::RedeemTerminal;
    let claims_active = action.selected_outcome();
    let actor_position_active = matches!(
        action,
        RepresentationActionV2::Denominate | RepresentationActionV2::Reconstitute
    );
    let (descriptor_raw, descriptor_staging) =
        descriptor_records.unwrap_or((fixture.descriptor_raw, fixture.descriptor_staging));
    let (graph_raw, graph_staging) =
        graph_records.unwrap_or((fixture.graph_raw, fixture.graph_staging));
    let mut metas = vec![
        AccountMeta::new_readonly(caller, false),
        AccountMeta::new_readonly(TEST_CALLER_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.caller_programdata, false),
        AccountMeta::new_readonly(fixture.actor.pubkey(), true),
        AccountMeta::new_readonly(fixture.representation_authority, false),
        AccountMeta::new_readonly(descriptor_raw, false),
        AccountMeta::new_readonly(descriptor_staging, false),
        AccountMeta::new_readonly(graph_raw, false),
        AccountMeta::new_readonly(graph_staging, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new(fixture.representation_replay, false),
        if claims_active {
            AccountMeta::new(fixture.aggregate, false)
        } else {
            AccountMeta::new_readonly(fixture.aggregate, false)
        },
        AccountMeta::new_readonly(fixture.activation_cache, false),
        AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.claims_programdata, false),
        AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.market, false),
        AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.core_programdata, false),
        if structured {
            AccountMeta::new(fixture.receipt_mint, false)
        } else {
            AccountMeta::new_readonly(fixture.receipt_mint, false)
        },
        if structured {
            AccountMeta::new(fixture.actor_receipt, false)
        } else {
            AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false)
        },
        AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
        if actor_position_active {
            AccountMeta::new(fixture.actor_position, false)
        } else {
            AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false)
        },
        AccountMeta::new_readonly(fixture.linked_basis_record, false),
        AccountMeta::new_readonly(fixture.linked_basis_staging, false),
        AccountMeta::new_readonly(fixture.product_record, false),
        AccountMeta::new_readonly(fixture.product_staging, false),
        AccountMeta::new_readonly(fixture.result_domain_record, false),
        AccountMeta::new_readonly(fixture.result_domain_staging, false),
        AccountMeta::new_readonly(fixture.portfolio_record, false),
        AccountMeta::new_readonly(fixture.portfolio_staging, false),
    ];
    assert_eq!(metas.len(), RATIONAL_BASE_ACCOUNT_COUNT_V2);
    let selected_outcome = decoded_request.header().selected_outcome;
    let physical_assets = if action.selected_outcome() {
        vec![
            *fixture
                .assets
                .get(usize::try_from(selected_outcome).expect("selected outcome index"))
                .expect("selected fixture asset"),
        ]
    } else {
        fixture.assets.to_vec()
    };
    for asset in physical_assets {
        let selected = action.selected_outcome();
        metas.extend([
            if selected {
                AccountMeta::new(asset.position, false)
            } else {
                AccountMeta::new_readonly(asset.position, false)
            },
            if selected {
                AccountMeta::new(asset.mint, false)
            } else {
                AccountMeta::new_readonly(asset.mint, false)
            },
            AccountMeta::new(asset.actor_token, false),
            if structured {
                AccountMeta::new(asset.structured_token, false)
            } else {
                AccountMeta::new_readonly(asset.structured_token, false)
            },
        ]);
    }
    assert_eq!(
        metas.len(),
        RATIONAL_BASE_ACCOUNT_COUNT_V2
            + usize::try_from(decoded_request.header().asset_count).expect("physical asset width")
                * RATIONAL_ASSET_ACCOUNT_COUNT_V2
    );
    if terminal {
        let terminal = fixture.terminal_accounts.expect("terminal fixture");
        metas.extend([
            AccountMeta::new_readonly(terminal.custody_caller, false),
            AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.custody_programdata, false),
            AccountMeta::new_readonly(terminal.coordinate_raw, false),
            AccountMeta::new_readonly(terminal.coordinate_staging, false),
            AccountMeta::new_readonly(terminal.realm_raw, false),
            AccountMeta::new_readonly(terminal.realm_staging, false),
            AccountMeta::new(terminal.custody_replay, false),
            AccountMeta::new_readonly(terminal.collateral_mint, false),
            AccountMeta::new(terminal.hoard, false),
            AccountMeta::new(terminal.recipient, false),
            AccountMeta::new_readonly(terminal.custody_authority, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
        ]);
        assert_eq!(
            metas.len(),
            RATIONAL_BASE_ACCOUNT_COUNT_V2
                + usize::try_from(decoded_request.header().asset_count)
                    .expect("physical asset width")
                    * RATIONAL_ASSET_ACCOUNT_COUNT_V2
                + RATIONAL_TERMINAL_ACCOUNT_COUNT_V2
        );
    }
    metas
}

fn wrapper_instruction(
    fixture: &Fixture,
    action: RepresentationActionV2,
    representation_revision: u64,
    fail_after: bool,
    descriptor_records: Option<(Pubkey, Pubkey)>,
    graph_records: Option<(Pubkey, Pubkey)>,
) -> Instruction {
    let request = request_bytes(fixture, action, representation_revision);
    let mut accounts = vec![AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false)];
    accounts.extend(claims_accounts(
        fixture,
        action,
        representation_revision,
        descriptor_records,
        graph_records,
    ));
    let mut data = Vec::with_capacity(request.len() + 1);
    data.push(u8::from(fail_after));
    data.extend_from_slice(&request);
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
    addresses: &[Pubkey],
) -> usize {
    let message = VersionedMessage::V0(
        v0::Message::try_compile(
            &payer,
            &[instruction],
            &[AddressLookupTableAccount {
                key: table,
                addresses: addresses.to_vec(),
            }],
            hash,
        )
        .expect("compressed v0"),
    );
    1 + 2 * 64 + message.serialize().len()
}

fn lookup_addresses(payer: Pubkey, actor: Pubkey, instructions: &[Instruction]) -> Vec<Pubkey> {
    let mut addresses = Vec::new();
    for instruction in instructions {
        if instruction.program_id != payer
            && instruction.program_id != actor
            && !addresses.contains(&instruction.program_id)
        {
            addresses.push(instruction.program_id);
        }
        for account in &instruction.accounts {
            if account.pubkey != payer
                && account.pubkey != actor
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
) -> (Pubkey, Vec<u64>) {
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
    let mut compute_units = vec![process_legacy(context, create).await];
    for chunk in addresses.chunks(20) {
        compute_units.push(
            process_legacy(
                context,
                extend_lookup_table(table, payer, Some(payer), chunk.to_vec()),
            )
            .await,
        );
    }
    let extension_clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("post-extension Clock");
    context
        .warp_to_slot(extension_clock.slot + 1)
        .expect("activate lookup addresses");
    (table, compute_units)
}

async fn submit_v0(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    instruction: Instruction,
    table: Pubkey,
    addresses: &[Pubkey],
) -> Result<Submission, BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let message = VersionedMessage::V0(
        v0::Message::try_compile(
            &context.payer.pubkey(),
            &[instruction],
            &[AddressLookupTableAccount {
                key: table,
                addresses: addresses.to_vec(),
            }],
            blockhash,
        )
        .expect("v0 message"),
    );
    let transaction = VersionedTransaction::try_new(message, &[&context.payer, &fixture.actor])
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
        replay: observed(context, fixture.representation_replay).await,
        aggregate: observed(context, fixture.aggregate).await,
        actor_position: observed(context, fixture.actor_position).await,
        positions: [
            observed(
                context,
                fixture.assets.first().expect("first asset").position,
            )
            .await,
            observed(
                context,
                fixture.assets.get(1).expect("second asset").position,
            )
            .await,
        ],
        receipt_mint: observed(context, fixture.receipt_mint).await,
        actor_receipt: observed(context, fixture.actor_receipt).await,
        shard_mints: [
            observed(context, fixture.assets.first().expect("first asset").mint).await,
            observed(context, fixture.assets.get(1).expect("second asset").mint).await,
        ],
        actor_shards: [
            observed(
                context,
                fixture.assets.first().expect("first asset").actor_token,
            )
            .await,
            observed(
                context,
                fixture.assets.get(1).expect("second asset").actor_token,
            )
            .await,
        ],
        structured_shards: [
            observed(
                context,
                fixture
                    .assets
                    .first()
                    .expect("first asset")
                    .structured_token,
            )
            .await,
            observed(
                context,
                fixture
                    .assets
                    .get(1)
                    .expect("second asset")
                    .structured_token,
            )
            .await,
        ],
        custody_replay: match fixture.terminal_accounts {
            Some(value) => Some(observed(context, value.custody_replay).await),
            None => None,
        },
        hoard: match fixture.terminal_accounts {
            Some(value) => Some(observed(context, value.hoard).await),
            None => None,
        },
        recipient: match fixture.terminal_accounts {
            Some(value) => Some(observed(context, value.recipient).await),
            None => None,
        },
    }
}

fn token_amount(account: &Account) -> u64 {
    TokenAccount::parse(&account.data)
        .expect("Token Account")
        .amount
}

fn mint_supply(account: &Account) -> u64 {
    SplMint::unpack(&account.data).expect("Mint").supply
}

fn assert_account_content_eq(actual: &Account, expected: &Account) {
    assert_eq!(actual.lamports, expected.lamports);
    assert_eq!(actual.owner, expected.owner);
    assert_eq!(actual.executable, expected.executable);
    assert_eq!(actual.data, expected.data);
}

fn replay_revision(account: &Account) -> u64 {
    assert_eq!(account.owner, CLAIMS_PROGRAM_ID);
    assert_eq!(account.data.len(), RATIONAL_REPLAY_BYTES_V2);
    assert_eq!(
        account.data.get(..8),
        Some(RATIONAL_REPLAY_MAGIC_V2.as_slice())
    );
    u64::from_le_bytes(
        account
            .data
            .get(80..88)
            .expect("replay revision")
            .try_into()
            .expect("revision width"),
    )
}

fn lbv2_revision(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(
        bytes
            .get(16..24)
            .expect("LBV2 revision")
            .try_into()
            .expect("LBV2 revision width"),
    )
}

fn lbv2_position_quantity(bytes: &[u8], outcome: u32) -> u64 {
    let index = usize::try_from(outcome).expect("outcome index");
    let offset = 128_usize
        .checked_add(index.checked_mul(8).expect("quantity offset"))
        .expect("quantity offset");
    u64::from_le_bytes(
        bytes
            .get(offset..offset + 8)
            .expect("LBV2 Position quantity")
            .try_into()
            .expect("LBV2 quantity width"),
    )
}

fn packet_measurements(
    payer: Pubkey,
    instruction: &Instruction,
    blockhash: Hash,
    table: Pubkey,
    addresses: &[Pubkey],
) -> (usize, usize, usize) {
    let legacy = legacy_wire_bytes(payer, instruction.clone(), blockhash);
    let no_alt = no_lookup_v0_wire_bytes(payer, instruction.clone(), blockhash);
    let live_alt =
        live_lookup_v0_wire_bytes(payer, instruction.clone(), blockhash, table, addresses);
    assert!(legacy > PACKET_LIMIT, "legacy must honestly overflow");
    assert!(
        no_alt > PACKET_LIMIT,
        "v0 without ALT must honestly overflow"
    );
    assert!(live_alt <= PACKET_LIMIT, "live ALT packet overflow");
    (legacy, no_alt, live_alt)
}

#[tokio::test]
async fn real_sbf_open_actions_are_exact_and_conserved() {
    let (test, fixture) = fixture(false);
    let mut context = test.start_with_context().await;
    let issue = wrapper_instruction(
        &fixture,
        RepresentationActionV2::IssueStructured,
        0,
        false,
        None,
        None,
    );
    let unwrap = wrapper_instruction(
        &fixture,
        RepresentationActionV2::UnwrapStructured,
        1,
        false,
        None,
        None,
    );
    let denominate = wrapper_instruction(
        &fixture,
        RepresentationActionV2::Denominate,
        2,
        false,
        None,
        None,
    );
    let reconstitute = wrapper_instruction(
        &fixture,
        RepresentationActionV2::Reconstitute,
        3,
        false,
        None,
        None,
    );
    assert_eq!(
        issue.accounts.len(),
        1 + RATIONAL_BASE_ACCOUNT_COUNT_V2
            + usize::try_from(OUTCOME_COUNT).expect("outcome width")
                * RATIONAL_ASSET_ACCOUNT_COUNT_V2
    );
    assert_eq!(
        issue.data.len(),
        1 + REQUEST_HEADER_BYTES_V2 + 2 * ASSET_BYTES_V2
    );
    let payer = context.payer.pubkey();
    let addresses = lookup_addresses(
        payer,
        fixture.actor.pubkey(),
        &[
            issue.clone(),
            unwrap.clone(),
            denominate.clone(),
            reconstitute.clone(),
        ],
    );
    let (table, lookup_cu) = create_live_lookup_table(&mut context, &addresses).await;
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("post-ALT blockhash");
    let (legacy, no_alt, live_alt) =
        packet_measurements(payer, &issue, blockhash, table, &addresses);
    eprintln!(
        "Rational V2 structured packet preflight: request={}, claims-frame={}, outer-metas={}, unique={}, legacy={}, v0-no-ALT={}, v0-live-ALT={}, ALT-CU={lookup_cu:?}",
        REQUEST_HEADER_BYTES_V2 + 2 * ASSET_BYTES_V2,
        RATIONAL_BASE_ACCOUNT_COUNT_V2
            + usize::try_from(OUTCOME_COUNT).expect("outcome width")
                * RATIONAL_ASSET_ACCOUNT_COUNT_V2,
        issue.accounts.len(),
        unique_account_count(&issue),
        legacy,
        no_alt,
        live_alt,
    );
    let before = snapshot(&mut context, &fixture).await;

    let issued = submit_v0(&mut context, &fixture, issue.clone(), table, &addresses)
        .await
        .expect("IssueStructured transaction");
    assert!(issued.accepted, "IssueStructured must commit");
    assert!(
        issued.wire_bytes <= PACKET_LIMIT,
        "IssueStructured packet overflow"
    );
    assert!(
        issued
            .logs
            .iter()
            .any(|log| log == &format!("Program {TOKEN_PROGRAM_ID} success")),
        "real Token-2022 must execute"
    );
    let after_issue = snapshot(&mut context, &fixture).await;
    assert_eq!(after_issue.aggregate, before.aggregate);
    assert_eq!(after_issue.actor_position, before.actor_position);
    assert_eq!(after_issue.positions, before.positions);
    assert_eq!(replay_revision(&after_issue.replay), 1);
    assert_eq!(mint_supply(&after_issue.receipt_mint), RECEIPT_SUPPLY + 1);
    assert_eq!(token_amount(&after_issue.actor_receipt), 1);
    for (index, (actor, structured)) in [(6_u64, 24_u64), (14, 56)].into_iter().enumerate() {
        let supply = mint_supply(after_issue.shard_mints.get(index).expect("shard Mint"));
        assert_eq!(
            supply,
            *SHARD_SUPPLIES.get(index).expect("expected shard supply")
        );
        assert_eq!(
            token_amount(after_issue.actor_shards.get(index).expect("actor shards")),
            actor
        );
        assert_eq!(
            token_amount(
                after_issue
                    .structured_shards
                    .get(index)
                    .expect("structured shards"),
            ),
            structured
        );
        assert_eq!(actor + structured, supply, "no hidden shard remainder");
    }

    let unwrapped = submit_v0(&mut context, &fixture, unwrap, table, &addresses)
        .await
        .expect("UnwrapStructured transaction");
    if !unwrapped.accepted {
        eprintln!(
            "UnwrapStructured refusal logs:\n{}",
            unwrapped.logs.join("\n")
        );
    }
    assert!(unwrapped.accepted, "UnwrapStructured must commit");
    assert!(
        unwrapped.wire_bytes <= PACKET_LIMIT,
        "UnwrapStructured packet overflow"
    );
    let after_unwrap = snapshot(&mut context, &fixture).await;
    assert_eq!(replay_revision(&after_unwrap.replay), 2);
    assert_eq!(after_unwrap.aggregate, before.aggregate);
    assert_eq!(after_unwrap.actor_position, before.actor_position);
    assert_eq!(after_unwrap.positions, before.positions);
    assert_account_content_eq(&after_unwrap.receipt_mint, &before.receipt_mint);
    assert_account_content_eq(&after_unwrap.actor_receipt, &before.actor_receipt);
    for (actual, expected) in after_unwrap.shard_mints.iter().zip(&before.shard_mints) {
        assert_account_content_eq(actual, expected);
    }
    for (actual, expected) in after_unwrap.actor_shards.iter().zip(&before.actor_shards) {
        assert_account_content_eq(actual, expected);
    }
    for (actual, expected) in after_unwrap
        .structured_shards
        .iter()
        .zip(&before.structured_shards)
    {
        assert_account_content_eq(actual, expected);
    }

    let denominated = submit_v0(&mut context, &fixture, denominate, table, &addresses)
        .await
        .expect("Denominate transaction");
    if !denominated.accepted {
        eprintln!("Denominate refusal logs:\n{}", denominated.logs.join("\n"));
    }
    assert!(denominated.accepted, "Denominate must commit");
    let after_denominate = snapshot(&mut context, &fixture).await;
    assert_eq!(replay_revision(&after_denominate.replay), 3);
    assert_eq!(lbv2_revision(&after_denominate.aggregate.data), 1);
    assert_eq!(lbv2_revision(&after_denominate.actor_position.data), 1);
    assert_eq!(
        lbv2_position_quantity(&after_denominate.actor_position.data, WINNER),
        1
    );
    assert_eq!(lbv2_revision(&after_denominate.positions[1].data), 1);
    assert_eq!(
        lbv2_position_quantity(&after_denominate.positions[1].data, WINNER),
        8
    );
    assert_eq!(mint_supply(&after_denominate.shard_mints[1]), 80);
    assert_eq!(token_amount(&after_denominate.actor_shards[1]), 31);

    let reconstituted = submit_v0(&mut context, &fixture, reconstitute, table, &addresses)
        .await
        .expect("Reconstitute transaction");
    if !reconstituted.accepted {
        eprintln!(
            "Reconstitute refusal logs:\n{}",
            reconstituted.logs.join("\n")
        );
    }
    assert!(reconstituted.accepted, "Reconstitute must commit");
    let after_reconstitute = snapshot(&mut context, &fixture).await;
    assert_eq!(replay_revision(&after_reconstitute.replay), 4);
    assert_eq!(lbv2_revision(&after_reconstitute.aggregate.data), 2);
    assert_eq!(lbv2_revision(&after_reconstitute.actor_position.data), 2);
    assert_eq!(
        lbv2_position_quantity(&after_reconstitute.actor_position.data, WINNER),
        2
    );
    assert_eq!(lbv2_revision(&after_reconstitute.positions[1].data), 2);
    assert_eq!(
        lbv2_position_quantity(&after_reconstitute.positions[1].data, WINNER),
        7
    );
    for (actual, expected) in after_reconstitute
        .shard_mints
        .iter()
        .zip(&before.shard_mints)
    {
        assert_account_content_eq(actual, expected);
    }
    for (actual, expected) in after_reconstitute
        .actor_shards
        .iter()
        .zip(&before.actor_shards)
    {
        assert_account_content_eq(actual, expected);
    }
    eprintln!(
        "Rational V2 open: request={}, claims-frame={}, outer-metas={}, unique={}, legacy={}, v0-no-ALT={}, v0-live-ALT={}, issue-v0={}, issue-CU={}, unwrap-v0={}, unwrap-CU={}, denominate-v0={}, denominate-CU={}, reconstitute-v0={}, reconstitute-CU={}, ALT-CU={lookup_cu:?}",
        REQUEST_HEADER_BYTES_V2 + 2 * ASSET_BYTES_V2,
        RATIONAL_BASE_ACCOUNT_COUNT_V2
            + usize::try_from(OUTCOME_COUNT).expect("outcome width")
                * RATIONAL_ASSET_ACCOUNT_COUNT_V2,
        issue.accounts.len(),
        unique_account_count(&issue),
        legacy,
        no_alt,
        live_alt,
        issued.wire_bytes,
        issued.compute_units,
        unwrapped.wire_bytes,
        unwrapped.compute_units,
        denominated.wire_bytes,
        denominated.compute_units,
        reconstituted.wire_bytes,
        reconstituted.compute_units,
    );
}

#[tokio::test]
async fn real_sbf_terminal_hostile_joins_and_late_child_failure_are_atomic() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    let positive = wrapper_instruction(
        &fixture,
        RepresentationActionV2::RedeemTerminal,
        0,
        false,
        None,
        None,
    );
    let late = wrapper_instruction(
        &fixture,
        RepresentationActionV2::RedeemTerminal,
        0,
        true,
        None,
        None,
    );
    let descriptor_substitution = wrapper_instruction(
        &fixture,
        RepresentationActionV2::RedeemTerminal,
        0,
        false,
        Some((
            fixture.alternate_descriptor_raw,
            fixture.alternate_descriptor_staging,
        )),
        None,
    );
    let graph_substitution = wrapper_instruction(
        &fixture,
        RepresentationActionV2::RedeemTerminal,
        0,
        false,
        None,
        Some((fixture.alternate_graph_raw, fixture.alternate_graph_staging)),
    );
    let expected_claims_accounts = RATIONAL_BASE_ACCOUNT_COUNT_V2
        + RATIONAL_ASSET_ACCOUNT_COUNT_V2
        + RATIONAL_TERMINAL_ACCOUNT_COUNT_V2;
    assert_eq!(positive.accounts.len(), 1 + expected_claims_accounts);
    assert_eq!(
        positive.data.len(),
        1 + REQUEST_HEADER_BYTES_V2 + ASSET_BYTES_V2
    );
    let payer = context.payer.pubkey();
    let instructions = [
        positive.clone(),
        late.clone(),
        descriptor_substitution.clone(),
        graph_substitution.clone(),
    ];
    let addresses = lookup_addresses(payer, fixture.actor.pubkey(), &instructions);
    let (table, lookup_cu) = create_live_lookup_table(&mut context, &addresses).await;
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("post-ALT blockhash");
    let (legacy, no_alt, live_alt) =
        packet_measurements(payer, &positive, blockhash, table, &addresses);
    eprintln!(
        "Rational V2 terminal packet preflight: request={}, claims-frame={}, outer-metas={}, unique={}, legacy={}, v0-no-ALT={}, v0-live-ALT={}, ALT-CU={lookup_cu:?}",
        REQUEST_HEADER_BYTES_V2 + ASSET_BYTES_V2,
        expected_claims_accounts,
        positive.accounts.len(),
        unique_account_count(&positive),
        legacy,
        no_alt,
        live_alt,
    );
    let before = snapshot(&mut context, &fixture).await;

    for (label, hostile) in [
        ("same-width descriptor", descriptor_substitution),
        ("same-width graph", graph_substitution),
    ] {
        let result = submit_v0(&mut context, &fixture, hostile, table, &addresses)
            .await
            .expect("hostile substitution transaction");
        assert!(!result.accepted, "{label} substitution must refuse");
        assert_eq!(
            snapshot(&mut context, &fixture).await,
            before,
            "{label} refusal must be byte-exact rollback"
        );
    }

    let late_result = submit_v0(&mut context, &fixture, late, table, &addresses)
        .await
        .expect("late rollback transaction");
    if !late_result.accepted {
        eprintln!(
            "Terminal late-refusal logs:\n{}",
            late_result.logs.join("\n")
        );
    }
    assert!(
        !late_result.accepted,
        "late wrapper must deliberately refuse"
    );
    assert!(
        late_result.wire_bytes <= PACKET_LIMIT,
        "late packet overflow"
    );
    assert!(
        late_result
            .logs
            .iter()
            .any(|log| log == &format!("Program {CUSTODY_PROGRAM_ID} success")),
        "real Custody must return before the late refusal"
    );
    assert!(
        late_result
            .logs
            .iter()
            .any(|log| log == &format!("Program {CLAIMS_PROGRAM_ID} success")),
        "real Claims must return before the late refusal"
    );
    assert_eq!(
        snapshot(&mut context, &fixture).await,
        before,
        "late refusal must roll back rational replay, Claims, Token-2022, and Custody"
    );

    let accepted = submit_v0(&mut context, &fixture, positive.clone(), table, &addresses)
        .await
        .expect("positive terminal transaction");
    assert!(accepted.accepted, "terminal composition must commit");
    assert!(
        accepted.wire_bytes <= PACKET_LIMIT,
        "terminal packet overflow"
    );
    let after = snapshot(&mut context, &fixture).await;
    assert_eq!(replay_revision(&after.replay), 1);
    assert_eq!(lbv2_revision(&after.aggregate.data), 1);
    assert_eq!(
        lbv2_revision(&after.positions.first().expect("first Position").data),
        0
    );
    assert_eq!(
        lbv2_revision(&after.positions.get(1).expect("second Position").data),
        1
    );
    assert_eq!(
        lbv2_position_quantity(
            &after.positions.get(1).expect("second Position").data,
            WINNER,
        ),
        6
    );
    assert_eq!(
        mint_supply(after.shard_mints.get(1).expect("winner Mint")),
        60
    );
    assert_eq!(
        token_amount(after.actor_shards.get(1).expect("winner actor shards")),
        11
    );
    assert_eq!(
        token_amount(
            after
                .structured_shards
                .get(1)
                .expect("winner structured shards"),
        ),
        49
    );
    assert_eq!(
        11 + 49,
        mint_supply(after.shard_mints.get(1).expect("winner Mint"))
    );
    assert_eq!(mint_supply(&after.receipt_mint), RECEIPT_SUPPLY);
    let custody_replay = after.custody_replay.as_ref().expect("Custody replay");
    assert_eq!(
        CustodyReplayV1::decode(&custody_replay.data)
            .expect("post Custody replay")
            .next_revision,
        CUSTODY_EXPECTED_REVISION + 1
    );
    assert_eq!(
        token_amount(after.hoard.as_ref().expect("Hoard")),
        INITIAL_HOARD_ATOMS - 1,
        "Custody Hoard principal must pay exactly one atom without violating terminal solvency"
    );
    assert_eq!(
        token_amount(after.recipient.as_ref().expect("recipient")),
        INITIAL_RECIPIENT_ATOMS + 1
    );
    eprintln!(
        "Rational V2 terminal: request={}, claims-frame={}, outer-metas={}, unique={}, legacy={}, v0-no-ALT={}, v0-live-ALT={}, positive-v0={}, positive-CU={}, late-v0={}, late-CU={}, ALT-CU={lookup_cu:?}",
        REQUEST_HEADER_BYTES_V2 + ASSET_BYTES_V2,
        expected_claims_accounts,
        positive.accounts.len(),
        unique_account_count(&positive),
        legacy,
        no_alt,
        live_alt,
        accepted.wire_bytes,
        accepted.compute_units,
        late_result.wire_bytes,
        late_result.compute_units,
    );
}
