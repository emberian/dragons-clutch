//! Executable evidence for the `RelayedMainnetStateV1` observation-record
//! transport, against the real compiled Resolution SBF ELF.
//!
//! **What this is, said at the exact resolution it holds.** A `ProgramTest`
//! bank executes the real adapter over synthetic accounts. The Ed25519
//! signatures are cryptographically real and are verified by the runtime's own
//! precompile before the program runs. Everything they attest is synthetic: the
//! account bytes are fixtures, the "mainnet" slot is a number, and the relayer
//! key is generated here. This is **not** devnet evidence, not mainnet
//! evidence, and not provider-availability evidence. The correct sentence about
//! the strongest case below is "the bank accepted an attestation asserting
//! mainnet state," never "the market observed mainnet."
//!
//! The adapter under test is `programs/dclutch-resolution-proof-sbf`. The
//! transport moved there from the banished gen-2 monolith without its content
//! changing: same Lean-authored wire, same hostile corpus, same real-ELF
//! execution. What changed is who owns the record, and therefore what a Market
//! is: a Core-owned `CoreState` at its derived address whose selected release
//! set names this Resolution Program, rather than a Market account the adapter
//! owned itself.
//!
//! The venue fixture is shaped from the published Meteora DBC source: a
//! 424-byte `VirtualPool` account (8-byte Anchor discriminator
//! `d5e005d1 6245775c` plus the 416-byte `PoolState` body) with
//! `migration_progress` at account offset 308 and `is_migrated` at 305. The
//! layout is real; the values are invented, so the fixture is labelled
//! synthetic-value rather than synthetic-shape.

use std::{env, fs, path::PathBuf};

use dclutch_core_contract::ContentId;
use dclutch_market_core_codec::{
    CoreState, Identity as CoreIdentity, MarketCoreStateSeedsV2, MarketIdentity, Phase, Readiness,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetV1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, activate_execution_role_into_v1,
    initialize_activation_cache_v1,
};
use dclutch_relay_contract::{
    RELAYED_ADAPTER_CONFIG_SCHEMA_RELEASE_ID_V1, RELAYED_ADAPTER_CONFIG_SCHEMA_RELEASE_PREIMAGE_V1,
    RELAYED_FAMILY_RELEASE_ID_V1, RELAYED_RECORD_PDA_DOMAIN_V1,
    RELAYED_RECORD_TRANSPORT_PROFILE_ID_V1, RELAYER_KEY_SET_SCHEMA_RELEASE_ID_V1,
    RELAYER_KEY_SET_SCHEMA_RELEASE_PREIMAGE_V1, SHA256_EMPTY_DIGEST,
    SOLANA_DEVNET_GENESIS_HASH_V1, SOLANA_MAINNET_GENESIS_HASH_V1,
    identity::{LOADER_V3_PROGRAM_ID, reconstruct_deployment_observation_v1},
    instruction::{
        APPEND_OBSERVATION_PREFIX_BYTES, AppendObservationInstructionV1, CreateRecordInstructionV1,
        RetireRecordInstructionV1, SEAL_RECORD_PREFIX_BYTES, SealRecordInstructionV1,
    },
    record::{RelayedObservationRecordViewV1, RelayedRecordPhaseV1},
    release::{
        AccountSetEntryV1, RelayedAdapterConfigV1, RelayerKeySetV1, SET_DIGEST_SEED_PREIMAGE_BYTES,
        account_set_id_preimage_len_v1, encode_account_set_id_preimage_v1,
        encode_set_digest_seed_preimage_v1,
    },
    signature::ED25519_PROGRAM_ID_3_0,
    wire::{AccountObservationV1, AttestationMessageV1, ObservationSetSealV1},
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1,
    ProgramIdentityV1,
};
use dclutch_rent_contract::{RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority, RentCreditV1};
use dclutch_resolution_codec::RESOLUTION_CONTROLLER_RELEASE_ID_V4;
use dclutch_source_contract::{
    CapacityEnvelope as SourceCapacityEnvelope, ContentId as SourceContentId,
    PROVIDER_RELEASE_SCHEMA_ID_V1, ProviderReleaseV1, PythAdapterConfigV1,
    RELAYED_PROVIDER_EXTENSION_RELEASE_ID_V1, SOURCE_FAILURE_POLICY_RELEASE_ID_V2,
    SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2, SOURCE_SPEC_SCHEMA_ID_V1, STATISTIC_SPEC_SCHEMA_ID_V1,
    RoundingBoundary, SourceAccessProfile, SourceCapacityProfileV1, SourceMaterialV2, SourceSpecV1,
    StatisticKind, StatisticSpecV1, WINDOW_SPEC_SCHEMA_ID_V1, WindowKind, WindowSpecV1,
};
use solana_account::Account;
use solana_program::{
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_program::instruction::InstructionError;
use solana_transaction::{Transaction, TransactionError};

/// The Resolution role Program: the executing adapter, and the record's owner.
const PROGRAM_ID: Pubkey = Pubkey::new_from_array([71; 32]);
/// The Core role Program: the Market's owner and derivation authority.
const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([72; 32]);
/// The Registry: the program that owns every finalized raw record.
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([73; 32]);
/// The Rent program that owns the Market's persisted RentCredit beneficiary.
const RENT_PROGRAM_ID: Pubkey = Pubkey::new_from_array([74; 32]);
const GENERATION: u64 = 73;
/// A finalized mainnet slot, as a number. Nothing was read to obtain it.
const OBSERVED_SLOT: u64 = 423_941_138;
const CREATED_UNIX: i64 = 1_756_000_000;
const WINDOW_MAX_AGE_SECONDS: u32 = 5_400;
const CLUSTER_SKEW_SECONDS: u64 = 120;

/// `dbcij3LWUppWqq96dh6gJWwBifmcGfLSB5D4DuSMaqN` (verified-on-chain, both clusters).
const DBC_PROGRAM: [u8; 32] = [
    0x09, 0x60, 0x0c, 0xa5, 0x24, 0xf7, 0xb1, 0xb7, 0xd6, 0xcc, 0xb1, 0xc3, 0x97, 0x3a, 0xa0, 0x33,
    0x0d, 0x19, 0x03, 0xda, 0x60, 0x1c, 0xc9, 0xb5, 0xde, 0xe3, 0xc6, 0x62, 0xb4, 0xca, 0xd1, 0x49,
];
/// `HUfnSSiJxgspQm6C1rkqv6L3XgVtn7AESApgCQpCXCYh`.
const DBC_PROGRAMDATA: [u8; 32] = [
    0xf4, 0xd1, 0x86, 0x75, 0x30, 0x52, 0x43, 0xdc, 0x37, 0x9e, 0xb4, 0x94, 0x57, 0xaf, 0xa7, 0xdd,
    0x60, 0x00, 0x24, 0x63, 0xdc, 0xdc, 0x6f, 0x11, 0xb2, 0x68, 0x5d, 0x23, 0x34, 0x9c, 0xfc, 0xba,
];
/// `SysvarC1ock11111111111111111111111111111111`, as read on the OTHER cluster.
const MAINNET_CLOCK: [u8; 32] = [
    0x06, 0xa7, 0xd5, 0x17, 0x18, 0xc7, 0x74, 0xc9, 0x28, 0x56, 0x63, 0x98, 0x69, 0x1d, 0x5e, 0xb6,
    0x8b, 0x5e, 0xb8, 0xa3, 0x9b, 0x4b, 0x6d, 0x5c, 0x73, 0x55, 0x5b, 0x21, 0x00, 0x00, 0x00, 0x00,
];
const DBC_POOL: [u8; 32] = [0x5a; 32];
/// `sha256("account:VirtualPool")[..8]`, agreeing with the deployed IDL and a
/// live mainnet pool account.
const VIRTUAL_POOL_DISCRIMINATOR: [u8; 8] = [0xd5, 0xe0, 0x05, 0xd1, 0x62, 0x45, 0x77, 0x5c];
/// 8-byte discriminator + `PoolState::INIT_SPACE`. The program has no `realloc`,
/// so the admitted length set is the singleton `{424}`.
const VIRTUAL_POOL_BYTES: usize = 424;
const MIGRATION_PROGRESS_OFFSET: usize = 308;
const IS_MIGRATED_OFFSET: usize = 305;
const FINISH_CURVE_TIMESTAMP_OFFSET: usize = 344;
const MIGRATION_PROGRESS_CREATED_POOL: u8 = 3;

const DEPLOYMENT_SLOT: u64 = 423_941_138;
const UPGRADE_AUTHORITY: [u8; 32] = [0x4a; 32];
const ELF_DIGEST: [u8; 32] = [0xee; 32];

struct Elves {
    core: Vec<u8>,
    resolution: Vec<u8>,
}

fn artifacts() -> Elves {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect(
        "SBF_OUT_DIR is required; build the Core and Resolution role programs with `cargo build-sbf --manifest-path programs/<name>/Cargo.toml` and point SBF_OUT_DIR at target/deploy",
    ));
    let resolution = fs::read(directory.join("dclutch_resolution_proof_sbf.so"))
        .expect("compiled Resolution ELF");
    assert_eq!(resolution.get(..4), Some(&[0x7f, b'E', b'L', b'F'][..]));
    eprintln!(
        "Resolution SBF ELF SHA-256: {:?}",
        hash(&resolution).to_bytes()
    );
    Elves {
        core: fs::read(directory.join("dclutch_core_sbf.so")).expect("compiled Core ELF"),
        resolution,
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
        ContentId::new(semantic).expect("semantic release"),
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

/// The Registry activation cache for one five-role release set.
///
/// The relay routes never invoke Core or Custody; the set is complete because a
/// release set IS five roles, and the one binding the adapter reads is
/// Resolution's, which must name the executing Program.
fn activation(core: ArtifactReleaseV1, resolution: ArtifactReleaseV1) -> ([u8; 32], Vec<u8>) {
    let release_set = ExecutionReleaseSetV1::new(
        binding(core),
        binding(core),
        binding(core),
        binding(resolution),
        binding(core),
    )
    .expect("execution release set");
    let release_set_id = hash(&release_set.to_bytes()).to_bytes();
    let content = ContentId::new(release_set_id).expect("release set identity");
    let mut bytes = vec![0; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut bytes, content).expect("activation cache");
    for (role, selected) in [
        (ExecutionRoleV1::Core, core),
        (ExecutionRoleV1::Claims, core),
        (ExecutionRoleV1::Trading, core),
        (ExecutionRoleV1::Resolution, resolution),
        (ExecutionRoleV1::Custody, core),
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

struct RecordPair {
    raw: Pubkey,
    staging: Pubkey,
}

/// Install one finalized Registry-owned raw record and its vacant staging PDA.
fn add_record(test: &mut ProgramTest, schema: [u8; 32], data: Vec<u8>) -> (RecordPair, [u8; 32]) {
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
    (RecordPair { raw, staging }, digest)
}

fn source_id(bytes: [u8; 32]) -> SourceContentId {
    SourceContentId::new(bytes).expect("nonzero deterministic Source identity")
}

/// The founding-time pinned ordered account set, exactly as the daemon derives
/// it: the relayer chooses none of these, it echoes the identity and the adapter
/// compares.
fn account_set() -> ([AccountSetEntryV1; 4], [u8; 32]) {
    let entries = [
        AccountSetEntryV1 {
            key: DBC_PROGRAM,
            expected_owner: LOADER_V3_PROGRAM_ID,
            inline_len: 36,
        },
        AccountSetEntryV1 {
            key: DBC_PROGRAMDATA,
            expected_owner: LOADER_V3_PROGRAM_ID,
            inline_len: 45,
        },
        AccountSetEntryV1 {
            key: DBC_POOL,
            expected_owner: DBC_PROGRAM,
            inline_len: 424,
        },
        AccountSetEntryV1 {
            key: MAINNET_CLOCK,
            expected_owner: sysvar::ID.to_bytes(),
            inline_len: 40,
        },
    ];
    let width = account_set_id_preimage_len_v1(entries.len()).expect("preimage width");
    let mut preimage = vec![0u8; width];
    encode_account_set_id_preimage_v1(
        &mut preimage,
        SOLANA_MAINNET_GENESIS_HASH_V1,
        RELAYED_FAMILY_RELEASE_ID_V1,
        &entries,
    )
    .expect("canonical account-set preimage");
    let account_set_id = hash(&preimage).to_bytes();
    (entries, account_set_id)
}

fn dbc_program_body() -> Vec<u8> {
    let mut data = vec![0u8; 36];
    data[..4].copy_from_slice(&2u32.to_le_bytes());
    data[4..36].copy_from_slice(&DBC_PROGRAMDATA);
    data
}

fn dbc_programdata_prefix() -> Vec<u8> {
    let mut data = vec![0u8; 45];
    data[..4].copy_from_slice(&3u32.to_le_bytes());
    data[4..12].copy_from_slice(&DEPLOYMENT_SLOT.to_le_bytes());
    data[12] = 1;
    data[13..45].copy_from_slice(&UPGRADE_AUTHORITY);
    data
}

/// A graduated pool: `migration_progress = CreatedPool`, `is_migrated = 1`, and
/// a nonzero `finish_curve_timestamp`. Layout real, values invented.
fn virtual_pool_body() -> Vec<u8> {
    let mut data = vec![0u8; VIRTUAL_POOL_BYTES];
    data[..8].copy_from_slice(&VIRTUAL_POOL_DISCRIMINATOR);
    data[MIGRATION_PROGRESS_OFFSET] = MIGRATION_PROGRESS_CREATED_POOL;
    data[IS_MIGRATED_OFFSET] = 1;
    data[FINISH_CURVE_TIMESTAMP_OFFSET..FINISH_CURVE_TIMESTAMP_OFFSET + 8]
        .copy_from_slice(&1_756_000_500u64.to_le_bytes());
    data
}

fn mainnet_clock_body() -> Vec<u8> {
    let mut data = vec![0u8; 40];
    data[..8].copy_from_slice(&OBSERVED_SLOT.to_le_bytes());
    data[32..40].copy_from_slice(&CREATED_UNIX.to_le_bytes());
    data
}

struct Position {
    body: Vec<u8>,
    data_len: u32,
    owner: [u8; 32],
    key: [u8; 32],
    executable: bool,
    tail_digest: [u8; 32],
}

fn positions() -> Vec<Position> {
    vec![
        Position {
            body: dbc_program_body(),
            data_len: 36,
            owner: LOADER_V3_PROGRAM_ID,
            key: DBC_PROGRAM,
            executable: true,
            tail_digest: SHA256_EMPTY_DIGEST,
        },
        Position {
            body: dbc_programdata_prefix(),
            data_len: 2_326_622,
            owner: LOADER_V3_PROGRAM_ID,
            key: DBC_PROGRAMDATA,
            executable: false,
            // For a ProgramData account inlined at exactly 45 bytes the tail
            // digest IS the deployed ELF digest, by construction.
            tail_digest: ELF_DIGEST,
        },
        Position {
            body: virtual_pool_body(),
            data_len: VIRTUAL_POOL_BYTES as u32,
            owner: DBC_PROGRAM,
            key: DBC_POOL,
            executable: false,
            tail_digest: SHA256_EMPTY_DIGEST,
        },
        Position {
            body: mainnet_clock_body(),
            data_len: 40,
            owner: sysvar::ID.to_bytes(),
            key: MAINNET_CLOCK,
            executable: false,
            tail_digest: SHA256_EMPTY_DIGEST,
        },
    ]
}

fn observation(position: &Position) -> AccountObservationV1<'_> {
    AccountObservationV1::new(
        position.key,
        position.owner,
        1_000_000,
        position.data_len,
        &position.body,
        position.executable,
        position.tail_digest,
    )
    .expect("canonical observation body")
}

struct SourceGraph {
    material: RecordPair,
    material_id: [u8; 32],
    spec: RecordPair,
    spec_id: [u8; 32],
    provider: RecordPair,
    window: RecordPair,
}

/// Build and install the whole V2 Source record graph this family needs.
///
/// The compact V2 material names its components by content identity instead of
/// carrying them inline, so every link below is a digest the adapter re-derives
/// from a record it authenticated separately: material -> spec -> provider
/// release, and material -> window.
fn source_graph(
    test: &mut ProgramTest,
    key_set_digest: [u8; 32],
    adapter_config_digest: [u8; 32],
) -> SourceGraph {
    let capacity = SourceCapacityProfileV1::new(
        SourceCapacityEnvelope::Measured,
        1,
        0,
        source_id([36; 32]),
        source_id([37; 32]),
        512,
        4,
    )
    .expect("canonical Source capacity");
    let capacity_id = source_id(hash(&capacity.to_bytes()).to_bytes());
    // The V1 material carried an inline Pyth-typed adapter config slot; the V2
    // material does not, and the relayed family names its configuration through
    // `decoding_rules_id` instead. The spec's `adapter_config_id` keeps a
    // canonical placeholder so no Pyth-shaped slot is ever read for this family.
    let unused_inline =
        PythAdapterConfigV1::new([0x2a; 32], -8, 100).expect("inline slot placeholder");
    let provider_value = ProviderReleaseV1::new(
        source_id(RELAYED_FAMILY_RELEASE_ID_V1),
        source_id(RELAYED_PROVIDER_EXTENSION_RELEASE_ID_V1),
        // The relayer key set IS the provider deployment release.
        source_id(key_set_digest),
        // ...and the pinned ordered account set is a decoding-rules fact.
        source_id(adapter_config_digest),
        source_id(RELAYED_RECORD_TRANSPORT_PROFILE_ID_V1),
    );
    let (provider, provider_digest) = add_record(
        test,
        PROVIDER_RELEASE_SCHEMA_ID_V1,
        provider_value.to_bytes().to_vec(),
    );

    let unit = source_id([0x51; 32]);
    let spec_value = SourceSpecV1::new(
        source_id([0x50; 32]),
        unit,
        source_id(provider_digest),
        SourceAccessProfile::RelayedObservationRecord,
        source_id(hash(&unused_inline.to_bytes()).to_bytes()),
        capacity_id,
    );
    let (spec, spec_digest) = add_record(
        test,
        SOURCE_SPEC_SCHEMA_ID_V1,
        spec_value.to_bytes().to_vec(),
    );

    let window_value = WindowSpecV1::new(
        source_id(spec_digest),
        WindowKind::Terminal,
        CREATED_UNIX,
        CREATED_UNIX,
        WINDOW_MAX_AGE_SECONDS,
        1,
        source_id([41; 32]),
    )
    .expect("pinned terminal window");
    let (window, window_digest) = add_record(
        test,
        WINDOW_SPEC_SCHEMA_ID_V1,
        window_value.to_bytes().to_vec(),
    );

    let statistic_value = StatisticSpecV1::new(
        unit,
        unit,
        StatisticKind::TerminalSample,
        RoundingBoundary::ExactRational,
        1,
        0,
        capacity_id,
        source_id([42; 32]),
        capacity,
    )
    .expect("canonical terminal statistic");
    let (_, statistic_digest) = add_record(
        test,
        STATISTIC_SPEC_SCHEMA_ID_V1,
        statistic_value.to_bytes().to_vec(),
    );

    let material_value = SourceMaterialV2::new(
        source_id([0x52; 32]),
        source_id(spec_digest),
        source_id(window_digest),
        source_id(statistic_digest),
        None,
        source_id(SOURCE_FAILURE_POLICY_RELEASE_ID_V2),
    );
    // The graph the material claims is the graph the records make. Asserting it
    // here turns a Custom(7) from the bank into a named fixture failure.
    material_value
        .validate_source_graph(
            source_id(spec_digest),
            spec_value,
            source_id(window_digest),
            window_value,
            source_id(statistic_digest),
            statistic_value,
            None,
            source_id(SOURCE_FAILURE_POLICY_RELEASE_ID_V2),
        )
        .expect("the V2 material's own graph predicate holds");
    let (material, material_digest) = add_record(
        test,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
        material_value.to_bytes().to_vec(),
    );

    SourceGraph {
        material,
        material_id: material_digest,
        spec,
        spec_id: spec_digest,
        provider,
        window,
    }
}

struct Fixture {
    test: Option<ProgramTest>,
    relayer: Keypair,
    worker: Keypair,
    market: Pubkey,
    activation: Pubkey,
    decoy_activation: Pubkey,
    record: Pubkey,
    record_bump: u8,
    graph: SourceGraph,
    key_set: RecordPair,
    config: RecordPair,
    rent_beneficiary: Pubkey,
    account_set_id: [u8; 32],
    positions: Vec<Position>,
}

fn fixture(seal_threshold: u8, extra_keys: &[[u8; 32]]) -> Fixture {
    let elves = artifacts();
    let relayer = Keypair::new();
    let worker = Keypair::new();
    let (_, account_set_id) = account_set();

    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    add_program(&mut test, "dclutch_core_sbf", CORE_PROGRAM_ID, &elves.core);
    add_program(
        &mut test,
        "dclutch_resolution_proof_sbf",
        PROGRAM_ID,
        &elves.resolution,
    );

    // The emitted schema identities are the hashes of the Lean-owned preimages.
    // Naming both keeps the emitter honest at fixture-build time rather than at
    // whatever the bank happens to refuse.
    assert_eq!(
        hash(RELAYER_KEY_SET_SCHEMA_RELEASE_PREIMAGE_V1).to_bytes(),
        RELAYER_KEY_SET_SCHEMA_RELEASE_ID_V1
    );
    assert_eq!(
        hash(RELAYED_ADAPTER_CONFIG_SCHEMA_RELEASE_PREIMAGE_V1).to_bytes(),
        RELAYED_ADAPTER_CONFIG_SCHEMA_RELEASE_ID_V1
    );

    let mut keys = vec![relayer.pubkey().to_bytes()];
    keys.extend_from_slice(extra_keys);
    keys.sort_unstable();
    let key_set_value =
        RelayerKeySetV1::new(&keys, seal_threshold).expect("canonical relayer key set");
    let (key_set, key_set_digest) = add_record(
        &mut test,
        RELAYER_KEY_SET_SCHEMA_RELEASE_ID_V1,
        key_set_value.to_bytes().expect("key set bytes").to_vec(),
    );

    let config_value = RelayedAdapterConfigV1::new(
        account_set_id,
        0,
        0,
        u64::from(WINDOW_MAX_AGE_SECONDS),
        CLUSTER_SKEW_SECONDS,
    )
    .expect("canonical relayed adapter config");
    let (config, config_digest) = add_record(
        &mut test,
        RELAYED_ADAPTER_CONFIG_SCHEMA_RELEASE_ID_V1,
        config_value.to_bytes().expect("config bytes").to_vec(),
    );

    let graph = source_graph(&mut test, key_set_digest, config_digest);

    let core_release = release(CORE_PROGRAM_ID, [0x41; 32], &elves.core);
    let resolution_release = release(
        PROGRAM_ID,
        RESOLUTION_CONTROLLER_RELEASE_ID_V4,
        &elves.resolution,
    );
    let (release_set, activation_data) = activation(core_release, resolution_release);
    let activation_account = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    test.add_account(
        activation_account,
        protocol_account(REGISTRY_PROGRAM_ID, activation_data),
    );

    // A complete, internally consistent, Registry-owned activation cache for a
    // DIFFERENT release set -- one whose Resolution role is some other Program.
    // It is exactly what an attacker would hold if activating a release set were
    // enough to put records under this Program.
    let (decoy_set, decoy_data) = activation(core_release, core_release);
    let decoy_activation = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &decoy_set],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    assert_ne!(decoy_set, release_set, "the decoy must be a different set");
    test.add_account(
        decoy_activation,
        protocol_account(REGISTRY_PROGRAM_ID, decoy_data),
    );

    // The Market's persisted beneficiary is a real RentCredit PDA under the Rent
    // program. The relay adapter never derives it: Core already persists which
    // account receives this Market's returned rent, and the adapter can only
    // agree with that.
    let refund_authority = RefundAuthority::new([0x61; 32]).expect("refund authority");
    let (rent_beneficiary, rent_credit_bump) = Pubkey::find_program_address(
        &[RENT_CREDIT_PDA_DOMAIN_V1, &refund_authority.to_bytes()],
        &RENT_PROGRAM_ID,
    );
    let rent_credit_value = RentCreditV1::new(refund_authority, rent_credit_bump);
    test.add_account(
        rent_beneficiary,
        protocol_account(RENT_PROGRAM_ID, rent_credit_value.to_bytes().to_vec()),
    );

    let mut identity = MarketIdentity {
        market_id: CoreIdentity::new([0xff; 32]).expect("placeholder Market"),
        realm_id: CoreIdentity::new([31; 32]).expect("Realm"),
        product_record: CoreIdentity::new([0x52; 32]).expect("Product record"),
        product_id: CoreIdentity::new([33; 32]).expect("Product"),
        resolution_policy: CoreIdentity::new(graph.material_id).expect("Source material"),
        capability_manifest: CoreIdentity::new([25; 32]).expect("manifest"),
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
        phase: Phase::Open,
        readiness: Readiness::Consumed,
        terminal_winner: 0,
        identity,
        outstanding_capabilities: 0,
        rent_beneficiary: CoreIdentity::new(rent_beneficiary.to_bytes()).expect("beneficiary"),
        terminal_receipt: None,
    };
    test.add_account(
        market,
        protocol_account(CORE_PROGRAM_ID, state.encode().expect("Core state").to_vec()),
    );

    let (record, record_bump) = Pubkey::find_program_address(
        &[
            RELAYED_RECORD_PDA_DOMAIN_V1,
            market.as_ref(),
            &GENERATION.to_le_bytes(),
            account_set_id.as_slice(),
            &OBSERVED_SLOT.to_le_bytes(),
        ],
        &PROGRAM_ID,
    );

    test.add_account(
        worker.pubkey(),
        Account::new(1_000_000_000, 0, &system_program::ID),
    );

    Fixture {
        test: Some(test),
        relayer,
        worker,
        market,
        activation: activation_account,
        decoy_activation,
        record,
        record_bump,
        graph,
        key_set,
        config,
        rent_beneficiary,
        account_set_id,
        positions: positions(),
    }
}

/// One substitution a hostile create makes, and nothing else.
///
/// Each field names a fact the adapter must take from the authenticated Market
/// or the content-addressed graph rather than from the caller. A `None` field
/// is the honest value.
#[derive(Clone, Copy, Default)]
struct CreateSubstitution {
    core_program: Option<Pubkey>,
    activation: Option<Pubkey>,
    rent_beneficiary: Option<Pubkey>,
    source_spec_id: Option<[u8; 32]>,
}

impl Fixture {
    fn create_instruction(&self, set_count: u16, seal_threshold: u8) -> Instruction {
        self.create_instruction_with(set_count, seal_threshold, CreateSubstitution::default())
    }

    fn create_instruction_with(
        &self,
        set_count: u16,
        seal_threshold: u8,
        substitution: CreateSubstitution,
    ) -> Instruction {
        let request = CreateRecordInstructionV1::new(
            GENERATION,
            OBSERVED_SLOT,
            set_count,
            seal_threshold,
            self.record_bump,
            self.graph.material_id,
            substitution.source_spec_id.unwrap_or(self.graph.spec_id),
            substitution
                .rent_beneficiary
                .unwrap_or(self.rent_beneficiary)
                .to_bytes(),
        )
        .expect("create request");
        Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(self.worker.pubkey(), true),
                AccountMeta::new_readonly(self.market, false),
                AccountMeta::new_readonly(
                    substitution.core_program.unwrap_or(CORE_PROGRAM_ID),
                    false,
                ),
                AccountMeta::new_readonly(substitution.activation.unwrap_or(self.activation), false),
                AccountMeta::new(self.record, false),
                AccountMeta::new_readonly(self.graph.material.raw, false),
                AccountMeta::new_readonly(self.graph.material.staging, false),
                AccountMeta::new_readonly(self.graph.spec.raw, false),
                AccountMeta::new_readonly(self.graph.spec.staging, false),
                AccountMeta::new_readonly(self.graph.provider.raw, false),
                AccountMeta::new_readonly(self.graph.provider.staging, false),
                AccountMeta::new_readonly(self.graph.window.raw, false),
                AccountMeta::new_readonly(self.graph.window.staging, false),
                AccountMeta::new_readonly(self.key_set.raw, false),
                AccountMeta::new_readonly(self.key_set.staging, false),
                AccountMeta::new_readonly(self.config.raw, false),
                AccountMeta::new_readonly(self.config.staging, false),
                AccountMeta::new_readonly(
                    substitution.rent_beneficiary.unwrap_or(self.rent_beneficiary),
                    false,
                ),
                AccountMeta::new_readonly(sysvar::rent::ID, false),
                AccountMeta::new_readonly(sysvar::clock::ID, false),
                AccountMeta::new_readonly(system_program::ID, false),
            ],
            data: request.to_bytes().expect("create bytes").to_vec(),
        }
    }

    fn append_instruction(&self, message: &[u8]) -> Instruction {
        let mut data = AppendObservationInstructionV1::new(GENERATION, OBSERVED_SLOT)
            .to_prefix_bytes()
            .expect("append prefix")
            .to_vec();
        data.extend_from_slice(message);
        Instruction {
            program_id: PROGRAM_ID,
            accounts: self.signature_frame(),
            data,
        }
    }

    fn seal_instruction(&self, message: &[u8]) -> Instruction {
        let mut data = SealRecordInstructionV1::new(GENERATION, OBSERVED_SLOT)
            .to_prefix_bytes()
            .expect("seal prefix")
            .to_vec();
        data.extend_from_slice(message);
        Instruction {
            program_id: PROGRAM_ID,
            accounts: self.signature_frame(),
            data,
        }
    }

    fn signature_frame(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.worker.pubkey(), true),
            AccountMeta::new_readonly(self.market, false),
            AccountMeta::new(self.record, false),
            AccountMeta::new_readonly(self.key_set.raw, false),
            AccountMeta::new_readonly(self.key_set.staging, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
            AccountMeta::new_readonly(sysvar::instructions::ID, false),
            AccountMeta::new_readonly(sysvar::clock::ID, false),
        ]
    }

    fn retire_instruction(&self) -> Instruction {
        Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(self.worker.pubkey(), true),
                AccountMeta::new_readonly(self.market, false),
                AccountMeta::new(self.record, false),
                AccountMeta::new(self.rent_beneficiary, false),
            ],
            data: RetireRecordInstructionV1::new(GENERATION)
                .to_bytes()
                .expect("retire bytes")
                .to_vec(),
        }
    }

    fn attestation(&self, index: usize, cluster: [u8; 32], slot: u64) -> Vec<u8> {
        let position = self.positions.get(index).expect("position");
        let message = AttestationMessageV1::new(
            cluster,
            RELAYED_FAMILY_RELEASE_ID_V1,
            [39; 32],
            self.account_set_id,
            slot,
            u16::try_from(index).expect("small"),
            u16::try_from(self.positions.len()).expect("small"),
            observation(position),
        )
        .expect("attestation message");
        let mut bytes = vec![0u8; message.encoded_len()];
        message.encode_into(&mut bytes).expect("encode");
        bytes
    }

    fn seal_message(&self, set_digest: [u8; 32]) -> Vec<u8> {
        ObservationSetSealV1::new(
            SOLANA_MAINNET_GENESIS_HASH_V1,
            RELAYED_FAMILY_RELEASE_ID_V1,
            self.account_set_id,
            OBSERVED_SLOT,
            u16::try_from(self.positions.len()).expect("small"),
            set_digest,
        )
        .expect("seal message")
        .to_bytes()
        .expect("seal bytes")
        .to_vec()
    }
}

/// Build the one-signature Ed25519 precompile instruction by hand.
///
/// Hand-building rather than using a helper is deliberate: the campaign has to
/// be able to produce descriptors that are subtly wrong, and a helper that can
/// only produce correct ones cannot test a refusal.
fn ed25519_instruction(
    signer: &Keypair,
    message: &[u8],
    message_offset: u16,
    message_instruction_index: u16,
) -> Instruction {
    let signature = signer.sign_message(message);
    let mut data = vec![0u8; 112];
    data[..2].copy_from_slice(&1u16.to_le_bytes());
    let fields: [(usize, u16); 7] = [
        (2, 48),
        (4, u16::MAX),
        (6, 16),
        (8, u16::MAX),
        (10, message_offset),
        (12, u16::try_from(message.len()).expect("message width")),
        (14, message_instruction_index),
    ];
    for (offset, value) in fields {
        data[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }
    data[16..48].copy_from_slice(&signer.pubkey().to_bytes());
    data[48..112].copy_from_slice(signature.as_ref());
    Instruction {
        program_id: Pubkey::new_from_array(ED25519_PROGRAM_ID_3_0),
        accounts: Vec::new(),
        data,
    }
}

async fn submit(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    signers: &[&Keypair],
) -> Result<(), BanksClientError> {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let mut all: Vec<&Keypair> = vec![&context.payer];
    all.extend_from_slice(signers);
    let transaction = Transaction::new_signed_with_payer(
        instructions,
        Some(&context.payer.pubkey()),
        &all,
        blockhash,
    );
    context.banks_client.process_transaction(transaction).await
}

async fn record_bytes(context: &mut ProgramTestContext, record: Pubkey) -> Vec<u8> {
    context
        .banks_client
        .get_account(record)
        .await
        .expect("bank read")
        .expect("record exists")
        .data
}

fn fold(running: [u8; 32], body: &[u8]) -> [u8; 32] {
    hashv(&[running.as_slice(), body]).to_bytes()
}

fn seed_digest(account_set_id: [u8; 32]) -> [u8; 32] {
    let mut preimage = [0u8; SET_DIGEST_SEED_PREIMAGE_BYTES];
    encode_set_digest_seed_preimage_v1(&mut preimage, account_set_id, OBSERVED_SLOT)
        .expect("seed preimage");
    hash(&preimage).to_bytes()
}

fn body_slice(message: &[u8]) -> &[u8] {
    let decoded = AttestationMessageV1::decode(message).expect("message decodes");
    let width = decoded.body().encoded_len();
    &message[message.len() - width..]
}

#[tokio::test]
async fn the_record_transport_runs_create_append_seal_and_retire() {
    let mut fixture = fixture(1, &[]);
    let mut context = fixture
        .test
        .take()
        .expect("test")
        .start_with_context()
        .await;

    submit(
        &mut context,
        &[fixture.create_instruction(4, 1)],
        &[&fixture.worker],
    )
    .await
    .expect("create the observation record");

    let mut running = seed_digest(fixture.account_set_id);
    {
        let data = record_bytes(&mut context, fixture.record).await;
        let view = RelayedObservationRecordViewV1::decode(&data).expect("record decodes");
        assert_eq!(view.phase(), Ok(RelayedRecordPhaseV1::Collecting));
        assert_eq!(view.set_count(), Ok(4));
        assert_eq!(view.filled_count(), Ok(0));
        assert_eq!(view.set_digest(), Ok(running));
        assert_eq!(
            view.observed_cluster_id(),
            Ok(SOLANA_MAINNET_GENESIS_HASH_V1)
        );
    }

    for index in 0..4 {
        let message = fixture.attestation(index, SOLANA_MAINNET_GENESIS_HASH_V1, OBSERVED_SLOT);
        let append = fixture.append_instruction(&message);
        let precompile = ed25519_instruction(
            &fixture.relayer,
            &message,
            u16::try_from(APPEND_OBSERVATION_PREFIX_BYTES).expect("offset"),
            1,
        );
        submit(&mut context, &[precompile, append], &[&fixture.worker])
            .await
            .unwrap_or_else(|error| panic!("append {index} failed: {error:?}"));
        running = fold(running, body_slice(&message));
        let data = record_bytes(&mut context, fixture.record).await;
        let view = RelayedObservationRecordViewV1::decode(&data).expect("record decodes");
        assert_eq!(
            view.filled_count(),
            Ok(u16::try_from(index + 1).expect("small"))
        );
        assert_eq!(view.set_digest(), Ok(running));
    }

    let seal = fixture.seal_message(running);
    let seal_ix = fixture.seal_instruction(&seal);
    let precompile = ed25519_instruction(
        &fixture.relayer,
        &seal,
        u16::try_from(SEAL_RECORD_PREFIX_BYTES).expect("offset"),
        1,
    );
    submit(&mut context, &[precompile, seal_ix], &[&fixture.worker])
        .await
        .expect("seal the completed set");

    let data = record_bytes(&mut context, fixture.record).await;
    let view = RelayedObservationRecordViewV1::decode(&data).expect("record decodes");
    assert_eq!(view.phase(), Ok(RelayedRecordPhaseV1::Sealed));
    assert_eq!(view.seal_count(), Ok(1));
    assert!(view.sealed_unix_seconds().expect("sealed time") > 0);

    // The Loopscale defense, executed against bytes the chain actually holds:
    // rebuild the deployment observation from the sealed record and hand it to
    // the existing release authenticator.
    let program = view.observation(0).expect("program body");
    let programdata = view.observation(1).expect("programdata body");
    let observed =
        reconstruct_deployment_observation_v1(program, programdata).expect("reconstruction");
    assert!(
        pinned_release(DEPLOYMENT_SLOT, ELF_DIGEST)
            .authenticate_deployment(observed)
            .is_ok()
    );
    // P-B: a venue redeploy moves the digest and the pinned release refuses.
    assert!(
        pinned_release(DEPLOYMENT_SLOT + 1, [0xef; 32])
            .authenticate_deployment(observed)
            .is_err()
    );

    let pool = view.observation(2).expect("pool body");
    assert_eq!(pool.data_len(), VIRTUAL_POOL_BYTES as u32);
    assert_eq!(
        pool.inline().get(MIGRATION_PROGRESS_OFFSET),
        Some(&MIGRATION_PROGRESS_CREATED_POOL)
    );
    assert_eq!(
        pool.inline().get(..8),
        Some(VIRTUAL_POOL_DISCRIMINATOR.as_slice())
    );

    // The rent goes where Core says this Market's rent goes, and the record's
    // whole balance moves: the worker prepaid it, and the beneficiary collects
    // it. Asserting the lamports is the difference between "the account is
    // gone" and "the account was returned".
    let record_lamports = context
        .banks_client
        .get_account(fixture.record)
        .await
        .expect("bank read")
        .expect("record exists")
        .lamports;
    let beneficiary_before = context
        .banks_client
        .get_account(fixture.rent_beneficiary)
        .await
        .expect("bank read")
        .expect("beneficiary exists")
        .lamports;
    submit(
        &mut context,
        &[fixture.retire_instruction()],
        &[&fixture.worker],
    )
    .await
    .expect("retire the record into the Market beneficiary");
    assert!(
        context
            .banks_client
            .get_account(fixture.record)
            .await
            .expect("bank read")
            .is_none_or(|account| account.data.is_empty() && account.lamports == 0)
    );
    assert_eq!(
        context
            .banks_client
            .get_account(fixture.rent_beneficiary)
            .await
            .expect("bank read")
            .expect("beneficiary exists")
            .lamports,
        beneficiary_before + record_lamports
    );
}

fn pinned_release(deployment_slot: u64, elf_digest: [u8; 32]) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        ProgramIdentityV1::new(DBC_PROGRAM).expect("program"),
        ProgramIdentityV1::new(LOADER_V3_PROGRAM_ID).expect("loader"),
        DBC_PROGRAMDATA,
        ContentId::new([0x77; 32]).expect("semantic release"),
        elf_digest,
        deployment_slot,
        ArtifactUpgradePolicyV1::ExactAuthority,
        Some(UPGRADE_AUTHORITY),
    )
    .expect("pinned artifact release")
}

fn refused(result: Result<(), BanksClientError>) -> TransactionError {
    match result {
        Err(BanksClientError::TransactionError(error)) => error,
        Err(BanksClientError::SimulationError { err, .. }) => err,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// The Resolution refusal taxonomy, as the adapter's discriminants.
///
/// A hostile case that refuses for the wrong reason is not evidence, so the
/// creation corpus below names the code it expects rather than accepting any
/// failure. `AccountFrame` is deliberately absent: a substitution that trips
/// the frame's no-alias rule has not reached the check it was written for.
const REFUSAL_MARKET_AUTHORITY: u32 = 3;
const REFUSAL_RESOLUTION_RELEASE: u32 = 5;
const REFUSAL_SOURCE_MATERIAL: u32 = 7;
const REFUSAL_TRANSITION: u32 = 12;

fn refused_with(result: Result<(), BanksClientError>, code: u32) {
    let error = refused(result);
    let TransactionError::InstructionError(_, InstructionError::Custom(observed)) = error else {
        panic!("expected a program refusal, got {error:?}");
    };
    assert_eq!(observed, code, "refused, but not for the reason under test");
}

#[tokio::test]
async fn the_hostile_corpus_is_refused_by_the_real_adapter() {
    let outsider = Keypair::new();
    let mut fixture = fixture(1, &[]);
    let mut context = fixture
        .test
        .take()
        .expect("test")
        .start_with_context()
        .await;

    // Creation first, because these are the facts the new home introduced: who
    // owns the Market, which release set that Market selected, where its rent
    // returns, and which Source spec its material actually names. Each is a
    // fact of authenticated state, and a caller that supplies a different one
    // is refused before any account is created.
    //
    for (name, substitution, code) in [
        (
            "a Core Program the Market is not owned by",
            CreateSubstitution {
                core_program: Some(PROGRAM_ID),
                ..CreateSubstitution::default()
            },
            REFUSAL_MARKET_AUTHORITY,
        ),
        (
            "a complete activation cache for a release set this Market did not select",
            CreateSubstitution {
                activation: Some(fixture.decoy_activation),
                ..CreateSubstitution::default()
            },
            REFUSAL_RESOLUTION_RELEASE,
        ),
        (
            "a rent beneficiary the Market does not name",
            CreateSubstitution {
                rent_beneficiary: Some(Pubkey::new_from_array([0x66; 32])),
                ..CreateSubstitution::default()
            },
            REFUSAL_MARKET_AUTHORITY,
        ),
        (
            "a Source spec identity the authenticated material does not name",
            CreateSubstitution {
                source_spec_id: Some([0x9a; 32]),
                ..CreateSubstitution::default()
            },
            REFUSAL_SOURCE_MATERIAL,
        ),
    ] {
        let result = submit(
            &mut context,
            &[fixture.create_instruction_with(4, 1, substitution)],
            &[&fixture.worker],
        )
        .await;
        assert!(result.is_err(), "{name} was accepted");
        refused_with(result, code);
    }
    // A seal threshold the release key set does not carry.
    refused_with(
        submit(
            &mut context,
            &[fixture.create_instruction(4, 2)],
            &[&fixture.worker],
        )
        .await,
        REFUSAL_TRANSITION,
    );

    submit(
        &mut context,
        &[fixture.create_instruction(4, 1)],
        &[&fixture.worker],
    )
    .await
    .expect("create the observation record");

    // A signer outside the release key set.
    let message = fixture.attestation(0, SOLANA_MAINNET_GENESIS_HASH_V1, OBSERVED_SLOT);
    let append = fixture.append_instruction(&message);
    let forged = ed25519_instruction(
        &outsider,
        &message,
        u16::try_from(APPEND_OBSERVATION_PREFIX_BYTES).expect("offset"),
        1,
    );
    refused(submit(&mut context, &[forged, append.clone()], &[&fixture.worker]).await);

    // A signature over the right message but not immediately preceding.
    let precompile = ed25519_instruction(
        &fixture.relayer,
        &message,
        u16::try_from(APPEND_OBSERVATION_PREFIX_BYTES).expect("offset"),
        2,
    );
    let filler = Instruction {
        program_id: system_program::ID,
        accounts: Vec::new(),
        data: vec![0; 4],
    };
    refused(
        submit(
            &mut context,
            &[precompile, filler, append.clone()],
            &[&fixture.worker],
        )
        .await,
    );

    // A descriptor naming a message offset the instruction does not carry.
    let wrong_offset = ed25519_instruction(&fixture.relayer, &message, 0, 1);
    refused(
        submit(
            &mut context,
            &[wrong_offset, append.clone()],
            &[&fixture.worker],
        )
        .await,
    );

    // The devnet twin: the venue Program account is byte-identical across
    // clusters, so nothing but the signed genesis hash can refuse this.
    let devnet = fixture.attestation(0, SOLANA_DEVNET_GENESIS_HASH_V1, OBSERVED_SLOT);
    let devnet_append = fixture.append_instruction(&devnet);
    let devnet_signature = ed25519_instruction(
        &fixture.relayer,
        &devnet,
        u16::try_from(APPEND_OBSERVATION_PREFIX_BYTES).expect("offset"),
        1,
    );
    refused(
        submit(
            &mut context,
            &[devnet_signature, devnet_append],
            &[&fixture.worker],
        )
        .await,
    );

    // A properly signed observation of a different finalized slot.
    let stale = fixture.attestation(0, SOLANA_MAINNET_GENESIS_HASH_V1, OBSERVED_SLOT - 1);
    let stale_append = fixture.append_instruction(&stale);
    let stale_signature = ed25519_instruction(
        &fixture.relayer,
        &stale,
        u16::try_from(APPEND_OBSERVATION_PREFIX_BYTES).expect("offset"),
        1,
    );
    refused(
        submit(
            &mut context,
            &[stale_signature, stale_append],
            &[&fixture.worker],
        )
        .await,
    );

    // A truncated message.
    let truncated = message.get(..message.len() - 1).expect("prefix").to_vec();
    let truncated_append = fixture.append_instruction(&truncated);
    let truncated_signature = ed25519_instruction(
        &fixture.relayer,
        &truncated,
        u16::try_from(APPEND_OBSERVATION_PREFIX_BYTES).expect("offset"),
        1,
    );
    refused(
        submit(
            &mut context,
            &[truncated_signature, truncated_append],
            &[&fixture.worker],
        )
        .await,
    );

    // The honest append still lands, so every refusal above was a refusal and
    // not a wedged record.
    let honest = ed25519_instruction(
        &fixture.relayer,
        &message,
        u16::try_from(APPEND_OBSERVATION_PREFIX_BYTES).expect("offset"),
        1,
    );
    submit(&mut context, &[honest, append.clone()], &[&fixture.worker])
        .await
        .expect("the honest append lands");

    // Replay of the same position.
    let replay = ed25519_instruction(
        &fixture.relayer,
        &message,
        u16::try_from(APPEND_OBSERVATION_PREFIX_BYTES).expect("offset"),
        1,
    );
    refused(submit(&mut context, &[replay, append], &[&fixture.worker]).await);

    // A seal before the set is complete.
    let running = fold(seed_digest(fixture.account_set_id), body_slice(&message));
    let early = fixture.seal_message(running);
    let early_ix = fixture.seal_instruction(&early);
    let early_signature = ed25519_instruction(
        &fixture.relayer,
        &early,
        u16::try_from(SEAL_RECORD_PREFIX_BYTES).expect("offset"),
        1,
    );
    refused(
        submit(
            &mut context,
            &[early_signature, early_ix],
            &[&fixture.worker],
        )
        .await,
    );
}

#[tokio::test]
async fn a_quorum_below_the_release_threshold_never_seals() {
    let second = Keypair::new();
    let third = Keypair::new();
    let mut fixture = fixture(3, &[second.pubkey().to_bytes(), third.pubkey().to_bytes()]);
    let mut context = fixture
        .test
        .take()
        .expect("test")
        .start_with_context()
        .await;
    submit(
        &mut context,
        &[fixture.create_instruction(4, 3)],
        &[&fixture.worker],
    )
    .await
    .expect("create the observation record");

    let mut running = seed_digest(fixture.account_set_id);
    for index in 0..4 {
        let message = fixture.attestation(index, SOLANA_MAINNET_GENESIS_HASH_V1, OBSERVED_SLOT);
        let append = fixture.append_instruction(&message);
        let precompile = ed25519_instruction(
            &fixture.relayer,
            &message,
            u16::try_from(APPEND_OBSERVATION_PREFIX_BYTES).expect("offset"),
            1,
        );
        submit(&mut context, &[precompile, append], &[&fixture.worker])
            .await
            .expect("append");
        running = fold(running, body_slice(&message));
    }

    let seal = fixture.seal_message(running);
    for signer in [&fixture.relayer, &second] {
        let seal_ix = fixture.seal_instruction(&seal);
        let precompile = ed25519_instruction(
            signer,
            &seal,
            u16::try_from(SEAL_RECORD_PREFIX_BYTES).expect("offset"),
            1,
        );
        submit(&mut context, &[precompile, seal_ix], &[&fixture.worker])
            .await
            .expect("partial seal");
    }
    {
        let data = record_bytes(&mut context, fixture.record).await;
        let view = RelayedObservationRecordViewV1::decode(&data).expect("record decodes");
        assert_eq!(view.seal_count(), Ok(2));
        assert_eq!(
            view.phase(),
            Ok(RelayedRecordPhaseV1::Collecting),
            "m-1 seals must not seal the record"
        );
    }

    // The same member sealing again is refused rather than counted twice.
    let repeat = fixture.seal_instruction(&seal);
    let precompile = ed25519_instruction(
        &fixture.relayer,
        &seal,
        u16::try_from(SEAL_RECORD_PREFIX_BYTES).expect("offset"),
        1,
    );
    refused(submit(&mut context, &[precompile, repeat], &[&fixture.worker]).await);

    let final_ix = fixture.seal_instruction(&seal);
    let precompile = ed25519_instruction(
        &third,
        &seal,
        u16::try_from(SEAL_RECORD_PREFIX_BYTES).expect("offset"),
        1,
    );
    submit(&mut context, &[precompile, final_ix], &[&fixture.worker])
        .await
        .expect("the quorum seals");
    let data = record_bytes(&mut context, fixture.record).await;
    let view = RelayedObservationRecordViewV1::decode(&data).expect("record decodes");
    assert_eq!(view.phase(), Ok(RelayedRecordPhaseV1::Sealed));
    assert_eq!(view.seal_count(), Ok(3));
}

/// The swap tripwire of §4.10, made checkable rather than rhetorical.
///
/// "Swapping trust roots never moves semantics" is only worth saying if it is
/// falsifiable. Two provider releases with **disjoint relayer key sets** — a
/// 1-of-1 and a 3-of-5 — differ in `provider_deployment_release_id` and agree,
/// byte for byte, in `decoding_rules_id`. If a future transport ever needs a
/// different `decoding_rules_id`, the family has leaked semantics into
/// transport and this test is where that shows up.
#[test]
fn two_disjoint_relayer_key_sets_share_one_decoding_rules_identity() {
    let (_, account_set_id) = account_set();
    let config = RelayedAdapterConfigV1::new(
        account_set_id,
        0,
        0,
        u64::from(WINDOW_MAX_AGE_SECONDS),
        CLUSTER_SKEW_SECONDS,
    )
    .expect("relayed adapter config");
    let decoding_rules_id = hash(&config.to_bytes().expect("config bytes")).to_bytes();

    let solo = RelayerKeySetV1::new(&[[0x11; 32]], 1).expect("1-of-1 key set");
    let mut quorum_keys = [[0x21; 32], [0x22; 32], [0x23; 32], [0x24; 32], [0x25; 32]];
    quorum_keys.sort_unstable();
    let quorum = RelayerKeySetV1::new(&quorum_keys, 3).expect("3-of-5 key set");
    let solo_id = hash(&solo.to_bytes().expect("bytes")).to_bytes();
    let quorum_id = hash(&quorum.to_bytes().expect("bytes")).to_bytes();
    assert_ne!(solo_id, quorum_id, "the two trust roots must be distinct");

    let release_of = |deployment: [u8; 32]| {
        ProviderReleaseV1::new(
            source_id(RELAYED_FAMILY_RELEASE_ID_V1),
            source_id(RELAYED_PROVIDER_EXTENSION_RELEASE_ID_V1),
            source_id(deployment),
            source_id(decoding_rules_id),
            source_id(RELAYED_RECORD_TRANSPORT_PROFILE_ID_V1),
        )
    };
    let poa = release_of(solo_id);
    let multi = release_of(quorum_id);

    assert_ne!(
        poa.provider_deployment_release_id(),
        multi.provider_deployment_release_id(),
        "the trust root must be the thing that moved"
    );
    assert_eq!(
        poa.decoding_rules_id().to_bytes(),
        multi.decoding_rules_id().to_bytes(),
        "swapping the trust root moved the decoding rules: the family has leaked semantics into transport"
    );
    assert_eq!(poa.provider_family_id(), multi.provider_family_id());
    assert_eq!(poa.transport_profile_id(), multi.transport_profile_id());

    // And the account set both rows resolve against is the same 32 bytes.
    assert_eq!(config.account_set_id(), account_set_id);
}
