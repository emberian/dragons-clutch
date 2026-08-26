//! Hostile chain-observation and exact-frame tests for the Rational V2 operator.

use dclutch_claims_svm::{ClaimsAggregateSeedsV1, ClaimsPositionSeedsV1, NO_POSITION_REVISION};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CUSTODY_REPLAY_BYTES_V1, CallerRoleV1 as CustodyCallerRoleV1, CompartmentV1, ContextV1,
    CustodyAuthoritySeedsV1, CustodyReplaySeedsV1, CustodyReplayV1, CustodyRequestV1,
    CustodyVaultSeedsV1, OperationV1,
};
use dclutch_economic_slice_kernel::{
    BasketAction, BasketFrame, MARKET_HEADER_BYTES, POSITION_HEADER_BYTES, Phase as EconomicPhase,
    SCALAR_BYTES, execute_basket, initialize_market, initialize_position,
};
use dclutch_market_core_codec::{
    CoreState, Identity, MarketCoreStateSeedsV2, MarketIdentity, Phase as CorePhase, Readiness,
};
use dclutch_rational_representation_v2_contract::{
    ASSET_BYTES_V2, AssetV2, CallerRoleV2, RATIONAL_REPLAY_SEED_V2,
    RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2, RATIONAL_SHARD_MINT_SEED_V2, RepresentationActionV2,
    RepresentationRequestHeaderV2, RepresentationRequestV2,
};
use dclutch_rational_representation_v2_kernel::{
    DESCRIPTOR_COEFFICIENT_BYTES, DESCRIPTOR_HEADER_BYTES, DESCRIPTOR_MAGIC_V3, GRAPH_EDGE_BYTES,
    GRAPH_HEADER_BYTES, GRAPH_MAGIC_V2, GRAPH_NODE_BYTES,
    REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3, REPRESENTATION_GRAPH_SCHEMA_RELEASE_ID_V2,
    SCHEMA_VERSION_V2,
};
use dclutch_rational_representation_v2_operator::{
    AssetObservationV2, Error, FinalizedRecordObservationV2, ObservedAccountV2,
    ProductEvidenceObservationV2, RationalObservationV2, ReplayObservationV2,
    SelectedActionInputV2, StructuredActionInputV2, TerminalObservationV2, construct_denominate,
    construct_issue_structured, construct_reconstitute, construct_redeem_terminal,
    construct_unwrap_structured,
};
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1, RealmV1Input,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1, ArtifactActivationInputV1,
    ArtifactReleaseV1, ArtifactUpgradePolicyV1, DeploymentObservationV1,
    activate_execution_role_into_v1, initialize_activation_cache_v1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1,
    ProgramIdentityV1,
};
use dclutch_token_svm::TOKEN_2022_PROGRAM_ID;
use solana_program::{hash::hash, instruction::AccountMeta, pubkey::Pubkey, rent::Rent};
use solana_program_option::COption;
use solana_program_pack::Pack;
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use spl_associated_token_account_interface::address::get_associated_token_address_with_program_id;
use spl_token_interface::state::{Account as SplAccount, AccountState, Mint as SplMint};

const CLAIMS: Pubkey = Pubkey::new_from_array([0xe1; 32]);
const CUSTODY: Pubkey = Pubkey::new_from_array([0xe2; 32]);
const REGISTRY: Pubkey = Pubkey::new_from_array([0xe3; 32]);
const CORE: Pubkey = Pubkey::new_from_array([0xe4; 32]);
const TRADING: Pubkey = Pubkey::new_from_array([0xe5; 32]);
const TOKEN: Pubkey = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);
const ACTOR: Pubkey = Pubkey::new_from_array([0x71; 32]);
const GENERATION: u64 = 29;
const OUTCOME_COUNT: u32 = 2;
const WINNER: u32 = 1;
const DENOMINATOR: u64 = 10;
const RECEIPT_SUPPLY: u64 = 7;
const COEFFICIENTS: [u64; 2] = [3, 7];
const SHARD_SUPPLIES: [u64; 2] = [30, 70];
const ACTOR_SHARDS: [u64; 2] = [9, 21];
const STRUCTURED_SHARDS: [u64; 2] = [21, 49];
const REPRESENTATION_REVISION: u64 = 2;
const CUSTODY_REVISION: u64 = 8;
const PARENT_CONTEXT: [u8; 32] = [0x68; 32];

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

fn identity(value: [u8; 32]) -> Identity {
    Identity::new(value).expect("nonzero identity")
}

fn program_identity(key: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(key.to_bytes()).expect("nonzero program")
}

fn programdata(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn release(program: Pubkey, seed: u8) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        program_identity(program),
        program_identity(bpf_loader_upgradeable::ID),
        programdata(program).to_bytes(),
        ContentId::new([seed; 32]).expect("semantic release"),
        [seed.wrapping_add(1); 32],
        0,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("artifact release")
}

fn artifact_id(value: ArtifactReleaseV1) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(hash(&value.to_bytes()).to_bytes()).expect("artifact id")
}

fn activation_input(value: ArtifactReleaseV1) -> ArtifactActivationInputV1 {
    let observation = DeploymentObservationV1::new(
        value.program().to_bytes(),
        bpf_loader_upgradeable::ID.to_bytes(),
        true,
        value.programdata(),
        bpf_loader_upgradeable::ID.to_bytes(),
        false,
        value.programdata(),
        bpf_loader_upgradeable::ID.to_bytes(),
        value.deployment_slot(),
        value.elf_digest(),
        value.upgrade_authority(),
    )
    .expect("deployment observation");
    ArtifactActivationInputV1::new(artifact_id(value), value, observation)
}

fn activation_cache() -> ([u8; 32], Vec<u8>) {
    let core = release(CORE, 0x41);
    let claims = release(CLAIMS, 0x42);
    let custody = release(CUSTODY, 0x43);
    let trading = release(TRADING, 0x44);
    let release_set = ExecutionReleaseSetV1::new(
        ExecutionRoleBindingV1::new(core.program(), artifact_id(core)),
        ExecutionRoleBindingV1::new(claims.program(), artifact_id(claims)),
        ExecutionRoleBindingV1::new(trading.program(), artifact_id(trading)),
        ExecutionRoleBindingV1::new(claims.program(), artifact_id(claims)),
        ExecutionRoleBindingV1::new(custody.program(), artifact_id(custody)),
    )
    .expect("release set");
    let release_set_id = hash(&release_set.to_bytes()).to_bytes();
    let content = ContentId::new(release_set_id).expect("release-set identity");
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
    (release_set_id, bytes)
}

#[derive(Clone)]
struct OwnedRecord {
    schema: [u8; 32],
    raw: Pubkey,
    staging: Pubkey,
    bytes: Vec<u8>,
}

impl OwnedRecord {
    fn new(schema: [u8; 32], bytes: Vec<u8>) -> Self {
        let digest = hash(&bytes).to_bytes();
        let raw = Pubkey::find_program_address(
            &[RAW_RECORD_PDA_SEED_V1, schema.as_slice(), digest.as_slice()],
            &REGISTRY,
        )
        .0;
        let staging = Pubkey::find_program_address(
            &[
                STAGING_CURSOR_PDA_SEED_V1,
                schema.as_slice(),
                digest.as_slice(),
            ],
            &REGISTRY,
        )
        .0;
        Self {
            schema,
            raw,
            staging,
            bytes,
        }
    }

    fn digest(&self) -> [u8; 32] {
        hash(&self.bytes).to_bytes()
    }

    fn observe(&self, rent: &Rent) -> FinalizedRecordObservationV2<'_> {
        FinalizedRecordObservationV2 {
            schema_id: self.schema,
            raw: ObservedAccountV2 {
                key: self.raw,
                owner: REGISTRY,
                lamports: rent.minimum_balance(self.bytes.len()),
                executable: false,
                data: &self.bytes,
            },
            staging: ObservedAccountV2 {
                key: self.staging,
                owner: system_program::ID,
                lamports: 1,
                executable: false,
                data: &[],
            },
        }
    }
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
    let multiplicities = if changed { [4, 6] } else { COEFFICIENTS };
    let first = *multiplicities.first().expect("first multiplicity");
    let second = *multiplicities.get(1).expect("second multiplicity");
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
            id: [0x45; 32],
            rank: 2,
            first_edge: 2,
            edge_count: 2,
            kind: 2,
            parameter: 0,
            exposure: [first * 10, second * 10],
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
            multiplicity: first,
        },
        GraphEdge {
            child_id: [0x44; 32],
            child_index: 3,
            multiplicity: second,
        },
    ];
    let mut output = vec![
        0;
        GRAPH_HEADER_BYTES
            + nodes.len() * GRAPH_NODE_BYTES
            + edges.len() * GRAPH_EDGE_BYTES
            + nodes.len() * 2 * SCALAR_BYTES
    ];
    put(&mut output, 0, &GRAPH_MAGIC_V2);
    put(&mut output, 8, &SCHEMA_VERSION_V2.to_le_bytes());
    put(&mut output, 16, &[0x31; 32]);
    put(&mut output, 48, &[0x45; 32]);
    put_u32(&mut output, 80, OUTCOME_COUNT);
    put_u32(
        &mut output,
        84,
        u32::try_from(nodes.len()).expect("node width"),
    );
    put_u32(
        &mut output,
        88,
        u32::try_from(edges.len()).expect("edge width"),
    );
    put_u64(&mut output, 96, 100);
    for (index, node) in nodes.iter().enumerate() {
        let offset = GRAPH_HEADER_BYTES + index * GRAPH_NODE_BYTES;
        put(&mut output, offset, &node.id);
        put_u32(&mut output, offset + 32, node.rank);
        put_u32(&mut output, offset + 36, node.first_edge);
        put_u32(&mut output, offset + 40, node.edge_count);
        *output.get_mut(offset + 44).expect("node kind") = node.kind;
        put_u64(&mut output, offset + 48, node.parameter);
    }
    let edge_start = GRAPH_HEADER_BYTES + nodes.len() * GRAPH_NODE_BYTES;
    for (index, edge) in edges.iter().enumerate() {
        let offset = edge_start + index * GRAPH_EDGE_BYTES;
        put(&mut output, offset, &edge.child_id);
        put_u32(&mut output, offset + 32, edge.child_index);
        put_u64(&mut output, offset + 40, edge.multiplicity);
    }
    let exposure_start = edge_start + edges.len() * GRAPH_EDGE_BYTES;
    for (node_index, node) in nodes.iter().enumerate() {
        for (outcome, amount) in node.exposure.iter().enumerate() {
            put_u64(
                &mut output,
                exposure_start + (node_index * 2 + outcome) * SCALAR_BYTES,
                *amount,
            );
        }
    }
    output
}

fn descriptor_bytes(
    graph_digest: [u8; 32],
    market: Pubkey,
    release_set: [u8; 32],
    receipt_mint: Pubkey,
    coefficients: [u64; 2],
) -> Vec<u8> {
    let mut output = vec![0; DESCRIPTOR_HEADER_BYTES + 2 * DESCRIPTOR_COEFFICIENT_BYTES];
    put(&mut output, 0, &DESCRIPTOR_MAGIC_V3);
    put(&mut output, 8, &3_u16.to_le_bytes());
    put(&mut output, 16, &[0x31; 32]);
    put(&mut output, 48, &graph_digest);
    put(&mut output, 80, &[0x45; 32]);
    put(&mut output, 112, market.as_ref());
    put(&mut output, 144, &release_set);
    put(&mut output, 176, receipt_mint.as_ref());
    put(&mut output, 208, &TOKEN_2022_PROGRAM_ID);
    put_u32(&mut output, 240, OUTCOME_COUNT);
    put_u64(&mut output, 248, DENOMINATOR);
    for (index, coefficient) in coefficients.iter().enumerate() {
        put_u64(
            &mut output,
            DESCRIPTOR_HEADER_BYTES + index * DESCRIPTOR_COEFFICIENT_BYTES,
            *coefficient,
        );
    }
    output
}

fn core_market(release_set: [u8; 32], realm: [u8; 32], terminal: bool) -> (Pubkey, Vec<u8>) {
    let mut market_identity = MarketIdentity {
        market_id: identity([1; 32]),
        realm_id: identity(realm),
        product_record: identity([0x60; 32]),
        product_id: identity([0x61; 32]),
        resolution_policy: identity([0x63; 32]),
        capability_manifest: identity([0x64; 32]),
        selected_release_set: identity(release_set),
        registry_program: identity(REGISTRY.to_bytes()),
        generation: GENERATION,
    };
    let market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(market_identity).as_slices(),
        &CORE,
    )
    .0;
    market_identity.market_id = identity(market.to_bytes());
    let state = CoreState {
        phase: if terminal {
            CorePhase::Terminal
        } else {
            CorePhase::Open
        },
        readiness: Readiness::Consumed,
        terminal_winner: if terminal { WINNER } else { 0 },
        identity: market_identity,
        outstanding_capabilities: 1,
        rent_beneficiary: identity([0x65; 32]),
        terminal_receipt: terminal.then(|| identity([0x66; 32])),
    };
    (market, state.encode().expect("Core state").to_vec())
}

fn economic_data(
    market: Pubkey,
    release_set: [u8; 32],
    custody_owners: [Pubkey; 2],
    terminal: bool,
) -> (Vec<u8>, [Vec<u8>; 2], Vec<u8>) {
    let width = usize::try_from(OUTCOME_COUNT).expect("small width");
    let mut aggregate = vec![0; MARKET_HEADER_BYTES + width * 3 * SCALAR_BYTES];
    let mut bootstrap = vec![0; POSITION_HEADER_BYTES + width * 2 * SCALAR_BYTES];
    let mut custody_positions = [
        vec![0; POSITION_HEADER_BYTES + width * 2 * SCALAR_BYTES],
        vec![0; POSITION_HEADER_BYTES + width * 2 * SCALAR_BYTES],
    ];
    let mut actor_position = vec![0; POSITION_HEADER_BYTES + width * 2 * SCALAR_BYTES];
    initialize_market(
        &mut aggregate,
        market.to_bytes(),
        release_set,
        REGISTRY.to_bytes(),
        OUTCOME_COUNT,
        EconomicPhase::Open,
        0,
    )
    .expect("Claims aggregate");
    initialize_position(&mut bootstrap, market.to_bytes(), [0x67; 32], OUTCOME_COUNT)
        .expect("bootstrap Position");
    initialize_position(
        &mut actor_position,
        market.to_bytes(),
        ACTOR.to_bytes(),
        OUTCOME_COUNT,
    )
    .expect("actor Position");
    for (position, owner) in custody_positions.iter_mut().zip(custody_owners) {
        initialize_position(position, market.to_bytes(), owner.to_bytes(), OUTCOME_COUNT)
            .expect("custody Position");
    }
    let complete = [7_u64, 7]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect::<Vec<_>>();
    execute_basket(
        &mut aggregate,
        None,
        Some(&mut bootstrap),
        BasketFrame {
            expected_market_revision: 0,
            expected_source_revision: None,
            expected_destination_revision: Some(0),
            action: BasketAction::MintCompleteSet,
            quantities: &complete,
            quantity_multiplier: 1,
        },
    )
    .expect("complete-set funding");
    for (index, quantities) in [[3_u64, 0], [0_u64, 7]].into_iter().enumerate() {
        let bytes = quantities
            .into_iter()
            .flat_map(u64::to_le_bytes)
            .collect::<Vec<_>>();
        execute_basket(
            &mut aggregate,
            Some(&mut bootstrap),
            Some(custody_positions.get_mut(index).expect("custody Position")),
            BasketFrame {
                expected_market_revision: 1 + u64::try_from(index).expect("revision"),
                expected_source_revision: Some(1 + u64::try_from(index).expect("revision")),
                expected_destination_revision: Some(0),
                action: BasketAction::Materialize,
                quantities: &bytes,
                quantity_multiplier: 1,
            },
        )
        .expect("materialize coefficient");
    }
    if terminal {
        *aggregate.get_mut(10).expect("phase byte") = 1;
        put_u32(&mut aggregate, 20, WINNER);
    }
    (aggregate, custody_positions, actor_position)
}

fn mint_data(authority: COption<Pubkey>, supply: u64, decimals: u8) -> Vec<u8> {
    let mut output = vec![0; SplMint::LEN];
    SplMint::pack(
        SplMint {
            mint_authority: authority,
            supply,
            decimals,
            is_initialized: true,
            freeze_authority: COption::None,
        },
        &mut output,
    )
    .expect("Mint bytes");
    output
}

fn token_data(mint: Pubkey, owner: Pubkey, amount: u64) -> Vec<u8> {
    let mut output = vec![0; SplAccount::LEN];
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
        &mut output,
    )
    .expect("Token Account bytes");
    output
}

#[derive(Clone)]
struct OwnedAsset {
    outcome: u32,
    custody_position_key: Pubkey,
    custody_position: Vec<u8>,
    shard_mint_key: Pubkey,
    shard_mint: Vec<u8>,
    actor_shard_key: Pubkey,
    actor_shard: Vec<u8>,
    structured_key: Pubkey,
    structured: Vec<u8>,
}

impl OwnedAsset {
    fn observe(&self) -> AssetObservationV2<'_> {
        AssetObservationV2 {
            outcome: self.outcome,
            claims_custody_position: observed(
                self.custody_position_key,
                CLAIMS,
                &self.custody_position,
            ),
            shard_mint: observed(self.shard_mint_key, TOKEN, &self.shard_mint),
            actor_shard_account: observed(self.actor_shard_key, TOKEN, &self.actor_shard),
            structured_custody_account: observed(self.structured_key, TOKEN, &self.structured),
        }
    }
}

#[derive(Clone)]
struct OwnedTerminal {
    terminal_coordinate: OwnedRecord,
    realm: OwnedRecord,
    collateral_mint_key: Pubkey,
    collateral_mint: Vec<u8>,
    custody_replay_key: Pubkey,
    custody_replay: Vec<u8>,
    hoard_key: Pubkey,
    hoard: Vec<u8>,
    recipient_key: Pubkey,
    recipient: Vec<u8>,
    custody_caller: Pubkey,
    custody_authority: Pubkey,
}

#[derive(Clone)]
struct Fixture {
    rent: Rent,
    release_set: [u8; 32],
    activation_cache_key: Pubkey,
    activation_cache: Vec<u8>,
    market: Pubkey,
    core: Vec<u8>,
    aggregate_key: Pubkey,
    aggregate: Vec<u8>,
    actor_position_key: Pubkey,
    actor_position: Vec<u8>,
    graph: OwnedRecord,
    alternate_graph: OwnedRecord,
    descriptor: OwnedRecord,
    alternate_descriptor: OwnedRecord,
    linked_basis: OwnedRecord,
    product_record: OwnedRecord,
    result_domain_record: OwnedRecord,
    portfolio_record: OwnedRecord,
    descriptor_id: [u8; 32],
    representation_authority: Pubkey,
    replay_key: Pubkey,
    replay: Vec<u8>,
    receipt_mint_key: Pubkey,
    receipt_mint: Vec<u8>,
    actor_receipt_key: Pubkey,
    actor_receipt: Vec<u8>,
    assets: [OwnedAsset; 2],
    terminal: Option<OwnedTerminal>,
}

fn observed<'a>(key: Pubkey, owner: Pubkey, data: &'a [u8]) -> ObservedAccountV2<'a> {
    ObservedAccountV2 {
        key,
        owner,
        lamports: 1,
        executable: false,
        data,
    }
}

impl Fixture {
    fn new(terminal: bool) -> Self {
        let rent = Rent::default();
        let (release_set, activation_cache) = activation_cache();
        let activation_cache_key =
            Pubkey::find_program_address(&[ACTIVATION_PDA_DOMAIN_V1, &release_set], &REGISTRY).0;
        let collateral_mint_key = Pubkey::new_from_array([0x74; 32]);
        let realm = RealmV1::new(RealmV1Input {
            token_program: TOKEN_2022_PROGRAM_ID,
            collateral_mint: collateral_mint_key.to_bytes(),
            collateral_adapter_release_id: [0x73; 32],
            mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
            freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
        })
        .expect("Realm");
        let realm_record = OwnedRecord::new(REALM_SCHEMA_RELEASE_ID_V1, realm.to_bytes().to_vec());
        let (market, core) = core_market(release_set, realm_record.digest(), terminal);
        let receipt_mint_key = Pubkey::new_from_array([0x75; 32]);
        let graph = OwnedRecord::new(
            REPRESENTATION_GRAPH_SCHEMA_RELEASE_ID_V2,
            graph_bytes(false),
        );
        let alternate_graph =
            OwnedRecord::new(REPRESENTATION_GRAPH_SCHEMA_RELEASE_ID_V2, graph_bytes(true));
        let descriptor = OwnedRecord::new(
            REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
            descriptor_bytes(
                graph.digest(),
                market,
                release_set,
                receipt_mint_key,
                COEFFICIENTS,
            ),
        );
        let alternate_descriptor = OwnedRecord::new(
            REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
            descriptor_bytes(
                graph.digest(),
                market,
                release_set,
                receipt_mint_key,
                [4, 6],
            ),
        );
        let descriptor_id = descriptor.digest();
        let linked_basis = OwnedRecord::new([0xa1; 32], vec![0xb1; 32]);
        let product_record = OwnedRecord::new([0xa2; 32], vec![0xb2; 32]);
        let result_domain_record = OwnedRecord::new([0xa3; 32], vec![0xb3; 32]);
        let portfolio_record = OwnedRecord::new([0xa4; 32], vec![0xb4; 32]);
        let representation_authority = Pubkey::find_program_address(
            &[
                RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
                descriptor_id.as_slice(),
            ],
            &CLAIMS,
        )
        .0;
        let identities: [(u32, Pubkey, Pubkey, Pubkey, Pubkey, Pubkey); 2] = std::array::from_fn(
            |index| {
                let outcome = u32::try_from(index).expect("outcome");
                let outcome_bytes = outcome.to_le_bytes();
                let shard_mint = Pubkey::find_program_address(
                    &[
                        RATIONAL_SHARD_MINT_SEED_V2,
                        descriptor_id.as_slice(),
                        &outcome_bytes,
                    ],
                    &CLAIMS,
                )
                .0;
                let custody_owner = Pubkey::find_program_address(
                &[
                    dclutch_rational_representation_v2_contract::RATIONAL_CLAIMS_CUSTODY_OWNER_SEED_V2,
                    descriptor_id.as_slice(),
                    &outcome_bytes,
                ],
                &CLAIMS,
            )
            .0;
                let position_key = Pubkey::find_program_address(
                    &ClaimsPositionSeedsV1::new(market.to_bytes(), custody_owner.to_bytes())
                        .expect("custody Position seeds")
                        .as_slices(),
                    &CLAIMS,
                )
                .0;
                let actor_shard =
                    get_associated_token_address_with_program_id(&ACTOR, &shard_mint, &TOKEN);
                let structured = get_associated_token_address_with_program_id(
                    &representation_authority,
                    &shard_mint,
                    &TOKEN,
                );
                (
                    outcome,
                    custody_owner,
                    position_key,
                    shard_mint,
                    actor_shard,
                    structured,
                )
            },
        );
        let (aggregate, positions, actor_position) = economic_data(
            market,
            release_set,
            [
                identities.first().expect("first identity").1,
                identities.get(1).expect("second identity").1,
            ],
            terminal,
        );
        let assets = std::array::from_fn(|index| {
            let value = identities.get(index).copied().expect("asset identity");
            OwnedAsset {
                outcome: value.0,
                custody_position_key: value.2,
                custody_position: positions.get(index).expect("custody Position").clone(),
                shard_mint_key: value.3,
                shard_mint: mint_data(
                    COption::Some(representation_authority),
                    *SHARD_SUPPLIES.get(index).expect("shard supply"),
                    0,
                ),
                actor_shard_key: value.4,
                actor_shard: token_data(
                    value.3,
                    ACTOR,
                    *ACTOR_SHARDS.get(index).expect("actor shards"),
                ),
                structured_key: value.5,
                structured: token_data(
                    value.3,
                    representation_authority,
                    *STRUCTURED_SHARDS.get(index).expect("structured shards"),
                ),
            }
        });
        let aggregate_key = Pubkey::find_program_address(
            &ClaimsAggregateSeedsV1::new(market.to_bytes())
                .expect("aggregate seeds")
                .as_slices(),
            &CLAIMS,
        )
        .0;
        let actor_position_key = Pubkey::find_program_address(
            &ClaimsPositionSeedsV1::new(market.to_bytes(), ACTOR.to_bytes())
                .expect("actor Position seeds")
                .as_slices(),
            &CLAIMS,
        )
        .0;
        let replay_key = Pubkey::find_program_address(
            &[
                RATIONAL_REPLAY_SEED_V2,
                descriptor_id.as_slice(),
                ACTOR.as_ref(),
            ],
            &CLAIMS,
        )
        .0;
        let replay = dclutch_rational_representation_v2_contract::RationalReplayV2::new(
            descriptor_id,
            ACTOR.to_bytes(),
            REPRESENTATION_REVISION,
        )
        .expect("Rational replay")
        .to_bytes()
        .to_vec();
        let actor_receipt_key =
            get_associated_token_address_with_program_id(&ACTOR, &receipt_mint_key, &TOKEN);
        let mut fixture = Self {
            rent,
            release_set,
            activation_cache_key,
            activation_cache,
            market,
            core,
            aggregate_key,
            aggregate,
            actor_position_key,
            actor_position,
            graph,
            alternate_graph,
            descriptor,
            alternate_descriptor,
            linked_basis,
            product_record,
            result_domain_record,
            portfolio_record,
            descriptor_id,
            representation_authority,
            replay_key,
            replay,
            receipt_mint_key,
            receipt_mint: mint_data(COption::Some(representation_authority), RECEIPT_SUPPLY, 0),
            actor_receipt_key,
            actor_receipt: token_data(receipt_mint_key, ACTOR, 4),
            assets,
            terminal: None,
        };
        if terminal {
            fixture.terminal = Some(fixture.build_terminal(realm_record, collateral_mint_key));
        }
        fixture
    }

    fn asset_observations(&self) -> [AssetObservationV2<'_>; 2] {
        std::array::from_fn(|index| self.assets.get(index).expect("asset").observe())
    }

    fn observation<'a>(
        &'a self,
        assets: &'a [AssetObservationV2<'a>],
        mode: Mode,
    ) -> RationalObservationV2<'a> {
        RationalObservationV2 {
            caller_role: CallerRoleV2::Trading,
            registry_program: REGISTRY,
            activation_cache: observed(self.activation_cache_key, REGISTRY, &self.activation_cache),
            descriptor: self.descriptor.observe(&self.rent),
            graph: self.graph.observe(&self.rent),
            product_evidence: ProductEvidenceObservationV2 {
                linked_basis: self.linked_basis.observe(&self.rent),
                product: self.product_record.observe(&self.rent),
                result_domain: self.result_domain_record.observe(&self.rent),
                portfolio: self.portfolio_record.observe(&self.rent),
            },
            core_market: observed(self.market, CORE, &self.core),
            claims_aggregate: observed(self.aggregate_key, CLAIMS, &self.aggregate),
            replay: ReplayObservationV2 {
                account: observed(self.replay_key, CLAIMS, &self.replay),
            },
            receipt_mint: observed(self.receipt_mint_key, TOKEN, &self.receipt_mint),
            actor_receipt_account: matches!(mode, Mode::Structured)
                .then(|| observed(self.actor_receipt_key, TOKEN, &self.actor_receipt)),
            actor_claims_position: matches!(mode, Mode::Selected)
                .then(|| observed(self.actor_position_key, CLAIMS, &self.actor_position)),
            assets,
            actor: ACTOR,
            parent_context: PARENT_CONTEXT,
            rent: &self.rent,
        }
    }

    fn build_terminal(&self, realm: OwnedRecord, collateral_mint_key: Pubkey) -> OwnedTerminal {
        let recipient_key = Pubkey::new_from_array([0x85; 32]);
        let request = self.expected_terminal_request_with(&realm, recipient_key);
        let mut custody_request = CustodyRequestV1 {
            operation: OperationV1::Transfer,
            caller_role: CustodyCallerRoleV1::Claims,
            source_compartment: CompartmentV1::HoardPrincipal,
            destination_compartment: CompartmentV1::External,
            release_set: self.release_set,
            market: self.market.to_bytes(),
            realm: realm.digest(),
            context: PARENT_CONTEXT,
            caller_program: CLAIMS.to_bytes(),
            semantic: ContextV1 {
                candidate: self.descriptor_id,
                source_owner: [0; 32],
                destination_owner: ACTOR.to_bytes(),
                order: [0x31; 32],
                parent_request_digest: hash(&request).to_bytes(),
                order_nonce: REPRESENTATION_REVISION,
                generation: GENERATION,
                page_index: 0,
                execution_index: 0,
                transfer_index: 0,
            },
            source: [0x91; 32],
            destination: recipient_key.to_bytes(),
            source_vault_context: self.market.to_bytes(),
            destination_vault_context: [0; 32],
            mint: collateral_mint_key.to_bytes(),
            token_program: TOKEN_2022_PROGRAM_ID,
            payer: [0; 32],
            rent_refund: [0; 32],
            expected_revision: CUSTODY_REVISION,
            resulting_revision: CUSTODY_REVISION + 1,
            amount: 1,
            rent_lamports: 0,
        };
        let hoard_key = Pubkey::find_program_address(
            &CustodyVaultSeedsV1::from_request(custody_request, true).as_slices(),
            &CUSTODY,
        )
        .0;
        custody_request.source = hoard_key.to_bytes();
        let custody_request_bytes = custody_request.to_bytes().expect("Custody request");
        let custody_replay_key = Pubkey::find_program_address(
            &CustodyReplaySeedsV1::from_request(custody_request).as_slices(),
            &CUSTODY,
        )
        .0;
        let custody_authority = Pubkey::find_program_address(
            &CustodyAuthoritySeedsV1::from_request(custody_request).as_slices(),
            &CUSTODY,
        )
        .0;
        let custody_caller = Pubkey::find_program_address(
            &dclutch_release_set_contract::CallerAuthoritySeedsV1::new(
                ContentId::new(self.release_set).expect("release set"),
                self.market.to_bytes(),
                ExecutionRoleV1::Claims,
                PARENT_CONTEXT,
                hash(&custody_request_bytes).to_bytes(),
            )
            .expect("Custody caller seeds")
            .as_slices(),
            &CLAIMS,
        )
        .0;
        let replay = CustodyReplayV1 {
            caller_role: CustodyCallerRoleV1::Claims,
            release_set: self.release_set,
            market: self.market.to_bytes(),
            realm: realm.digest(),
            context: PARENT_CONTEXT,
            caller_program: CLAIMS.to_bytes(),
            rent_refund: ACTOR.to_bytes(),
            open_vault_count: 1,
            next_revision: CUSTODY_REVISION,
            generation: GENERATION,
            last_request_digest: [0x92; 32],
            last_poststate_commitment: [0x93; 32],
        };
        let replay_bytes = replay.to_bytes().expect("Custody replay").to_vec();
        assert_eq!(replay_bytes.len(), CUSTODY_REPLAY_BYTES_V1);
        OwnedTerminal {
            terminal_coordinate: OwnedRecord::new([0xa5; 32], vec![0xb5; 32]),
            realm,
            collateral_mint_key,
            collateral_mint: mint_data(COption::None, 6, 6),
            custody_replay_key,
            custody_replay: replay_bytes,
            hoard_key,
            hoard: token_data(collateral_mint_key, custody_authority, 1),
            recipient_key,
            recipient: token_data(collateral_mint_key, ACTOR, 5),
            custody_caller,
            custody_authority,
        }
    }

    fn expected_terminal_request_with(&self, realm: &OwnedRecord, recipient: Pubkey) -> Vec<u8> {
        let selected = self.assets.get(1).expect("winner asset");
        let custody_owner = Pubkey::find_program_address(
            &[
                dclutch_rational_representation_v2_contract::RATIONAL_CLAIMS_CUSTODY_OWNER_SEED_V2,
                self.descriptor_id.as_slice(),
                &WINNER.to_le_bytes(),
            ],
            &CLAIMS,
        )
        .0;
        let asset = AssetV2 {
            shard_mint: selected.shard_mint_key.to_bytes(),
            actor_shard_account: selected.actor_shard_key.to_bytes(),
            structured_custody_account: selected.structured_key.to_bytes(),
            claims_custody_owner: custody_owner.to_bytes(),
            coefficient: 7,
            expected_shard_supply: 70,
            expected_actor_shards: 21,
            expected_structured_shards: 49,
        };
        let mut rows = vec![0; ASSET_BYTES_V2];
        asset.encode_into(&mut rows).expect("asset row");
        let request = RepresentationRequestV2::new(
            RepresentationRequestHeaderV2 {
                action: RepresentationActionV2::RedeemTerminal,
                caller_role: CallerRoleV2::Trading,
                release_set: self.release_set,
                market: self.market.to_bytes(),
                graph_id: [0x31; 32],
                descriptor_id: self.descriptor_id,
                parent_context: PARENT_CONTEXT,
                actor: ACTOR.to_bytes(),
                receipt_mint: self.receipt_mint_key.to_bytes(),
                receipt_account: [0; 32],
                representation_authority: self.representation_authority.to_bytes(),
                token_program: TOKEN_2022_PROGRAM_ID,
                realm: realm.digest(),
                collateral_recipient: recipient.to_bytes(),
                expected_representation_revision: REPRESENTATION_REVISION,
                expected_claims_market_revision: 3,
                expected_actor_position_revision: NO_POSITION_REVISION,
                expected_custody_position_revision: 1,
                expected_custody_replay_revision: CUSTODY_REVISION,
                generation: GENERATION,
                quantity: 1,
                denominator: DENOMINATOR,
                expected_receipt_supply: RECEIPT_SUPPLY,
                outcome_count: OUTCOME_COUNT,
                selected_outcome: WINNER,
                asset_count: 1,
            },
            &rows,
        )
        .expect("terminal request");
        let mut output = vec![
            0;
            dclutch_rational_representation_v2_contract::REQUEST_HEADER_BYTES_V2
                + ASSET_BYTES_V2
        ];
        request.encode_into(&mut output).expect("request bytes");
        output
    }

    fn terminal_observation(&self) -> TerminalObservationV2<'_> {
        let terminal = self.terminal.as_ref().expect("terminal fixture");
        TerminalObservationV2 {
            outcome: WINNER,
            quantity: 1,
            realm: terminal.realm.observe(&self.rent),
            terminal_coordinate: terminal.terminal_coordinate.observe(&self.rent),
            custody_replay: observed(
                terminal.custody_replay_key,
                CUSTODY,
                &terminal.custody_replay,
            ),
            collateral_mint: observed(
                terminal.collateral_mint_key,
                TOKEN,
                &terminal.collateral_mint,
            ),
            hoard: observed(terminal.hoard_key, TOKEN, &terminal.hoard),
            collateral_recipient: observed(terminal.recipient_key, TOKEN, &terminal.recipient),
        }
    }
}

#[derive(Clone, Copy)]
enum Mode {
    Selected,
    Structured,
    Terminal,
}

fn assert_meta(meta: &AccountMeta, key: Pubkey, signer: bool, writable: bool) {
    assert_eq!(meta.pubkey, key);
    assert_eq!(meta.is_signer, signer);
    assert_eq!(meta.is_writable, writable);
}

#[test]
fn all_five_requests_roundtrip_with_exact_sparse_or_full_frames() {
    let open = Fixture::new(false);
    let open_assets = open.asset_observations();
    let selected = open_assets.get(1..2).expect("selected asset");
    let denominate = construct_denominate(
        open.observation(selected, Mode::Selected),
        SelectedActionInputV2 {
            outcome: WINNER,
            quantity: 2,
        },
    )
    .expect("Denominate");
    let reconstitute = construct_reconstitute(
        open.observation(selected, Mode::Selected),
        SelectedActionInputV2 {
            outcome: WINNER,
            quantity: 2,
        },
    )
    .expect("Reconstitute");
    let issue = construct_issue_structured(
        open.observation(&open_assets, Mode::Structured),
        StructuredActionInputV2 { quantity: 2 },
    )
    .expect("IssueStructured");
    let unwrap = construct_unwrap_structured(
        open.observation(&open_assets, Mode::Structured),
        StructuredActionInputV2 { quantity: 2 },
    )
    .expect("UnwrapStructured");
    let terminal = Fixture::new(true);
    let terminal_assets = terminal.asset_observations();
    let redeem = construct_redeem_terminal(
        terminal.observation(
            terminal_assets.get(1..2).expect("selected asset"),
            Mode::Terminal,
        ),
        terminal.terminal_observation(),
    )
    .expect("RedeemTerminal");

    for (built, action, assets, metas) in [
        (
            &denominate,
            RepresentationActionV2::Denominate,
            1_u32,
            36_usize,
        ),
        (&reconstitute, RepresentationActionV2::Reconstitute, 1, 36),
        (&issue, RepresentationActionV2::IssueStructured, 2, 40),
        (&unwrap, RepresentationActionV2::UnwrapStructured, 2, 40),
        (&redeem, RepresentationActionV2::RedeemTerminal, 1, 49),
    ] {
        let request = RepresentationRequestV2::decode(&built.instruction.data)
            .expect("operator request roundtrip");
        assert_eq!(request.header().action, action);
        assert_eq!(request.header().asset_count, assets);
        assert_eq!(request.physical_account_count(), Ok(metas));
        assert_eq!(built.instruction.accounts.len(), metas);
        assert_eq!(
            built.request_digest,
            hash(&built.instruction.data).to_bytes()
        );
    }
    assert_eq!(denominate.assets.len(), 1);
    assert_eq!(issue.assets.len(), 2);
    assert!(issue.terminal.is_none());
    assert_eq!(
        redeem.terminal.expect("terminal identities").realm,
        Pubkey::new_from_array(
            terminal
                .terminal
                .as_ref()
                .expect("terminal fixture")
                .realm
                .digest(),
        )
    );
}

#[test]
fn account_order_signers_writability_and_sentinels_are_exact() {
    let fixture = Fixture::new(false);
    let assets = fixture.asset_observations();
    let issue = construct_issue_structured(
        fixture.observation(&assets, Mode::Structured),
        StructuredActionInputV2 { quantity: 1 },
    )
    .expect("IssueStructured");
    let metas = &issue.instruction.accounts;
    assert_meta(
        metas.first().expect("caller"),
        metas.first().expect("caller").pubkey,
        true,
        false,
    );
    assert_meta(metas.get(1).expect("caller program"), TRADING, false, false);
    assert_meta(
        metas.get(2).expect("caller ProgramData"),
        programdata(TRADING),
        false,
        false,
    );
    assert_meta(metas.get(3).expect("actor"), ACTOR, true, false);
    assert_meta(
        metas.get(11).expect("replay"),
        fixture.replay_key,
        false,
        true,
    );
    assert_meta(
        metas.get(12).expect("aggregate"),
        fixture.aggregate_key,
        false,
        false,
    );
    assert_meta(
        metas.get(20).expect("receipt Mint"),
        fixture.receipt_mint_key,
        false,
        true,
    );
    assert_meta(
        metas.get(21).expect("actor receipt"),
        fixture.actor_receipt_key,
        false,
        true,
    );
    assert_meta(metas.get(22).expect("Token program"), TOKEN, false, false);
    assert_meta(
        metas.get(23).expect("Position sentinel"),
        CLAIMS,
        false,
        false,
    );
    for (offset, record) in [
        (24, &fixture.linked_basis),
        (26, &fixture.product_record),
        (28, &fixture.result_domain_record),
        (30, &fixture.portfolio_record),
    ] {
        assert_meta(
            metas.get(offset).expect("immutable raw record"),
            record.raw,
            false,
            false,
        );
        assert_meta(
            metas.get(offset + 1).expect("immutable staging cursor"),
            record.staging,
            false,
            false,
        );
    }
    for (row, asset) in fixture.assets.iter().enumerate() {
        let offset = 32 + row * 4;
        assert_meta(
            metas.get(offset).expect("custody Position"),
            asset.custody_position_key,
            false,
            false,
        );
        assert_meta(
            metas.get(offset + 1).expect("shard Mint"),
            asset.shard_mint_key,
            false,
            false,
        );
        assert_meta(
            metas.get(offset + 2).expect("actor shard"),
            asset.actor_shard_key,
            false,
            true,
        );
        assert_meta(
            metas.get(offset + 3).expect("structured shard"),
            asset.structured_key,
            false,
            true,
        );
    }

    let selected = construct_denominate(
        fixture.observation(assets.get(1..2).expect("selected asset"), Mode::Selected),
        SelectedActionInputV2 {
            outcome: WINNER,
            quantity: 1,
        },
    )
    .expect("Denominate");
    let selected_metas = &selected.instruction.accounts;
    assert_meta(
        selected_metas.get(20).expect("receipt sentinel"),
        fixture.receipt_mint_key,
        false,
        false,
    );
    assert_meta(
        selected_metas.get(21).expect("receipt account sentinel"),
        CLAIMS,
        false,
        false,
    );
    assert_meta(
        selected_metas.get(23).expect("actor Position"),
        fixture.actor_position_key,
        false,
        true,
    );
    let asset = fixture.assets.get(1).expect("winner asset");
    assert_meta(
        selected_metas.get(32).expect("custody Position"),
        asset.custody_position_key,
        false,
        true,
    );
    assert_meta(
        selected_metas.get(33).expect("shard Mint"),
        asset.shard_mint_key,
        false,
        true,
    );
    assert_meta(
        selected_metas.get(34).expect("actor shard"),
        asset.actor_shard_key,
        false,
        true,
    );
    assert_meta(
        selected_metas.get(35).expect("structured shard"),
        asset.structured_key,
        false,
        false,
    );

    let terminal_fixture = Fixture::new(true);
    let terminal_assets = terminal_fixture.asset_observations();
    let redeem = construct_redeem_terminal(
        terminal_fixture.observation(
            terminal_assets.get(1..2).expect("winner asset"),
            Mode::Terminal,
        ),
        terminal_fixture.terminal_observation(),
    )
    .expect("RedeemTerminal");
    let terminal_metas = &redeem.instruction.accounts;
    assert_meta(
        terminal_metas.get(23).expect("terminal Position sentinel"),
        CLAIMS,
        false,
        false,
    );
    let terminal = terminal_fixture
        .terminal
        .as_ref()
        .expect("terminal fixture");
    for (offset, key, writable) in [
        (36, terminal.custody_caller, false),
        (37, CUSTODY, false),
        (38, programdata(CUSTODY), false),
        (39, terminal.terminal_coordinate.raw, false),
        (40, terminal.terminal_coordinate.staging, false),
        (41, terminal.realm.raw, false),
        (42, terminal.realm.staging, false),
        (43, terminal.custody_replay_key, true),
        (44, terminal.collateral_mint_key, false),
        (45, terminal.hoard_key, true),
        (46, terminal.recipient_key, true),
        (47, terminal.custody_authority, false),
        (48, TOKEN, false),
    ] {
        assert_meta(
            terminal_metas.get(offset).expect("terminal meta"),
            key,
            false,
            writable,
        );
    }
    assert_eq!(
        terminal_metas.get(9).expect("Rent").pubkey,
        sysvar::rent::ID
    );
    assert_eq!(
        terminal_metas.get(10).expect("System").pubkey,
        system_program::ID
    );
}

#[test]
fn hostile_chain_substitution_zero_width_replay_and_winner_refuse() {
    let fixture = Fixture::new(false);
    let assets = fixture.asset_observations();
    let selected = assets.get(1..2).expect("selected asset");

    let mut zero = fixture.observation(selected, Mode::Selected);
    zero.actor = Pubkey::default();
    assert_eq!(
        construct_denominate(
            zero,
            SelectedActionInputV2 {
                outcome: WINNER,
                quantity: 1,
            },
        ),
        Err(Error::ZeroIdentity)
    );
    assert_eq!(
        construct_denominate(
            fixture.observation(&[], Mode::Selected),
            SelectedActionInputV2 {
                outcome: WINNER,
                quantity: 1,
            },
        ),
        Err(Error::InvalidWidth)
    );
    assert_eq!(
        construct_denominate(
            fixture.observation(selected, Mode::Selected),
            SelectedActionInputV2 {
                outcome: WINNER,
                quantity: 0,
            },
        ),
        Err(Error::InvalidAction)
    );
    assert_eq!(
        construct_issue_structured(
            fixture.observation(selected, Mode::Structured),
            StructuredActionInputV2 { quantity: 1 },
        ),
        Err(Error::InvalidWidth)
    );
    assert_eq!(
        construct_issue_structured(
            fixture.observation(&assets, Mode::Structured),
            StructuredActionInputV2 { quantity: 0 },
        ),
        Err(Error::InvalidAction)
    );

    let mut graph_substitution = fixture.observation(selected, Mode::Selected);
    graph_substitution.graph = fixture.alternate_graph.observe(&fixture.rent);
    assert!(
        construct_denominate(
            graph_substitution,
            SelectedActionInputV2 {
                outcome: WINNER,
                quantity: 1,
            },
        )
        .is_err()
    );

    let mut descriptor_substitution = fixture.observation(selected, Mode::Selected);
    descriptor_substitution.descriptor = fixture.alternate_descriptor.observe(&fixture.rent);
    assert!(
        construct_denominate(
            descriptor_substitution,
            SelectedActionInputV2 {
                outcome: WINNER,
                quantity: 1,
            },
        )
        .is_err()
    );

    let mut replay_key_substitution = fixture.observation(selected, Mode::Selected);
    replay_key_substitution.replay.account.key = Pubkey::new_from_array([0xaa; 32]);
    assert_eq!(
        construct_denominate(
            replay_key_substitution,
            SelectedActionInputV2 {
                outcome: WINNER,
                quantity: 1,
            },
        ),
        Err(Error::InvalidReplay)
    );

    let terminal_fixture = Fixture::new(true);
    let terminal_assets = terminal_fixture.asset_observations();
    let mut wrong_winner = terminal_fixture.terminal_observation();
    wrong_winner.outcome = 0;
    assert_eq!(
        construct_redeem_terminal(
            terminal_fixture.observation(
                terminal_assets
                    .first()
                    .map(std::slice::from_ref)
                    .expect("first asset"),
                Mode::Terminal,
            ),
            wrong_winner,
        ),
        Err(Error::InvalidTerminal)
    );
    let mut zero_terminal = terminal_fixture.terminal_observation();
    zero_terminal.quantity = 0;
    assert_eq!(
        construct_redeem_terminal(
            terminal_fixture.observation(
                terminal_assets.get(1..2).expect("winner asset"),
                Mode::Terminal,
            ),
            zero_terminal,
        ),
        Err(Error::InvalidAction)
    );

    let canonical = construct_issue_structured(
        fixture.observation(&assets, Mode::Structured),
        StructuredActionInputV2 { quantity: 1 },
    )
    .expect("canonical order");
    let mut reordered = canonical.instruction.accounts.clone();
    reordered.swap(32, 33);
    assert_ne!(reordered, canonical.instruction.accounts);
    assert_eq!(
        canonical
            .instruction
            .accounts
            .get(32)
            .expect("Position")
            .pubkey,
        fixture
            .assets
            .first()
            .expect("first asset")
            .custody_position_key
    );
    assert_eq!(
        canonical.instruction.accounts.get(33).expect("Mint").pubkey,
        fixture.assets.first().expect("first asset").shard_mint_key
    );
}
