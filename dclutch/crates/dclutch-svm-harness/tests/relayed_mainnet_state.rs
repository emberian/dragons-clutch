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

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    CapabilityEntryV1, CapabilityFundingDerivationV1, CapabilityManifestV1, CompartmentFundingV1,
    ContentId as CapabilityContentId, FUNDING_STATE_BYTES, FundingAmountsV1,
    FundingCustodyObservationV1, FundingQuoteV1, FundingStateV1, FundingStatus,
    MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_core_contract::ContentId;
use dclutch_market_core_codec::{
    CoreState, Identity as CoreIdentity, MarketCoreStateSeedsV2, MarketIdentity, Phase, Readiness,
};
use dclutch_product_runtime_v2::{
    ContentId as ProductContentId, PortfolioInputV2, ResultDomainInputV2, compile_portfolio_v2,
    compile_result_domain_v2, portfolio_record_bytes, result_domain_record_bytes,
};
use dclutch_product_runtime_v2_admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_BYTES_V2, PRODUCT_RECORD_SCHEMA_ID_V2, ProductRecordV2,
    RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_program_test_evidence::TransactionEvidence;
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ARTIFACT_RELEASE_SCHEMA_ID_V1, ActivatedExecutionReleaseSetV1, ArtifactActivationInputV1,
    ArtifactReleaseV1, ArtifactUpgradePolicyV1, DeploymentObservationV1,
    activate_execution_role_into_v1, initialize_activation_cache_v1,
};
use dclutch_relay_contract::{
    RELAYED_ADAPTER_CONFIG_SCHEMA_RELEASE_ID_V1, RELAYED_ADAPTER_CONFIG_SCHEMA_RELEASE_PREIMAGE_V1,
    RELAYED_FAMILY_RELEASE_ID_V1, RELAYED_RECORD_PDA_DOMAIN_V1,
    RELAYED_RECORD_TRANSPORT_PROFILE_ID_V1, RELAYER_KEY_SET_SCHEMA_RELEASE_ID_V1,
    RELAYER_KEY_SET_SCHEMA_RELEASE_PREIMAGE_V1, SHA256_EMPTY_DIGEST, SOLANA_DEVNET_GENESIS_HASH_V1,
    SOLANA_MAINNET_GENESIS_HASH_V1,
    identity::{LOADER_V3_PROGRAM_ID, reconstruct_deployment_observation_v1},
    instruction::{
        APPEND_OBSERVATION_PREFIX_BYTES, AppendObservationInstructionV1,
        CONSUME_RECORD_PREFIX_BYTES, CommitDeadlineFailureInstructionV1,
        ConsumeRecordInstructionV1, CreateRecordInstructionV1, RetireRecordInstructionV1,
        SEAL_RECORD_PREFIX_BYTES, SealRecordInstructionV1,
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
use dclutch_resolution_codec::{
    RESOLUTION_CERTIFICATE_BYTES_V2, RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
    RESOLUTION_CONTROLLER_RELEASE_ID_V4, ResolutionCertificateKindV2, ResolutionCertificateV2,
};
use dclutch_resolution_proof_sbf::ResolutionError;
use dclutch_source_contract::{
    CapacityEnvelope as SourceCapacityEnvelope, ContentId as SourceContentId,
    PROVIDER_RELEASE_SCHEMA_ID_V1, ProviderReleaseV1, RELAYED_PROVIDER_EXTENSION_RELEASE_ID_V1,
    RoundingBoundary, SOURCE_FAILURE_POLICY_RELEASE_ID_V2, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
    SOURCE_SPEC_SCHEMA_ID_V1, STATISTIC_SPEC_SCHEMA_ID_V1, SourceAccessProfile,
    SourceCapacityProfileV1, SourceMaterialV2, SourceResolutionPhaseV1, SourceResolutionStateV2,
    SourceSpecV1, StatisticKind, StatisticSpecV1, WINDOW_SPEC_SCHEMA_ID_V1, WindowKind,
    WindowSpecV1,
};
use solana_account::Account;
use solana_program::clock::Clock;
use solana_program::instruction::InstructionError;
use solana_program::{
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
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
/// The window's closed lower bound. The default fixture's attested foreign
/// clock (`mainnet_clock_body`) sits at `CREATED_UNIX`, the closing edge, so
/// widening the window down to here changes nothing about the happy path --
/// it only gives the window real width to refuse *below*, the way
/// `require_window_admits` distinguishes "before the market opened" from
/// "after it closed." A degenerate `start == end` window could never exercise
/// that lower edge at all.
const WINDOW_START_UNIX: i64 = CREATED_UNIX - 900;
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

/// The devnet `Clock` this campaign pins, so both time bounds are exact rather
/// than whatever wall clock the bank was started with.
const DEVNET_NOW: i64 = CREATED_UNIX + 600;
/// The terminal sequence naming the certificate this resolution writes.
const TERMINAL_SEQUENCE: u64 = 1;

/// The bounty this market disclosed, before it opened, for whoever walks it to
/// its pre-disclosed failure outcome.
///
/// It is a manifest quote rather than a walk-time argument, which is the whole
/// difference between a prepaid permissionless path and an unfunded promise:
/// `ResolutionCertificateV2::validate_shape` refuses a `ResolutionFailure`
/// whose `work_paid` is zero, so a walk that could not be paid for could not
/// have encoded its own certificate either.
const BOUNTY: u64 = 250_000;
/// Lean-owned Runtime V2 certificate wire tags, used as PDA seeds.
const RESOLUTION_SUCCESS_KIND: u8 = 1;
const RESOLUTION_FAILURE_KIND: u8 = 4;
/// The demo graduation Product: one ordinary outcome plus the explicit failure
/// outcome.  A terminal-window graduation proposition can only ever be *proved*
/// by graduation, so there is exactly one ordinary cell to select and the other
/// half of the partition is the pre-disclosed failure the deadline walk reaches.
///
/// One ordinary cell means zero cuts, and that is the honest shape rather than a
/// simplification: a domain with a cut at `CreatedPool` would have an ordinary
/// cell nothing could ever select, and a partition with a dead cell is a
/// partition minting liabilities against an outcome that cannot happen.
const GRADUATION_OUTCOME_COUNT: u32 = 2;
/// Ordinary regions, which is `GRADUATION_OUTCOME_COUNT` minus the failure cell.
const GRADUATION_REGION_COUNT: u32 = GRADUATION_OUTCOME_COUNT - 1;

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

fn capability_id(bytes: [u8; 32]) -> CapabilityContentId {
    CapabilityContentId::new(bytes).expect("nonzero capability identity")
}

/// One Resolution-controller funding entry, quoting this market's own bounty.
///
/// `config_id` is the only thing that varies across the three entries, and it
/// is what makes one of them the *explicit-failure* compartment rather than
/// the recovery or exhaustion one: `funded::plan_funding_release` admits the
/// entry whose configuration is this market's own Source material and refuses
/// every other, which is exactly the binding `core_effect`'s
/// `authenticate_funding_entries` established when the three were created.
///
/// The quote pays rent out of the Rent compartment and the walk out of the
/// Bounty compartment. Nothing else is applicable: this capability creates one
/// account and pays one worker.
fn funding_entry(config: [u8; 32]) -> CapabilityEntryV1 {
    let quote = FundingQuoteV1::new(
        FundingAmountsV1::new(
            CompartmentFundingV1::native_lamports(
                Rent::default().minimum_balance(FUNDING_STATE_BYTES),
            )
            .expect("funding-state rent"),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::native_lamports(BOUNTY).expect("worker bounty"),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
        )
        .expect("typed funding amounts"),
        None,
    )
    .expect("funding quote");
    CapabilityEntryV1::new(
        capability_id(hashv(&[b"dclutch/relayed/capability/", &config]).to_bytes()),
        capability_id(RESOLUTION_CONTROLLER_RELEASE_ID_V4),
        capability_id(config),
        capability_id([0xa4; 32]),
        capability_id([0xa5; 32]),
        capability_id([0xa6; 32]),
        ActivationPolicy::RequiredAtFounding,
        0,
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        quote,
    )
    .expect("Resolution funding entry")
}

/// The `FundingStateV1` a market's founding leaves in one compartment.
///
/// Built through the same two production calls `core_effect::new_funding`
/// makes -- `FundingStateV1::new` against an observed native custody, then
/// `activate` -- rather than by writing bytes that look right. Which account it
/// lands in is `CapabilityFundingDerivationV1`'s answer, not this fixture's, so
/// the address folds in the Market, the generation, the manifest identity and
/// the state's own entry index. That is why presenting a *different* live
/// compartment of the same market is refused by a config comparison rather
/// than by an address check: the exhaustion compartment sits at its own correct
/// address too.
fn pending_funding(manifest: CapabilityManifestV1<'_>, entry_index: u16) -> FundingStateV1 {
    let rent = Rent::default().minimum_balance(FUNDING_STATE_BYTES);
    let custody = native_custody(
        rent.checked_mul(2)
            .and_then(|value| value.checked_add(BOUNTY))
            .expect("bounded funding custody"),
    );
    FundingStateV1::new(manifest_identity(manifest), manifest, entry_index, custody)
        .expect("pending FundingState")
}

fn native_custody(lamports: u64) -> FundingCustodyObservationV1 {
    FundingCustodyObservationV1::native_only(
        lamports,
        Rent::default().minimum_balance(FUNDING_STATE_BYTES),
    )
    .expect("native funding custody")
}

/// A compartment's own address, from the state its own bytes describe.
fn funding_key(
    market: Pubkey,
    manifest: CapabilityManifestV1<'_>,
    entry_index: u16,
) -> (Pubkey, FundingStateV1) {
    let pending = pending_funding(manifest, entry_index);
    let derivation = CapabilityFundingDerivationV1::new(
        market.to_bytes(),
        GENERATION,
        manifest_identity(manifest),
        manifest,
        pending,
    )
    .expect("funding derivation");
    (
        Pubkey::find_program_address(&derivation.seed_components(), &PROGRAM_ID).0,
        pending,
    )
}

/// Install one activated compartment, holding exactly rent plus its bounty.
fn add_active_funding(
    test: &mut ProgramTest,
    market: Pubkey,
    manifest: CapabilityManifestV1<'_>,
    entry_index: u16,
) -> Pubkey {
    let rent = Rent::default().minimum_balance(FUNDING_STATE_BYTES);
    let (key, mut state) = funding_key(market, manifest, entry_index);
    state
        .activate(
            manifest_identity(manifest),
            manifest,
            native_custody(
                rent.checked_mul(2)
                    .and_then(|value| value.checked_add(BOUNTY))
                    .expect("bounded funding custody"),
            ),
            1,
        )
        .expect("active FundingState");
    test.add_account(
        key,
        Account {
            lamports: rent + BOUNTY,
            data: state.to_bytes().to_vec(),
            owner: PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    key
}

/// The manifest's own content identity, recomputed from the encoded bytes.
fn manifest_identity(manifest: CapabilityManifestV1<'_>) -> CapabilityContentId {
    capability_id(hash(manifest.as_bytes()).to_bytes())
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

/// The Product Runtime V2 graph a consumption maps its result through.
///
/// One ordinary region and one explicit failure region.  The single cut at
/// `MIGRATION_PROGRESS_CREATED_POOL` is not decoration: the decoding rules hand
/// the consumer the `MigrationProgress` discriminant itself, so the Product --
/// not the venue table -- is what decides which outcome that discriminant
/// selects, and a Product carving the same observable differently needs no
/// adapter change at all.
struct ProductGraph {
    product: RecordPair,
    product_record_digest: [u8; 32],
    result_domain: RecordPair,
    portfolio: RecordPair,
    coordinate_domain_id: [u8; 32],
    result_unit_id: [u8; 32],
}

fn product_graph(test: &mut ProgramTest) -> ProductGraph {
    let product_id = ProductContentId::new([0x60; 32]).expect("Product identity");
    let coordinate_domain_id = [0x61; 32];
    let result_unit_id = [0x62; 32];
    let liability_basis_id = ProductContentId::new([0x63; 32]).expect("liability basis");
    let representation_release_id =
        ProductContentId::new([0x64; 32]).expect("representation release");

    let cuts: [i128; 0] = [];
    let mut domain_bytes = vec![0; result_domain_record_bytes(cuts.len()).expect("domain width")];
    compile_result_domain_v2(
        ResultDomainInputV2 {
            product_id,
            coordinate_domain_id: ProductContentId::new(coordinate_domain_id)
                .expect("coordinate domain"),
            result_unit_id: ProductContentId::new(result_unit_id).expect("result unit"),
            liability_basis_id,
            representation_release_id,
            mapping_release_id: ProductContentId::new([0x65; 32]).expect("mapping release"),
            cut_denominator: 1,
            cuts: &cuts,
        },
        &mut domain_bytes,
    )
    .expect("canonical graduation result domain");
    let (result_domain, domain_digest) = add_record(test, RESULT_DOMAIN_SCHEMA_ID_V2, domain_bytes);

    let coefficients = [1_u64; GRADUATION_OUTCOME_COUNT as usize];
    let mut portfolio_bytes =
        vec![0; portfolio_record_bytes(coefficients.len()).expect("portfolio width")];
    compile_portfolio_v2(
        PortfolioInputV2 {
            product_id,
            result_domain_id: ProductContentId::new(domain_digest).expect("domain digest"),
            claim_basis_id: ProductContentId::new([0x66; 32]).expect("claim basis"),
            liability_basis_id,
            representation_release_id,
            denominator: 1,
            coefficients: &coefficients,
        },
        &mut portfolio_bytes,
    )
    .expect("canonical graduation portfolio");
    let (portfolio, portfolio_digest) = add_record(test, PORTFOLIO_SCHEMA_ID_V2, portfolio_bytes);

    let mut product_bytes = vec![0; PRODUCT_RECORD_BYTES_V2];
    ProductRecordV2::new(
        product_id,
        ProductContentId::new(domain_digest).expect("domain digest"),
        ProductContentId::new(portfolio_digest).expect("portfolio digest"),
    )
    .encode_into(&mut product_bytes)
    .expect("canonical Product graph root");
    let (product, product_record_digest) =
        add_record(test, PRODUCT_RECORD_SCHEMA_ID_V2, product_bytes);

    ProductGraph {
        product,
        product_record_digest,
        result_domain,
        portfolio,
        coordinate_domain_id,
        result_unit_id,
    }
}

/// The venue's founding-time pinned deployment: P-B, as a record.
///
/// A market on a third-party venue is a market that can terminate in "the venue
/// changed", and this is the record that makes that true rather than a wish.
fn venue_release(deployment_slot: u64, elf_digest: [u8; 32]) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        ProgramIdentityV1::new(DBC_PROGRAM).expect("venue program"),
        ProgramIdentityV1::new(LOADER_V3_PROGRAM_ID).expect("loader"),
        DBC_PROGRAMDATA,
        ContentId::new([0x67; 32]).expect("venue semantic release"),
        elf_digest,
        deployment_slot,
        ArtifactUpgradePolicyV1::ExactAuthority,
        Some(UPGRADE_AUTHORITY),
    )
    .expect("pinned venue artifact release")
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
    product: &ProductGraph,
    venue_release_digest: [u8; 32],
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

    // The Source and the Product have to be about the same thing, and the
    // consumer checks it, so the spec's coordinate domain and unit are the
    // Product domain's own rather than two more fixture constants.
    let unit = source_id(product.result_unit_id);
    let spec_value = SourceSpecV1::new(
        source_id(product.coordinate_domain_id),
        unit,
        source_id(provider_digest),
        SourceAccessProfile::RelayedObservationRecord,
        // The V1 material carried an inline Pyth-typed adapter-config slot and
        // the V2 material does not, so for this family the slot names the
        // *venue's* pinned deployment instead: which third-party program a
        // market is about is a founding-time content identity, not an argument.
        source_id(venue_release_digest),
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
        WINDOW_START_UNIX,
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
        source_id(product.product_record_digest),
        source_id(spec_digest),
        source_id(window_digest),
        source_id(statistic_digest),
        None,
        source_id(SOURCE_FAILURE_POLICY_RELEASE_ID_V2),
    );
    // The graph the material claims is the graph the records make. Asserting it
    // here turns a `SourceMaterial` refusal from the bank into a named fixture failure.
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
    product: ProductGraph,
    venue: RecordPair,
    source_state: Pubkey,
    certificate: Pubkey,
    capability_manifest: RecordPair,
    failure_funding: Pubkey,
    exhaustion_funding: Pubkey,
}

fn fixture(seal_threshold: u8, extra_keys: &[[u8; 32]]) -> Fixture {
    fixture_with_venue(seal_threshold, extra_keys, DEPLOYMENT_SLOT, ELF_DIGEST)
}

/// Build the whole world, with the venue's pinned deployment as a parameter.
///
/// A pinned release that disagrees with the attested bodies is the executable
/// form of "the venue was upgraded mid-market", which is why it is a knob here
/// rather than a constant.
fn fixture_with_venue(
    seal_threshold: u8,
    extra_keys: &[[u8; 32]],
    pinned_deployment_slot: u64,
    pinned_elf_digest: [u8; 32],
) -> Fixture {
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

    let product = product_graph(&mut test);
    let (venue, venue_digest) = add_record(
        &mut test,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        venue_release(pinned_deployment_slot, pinned_elf_digest)
            .to_bytes()
            .to_vec(),
    );
    let graph = source_graph(
        &mut test,
        key_set_digest,
        config_digest,
        &product,
        venue_digest,
    );

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

    // The capability manifest, and the reason the deadline walk has a bounty at
    // all. Three `RESOLUTION_CONTROLLER_RELEASE_ID_V4` entries in the order
    // `core_effect`'s `authenticate_funding_entries` fixes them -- recovery
    // allocation, recovery policy, then THIS MARKET'S OWN Source material -- so
    // the explicit-failure compartment is identified by what its manifest entry
    // configures rather than by an account position. Only the third entry's
    // config is a real identity here; the first two exist so the failure
    // compartment is not entry zero and a walk that read the wrong index would
    // be visible rather than accidentally correct.
    let mut entries = [
        funding_entry([0xa1; 32]),
        funding_entry([0xa2; 32]),
        funding_entry(graph.material_id),
    ];
    // A manifest is strictly ordered by capability-kind identity, so the entry
    // *index* of the explicit-failure compartment is whatever the sort makes it
    // rather than a number a fixture may choose. Both indices below are
    // discovered by asking which entry configures which identity, which is the
    // same question the route asks.
    entries.sort_unstable_by_key(|entry| entry.kind_id().to_bytes());
    let mut manifest_bytes = vec![0; MANIFEST_HEADER_BYTES + 3 * CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&entries, &mut manifest_bytes).expect("capability manifest");
    let manifest = CapabilityManifestV1::decode(&manifest_bytes).expect("manifest view");
    let entry_index_of = |config: [u8; 32]| {
        (0..manifest.entry_count())
            .find(|index| {
                manifest
                    .entry(*index)
                    .expect("manifest entry")
                    .config_id()
                    .to_bytes()
                    == config
            })
            .expect("the manifest configures this identity")
    };
    let failure_entry_index = entry_index_of(graph.material_id);
    let exhaustion_entry_index = entry_index_of([0xa2; 32]);
    let (capability_manifest, manifest_digest) = add_record(
        &mut test,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        manifest_bytes.clone(),
    );

    let mut identity = MarketIdentity {
        market_id: CoreIdentity::new([0xff; 32]).expect("placeholder Market"),
        realm_id: CoreIdentity::new([31; 32]).expect("Realm"),
        product_record: CoreIdentity::new(product.product_record_digest).expect("Product record"),
        product_id: CoreIdentity::new([33; 32]).expect("Product"),
        resolution_policy: CoreIdentity::new(graph.material_id).expect("Source material"),
        capability_manifest: CoreIdentity::new(manifest_digest).expect("manifest"),
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
        protocol_account(
            CORE_PROGRAM_ID,
            state.encode().expect("Core state").to_vec(),
        ),
    );

    // Two live compartments, both correctly owned, both at their own derived
    // addresses, both Active. Only one of them is this walk's: the third entry
    // configures this market's own Source material. The second is the
    // exhaustion compartment, and it is here so that "present a different
    // compartment" is a case with a REAL account behind it rather than a
    // fabricated one.
    let failure_funding = add_active_funding(&mut test, market, manifest, failure_entry_index);
    let exhaustion_funding =
        add_active_funding(&mut test, market, manifest, exhaustion_entry_index);
    assert_ne!(failure_funding, exhaustion_funding);

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

    // The Source resolution state is Resolution-owned and already Primary. Its
    // creation is the Core-effect route's business, not this family's; what the
    // consumer needs is that one exists, at its own derived address, bound to
    // this Market, generation and material.
    let (source_state, source_bump) = Pubkey::find_program_address(
        &[
            dclutch_source_contract::SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
            market.as_ref(),
            &GENERATION.to_le_bytes(),
        ],
        &PROGRAM_ID,
    );
    let fresh = SourceResolutionStateV2::fresh(
        market.to_bytes(),
        GENERATION,
        source_id(graph.material_id),
        rent_beneficiary.to_bytes(),
        source_bump,
        0,
        0,
    )
    .expect("fresh primary Source state")
    .state();
    test.add_account(
        source_state,
        protocol_account(PROGRAM_ID, fresh.to_bytes().to_vec()),
    );

    // The certificate address is the Resolution role's existing namespace, keyed
    // by the terminal's own wire tag and the sequence. Success and failure are
    // different addresses, so a market that resolved cannot have its certificate
    // overwritten by a later failure walk, or the reverse.
    let mut certificate = Pubkey::default();
    for kind in [RESOLUTION_SUCCESS_KIND, RESOLUTION_FAILURE_KIND] {
        let account_key = Pubkey::find_program_address(
            &[
                RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
                source_state.as_ref(),
                &[kind],
                &TERMINAL_SEQUENCE.to_le_bytes(),
            ],
            &PROGRAM_ID,
        )
        .0;
        test.add_account(
            account_key,
            Account::new(
                Rent::default().minimum_balance(RESOLUTION_CERTIFICATE_BYTES_V2),
                0,
                &system_program::ID,
            ),
        );
        if kind == RESOLUTION_SUCCESS_KIND {
            certificate = account_key;
        }
    }

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
        product,
        venue,
        source_state,
        certificate,
        capability_manifest,
        failure_funding,
        exhaustion_funding,
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
                AccountMeta::new_readonly(
                    substitution.activation.unwrap_or(self.activation),
                    false,
                ),
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
                    substitution
                        .rent_beneficiary
                        .unwrap_or(self.rent_beneficiary),
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

    /// The pinned account set, in the wire form the consume instruction carries.
    ///
    /// Byte-identical to the entries' contribution to the `account_set_id`
    /// preimage, so a caller cannot produce a different identity by writing the
    /// same field twice.
    fn entry_bytes(&self, substitution: ConsumeSubstitution) -> Vec<u8> {
        let (mut entries, _) = account_set();
        if let Some(width) = substitution.venue_inline_len {
            entries[2].inline_len = width;
        }
        if let Some(owner) = substitution.venue_owner {
            entries[2].expected_owner = owner;
        }
        let mut bytes = Vec::new();
        for entry in entries {
            bytes.extend_from_slice(&entry.key);
            bytes.extend_from_slice(&entry.expected_owner);
            bytes.extend_from_slice(&entry.inline_len.to_le_bytes());
        }
        bytes
    }

    fn consume_instruction(&self, substitution: ConsumeSubstitution) -> Instruction {
        let mut data = ConsumeRecordInstructionV1::new(
            GENERATION,
            substitution.observed_slot.unwrap_or(OBSERVED_SLOT),
            TERMINAL_SEQUENCE,
            self.graph.material_id,
            self.graph.spec_id,
            4,
        )
        .expect("consume request")
        .to_prefix_bytes()
        .expect("consume prefix")
        .to_vec();
        assert_eq!(data.len(), CONSUME_RECORD_PREFIX_BYTES);
        data.extend_from_slice(&self.entry_bytes(substitution));
        Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(self.worker.pubkey(), true),
                AccountMeta::new_readonly(self.market, false),
                AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
                AccountMeta::new_readonly(self.activation, false),
                AccountMeta::new(substitution.record.unwrap_or(self.record), false),
                AccountMeta::new(self.source_state, false),
                AccountMeta::new(self.certificate, false),
                AccountMeta::new_readonly(self.graph.material.raw, false),
                AccountMeta::new_readonly(self.graph.material.staging, false),
                AccountMeta::new_readonly(self.graph.spec.raw, false),
                AccountMeta::new_readonly(self.graph.spec.staging, false),
                AccountMeta::new_readonly(self.graph.provider.raw, false),
                AccountMeta::new_readonly(self.graph.provider.staging, false),
                AccountMeta::new_readonly(self.graph.window.raw, false),
                AccountMeta::new_readonly(self.graph.window.staging, false),
                AccountMeta::new_readonly(substitution.config.unwrap_or(self.config.raw), false),
                AccountMeta::new_readonly(self.config.staging, false),
                AccountMeta::new_readonly(self.venue.raw, false),
                AccountMeta::new_readonly(self.venue.staging, false),
                AccountMeta::new_readonly(self.product.product.raw, false),
                AccountMeta::new_readonly(self.product.product.staging, false),
                AccountMeta::new_readonly(
                    substitution
                        .result_domain
                        .unwrap_or(self.product.result_domain.raw),
                    false,
                ),
                AccountMeta::new_readonly(self.product.result_domain.staging, false),
                AccountMeta::new_readonly(self.product.portfolio.raw, false),
                AccountMeta::new_readonly(self.product.portfolio.staging, false),
                AccountMeta::new_readonly(sysvar::clock::ID, false),
                AccountMeta::new_readonly(sysvar::rent::ID, false),
                AccountMeta::new_readonly(system_program::ID, false),
            ],
            data,
        }
    }

    /// The certificate address for a terminal of one kind.
    ///
    /// Success and failure are different *addresses* for one Source state at one
    /// sequence, so neither can overwrite the other.
    fn certificate_of(&self, kind: u8) -> Pubkey {
        Pubkey::find_program_address(
            &[
                RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
                self.source_state.as_ref(),
                &[kind],
                &TERMINAL_SEQUENCE.to_le_bytes(),
            ],
            &PROGRAM_ID,
        )
        .0
    }

    fn deadline_failure_instruction(&self, substitution: DeadlineSubstitution) -> Instruction {
        Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(self.worker.pubkey(), true),
                AccountMeta::new_readonly(self.market, false),
                AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
                AccountMeta::new_readonly(self.activation, false),
                AccountMeta::new(self.source_state, false),
                AccountMeta::new(self.certificate_of(RESOLUTION_FAILURE_KIND), false),
                AccountMeta::new_readonly(self.graph.material.raw, false),
                AccountMeta::new_readonly(self.graph.material.staging, false),
                AccountMeta::new_readonly(self.graph.window.raw, false),
                AccountMeta::new_readonly(self.graph.window.staging, false),
                AccountMeta::new_readonly(self.product.product.raw, false),
                AccountMeta::new_readonly(self.product.product.staging, false),
                AccountMeta::new_readonly(self.product.result_domain.raw, false),
                AccountMeta::new_readonly(self.product.result_domain.staging, false),
                AccountMeta::new_readonly(self.product.portfolio.raw, false),
                AccountMeta::new_readonly(self.product.portfolio.staging, false),
                AccountMeta::new_readonly(self.capability_manifest.raw, false),
                AccountMeta::new_readonly(self.capability_manifest.staging, false),
                AccountMeta::new(substitution.funding.unwrap_or(self.failure_funding), false),
                AccountMeta::new_readonly(sysvar::clock::ID, false),
                AccountMeta::new_readonly(sysvar::rent::ID, false),
                AccountMeta::new_readonly(system_program::ID, false),
            ],
            data: CommitDeadlineFailureInstructionV1::new(
                GENERATION,
                substitution.terminal_sequence.unwrap_or(TERMINAL_SEQUENCE),
            )
            .expect("deadline failure request")
            .to_bytes()
            .expect("deadline failure bytes")
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

/// One substitution a hostile deadline walk makes, and nothing else.
///
/// The walk's instruction carries a generation and a terminal sequence and
/// nothing else, so there is very little for a caller to lie about — which is
/// the property, not an omission. What remains is the one account whose
/// identity is not fixed by the Market's own bytes: which escrow gets debited.
#[derive(Clone, Copy, Default)]
struct DeadlineSubstitution {
    funding: Option<Pubkey>,
    terminal_sequence: Option<u64>,
}

/// One substitution a hostile consumption makes, and nothing else.
///
/// Every field names a fact the consumer must take from the authenticated
/// Market, the content-addressed Source graph, or the certified record, rather
/// than from whoever is calling.
#[derive(Clone, Copy, Default)]
struct ConsumeSubstitution {
    record: Option<Pubkey>,
    config: Option<Pubkey>,
    result_domain: Option<Pubkey>,
    observed_slot: Option<u64>,
    venue_inline_len: Option<u16>,
    venue_owner: Option<[u8; 32]>,
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

/// The extent of a signed legacy transaction on the wire.
///
/// It MEASURES and does not judge, and deliberately does not carry a copy of
/// Solana's 1,232-byte `PACKET_DATA_BYTES` to compare against.
/// `solana-program-test` submits no packet and cannot enforce that maximum
/// itself -- Found31 was ten bytes over and survived every fixture test in the
/// tree -- so the number has to be checked somewhere a campaign cannot quietly
/// satisfy. That place is the tier's own witness
/// (`tools/gauntlet/resolution-relayed/witnesses.json`), which reads the
/// recorded extents back and compares them to the limit without asking this
/// file's opinion. Two of this campaign's transactions are over it.
fn wire_extent(signatures: usize, message: &[u8]) -> usize {
    1 + signatures * 64 + message.len()
}

/// Submit, and record the runtime's own account of what happened.
///
/// Identical to [`submit`] except that it names the transaction for the census
/// and asks the bank for metadata instead of a bare result. The evidence is
/// emitted BEFORE the caller gets to assert anything, so a case that fails its
/// own assertion still leaves behind what the chain did.
///
/// Only the cases the census binds go through here. The hostile corpora and the
/// internal plumbing keep [`submit`], because a campaign that labelled every
/// transaction it happens to send would be claiming coverage it has not written
/// a binding for.
async fn submit_recorded(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    signers: &[&Keypair],
    label: &str,
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
    let signature = transaction
        .signatures
        .first()
        .ok_or(BanksClientError::ClientError("unsigned transaction"))?
        .to_string();
    let extent = wire_extent(
        transaction.signatures.len(),
        &transaction.message.serialize(),
    );
    let slot = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .map_or(0, |clock| clock.slot);
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await?;
    let outcome = processed.result.clone();
    let failure = outcome.clone().err().map(|error| format!("{error:?}"));
    let (logs, units) = processed
        .metadata
        .map(|metadata| (metadata.log_messages, metadata.compute_units_consumed))
        .unwrap_or_default();
    dclutch_program_test_evidence::record(&TransactionEvidence {
        label,
        signature: &signature,
        slot,
        error: failure.as_deref(),
        logs: &logs,
        compute_units_consumed: Some(units),
        wire_bytes: Some(extent),
    })
    .expect("campaign evidence must be writable when the gauntlet asked for it");
    outcome.map_err(BanksClientError::TransactionError)
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

    submit_recorded(
        &mut context,
        &[fixture.create_instruction(4, 1)],
        &[&fixture.worker],
        "relayed transport: create the observation record",
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
        submit_recorded(
            &mut context,
            &[precompile, append],
            &[&fixture.worker],
            &format!("relayed transport: append observation {index}"),
        )
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
    submit_recorded(
        &mut context,
        &[precompile, seal_ix],
        &[&fixture.worker],
        "relayed transport: seal the completed set",
    )
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
    submit_recorded(
        &mut context,
        &[fixture.retire_instruction()],
        &[&fixture.worker],
        "relayed transport: retire the record into the Market beneficiary",
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
///
/// These used to be the numbers 3, 5, 7 and 12, typed out here. They were a
/// second opinion about a taxonomy that already had an owner, and a renumber
/// would have left them silently naming the wrong guard rather than failing.
/// They come from the adapter now.
const REFUSAL_MARKET_AUTHORITY: u32 = ResolutionError::MarketAuthority as u32;
const REFUSAL_RESOLUTION_RELEASE: u32 = ResolutionError::ResolutionRelease as u32;
const REFUSAL_SOURCE_MATERIAL: u32 = ResolutionError::SourceMaterial as u32;
const REFUSAL_TRANSITION: u32 = ResolutionError::Transition as u32;

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
        REFUSAL_TRANSITION_LIVENESS,
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

// ---------------------------------------------------------------------------
// The consumer: one sealed record into one terminal result.
//
// Everything above this line moved bytes nobody had read. What follows is the
// route that reads them, and the same honesty label applies with one addition:
// the venue *layout* is real and verified from the published Meteora source,
// the venue *values* are invented, and "the bank accepted an attestation
// asserting a graduated mainnet pool" is the strongest true sentence about the
// green case. Nothing here observed mainnet.
// ---------------------------------------------------------------------------

// The consumer half of the same taxonomy, and derived for the same reason.
// `REFUSAL_TRANSITION_LIVENESS` is deliberately the same variant as
// `REFUSAL_TRANSITION` above under a name that says what the consumer route is
// asking of it; that the two are one code is a fact about the adapter, and it
// stays true here by construction rather than by two people typing 12.
const REFUSAL_ACCOUNT_FRAME: u32 = ResolutionError::AccountFrame as u32;
const REFUSAL_FINALIZED_RECORD: u32 = ResolutionError::FinalizedRecord as u32;
const REFUSAL_PRODUCT_DOMAIN: u32 = ResolutionError::ProductDomain as u32;
const REFUSAL_PROVIDER_OBSERVATION: u32 = ResolutionError::ProviderObservation as u32;
const REFUSAL_OUTPUT_STATE: u32 = ResolutionError::OutputState as u32;
const REFUSAL_RELAYED_RECORD: u32 = ResolutionError::RelayedRecord as u32;
const REFUSAL_RELAYED_WINDOW: u32 = ResolutionError::RelayedWindow as u32;
const REFUSAL_TRANSITION_LIVENESS: u32 = ResolutionError::Transition as u32;
const REFUSAL_FUNDING: u32 = ResolutionError::Funding as u32;

/// Pin the devnet clock so both time bounds are exact.
///
/// Without this the bank carries whatever wall clock it was started with, and
/// the two-clock staleness join would be measuring the distance between a
/// fixture constant and the day the suite happened to run.
async fn pin_devnet_clock(context: &mut ProgramTestContext) {
    let mut clock: Clock = context
        .banks_client
        .get_sysvar()
        .await
        .expect("clock sysvar");
    clock.unix_timestamp = DEVNET_NOW;
    clock.epoch_start_timestamp = DEVNET_NOW;
    context.set_sysvar(&clock);
}

/// Drive one fixture's record from nothing to sealed, with the bodies it holds.
///
/// The bodies are read out of the fixture rather than rebuilt, so a test that
/// wants a pre-terminal pool or a stale foreign clock edits `positions` and gets
/// a record that is honestly sealed over exactly those bytes -- a real quorum
/// standing behind a real statement, which is the only interesting kind of
/// hostile input for a consumer.
async fn seal_record(context: &mut ProgramTestContext, fixture: &Fixture) {
    submit(
        context,
        &[fixture.create_instruction(4, 1)],
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
        submit(context, &[precompile, append], &[&fixture.worker])
            .await
            .unwrap_or_else(|error| panic!("append {index} failed: {error:?}"));
        running = fold(running, body_slice(&message));
    }

    let seal = fixture.seal_message(running);
    let seal_ix = fixture.seal_instruction(&seal);
    let precompile = ed25519_instruction(
        &fixture.relayer,
        &seal,
        u16::try_from(SEAL_RECORD_PREFIX_BYTES).expect("offset"),
        1,
    );
    submit(context, &[precompile, seal_ix], &[&fixture.worker])
        .await
        .expect("seal the completed set");
}

async fn start(fixture: &mut Fixture) -> ProgramTestContext {
    let mut context = fixture
        .test
        .take()
        .expect("test")
        .start_with_context()
        .await;
    pin_devnet_clock(&mut context).await;
    context
}

#[tokio::test]
async fn a_sealed_graduation_resolves_the_market_through_the_products_own_domain() {
    let mut fixture = fixture(1, &[]);
    let mut context = start(&mut fixture).await;
    seal_record(&mut context, &fixture).await;

    submit_recorded(
        &mut context,
        &[fixture.consume_instruction(ConsumeSubstitution::default())],
        &[&fixture.worker],
        "relayed consumption: a sealed graduation resolves the market",
    )
    .await
    .expect("consume the sealed graduation record");

    // The record is spent. One signed observation resolves at most one market
    // state, and this is where that stops being a design intention.
    let data = record_bytes(&mut context, fixture.record).await;
    let view = RelayedObservationRecordViewV1::decode(&data).expect("record decodes");
    assert_eq!(view.phase(), Ok(RelayedRecordPhaseV1::Consumed));

    // The Source is terminal, on the primary route, and carries the evidence
    // identity rather than a caller-supplied one.
    let source_data = record_bytes(&mut context, fixture.source_state).await;
    let source = SourceResolutionStateV2::decode(&source_data).expect("Source state decodes");
    assert_eq!(source.phase(), SourceResolutionPhaseV1::Resolved);
    let projection = source.terminal_projection().expect("terminal projection");
    assert_eq!(projection.terminal_sequence(), TERMINAL_SEQUENCE);

    let certificate_data = record_bytes(&mut context, fixture.certificate).await;
    let certificate =
        ResolutionCertificateV2::decode(&certificate_data).expect("certificate decodes");
    assert_eq!(
        certificate.kind,
        ResolutionCertificateKindV2::ResolutionSuccess
    );
    assert_eq!(certificate.market, fixture.market.to_bytes());
    assert_eq!(certificate.generation, GENERATION);
    // The atom is the venue's own discriminant, carried whole. The Product's
    // single cut at `CreatedPool` is what turned it into an outcome, which is
    // why a Product that carved the same observable differently would need no
    // adapter change.
    assert_eq!(
        certificate.result_numerator,
        i128::from(MIGRATION_PROGRESS_CREATED_POOL)
    );
    assert_eq!(certificate.result_denominator, 1);
    assert_eq!(
        certificate.selector, 0,
        "the graduated cell is the Product's only ordinary one"
    );
    assert!(
        certificate.selector < GRADUATION_REGION_COUNT,
        "an observation route may never reach the failure selector"
    );
    assert_eq!(certificate.observed_at, CREATED_UNIX as u64);
    assert_eq!(
        certificate.provider_evidence,
        projection.resolution_evidence_id().to_bytes(),
        "the certificate and the Source must name one evidence identity"
    );
    assert_ne!(certificate.provider_evidence, [0; 32]);

    // Consuming twice is refused by the record's own phase, not by a lucky
    // ordering: the second attempt is a fresh transaction against a spent
    // record.
    refused_with(
        submit(
            &mut context,
            &[fixture.consume_instruction(ConsumeSubstitution::default())],
            &[&fixture.worker],
        )
        .await,
        REFUSAL_RELAYED_RECORD,
    );
}

#[tokio::test]
async fn an_unsealed_record_cannot_be_consumed() {
    let mut fixture = fixture(1, &[]);
    let mut context = start(&mut fixture).await;

    submit(
        &mut context,
        &[fixture.create_instruction(4, 1)],
        &[&fixture.worker],
    )
    .await
    .expect("create the observation record");

    // Empty, then partially filled: a record with no quorum behind it is not
    // evidence at any fill level, and neither is one the quorum has not signed.
    refused_with(
        submit(
            &mut context,
            &[fixture.consume_instruction(ConsumeSubstitution::default())],
            &[&fixture.worker],
        )
        .await,
        REFUSAL_RELAYED_RECORD,
    );

    let message = fixture.attestation(0, SOLANA_MAINNET_GENESIS_HASH_V1, OBSERVED_SLOT);
    let precompile = ed25519_instruction(
        &fixture.relayer,
        &message,
        u16::try_from(APPEND_OBSERVATION_PREFIX_BYTES).expect("offset"),
        1,
    );
    submit(
        &mut context,
        &[precompile, fixture.append_instruction(&message)],
        &[&fixture.worker],
    )
    .await
    .expect("append the first position");
    refused_with(
        submit(
            &mut context,
            &[fixture.consume_instruction(ConsumeSubstitution::default())],
            &[&fixture.worker],
        )
        .await,
        REFUSAL_RELAYED_RECORD,
    );
}

#[tokio::test]
async fn a_pre_terminal_pool_says_the_window_is_unsatisfied_and_leaves_the_market_live() {
    // The load-bearing refusal of the whole family. A terminal-window graduation
    // proposition can only ever be *proved* by graduation, so a pool that has
    // not migrated is no answer rather than a negative one -- and the honest
    // response is to leave the market open, not to resolve it early off a state
    // that is still moving.
    for progress in [0_u8, 1, 2] {
        let mut fixture = fixture(1, &[]);
        let mut pool = virtual_pool_body();
        pool[MIGRATION_PROGRESS_OFFSET] = progress;
        pool[IS_MIGRATED_OFFSET] = 0;
        fixture.positions[2].body = pool;

        let mut context = start(&mut fixture).await;
        seal_record(&mut context, &fixture).await;
        refused_with(
            submit(
                &mut context,
                &[fixture.consume_instruction(ConsumeSubstitution::default())],
                &[&fixture.worker],
            )
            .await,
            REFUSAL_RELAYED_WINDOW,
        );

        // Nothing moved. The record is still sealed and still consumable by a
        // later, honest observation of the same market.
        let data = record_bytes(&mut context, fixture.record).await;
        let view = RelayedObservationRecordViewV1::decode(&data).expect("record decodes");
        assert_eq!(view.phase(), Ok(RelayedRecordPhaseV1::Sealed));
        let source_data = record_bytes(&mut context, fixture.source_state).await;
        let source = SourceResolutionStateV2::decode(&source_data).expect("Source decodes");
        assert_eq!(source.phase(), SourceResolutionPhaseV1::Primary);
    }
}

#[tokio::test]
async fn a_venue_upgraded_mid_market_refuses_every_later_observation() {
    // P-B, executed end to end. The Source spec pinned one deployment at
    // founding; the attested `ProgramData` reports another. Nothing about the
    // relayer, the quorum or the pool changed, and the market still cannot
    // resolve -- which is precisely the cost the Product has to disclose.
    let mut fixture = fixture_with_venue(1, &[], DEPLOYMENT_SLOT + 1, [0xef; 32]);
    let mut context = start(&mut fixture).await;
    seal_record(&mut context, &fixture).await;
    refused_with(
        submit(
            &mut context,
            &[fixture.consume_instruction(ConsumeSubstitution::default())],
            &[&fixture.worker],
        )
        .await,
        REFUSAL_PROVIDER_OBSERVATION,
    );
}

#[tokio::test]
async fn the_consumption_hostile_corpus_is_refused_by_the_real_adapter() {
    let mut fixture = fixture(1, &[]);
    let decoy_config = RelayedAdapterConfigV1::new(
        [0x7a; 32],
        0,
        0,
        u64::from(WINDOW_MAX_AGE_SECONDS),
        CLUSTER_SKEW_SECONDS,
    )
    .expect("a second, differently pinned adapter configuration");
    let decoy = {
        let test = fixture.test.as_mut().expect("fixture test");
        let (pair, _) = add_record(
            test,
            RELAYED_ADAPTER_CONFIG_SCHEMA_RELEASE_ID_V1,
            decoy_config.to_bytes().expect("decoy bytes").to_vec(),
        );
        pair
    };
    let mut context = start(&mut fixture).await;
    seal_record(&mut context, &fixture).await;

    // A substituted record: the instruction names a slot the presented record
    // was never addressed by. The record PDA is a function of the observed slot,
    // so this is the equivocation bound refusing.
    refused_with(
        submit(
            &mut context,
            &[fixture.consume_instruction(ConsumeSubstitution {
                observed_slot: Some(OBSERVED_SLOT + 1),
                ..ConsumeSubstitution::default()
            })],
            &[&fixture.worker],
        )
        .await,
        REFUSAL_OUTPUT_STATE,
    );

    // An account that is not a record at all in the record position. The key set
    // is a real, canonical, Registry-owned record of another schema that this
    // frame does not otherwise name, so the refusal is the record position's own
    // custody check rather than an alias.
    refused_with(
        submit(
            &mut context,
            &[fixture.consume_instruction(ConsumeSubstitution {
                record: Some(fixture.key_set.raw),
                ..ConsumeSubstitution::default()
            })],
            &[&fixture.worker],
        )
        .await,
        REFUSAL_OUTPUT_STATE,
    );

    // And presenting an account the frame already names somewhere else is
    // refused before any of that, by the no-alias policy: without it a caller
    // could put the adapter configuration in the record position and have two
    // positions read one account.
    refused_with(
        submit(
            &mut context,
            &[fixture.consume_instruction(ConsumeSubstitution {
                record: Some(fixture.config.raw),
                ..ConsumeSubstitution::default()
            })],
            &[&fixture.worker],
        )
        .await,
        REFUSAL_ACCOUNT_FRAME,
    );

    // Wrong decoding rules: a real, canonical, Registry-owned adapter
    // configuration that the provider release does not name. The refusal is the
    // content-address link, which is the only thing that could catch it.
    refused_with(
        submit(
            &mut context,
            &[fixture.consume_instruction(ConsumeSubstitution {
                config: Some(decoy.raw),
                ..ConsumeSubstitution::default()
            })],
            &[&fixture.worker],
        )
        .await,
        REFUSAL_FINALIZED_RECORD,
    );

    // A substituted result domain: the Product record names its own domain by
    // digest, so a caller cannot choose the partition their resolution maps
    // through. The substitute is another canonical Registry-owned record the
    // frame does not otherwise name, so this is the digest link refusing and not
    // the alias policy.
    refused_with(
        submit(
            &mut context,
            &[fixture.consume_instruction(ConsumeSubstitution {
                result_domain: Some(decoy.raw),
                ..ConsumeSubstitution::default()
            })],
            &[&fixture.worker],
        )
        .await,
        REFUSAL_PRODUCT_DOMAIN,
    );

    // Tampered account-set entries. The entries arrive as caller input, so both
    // of these are the digest refusing rather than a field comparison: a
    // narrower inline window for the venue position, and the Loopscale
    // substitution of its owning program.
    refused_with(
        submit(
            &mut context,
            &[fixture.consume_instruction(ConsumeSubstitution {
                venue_inline_len: Some(352),
                ..ConsumeSubstitution::default()
            })],
            &[&fixture.worker],
        )
        .await,
        REFUSAL_PROVIDER_OBSERVATION,
    );
    refused_with(
        submit(
            &mut context,
            &[fixture.consume_instruction(ConsumeSubstitution {
                venue_owner: Some([0x99; 32]),
                ..ConsumeSubstitution::default()
            })],
            &[&fixture.worker],
        )
        .await,
        REFUSAL_PROVIDER_OBSERVATION,
    );

    // And the honest case still passes afterwards, so every refusal above was
    // the substitution and not a broken fixture.
    submit(
        &mut context,
        &[fixture.consume_instruction(ConsumeSubstitution::default())],
        &[&fixture.worker],
    )
    .await
    .expect("the honest consumption after the corpus");
}

/// One named corruption of an otherwise canonical `VirtualPool` body.
///
/// Named rather than written inline because it is a hostile-corpus row, and a
/// corpus row wants to read as one thing: a sentence saying what the relayer
/// lied about, and the edit that tells the lie.
type SignedPoolCorruption<'a> = (&'a str, Box<dyn Fn(&mut Vec<u8>)>);

#[tokio::test]
async fn a_signed_but_incoherent_or_foreign_pool_body_refuses_on_its_own_field() {
    // Every case here is a *real* quorum standing behind a *real* signed
    // statement. That is what makes them worth executing: the relayer is
    // trusted with the reading and can lie about it, and these are the lies the
    // decoding rules catch on this cluster rather than on the relayer's word.
    let cases: [SignedPoolCorruption<'_>; 3] = [
        (
            "a pool claiming migration with `is_migrated` still zero",
            Box::new(|pool: &mut Vec<u8>| {
                pool[IS_MIGRATED_OFFSET] = 0;
            }),
        ),
        (
            "a pool claiming migration with no finish timestamp",
            Box::new(|pool: &mut Vec<u8>| {
                pool[FINISH_CURVE_TIMESTAMP_OFFSET..FINISH_CURVE_TIMESTAMP_OFFSET + 8]
                    .copy_from_slice(&0_u64.to_le_bytes());
            }),
        ),
        (
            "a `TransferHookPool`, which shares the identical 424-byte body",
            Box::new(|pool: &mut Vec<u8>| {
                pool[..8].copy_from_slice(&[0xed, 0xdb, 0xb8, 0x17, 0x2a, 0xbd, 0xa9, 0x23]);
            }),
        ),
    ];
    for (name, mutate) in cases {
        let mut fixture = fixture(1, &[]);
        let mut pool = virtual_pool_body();
        mutate(&mut pool);
        fixture.positions[2].body = pool;
        let mut context = start(&mut fixture).await;
        seal_record(&mut context, &fixture).await;
        let result = submit(
            &mut context,
            &[fixture.consume_instruction(ConsumeSubstitution::default())],
            &[&fixture.worker],
        )
        .await;
        assert!(result.is_err(), "{name} was accepted");
        refused_with(result, REFUSAL_PROVIDER_OBSERVATION);
    }
}

#[tokio::test]
async fn a_foreign_clock_that_is_stale_or_from_another_slot_refuses() {
    // The two-clock rules, executed. The first case is a relayer that held a
    // signed message; the second is one that stitched a clock from a different
    // snapshot into a set addressed by this slot. Neither is a decode failure
    // and neither could be caught anywhere but here: filling only moves bytes
    // the signer committed to, so foreign time is a resolution-time question.
    for (name, slot, timestamp) in [
        (
            "a foreign clock older than the staleness bound",
            OBSERVED_SLOT,
            DEVNET_NOW - i64::from(WINDOW_MAX_AGE_SECONDS) - 1,
        ),
        (
            "a foreign clock from a different slot than the record's own",
            OBSERVED_SLOT - 1,
            CREATED_UNIX,
        ),
        (
            "a foreign clock further ahead than the admitted skew",
            OBSERVED_SLOT,
            DEVNET_NOW + i64::try_from(CLUSTER_SKEW_SECONDS).expect("skew") + 1,
        ),
    ] {
        let mut fixture = fixture(1, &[]);
        let mut clock = vec![0_u8; 40];
        clock[..8].copy_from_slice(&slot.to_le_bytes());
        clock[32..40].copy_from_slice(&timestamp.to_le_bytes());
        fixture.positions[3].body = clock;
        let mut context = start(&mut fixture).await;
        seal_record(&mut context, &fixture).await;
        let result = submit(
            &mut context,
            &[fixture.consume_instruction(ConsumeSubstitution::default())],
            &[&fixture.worker],
        )
        .await;
        assert!(result.is_err(), "{name} was accepted");
        refused_with(result, REFUSAL_PROVIDER_OBSERVATION);
    }
}

#[tokio::test]
async fn an_observation_outside_the_products_window_refuses_even_when_it_is_fresh() {
    // The two time-bound refusals, and the reason there are two. Both
    // observations below are perfectly fresh -- the relayer was prompt, the
    // clocks agree -- and both are about the wrong moment. A market resolved
    // by a fresh observation of the wrong week would be resolved by evidence
    // about something the Product never sold. `require_window_admits` names
    // why the edges differ: below `start` the market had not started selling
    // yet; above `end` the observation is *late*, the case a provider cadence
    // straddling the deadline produces.
    for (name, timestamp) in [
        ("before the window opened", WINDOW_START_UNIX - 1),
        ("after the window closed", CREATED_UNIX + 1),
    ] {
        let mut fixture = fixture(1, &[]);
        let mut clock = vec![0_u8; 40];
        clock[..8].copy_from_slice(&OBSERVED_SLOT.to_le_bytes());
        clock[32..40].copy_from_slice(&timestamp.to_le_bytes());
        fixture.positions[3].body = clock;
        let mut context = start(&mut fixture).await;
        seal_record(&mut context, &fixture).await;
        let result = submit(
            &mut context,
            &[fixture.consume_instruction(ConsumeSubstitution::default())],
            &[&fixture.worker],
        )
        .await;
        assert!(result.is_err(), "{name} was accepted");
        refused_with(result, REFUSAL_RELAYED_WINDOW);
    }
}

/// Move the pinned devnet clock past a market's primary deadline.
///
/// The deadline is the window's own instant plus its liveness grace, and the
/// comparison is strict, so this lands one second after the last moment an
/// honest resolution could have arrived.
async fn warp_past_the_deadline(context: &mut ProgramTestContext) {
    let mut clock: Clock = context
        .banks_client
        .get_sysvar()
        .await
        .expect("clock sysvar");
    clock.unix_timestamp = CREATED_UNIX + i64::from(WINDOW_MAX_AGE_SECONDS) + 1;
    context.set_sysvar(&clock);
}

#[tokio::test]
async fn no_late_observation_can_resolve_a_market_past_its_primary_deadline() {
    // One half of the liveness argument; the other half is
    // `a_silent_relayer_cannot_make_the_market_unresolvable`.
    //
    // Past the primary deadline the observation route refuses, permanently and
    // for every possible record: a relayer who goes quiet and comes back late
    // cannot resolve the market on its own schedule.  What that leaves is a
    // market pinned at `Primary`, and the walk is what takes it from there to
    // its pre-disclosed outcome.
    //
    // The refusal is the two-clock staleness bound rather than the window, and
    // that is worth naming because the two coincide *by construction* at exactly
    // this moment: the walk's deadline is `window.end + window.max_age`, and the
    // configuration's own `max_observation_age_seconds` is the same grace.  So
    // the last second an observation may resolve the market and the first second
    // the failure walk may take it are adjacent, with no gap in which neither
    // route can act.  The window bound is what refuses an observation that is
    // *fresh* and about the wrong moment, which is a different test.
    let mut fixture = fixture(1, &[]);
    let mut context = start(&mut fixture).await;
    seal_record(&mut context, &fixture).await;
    warp_past_the_deadline(&mut context).await;

    refused_with(
        submit(
            &mut context,
            &[fixture.consume_instruction(ConsumeSubstitution::default())],
            &[&fixture.worker],
        )
        .await,
        REFUSAL_PROVIDER_OBSERVATION,
    );
    let source_data = record_bytes(&mut context, fixture.source_state).await;
    let source = SourceResolutionStateV2::decode(&source_data).expect("Source decodes");
    assert_eq!(source.phase(), SourceResolutionPhaseV1::Primary);
}

#[test]
fn an_unfunded_failure_certificate_cannot_exist() {
    // Why the deadline walk has to debit an escrow, stated as an executed
    // refusal rather than a paragraph -- and why it could not have shipped as a
    // route that simply commits the failure and pays nobody.
    //
    // The Lean-owned terminal schema refuses a `ResolutionFailure` whose
    // `funding_allocation` is zero or whose `work_paid` is zero, which encodes
    // section 4.8's "bounded, prepaid, permissionless path that pays whoever
    // walks it" as a *decode-time* invariant.  There is no such thing as an
    // unfunded failure certificate, so there was no honest half-measure: the
    // route landed with the V1-to-V2 port of the funded controller that debits
    // a `FundingState` and credits the worker, and
    // `a_silent_relayer_cannot_make_the_market_unresolvable` executes it.
    //
    // This case stays because it is the constraint's own witness: it is the
    // thing that would fail if the schema were ever loosened to let a route mint
    // a bounty nobody paid.
    let unfunded = ResolutionCertificateV2 {
        kind: ResolutionCertificateKindV2::ResolutionFailure,
        market: [0x21; 32],
        route: [0; 32],
        source_material: [0x22; 32],
        product_record_digest: [0x23; 32],
        provider_evidence: [0; 32],
        funding_allocation: [0; 32],
        receipt_account: [0x24; 32],
        generation: GENERATION,
        attempt_index: 0,
        schedule_index: 0,
        selector: GRADUATION_OUTCOME_COUNT - 1,
        work_paid: 0,
        funding_remaining: 0,
        result_numerator: 0,
        result_denominator: 0,
        observed_at: 0,
    };
    assert!(
        unfunded
            .validate_terminal_product(unfunded.product_record_digest, GRADUATION_OUTCOME_COUNT)
            .is_ok(),
        "the kind and the selector agree; it is the funding that does not exist"
    );
    assert!(
        unfunded.to_bytes().is_err(),
        "an unfunded failure certificate encoded, and section 4.8's prepayment rule is not an invariant after all"
    );

    // With a bounty allocation and a credited worker the same certificate is
    // canonical, which is the shape the walk does produce -- and the committing
    // case above asserts exactly those two fields on the certificate the chain
    // wrote.
    let funded = ResolutionCertificateV2 {
        funding_allocation: [0x25; 32],
        work_paid: 100_000,
        ..unfunded
    };
    assert!(funded.to_bytes().is_ok());
}

/// The lamports one account holds right now.
async fn lamports_of(context: &mut ProgramTestContext, key: Pubkey) -> u64 {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("bank read")
        .map_or(0, |account| account.lamports)
}

/// Every fact the walk is supposed to move, read back off the chain at once.
///
/// A refusal case compares two of these and a committing case compares the
/// difference against the manifest's quote, so no case can pass by asserting
/// only the half of the state it happens to care about.
#[derive(Debug, Eq, PartialEq)]
struct WalkState {
    source_phase: SourceResolutionPhaseV1,
    failure_escrow: u64,
    exhaustion_escrow: u64,
    worker: u64,
    failure_certificate_owner: Pubkey,
    success_certificate_owner: Pubkey,
}

async fn walk_state(context: &mut ProgramTestContext, fixture: &Fixture) -> WalkState {
    let source =
        SourceResolutionStateV2::decode(&record_bytes(context, fixture.source_state).await)
            .expect("Source decodes");
    let owner_of =
        |account: Option<Account>| account.map_or(system_program::ID, |account| account.owner);
    WalkState {
        source_phase: source.phase(),
        failure_escrow: lamports_of(context, fixture.failure_funding).await,
        exhaustion_escrow: lamports_of(context, fixture.exhaustion_funding).await,
        worker: lamports_of(context, fixture.worker.pubkey()).await,
        failure_certificate_owner: owner_of(
            context
                .banks_client
                .get_account(fixture.certificate_of(RESOLUTION_FAILURE_KIND))
                .await
                .expect("bank read"),
        ),
        success_certificate_owner: owner_of(
            context
                .banks_client
                .get_account(fixture.certificate_of(RESOLUTION_SUCCESS_KIND))
                .await
                .expect("bank read"),
        ),
    }
}

#[tokio::test]
async fn a_silent_relayer_cannot_make_the_market_unresolvable() {
    // §4.8's headline property, executed rather than argued: **a silent relayer
    // cannot make a market unresolvable, only drive it to a pre-disclosed
    // outcome, along a bounded, prepaid, permissionless path that pays whoever
    // walks it.** Every clause is an assertion below.
    //
    // Nothing about the relayer is an input. No record is created, no
    // observation appended, no set sealed, and the frame carries no provider
    // release, no adapter configuration and no key set -- so this route runs in
    // exactly the world where the relayer has stopped answering, which is the
    // only world it is for.
    let mut fixture = fixture(1, &[]);
    let mut context = start(&mut fixture).await;
    warp_past_the_deadline(&mut context).await;
    let before = walk_state(&mut context, &fixture).await;
    assert_eq!(before.source_phase, SourceResolutionPhaseV1::Primary);
    assert_eq!(
        before.failure_escrow,
        Rent::default().minimum_balance(FUNDING_STATE_BYTES) + BOUNTY,
        "the escrow starts holding rent plus exactly the quoted bounty"
    );

    submit_recorded(
        &mut context,
        &[fixture.deadline_failure_instruction(DeadlineSubstitution::default())],
        &[&fixture.worker],
        "relayed liveness: a silent market walks to its pre-disclosed failure",
    )
    .await
    .expect("a silent market walks to its pre-disclosed failure");

    let after = walk_state(&mut context, &fixture).await;
    assert_eq!(
        after.source_phase,
        SourceResolutionPhaseV1::FailureCommitted,
        "the walk is Primary -> Exhausted -> FailureCommitted in one transition"
    );
    assert_eq!(
        after.worker - before.worker,
        BOUNTY,
        "PAYS WHOEVER WALKS IT: the worker is credited the manifest's own quote"
    );
    assert_eq!(
        before.failure_escrow - after.failure_escrow,
        BOUNTY,
        "PREPAID: the bounty came out of the escrow the market funded at founding"
    );
    assert_eq!(
        after.exhaustion_escrow, before.exhaustion_escrow,
        "the compartment this walk is not about is untouched"
    );
    assert_eq!(
        after.success_certificate_owner,
        system_program::ID,
        "success and failure are different addresses; a failure walk cannot occupy the success one"
    );

    let funding =
        FundingStateV1::decode(&record_bytes(&mut context, fixture.failure_funding).await)
            .expect("Funding state decodes");
    assert_eq!(funding.status(), FundingStatus::Active);
    assert_eq!(
        funding.remaining().bounty().amount(),
        0,
        "the whole disclosed bounty is spent, so a second walker has nothing to be paid from"
    );

    let certificate = ResolutionCertificateV2::decode(
        &record_bytes(
            &mut context,
            fixture.certificate_of(RESOLUTION_FAILURE_KIND),
        )
        .await,
    )
    .expect("terminal certificate decodes");
    assert_eq!(
        certificate.kind,
        ResolutionCertificateKindV2::ResolutionFailure
    );
    assert_eq!(certificate.market, fixture.market.to_bytes());
    assert_eq!(certificate.generation, GENERATION);
    assert_eq!(
        certificate.selector, GRADUATION_REGION_COUNT,
        "PRE-DISCLOSED OUTCOME: the selector is the Product's own explicit failure region, \
         not a value this route chose"
    );
    assert_eq!(
        certificate.route, [0; 32],
        "no route: this terminal is attributable to no provider, which is the whole content \
         of the claim that the relayer went silent"
    );
    assert_eq!(
        certificate.provider_evidence, [0; 32],
        "and no observation stands behind it"
    );
    assert_eq!(
        certificate.funding_allocation, fixture.graph.material_id,
        "the allocation identity is the market's own Source material, which is what makes \
         the explicit-failure compartment identifiable at all"
    );
    assert_eq!(certificate.work_paid, BOUNTY);
    assert_eq!(certificate.funding_remaining, 0);
    assert_eq!(
        certificate.attempt_index, 0,
        "zero legs were skipped: this material bought none"
    );
}

#[tokio::test]
async fn the_walk_refuses_before_the_deadline_it_is_named_for() {
    // BOUNDED, in the direction that matters: the walk is a liveness path, not
    // a way to end a market early. The deadline is `window.end + max_age`, both
    // of them the market's own founding-time content, and one second before it
    // an observation can still resolve this market honestly.
    let mut fixture = fixture(1, &[]);
    let mut context = start(&mut fixture).await;
    let before = walk_state(&mut context, &fixture).await;

    refused_with(
        submit_recorded(
            &mut context,
            &[fixture.deadline_failure_instruction(DeadlineSubstitution::default())],
            &[&fixture.worker],
            "relayed liveness: a walk before the deadline refuses",
        )
        .await,
        REFUSAL_TRANSITION,
    );
    assert_eq!(
        walk_state(&mut context, &fixture).await,
        before,
        "an early walk moves neither the Source, nor either escrow, nor the walker"
    );
}

#[tokio::test]
async fn the_bounty_cannot_be_collected_twice() {
    // The certificate is a PDA of the Source state, the kind and the sequence,
    // and the Source state is terminal after the first walk, so the second
    // attempt has several independent reasons to refuse. Which one it reaches
    // is worth pinning: `Funding` (14), not `Transition` (12), because
    // `plan_deadline_failure_v1` debits BEFORE it transitions, and the
    // compartment's bounty is already spent. A walk that cannot be paid for
    // cannot move the market -- that ordering was a claim in `funded.rs`'s
    // doc comment and this is the case that executes it.
    let mut fixture = fixture(1, &[]);
    let mut context = start(&mut fixture).await;
    warp_past_the_deadline(&mut context).await;
    submit_recorded(
        &mut context,
        &[fixture.deadline_failure_instruction(DeadlineSubstitution::default())],
        &[&fixture.worker],
        "relayed liveness: a silent market walks to its pre-disclosed failure",
    )
    .await
    .expect("the first walk commits");
    let after_first = walk_state(&mut context, &fixture).await;

    refused_with(
        submit_recorded(
            &mut context,
            &[fixture.deadline_failure_instruction(DeadlineSubstitution::default())],
            &[&fixture.worker],
            "relayed liveness: a second walk cannot collect the bounty twice",
        )
        .await,
        REFUSAL_FUNDING,
    );
    assert_eq!(
        walk_state(&mut context, &fixture).await,
        after_first,
        "the second walk pays nobody and moves nothing"
    );
}

#[tokio::test]
async fn a_live_compartment_of_the_same_market_cannot_stand_in_for_the_escrow() {
    // The substituted-compensation case, and the reason it is interesting: the
    // exhaustion compartment is a REAL `FundingState` of this same market, at
    // its own correctly derived address, correctly owned by this Program, and
    // Active. Every check that is about *shape* passes on it.
    //
    // What refuses is the check that is about *identity*: the explicit-failure
    // compartment is the one whose manifest entry configures this market's own
    // Source material, exactly the binding `core_effect` established when the
    // three compartments were created. Substituting the neighbour is therefore
    // refused by that comparison rather than by an account-position convention,
    // which is what makes the refusal survive a frame reordering.
    let mut fixture = fixture(1, &[]);
    let mut context = start(&mut fixture).await;
    warp_past_the_deadline(&mut context).await;
    let before = walk_state(&mut context, &fixture).await;

    refused_with(
        submit_recorded(
            &mut context,
            &[fixture.deadline_failure_instruction(DeadlineSubstitution {
                funding: Some(fixture.exhaustion_funding),
                ..DeadlineSubstitution::default()
            })],
            &[&fixture.worker],
            "relayed liveness: a live compartment that is not the escrow refuses",
        )
        .await,
        REFUSAL_FUNDING,
    );
    assert_eq!(walk_state(&mut context, &fixture).await, before);
}

#[tokio::test]
async fn an_escrow_that_does_not_hold_what_the_market_promised_refuses() {
    // The unfunded shape. The `FundingState` bytes stay exactly canonical --
    // same entry index, same manifest, same Active status, same derived address
    // -- and one lamport goes missing from the account.
    //
    // That is the only way to reach `validate_against`'s custody comparison at
    // all, because the address folds in the state's own bytes: change the bytes
    // and the account is at a different address; change the lamports and the
    // account is at the right address holding the wrong thing. A route that
    // trusted the bytes would pay a bounty out of an escrow that cannot cover
    // it, and the shortfall would surface an instruction later as an unrelated
    // rent failure.
    let mut fixture = fixture(1, &[]);
    let mut context = start(&mut fixture).await;
    warp_past_the_deadline(&mut context).await;
    let mut skimmed = context
        .banks_client
        .get_account(fixture.failure_funding)
        .await
        .expect("bank read")
        .expect("the escrow exists");
    skimmed.lamports -= 1;
    context.set_account(&fixture.failure_funding, &skimmed.into());
    let before = walk_state(&mut context, &fixture).await;

    refused_with(
        submit_recorded(
            &mut context,
            &[fixture.deadline_failure_instruction(DeadlineSubstitution::default())],
            &[&fixture.worker],
            "relayed liveness: an escrow one lamport short refuses",
        )
        .await,
        REFUSAL_FUNDING,
    );
    assert_eq!(
        walk_state(&mut context, &fixture).await,
        before,
        "one lamport short is short: the walk commits nothing and pays nobody"
    );
}
