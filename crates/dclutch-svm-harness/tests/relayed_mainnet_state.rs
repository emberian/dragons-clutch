//! Executable evidence for the `RelayedMainnetStateV1` observation-record
//! transport, against the real compiled dClutch SBF ELF.
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
//! The venue fixture is shaped from the published Meteora DBC source: a
//! 424-byte `VirtualPool` account (8-byte Anchor discriminator
//! `d5e005d1 6245775c` plus the 416-byte `PoolState` body) with
//! `migration_progress` at account offset 308 and `is_migrated` at 305. The
//! layout is real; the values are invented, so the fixture is labelled
//! synthetic-value rather than synthetic-shape.

use std::{env, fs, path::PathBuf};

use dclutch_core_contract::{ContentId, MarketIdentity, MarketRoot, Phase};
use dclutch_market_contract::market::{CategoricalMarketV1, CategoricalSettlementSummaryV1};
use dclutch_product_contract::{
    ContentId as ProductContentId,
    capacity::CapacityProfileId,
    product::{InstanceV1, InstanceV1Input},
    result_domain::{FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1, FiniteResultDomainV1},
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{ArtifactReleaseV1, ArtifactUpgradePolicyV1};
use dclutch_relay_contract::{
    RELAYED_ADAPTER_CONFIG_SCHEMA_RELEASE_PREIMAGE_V1, RELAYED_FAMILY_RELEASE_ID_V1,
    RELAYED_RECORD_PDA_DOMAIN_V1, RELAYED_RECORD_TRANSPORT_PROFILE_ID_V1,
    RELAYER_KEY_SET_SCHEMA_RELEASE_PREIMAGE_V1, SHA256_EMPTY_DIGEST, SOLANA_DEVNET_GENESIS_HASH_V1,
    SOLANA_MAINNET_GENESIS_HASH_V1,
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
use dclutch_release_set_contract::ProgramIdentityV1;
use dclutch_rent_contract::{RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority, RentCreditV1};
use dclutch_source_contract::{
    CapacityEnvelope as SourceCapacityEnvelope, ContentId as SourceContentId, ProviderReleaseV1,
    PythAdapterConfigV1, RELAYED_PROVIDER_EXTENSION_RELEASE_ID_V1, ResolutionPolicyV1,
    RoundingBoundary, SOURCE_MATERIAL_BYTES, SOURCE_MATERIAL_SCHEMA_RELEASE_PREIMAGE_V1,
    SourceAccessProfile, SourceCapacityProfileV1, SourceMaterialInputV1, SourceSpecV1,
    StatisticKind, StatisticSpecV1, WindowKind, WindowSpecV1, encode_source_material_into_v1,
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
use solana_sdk_ids::{system_program, sysvar};
use solana_transaction::{Transaction, TransactionError};

const PROGRAM_ID: Pubkey = Pubkey::new_from_array([71; 32]);
const GENERATION: u64 = 73;
/// Two Market children exist before any record: the Fund and the custody child.
const OPEN_CHILD_COUNT: u64 = 2;
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

fn require_real_elf() -> Vec<u8> {
    let directory = env::var("SBF_OUT_DIR").expect(
        "SBF_OUT_DIR is required; build the real adapter with `cargo build-sbf --manifest-path programs/dclutch-sbf/Cargo.toml` and point SBF_OUT_DIR at target/deploy",
    );
    let artifact = PathBuf::from(directory).join("dclutch_sbf.so");
    let bytes = fs::read(&artifact).unwrap_or_else(|error| {
        panic!(
            "cannot read the compiled dClutch SBF ELF {}: {error}",
            artifact.display()
        )
    });
    assert_eq!(bytes.get(..4), Some(&[0x7f, b'E', b'L', b'F'][..]));
    eprintln!("dClutch SBF ELF SHA-256: {:?}", hash(&bytes).to_bytes());
    bytes
}

fn protocol_account(data: Vec<u8>) -> Account {
    Account {
        lamports: Rent::default().minimum_balance(data.len()).max(1),
        data,
        owner: PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    }
}

fn finalized_record(schema_label: &[u8], content: Vec<u8>) -> (Pubkey, Pubkey, Account, [u8; 32]) {
    let schema = hash(schema_label).to_bytes();
    let digest = hash(&content).to_bytes();
    let raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, schema.as_slice(), digest.as_slice()],
        &PROGRAM_ID,
    )
    .0;
    let cursor = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            schema.as_slice(),
            digest.as_slice(),
        ],
        &PROGRAM_ID,
    )
    .0;
    (raw, cursor, protocol_account(content), digest)
}

fn product_id(bytes: [u8; 32]) -> ProductContentId {
    ProductContentId::new(bytes).expect("nonzero deterministic Product identity")
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

fn source_material_bytes(
    account_set_id: [u8; 32],
    key_set_digest: [u8; 32],
    adapter_config_digest: [u8; 32],
) -> (Vec<u8>, [u8; 32], [u8; 32], [u8; 32]) {
    let _ = account_set_id;
    let result_domain =
        FiniteResultDomainV1::new(product_id([0xb1; 32]), product_id([0xb2; 32]), 1, &[])
            .expect("canonical binary Product result domain");
    let result_domain_bytes = result_domain.to_bytes();
    let result_domain_digest = hashv(&[
        FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1,
        &[0],
        result_domain_bytes.as_slice(),
    ])
    .to_bytes();
    let claim_id = [33; 32];
    let instance = InstanceV1::new(InstanceV1Input {
        terms_id: product_id([32; 32]),
        occurrence_id: product_id([34; 32]),
        claim_basis_id: product_id(claim_id),
        result_domain_id: product_id(result_domain_digest),
        capacity_profile_id: CapacityProfileId::new(product_id([35; 32])),
        partition_cell_count: 2,
    })
    .expect("canonical Product instance");
    let instance_digest = hash(&instance.to_bytes()).to_bytes();
    let capacity = SourceCapacityProfileV1::new(
        SourceCapacityEnvelope::Measured,
        1,
        0,
        source_id([36; 32]),
        source_id([37; 32]),
        512,
        // The record is a direct Market child, so the number of them a caller
        // may impose on a Market is bounded here and nowhere else.
        4,
    )
    .expect("canonical Source capacity");
    let capacity_id = source_id(hash(&capacity.to_bytes()).to_bytes());
    let unused_inline =
        PythAdapterConfigV1::new([0x2a; 32], -8, 100).expect("inline slot placeholder");
    let provider = ProviderReleaseV1::new(
        source_id(RELAYED_FAMILY_RELEASE_ID_V1),
        source_id(RELAYED_PROVIDER_EXTENSION_RELEASE_ID_V1),
        // The relayer key set IS the provider deployment release.
        source_id(key_set_digest),
        // ...and the pinned ordered account set is a decoding-rules fact.
        source_id(adapter_config_digest),
        source_id(RELAYED_RECORD_TRANSPORT_PROFILE_ID_V1),
    );
    let provider_id = source_id(hash(&provider.to_bytes()).to_bytes());
    // The V1 material carries an inline Pyth-typed adapter config slot whose
    // digest it binds to `adapter_config_id`. The relayed family cannot use that
    // slot -- it is 64 bytes and Pyth-typed -- so the relay configuration is a
    // separate raw record named by `decoding_rules_id`, and the inline slot is
    // filled with a canonical placeholder. A V2 material would drop the slot.
    let source = SourceSpecV1::new(
        source_id(result_domain.coordinate_domain_id().to_bytes()),
        source_id(result_domain.result_unit_id().to_bytes()),
        provider_id,
        SourceAccessProfile::RelayedObservationRecord,
        source_id(hash(&unused_inline.to_bytes()).to_bytes()),
        capacity_id,
    );
    let source_spec_id = source_id(hash(&source.to_bytes()).to_bytes());
    let window = WindowSpecV1::new(
        source_spec_id,
        WindowKind::Terminal,
        CREATED_UNIX,
        CREATED_UNIX,
        WINDOW_MAX_AGE_SECONDS,
        1,
        source_id([41; 32]),
    )
    .expect("pinned terminal window");
    let window_id = source_id(hash(&window.to_bytes()).to_bytes());
    let statistic = StatisticSpecV1::new(
        source_id(result_domain.result_unit_id().to_bytes()),
        source_id(result_domain.result_unit_id().to_bytes()),
        StatisticKind::TerminalSample,
        RoundingBoundary::ExactRational,
        1,
        0,
        capacity_id,
        source_id([42; 32]),
        capacity,
    )
    .expect("canonical terminal statistic");
    let statistic_id = source_id(hash(&statistic.to_bytes()).to_bytes());
    let policy = ResolutionPolicyV1::new(
        capacity_id,
        source_id(instance_digest),
        source_spec_id,
        window_id,
        statistic_id,
        source_id(result_domain_digest),
        None,
    );
    let mut material = vec![0; SOURCE_MATERIAL_BYTES];
    encode_source_material_into_v1(
        &mut material,
        SourceMaterialInputV1 {
            policy: &policy,
            capacity_profile_id: capacity_id,
            capacity_profile: &capacity,
            primary_source_id: source_spec_id,
            primary_source: &source,
            primary_provider_release_id: provider_id,
            primary_provider_release: &provider,
            primary_adapter_config: &unused_inline,
            window_id,
            window: &window,
            statistic_id,
            statistic: &statistic,
            product_instance_id: source_id(instance_digest),
            product_instance: &instance,
            result_domain: &result_domain,
            recovery: None,
        },
    )
    .expect("canonical Product-bound Source material");
    (
        material,
        instance_digest,
        claim_id,
        source_spec_id.to_bytes(),
    )
}

struct Fixture {
    test: Option<ProgramTest>,
    relayer: Keypair,
    worker: Keypair,
    market: Pubkey,
    record: Pubkey,
    record_bump: u8,
    material: Pubkey,
    material_cursor: Pubkey,
    material_id: [u8; 32],
    source_spec_id: [u8; 32],
    key_set: Pubkey,
    key_set_cursor: Pubkey,
    config: Pubkey,
    config_cursor: Pubkey,
    rent_credit: Pubkey,
    rent_beneficiary: Pubkey,
    account_set_id: [u8; 32],
    positions: Vec<Position>,
}

fn fixture(seal_threshold: u8, extra_keys: &[[u8; 32]]) -> Fixture {
    let elf = require_real_elf();
    let relayer = Keypair::new();
    let worker = Keypair::new();
    let (_, account_set_id) = account_set();

    let mut keys = vec![relayer.pubkey().to_bytes()];
    keys.extend_from_slice(extra_keys);
    keys.sort_unstable();
    let key_set_value =
        RelayerKeySetV1::new(&keys, seal_threshold).expect("canonical relayer key set");
    let (key_set, key_set_cursor, key_set_account, key_set_digest) = finalized_record(
        RELAYER_KEY_SET_SCHEMA_RELEASE_PREIMAGE_V1,
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
    let (config, config_cursor, config_account, config_digest) = finalized_record(
        RELAYED_ADAPTER_CONFIG_SCHEMA_RELEASE_PREIMAGE_V1,
        config_value.to_bytes().expect("config bytes").to_vec(),
    );

    let (material_bytes, product_instance_id, claim_id, source_spec_id) =
        source_material_bytes(account_set_id, key_set_digest, config_digest);
    // The adapter-owned content links the record contract re-checks on
    // authentication. Asserting them here turns a Custom(5) from the bank into
    // a named fixture failure.
    for (id_offset, value_offset, length) in [
        (256usize, 288usize, 112usize),
        (400, 432, 192),
        (624, 656, 112),
        (768, 800, 176),
        (1360, 1392, 176),
        (544, 1568, 64),
    ] {
        let expected = hash(&material_bytes[value_offset..value_offset + length]).to_bytes();
        assert_eq!(
            &material_bytes[id_offset..id_offset + 32],
            expected.as_slice(),
            "material content link at {id_offset} -> {value_offset} does not hold"
        );
    }
    let (material, material_cursor, material_account, material_digest) =
        finalized_record(SOURCE_MATERIAL_SCHEMA_RELEASE_PREIMAGE_V1, material_bytes);
    let material_id = ContentId::new(material_digest).expect("SourceMaterial ID");

    let rent_beneficiary = Pubkey::new_from_array([0x61; 32]);
    let refund_authority =
        RefundAuthority::new(rent_beneficiary.to_bytes()).expect("refund authority");
    let refund_authority_bytes = refund_authority.to_bytes();
    let (rent_credit, rent_credit_bump) = Pubkey::find_program_address(
        &[RENT_CREDIT_PDA_DOMAIN_V1, refund_authority_bytes.as_slice()],
        &PROGRAM_ID,
    );
    let rent_credit_state = RentCreditV1::new(refund_authority, rent_credit_bump);

    let identity = MarketIdentity::new(
        ContentId::new([31; 32]).expect("Realm ID"),
        ContentId::new(product_instance_id).expect("Product Instance ID"),
        ContentId::new(claim_id).expect("Claim ID"),
        material_id,
        ContentId::new([25; 32]).expect("capability manifest ID"),
        GENERATION,
    );
    let identity_digest = hash(&identity.to_bytes()).to_bytes();
    let market = Pubkey::find_program_address(
        &[b"dclutch/market-root/v1", identity_digest.as_slice()],
        &PROGRAM_ID,
    )
    .0;
    let mut root =
        MarketRoot::founding(identity, rent_beneficiary.to_bytes()).expect("founding root");
    root.register_child(GENERATION, 0).expect("Fund child");
    root.register_child(GENERATION, 1).expect("custody child");
    root.transition_phase(GENERATION, Phase::Open)
        .expect("Open prerequisite state");
    let market_value =
        CategoricalMarketV1::<2>::new(root, 0, [0, 0], CategoricalSettlementSummaryV1::empty())
            .expect("provider-neutral Open Market");
    let mut market_bytes =
        vec![0_u8; CategoricalMarketV1::<2>::encoded_len().expect("binary Market width")];
    market_value
        .encode(&mut market_bytes)
        .expect("canonical Open Market bytes");

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

    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.add_program("dclutch_sbf", PROGRAM_ID, None);
    let _ = elf;
    test.add_account(market, protocol_account(market_bytes));
    test.add_account(material, material_account);
    test.add_account(material_cursor, Account::new(0, 0, &system_program::ID));
    test.add_account(key_set, key_set_account);
    test.add_account(key_set_cursor, Account::new(0, 0, &system_program::ID));
    test.add_account(config, config_account);
    test.add_account(config_cursor, Account::new(0, 0, &system_program::ID));
    test.add_account(
        rent_credit,
        protocol_account(rent_credit_state.to_bytes().to_vec()),
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
        record,
        record_bump,
        material,
        material_cursor,
        material_id: material_digest,
        source_spec_id,
        key_set,
        key_set_cursor,
        config,
        config_cursor,
        rent_credit,
        rent_beneficiary,
        account_set_id,
        positions: positions(),
    }
}

impl Fixture {
    fn create_instruction(&self, set_count: u16, seal_threshold: u8) -> Instruction {
        let request = CreateRecordInstructionV1::new(
            GENERATION,
            OBSERVED_SLOT,
            OPEN_CHILD_COUNT,
            set_count,
            seal_threshold,
            self.record_bump,
            self.material_id,
            self.source_spec_id,
            self.rent_beneficiary.to_bytes(),
        )
        .expect("create request");
        Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(self.worker.pubkey(), true),
                AccountMeta::new(self.market, false),
                AccountMeta::new(self.record, false),
                AccountMeta::new_readonly(self.material, false),
                AccountMeta::new_readonly(self.material_cursor, false),
                AccountMeta::new_readonly(self.key_set, false),
                AccountMeta::new_readonly(self.key_set_cursor, false),
                AccountMeta::new_readonly(self.config, false),
                AccountMeta::new_readonly(self.config_cursor, false),
                AccountMeta::new_readonly(self.rent_credit, false),
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
            accounts: vec![
                AccountMeta::new(self.worker.pubkey(), true),
                AccountMeta::new_readonly(self.market, false),
                AccountMeta::new(self.record, false),
                AccountMeta::new_readonly(self.key_set, false),
                AccountMeta::new_readonly(self.key_set_cursor, false),
                AccountMeta::new_readonly(sysvar::rent::ID, false),
                AccountMeta::new_readonly(sysvar::instructions::ID, false),
                AccountMeta::new_readonly(sysvar::clock::ID, false),
            ],
            data,
        }
    }

    fn signature_frame(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.worker.pubkey(), true),
            AccountMeta::new_readonly(self.market, false),
            AccountMeta::new(self.record, false),
            AccountMeta::new_readonly(self.key_set, false),
            AccountMeta::new_readonly(self.key_set_cursor, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
            AccountMeta::new_readonly(sysvar::instructions::ID, false),
            AccountMeta::new_readonly(sysvar::clock::ID, false),
        ]
    }

    fn retire_instruction(&self, child_count: u64) -> Instruction {
        Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(self.worker.pubkey(), true),
                AccountMeta::new(self.market, false),
                AccountMeta::new(self.record, false),
                AccountMeta::new(self.rent_credit, false),
            ],
            data: RetireRecordInstructionV1::new(GENERATION, child_count)
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

    submit(
        &mut context,
        &[fixture.retire_instruction(OPEN_CHILD_COUNT + 1)],
        &[&fixture.worker],
    )
    .await
    .expect("retire the record into its RentCredit");
    assert!(
        context
            .banks_client
            .get_account(fixture.record)
            .await
            .expect("bank read")
            .is_none_or(|account| account.data.is_empty())
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
