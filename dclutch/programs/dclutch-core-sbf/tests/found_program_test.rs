//! Real-ELF Core Found37 infrastructure and Runtime Product V2 composition.

use std::{env, fs, path::PathBuf, vec, vec::Vec};

use dclutch_market::capability_manifest::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    CapabilityEntryV1, CapabilityFundingDerivationV1, CapabilityManifestV1, CompartmentFundingV1,
    ContentId as CapabilityContentId, FUNDING_STATE_BYTES, FundingAmountsV1,
    FundingCustodyObservationV1, FundingQuoteV1, FundingStateV1, MANIFEST_HEADER_BYTES,
    MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_market::capability_program::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1, SelectedRecordBumpsV1,
};
use dclutch_claims::{
    founding_v5::ClaimsFoundingAggregateSeedsV5,
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        liability_basis_vector_width_v2,
    },
    protocol_position_v2::{
        PROTOCOL_POSITION_ADMISSION_BYTES_V2, ProtocolPositionAdmissionSeedsV2,
        ProtocolPositionSeedsV2,
    },
};
use dclutch_core_contract::ContentId as CoreContentId;
use dclutch_custody::{
    CompartmentV1, PROJECTED_CUSTODY_STATE_BYTES_V2, PROJECTED_HOARD_CONTEXT_DOMAIN_V1,
    ProjectedCallerRoleV1, ProjectedCustodyLockReceiptV1, ProjectedCustodyOperationV1,
    ProjectedCustodyPhaseV1, ProjectedCustodyRequestV1, ProjectedCustodyStateSeedsV2,
    ProjectedCustodyStateV2,
};
use dclutch_market::{
    Action, CoreState, Identity, MarketCoreStateSeedsV2, MarketIdentity, Phase,
    ProjectFoundRequestV2, Readiness, Request, SERIES_FOUNDING_PERMIT_BYTES_V1, STATE_BYTES,
    SeriesFoundingPermitSeedsV1, SeriesFoundingPermitV1, SeriesPermitExpiryRequestV1,
};
use dclutch_product::payoff::{
    registry_v3::{GRADED_BASIS_RECORD_SCHEMA_ID_V3, PRICE_GATE_RECORD_SCHEMA_ID_V1},
    runtime_v3::{
        BasisInputV3, BasisKindV3, ProductBasisV3, SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
        basis_record_bytes_v3, compile_basis_v3, semantic_basis_preimage_v3,
    },
};
use dclutch_product::{ContentId, portfolio_record_bytes, result_domain_record_bytes};
use dclutch_product::admission::{FinalizedRecordCoordinateV2, PRODUCT_RECORD_BYTES_V2};
use dclutch_product_runtime_v2_operator::{ProductCompilationInputV2, compile_product_records_v2};
use dclutch_market::realm::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1, RealmV1Input,
};
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, activate_execution_role_into_v1,
    initialize_activation_cache_v1,
};
use dclutch_registry::release_set::{
    ArtifactReleaseIdV1, CallerAuthoritySeedsV1, CapabilityExecutionSelectionV1,
    EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1, ExecutionReleaseSetV1, ExecutionRoleBindingV1,
    ExecutionRoleV1, PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2, ProgramIdentityV1,
    ProtocolInfrastructureProfileV1, ProtocolInfrastructureProfileV2,
};
use dclutch_market::rent::{
    RefundAuthority,
    lifecycle_v2::{
        LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleAccountIdV2, LifecycleRentCreditV2,
    },
};
use dclutch_trading::series::{
    AccountKeyV3, AuthenticatedProductProjectionV2, SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3,
    SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3, SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
    admit_occurrence_bytes, admit_ticket, funding_list_id, future_market_projection,
    generated::{
        SERIES_EXAMPLE_OCCURRENCE_V3, SERIES_EXAMPLE_TEMPLATE_V3, SERIES_EXAMPLE_TICKET_V3,
        SERIES_OCCURRENCE_CAPABILITY_MANIFEST_OFFSET_V3,
        SERIES_OCCURRENCE_CAPABILITY_NATIVE_OFFSET_V3, SERIES_OCCURRENCE_FOUNDING_WORK_OFFSET_V3,
        SERIES_OCCURRENCE_FUNDING_LIST_OFFSET_V3, SERIES_OCCURRENCE_HOARD_PRINCIPAL_OFFSET_V3,
        SERIES_OCCURRENCE_INDEX_OFFSET_V3, SERIES_OCCURRENCE_MARKET_OFFSET_V3,
        SERIES_OCCURRENCE_MARKET_RENT_OFFSET_V3, SERIES_OCCURRENCE_PRODUCT_RECORD_OFFSET_V3,
        SERIES_OCCURRENCE_RATIONAL_REPRESENTATION_OFFSET_V3,
        SERIES_OCCURRENCE_RESOLUTION_POLICY_OFFSET_V3, SERIES_OCCURRENCE_SCHEDULED_SLOT_OFFSET_V3,
        SERIES_TEMPLATE_CLOSE_RENT_OFFSET_V3, SERIES_TEMPLATE_FIRST_SLOT_OFFSET_V3,
        SERIES_TEMPLATE_OCCURRENCE_COUNT_OFFSET_V3, SERIES_TEMPLATE_PERIOD_SLOTS_OFFSET_V3,
        SERIES_TEMPLATE_PROJECTION_ROOT_OFFSET_V3, SERIES_TEMPLATE_REALM_OFFSET_V3,
        SERIES_TEMPLATE_REFUND_OWNER_OFFSET_V3, SERIES_TEMPLATE_RELEASE_SET_OFFSET_V3,
        SERIES_TEMPLATE_RETRY_WINDOW_OFFSET_V3, SERIES_TICKET_CAPABILITY_NATIVE_OFFSET_V3,
        SERIES_TICKET_FOUNDER_OFFSET_V3, SERIES_TICKET_FOUNDING_WORK_OFFSET_V3,
        SERIES_TICKET_FUNDING_LIST_OFFSET_V3, SERIES_TICKET_HOARD_PRINCIPAL_OFFSET_V3,
        SERIES_TICKET_INDEX_OFFSET_V3, SERIES_TICKET_MARKET_OFFSET_V3,
        SERIES_TICKET_MARKET_RENT_OFFSET_V3, SERIES_TICKET_OCCURRENCE_ID_OFFSET_V3,
        SERIES_TICKET_REFUND_OWNER_OFFSET_V3, SERIES_TICKET_TEMPLATE_OFFSET_V3,
    },
    occurrence_content_id,
    replay::{SeriesStateV3, TicketPhaseV3, TicketStateSeedsV3, TicketStateV3},
    series_core_consume_request, template_content_id, ticket_content_id,
};
use dclutch_source::{
    CapacityEnvelope, ContentId as SourceContentId, MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1,
    ManipulationFloorBasis, ManipulationFloorV1, SOURCE_CAPACITY_PROFILE_SCHEMA_ID_V1,
    SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3, SOURCE_SPEC_SCHEMA_ID_V1, SourceAccessProfile,
    SourceCapacityProfileV1, SourceMaterialV3, SourceSpecV1,
};
use solana_account::Account;
use solana_address_lookup_table_interface::{
    program as lookup_table_program,
    state::{AddressLookupTable, LookupTableMeta},
};
use solana_hash::Hash;
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_program::{
    clock::Clock,
    hash::{hash, hashv},
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
use solana_transaction::versioned::VersionedTransaction;
use spl_token_interface::state::{Account as SplAccount, AccountState as SplAccountState};

const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc1; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc2; 32]);
const RENT_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc3; 32]);
const TRADING_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc4; 32]);
const CLAIMS_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc5; 32]);
const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc6; 32]);
const RESOLUTION_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc7; 32]);
const TOKEN_PROGRAM_ID: Pubkey = Pubkey::new_from_array(dclutch_custody::token_svm::LEGACY_TOKEN_PROGRAM_ID);
const COLLATERAL_MINT: Pubkey = Pubkey::new_from_array([0xb2; 32]);
const LOOKUP_TABLE: Pubkey = Pubkey::new_from_array([0xb8; 32]);
const GENERATION: u64 = 1;

/// Preimage of this campaign's fixed key seed.
///
/// The seed is HASHED FROM THIS STRING rather than written down as an opaque
/// 32-byte constant, so what is pinned is the sentence and not a magic number
/// nobody can check. Changing this string moves every fixture key and every
/// compute-unit number this campaign produces, which is a re-pin of
/// `tools/gauntlet/CU_BUDGETS.json` and not a refactor.
const FIXTURE_SEED_PREIMAGE_V1: &[u8] =
    b"dclutch/gauntlet/tier4/found-program-test/keypair-seed/v1";

/// Domain separator for the per-role derivation below.
const FIXTURE_KEY_DOMAIN_V1: &[u8] = b"dclutch/gauntlet/tier4/fixture-key/v1";

/// Role names. Part of the derivation, so they live in one place: a typo at a
/// call site would silently be a different key rather than a compile error.
const ROLE_PAYER: &[u8] = b"payer";
const ROLE_HOARD_VAULT: &[u8] = b"hoard-vault";
const ROLE_FUNDING_SOURCE_VAULT: &[u8] = b"funding-source-vault";
const ROLE_FUNDING_SOURCE_REPLAY_VAULT: &[u8] = b"funding-source-replay-vault";
const ROLE_SUBSTITUTED_CLAIMS_PROGRAMDATA: &[u8] = b"substituted-claims-programdata";
/// A profile account whose contents are canonical V2 bytes at the wrong PDA.
const ROLE_WRONG_INFRASTRUCTURE_PROFILE: &[u8] = b"wrong-infrastructure-profile";

/// Deterministic 32 bytes for one fixture role.
///
/// # Why this exists
///
/// This campaign used to draw its fixture addresses from `Keypair::new()` and
/// `Pubkey::new_unique()`. Both are per-run: `Keypair::new()` is random, and
/// `Pubkey::new_unique()` reads a process-global counter, so with four
/// `series_consume` tests running concurrently in one binary the values a given
/// test receives depend on thread interleaving. A different address changes how
/// many iterations `find_program_address` needs to find an off-curve bump, and
/// each iteration is one `sol_create_program_address` syscall at **1,500 CU** —
/// which is the entire reason `tools/gauntlet/CU_BUDGETS.md` records a
/// 24,000-CU band on the tier-4 founding case and had to admit that six runs
/// did not bound the substituted-ProgramData one.
///
/// Seeded, the band is zero and the budget tolerances drop to their floor.
///
/// # The derivation
///
/// ```text
/// seed     = SHA-256(FIXTURE_SEED_PREIMAGE_V1)
/// material = SHA-256(FIXTURE_KEY_DOMAIN_V1 || 0 || seed || 0 || role || 0)
/// ```
///
/// `hashv` is SHA-256. This is the same shape as the successor bootstrap's
/// `--keypair-seed` derivation (`tools/local-validator/bootstrap/successor/
/// src/seed.rs`) under its own domain, so the two campaigns cannot derive the
/// same key from the same role name.
///
/// # Why no safety gate here
///
/// The bootstrap's flag needs one because it can be pointed at an RPC endpoint.
/// This cannot: these keys exist only inside a `solana-program-test` bank that
/// is created and dropped inside one test process. There is no cluster, no
/// network and no funded account for a reproducible key to endanger. The seed
/// is checked in deliberately — a fixture whose seed nobody can read is a
/// fixture nobody can reproduce.
fn fixture_key_material(role: &[u8]) -> [u8; 32] {
    let seed = hashv(&[FIXTURE_SEED_PREIMAGE_V1]).to_bytes();
    hashv(&[
        FIXTURE_KEY_DOMAIN_V1,
        &[0],
        seed.as_slice(),
        &[0],
        role,
        &[0],
    ])
    .to_bytes()
}

/// The signing keypair for one fixture role.
fn fixture_keypair(role: &[u8]) -> Keypair {
    // Every 32-byte string is a valid ed25519 secret seed, so this is total.
    Keypair::new_from_array(fixture_key_material(role))
}

/// A fixture address that is never signed for.
fn fixture_pubkey(role: &[u8]) -> Pubkey {
    Pubkey::new_from_array(fixture_key_material(role))
}

struct Artifacts {
    core: Vec<u8>,
    claims: Vec<u8>,
    custody: Vec<u8>,
    registry: Vec<u8>,
    rent: Vec<u8>,
    resolution: Vec<u8>,
    trading: Vec<u8>,
}

#[derive(Clone)]
struct Record {
    raw: Pubkey,
    staging: Pubkey,
    digest: [u8; 32],
    data: Vec<u8>,
}

struct Fixture {
    test: Option<ProgramTest>,
    payer: Keypair,
    market: Pubkey,
    rent_credit: Pubkey,
    realm: Record,
    product: Record,
    domain: Record,
    portfolio: Record,
    linked_basis: Record,
    /// The `DCLTPGT1` certificate, present only for a curved basis.
    price_gate: Option<Record>,
    source: Record,
    source_spec: Record,
    capacity_profile: Record,
    manipulation_floor: Record,
    manifest: Record,
    release_set: Record,
    cache: Pubkey,
    /// The sealed predecessor profile, still on chain and never again read.
    predecessor_profile: Pubkey,
    core_programdata: Pubkey,
    trading_programdata: Pubkey,
    claims_programdata: Pubkey,
    custody_programdata: Pubkey,
    resolution_programdata: Pubkey,
    registry_programdata: Pubkey,
    rent_programdata: Pubkey,
    profile: Pubkey,
    /// Canonical V2 profile bytes, retained so a hostile can move only the PDA.
    profile_data: Vec<u8>,
    registry_artifact: Record,
    rent_artifact: Record,
    /// The Registry record an operator would publish AFTER the upgrade -- the
    /// re-release the superseded-deployment refusal points them at. It is
    /// finalized, well-formed, and binds the generation actually deployed; it
    /// is simply not the record the sealed profile pins. Planted in every
    /// world: its address is a function of its own digest, so it collides with
    /// nothing and no other test reads it.
    republished_registry_artifact: Record,
    /// The slot the bank must be warped to before submitting, when this world
    /// carries an upgraded deployment generation.
    bank_slot: Option<u64>,
    outcome_count: u32,
}

struct SeriesFixture {
    base: Fixture,
    caller_authority: Pubkey,
    root: Pubkey,
    root_data: Vec<u8>,
    ticket_state: Pubkey,
    ticket_state_data: Vec<u8>,
    template: Record,
    occurrence: Record,
    ticket: Record,
    funding: Pubkey,
    funding_data: Vec<u8>,
    funding_lamports: u64,
    permit: Pubkey,
    permit_lamports: u64,
    projected_replay: Pubkey,
    hoard: Pubkey,
    funding_source: Pubkey,
    funding_source_replay: Pubkey,
    aggregate: Pubkey,
    position: Pubkey,
    admission: Pubkey,
    claims_programdata_meta: Pubkey,
    lock_receipt: [u8; dclutch_custody::PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1],
    request: [u8; dclutch_market::SERIES_CORE_REQUEST_BYTES_V1],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SeriesFault {
    None,
    LateHoardBalance,
    BatchClaimsProgramdata,
    /// The Market PDA holds one lamport LESS than the occurrence budgeted.
    ///
    /// The negative control for the rent floor: underfunding must refuse, and a
    /// genesis is the only place to express it, since nobody can take lamports
    /// back out of a keyless PDA.
    UnderfundedMarket,
}

fn artifacts() -> Artifacts {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    Artifacts {
        core: fs::read(directory.join("dclutch_core_sbf.so")).expect("Core ELF"),
        claims: fs::read(directory.join("dclutch_claims_sbf.so")).expect("Claims ELF"),
        custody: fs::read(directory.join("dclutch_custody_sbf.so")).expect("Custody ELF"),
        registry: fs::read(directory.join("dclutch_registry_sbf.so")).expect("Registry ELF"),
        rent: fs::read(directory.join("dclutch_rent_sbf.so")).expect("Rent ELF"),
        resolution: fs::read(directory.join("dclutch_resolution_proof_sbf.so"))
            .expect("Resolution ELF"),
        trading: fs::read(directory.join("dclutch_series_consume_caller_sbf.so"))
            .expect("Trading caller ELF"),
    }
}

fn identity(bytes: [u8; 32]) -> Identity {
    Identity::new(bytes).expect("identity")
}

fn product_id(byte: u8) -> ContentId {
    ContentId::new([byte; 32]).expect("Product identity")
}

fn source_id(byte: u8) -> SourceContentId {
    SourceContentId::new([byte; 32]).expect("Source identity")
}

fn program_identity(program: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(program.to_bytes()).expect("program")
}

fn programdata_address(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

/// Compose Loader V3 ProgramData reporting `deployment_slot`.
///
/// The slot is a PARAMETER rather than a constant `0` because moving it is the
/// whole of a program upgrade as every reader in this tree can observe one:
/// there is no `bpf_loader_upgradeable::Upgrade` in any harness, and a pinned
/// release is superseded precisely when the generation the Loader reports stops
/// being the generation the release bound. See [`RegistryDeploymentV1`].
fn programdata_bytes(elf: &[u8], authority: Option<Pubkey>, deployment_slot: u64) -> Vec<u8> {
    let mut bytes = vec![0; 45 + elf.len()];
    bytes
        .get_mut(0..4)
        .expect("tag")
        .copy_from_slice(&3_u32.to_le_bytes());
    bytes
        .get_mut(4..12)
        .expect("slot")
        .copy_from_slice(&deployment_slot.to_le_bytes());
    match authority {
        Some(authority) => {
            *bytes.get_mut(12).expect("authority tag") = 1;
            bytes
                .get_mut(13..45)
                .expect("authority")
                .copy_from_slice(authority.as_ref());
        }
        None => *bytes.get_mut(12).expect("authority tag") = 0,
    }
    bytes.get_mut(45..).expect("ELF").copy_from_slice(elf);
    bytes
}

fn add_program(
    test: &mut ProgramTest,
    name: &'static str,
    program: Pubkey,
    elf: &[u8],
    authority: Option<Pubkey>,
    deployment_slot: u64,
) {
    test.add_upgradeable_program_to_genesis(name, &program);
    let data = programdata_bytes(elf, authority, deployment_slot);
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

fn release(
    program: Pubkey,
    elf: &[u8],
    semantic: u8,
    authority: Option<Pubkey>,
) -> ArtifactReleaseV1 {
    release_at_slot(program, elf, semantic, authority, GENESIS_DEPLOYMENT_SLOT)
}

/// A release binding an explicit deployment generation.
///
/// Only the republished Registry record needs one: it is the "re-release" the
/// superseded-deployment refusal points an operator at, and its whole point is
/// that it binds the generation the upgrade produced rather than the one the
/// sealed profile pins.
fn release_at_slot(
    program: Pubkey,
    elf: &[u8],
    semantic: u8,
    authority: Option<Pubkey>,
    deployment_slot: u64,
) -> ArtifactReleaseV1 {
    let (policy, authority_bytes) = match authority {
        Some(authority) => (
            ArtifactUpgradePolicyV1::ExactAuthority,
            Some(authority.to_bytes()),
        ),
        None => (ArtifactUpgradePolicyV1::Immutable, None),
    };
    ArtifactReleaseV1::new(
        program_identity(program),
        program_identity(bpf_loader_upgradeable::ID),
        programdata_address(program).to_bytes(),
        CoreContentId::new([semantic; 32]).expect("semantic release"),
        hash(elf).to_bytes(),
        deployment_slot,
        policy,
        authority_bytes,
    )
    .expect("artifact release")
}

fn artifact_id(value: ArtifactReleaseV1) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(hash(&value.to_bytes()).to_bytes()).expect("artifact ID")
}

fn binding(value: ArtifactReleaseV1) -> ExecutionRoleBindingV1 {
    ExecutionRoleBindingV1::new(value.program(), artifact_id(value))
}

fn activation_input(value: ArtifactReleaseV1) -> ArtifactActivationInputV1 {
    ArtifactActivationInputV1::new(
        artifact_id(value),
        value,
        DeploymentObservationV1::new(
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
        .expect("deployment"),
    )
}

impl Record {
    fn new(schema: [u8; 32], data: Vec<u8>) -> Self {
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
        Self {
            raw,
            staging,
            digest,
            data,
        }
    }

    fn from_coordinate(coordinate: FinalizedRecordCoordinateV2, data: Vec<u8>) -> Self {
        let record = Self::new(coordinate.schema_id.to_bytes(), data);
        assert_eq!(record.digest, coordinate.content_digest.to_bytes());
        assert_eq!(record.raw.to_bytes(), coordinate.raw_account.to_bytes());
        assert_eq!(
            record.staging.to_bytes(),
            coordinate.staging_account.to_bytes()
        );
        record
    }

    fn add(&self, test: &mut ProgramTest) {
        test.add_account(
            self.raw,
            Account {
                lamports: Rent::default().minimum_balance(self.data.len()),
                data: self.data.clone(),
                owner: REGISTRY_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        );
        test.add_account(
            self.staging,
            Account {
                lamports: 1,
                data: Vec::new(),
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            },
        );
    }
}

fn capability_id(byte: u8) -> CapabilityContentId {
    CapabilityContentId::new([byte; 32]).expect("capability identity")
}

fn funded_manifest_record() -> Record {
    let none = CompartmentFundingV1::not_applicable();
    let amounts = FundingAmountsV1::new(
        none,
        none,
        CompartmentFundingV1::native_lamports(1_000).expect("work funding"),
        none,
        none,
        none,
        none,
    )
    .expect("funding amounts");
    let quote = FundingQuoteV1::new(amounts, None).expect("native quote");
    let entry = CapabilityEntryV1::new(
        capability_id(0x91),
        capability_id(0x92),
        capability_id(0x93),
        capability_id(0x94),
        capability_id(0x95),
        capability_id(0x96),
        ActivationPolicy::RequiredAtFounding,
        0,
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        quote,
    )
    .expect("manifest entry");
    let mut bytes = vec![0; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&[entry], &mut bytes).expect("funded manifest");
    Record::new(CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, bytes)
}

/// A degree-2 curved Product graph, and the `DCLTPGT1` certificate that admits
/// it.
///
/// The certificate is built the cheapest genuinely-valid way: **one atom, with
/// `weight == mass == 1`**. The hull identity then collapses to
/// `price_j * 1 == 1 * payout_j`, so the certified price vector *is* the payout
/// vector the basis produces at that coordinate -- and it partitions the scale
/// for free, because a payout vector already does.
///
/// The basis is compiled three times, which is not waste but ordering: the
/// certificate's prices depend on the basis's payouts, and the basis's digest
/// field depends on the certificate. Neither the digest nor the result domain
/// changes what the evaluator pays, so the fixed point is reached in one pass
/// rather than iterated.
fn curved_product_graph() -> (Record, Record, Record, Record, Record, u32, [u8; 32]) {
    // Small enough that the fixture's principal capacity yields at least one
    // complete set: `derive_principal_cap_sets` divides the cap by the basis
    // scale, and this fixture's hoard principal is 5_000.
    const SCALE: u64 = 100;
    const WIDTH: u32 = 5;
    let knots: Vec<i128> = vec![0, 0, 0, 1, 2, 3, 3, 3];
    let failure = vec![SCALE / u64::from(WIDTH); WIDTH as usize];
    let kind = BasisKindV3::SplineDegree2To3 {
        degree: 2,
        interior_multiplicity: false,
    };
    let basis_bytes =
        basis_record_bytes_v3(kind, WIDTH as usize, knots.len(), 0).expect("basis width");

    let base_input = BasisInputV3 {
        kind,
        product_id: product_id(1).to_bytes(),
        result_domain_id: [0xf2; 32],
        coordinate_domain_id: product_id(2).to_bytes(),
        result_unit_id: product_id(3).to_bytes(),
        evaluator_release_id: [0xf3; 32],
        basis_width: WIDTH,
        payout_scale: SCALE,
        knot_denominator: 1,
        knots: &knots,
        terms: &[],
        failure_payouts: &failure,
        price_gate_certificate_digest: [1_u8; 32],
    };

    // Pass one: a placeholder digest, purely to read the payouts off the live
    // evaluator at the atom coordinate.
    let (atom_numerator, atom_denominator) = (3_i64, 2_u32);
    let mut probe = vec![0_u8; basis_bytes];
    compile_basis_v3(base_input, &mut probe).expect("probe basis");
    let mut payouts = vec![0_u64; WIDTH as usize];
    ProductBasisV3::decode(&probe)
        .expect("probe decodes")
        .evaluate_rational(
            i128::from(atom_numerator),
            u64::from(atom_denominator),
            &mut payouts,
        )
        .expect("the live evaluator pays");
    assert_eq!(payouts.iter().sum::<u64>(), SCALE, "an exact partition");

    let mut certificate = [0_u8; 320];
    certificate[0..8].copy_from_slice(b"DCLTPGT1");
    certificate[8..10].copy_from_slice(&1_u16.to_le_bytes());
    certificate[10..12].copy_from_slice(&1_u16.to_le_bytes());
    certificate[12..16].copy_from_slice(&u32::try_from(SCALE).expect("scale").to_le_bytes());
    certificate[16..24].copy_from_slice(&1_u64.to_le_bytes());
    certificate[24] = 2;
    certificate[25] = u8::try_from(WIDTH).expect("width");
    certificate[26] = 1;
    for (claim, payout) in payouts.iter().enumerate() {
        certificate[40 + claim * 8..48 + claim * 8].copy_from_slice(&payout.to_le_bytes());
    }
    certificate[120..128].copy_from_slice(&1_u64.to_le_bytes());
    certificate[200..208].copy_from_slice(&atom_numerator.to_le_bytes());
    certificate[280..284].copy_from_slice(&atom_denominator.to_le_bytes());
    let price_gate = Record::new(PRICE_GATE_RECORD_SCHEMA_ID_V1, certificate.to_vec());

    // Pass two: the real digest, for the semantic identity the Product commits.
    let provisional_input = BasisInputV3 {
        price_gate_certificate_digest: price_gate.digest,
        ..base_input
    };
    let mut provisional = vec![0_u8; basis_bytes];
    compile_basis_v3(provisional_input, &mut provisional).expect("provisional basis");
    let semantic = semantic_basis_preimage_v3(&provisional).expect("semantic basis");
    let liability_basis_id = ContentId::new(
        hashv(&[
            SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
            semantic.prefix(),
            semantic.suffix(),
        ])
        .to_bytes(),
    )
    .expect("semantic basis ID");

    // `outcome_count == cuts.len() + 2`, and it must equal the basis width.
    let cuts: Vec<i128> = (0_i128..i128::from(WIDTH) - 2).collect();
    let coefficients = vec![7_u64; cuts.len() + 2];
    let mut product = [0_u8; PRODUCT_RECORD_BYTES_V2];
    let mut domain = vec![0_u8; result_domain_record_bytes(cuts.len()).expect("domain bytes")];
    let mut portfolio =
        vec![0_u8; portfolio_record_bytes(coefficients.len()).expect("portfolio bytes")];
    let report = compile_product_records_v2(
        REGISTRY_PROGRAM_ID,
        ProductCompilationInputV2 {
            product_id: product_id(1),
            coordinate_domain_id: product_id(2),
            result_unit_id: product_id(3),
            claim_basis_id: product_id(4),
            liability_basis_id,
            representation_release_id: product_id(6),
            mapping_release_id: product_id(7),
            cut_denominator: 1,
            cuts: &cuts,
            portfolio_denominator: 9,
            coefficients: &coefficients,
        },
        &mut product,
        &mut domain,
        &mut portfolio,
    )
    .expect("curved Product graph");
    assert_eq!(
        report.outcome_count, WIDTH,
        "the basis width is the outcome count"
    );

    // Pass three: the real result domain, which the semantic preimage omits so
    // the identity above still stands.
    let final_input = BasisInputV3 {
        result_domain_id: report.receipt.result_domain.content_digest.to_bytes(),
        ..provisional_input
    };
    let mut linked_basis = vec![0_u8; basis_bytes];
    compile_basis_v3(final_input, &mut linked_basis).expect("linked curved basis");
    (
        Record::from_coordinate(report.receipt.product, product.to_vec()),
        Record::from_coordinate(report.receipt.result_domain, domain),
        Record::from_coordinate(report.receipt.portfolio, portfolio),
        Record::new(GRADED_BASIS_RECORD_SCHEMA_ID_V3, linked_basis),
        price_gate,
        report.outcome_count,
        product_id(1).to_bytes(),
    )
}

fn product_graph() -> (Record, Record, Record, Record, u32, [u8; 32]) {
    let provisional_input = BasisInputV3 {
        kind: BasisKindV3::CategoricalQ1,
        product_id: product_id(1).to_bytes(),
        result_domain_id: [0xf2; 32],
        coordinate_domain_id: product_id(2).to_bytes(),
        result_unit_id: product_id(3).to_bytes(),
        evaluator_release_id: [0xf3; 32],
        basis_width: 258,
        payout_scale: 1,
        knot_denominator: 1,
        knots: &[],
        terms: &[],
        failure_payouts: &[],
        // Exempt by proof: degree 0 and 1 need no price gate,
        // and a digest offered alongside one is refused.
        price_gate_certificate_digest: [0_u8; 32],
    };
    let basis_width =
        basis_record_bytes_v3(BasisKindV3::CategoricalQ1, 258, 0, 0).expect("basis width");
    let mut provisional_basis = vec![0_u8; basis_width];
    compile_basis_v3(provisional_input, &mut provisional_basis).expect("provisional basis");
    let semantic = semantic_basis_preimage_v3(&provisional_basis).expect("semantic basis");
    let liability_basis_id = ContentId::new(
        hashv(&[
            SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
            semantic.prefix(),
            semantic.suffix(),
        ])
        .to_bytes(),
    )
    .expect("semantic basis ID");
    let cuts: Vec<i128> = (-128_i128..128).collect();
    let coefficients = vec![7_u64; cuts.len() + 2];
    let mut product = [0_u8; PRODUCT_RECORD_BYTES_V2];
    let mut domain = vec![0_u8; result_domain_record_bytes(cuts.len()).expect("domain bytes")];
    let mut portfolio =
        vec![0_u8; portfolio_record_bytes(coefficients.len()).expect("portfolio bytes")];
    let report = compile_product_records_v2(
        REGISTRY_PROGRAM_ID,
        ProductCompilationInputV2 {
            product_id: product_id(1),
            coordinate_domain_id: product_id(2),
            result_unit_id: product_id(3),
            claim_basis_id: product_id(4),
            liability_basis_id,
            representation_release_id: product_id(6),
            mapping_release_id: product_id(7),
            cut_denominator: 1,
            cuts: &cuts,
            portfolio_denominator: 9,
            coefficients: &coefficients,
        },
        &mut product,
        &mut domain,
        &mut portfolio,
    )
    .expect("runtime Product graph");
    let final_input = BasisInputV3 {
        result_domain_id: report.receipt.result_domain.content_digest.to_bytes(),
        ..provisional_input
    };
    let mut linked_basis = vec![0_u8; basis_width];
    compile_basis_v3(final_input, &mut linked_basis).expect("linked basis");
    (
        Record::from_coordinate(report.receipt.product, product.to_vec()),
        Record::from_coordinate(report.receipt.result_domain, domain),
        Record::from_coordinate(report.receipt.portfolio, portfolio),
        Record::new(GRADED_BASIS_RECORD_SCHEMA_ID_V3, linked_basis),
        report.outcome_count,
        product_id(1).to_bytes(),
    )
}

fn put(target: &mut [u8], offset: usize, source: &[u8]) {
    target
        .get_mut(offset..offset + source.len())
        .expect("fixture field")
        .copy_from_slice(source);
}

fn series_fixture(fault: SeriesFault) -> SeriesFixture {
    let mut base = fixture(false);
    let test = base.test.as_mut().expect("ProgramTest");
    let rent = Rent::default();
    if fault == SeriesFault::UnderfundedMarket {
        // Re-declare the Market PDA one lamport short of what the occurrence
        // budgets. Genesis is the only place this can be expressed: nothing can
        // take lamports back out of a keyless system-owned PDA once they land.
        test.add_account(
            base.market,
            Account {
                lamports: rent
                    .minimum_balance(STATE_BYTES)
                    .checked_sub(1)
                    .expect("underfunded market"),
                data: Vec::new(),
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            },
        );
    }
    let manifest = CapabilityManifestV1::decode(&base.manifest.data).expect("manifest");
    let manifest_id = CapabilityContentId::new(base.manifest.digest).expect("manifest ID");
    let funding_rent = rent.minimum_balance(FUNDING_STATE_BYTES);
    let funding_lamports = funding_rent.checked_add(1_000).expect("funding lamports");
    let custody = FundingCustodyObservationV1::native_only(funding_lamports, funding_rent)
        .expect("funding custody");
    let funding_state =
        FundingStateV1::new(manifest_id, manifest, 0, custody).expect("pending FundingState");
    let derivation = CapabilityFundingDerivationV1::new(
        base.market.to_bytes(),
        GENERATION,
        manifest_id,
        manifest,
        funding_state,
    )
    .expect("funding derivation");
    let funding =
        Pubkey::find_program_address(&derivation.seed_components(), &TRADING_PROGRAM_ID).0;
    let funding_key = AccountKeyV3::new(funding.to_bytes()).expect("funding key");
    let funding_list = funding_list_id(&[funding_key]).expect("funding list");

    let market_rent = rent.minimum_balance(STATE_BYTES);
    let capability_native = funding_lamports;
    let founding_work = 777_u64;
    let hoard_principal = 5_000_u64;
    let mut occurrence_bytes = SERIES_EXAMPLE_OCCURRENCE_V3;
    put(
        &mut occurrence_bytes,
        SERIES_OCCURRENCE_INDEX_OFFSET_V3,
        &0_u32.to_le_bytes(),
    );
    put(
        &mut occurrence_bytes,
        SERIES_OCCURRENCE_SCHEDULED_SLOT_OFFSET_V3,
        &0_u64.to_le_bytes(),
    );
    put(
        &mut occurrence_bytes,
        SERIES_OCCURRENCE_PRODUCT_RECORD_OFFSET_V3,
        &base.product.digest,
    );
    put(
        &mut occurrence_bytes,
        SERIES_OCCURRENCE_RESOLUTION_POLICY_OFFSET_V3,
        &base.source.digest,
    );
    put(
        &mut occurrence_bytes,
        SERIES_OCCURRENCE_RATIONAL_REPRESENTATION_OFFSET_V3,
        &[0x82; 32],
    );
    put(
        &mut occurrence_bytes,
        SERIES_OCCURRENCE_CAPABILITY_MANIFEST_OFFSET_V3,
        &base.manifest.digest,
    );
    put(
        &mut occurrence_bytes,
        SERIES_OCCURRENCE_FUNDING_LIST_OFFSET_V3,
        &funding_list.to_bytes(),
    );
    put(
        &mut occurrence_bytes,
        SERIES_OCCURRENCE_MARKET_OFFSET_V3,
        base.market.as_ref(),
    );
    for (offset, value) in [
        (SERIES_OCCURRENCE_HOARD_PRINCIPAL_OFFSET_V3, hoard_principal),
        (SERIES_OCCURRENCE_MARKET_RENT_OFFSET_V3, market_rent),
        (
            SERIES_OCCURRENCE_CAPABILITY_NATIVE_OFFSET_V3,
            capability_native,
        ),
        (SERIES_OCCURRENCE_FOUNDING_WORK_OFFSET_V3, founding_work),
    ] {
        put(&mut occurrence_bytes, offset, &value.to_le_bytes());
    }
    let occurrence_id = occurrence_content_id(&occurrence_bytes).expect("occurrence ID");

    let mut template_bytes = SERIES_EXAMPLE_TEMPLATE_V3;
    put(
        &mut template_bytes,
        SERIES_TEMPLATE_OCCURRENCE_COUNT_OFFSET_V3,
        &1_u32.to_le_bytes(),
    );
    for (offset, value) in [
        (SERIES_TEMPLATE_FIRST_SLOT_OFFSET_V3, 0_u64),
        (SERIES_TEMPLATE_PERIOD_SLOTS_OFFSET_V3, 1_u64),
        (SERIES_TEMPLATE_RETRY_WINDOW_OFFSET_V3, 10_000_u64),
        (SERIES_TEMPLATE_CLOSE_RENT_OFFSET_V3, 10_u64),
    ] {
        put(&mut template_bytes, offset, &value.to_le_bytes());
    }
    put(
        &mut template_bytes,
        SERIES_TEMPLATE_REALM_OFFSET_V3,
        &base.realm.digest,
    );
    put(
        &mut template_bytes,
        SERIES_TEMPLATE_RELEASE_SET_OFFSET_V3,
        &base.release_set.digest,
    );
    put(
        &mut template_bytes,
        SERIES_TEMPLATE_PROJECTION_ROOT_OFFSET_V3,
        &occurrence_id.to_bytes(),
    );
    put(
        &mut template_bytes,
        SERIES_TEMPLATE_REFUND_OWNER_OFFSET_V3,
        base.payer.pubkey().as_ref(),
    );
    let template_id = template_content_id(&template_bytes).expect("Template ID");

    let mut ticket_bytes = SERIES_EXAMPLE_TICKET_V3;
    put(
        &mut ticket_bytes,
        SERIES_TICKET_INDEX_OFFSET_V3,
        &0_u32.to_le_bytes(),
    );
    put(
        &mut ticket_bytes,
        SERIES_TICKET_TEMPLATE_OFFSET_V3,
        &template_id.to_bytes(),
    );
    put(
        &mut ticket_bytes,
        SERIES_TICKET_OCCURRENCE_ID_OFFSET_V3,
        &occurrence_id.to_bytes(),
    );
    put(
        &mut ticket_bytes,
        SERIES_TICKET_MARKET_OFFSET_V3,
        base.market.as_ref(),
    );
    put(
        &mut ticket_bytes,
        SERIES_TICKET_FUNDING_LIST_OFFSET_V3,
        &funding_list.to_bytes(),
    );
    put(
        &mut ticket_bytes,
        SERIES_TICKET_FOUNDER_OFFSET_V3,
        base.payer.pubkey().as_ref(),
    );
    put(
        &mut ticket_bytes,
        SERIES_TICKET_REFUND_OWNER_OFFSET_V3,
        base.payer.pubkey().as_ref(),
    );
    for (offset, value) in [
        (SERIES_TICKET_HOARD_PRINCIPAL_OFFSET_V3, hoard_principal),
        (SERIES_TICKET_MARKET_RENT_OFFSET_V3, market_rent),
        (SERIES_TICKET_CAPABILITY_NATIVE_OFFSET_V3, capability_native),
        (SERIES_TICKET_FOUNDING_WORK_OFFSET_V3, founding_work),
    ] {
        put(&mut ticket_bytes, offset, &value.to_le_bytes());
    }

    let admitted = admit_occurrence_bytes(&template_bytes, &occurrence_bytes, &[])
        .expect("admitted occurrence");
    let admitted_ticket = admit_ticket(&ticket_bytes).expect("admitted Ticket");
    admitted
        .require_ticket(admitted_ticket.ticket())
        .expect("Ticket join");
    let product_projection = AuthenticatedProductProjectionV2::new(
        CoreContentId::new(base.product.digest).expect("Product record"),
        CoreContentId::new(product_id(1).to_bytes()).expect("stable Product"),
        CoreContentId::new(base.domain.digest).expect("result domain"),
    );
    let future = future_market_projection(
        admitted,
        product_projection,
        AccountKeyV3::new(REGISTRY_PROGRAM_ID.to_bytes()).expect("Registry"),
    )
    .expect("future Market");
    assert_eq!(
        future.committed_address().to_bytes(),
        base.market.to_bytes()
    );
    assert_eq!(
        Pubkey::find_program_address(&future.seeds().as_slices(), &CORE_PROGRAM_ID).0,
        base.market
    );

    let selection = CapabilityExecutionSelectionV1::from_bytes(
        0,
        base.manifest.digest,
        [0x97; 32],
        [0x98; 32],
        template_id.to_bytes(),
    )
    .expect("root selection");
    let header = CapabilityRootHeaderV1::new(
        CoreContentId::new(base.release_set.digest).expect("release set"),
        base.market.to_bytes(),
        GENERATION,
        selection,
        SelectedRecordBumpsV1::default(),
    )
    .expect("root header");
    let root = Pubkey::find_program_address(&header.seeds().as_slices(), &TRADING_PROGRAM_ID).0;
    let series_state = SeriesStateV3::new(10)
        .prepare_ticket(0)
        .expect("prepared Series")
        .encode(1)
        .expect("Series bytes");
    let mut root_data = vec![0; CAPABILITY_ROOT_HEADER_BYTES_V1 + series_state.len()];
    root_data
        .get_mut(..CAPABILITY_ROOT_HEADER_BYTES_V1)
        .expect("root header")
        .copy_from_slice(&header.to_bytes());
    root_data
        .get_mut(CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .expect("Series tail")
        .copy_from_slice(&series_state);
    test.add_account(
        root,
        Account {
            lamports: rent.minimum_balance(root_data.len()),
            data: root_data.clone(),
            owner: TRADING_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let ticket_id = ticket_content_id(&ticket_bytes).expect("Ticket ID");
    let ticket_seeds = TicketStateSeedsV3::new(root.to_bytes(), ticket_id);
    let ticket_state =
        Pubkey::find_program_address(&ticket_seeds.as_slices(), &TRADING_PROGRAM_ID).0;
    let ticket_state_data = TicketStateV3::prepared(ticket_id).encode().to_vec();
    test.add_account(
        ticket_state,
        Account {
            lamports: rent.minimum_balance(ticket_state_data.len()),
            data: ticket_state_data.clone(),
            owner: TRADING_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let request = series_core_consume_request(
        admitted,
        admitted_ticket,
        product_projection,
        AccountKeyV3::new(ticket_state.to_bytes()).expect("Ticket state"),
        1,
        0,
    )
    .expect("Series Core request")
    .encode()
    .expect("Series Core request bytes");
    let caller_seeds = CallerAuthoritySeedsV1::from_bytes(
        base.release_set.digest,
        base.market.to_bytes(),
        ExecutionRoleV1::Trading,
        ticket_id.to_bytes(),
        hash(&request).to_bytes(),
    )
    .expect("Trading caller seeds");
    let caller_authority =
        Pubkey::find_program_address(&caller_seeds.as_slices(), &TRADING_PROGRAM_ID).0;
    test.add_account(
        caller_authority,
        Account {
            lamports: 1,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let template = Record::new(
        SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
        template_bytes.to_vec(),
    );
    let occurrence = Record::new(
        SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3,
        occurrence_bytes.to_vec(),
    );
    let ticket = Record::new(SERIES_TICKET_SCHEMA_RELEASE_ID_V3, ticket_bytes.to_vec());
    for record in [&template, &occurrence, &ticket] {
        record.add(test);
    }
    let funding_data = funding_state.to_bytes().to_vec();
    test.add_account(
        funding,
        Account {
            lamports: funding_lamports,
            data: funding_data.clone(),
            owner: TRADING_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let hoard = fixture_pubkey(ROLE_HOARD_VAULT);
    let funding_source = fixture_pubkey(ROLE_FUNDING_SOURCE_VAULT);
    let funding_source_replay = fixture_pubkey(ROLE_FUNDING_SOURCE_REPLAY_VAULT);
    let claims_programdata_meta = if fault == SeriesFault::BatchClaimsProgramdata {
        let substituted = fixture_pubkey(ROLE_SUBSTITUTED_CLAIMS_PROGRAMDATA);
        let data = programdata_bytes(&[0x42; 32], None, GENESIS_DEPLOYMENT_SLOT);
        test.add_account(
            substituted,
            Account {
                lamports: rent.minimum_balance(data.len()),
                data,
                owner: bpf_loader_upgradeable::ID,
                executable: false,
                rent_epoch: 0,
            },
        );
        substituted
    } else {
        base.claims_programdata
    };
    let projected_context = hashv(&[
        PROJECTED_HOARD_CONTEXT_DOMAIN_V1,
        ticket_id.to_bytes().as_slice(),
    ])
    .to_bytes();
    let projected_request = ProjectedCustodyRequestV1 {
        operation: ProjectedCustodyOperationV1::LockHoardAndCloseSource,
        caller_role: ProjectedCallerRoleV1::TradingCapability,
        market: base.market.to_bytes(),
        generation: GENERATION,
        realm: base.realm.digest,
        product_record: base.product.digest,
        product: product_id(1).to_bytes(),
        source: base.source.digest,
        release_set: base.release_set.digest,
        projection_receipt_digest: [0xd0; 32],
        parent_capability_root: root.to_bytes(),
        context_digest: projected_context,
        caller_program: TRADING_PROGRAM_ID.to_bytes(),
        payer: base.payer.pubkey().to_bytes(),
        core_program: CORE_PROGRAM_ID.to_bytes(),
        rent_program: RENT_PROGRAM_ID.to_bytes(),
        refund_owner: base.payer.pubkey().to_bytes(),
        rent_credit: base.rent_credit.to_bytes(),
        hoard_vault: hoard.to_bytes(),
        funding_source_vault: funding_source.to_bytes(),
        funding_source_context: ticket_id.to_bytes(),
        funding_source_compartment: CompartmentV1::SeriesEscrow,
        mint: COLLATERAL_MINT.to_bytes(),
        token_program: TOKEN_PROGRAM_ID.to_bytes(),
        collateral_release: [0xb3; 32],
        expiry_slot: 10_000,
        expected_revision: 2,
        resulting_revision: 3,
        amount: hoard_principal,
        state_rent_lamports: rent.minimum_balance(PROJECTED_CUSTODY_STATE_BYTES_V2),
        vault_rent_lamports: rent.minimum_balance(SplAccount::LEN),
        funding_source_replay_revision: 3,
        funding_source_state_rent_lamports: 1,
        funding_source_vault_rent_lamports: 1,
    };
    let lock_request_digest = hash(
        &projected_request
            .encode()
            .expect("projected LockAndClose request"),
    )
    .to_bytes();
    let projected_seeds = ProjectedCustodyStateSeedsV2::from_request(projected_request);
    let (projected_replay, projected_bump) =
        Pubkey::find_program_address(&projected_seeds.as_slices(), &CUSTODY_PROGRAM_ID);
    let projected_state = ProjectedCustodyStateV2 {
        phase: ProjectedCustodyPhaseV1::HoardLocked,
        request: projected_request,
        next_revision: 3,
        locked_amount: hoard_principal,
        last_request_digest: lock_request_digest,
        principal_cap_sets: u64::MAX,
        bump: projected_bump,
    };
    let projected_data = projected_state.encode().expect("projected state").to_vec();
    test.add_account(
        projected_replay,
        Account {
            lamports: rent.minimum_balance(projected_data.len()),
            data: projected_data,
            owner: CUSTODY_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    let mut hoard_data = vec![0_u8; SplAccount::LEN];
    SplAccount::pack(
        SplAccount {
            mint: COLLATERAL_MINT,
            owner: Pubkey::new_from_array([0xb4; 32]),
            amount: if fault == SeriesFault::LateHoardBalance {
                hoard_principal - 1
            } else {
                hoard_principal
            },
            delegate: COption::None,
            state: SplAccountState::Initialized,
            is_native: COption::None,
            delegated_amount: 0,
            close_authority: COption::None,
        },
        &mut hoard_data,
    )
    .expect("Hoard token account");
    test.add_account(
        hoard,
        Account {
            lamports: rent.minimum_balance(hoard_data.len()),
            data: hoard_data,
            owner: TOKEN_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    let lock_receipt = ProjectedCustodyLockReceiptV1 {
        market: base.market.to_bytes(),
        release_set: base.release_set.digest,
        context_digest: projected_context,
        source_vault: funding_source.to_bytes(),
        source_replay: funding_source_replay.to_bytes(),
        hoard_vault: hoard.to_bytes(),
        rent_credit: base.rent_credit.to_bytes(),
        request_digest: lock_request_digest,
        amount: hoard_principal,
        source_vault_rent_lamports: 1,
        source_replay_rent_lamports: 1,
        resulting_revision: 3,
    }
    .encode()
    .expect("LockAndClose receipt");

    let permit_seeds = SeriesFoundingPermitSeedsV1::new(
        identity(base.release_set.digest),
        identity(base.market.to_bytes()),
        identity(ticket_id.to_bytes()),
    );
    let permit = Pubkey::find_program_address(&permit_seeds.as_slices(), &CORE_PROGRAM_ID).0;
    let permit_lamports = rent.minimum_balance(SERIES_FOUNDING_PERMIT_BYTES_V1);
    test.add_account(
        permit,
        Account {
            lamports: permit_lamports,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let aggregate_seeds =
        ClaimsFoundingAggregateSeedsV5::new(base.market.to_bytes()).expect("aggregate seeds");
    let aggregate =
        Pubkey::find_program_address(&aggregate_seeds.as_slices(), &CLAIMS_PROGRAM_ID).0;
    let position_seeds =
        ProtocolPositionSeedsV2::new(aggregate.to_bytes(), base.payer.pubkey().to_bytes())
            .expect("Position seeds");
    let position = Pubkey::find_program_address(&position_seeds.as_slices(), &CLAIMS_PROGRAM_ID).0;
    let admission_seeds =
        ProtocolPositionAdmissionSeedsV2::new(aggregate.to_bytes(), base.payer.pubkey().to_bytes())
            .expect("admission seeds");
    let admission =
        Pubkey::find_program_address(&admission_seeds.as_slices(), &CLAIMS_PROGRAM_ID).0;
    for (key, width) in [
        (
            aggregate,
            liability_basis_vector_width_v2(
                LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
                base.outcome_count,
            )
            .expect("aggregate width"),
        ),
        (
            position,
            liability_basis_vector_width_v2(
                LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
                base.outcome_count,
            )
            .expect("Position width"),
        ),
        (admission, PROTOCOL_POSITION_ADMISSION_BYTES_V2),
    ] {
        test.add_account(
            key,
            Account {
                lamports: rent.minimum_balance(width),
                data: Vec::new(),
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            },
        );
    }

    SeriesFixture {
        base,
        caller_authority,
        root,
        root_data,
        ticket_state,
        ticket_state_data,
        template,
        occurrence,
        ticket,
        funding,
        funding_data,
        funding_lamports,
        permit,
        permit_lamports,
        projected_replay,
        hoard,
        funding_source,
        funding_source_replay,
        aggregate,
        position,
        admission,
        claims_programdata_meta,
        lock_receipt,
        request,
    }
}

/// The generation every program in this fixture is deployed in by default, and
/// the one every `ArtifactReleaseV1` here pins.
const GENESIS_DEPLOYMENT_SLOT: u64 = 0;
/// The Registry's Loader upgrade authority in the worlds that have one.
///
/// It never signs anything here: an upgrade is simulated by restaging
/// ProgramData, and the Found route only ever COMPARES this key against the one
/// the release bound.
const REGISTRY_UPGRADE_AUTHORITY: Pubkey = Pubkey::new_from_array([0xd2; 32]);
/// The generation the Registry lands in when this campaign upgrades it.
///
/// Any later slot works; this is `direct-hot/src/waist.rs`'s
/// `UPGRADED_DEPLOYMENT_SLOT`, reused so the two upgrade fixtures in this tree
/// describe the same event with the same number.
const UPGRADED_REGISTRY_DEPLOYMENT_SLOT: u64 = 531;

/// Which Registry deployment generation the bank observes.
///
/// This is the ruling's §8.1 obligation -- P-008's brick, reproduced rather
/// than argued. The profile pins the Registry's `ArtifactReleaseV1` BY CONTENT,
/// deployment slot included, so an upgrade that moves the Registry's bytes to a
/// new generation supersedes the very selection every consumer authenticates
/// against, and the write-once profile cannot be re-pointed at the new one.
///
/// The two variants are one bank each, and they differ in EXACTLY ONE FACT: the
/// slot Loader V3 ProgramData reports for the Registry. The ELF bytes are
/// byte-identical across both, which is deliberate and is what makes this a
/// measurement rather than a tautology -- a fixture whose upgrade also changed
/// the ELF digest could not distinguish "the slot pin refuses the release that
/// moved" from "some other coordinate of the release changed". The slot alone
/// bricks it.
///
/// Two banks and not one, for the reason `waist.rs:265-305` documents at
/// length: `solana-program-test` holds exactly one nonzero deployment
/// generation on its fork, and `warp_to_slot` roots the slot below and drops
/// every bank under it, so staging a before-generation and an after-generation
/// in the same bank makes the program cache reload the first one forever.
///
/// **This is measured against the V2 profile, which is what the shipped
/// consumers read**, not against V1. The brick is not a property of the
/// profile's VERSION -- it is a property of a content pin over a write-once
/// account with one vacancy. V1 was bricked because its vacancy was spent and
/// it had no succession route; V2's vacancy is spent by the cohort-9 ceremony,
/// so the constraint P-008 names as standing ("an infrastructure upgrade is a
/// Core-release-class event, forever") is exactly what these two banks measure.
/// A literal V1-only-consumer reproduction is no longer runnable at HEAD --
/// `authenticate_profile` reads V2 and nothing else (ruling §6, no fallback) --
/// and reverting the consumers to build one would prove less about the code
/// that ships.
#[derive(Clone, Copy, Eq, PartialEq)]
enum RegistryDeploymentV1 {
    /// An IMMUTABLE Registry in the generation its binding pins. This is the
    /// world every other test in this file already runs in, and it is a world
    /// where the brick is structurally unreachable — see
    /// [`RegistryDeploymentV1::upgrade_authority`].
    AsPinned,
    /// A MUTABLE Registry, still in the generation its binding pins. The
    /// control: decision 0012's iteration substrate, admitted while the slot
    /// pin holds.
    MutablePinned,
    /// The same mutable Registry, one upgrade later. The pin breaks.
    Upgraded,
    /// An IMMUTABLE Registry standing in a later generation anyway. No upgrade
    /// could have put it there, so the observation is substituted rather than
    /// superseded, and the route must not offer the re-release remedy.
    ImmutableMoved,
}

impl RegistryDeploymentV1 {
    /// What the Registry's ProgramData reports.
    const fn observed_deployment_slot(self) -> u64 {
        match self {
            Self::AsPinned | Self::MutablePinned => GENESIS_DEPLOYMENT_SLOT,
            Self::Upgraded | Self::ImmutableMoved => UPGRADED_REGISTRY_DEPLOYMENT_SLOT,
        }
    }

    /// The Registry's Loader upgrade authority in this world.
    ///
    /// `None` is an immutable Registry, and it is why the brick pair below is
    /// NOT simply the default world with a moved slot. A program with no
    /// upgrade authority cannot be upgraded, and the route refuses to call a
    /// moved slot under one an upgrade: `require_pinned_deployment` names a
    /// later generation `ReleaseSuperseded` only when the observed authority is
    /// still the one the release bound, and otherwise refuses the generic
    /// `Infrastructure` — a SUBSTITUTED ProgramData is not a supersession, and
    /// the refusal an operator reads must not promise a re-release remedy for a
    /// world where nothing was released. Measured here, not assumed: with an
    /// immutable Registry this pair refuses `Infrastructure` (0x300f) and the
    /// brick's own narrative would have been mis-attributed.
    ///
    /// So P-008's brick is a property of decision 0012's MUTABLE iteration
    /// substrate — which is exactly what devnet runs, and exactly why the
    /// Registry was upgradable enough to brick the protocol in the first place.
    const fn upgrade_authority(self) -> Option<Pubkey> {
        match self {
            Self::AsPinned | Self::ImmutableMoved => None,
            Self::MutablePinned | Self::Upgraded => Some(REGISTRY_UPGRADE_AUTHORITY),
        }
    }

    /// The slot the bank must run at, or `None` to leave it where
    /// `ProgramTest` starts it.
    ///
    /// A Loader V3 program is visible from `deployment_slot + 1`, and the
    /// deployment slot must be an ancestor of the executing slot.
    const fn bank_slot(self) -> Option<u64> {
        match self {
            Self::AsPinned | Self::MutablePinned => None,
            Self::Upgraded | Self::ImmutableMoved => Some(UPGRADED_REGISTRY_DEPLOYMENT_SLOT + 1),
        }
    }
}

/// Which infrastructure profile stands in the fixture's world.
///
/// `Succeeded` is the world after the succession ceremony: the V2 profile at
/// the V2 PDA, which is the only profile any redeployed consumer reads.
/// `PredecessorOnly` is the world BETWEEN the Registry upgrade and the
/// ceremony -- the predecessor still written at its own address, the
/// succession address still vacant. That window is structural (a first
/// admission can only hash deployed bytes, so the ceremony cannot precede the
/// upgrade), and every profile-reading route must refuse inside it by name.
#[derive(Clone, Copy, Eq, PartialEq)]
enum SuccessionStateV1 {
    Succeeded,
    PredecessorOnly,
}

fn fixture(core_mutable: bool) -> Fixture {
    fixture_with(
        core_mutable,
        false,
        SuccessionStateV1::Succeeded,
        RegistryDeploymentV1::AsPinned,
    )
}

/// The same fixture, over a degree-2 curved basis and its certificate.
fn curved_fixture() -> Fixture {
    fixture_with(
        false,
        true,
        SuccessionStateV1::Succeeded,
        RegistryDeploymentV1::AsPinned,
    )
}

fn fixture_with(
    core_mutable: bool,
    curved: bool,
    succession: SuccessionStateV1,
    registry_deployment: RegistryDeploymentV1,
) -> Fixture {
    let artifacts = artifacts();
    let mutable_authority = core_mutable.then(|| Pubkey::new_from_array([0xd1; 32]));
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    // Every program but the Registry stays in the genesis generation, so the
    // bank never holds more than ONE nonzero deployment generation -- the
    // constraint `solana-program-test` enforces by reloading the first
    // generation forever if a second is staged (`direct-hot/src/waist.rs`, on
    // `observed_deployment_slot`). The Registry is the only program this
    // campaign upgrades, so it is the only one that needs the other generation.
    add_program(
        &mut test,
        "dclutch_core_sbf",
        CORE_PROGRAM_ID,
        &artifacts.core,
        mutable_authority,
        GENESIS_DEPLOYMENT_SLOT,
    );
    add_program(
        &mut test,
        "dclutch_registry_sbf",
        REGISTRY_PROGRAM_ID,
        &artifacts.registry,
        registry_deployment.upgrade_authority(),
        registry_deployment.observed_deployment_slot(),
    );
    add_program(
        &mut test,
        "dclutch_rent_sbf",
        RENT_PROGRAM_ID,
        &artifacts.rent,
        None,
        GENESIS_DEPLOYMENT_SLOT,
    );
    add_program(
        &mut test,
        "dclutch_series_consume_caller_sbf",
        TRADING_PROGRAM_ID,
        &artifacts.trading,
        None,
        GENESIS_DEPLOYMENT_SLOT,
    );
    add_program(
        &mut test,
        "dclutch_claims_sbf",
        CLAIMS_PROGRAM_ID,
        &artifacts.claims,
        None,
        GENESIS_DEPLOYMENT_SLOT,
    );
    add_program(
        &mut test,
        "dclutch_custody_sbf",
        CUSTODY_PROGRAM_ID,
        &artifacts.custody,
        None,
        GENESIS_DEPLOYMENT_SLOT,
    );
    add_program(
        &mut test,
        "dclutch_resolution_proof_sbf",
        RESOLUTION_PROGRAM_ID,
        &artifacts.resolution,
        None,
        GENESIS_DEPLOYMENT_SLOT,
    );
    // The Clock the runtime serves has to agree with the bank the fixture will
    // warp to, or the upgraded generation is staged at a slot the executing
    // bank does not descend from.
    if let Some(slot) = registry_deployment.bank_slot() {
        test.add_sysvar_account(
            sysvar::clock::ID,
            &solana_program::clock::Clock {
                slot,
                ..solana_program::clock::Clock::default()
            },
        );
    }
    let core_release = release(CORE_PROGRAM_ID, &artifacts.core, 0xa0, mutable_authority);
    // Pinned to the GENESIS generation in every world -- the release is what the
    // sealed profile names, and it never moves. Only ProgramData does.
    let registry_release = release(
        REGISTRY_PROGRAM_ID,
        &artifacts.registry,
        0xa1,
        registry_deployment.upgrade_authority(),
    );
    let rent_release = release(RENT_PROGRAM_ID, &artifacts.rent, 0xa2, None);
    let trading_release = release(TRADING_PROGRAM_ID, &artifacts.trading, 0xa3, None);
    let claims_release = release(CLAIMS_PROGRAM_ID, &artifacts.claims, 0xa4, None);
    let custody_release = release(CUSTODY_PROGRAM_ID, &artifacts.custody, 0xa5, None);
    let resolution_release = release(RESOLUTION_PROGRAM_ID, &artifacts.resolution, 0xa6, None);
    let core_binding = binding(core_release);
    let release_set_value = ExecutionReleaseSetV1::new(
        core_binding,
        binding(claims_release),
        binding(trading_release),
        binding(resolution_release),
        binding(custody_release),
    )
    .expect("release set");
    let release_set_id =
        CoreContentId::new(hash(&release_set_value.to_bytes()).to_bytes()).expect("release set ID");
    let mut cache_data = vec![0; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut cache_data, release_set_id).expect("cache");
    for (role, selected_release) in [
        (ExecutionRoleV1::Core, core_release),
        (ExecutionRoleV1::Claims, claims_release),
        (ExecutionRoleV1::Trading, trading_release),
        (ExecutionRoleV1::Resolution, resolution_release),
        (ExecutionRoleV1::Custody, custody_release),
    ] {
        activate_execution_role_into_v1(
            &mut cache_data,
            release_set_id,
            &release_set_value,
            role,
            &activation_input(selected_release),
        )
        .expect("activate");
    }
    let cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, release_set_id.as_bytes()],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    test.add_account(
        cache,
        Account {
            lamports: Rent::default().minimum_balance(cache_data.len()),
            data: cache_data,
            owner: REGISTRY_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let (product, domain, portfolio, linked_basis, price_gate, outcome_count, stable_product_id) =
        if curved {
            let (product, domain, portfolio, basis, gate, outcomes, id) = curved_product_graph();
            (product, domain, portfolio, basis, Some(gate), outcomes, id)
        } else {
            let (product, domain, portfolio, basis, outcomes, id) = product_graph();
            (product, domain, portfolio, basis, None, outcomes, id)
        };
    let realm_value = RealmV1::new(RealmV1Input {
        token_program: dclutch_custody::token_svm::LEGACY_TOKEN_PROGRAM_ID,
        collateral_mint: [0xb2; 32],
        collateral_adapter_release_id: [0xb3; 32],
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    })
    .expect("Realm");
    let realm = Record::new(REALM_SCHEMA_RELEASE_ID_V1, realm_value.to_bytes().to_vec());
    let capacity_profile = SourceCapacityProfileV1::new(
        CapacityEnvelope::Provisional,
        1,
        0,
        source_id(0xd5),
        source_id(0xd6),
        208,
        0,
    )
    .expect("Source capacity profile")
    .bounding_principal(1, 4)
    .expect("bounded principal ratio");
    let capacity_profile = Record::new(
        SOURCE_CAPACITY_PROFILE_SCHEMA_ID_V1,
        capacity_profile.to_bytes().to_vec(),
    );
    let adapter_config_id = source_id(0xda);
    let source_spec_value = SourceSpecV1::new(
        source_id(0xd7),
        source_id(0xd8),
        source_id(0xd9),
        SourceAccessProfile::RelayedObservationRecord,
        adapter_config_id,
        SourceContentId::new(capacity_profile.digest).expect("capacity identity"),
    );
    let source_spec = Record::new(
        SOURCE_SPEC_SCHEMA_ID_V1,
        source_spec_value.to_bytes().to_vec(),
    );
    // κ=1/4 over a 20,000-atom venue floor admits exactly 5,000 complete
    // sets at this fixture's unit basis. Series consumes that full bound.
    let manipulation_floor_value = ManipulationFloorV1::new(
        ManipulationFloorBasis::ObservedDepth,
        SourceContentId::new(source_spec.digest).expect("SourceSpec identity"),
        adapter_config_id,
        SourceContentId::new(COLLATERAL_MINT.to_bytes()).expect("collateral unit"),
        source_id(0xdb),
        20_000,
    );
    let manipulation_floor = Record::new(
        MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1,
        manipulation_floor_value.to_bytes().to_vec(),
    );
    let source_value = SourceMaterialV3::bounded_by_floor(
        SourceContentId::new(product.digest).expect("Product root"),
        SourceContentId::new(source_spec.digest).expect("SourceSpec identity"),
        source_id(0xb5),
        source_id(0xb6),
        None,
        source_id(0xb7),
        SourceContentId::new(manipulation_floor.digest).expect("manipulation floor identity"),
    );
    let source = Record::new(
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        source_value.to_bytes().to_vec(),
    );
    let manifest = funded_manifest_record();
    let release_set = Record::new(
        EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1,
        release_set_value.to_bytes().to_vec(),
    );
    let registry_artifact = Record::new(
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        registry_release.to_bytes().to_vec(),
    );
    let rent_artifact = Record::new(
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        rent_release.to_bytes().to_vec(),
    );
    // The same Registry ELF, re-released against the generation the upgrade
    // produced. Everything about it is honest; it is the escape the P-008
    // narrative has to kill.
    let republished_registry_artifact = Record::new(
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        release_at_slot(
            REGISTRY_PROGRAM_ID,
            &artifacts.registry,
            0xa1,
            registry_deployment.upgrade_authority(),
            UPGRADED_REGISTRY_DEPLOYMENT_SLOT,
        )
        .to_bytes()
        .to_vec(),
    );
    if let Some(certificate) = price_gate.as_ref() {
        certificate.add(&mut test);
    }
    for record in [
        &realm,
        &product,
        &domain,
        &portfolio,
        &linked_basis,
        &source,
        &source_spec,
        &capacity_profile,
        &manipulation_floor,
        &manifest,
        &release_set,
        &registry_artifact,
        &rent_artifact,
        &republished_registry_artifact,
    ] {
        record.add(&mut test);
    }

    let payer = fixture_keypair(ROLE_PAYER);
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
    let market_identity = MarketIdentity {
        market_id: identity([0xff; 32]),
        realm_id: identity(realm.digest),
        product_record: identity(product.digest),
        product_id: identity(stable_product_id),
        resolution_policy: identity(source.digest),
        capability_manifest: identity(manifest.digest),
        selected_release_set: identity(release_set.digest),
        registry_program: identity(REGISTRY_PROGRAM_ID.to_bytes()),
        generation: GENERATION,
    };
    let market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(market_identity).as_slices(),
        &CORE_PROGRAM_ID,
    )
    .0;
    let refund = RefundAuthority::new(payer.pubkey().to_bytes()).expect("refund");
    let (rent_credit, rent_bump) = Pubkey::find_program_address(
        &[
            LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
            market.as_ref(),
            &GENERATION.to_le_bytes(),
        ],
        &RENT_PROGRAM_ID,
    );
    let rent_credit_data = LifecycleRentCreditV2::new(
        refund,
        LifecycleAccountIdV2::new(market.to_bytes()).expect("Market"),
        LifecycleAccountIdV2::new(release_set.digest).expect("release set"),
        GENERATION,
        rent_bump,
    )
    .expect("lifecycle RentCredit")
    .to_bytes()
    .to_vec();
    test.add_account(
        rent_credit,
        Account {
            lamports: Rent::default().minimum_balance(rent_credit_data.len()),
            data: rent_credit_data,
            owner: RENT_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    test.add_account(
        market,
        Account {
            lamports: Rent::default().minimum_balance(STATE_BYTES),
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    // The succession profile every redeployed consumer reads (ruling section 6:
    // V2 only, no fallback). The predecessor ids carry the cohort-9 shape the
    // ceremony records -- the Registry moved, so its predecessor is the distinct
    // release this profile succeeded, while Rent stayed byte-identical and holds
    // the same id on both sides of the succession.
    let predecessor_registry_release =
        release(REGISTRY_PROGRAM_ID, &artifacts.registry, 0xb1, None);
    let profile_value = ProtocolInfrastructureProfileV2::new(
        binding(registry_release),
        binding(rent_release),
        artifact_id(predecessor_registry_release),
        artifact_id(rent_release),
    )
    .expect("infrastructure succession profile");
    // The predecessor selection, as it stood before the Registry moved: the same
    // two programs, bound to the registry release the succession replaces. It is
    // planted in EVERY world, because the succession never touches it -- it stays
    // written at its own address forever, perfectly decodable, and never again an
    // authority.
    let predecessor_value = ProtocolInfrastructureProfileV1::new(
        binding(predecessor_registry_release),
        binding(rent_release),
    )
    .expect("predecessor infrastructure profile");
    let predecessor_profile = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
        &CORE_PROGRAM_ID,
    )
    .0;
    let predecessor_data = predecessor_value.to_bytes().to_vec();
    test.add_account(
        predecessor_profile,
        Account {
            lamports: Rent::default().minimum_balance(predecessor_data.len()),
            data: predecessor_data,
            owner: CORE_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    // The one address every redeployed consumer derives, and the only profile
    // any of them reads. Vacant until the ceremony creates it.
    let profile = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
        &CORE_PROGRAM_ID,
    )
    .0;
    let profile_data = profile_value.to_bytes().to_vec();
    if succession == SuccessionStateV1::Succeeded {
        test.add_account(
            profile,
            Account {
                lamports: Rent::default().minimum_balance(profile_data.len()),
                data: profile_data.clone(),
                owner: CORE_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        );
    }
    Fixture {
        test: Some(test),
        payer,
        market,
        rent_credit,
        realm,
        product,
        domain,
        portfolio,
        linked_basis,
        price_gate,
        source,
        source_spec,
        capacity_profile,
        manipulation_floor,
        manifest,
        release_set,
        cache,
        predecessor_profile,
        core_programdata: programdata_address(CORE_PROGRAM_ID),
        trading_programdata: programdata_address(TRADING_PROGRAM_ID),
        claims_programdata: programdata_address(CLAIMS_PROGRAM_ID),
        custody_programdata: programdata_address(CUSTODY_PROGRAM_ID),
        resolution_programdata: programdata_address(RESOLUTION_PROGRAM_ID),
        registry_programdata: programdata_address(REGISTRY_PROGRAM_ID),
        rent_programdata: programdata_address(RENT_PROGRAM_ID),
        profile,
        profile_data,
        registry_artifact,
        rent_artifact,
        republished_registry_artifact,
        bank_slot: registry_deployment.bank_slot(),
        outcome_count,
    }
}

fn found_instruction(fixture: &Fixture, swap_artifacts: bool) -> Instruction {
    let (registry_raw, registry_staging, rent_raw, rent_staging) = if swap_artifacts {
        (
            fixture.rent_artifact.raw,
            fixture.rent_artifact.staging,
            fixture.registry_artifact.raw,
            fixture.registry_artifact.staging,
        )
    } else {
        (
            fixture.registry_artifact.raw,
            fixture.registry_artifact.staging,
            fixture.rent_artifact.raw,
            fixture.rent_artifact.staging,
        )
    };
    Instruction {
        program_id: CORE_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(fixture.payer.pubkey(), true),
            AccountMeta::new(fixture.market, false),
            AccountMeta::new_readonly(fixture.rent_credit, false),
            AccountMeta::new_readonly(RENT_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.realm.raw, false),
            AccountMeta::new_readonly(fixture.realm.staging, false),
            AccountMeta::new_readonly(fixture.product.raw, false),
            AccountMeta::new_readonly(fixture.product.staging, false),
            AccountMeta::new_readonly(fixture.domain.raw, false),
            AccountMeta::new_readonly(fixture.domain.staging, false),
            AccountMeta::new_readonly(fixture.portfolio.raw, false),
            AccountMeta::new_readonly(fixture.portfolio.staging, false),
            AccountMeta::new_readonly(fixture.linked_basis.raw, false),
            AccountMeta::new_readonly(fixture.linked_basis.staging, false),
            AccountMeta::new_readonly(fixture.source.raw, false),
            AccountMeta::new_readonly(fixture.source.staging, false),
            AccountMeta::new_readonly(fixture.source_spec.raw, false),
            AccountMeta::new_readonly(fixture.source_spec.staging, false),
            AccountMeta::new_readonly(fixture.capacity_profile.raw, false),
            AccountMeta::new_readonly(fixture.capacity_profile.staging, false),
            AccountMeta::new_readonly(fixture.manipulation_floor.raw, false),
            AccountMeta::new_readonly(fixture.manipulation_floor.staging, false),
            AccountMeta::new_readonly(fixture.manifest.raw, false),
            AccountMeta::new_readonly(fixture.manifest.staging, false),
            AccountMeta::new_readonly(fixture.cache, false),
            AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.core_programdata, false),
            AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(fixture.profile, false),
            AccountMeta::new_readonly(registry_raw, false),
            AccountMeta::new_readonly(registry_staging, false),
            AccountMeta::new_readonly(fixture.registry_programdata, false),
            AccountMeta::new_readonly(rent_raw, false),
            AccountMeta::new_readonly(rent_staging, false),
            AccountMeta::new_readonly(fixture.rent_programdata, false),
        ]
        .into_iter()
        // **Appended last, and only when the basis needs one.** The canonical
        // 37-account frame is byte-for-byte what it always was; a curved basis
        // makes it 39. Nothing before the pair moves.
        .chain(fixture.price_gate.iter().flat_map(|certificate| {
            [
                AccountMeta::new_readonly(certificate.raw, false),
                AccountMeta::new_readonly(certificate.staging, false),
            ]
        }))
        .collect(),
        data: Request::administrative(
            Action::Found,
            GENERATION,
            identity(fixture.market.to_bytes()),
        )
        .encode()
        .expect("Found request")
        .to_vec(),
    }
}

fn project_found_instruction(fixture: &Fixture, swap_artifacts: bool) -> Instruction {
    let mut instruction = found_instruction(fixture, swap_artifacts);
    *instruction.accounts.first_mut().expect("payer") =
        AccountMeta::new_readonly(fixture.payer.pubkey(), false);
    *instruction.accounts.get_mut(1).expect("Market") =
        AccountMeta::new_readonly(fixture.market, false);
    instruction
        .accounts
        .remove(dclutch_market::FOUND_RENT_SYSVAR_INDEX_V3);
    assert_eq!(
        instruction.accounts.len(),
        dclutch_market::PROJECT_FOUND_ACCOUNT_COUNT_V2
    );
    let found = Request::administrative(
        Action::Found,
        GENERATION,
        identity(fixture.market.to_bytes()),
    );
    instruction.data = ProjectFoundRequestV2::new(found)
        .expect("ProjectFound")
        .encode()
        .expect("ProjectFound bytes")
        .to_vec();
    instruction
}

/// Install one canonical lookup table for the complete instruction frame.
///
/// The fee payer and instruction signers remain static. Every other key,
/// including the invoked program, is sorted by raw public-key bytes and
/// deduplicated before the message compiler chooses its writable/readonly
/// indexes. The bank therefore resolves the same table the packet names.
fn add_instruction_lookup(
    test: &mut ProgramTest,
    instructions: &[Instruction],
) -> AddressLookupTableAccount {
    let mut addresses = instructions
        .iter()
        .flat_map(|instruction| {
            core::iter::once(instruction.program_id).chain(
                instruction
                    .accounts
                    .iter()
                    .filter(|meta| !meta.is_signer)
                    .map(|meta| meta.pubkey),
            )
        })
        .filter(|address| *address != LOOKUP_TABLE)
        .collect::<Vec<_>>();
    addresses.sort_unstable_by_key(Pubkey::to_bytes);
    addresses.dedup();
    let data = AddressLookupTable {
        meta: LookupTableMeta::default(),
        addresses: addresses.as_slice().into(),
    }
    .serialize_for_tests()
    .expect("lookup-table bytes");
    test.add_account(
        LOOKUP_TABLE,
        Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner: lookup_table_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
    AddressLookupTableAccount {
        key: LOOKUP_TABLE,
        addresses,
    }
}

fn signed_v0(
    payer: &Pubkey,
    instructions: &[Instruction],
    lookup: &AddressLookupTableAccount,
    blockhash: Hash,
    signers: &[&Keypair],
) -> VersionedTransaction {
    let message = VersionedMessage::V0(
        v0::Message::try_compile(
            payer,
            instructions,
            core::slice::from_ref(lookup),
            blockhash,
        )
        .expect("canonical v0 message"),
    );
    let transaction =
        VersionedTransaction::try_new(message, signers).expect("signed v0 transaction");
    wire_extent(
        transaction.signatures.len(),
        &transaction.message.serialize(),
    );
    transaction
}

async fn execute(
    mut fixture: Fixture,
    instruction: Instruction,
) -> (Fixture, solana_program_test::ProgramTestContext, bool) {
    let mut test = fixture.test.take().expect("ProgramTest");
    let lookup = add_instruction_lookup(&mut test, core::slice::from_ref(&instruction));
    let mut context = test.start_with_context().await;
    warp_to_deployment_generation(&mut context, &fixture);
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = signed_v0(
        &context.payer.pubkey(),
        &[instruction],
        &lookup,
        blockhash,
        &[&context.payer, &fixture.payer],
    );
    let accepted = context
        .banks_client
        .process_transaction(transaction)
        .await
        .is_ok();
    (fixture, context, accepted)
}

/// Put the bank one slot past this world's deployment generation.
///
/// `ProgramTest` starts at slot 1, where ProgramData reporting a later slot
/// makes the program invisible and the runtime reports a program-cache
/// replacement rather than anything about the deployment. Worlds that never
/// moved a generation are left exactly where they started, so this is inert for
/// every test that predates the upgrade fixture.
fn warp_to_deployment_generation(
    context: &mut solana_program_test::ProgramTestContext,
    fixture: &Fixture,
) {
    if let Some(slot) = fixture.bank_slot {
        context
            .warp_to_slot(slot)
            .expect("warp the bank one slot past the deployment generation");
    }
}

/// Submit a Found frame and render any refusal, for hostiles that must name one.
///
/// [`execute`] answers only whether the bank accepted, which cannot tell a
/// refusal at the conjunct under test from one three checks earlier -- and a
/// hostile that asserts only "not accepted" is a test of nothing.
async fn execute_reporting_failure(
    mut fixture: Fixture,
    instruction: Instruction,
) -> (
    Fixture,
    solana_program_test::ProgramTestContext,
    Option<String>,
) {
    let mut test = fixture.test.take().expect("ProgramTest");
    let lookup = add_instruction_lookup(&mut test, core::slice::from_ref(&instruction));
    let mut context = test.start_with_context().await;
    warp_to_deployment_generation(&mut context, &fixture);
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = signed_v0(
        &context.payer.pubkey(),
        &[instruction],
        &lookup,
        blockhash,
        &[&context.payer, &fixture.payer],
    );
    let failure = context
        .banks_client
        .process_transaction(transaction)
        .await
        .err()
        .map(|error| format!("{error:?}"));
    (fixture, context, failure)
}

async fn execute_project(
    mut fixture: Fixture,
    instruction: Instruction,
) -> (Fixture, solana_program_test::ProgramTestContext, bool) {
    let mut test = fixture.test.take().expect("ProgramTest");
    let lookup = add_instruction_lookup(&mut test, core::slice::from_ref(&instruction));
    let context = test.start_with_context().await;
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = signed_v0(
        &context.payer.pubkey(),
        &[instruction],
        &lookup,
        blockhash,
        &[&context.payer],
    );
    let accepted = context
        .banks_client
        .process_transaction(transaction)
        .await
        .is_ok();
    (fixture, context, accepted)
}

fn series_instruction(fixture: &SeriesFixture) -> Instruction {
    let mut accounts = found_instruction(&fixture.base, false).accounts;
    *accounts.first_mut().expect("caller meta") = AccountMeta::new(fixture.caller_authority, false);
    accounts.extend_from_slice(&[
        AccountMeta::new_readonly(TRADING_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.base.trading_programdata, false),
        AccountMeta::new_readonly(fixture.root, false),
        AccountMeta::new_readonly(fixture.ticket_state, false),
        AccountMeta::new_readonly(fixture.template.raw, false),
        AccountMeta::new_readonly(fixture.template.staging, false),
        AccountMeta::new_readonly(fixture.occurrence.raw, false),
        AccountMeta::new_readonly(fixture.occurrence.staging, false),
        AccountMeta::new_readonly(fixture.ticket.raw, false),
        AccountMeta::new_readonly(fixture.ticket.staging, false),
        AccountMeta::new_readonly(sysvar::clock::ID, false),
        AccountMeta::new_readonly(fixture.funding, false),
        AccountMeta::new(fixture.permit, false),
        AccountMeta::new_readonly(fixture.projected_replay, false),
        AccountMeta::new_readonly(fixture.hoard, false),
        AccountMeta::new_readonly(fixture.funding_source, false),
        AccountMeta::new_readonly(fixture.funding_source_replay, false),
        AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.claims_programdata_meta, false),
        AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.base.custody_programdata, false),
        AccountMeta::new_readonly(fixture.aggregate, false),
        AccountMeta::new_readonly(fixture.position, false),
        AccountMeta::new_readonly(fixture.admission, false),
        AccountMeta::new_readonly(fixture.base.payer.pubkey(), false),
    ]);
    let mut data = fixture.request.to_vec();
    data.extend_from_slice(&fixture.lock_receipt);
    let fixed = accounts
        .get(..dclutch_core_sbf::SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V1)
        .expect("complete Series fixed frame");
    for (left_index, left) in fixed.iter().enumerate() {
        for (right_index, right) in fixed.iter().enumerate().skip(left_index + 1) {
            assert_ne!(
                left.pubkey, right.pubkey,
                "Series fixed-frame alias at {left_index}/{right_index}"
            );
        }
        for (right_index, right) in accounts.iter().enumerate() {
            if left.pubkey == right.pubkey {
                assert!(
                    !right.is_signer || left.is_signer,
                    "Series privilege union makes fixed account {left_index} signer at {right_index}"
                );
                assert!(
                    !right.is_writable || left.is_writable,
                    "Series privilege union makes fixed account {left_index} writable at {right_index}"
                );
            }
        }
    }
    Instruction {
        program_id: TRADING_PROGRAM_ID,
        accounts,
        data,
    }
}

/// Which infrastructure account the permit-expiry frame presents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExpiryProfileV1 {
    /// The post-ceremony V2 profile at the V2 PDA.
    Successor,
    /// Canonical V2 bytes, Core-owned and rent-exempt, but at another address.
    WrongAddress,
    /// The still-present, still-decodable V1 predecessor profile.
    SealedPredecessor,
}

impl ExpiryProfileV1 {
    fn address(self, fixture: &SeriesFixture) -> Pubkey {
        match self {
            Self::Successor => fixture.base.profile,
            Self::WrongAddress => fixture_pubkey(ROLE_WRONG_INFRASTRUCTURE_PROFILE),
            Self::SealedPredecessor => fixture.base.predecessor_profile,
        }
    }
}

/// Put the deterministic Series fixture in exactly the replay state expiry reads.
///
/// The immutable records and the permit candidate are unchanged. Only the two
/// Trading-owned mutable replay accounts advance through their public kernels:
/// the root settles occurrence zero, and the ticket settles as Expired. This is
/// the state the real Series expiry path leaves before Core reclaims an
/// unallocated permit.
fn series_expiry_fixture(
    permit: SeriesFoundingPermitV1,
    profile: ExpiryProfileV1,
) -> SeriesFixture {
    let mut fixture = series_fixture(SeriesFault::None);
    let admitted = admit_occurrence_bytes(&fixture.template.data, &fixture.occurrence.data, &[])
        .expect("expiry occurrence");
    let admitted_ticket = admit_ticket(&fixture.ticket.data).expect("expiry Ticket");
    let retry_through = admitted
        .template()
        .retry_through(admitted.occurrence().occurrence())
        .expect("retry deadline");
    let intent = permit.intent();
    assert_eq!(
        intent.trading_program().to_bytes(),
        TRADING_PROGRAM_ID.to_bytes()
    );
    assert_eq!(intent.parent_root().to_bytes(), fixture.root.to_bytes());
    assert_eq!(
        intent.release_set().to_bytes(),
        admitted.template().release_set().to_bytes(),
        "permit/template release set"
    );
    assert_eq!(
        intent.market().to_bytes(),
        admitted.occurrence().market().to_bytes(),
        "permit/occurrence market"
    );
    assert_eq!(
        intent.product_record().to_bytes(),
        admitted.occurrence().product_record().to_bytes(),
        "permit/occurrence Product"
    );
    assert_eq!(
        intent.founder().to_bytes(),
        admitted_ticket.ticket().founder().to_bytes(),
        "permit/Ticket founder"
    );
    assert_eq!(
        intent.ticket_context().to_bytes(),
        admitted_ticket.content_id().to_bytes(),
        "permit/Ticket identity"
    );
    assert_eq!(
        intent.generation(),
        u64::from(admitted.occurrence().occurrence()) + 1,
        "permit occurrence generation"
    );
    assert_eq!(intent.expiry_slot(), retry_through, "permit retry deadline");
    assert_eq!(
        intent.rent_credit().to_bytes(),
        fixture.base.rent_credit.to_bytes(),
        "permit RentCredit"
    );
    let test = fixture.base.test.as_mut().expect("ProgramTest");
    let rent = Rent::default();

    let prepared_series = SeriesStateV3::decode(
        fixture
            .root_data
            .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .expect("Series root tail"),
        1,
    )
    .expect("prepared Series state");
    let terminal_series = prepared_series
        .settle_current(prepared_series.revision(), 1)
        .expect("terminal Series state")
        .encode(1)
        .expect("terminal Series bytes");
    fixture
        .root_data
        .get_mut(CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .expect("Series root tail")
        .copy_from_slice(&terminal_series);
    test.add_account(
        fixture.root,
        Account {
            lamports: rent.minimum_balance(fixture.root_data.len()),
            data: fixture.root_data.clone(),
            owner: TRADING_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let prepared_ticket =
        TicketStateV3::decode(&fixture.ticket_state_data).expect("prepared Ticket state");
    fixture.ticket_state_data = prepared_ticket
        .settle(prepared_ticket.revision(), TicketPhaseV3::Expired)
        .expect("expired Ticket state")
        .encode()
        .to_vec();
    let header = CapabilityRootHeaderV1::decode(
        fixture
            .root_data
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .expect("root header"),
    )
    .expect("root header");
    assert_eq!(
        Pubkey::find_program_address(&header.seeds().as_slices(), &TRADING_PROGRAM_ID).0,
        fixture.root,
        "Series root PDA"
    );
    assert_eq!(
        header.release_set().to_bytes(),
        admitted.template().release_set().to_bytes(),
        "root/template release set"
    );
    assert_eq!(
        header.selection().config().to_bytes(),
        admitted.template_id().to_bytes(),
        "root/template selector"
    );
    let terminal_series = SeriesStateV3::decode(
        fixture
            .root_data
            .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .expect("terminal Series tail"),
        1,
    )
    .expect("terminal Series state");
    assert_eq!(terminal_series.next_occurrence(), 1);
    assert!(!terminal_series.current_ticket_prepared());
    let expired_ticket =
        TicketStateV3::decode(&fixture.ticket_state_data).expect("expired Ticket state");
    let ticket_seeds =
        TicketStateSeedsV3::new(fixture.root.to_bytes(), admitted_ticket.content_id());
    assert_eq!(
        Pubkey::find_program_address(&ticket_seeds.as_slices(), &TRADING_PROGRAM_ID).0,
        fixture.ticket_state,
        "Ticket-state PDA"
    );
    assert_eq!(
        expired_ticket.ticket_record_id(),
        admitted_ticket.content_id(),
        "Ticket-state record identity"
    );
    assert_eq!(expired_ticket.phase(), TicketPhaseV3::Expired);
    test.add_account(
        fixture.ticket_state,
        Account {
            lamports: rent.minimum_balance(fixture.ticket_state_data.len()),
            data: fixture.ticket_state_data.clone(),
            owner: TRADING_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let expiry_slot = permit.intent().expiry_slot();
    let bank_slot = expiry_slot
        .checked_add(1)
        .expect("slot after permit expiry");
    fixture.base.bank_slot = Some(bank_slot);
    test.add_sysvar_account(
        sysvar::clock::ID,
        &Clock {
            slot: bank_slot,
            ..Clock::default()
        },
    );

    if profile == ExpiryProfileV1::WrongAddress {
        // Keep every account fact except the address identical to the accepting
        // V2 profile. A refusal here therefore owns the PDA/domain conjunct,
        // not a malformed body, owner, width, or rent balance.
        test.add_account(
            fixture_pubkey(ROLE_WRONG_INFRASTRUCTURE_PROFILE),
            Account {
                lamports: rent.minimum_balance(fixture.base.profile_data.len()),
                data: fixture.base.profile_data.clone(),
                owner: CORE_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        );
    }

    fixture
}

/// Compose the shipped 25-account permit-expiry frame in its exact order.
fn series_expiry_instruction(
    fixture: &SeriesFixture,
    permit: SeriesFoundingPermitV1,
    profile: ExpiryProfileV1,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(fixture.permit, false),
        AccountMeta::new(fixture.base.rent_credit, false),
        AccountMeta::new_readonly(RENT_PROGRAM_ID, false),
        AccountMeta::new_readonly(profile.address(fixture), false),
        AccountMeta::new_readonly(fixture.base.registry_artifact.raw, false),
        AccountMeta::new_readonly(fixture.base.registry_artifact.staging, false),
        AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.base.registry_programdata, false),
        AccountMeta::new_readonly(fixture.base.rent_artifact.raw, false),
        AccountMeta::new_readonly(fixture.base.rent_artifact.staging, false),
        AccountMeta::new_readonly(fixture.base.rent_programdata, false),
        AccountMeta::new_readonly(fixture.base.cache, false),
        AccountMeta::new_readonly(TRADING_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.base.trading_programdata, false),
        AccountMeta::new_readonly(fixture.root, false),
        AccountMeta::new_readonly(fixture.ticket_state, false),
        AccountMeta::new_readonly(fixture.template.raw, false),
        AccountMeta::new_readonly(fixture.template.staging, false),
        AccountMeta::new_readonly(fixture.occurrence.raw, false),
        AccountMeta::new_readonly(fixture.occurrence.staging, false),
        AccountMeta::new_readonly(fixture.ticket.raw, false),
        AccountMeta::new_readonly(fixture.ticket.staging, false),
        AccountMeta::new_readonly(sysvar::clock::ID, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
    ];
    assert_eq!(
        accounts.len(),
        dclutch_core_sbf::SERIES_PERMIT_EXPIRY_ACCOUNT_COUNT_V1,
        "the driver must cover the complete expiry frame"
    );
    for (left_index, left) in accounts.iter().enumerate() {
        for (right_index, right) in accounts.iter().enumerate().skip(left_index + 1) {
            assert_ne!(
                left.pubkey, right.pubkey,
                "Series expiry frame alias at {left_index}/{right_index}"
            );
        }
    }
    Instruction {
        program_id: CORE_PROGRAM_ID,
        accounts,
        data: SeriesPermitExpiryRequestV1::new(permit)
            .encode()
            .expect("Series permit expiry request")
            .to_vec(),
    }
}

/// Ask the compiled Core to author the exact permit the expiry campaign uses.
///
/// This deliberately does not duplicate `build_permit_plan`'s construction in
/// the fixture. The positive Consume route owns those 608 bytes; the expiry
/// route receives what that owner actually persisted.
async fn issued_series_permit() -> SeriesFoundingPermitV1 {
    let fixture = series_fixture(SeriesFault::None);
    let permit_address = fixture.permit;
    let (fixture, context, failure) = execute_series(
        fixture,
        "Series Consume authors the permit-expiry campaign input",
    )
    .await;
    assert_eq!(failure, None, "the permit author must complete");
    let account = context
        .banks_client
        .get_account(permit_address)
        .await
        .expect("permit query")
        .expect("Core-authored permit");
    assert_eq!(account.owner, CORE_PROGRAM_ID);
    let permit = SeriesFoundingPermitV1::decode(&account.data).expect("Core-authored permit bytes");
    drop(context);
    drop(fixture);
    permit
}

/// Submit one expiry frame and retain the two accounts whose conservation and
/// rollback the campaign asserts.
async fn execute_series_expiry(
    mut fixture: SeriesFixture,
    permit: SeriesFoundingPermitV1,
    profile: ExpiryProfileV1,
) -> (
    SeriesFixture,
    ProgramTestContext,
    Result<(), TransactionError>,
    Account,
    Account,
) {
    let instruction = series_expiry_instruction(&fixture, permit, profile);
    let mut test = fixture.base.test.take().expect("ProgramTest");
    let lookup = add_instruction_lookup(&mut test, core::slice::from_ref(&instruction));
    let mut context = test.start_with_context().await;
    warp_to_deployment_generation(&mut context, &fixture.base);
    let permit_before = context
        .banks_client
        .get_account(fixture.permit)
        .await
        .expect("permit query")
        .expect("prefunded permit");
    let credit_before = context
        .banks_client
        .get_account(fixture.base.rent_credit)
        .await
        .expect("RentCredit query")
        .expect("RentCredit");
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = signed_v0(
        &context.payer.pubkey(),
        &[instruction],
        &lookup,
        blockhash,
        &[&context.payer],
    );
    let result = context
        .banks_client
        .process_transaction(transaction)
        .await
        .map_err(|error| match error {
            BanksClientError::TransactionError(error) => error,
            BanksClientError::SimulationError { err, .. } => err,
            other => panic!("unexpected banks error: {other:?}"),
        });
    (fixture, context, result, permit_before, credit_before)
}

/// Assert the compiled expiry entry refused at the named Core discriminant.
#[track_caller]
fn assert_expiry_refused(
    result: Result<(), TransactionError>,
    expected: dclutch_core_sbf::CoreSbfError,
) {
    assert_eq!(
        result.expect_err("this expiry frame must refuse"),
        TransactionError::InstructionError(0, InstructionError::Custom(expected as u32),),
        "expected {expected:?}"
    );
}

/// Submit one Series occurrence and, if the gauntlet asked, record it.
///
/// `label` is the census binding key. The evidence is written only when
/// `DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR` is set, so this stays an ordinary test
/// under a bare `cargo test`. The submit goes through
/// `process_transaction_with_metadata` rather than `process_transaction`
/// because the census cross-checks every claimed route against the runtime's
/// own `Program <address> invoke [n]` lines, and a producer that cannot
/// surface those cannot be corroborated.
async fn execute_series(
    mut fixture: SeriesFixture,
    label: &str,
) -> (
    SeriesFixture,
    solana_program_test::ProgramTestContext,
    Option<String>,
) {
    let mut test = fixture.base.test.take().expect("ProgramTest");
    let instruction = series_instruction(&fixture);
    let lookup = add_instruction_lookup(&mut test, core::slice::from_ref(&instruction));
    let context = test.start_with_context().await;
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = signed_v0(
        &context.payer.pubkey(),
        &[instruction],
        &lookup,
        blockhash,
        &[&context.payer],
    );
    let failure = submit_and_record(&context, transaction, label).await;
    (fixture, context, failure)
}

/// Assert that a rendered `TransactionError` is exactly the named refusal.
///
/// The rendering is `{:?}` of a `TransactionError`, so the code arrives as
/// `Custom(<decimal>)`. Deriving the token from the enum rather than typing the
/// number keeps this honest through a renumber, and including the closing
/// parenthesis makes it an exact match: the hand-written form was
/// `contains("Custom(3)")`, which also accepts `Custom(30)` and `Custom(300)`.
#[track_caller]
fn assert_refused_with(failure: &str, expected: u32, named: &str) {
    let token = format!("Custom({expected})");
    assert!(
        failure.contains(&token),
        "expected {named} ({token}), got {failure}"
    );
}

/// Solana's legacy transaction packet ceiling.
const PACKET_DATA_BYTES: usize = 1_232;

/// The exact wire extent of one signed transaction.
///
/// One shortvec byte for the signature count, 64 bytes per signature, then the
/// serialised message. This is what a validator would receive.
fn wire_extent(signatures: usize, message: &[u8]) -> usize {
    let extent = 1 + signatures * 64 + message.len();
    assert!(
        extent <= PACKET_DATA_BYTES,
        "the transaction serialises to {extent} bytes, past Solana's \
         {PACKET_DATA_BYTES}-byte packet maximum"
    );
    extent
}

/// Submit one already-built transaction and, if asked, record it as evidence.
async fn submit_and_record(
    context: &solana_program_test::ProgramTestContext,
    transaction: VersionedTransaction,
    label: &str,
) -> Option<String> {
    let signature = transaction
        .signatures
        .first()
        .copied()
        .expect("a signed transaction has a signature")
        .to_string();
    let wire_bytes = wire_extent(
        transaction.signatures.len(),
        &transaction.message.serialize(),
    );
    let outcome = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("the bank processed the transaction");
    let failure = outcome.result.err().map(|error| format!("{error:?}"));
    let slot = context
        .banks_client
        .get_root_slot()
        .await
        .unwrap_or_default();
    let (logs, compute_units) = outcome.metadata.map_or_else(
        || (Vec::new(), None),
        |metadata| (metadata.log_messages, Some(metadata.compute_units_consumed)),
    );
    dclutch_program_test_evidence::record(&dclutch_program_test_evidence::TransactionEvidence {
        label,
        signature: &signature,
        slot,
        error: failure.as_deref(),
        logs: &logs,
        compute_units_consumed: compute_units,
        wire_bytes: Some(wire_bytes),
    })
    .expect("campaign evidence must be writable when the gauntlet asked for it");
    failure
}

/// Consume one Series occurrence, then submit the SAME occurrence again.
///
/// The replay carries a fresh blockhash, so it is a genuinely distinct
/// transaction rather than one the bank rejects as already processed. Anything
/// less would prove nothing about the program: duplicate-signature rejection is
/// the runtime declining to look, not Core declining to act twice on one
/// ticket.
async fn execute_series_twice(
    mut fixture: SeriesFixture,
    first_label: &str,
    replay_label: &str,
) -> (
    SeriesFixture,
    solana_program_test::ProgramTestContext,
    Option<String>,
    Option<String>,
) {
    let mut test = fixture.base.test.take().expect("ProgramTest");
    let instruction = series_instruction(&fixture);
    let lookup = add_instruction_lookup(&mut test, core::slice::from_ref(&instruction));
    let mut context = test.start_with_context().await;
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let first = submit_and_record(
        &context,
        signed_v0(
            &context.payer.pubkey(),
            core::slice::from_ref(&instruction),
            &lookup,
            blockhash,
            &[&context.payer],
        ),
        first_label,
    )
    .await;
    let replay_blockhash = context
        .get_new_latest_blockhash()
        .await
        .expect("a distinct blockhash, so the replay is a distinct transaction");
    assert_ne!(
        replay_blockhash, blockhash,
        "without a new blockhash the bank would refuse the replay as already processed,          which proves nothing about Core"
    );
    let replay = submit_and_record(
        &context,
        signed_v0(
            &context.payer.pubkey(),
            &[instruction],
            &lookup,
            replay_blockhash,
            &[&context.payer],
        ),
        replay_label,
    )
    .await;
    (fixture, context, first, replay)
}

#[tokio::test]
async fn real_found37_accepts_258_outcomes_after_pinned_infrastructure_auth() {
    let fixture = fixture(false);
    let instruction = found_instruction(&fixture, false);
    let (fixture, context, accepted) = execute(fixture, instruction).await;
    assert!(accepted);
    assert_eq!(fixture.outcome_count, 258);
    let market = context
        .banks_client
        .get_account(fixture.market)
        .await
        .expect("Market query")
        .expect("Market");
    assert_eq!(market.owner, CORE_PROGRAM_ID);
    let state = CoreState::decode(&market.data).expect("CoreState");
    assert_eq!(state.phase, Phase::Founding);
    assert_eq!(state.readiness, Readiness::Prepaid);
    assert_eq!(
        state.identity.product_record.to_bytes(),
        fixture.product.digest
    );
    assert_eq!(
        state.identity.registry_program.to_bytes(),
        REGISTRY_PROGRAM_ID.to_bytes()
    );
}

#[tokio::test]
async fn project_found36_authenticates_without_signature_or_market_mutation() {
    let fixture = fixture(false);
    let instruction = project_found_instruction(&fixture, false);
    let payer = instruction.accounts.first().expect("projection payer meta");
    let market = instruction.accounts.get(1).expect("projection Market meta");
    assert!(!payer.is_signer);
    assert!(!payer.is_writable);
    assert!(!market.is_writable);
    let (fixture, context, accepted) = execute_project(fixture, instruction).await;
    assert!(accepted);
    let market = context
        .banks_client
        .get_account(fixture.market)
        .await
        .expect("Market query")
        .expect("vacant Market");
    assert_eq!(market.owner, system_program::ID);
    assert!(market.data.is_empty());
    assert_eq!(
        market.lamports,
        Rent::default().minimum_balance(STATE_BYTES)
    );
}

#[tokio::test]
async fn projected_found36_refuses_swapped_infrastructure_without_market_mutation() {
    let fixture = fixture(false);
    let instruction = project_found_instruction(&fixture, true);
    let (fixture, context, accepted) = execute_project(fixture, instruction).await;
    assert!(!accepted);
    let market = context
        .banks_client
        .get_account(fixture.market)
        .await
        .expect("Market query")
        .expect("vacant Market");
    assert_eq!(market.owner, system_program::ID);
    assert!(market.data.is_empty());
}

#[tokio::test]
async fn superseded_project_found37_frame_refuses_without_market_mutation() {
    let fixture = fixture(false);
    let mut instruction = project_found_instruction(&fixture, false);
    instruction.accounts.insert(
        dclutch_market::FOUND_RENT_SYSVAR_INDEX_V3,
        AccountMeta::new_readonly(sysvar::rent::ID, false),
    );
    assert_eq!(
        instruction.accounts.len(),
        dclutch_market::FOUND_ACCOUNT_COUNT_V3
    );
    let (fixture, context, accepted) = execute_project(fixture, instruction).await;
    assert!(!accepted);
    let market = context
        .banks_client
        .get_account(fixture.market)
        .await
        .expect("Market query")
        .expect("vacant Market");
    assert_eq!(market.owner, system_program::ID);
    assert!(market.data.is_empty());
}

#[tokio::test]
async fn swapped_registry_and_rent_artifacts_refuse_without_market_write() {
    let fixture = fixture(false);
    let instruction = found_instruction(&fixture, true);
    let (fixture, context, accepted) = execute(fixture, instruction).await;
    assert!(!accepted);
    let market = context
        .banks_client
        .get_account(fixture.market)
        .await
        .expect("Market query")
        .expect("vacant Market");
    assert_eq!(market.owner, system_program::ID);
    assert!(market.data.is_empty());
    assert_eq!(
        market.lamports,
        Rent::default().minimum_balance(STATE_BYTES)
    );
}

/// Founding refuses by name in the window before the ceremony.
///
/// The reader's half of the ruling's §8.1 obligation
/// (`docs/design/PROFILE_UPGRADE_RULING_2026_08_31.md`). The window between the
/// Registry upgrade and the succession ceremony is STRUCTURAL -- a first
/// admission can only hash deployed bytes, so the ceremony cannot precede the
/// upgrade -- and inside it founding must refuse rather than quietly proceed
/// against a selection the chain has stopped standing behind. The frame is the
/// honest one a redeployed host builds, aimed at the address every consumer now
/// derives; that address is simply not written yet. Exactly one fact about the
/// world differs from the accepting case, so this cannot pass for an unrelated
/// malformation.
#[tokio::test]
async fn founding_refuses_while_only_the_predecessor_profile_stands() {
    let fixture = fixture_with(
        false,
        false,
        SuccessionStateV1::PredecessorOnly,
        RegistryDeploymentV1::AsPinned,
    );
    let instruction = found_instruction(&fixture, false);
    let (fixture, context, failure) = execute_reporting_failure(fixture, instruction).await;
    let failure = failure.expect("a vacant succession profile must refuse founding");
    assert_refused_with(
        &failure,
        dclutch_core_sbf::CoreSbfError::Infrastructure as u32,
        "CoreSbfError::Infrastructure on a succession profile that does not exist yet",
    );
    // Nothing was created on the way to the refusal.
    let market = context
        .banks_client
        .get_account(fixture.market)
        .await
        .expect("Market query")
        .expect("vacant Market");
    assert_eq!(market.owner, system_program::ID);
    assert!(market.data.is_empty());
}

/// The sealed predecessor cannot be presented as the authority again.
///
/// The other half of "V2 only, no fallback" (ruling §6). Here the succession
/// HAS happened and the world is complete, so nothing is missing and nothing is
/// malformed: the predecessor still stands at its own address, still decodes,
/// and still names the same two programs. A caller aims the frame at it anyway.
/// It is refused for the only reason that matters -- it is not the account this
/// program reads -- which is what makes the predecessor a historical record
/// rather than a second live authentication path.
#[tokio::test]
async fn founding_refuses_a_frame_aimed_at_the_sealed_predecessor_profile() {
    let fixture = fixture(false);
    let mut instruction = found_instruction(&fixture, false);
    let slot = instruction
        .accounts
        .iter()
        .position(|meta| meta.pubkey == fixture.profile)
        .expect("the honest frame carries the succession profile");
    instruction.accounts[slot].pubkey = fixture.predecessor_profile;
    let (fixture, context, failure) = execute_reporting_failure(fixture, instruction).await;
    let failure = failure.expect("the sealed predecessor must never authenticate again");
    assert_refused_with(
        &failure,
        dclutch_core_sbf::CoreSbfError::Infrastructure as u32,
        "CoreSbfError::Infrastructure on the sealed predecessor profile",
    );
    let market = context
        .banks_client
        .get_account(fixture.market)
        .await
        .expect("Market query")
        .expect("vacant Market");
    assert_eq!(market.owner, system_program::ID);
    assert!(market.data.is_empty());
}

/// P-008's brick, reproduced: a Registry upgrade supersedes the sealed
/// selection and founding stops.
///
/// The ruling's §8.1 obligation
/// (`docs/design/PROFILE_UPGRADE_RULING_2026_08_31.md`) — "the measurement that
/// makes everything after it a fix rather than a feature". PROFILE-2 proved the
/// reader's half (a profile that is not written yet refuses); this is the half
/// where nothing is missing and nothing is malformed. The profile stands, it
/// decodes, it names the right two programs, the records are finalized and
/// present, and the Registry it selects is deployed and executable. One fact
/// moved: the generation the Loader reports for the Registry. That alone
/// refuses founding, by name, forever — because the profile is write-once and
/// its selection is pinned BY CONTENT, deployment slot included.
///
/// The pair is the point, and it is why there are two banks. The `AsPinned`
/// bank is the control: the identical frame, the identical ELF bytes, the
/// identical records, founding lands. A campaign that only showed the refusal
/// could not distinguish "the slot pin refuses the release that moved" from
/// "this fixture refuses" — the discrimination `slot_pin_supersession.rs` makes
/// explicit and this pair inherits.
#[tokio::test]
async fn a_registry_upgrade_supersedes_the_pinned_selection_and_bricks_founding() {
    // The pin holds. Same bytes, same records, same frame, same authority.
    let held = fixture_with(
        false,
        false,
        SuccessionStateV1::Succeeded,
        RegistryDeploymentV1::MutablePinned,
    );
    let instruction = found_instruction(&held, false);
    let (held, context, accepted) = execute(held, instruction).await;
    assert!(
        accepted,
        "the control must found while the pinned generation is the deployed one"
    );
    let market = context
        .banks_client
        .get_account(held.market)
        .await
        .expect("Market query")
        .expect("Market");
    assert_eq!(market.owner, CORE_PROGRAM_ID);

    // The pin breaks. The Registry moved to a later generation and nothing
    // else in the world changed at all.
    let upgraded = fixture_with(
        false,
        false,
        SuccessionStateV1::Succeeded,
        RegistryDeploymentV1::Upgraded,
    );
    let instruction = found_instruction(&upgraded, false);
    let (upgraded, context, failure) = execute_reporting_failure(upgraded, instruction).await;
    let failure = failure.expect("an upgraded Registry must supersede the sealed selection");
    assert_refused_with(
        &failure,
        dclutch_core_sbf::CoreSbfError::ReleaseSuperseded as u32,
        "CoreSbfError::ReleaseSuperseded on a Registry deployment the sealed profile no longer pins",
    );
    // Nothing was created on the way to the brick.
    let market = context
        .banks_client
        .get_account(upgraded.market)
        .await
        .expect("Market query")
        .expect("vacant Market");
    assert_eq!(market.owner, system_program::ID);
    assert!(market.data.is_empty());
}

/// The supersession refusal is reserved for a world where an upgrade was
/// possible.
///
/// The discriminator the brick pair above rests on, and it was MEASURED rather
/// than assumed: the first version of that pair ran an immutable Registry and
/// refused `Infrastructure`, not `ReleaseSuperseded`. That is the route being
/// right. `require_pinned_deployment` names a later generation a supersession
/// only when the observed upgrade authority is still the one the release bound;
/// a moved slot under an absent authority is a SUBSTITUTED ProgramData, because
/// a program with no upgrade authority cannot have been upgraded. The
/// distinction is operator-facing — `ReleaseSuperseded` promises a re-release
/// remedy, and promising it to someone whose ProgramData was swapped would send
/// them to republish a record that could not help.
///
/// Which is also why P-008 is a story about decision 0012's mutable iteration
/// substrate: the Registry bricked the protocol because it was upgradable.
#[tokio::test]
async fn a_moved_slot_under_no_upgrade_authority_is_substituted_rather_than_superseded() {
    // The brick's world in every respect but one: the Registry binds no upgrade
    // authority, so the generation it is standing in cannot have been reached
    // by an upgrade.
    let fixture = fixture_with(
        false,
        false,
        SuccessionStateV1::Succeeded,
        RegistryDeploymentV1::ImmutableMoved,
    );
    let instruction = found_instruction(&fixture, false);
    let (fixture, context, failure) = execute_reporting_failure(fixture, instruction).await;
    let failure = failure.expect("a substituted Registry ProgramData must still refuse founding");
    assert_refused_with(
        &failure,
        dclutch_core_sbf::CoreSbfError::Infrastructure as u32,
        "CoreSbfError::Infrastructure on a moved slot no upgrade authority could have moved",
    );
    let market = context
        .banks_client
        .get_account(fixture.market)
        .await
        .expect("Market query")
        .expect("vacant Market");
    assert_eq!(market.owner, system_program::ID);
    assert!(market.data.is_empty());
}

/// The first escape is dead: republishing the record cannot re-point the pin.
///
/// The other half of §8.1 — "prove the two escapes dead". The refusal an
/// operator reads from the brick above points at a re-release, and
/// `infrastructure.rs:457-462` says so in as many words. So take that advice
/// exactly: publish a finalized `ArtifactReleaseV1` for the Registry binding the
/// generation that is actually deployed, and present it. It is a perfectly good
/// record — it authenticates, and its own pin HOLDS, which is what makes this a
/// real escape attempt rather than a malformed frame. It is refused anyway, at
/// the content pin, because the sealed profile names a specific record by
/// digest and a different record has a different digest. There is no route from
/// here to a profile that names the new one.
///
/// The second escape — rewriting the profile — is dead by vacancy, and is
/// already proved by name in `infrastructure_succession_program_test.rs`
/// (`InfrastructureAlreadySucceeded` on the replayed ceremony, and the V1
/// vacancy check in `infrastructure_program_test.rs`). It is cited rather than
/// duplicated here.
#[tokio::test]
async fn the_republished_registry_record_cannot_escape_the_sealed_content_pin() {
    let fixture = fixture_with(
        false,
        false,
        SuccessionStateV1::Succeeded,
        RegistryDeploymentV1::Upgraded,
    );
    let mut instruction = found_instruction(&fixture, false);
    let mut swapped = 0;
    for meta in instruction.accounts.iter_mut() {
        if meta.pubkey == fixture.registry_artifact.raw {
            meta.pubkey = fixture.republished_registry_artifact.raw;
            swapped += 1;
        } else if meta.pubkey == fixture.registry_artifact.staging {
            meta.pubkey = fixture.republished_registry_artifact.staging;
            swapped += 1;
        }
    }
    assert_eq!(
        swapped, 2,
        "the honest frame must carry the pinned Registry record's raw and staging accounts"
    );
    let (fixture, context, failure) = execute_reporting_failure(fixture, instruction).await;
    let failure = failure.expect("a record the sealed profile does not name must be refused");
    assert_refused_with(
        &failure,
        dclutch_core_sbf::CoreSbfError::Infrastructure as u32,
        "CoreSbfError::Infrastructure on a republished record the write-once profile cannot name",
    );
    let market = context
        .banks_client
        .get_account(fixture.market)
        .await
        .expect("Market query")
        .expect("vacant Market");
    assert_eq!(market.owner, system_program::ID);
    assert!(market.data.is_empty());
}

#[tokio::test]
async fn slot_pinned_mutable_core_release_accepts_after_profile_init() {
    let fixture = fixture(true);
    let instruction = found_instruction(&fixture, false);
    let (fixture, context, accepted) = execute(fixture, instruction).await;
    assert!(accepted);
    let market = context
        .banks_client
        .get_account(fixture.market)
        .await
        .expect("Market query")
        .expect("Market");
    assert_eq!(market.owner, CORE_PROGRAM_ID);
    let state = CoreState::decode(&market.data).expect("CoreState");
    assert_eq!(state.phase, Phase::Founding);
    assert_eq!(state.readiness, Readiness::Prepaid);
}

async fn assert_series_found_rollback(
    fixture: &SeriesFixture,
    context: &solana_program_test::ProgramTestContext,
) {
    let market = context
        .banks_client
        .get_account(fixture.base.market)
        .await
        .expect("Market query")
        .expect("vacant Market");
    assert_eq!(market.owner, system_program::ID);
    assert!(market.data.is_empty());
    assert_eq!(
        market.lamports,
        Rent::default().minimum_balance(STATE_BYTES)
    );

    let permit = context
        .banks_client
        .get_account(fixture.permit)
        .await
        .expect("permit query")
        .expect("vacant permit");
    assert_eq!(permit.owner, system_program::ID);
    assert_eq!(permit.lamports, fixture.permit_lamports);
    assert!(permit.data.is_empty());

    for key in [fixture.aggregate, fixture.position, fixture.admission] {
        let candidate = context
            .banks_client
            .get_account(key)
            .await
            .expect("Claims candidate query")
            .expect("vacant Claims candidate");
        assert_eq!(candidate.owner, system_program::ID);
        assert!(candidate.data.is_empty());
    }

    let root = context
        .banks_client
        .get_account(fixture.root)
        .await
        .expect("root query")
        .expect("root");
    assert_eq!(root.owner, TRADING_PROGRAM_ID);
    assert_eq!(root.data, fixture.root_data);

    let ticket = context
        .banks_client
        .get_account(fixture.ticket_state)
        .await
        .expect("Ticket query")
        .expect("Ticket state");
    assert_eq!(ticket.owner, TRADING_PROGRAM_ID);
    assert_eq!(ticket.data, fixture.ticket_state_data);

    let funding = context
        .banks_client
        .get_account(fixture.funding)
        .await
        .expect("Funding query")
        .expect("FundingState");
    assert_eq!(funding.owner, TRADING_PROGRAM_ID);
    assert_eq!(funding.lamports, fixture.funding_lamports);
    assert_eq!(funding.data, fixture.funding_data);

    let caller = context
        .banks_client
        .get_account(fixture.caller_authority)
        .await
        .expect("caller query")
        .expect("caller PDA");
    assert_eq!(caller.owner, system_program::ID);
    assert_eq!(caller.lamports, 1);
    assert!(caller.data.is_empty());
}

/// Ruling §8.3's permit-refund arm, through the compiled Core entrypoint.
///
/// The positive world presents the post-ceremony V2 profile and drains the
/// unallocated permit into its creation-fixed RentCredit. The two hostile
/// worlds hold every later Series fact constant and perturb only the profile
/// selection: first an exact copy of the V2 bytes at the wrong PDA, then the
/// sealed V1 predecessor. Both must refuse as `Infrastructure`, and both must
/// leave the permit and RentCredit byte-for-byte unchanged.
#[tokio::test]
async fn series_permit_expiry_uses_only_the_authenticated_successor_profile() {
    let permit = issued_series_permit().await;

    let fixture = series_expiry_fixture(permit, ExpiryProfileV1::Successor);
    let (fixture, context, accepted, permit_before, credit_before) =
        execute_series_expiry(fixture, permit, ExpiryProfileV1::Successor).await;
    assert_eq!(accepted, Ok(()), "the V2-authenticated refund must land");
    assert_eq!(permit_before.owner, system_program::ID);
    assert!(permit_before.data.is_empty());
    assert_eq!(permit_before.lamports, fixture.permit_lamports);
    let permit_after = context
        .banks_client
        .get_account(fixture.permit)
        .await
        .expect("permit query");
    if let Some(account) = permit_after {
        assert_eq!(account.owner, system_program::ID);
        assert!(account.data.is_empty());
        assert_eq!(account.lamports, 0, "the accepted refund closes the permit");
    }
    let credit_after = context
        .banks_client
        .get_account(fixture.base.rent_credit)
        .await
        .expect("RentCredit query")
        .expect("RentCredit");
    assert_eq!(credit_after.owner, credit_before.owner);
    assert_eq!(credit_after.data, credit_before.data);
    assert_eq!(
        credit_after.lamports,
        credit_before
            .lamports
            .checked_add(permit_before.lamports)
            .expect("refund balance"),
        "the unfunded 25-account arm returns every permit lamport"
    );
    drop(context);
    drop(fixture);

    for profile in [
        ExpiryProfileV1::WrongAddress,
        ExpiryProfileV1::SealedPredecessor,
    ] {
        let fixture = series_expiry_fixture(permit, profile);
        let (fixture, context, refused, permit_before, credit_before) =
            execute_series_expiry(fixture, permit, profile).await;
        assert_expiry_refused(refused, dclutch_core_sbf::CoreSbfError::Infrastructure);

        let permit_after = context
            .banks_client
            .get_account(fixture.permit)
            .await
            .expect("permit query")
            .expect("refused permit");
        let credit_after = context
            .banks_client
            .get_account(fixture.base.rent_credit)
            .await
            .expect("RentCredit query")
            .expect("RentCredit");
        assert_eq!(permit_after, permit_before, "{profile:?} permit rollback");
        assert_eq!(credit_after, credit_before, "{profile:?} credit rollback");

        let presented = context
            .banks_client
            .get_account(profile.address(&fixture))
            .await
            .expect("profile query")
            .expect("presented profile");
        assert_eq!(presented.owner, CORE_PROGRAM_ID);
        match profile {
            ExpiryProfileV1::WrongAddress => {
                let canonical = context
                    .banks_client
                    .get_account(fixture.base.profile)
                    .await
                    .expect("canonical profile query")
                    .expect("canonical V2 profile");
                assert_eq!(presented, canonical, "only the profile address differs");
                assert_ne!(profile.address(&fixture), fixture.base.profile);
            }
            ExpiryProfileV1::SealedPredecessor => {
                assert_eq!(
                    presented.data.len(),
                    dclutch_registry::release_set::PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1
                );
                ProtocolInfrastructureProfileV1::decode(&presented.data)
                    .expect("the sealed predecessor remains canonical V1");
            }
            ExpiryProfileV1::Successor => unreachable!("positive arm ran above"),
        }
    }
}

#[tokio::test]
async fn series_consume_accepts_258_outcomes_and_commits_found_with_permit() {
    let fixture = series_fixture(SeriesFault::None);
    let (fixture, context, failure) = execute_series(
        fixture,
        "Series occurrence Consume founds its Market with a Core permit",
    )
    .await;
    assert_eq!(failure, None, "Series Found must complete under 1.4M CU");
    assert_eq!(fixture.base.outcome_count, 258);

    let market = context
        .banks_client
        .get_account(fixture.base.market)
        .await
        .expect("Market query")
        .expect("founded Market");
    assert_eq!(market.owner, CORE_PROGRAM_ID);
    let state = CoreState::decode(&market.data).expect("Core state");
    assert_eq!(state.phase, Phase::Founding);
    assert_eq!(state.readiness, Readiness::Prepaid);

    let permit = context
        .banks_client
        .get_account(fixture.permit)
        .await
        .expect("permit query")
        .expect("Core permit");
    assert_eq!(permit.owner, CORE_PROGRAM_ID);
    assert_eq!(permit.lamports, fixture.permit_lamports);
    let permit = SeriesFoundingPermitV1::decode(&permit.data).expect("Series founding permit");
    assert_eq!(
        permit.intent().market().to_bytes(),
        fixture.base.market.to_bytes()
    );
    assert_eq!(
        permit.intent().parent_root().to_bytes(),
        fixture.root.to_bytes()
    );
    assert_eq!(
        permit.intent().projected_replay().to_bytes(),
        fixture.projected_replay.to_bytes()
    );
    assert_eq!(
        permit.intent().claims_program().to_bytes(),
        CLAIMS_PROGRAM_ID.to_bytes()
    );
    assert_eq!(permit.intent().quantity(), 5_000);
    assert_eq!(permit.intent().basis_scale(), 1);
}

#[tokio::test]
async fn series_consume_hostile_programdata_refuses_with_byte_exact_rollback() {
    let fixture = series_fixture(SeriesFault::BatchClaimsProgramdata);
    let (fixture, context, failure) = execute_series(
        fixture,
        "Series Consume refuses a substituted Claims ProgramData",
    )
    .await;
    let failure = failure.expect("substituted Claims ProgramData must refuse");
    assert_refused_with(
        &failure,
        dclutch_core_sbf::CoreSbfError::Release as u32,
        "Core's exact current-release refusal",
    );
    assert_series_found_rollback(&fixture, &context).await;
}

#[tokio::test]
async fn series_consume_refuses_to_consume_the_same_ticket_twice() {
    // The ticket is the whole point of a Series occurrence: it is the one-shot
    // authority to found this Market at this index. If a second identical
    // transaction could consume it again, the Series would mint a second
    // liability from one prepayment.
    let fixture = series_fixture(SeriesFault::None);
    let (fixture, context, first, replay) = execute_series_twice(
        fixture,
        "Series occurrence Consume founds its Market with a Core permit (replay campaign)",
        "Series Consume refuses a replayed ticket",
    )
    .await;
    assert_eq!(first, None, "the first occurrence must found its Market");
    let replay = replay.expect("a consumed ticket must not fund a second Found");

    // The Market survives the refused replay exactly as the first Consume left
    // it. A replay that refused but disturbed committed state would be a worse
    // failure than one that succeeded, because nothing would report it.
    let market = context
        .banks_client
        .get_account(fixture.base.market)
        .await
        .expect("Market query")
        .expect("the Market founded by the first occurrence");
    assert_eq!(market.owner, CORE_PROGRAM_ID);
    let state = CoreState::decode(&market.data).expect("Core state");
    assert_eq!(state.phase, Phase::Founding);
    assert_eq!(state.readiness, Readiness::Prepaid);

    let permit = context
        .banks_client
        .get_account(fixture.permit)
        .await
        .expect("permit query")
        .expect("the permit written by the first occurrence");
    assert_eq!(permit.owner, CORE_PROGRAM_ID);
    assert_eq!(permit.lamports, fixture.permit_lamports);
    let permit = SeriesFoundingPermitV1::decode(&permit.data).expect("Series founding permit");
    assert_eq!(
        permit.intent().market().to_bytes(),
        fixture.base.market.to_bytes()
    );
    // Pinned to the exact code, not merely "some refusal". `CoreSbfError::Market`
    // is the Market PDA, owner, width, phase, or generation refusing. That is
    // the honest guard for a replay -- the first Consume moved the Market out of
    // the prestate this occurrence requires, so the second cannot proceed. A
    // refactor that moved the refusal to a different guard has to say so here.
    assert_refused_with(
        &replay,
        dclutch_core_sbf::CoreSbfError::Market as u32,
        "CoreSbfError::Market on a replayed ticket",
    );
}

#[tokio::test]
async fn series_consume_late_hoard_refusal_rolls_back_found_and_all_replay_state() {
    let fixture = series_fixture(SeriesFault::LateHoardBalance);
    let (fixture, context, failure) =
        execute_series(fixture, "Series Consume refuses a late Hoard postcondition").await;
    let failure = failure.expect("late Hoard postcondition must refuse");
    assert_refused_with(
        &failure,
        dclutch_core_sbf::CoreSbfError::ChildAck as u32,
        "CoreSbfError::ChildAck on a late postcondition",
    );
    assert_series_found_rollback(&fixture, &context).await;
}

/// Emit the `series_consume` campaign as a bundle a live validator can run.
///
/// # Why this exists, and why it is a test rather than a tool
///
/// `series_consume` is the only executed Series route in the tree, and until
/// now it had executed only inside `ProgramTest`. Moving it onto a real
/// validator means reproducing roughly 1,250 lines of fixture — a batch of
/// finalized Registry records, six loader-v3 program pairs, a projected-Custody
/// lock, exact-rent vacant accounts — and porting that into a host tool is a
/// multi-hour rewrite with a wide surface for silent divergence.
///
/// So it is not ported. The fixture stays exactly where it is and keeps its one
/// author, and this test changes only the SINK: it builds the campaign the way
/// every other Series test does, starts the genesis it would have run against,
/// and then reads every account the instruction names straight back out of the
/// banks client. What is written out is therefore what `ProgramTest` itself
/// constructed, not a second reconstruction of it.
///
/// That reading is what disposes of the two hazards a hand-port would have hit:
/// the exact-lamports rule (`series_consume` compares
/// `market.lamports()` to `request.market_rent()` with `!=`, so a rent
/// heuristic silently refuses) and the six loader-v3 Program/ProgramData pairs
/// (whose deployment slot flows into the release-set digest and therefore into
/// the Market PDA, making deploy-then-derive circular with genesis). Both are
/// simply observed rather than recomputed.
///
/// It is `#[ignore]`d because it writes to a directory and is a build step for
/// a validator run, not an assertion about the protocol. The assertions about
/// the protocol are the four `series_consume_*` tests above, which are
/// unchanged.
///
/// Run it as:
///
/// ```sh
/// SBF_OUT_DIR=... DCLUTCH_SERIES_CAMPAIGN_DIR=/abs/dir \
///   cargo test --manifest-path programs/dclutch-core-sbf/Cargo.toml \
///   --test found_program_test -- --ignored emit_series_consume_validator_campaign
/// ```
#[tokio::test]
#[ignore = "emits a genesis and instruction bundle for a live local validator"]
async fn emit_series_consume_validator_campaign() {
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

    let directory = PathBuf::from(
        env::var("DCLUTCH_SERIES_CAMPAIGN_DIR")
            .expect("DCLUTCH_SERIES_CAMPAIGN_DIR is required to emit the campaign"),
    );

    let mut fixture = series_fixture(SeriesFault::None);
    let mut test = fixture.base.test.take().expect("ProgramTest");
    let instruction = series_instruction(&fixture);
    let lookup = add_instruction_lookup(&mut test, core::slice::from_ref(&instruction));
    let context = test.start_with_context().await;

    let (written, absent) = emit_series_validator_genesis(
        &context,
        &instruction,
        &lookup,
        &[
            fixture.base.core_programdata,
            RESOLUTION_PROGRAM_ID,
            fixture.base.resolution_programdata,
        ],
        &directory.join("accounts"),
    )
    .await;

    let manifest = serde_json::json!({
        "schema": "dclutch-series-consume-validator-campaign-v1",
        "programId": instruction.program_id.to_string(),
        "lookupTable": lookup.key.to_string(),
        "dataBase64": BASE64.encode(&instruction.data),
        "accounts": instruction
            .accounts
            .iter()
            .map(|meta| serde_json::json!({
                "pubkey": meta.pubkey.to_string(),
                "isSigner": meta.is_signer,
                "isWritable": meta.is_writable,
            }))
            .collect::<Vec<_>>(),
        // Measured, not guessed: docs/reference/budgets.md records 722,142 CU
        // for this route. ProgramTest hands out 1,400,000 globally and a real
        // validator gives 200,000, so the limit has to travel with the bundle.
        "computeUnitLimit": 900_000,
        "genesisAccountCount": written,
        "genesisOnly": [
            fixture.base.core_programdata.to_string(),
            RESOLUTION_PROGRAM_ID.to_string(),
            fixture.base.resolution_programdata.to_string(),
        ],
        "absentByDesign": absent,
        // What a caller must check AFTER the transaction finalizes. These are
        // the same facts `series_consume_accepts_258_outcomes_and_commits_found_with_permit`
        // asserts, carried out to whoever submits the packet.
        "expect": {
            "outcomeCount": fixture.base.outcome_count,
            "market": fixture.base.market.to_string(),
            "marketOwner": CORE_PROGRAM_ID.to_string(),
            "permit": fixture.permit.to_string(),
            "permitOwner": CORE_PROGRAM_ID.to_string(),
            "permitLamports": fixture.permit_lamports,
            "resolutionProgram": RESOLUTION_PROGRAM_ID.to_string(),
            "resolutionProgramdata": fixture.base.resolution_programdata.to_string(),
        }
    });
    write_series_emitted_json(&directory.join("campaign.json"), &manifest);

    assert!(written > 0, "the campaign emitted no genesis accounts");
    println!(
        "series-consume campaign: {written} genesis accounts ({} absent by design), \
         {} metas, {} data bytes, lookup table {} -> {}",
        absent.len(),
        instruction.accounts.len(),
        instruction.data.len(),
        lookup.key,
        directory.display()
    );
}

/// Emit the compiled 25-account Series permit-expiry route for a live validator.
///
/// This is the second physically executable Series transition exported from
/// the fixture. It does not synthesize a permit: [`issued_series_permit`] first
/// asks the real `series_consume` entrypoint to author the exact 608 bytes. The
/// three output worlds then differ only in the infrastructure profile the
/// expiry frame presents:
///
/// - `successor` carries the authenticated V2 profile and must move every
///   unallocated permit lamport into its creation-fixed RentCredit;
/// - `wrong-address` carries byte-identical V2 data at the wrong PDA;
/// - `sealed-predecessor` carries the still-decodable V1 profile.
///
/// The latter two are transaction-level rollback campaigns, not simulations.
/// Start each validator with `--warp-slot` set to the emitted
/// `minimumExecutionSlot`; the retry deadline is a protocol fact and this
/// emitter does not shorten it for the test.
///
/// ```sh
/// SBF_OUT_DIR=... DCLUTCH_SERIES_EXPIRY_CAMPAIGN_DIR=/abs/dir \
///   cargo test --manifest-path programs/dclutch-core-sbf/Cargo.toml \
///   --test found_program_test -- --ignored emit_series_permit_expiry_validator_campaign
/// ```
#[tokio::test]
#[ignore = "emits three genesis/instruction bundles for a live local validator"]
async fn emit_series_permit_expiry_validator_campaign() {
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

    let root = PathBuf::from(
        env::var("DCLUTCH_SERIES_EXPIRY_CAMPAIGN_DIR")
            .expect("DCLUTCH_SERIES_EXPIRY_CAMPAIGN_DIR is required to emit the campaign"),
    );
    let permit = issued_series_permit().await;

    for (case, profile) in [
        ("successor", ExpiryProfileV1::Successor),
        ("wrong-address", ExpiryProfileV1::WrongAddress),
        ("sealed-predecessor", ExpiryProfileV1::SealedPredecessor),
    ] {
        let directory = root.join(case);
        let mut fixture = series_expiry_fixture(permit, profile);
        let instruction = series_expiry_instruction(&fixture, permit, profile);
        let mut test = fixture.base.test.take().expect("ProgramTest");
        let lookup = add_instruction_lookup(&mut test, core::slice::from_ref(&instruction));
        let mut context = test.start_with_context().await;
        warp_to_deployment_generation(&mut context, &fixture.base);

        let permit_before = context
            .banks_client
            .get_account(fixture.permit)
            .await
            .expect("permit query")
            .expect("prefunded permit");
        let credit_before = context
            .banks_client
            .get_account(fixture.base.rent_credit)
            .await
            .expect("RentCredit query")
            .expect("RentCredit");
        assert_eq!(permit_before.owner, system_program::ID);
        assert!(permit_before.data.is_empty());
        assert_eq!(permit_before.lamports, fixture.permit_lamports);

        let (written, absent) = emit_series_validator_genesis(
            &context,
            &instruction,
            &lookup,
            &[fixture.base.core_programdata],
            &directory.join("accounts"),
        )
        .await;
        let expiry_slot = permit.intent().expiry_slot();
        let minimum_execution_slot = expiry_slot.checked_add(1).expect("post-expiry slot");
        let manifest = serde_json::json!({
            "schema": "dclutch-series-permit-expiry-validator-campaign-v1",
            "case": case,
            "programId": instruction.program_id.to_string(),
            "lookupTable": lookup.key.to_string(),
            "dataBase64": BASE64.encode(&instruction.data),
            "accounts": instruction
                .accounts
                .iter()
                .map(|meta| serde_json::json!({
                    "pubkey": meta.pubkey.to_string(),
                    "isSigner": meta.is_signer,
                    "isWritable": meta.is_writable,
                }))
                .collect::<Vec<_>>(),
            "computeUnitLimit": 1_400_000_u32,
            "genesisAccountCount": written,
            "genesisOnly": [fixture.base.core_programdata.to_string()],
            "absentByDesign": absent,
            "expect": {
                "expirySlot": expiry_slot,
                "minimumExecutionSlot": minimum_execution_slot,
                "permit": fixture.permit.to_string(),
                "permitOwnerBefore": permit_before.owner.to_string(),
                "permitLamportsBefore": permit_before.lamports,
                "permitDataBase64Before": BASE64.encode(&permit_before.data),
                "rentCredit": fixture.base.rent_credit.to_string(),
                "rentCreditOwner": credit_before.owner.to_string(),
                "rentCreditLamportsBefore": credit_before.lamports,
                "rentCreditDataBase64Before": BASE64.encode(&credit_before.data),
                "refundLamports": permit_before.lamports,
            }
        });
        write_series_emitted_json(&directory.join("campaign.json"), &manifest);
        println!(
            "series permit-expiry {case}: {written} genesis accounts ({} absent by design), \
             {} metas, lookup table {}, minimum slot {} -> {}",
            absent.len(),
            instruction.accounts.len(),
            lookup.key,
            minimum_execution_slot,
            directory.display()
        );
    }
}

/// Export every physical account one validator frame can observe.
///
/// This is intentionally shared by the Consume and permit-expiry emitters so
/// the account-dir representation has one author. Absent keys remain explicit
/// manifest facts; they are never materialized as zero-valued accounts.
async fn emit_series_validator_genesis(
    context: &ProgramTestContext,
    instruction: &Instruction,
    lookup: &AddressLookupTableAccount,
    extra_genesis_keys: &[Pubkey],
    accounts_directory: &std::path::Path,
) -> (usize, Vec<String>) {
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
    use std::collections::BTreeSet;

    fs::create_dir_all(accounts_directory).expect("campaign account directory");
    let mut keys: BTreeSet<Pubkey> = BTreeSet::new();
    keys.insert(instruction.program_id);
    keys.insert(lookup.key);
    keys.extend(instruction.accounts.iter().map(|meta| meta.pubkey));
    keys.extend(lookup.addresses.iter().copied());
    keys.extend(extra_genesis_keys.iter().copied());

    let mut written = 0_usize;
    let mut absent = Vec::new();
    for key in keys {
        let Some(account) = context
            .banks_client
            .get_account(key)
            .await
            .expect("banks client")
        else {
            absent.push(key.to_string());
            continue;
        };
        let body = serde_json::json!({
            "pubkey": key.to_string(),
            "account": {
                "lamports": account.lamports,
                "data": [BASE64.encode(&account.data), "base64"],
                "owner": account.owner.to_string(),
                "executable": account.executable,
                "rentEpoch": account.rent_epoch,
            }
        });
        write_series_emitted_json(&accounts_directory.join(format!("{key}.json")), &body);
        written = written.checked_add(1).expect("genesis account count");
    }
    (written, absent)
}

/// Atomically publish one emitter artifact.
fn write_series_emitted_json(path: &std::path::Path, value: &serde_json::Value) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("emitter output directory");
    }
    let temporary = path.with_extension("json.partial");
    let mut bytes = serde_json::to_vec_pretty(value).expect("emitted JSON");
    bytes.push(b'\n');
    fs::write(&temporary, bytes).expect("write temporary emitted JSON");
    fs::rename(&temporary, path).expect("publish emitted JSON");
}

/// A STRANGER'S ONE LAMPORT NO LONGER STRANDS A SCHEDULED OCCURRENCE.
///
/// Census row R13's class, and this instance was worse than the one the census
/// named. R13's victim declared a balance a slot early and was front-run in the
/// gap. Here the victim cannot choose a different account at all:
/// `series_consume.rs:825-829` pins `request.market()` to
/// `occurrence.market()`, so the market PDA's address is **published on chain
/// in the occurrence record before the founding transaction exists**. Anyone
/// could read a scheduled occurrence, send its market PDA a single lamport, and
/// strand that occurrence's prepaid ticket forever, for about one lamport plus
/// a fee, with no race to win.
///
/// This harness already knew about the rule and routed around it rather than
/// treating it as a defect: `emit_series_consume_validator_campaign`'s header
/// names "the exact-lamports rule (`series_consume` compares
/// `market.lamports()` to `request.market_rent()` with `!=`, so a rent
/// heuristic silently refuses)" as one of two hazards a host port would hit.
/// It is now a floor, so that hazard is gone for any future port too.
///
/// The attack is executed, not simulated: a funded keypair that is nobody sends
/// the lamport in its own finalized transaction, after genesis and before the
/// Consume lands, and the test asserts it arrived before submitting.
#[tokio::test]
async fn a_strangers_lamport_cannot_strand_a_scheduled_series_occurrence() {
    let mut fixture = series_fixture(SeriesFault::None);
    let mut test = fixture.base.test.take().expect("ProgramTest");
    let instruction = series_instruction(&fixture);
    let lookup = add_instruction_lookup(&mut test, core::slice::from_ref(&instruction));
    let mut context = test.start_with_context().await;

    let budgeted = context
        .banks_client
        .get_account(fixture.base.market)
        .await
        .expect("market query")
        .expect("prepaid market")
        .lamports;

    // THE ATTACK. The occurrence record names this address publicly; the griefer
    // needs no permission, no race, and no knowledge the chain does not already
    // publish.
    let griefer = Keypair::new();
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let payer = context.payer.pubkey();
    context
        .banks_client
        .process_transaction(solana_transaction::Transaction::new_signed_with_payer(
            &[solana_system_interface::instruction::transfer(
                &payer,
                &griefer.pubkey(),
                Rent::default()
                    .minimum_balance(0)
                    .checked_mul(64)
                    .expect("griefer funding"),
            )],
            Some(&payer),
            &[&context.payer],
            blockhash,
        ))
        .await
        .expect("the stranger is funded like anyone else");
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    context
        .banks_client
        .process_transaction(solana_transaction::Transaction::new_signed_with_payer(
            &[solana_system_interface::instruction::transfer(
                &griefer.pubkey(),
                &fixture.base.market,
                1,
            )],
            Some(&griefer.pubkey()),
            &[&griefer],
            blockhash,
        ))
        .await
        .expect("one lamport to a published address needs nobody's permission");
    assert_eq!(
        context
            .banks_client
            .get_account(fixture.base.market)
            .await
            .expect("market query")
            .expect("donated market")
            .lamports,
        budgeted + 1,
        "the attack must have landed, or nothing below is a test"
    );

    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("post-donation blockhash");
    let transaction = signed_v0(
        &context.payer.pubkey(),
        &[instruction],
        &lookup,
        blockhash,
        &[&context.payer],
    );
    let failure = submit_and_record(
        &context,
        transaction,
        "Series occurrence Consume survives a one-lamport front-run",
    )
    .await;
    assert_eq!(
        failure, None,
        "a stranger's lamport must not be able to strand this occurrence"
    );

    // The Market is founded on the occurrence's own terms, and the donation is
    // simply carried by the account rather than accounted anywhere.
    let market = context
        .banks_client
        .get_account(fixture.base.market)
        .await
        .expect("Market query")
        .expect("founded Market");
    assert_eq!(market.owner, CORE_PROGRAM_ID);
    assert_eq!(market.lamports, budgeted + 1);
    let state = CoreState::decode(&market.data).expect("Core state");
    assert_eq!(state.phase, Phase::Founding);
    assert_eq!(state.readiness, Readiness::Prepaid);
}

/// UNDERFUNDING A SCHEDULED OCCURRENCE'S MARKET STILL REFUSES, AT THE CODE.
///
/// The negative control for the floor above, and the reason it is a separate
/// test rather than an assertion inside that one: it must run against a market
/// account that holds LESS than the occurrence budgeted, which is a different
/// genesis, not a different transaction.
///
/// This is the entire safety content of the comparison that was relaxed. If it
/// ever passes, the floor became no check at all.
#[tokio::test]
async fn a_market_short_of_its_occurrences_budgeted_rent_still_refuses() {
    let fixture = series_fixture(SeriesFault::UnderfundedMarket);
    let (_fixture, _context, failure) = execute_series(
        fixture,
        "Series occurrence Consume against an underfunded Market",
    )
    .await;
    let failure = failure.expect("an underfunded Market must refuse");
    assert_refused_with(
        &failure,
        dclutch_core_sbf::CoreSbfError::Reference as u32,
        "underfunded Series Market rent",
    );
    // Seal the lie: prove the genesis really was short, and that the refusal
    // left the Market vacant rather than founding it anyway. Without this the
    // test would still pass if the fault had silently not been applied and
    // something else had refused.
    let market = _context
        .banks_client
        .get_account(_fixture.base.market)
        .await
        .expect("market query")
        .expect("underfunded market");
    assert_eq!(
        market.lamports,
        Rent::default()
            .minimum_balance(STATE_BYTES)
            .checked_sub(1)
            .expect("underfunded market"),
        "the fault must actually have been applied"
    );
    assert_eq!(market.owner, system_program::ID);
    assert!(
        market.data.is_empty(),
        "the refused found allocated nothing"
    );
}

/// **Curvature founds, on a real Core ELF.**
///
/// This is the acceptance condition the cut exists for: a degree-2 basis, a
/// `DCLTPGT1` no-arbitrage certificate whose hull identity Core recomputes
/// through the production evaluator, and a Market that reaches `Founding`.
///
/// The frame is 39 accounts rather than 37 — the certificate pair, appended
/// last — and every coordinate before it is unmoved, which is why the twelve
/// tests above still pass untouched.
#[tokio::test]
async fn a_degree_two_market_founds_with_a_valid_price_gate_certificate() {
    let fixture = curved_fixture();
    assert!(
        fixture.price_gate.is_some(),
        "the curved fixture carries a certificate"
    );
    let instruction = found_instruction(&fixture, false);
    assert_eq!(
        instruction.accounts.len(),
        dclutch_market::FOUND_PRICE_GATE_ACCOUNT_COUNT_V3,
        "the extended frame is the canonical one plus the certificate pair"
    );

    let (fixture, context, accepted) = execute(fixture, instruction).await;
    assert!(accepted, "a curved basis with a valid certificate founds");
    assert_eq!(fixture.outcome_count, 5);

    let market = context
        .banks_client
        .get_account(fixture.market)
        .await
        .expect("Market query")
        .expect("Market");
    assert_eq!(market.owner, CORE_PROGRAM_ID);
    let state = CoreState::decode(&market.data).expect("CoreState");
    assert_eq!(state.phase, Phase::Founding);
}

/// **And without the certificate it refuses, by name.**
///
/// The same curved basis, the same 37-account canonical frame every graded
/// market uses. The refusal is `PriceGateRequired` (0x3012) and not a length
/// mismatch or a generic `Reference`: the frame is well formed, the basis is
/// well formed, and what is missing is the no-arbitrage witness.
///
/// This is the red-proof for the founding gate. Without it the conjunct could
/// be unreachable and every test above would still pass.
#[tokio::test]
async fn a_degree_two_market_without_its_certificate_refuses_by_name() {
    let mut fixture = curved_fixture();
    // Drop only the certificate, so the instruction builds the canonical frame.
    fixture.price_gate = None;
    let instruction = found_instruction(&fixture, false);
    assert_eq!(
        instruction.accounts.len(),
        dclutch_market::FOUND_ACCOUNT_COUNT_V3,
        "the canonical frame, exactly as a graded founding builds it"
    );

    // Executed inline rather than through `execute`, which returns only a
    // boolean: the whole point of this test is WHICH refusal.
    let mut fixture = fixture;
    let mut test = fixture.test.take().expect("ProgramTest");
    let lookup = add_instruction_lookup(&mut test, core::slice::from_ref(&instruction));
    let context = test.start_with_context().await;
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let outcome = context
        .banks_client
        .process_transaction(signed_v0(
            &context.payer.pubkey(),
            &[instruction],
            &lookup,
            blockhash,
            &[&context.payer, &fixture.payer],
        ))
        .await;
    let refusal = format!(
        "{:?}",
        outcome.expect_err("a curved basis with no certificate")
    );
    assert!(
        refusal.contains(&format!(
            "Custom({})",
            dclutch_core_sbf::CoreSbfError::PriceGateRequired as u32
        )),
        "expected PriceGateRequired (0x3012), got {refusal}"
    );
}
