//! Real-ELF Core Found31 infrastructure and Runtime Product V2 composition.

use std::{env, fs, path::PathBuf, vec, vec::Vec};

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    CapabilityEntryV1, CapabilityFundingDerivationV1, CapabilityManifestV1, CompartmentFundingV1,
    ContentId as CapabilityContentId, FUNDING_STATE_BYTES, FundingAmountsV1,
    FundingCustodyObservationV1, FundingQuoteV1, FundingStateV1, MANIFEST_HEADER_BYTES,
    MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
};
use dclutch_claims_svm::{
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
use dclutch_custody_contract::{
    CompartmentV1, PROJECTED_CUSTODY_STATE_BYTES_V1, PROJECTED_HOARD_CONTEXT_DOMAIN_V1,
    ProjectedCallerRoleV1, ProjectedCustodyLockReceiptV1, ProjectedCustodyOperationV1,
    ProjectedCustodyPhaseV1, ProjectedCustodyRequestV1, ProjectedCustodyStateSeedsV1,
    ProjectedCustodyStateV1,
};
use dclutch_market_core_codec::{
    Action, CoreState, Identity, MarketCoreStateSeedsV2, MarketIdentity, Phase,
    ProjectFoundRequestV1, Readiness, Request, SERIES_FOUNDING_PERMIT_BYTES_V1, STATE_BYTES,
    SeriesFoundingPermitSeedsV1, SeriesFoundingPermitV1,
};
use dclutch_product_payoff_v2_codec::{
    registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
    runtime_v3::{
        BasisInputV3, BasisKindV3, SEMANTIC_BASIS_CONTENT_DOMAIN_V3, basis_record_bytes_v3,
        compile_basis_v3, semantic_basis_preimage_v3,
    },
};
use dclutch_product_runtime_v2::{ContentId, portfolio_record_bytes, result_domain_record_bytes};
use dclutch_product_runtime_v2_admission::{FinalizedRecordCoordinateV2, PRODUCT_RECORD_BYTES_V2};
use dclutch_product_runtime_v2_operator::{ProductCompilationInputV2, compile_product_records_v2};
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1, RealmV1Input,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, activate_execution_role_into_v1,
    initialize_activation_cache_v1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, CallerAuthoritySeedsV1, CapabilityExecutionSelectionV1,
    EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1, ExecutionReleaseSetV1, ExecutionRoleBindingV1,
    ExecutionRoleV1, PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1, ProgramIdentityV1,
    ProtocolInfrastructureProfileV1,
};
use dclutch_rent_contract::{
    RefundAuthority,
    lifecycle_v2::{
        LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleAccountIdV2, LifecycleRentCreditV2,
    },
};
use dclutch_series_v3_kernel::{
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
    replay::{SeriesStateV3, TicketStateSeedsV3, TicketStateV3},
    series_core_consume_request, template_content_id, ticket_content_id,
};
use dclutch_source_contract::{
    ContentId as SourceContentId, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2, SourceMaterialV2,
};
use solana_account::Account;
use solana_program::{
    hash::{hash, hashv},
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
use spl_token_interface::state::{Account as SplAccount, AccountState as SplAccountState};

const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc1; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc2; 32]);
const RENT_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc3; 32]);
const TRADING_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc4; 32]);
const CLAIMS_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc5; 32]);
const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc6; 32]);
const TOKEN_PROGRAM_ID: Pubkey = Pubkey::new_from_array(dclutch_token_svm::LEGACY_TOKEN_PROGRAM_ID);
const COLLATERAL_MINT: Pubkey = Pubkey::new_from_array([0xb2; 32]);
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
    registry: Vec<u8>,
    rent: Vec<u8>,
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
    source: Record,
    manifest: Record,
    release_set: Record,
    cache: Pubkey,
    core_programdata: Pubkey,
    trading_programdata: Pubkey,
    claims_programdata: Pubkey,
    custody_programdata: Pubkey,
    registry_programdata: Pubkey,
    rent_programdata: Pubkey,
    profile: Pubkey,
    registry_artifact: Record,
    rent_artifact: Record,
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
    lock_receipt: [u8; dclutch_custody_contract::PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1],
    request: [u8; dclutch_market_core_codec::SERIES_CORE_REQUEST_BYTES_V1],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SeriesFault {
    None,
    LateHoardBalance,
    BatchClaimsProgramdata,
}

fn artifacts() -> Artifacts {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    Artifacts {
        core: fs::read(directory.join("dclutch_core_sbf.so")).expect("Core ELF"),
        registry: fs::read(directory.join("dclutch_registry_sbf.so")).expect("Registry ELF"),
        rent: fs::read(directory.join("dclutch_rent_sbf.so")).expect("Rent ELF"),
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

fn programdata_bytes(elf: &[u8], authority: Option<Pubkey>) -> Vec<u8> {
    let mut bytes = vec![0; 45 + elf.len()];
    bytes
        .get_mut(0..4)
        .expect("tag")
        .copy_from_slice(&3_u32.to_le_bytes());
    bytes
        .get_mut(4..12)
        .expect("slot")
        .copy_from_slice(&0_u64.to_le_bytes());
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
) {
    test.add_upgradeable_program_to_genesis(name, &program);
    let data = programdata_bytes(elf, authority);
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
        0,
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
        let data = programdata_bytes(&[0x42; 32], None);
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
        state_rent_lamports: rent.minimum_balance(PROJECTED_CUSTODY_STATE_BYTES_V1),
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
    let projected_seeds = ProjectedCustodyStateSeedsV1::from_request(projected_request);
    let (projected_replay, projected_bump) =
        Pubkey::find_program_address(&projected_seeds.as_slices(), &CUSTODY_PROGRAM_ID);
    let projected_state = ProjectedCustodyStateV1 {
        phase: ProjectedCustodyPhaseV1::HoardLocked,
        request: projected_request,
        next_revision: 3,
        locked_amount: hoard_principal,
        last_request_digest: lock_request_digest,
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

fn fixture(core_mutable: bool) -> Fixture {
    let artifacts = artifacts();
    let mutable_authority = core_mutable.then(|| Pubkey::new_from_array([0xd1; 32]));
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    add_program(
        &mut test,
        "dclutch_core_sbf",
        CORE_PROGRAM_ID,
        &artifacts.core,
        mutable_authority,
    );
    add_program(
        &mut test,
        "dclutch_registry_sbf",
        REGISTRY_PROGRAM_ID,
        &artifacts.registry,
        None,
    );
    add_program(
        &mut test,
        "dclutch_rent_sbf",
        RENT_PROGRAM_ID,
        &artifacts.rent,
        None,
    );
    add_program(
        &mut test,
        "dclutch_series_consume_caller_sbf",
        TRADING_PROGRAM_ID,
        &artifacts.trading,
        None,
    );
    add_program(
        &mut test,
        "dclutch_core_sbf",
        CLAIMS_PROGRAM_ID,
        &artifacts.core,
        None,
    );
    add_program(
        &mut test,
        "dclutch_core_sbf",
        CUSTODY_PROGRAM_ID,
        &artifacts.core,
        None,
    );
    let core_release = release(CORE_PROGRAM_ID, &artifacts.core, 0xa0, mutable_authority);
    let registry_release = release(REGISTRY_PROGRAM_ID, &artifacts.registry, 0xa1, None);
    let rent_release = release(RENT_PROGRAM_ID, &artifacts.rent, 0xa2, None);
    let trading_release = release(TRADING_PROGRAM_ID, &artifacts.trading, 0xa3, None);
    let claims_release = release(CLAIMS_PROGRAM_ID, &artifacts.core, 0xa4, None);
    let custody_release = release(CUSTODY_PROGRAM_ID, &artifacts.core, 0xa5, None);
    let core_binding = binding(core_release);
    let release_set_value = ExecutionReleaseSetV1::new(
        core_binding,
        binding(claims_release),
        binding(trading_release),
        core_binding,
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
        (ExecutionRoleV1::Resolution, core_release),
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

    let (product, domain, portfolio, linked_basis, outcome_count, stable_product_id) =
        product_graph();
    let realm_value = RealmV1::new(RealmV1Input {
        token_program: dclutch_token_svm::LEGACY_TOKEN_PROGRAM_ID,
        collateral_mint: [0xb2; 32],
        collateral_adapter_release_id: [0xb3; 32],
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    })
    .expect("Realm");
    let realm = Record::new(REALM_SCHEMA_RELEASE_ID_V1, realm_value.to_bytes().to_vec());
    let source_value = SourceMaterialV2::new(
        SourceContentId::new(product.digest).expect("Product root"),
        source_id(0xb4),
        source_id(0xb5),
        source_id(0xb6),
        None,
        source_id(0xb7),
    );
    let source = Record::new(
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
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
    for record in [
        &realm,
        &product,
        &domain,
        &portfolio,
        &linked_basis,
        &source,
        &manifest,
        &release_set,
        &registry_artifact,
        &rent_artifact,
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
    let profile_value =
        ProtocolInfrastructureProfileV1::new(binding(registry_release), binding(rent_release))
            .expect("infrastructure profile");
    let profile = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
        &CORE_PROGRAM_ID,
    )
    .0;
    let profile_data = profile_value.to_bytes().to_vec();
    test.add_account(
        profile,
        Account {
            lamports: Rent::default().minimum_balance(profile_data.len()),
            data: profile_data,
            owner: CORE_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
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
        source,
        manifest,
        release_set,
        cache,
        core_programdata: programdata_address(CORE_PROGRAM_ID),
        trading_programdata: programdata_address(TRADING_PROGRAM_ID),
        claims_programdata: programdata_address(CLAIMS_PROGRAM_ID),
        custody_programdata: programdata_address(CUSTODY_PROGRAM_ID),
        registry_programdata: programdata_address(REGISTRY_PROGRAM_ID),
        rent_programdata: programdata_address(RENT_PROGRAM_ID),
        profile,
        registry_artifact,
        rent_artifact,
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
            AccountMeta::new_readonly(fixture.source.raw, false),
            AccountMeta::new_readonly(fixture.source.staging, false),
            AccountMeta::new_readonly(fixture.manifest.raw, false),
            AccountMeta::new_readonly(fixture.manifest.staging, false),
            AccountMeta::new_readonly(fixture.release_set.raw, false),
            AccountMeta::new_readonly(fixture.release_set.staging, false),
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
        ],
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
    let found = Request::administrative(
        Action::Found,
        GENERATION,
        identity(fixture.market.to_bytes()),
    );
    instruction.data = ProjectFoundRequestV1::new(found)
        .expect("ProjectFound")
        .encode()
        .expect("ProjectFound bytes")
        .to_vec();
    instruction
}

async fn execute(
    mut fixture: Fixture,
    instruction: Instruction,
) -> (Fixture, solana_program_test::ProgramTestContext, bool) {
    let test = fixture.test.take().expect("ProgramTest");
    let context = test.start_with_context().await;
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
    let accepted = context
        .banks_client
        .process_transaction(transaction)
        .await
        .is_ok();
    (fixture, context, accepted)
}

async fn execute_project(
    mut fixture: Fixture,
    instruction: Instruction,
) -> (Fixture, solana_program_test::ProgramTestContext, bool) {
    let test = fixture.test.take().expect("ProgramTest");
    let context = test.start_with_context().await;
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
        AccountMeta::new_readonly(fixture.base.linked_basis.raw, false),
        AccountMeta::new_readonly(fixture.base.linked_basis.staging, false),
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
    Instruction {
        program_id: TRADING_PROGRAM_ID,
        accounts,
        data,
    }
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
    let test = fixture.base.test.take().expect("ProgramTest");
    let context = test.start_with_context().await;
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[series_instruction(&fixture)],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    let failure = submit_and_record(&context, transaction, label).await;
    (fixture, context, failure)
}

/// Submit one already-built transaction and, if asked, record it as evidence.
async fn submit_and_record(
    context: &solana_program_test::ProgramTestContext,
    transaction: Transaction,
    label: &str,
) -> Option<String> {
    let signature = transaction
        .signatures
        .first()
        .copied()
        .expect("a signed transaction has a signature")
        .to_string();
    let outcome = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("the bank processed the transaction");
    let failure = outcome.result.err().map(|error| format!("{error:?}"));
    let slot = context.banks_client.get_root_slot().await.unwrap_or_default();
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
        // This campaign does not measure its wire extent; `None` says so
        // rather than implying the frame fits Solana's packet maximum.
        wire_bytes: None,
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
    let test = fixture.base.test.take().expect("ProgramTest");
    let mut context = test.start_with_context().await;
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let instruction = series_instruction(&fixture);
    let first = submit_and_record(
        &context,
        Transaction::new_signed_with_payer(
            core::slice::from_ref(&instruction),
            Some(&context.payer.pubkey()),
            &[&context.payer],
            blockhash,
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
        Transaction::new_signed_with_payer(
            &[instruction],
            Some(&context.payer.pubkey()),
            &[&context.payer],
            replay_blockhash,
        ),
        replay_label,
    )
    .await;
    (fixture, context, first, replay)
}

#[tokio::test]
async fn real_found31_accepts_258_outcomes_after_immutable_infrastructure_auth() {
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
async fn project_found31_authenticates_without_signature_or_market_mutation() {
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
async fn projected_found31_refuses_swapped_infrastructure_without_market_mutation() {
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

#[tokio::test]
async fn mutable_core_release_refuses_after_profile_init_without_market_write() {
    let fixture = fixture(true);
    let instruction = found_instruction(&fixture, false);
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

#[tokio::test]
async fn series_consume_accepts_258_outcomes_and_commits_found_with_permit() {
    let fixture = series_fixture(SeriesFault::None);
    let (fixture, context, failure) = execute_series(fixture, "Series occurrence Consume founds its Market with a Core permit").await;
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
async fn series_consume_hostile_batch_programdata_refuses_with_byte_exact_rollback() {
    let fixture = series_fixture(SeriesFault::BatchClaimsProgramdata);
    let (fixture, context, failure) = execute_series(fixture, "Series Consume refuses a substituted Claims ProgramData").await;
    let failure = failure.expect("substituted Claims ProgramData must refuse");
    assert!(
        failure.contains("Custom(3)"),
        "Registry batch must expose its exact Deployment refusal, got {failure}"
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
    // Pinned to the exact code, not merely "some refusal". CoreSbfError::Market
    // is 5: the Market PDA, owner, width, phase, or generation refused. That is
    // the honest guard for a replay -- the first Consume moved the Market out of
    // the prestate this occurrence requires, so the second cannot proceed. A
    // refactor that moved the refusal to a different guard has to say so here.
    assert!(
        replay.contains("Custom(5)"),
        "a replayed ticket must be refused by CoreSbfError::Market, got {replay}"
    );
}

#[tokio::test]
async fn series_consume_late_hoard_refusal_rolls_back_found_and_all_replay_state() {
    let fixture = series_fixture(SeriesFault::LateHoardBalance);
    let (fixture, context, failure) = execute_series(fixture, "Series Consume refuses a late Hoard postcondition").await;
    let failure = failure.expect("late Hoard postcondition must refuse");
    assert!(
        failure.contains("Custom(11)"),
        "late postcondition must be CoreSbfError::ChildAck, got {failure}"
    );
    assert_series_found_rollback(&fixture, &context).await;
}
