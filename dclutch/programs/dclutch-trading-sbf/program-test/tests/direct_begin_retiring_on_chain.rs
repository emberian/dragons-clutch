//! `DCLTDBR1` on a real bank, on the real Trading ELF.
//!
//! # Why this file had to exist
//!
//! `direct_begin_retiring_v1` was the only Direct top-level route with no
//! on-chain execution anywhere. It had twelve unit tests across three files and
//! not one of them called `process_direct_begin_retiring_v1`; they exercised
//! `authenticate_market_bytes` and `prepare_retiring_tail` -- two leaf functions
//! -- and the generated route census recorded the entry point as
//! `NEVER-EXECUTED, no stated reason`.
//!
//! That gap is not theoretical on this particular route. Its own source carries
//! the reason at `direct_begin_retiring_v1.rs:673-750`: converting the two
//! Registry reauthentication CPIs into one activation-cache read left LLVM a
//! single call site, it inlined `reauthenticate_roles`, and
//! `process_direct_begin_retiring_v1` went from 3,712 to exactly 4,096 of the
//! 4,096 bytes an SBPF v0 frame gets -- 43 frame-overwrite diagnostics, calls
//! the toolchain says may write over their own locals. `#[inline(never)]` on
//! two functions is the whole of the fix, and an `#[inline(never)]` is a
//! REQUEST: nothing in the type system holds it, and the diagnostic that would
//! report its loss is emitted by a build step that exits zero. A route whose
//! correctness rests on a frame boundary has to be RUN, on the real ELF, or
//! nobody finds out.
//!
//! # What this file asserts
//!
//! * The route EXECUTES: a canonical twenty-account frame lands, and the root
//!   account read back out of the bank carries a `Retiring` Direct tail decoded
//!   by the real codec.
//! * The receipt Trading returns is the exact `DCLTDRR1` the request implies,
//!   joined to the observed post-root digest through
//!   `DirectBeginRetiringReceiptV1::authenticate_for_request` -- not merely
//!   structurally decodable.
//! * The transition is once-only: the same instruction bytes, resubmitted
//!   against the root they just moved, refuse as `Root`.
//! * Six hostile frames, each naming its exact code, and the signer gate is
//!   shown to refuse EARLIER than the content join it would otherwise be
//!   indistinguishable from -- by compute, which is the only evidence available
//!   when two refusals share a discriminant.
//!
//! # What it prints
//!
//! The measured compute of the success arm, at one fixture draw. Reported, not
//! gated: this route makes no CPI and its cost is dominated by PDA searches
//! whose depth is a lottery redrawn by every rebuild (ledger M-61), so a
//! threshold here would be a number about these keys rather than about this
//! route. What matters is that the figure EXISTS, because until this file it
//! did not.

use dclutch_account_profile_contract::ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1;
use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    CapabilityEntryV1, CapabilityManifestV1, CompartmentFundingV1,
    ContentId as CapabilityContentId, FundingAmountsV1, FundingQuoteV1, MANIFEST_HEADER_BYTES,
    MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1, SelectedRecordBumpsV1,
    set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, v4::CapabilityProgramV4,
};
use dclutch_core_contract::ContentId as CoreContentId;
use dclutch_direct_codec::{
    begin_retiring_bundle_v1::{
        direct_begin_retiring_account_profile_schema_v1,
        direct_begin_retiring_descriptor_schema_v1, direct_begin_retiring_effect_schema_v1,
    },
    ordinary_geometry_v3::DirectOrdinaryGeometryV3,
    program_set_v4::build_direct_inline_ordinary_lifecycle_program_set_v1,
    retirement_v1::{
        DIRECT_BEGIN_RETIRING_ACCOUNT_COUNT_V1, DIRECT_BEGIN_RETIRING_ACTIVATION_CACHE_ACCOUNT_V1,
        DIRECT_BEGIN_RETIRING_CONFIG_RAW_ACCOUNT_V1,
        DIRECT_BEGIN_RETIRING_CONFIG_STAGING_ACCOUNT_V1,
        DIRECT_BEGIN_RETIRING_CORE_PROGRAM_ACCOUNT_V1,
        DIRECT_BEGIN_RETIRING_CORE_PROGRAMDATA_ACCOUNT_V1,
        DIRECT_BEGIN_RETIRING_DESCRIPTOR_RAW_ACCOUNT_V1,
        DIRECT_BEGIN_RETIRING_DESCRIPTOR_STAGING_ACCOUNT_V1,
        DIRECT_BEGIN_RETIRING_EFFECT_RAW_ACCOUNT_V1,
        DIRECT_BEGIN_RETIRING_EFFECT_STAGING_ACCOUNT_V1,
        DIRECT_BEGIN_RETIRING_MANIFEST_RAW_ACCOUNT_V1, DIRECT_BEGIN_RETIRING_MARKET_ACCOUNT_V1,
        DIRECT_BEGIN_RETIRING_PROFILE_RAW_ACCOUNT_V1,
        DIRECT_BEGIN_RETIRING_PROFILE_STAGING_ACCOUNT_V1,
        DIRECT_BEGIN_RETIRING_PROGRAM_SET_RAW_ACCOUNT_V1,
        DIRECT_BEGIN_RETIRING_PROGRAM_SET_STAGING_ACCOUNT_V1,
        DIRECT_BEGIN_RETIRING_REGISTRY_ACCOUNT_V1, DIRECT_BEGIN_RETIRING_RENT_ACCOUNT_V1,
        DIRECT_BEGIN_RETIRING_REQUEST_BYTES_V1, DIRECT_BEGIN_RETIRING_ROOT_TOP_ACCOUNT_V1,
        DIRECT_BEGIN_RETIRING_SELECTOR_OFFSET_V1, DIRECT_BEGIN_RETIRING_TRADING_PROGRAM_ACCOUNT_V1,
        DIRECT_BEGIN_RETIRING_TRADING_PROGRAMDATA_ACCOUNT_V1, DirectBeginRetiringReceiptV1,
        DirectBeginRetiringRequestV1, direct_begin_retiring_account_privileges_v1,
        direct_begin_retiring_context_v1,
    },
    successor::{
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1, DIRECT_ROOT_STATE_BYTES_V1, DirectExecutionConfigV1,
        DirectRootPhaseV1, DirectRootStateV1,
    },
};
use dclutch_direct_hot_program_test_support::{
    DIRECT_HOT_FIXTURE_CAPACITY_PROFILE_V5, DirectHotDeploymentWidthsV5,
    build_direct_hot_artifact_fixture_v5,
    waist::{
        CORE_PROGRAM_ID, Elves, REGISTRY_PROGRAM_ID, RefusedExecution, Releases,
        SuccessfulExecution, TRADING_PROGRAM_ID, add_lookup_table, add_release_waist,
        canonical_lookup_addresses, elves, fixture_substrate, program_test_without_forced_budget,
        programdata_v2, start_with_substrate, submit_v0_observed,
    },
};
use dclutch_effect_kernel::v2::SCHEMA_RELEASE_ID as EFFECT_SCHEMA_RELEASE_ID_V2;
use dclutch_market_core_codec::{
    CoreState, Identity as CoreIdentity, MarketCoreStateSeedsV2, MarketIdentity, Phase, Readiness,
    STATE_BYTES, StateBumpsV1,
};
use dclutch_record_contract::{ContentDigest, RecordKeyV1, RecordPdaSeedsV1, SchemaReleaseId};
use dclutch_trading_sbf::TradingSbfError;
use solana_account::Account;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction, InstructionError},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::{
    signature::{Keypair, Signer},
    transaction::TransactionError,
};
use solana_sdk_ids::{compute_budget, sysvar};

/// The Market generation every identity in this fixture binds.
const GENERATION: u64 = 9;
/// The sole manifest entry this Market's activation selected.
const ENTRY_INDEX: u16 = 0;
/// The Direct config's price scale, as the Hot fixture states it.
const PRICE_SCALE: u64 = 100;
/// The Direct config's per-side fee, as the Hot fixture states it.
const FEE_BPS: u16 = 50;
/// A slot far past any bank this test runs at, so the manifest entry's
/// activation deadline is never the thing under examination.
const ACTIVATION_DEADLINE_SLOT: u64 = 1_000_000;
/// Budget requested by the transaction itself, the way a public caller does.
const COMPUTE_LIMIT: u32 = 400_000;

/// The Direct config's fee recipient. It never signs and never appears in the
/// frame; it exists because `DirectExecutionConfigV1` requires a nonzero one.
const FEE_RECIPIENT: Pubkey = Pubkey::new_from_array([0xb1; 32]);
/// An account owned by Trading, carrying byte-identical root contents, at an
/// address the root header's own seeds do not produce.
const FOREIGN_ROOT: Pubkey = Pubkey::new_from_array([0xb2; 32]);

/// The custom program code a refusal carried, so a test can name it rather than
/// assert a bare `is_err()`. Same shape as `direct_hot_fee_pair.rs`.
fn refusal_code(error: &BanksClientError) -> Option<u32> {
    let transaction = match error {
        BanksClientError::TransactionError(value) => value,
        BanksClientError::SimulationError { err, .. } => err,
        _ => return None,
    };
    match transaction {
        TransactionError::InstructionError(_, InstructionError::Custom(code)) => Some(*code),
        _ => None,
    }
}

/// The refusal of a submission that had to refuse.
///
/// `Result::expect_err` is unavailable here: `SuccessfulExecution` carries the
/// whole program log and is deliberately not `Debug`, so a failed expectation
/// could not print itself. This states the same thing, names the arm, and
/// reports what an unexpected success actually cost -- which is the one number
/// worth having when a hostile frame lands.
fn refused(
    outcome: Result<SuccessfulExecution, RefusedExecution>,
    label: &str,
) -> RefusedExecution {
    match outcome {
        Ok(success) => panic!(
            "{label} was expected to refuse and LANDED at {} compute units",
            success.compute_units_consumed,
        ),
        Err(refusal) => refusal,
    }
}

/// One finalized content-addressed record and the two coordinates it occupies.
#[derive(Clone, Debug)]
struct Record {
    raw: Pubkey,
    staging: Pubkey,
    raw_bump: u8,
    staging_bump: u8,
    digest: [u8; 32],
    bytes: Vec<u8>,
}

/// Derive one finalized record's raw/staging pair under the Registry, exactly
/// the way `authenticate_finalized_record` re-derives it.
///
/// The seed tuple is NOT restated here: `dclutch-record-contract` owns the
/// domain constants and exports the constructors that place them, so this
/// reader takes the domain from `seeds.domain()` rather than naming it. A
/// second spelling is a second source of truth (`DOMAIN_RAW_RESTATEMENT`).
fn record(registry: Pubkey, schema: [u8; 32], bytes: Vec<u8>) -> Record {
    let digest = hash(&bytes).to_bytes();
    let key = RecordKeyV1::new(
        SchemaReleaseId::new(schema).expect("schema release id is nonzero"),
        ContentDigest::new(digest).expect("content digest is nonzero"),
    );
    let (raw, raw_bump) = record_address(key.raw_record_pda_seeds(), registry);
    let (staging, staging_bump) = record_address(key.staging_cursor_pda_seeds(), registry);
    Record {
        raw,
        staging,
        raw_bump,
        staging_bump,
        digest,
        bytes,
    }
}

/// One record address and bump from the contract-owned seed material.
fn record_address(seeds: RecordPdaSeedsV1, registry: Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            seeds.domain(),
            seeds.schema_release_id().as_bytes(),
            seeds.expected_digest().as_bytes(),
        ],
        &registry,
    )
}

/// Install one Registry-owned raw record, rent exempt at its exact width.
///
/// The staging cursor is deliberately NOT installed. `authenticate_finalized_record`
/// requires it System-owned with zero data, which is what a nonexistent address
/// is on chain -- so a fixture that created one would be staging a state
/// finalization does not leave behind.
fn install_record(test: &mut ProgramTest, value: &Record) {
    test.add_account(
        value.raw,
        Account {
            lamports: Rent::default().minimum_balance(value.bytes.len()),
            data: value.bytes.clone(),
            owner: REGISTRY_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
}

/// Everything one begin-retiring case needs to submit and to check itself.
struct BeginRetiringCase {
    /// The exact 320-byte `DCLTDBR1` request.
    request_bytes: [u8; DIRECT_BEGIN_RETIRING_REQUEST_BYTES_V1],
    /// The canonical twenty metas, in wire order.
    metas: Vec<AccountMeta>,
    /// The composite Direct root: the sole writable coordinate.
    root: Pubkey,
    /// The root's exact pre-transition bytes.
    root_bytes: Vec<u8>,
    /// The canonical Core Market coordinate.
    market: Pubkey,
}

impl BeginRetiringCase {
    fn instruction(&self) -> Instruction {
        Instruction {
            program_id: TRADING_PROGRAM_ID,
            accounts: self.metas.clone(),
            data: self.request_bytes.to_vec(),
        }
    }

    /// The same frame with one coordinate replaced.
    fn instruction_with(&self, index: usize, meta: AccountMeta) -> Instruction {
        let mut instruction = self.instruction();
        *instruction
            .accounts
            .get_mut(index)
            .expect("begin-retiring coordinate") = meta;
        instruction
    }

    /// The same frame carrying different request bytes.
    fn instruction_with_data(&self, data: Vec<u8>) -> Instruction {
        let mut instruction = self.instruction();
        instruction.data = data;
        instruction
    }
}

/// The transaction a public caller sends: its own budget, then the route.
///
/// The ComputeBudget instruction leads rather than trails. The runtime clears
/// return data at the start of every top-level instruction, so a trailing one
/// would erase the `DCLTDRR1` receipt this route's whole evidence rests on --
/// the transaction would succeed and its receipt would be gone.
fn transaction_instructions(instruction: Instruction, limit: u32) -> [Instruction; 2] {
    [
        Instruction {
            program_id: compute_budget::ID,
            accounts: Vec::new(),
            data: {
                let mut data = vec![2];
                data.extend_from_slice(&limit.to_le_bytes());
                data
            },
        },
        instruction,
    ]
}

/// Build the complete begin-retiring world and install every account it needs.
///
/// `phase` is the Core Market's phase, and it is the ONE input: the Market
/// address is seeded by its identity, which does not carry the phase, so the
/// `Open` and `Retiring` worlds are the same twenty addresses and the same root.
/// Everything else the request states is derived from the bytes actually
/// installed, so the not-`Retiring` case differs from the canonical one in
/// exactly the fact under examination.
fn build_case(
    test: &mut ProgramTest,
    releases: Releases,
    artifacts: &Elves,
    phase: Phase,
) -> BeginRetiringCase {
    let rent = Rent::default();
    let substrate = fixture_substrate();
    let widths = DirectHotDeploymentWidthsV5::new(
        programdata_v2(substrate, &artifacts.trading).len(),
        programdata_v2(substrate, &artifacts.claims).len(),
        programdata_v2(substrate, &artifacts.core).len(),
    )
    .expect("real Direct deployment widths");

    // The ordinary Direct release this Market runs, and the four-selector
    // lifecycle ProgramSet built from it. The begin-retiring descriptor is not
    // authored here: it is DERIVED from the ordinary bundle by its semantic
    // owner, which is what binds its kind, capacity profile, root schema and
    // derivation policy to the ones the manifest entry names.
    let fixture = build_direct_hot_artifact_fixture_v5(widths, DirectOrdinaryGeometryV3::CANONICAL)
        .expect("canonical Direct artifact fixture");
    let release = build_direct_inline_ordinary_lifecycle_program_set_v1(
        fixture.bundle,
        DIRECT_HOT_FIXTURE_CAPACITY_PROFILE_V5,
    )
    .expect("ordinary + begin-retiring + close + activation ProgramSet");
    let ordinary = CapabilityProgramV4::decode(&release.ordinary.descriptor)
        .expect("canonical ordinary descriptor");

    let config = DirectExecutionConfigV1::new(PRICE_SCALE, FEE_BPS, FEE_RECIPIENT.to_bytes())
        .expect("Direct execution config")
        .encode();
    let config_record = record(
        REGISTRY_PROGRAM_ID,
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
        config.to_vec(),
    );
    let program_set_record = record(
        REGISTRY_PROGRAM_ID,
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        release.program_set.clone(),
    );
    let descriptor_record = record(
        REGISTRY_PROGRAM_ID,
        direct_begin_retiring_descriptor_schema_v1(),
        release.begin_retiring.descriptor.clone(),
    );
    let profile_record = record(
        REGISTRY_PROGRAM_ID,
        direct_begin_retiring_account_profile_schema_v1(),
        release.begin_retiring.account_profile.clone(),
    );
    let effect_record = record(
        REGISTRY_PROGRAM_ID,
        direct_begin_retiring_effect_schema_v1(),
        release.begin_retiring.effect.clone(),
    );
    // The schemas the route re-derives these two pairs under are the general
    // ones, not begin-retiring's own names for them. Held here so a rename on
    // either side is a compile-time argument rather than a silent address move.
    assert_eq!(
        direct_begin_retiring_account_profile_schema_v1(),
        ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1,
    );
    assert_eq!(
        direct_begin_retiring_effect_schema_v1(),
        EFFECT_SCHEMA_RELEASE_ID_V2,
    );
    assert_eq!(
        program_set_record.digest, release.program_set_id,
        "the ProgramSet record is not the set the manifest entry names",
    );
    assert_eq!(
        descriptor_record.digest,
        release.begin_retiring.descriptor_id
    );
    assert_eq!(
        profile_record.digest,
        release.begin_retiring.account_profile_id
    );
    assert_eq!(effect_record.digest, release.begin_retiring.effect_id);

    // The manifest the Market's activation read, with the one entry the root's
    // selection names. Its coordinates come from the ordinary descriptor
    // because that is what a founding selects; the begin-retiring descriptor
    // inherits every one of them and `validate_selection` rejoins them.
    let content = |value: [u8; 32]| CapabilityContentId::new(value).expect("nonzero identity");
    let amounts = FundingAmountsV1::new(
        CompartmentFundingV1::native_lamports(1).expect("native funding"),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
    )
    .expect("funding amounts");
    let entry = CapabilityEntryV1::new(
        content(ordinary.kind().to_bytes()),
        content(release.program_set_id),
        content(config_record.digest),
        content(ordinary.capacity_profile().to_bytes()),
        content(ordinary.root_schema().to_bytes()),
        content(ordinary.derivation_policy().to_bytes()),
        ActivationPolicy::PrepaidLazy,
        ACTIVATION_DEADLINE_SLOT,
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        FundingQuoteV1::new(amounts, None).expect("funding quote"),
    )
    .expect("canonical manifest entry");
    let mut manifest = vec![0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&[entry], &mut manifest).expect("canonical manifest");
    let manifest_record = record(
        REGISTRY_PROGRAM_ID,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        manifest,
    );

    let selection = dclutch_release_set_contract::CapabilityExecutionSelectionV1::new(
        ENTRY_INDEX,
        content(manifest_record.digest),
        content(ordinary.kind().to_bytes()),
        content(release.program_set_id),
        content(config_record.digest),
    )
    .expect("persisted activation selection")
    .with_capability_release_record_bumps(
        program_set_record.raw_bump,
        program_set_record.staging_bump,
    );

    // The canonical Core Market. Its address is seeded by the identity, which
    // carries the manifest the root selects -- so the manifest has to exist
    // before the Market does, and the Market before the root.
    let core_identity = |value: [u8; 32]| CoreIdentity::new(value).expect("nonzero Core identity");
    let provisional = MarketIdentity {
        market_id: core_identity([0x71; 32]),
        realm_id: core_identity([0x72; 32]),
        product_record: core_identity([0x73; 32]),
        product_id: core_identity([0x74; 32]),
        resolution_policy: core_identity([0x75; 32]),
        capability_manifest: core_identity(manifest_record.digest),
        selected_release_set: core_identity(releases.release_set),
        registry_program: core_identity(REGISTRY_PROGRAM_ID.to_bytes()),
        generation: GENERATION,
    };
    let market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(provisional).as_slices(),
        &CORE_PROGRAM_ID,
    )
    .0;
    let identity = MarketIdentity {
        market_id: core_identity(market.to_bytes()),
        ..provisional
    };
    let (founding_market, market_bump) = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(identity).as_slices(),
        &CORE_PROGRAM_ID,
    );
    assert_eq!(
        founding_market, market,
        "the Market seeds do not reproduce the address they derived",
    );
    let retiring = phase == Phase::Retiring;
    let market_bytes = CoreState {
        phase,
        readiness: Readiness::Consumed,
        terminal_winner: if retiring { 1 } else { 0 },
        identity,
        outstanding_capabilities: 1,
        principal_cap_sets: u64::MAX,
        rent_beneficiary: core_identity([0x76; 32]),
        // A canonical `Open` Market has no terminal receipt and a canonical
        // `Retiring` one must have one; `CoreState::valid_static` refuses the
        // other pairings, so this is not a choice the fixture gets to make.
        terminal_receipt: if retiring {
            Some(core_identity([0x77; 32]))
        } else {
            None
        },
        bumps: StateBumpsV1 {
            market: StateBumpsV1::record(market_bump),
            realm_raw_record: None,
            realm_staging_record: None,
            ..StateBumpsV1::UNRECORDED
        },
    }
    .encode()
    .expect("canonical Core Market state")
    .to_vec();
    assert_eq!(market_bytes.len(), STATE_BYTES);
    test.add_account(
        market,
        Account {
            lamports: rent.minimum_balance(market_bytes.len()),
            data: market_bytes.clone(),
            owner: CORE_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    // The composite root: the immutable activation header, then the mutable
    // 24-byte Direct tail, `Open` with no maker roots outstanding.
    let header = CapabilityRootHeaderV1::new(
        CoreContentId::new(releases.release_set).expect("release set identity"),
        market.to_bytes(),
        GENERATION,
        selection,
        SelectedRecordBumpsV1::new(
            manifest_record.raw_bump,
            manifest_record.staging_bump,
            config_record.raw_bump,
            config_record.staging_bump,
        ),
    )
    .expect("immutable root header");
    let mut root_bytes =
        Vec::with_capacity(CAPABILITY_ROOT_HEADER_BYTES_V1 + DIRECT_ROOT_STATE_BYTES_V1);
    root_bytes.extend_from_slice(&header.to_bytes());
    root_bytes.extend_from_slice(&DirectRootStateV1::new().encode());
    let root = Pubkey::find_program_address(&header.seeds().as_slices(), &TRADING_PROGRAM_ID).0;
    let root_account = Account {
        lamports: rent.minimum_balance(root_bytes.len()),
        data: root_bytes.clone(),
        owner: TRADING_PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };
    test.add_account(root, root_account.clone());
    // The substituted root of the hostile arm: the same bytes, the same owner,
    // an address the header's seeds do not produce. Installed unconditionally
    // so the canonical and hostile banks are otherwise identical.
    test.add_account(FOREIGN_ROOT, root_account);

    for value in [
        &manifest_record,
        &program_set_record,
        &descriptor_record,
        &config_record,
        &profile_record,
        &effect_record,
    ] {
        install_record(test, value);
    }

    let request = DirectBeginRetiringRequestV1 {
        release_set: releases.release_set,
        market: market.to_bytes(),
        context: direct_begin_retiring_context_v1(
            releases.release_set,
            market.to_bytes(),
            root.to_bytes(),
            manifest_record.digest,
            release.program_set_id,
            config_record.digest,
            GENERATION,
            ENTRY_INDEX,
        ),
        root: root.to_bytes(),
        manifest: manifest_record.digest,
        program_set: release.program_set_id,
        config: config_record.digest,
        expected_market_digest: hash(&market_bytes).to_bytes(),
        expected_root_digest: hash(&root_bytes).to_bytes(),
        generation: GENERATION,
        entry_index: ENTRY_INDEX,
    }
    .to_bytes()
    .expect("canonical begin-retiring request");

    let mut metas = vec![
        AccountMeta::new_readonly(Pubkey::default(), false);
        DIRECT_BEGIN_RETIRING_ACCOUNT_COUNT_V1
    ];
    let mut put = |index: usize, key: Pubkey| {
        let (writable, _) =
            direct_begin_retiring_account_privileges_v1(index).expect("coordinate privileges");
        *metas.get_mut(index).expect("coordinate") = AccountMeta {
            pubkey: key,
            is_signer: false,
            is_writable: writable,
        };
    };
    put(DIRECT_BEGIN_RETIRING_ROOT_TOP_ACCOUNT_V1, root);
    put(DIRECT_BEGIN_RETIRING_MARKET_ACCOUNT_V1, market);
    put(
        DIRECT_BEGIN_RETIRING_MANIFEST_RAW_ACCOUNT_V1,
        manifest_record.raw,
    );
    put(
        DIRECT_BEGIN_RETIRING_PROGRAM_SET_RAW_ACCOUNT_V1,
        program_set_record.raw,
    );
    put(
        DIRECT_BEGIN_RETIRING_PROGRAM_SET_STAGING_ACCOUNT_V1,
        program_set_record.staging,
    );
    put(
        DIRECT_BEGIN_RETIRING_DESCRIPTOR_RAW_ACCOUNT_V1,
        descriptor_record.raw,
    );
    put(
        DIRECT_BEGIN_RETIRING_DESCRIPTOR_STAGING_ACCOUNT_V1,
        descriptor_record.staging,
    );
    put(
        DIRECT_BEGIN_RETIRING_CONFIG_RAW_ACCOUNT_V1,
        config_record.raw,
    );
    put(
        DIRECT_BEGIN_RETIRING_CONFIG_STAGING_ACCOUNT_V1,
        config_record.staging,
    );
    put(
        DIRECT_BEGIN_RETIRING_PROFILE_RAW_ACCOUNT_V1,
        profile_record.raw,
    );
    put(
        DIRECT_BEGIN_RETIRING_PROFILE_STAGING_ACCOUNT_V1,
        profile_record.staging,
    );
    put(
        DIRECT_BEGIN_RETIRING_EFFECT_RAW_ACCOUNT_V1,
        effect_record.raw,
    );
    put(
        DIRECT_BEGIN_RETIRING_EFFECT_STAGING_ACCOUNT_V1,
        effect_record.staging,
    );
    put(
        DIRECT_BEGIN_RETIRING_ACTIVATION_CACHE_ACCOUNT_V1,
        releases.activation,
    );
    put(
        DIRECT_BEGIN_RETIRING_CORE_PROGRAM_ACCOUNT_V1,
        CORE_PROGRAM_ID,
    );
    put(
        DIRECT_BEGIN_RETIRING_CORE_PROGRAMDATA_ACCOUNT_V1,
        releases.core_programdata,
    );
    put(
        DIRECT_BEGIN_RETIRING_TRADING_PROGRAM_ACCOUNT_V1,
        TRADING_PROGRAM_ID,
    );
    put(
        DIRECT_BEGIN_RETIRING_TRADING_PROGRAMDATA_ACCOUNT_V1,
        releases.trading_programdata,
    );
    put(
        DIRECT_BEGIN_RETIRING_REGISTRY_ACCOUNT_V1,
        REGISTRY_PROGRAM_ID,
    );
    put(DIRECT_BEGIN_RETIRING_RENT_ACCOUNT_V1, sysvar::rent::ID);

    for (index, meta) in metas.iter().enumerate() {
        assert_ne!(
            meta.pubkey,
            Pubkey::default(),
            "begin-retiring coordinate {index} was never filled in",
        );
        assert!(
            !meta.is_signer,
            "the route admits no signer at any coordinate"
        );
    }

    BeginRetiringCase {
        request_bytes: request,
        metas,
        root,
        root_bytes,
        market,
    }
}

async fn account(context: &mut ProgramTestContext, key: Pubkey) -> Account {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account read")
        .expect("live account")
}

/// The whole point of the file: `process_direct_begin_retiring_v1` executes.
///
/// # What lands, and what it proves
///
/// The transaction is the one a public caller sends -- its own compute budget,
/// then a bare twenty-account `DCLTDBR1` instruction straight to Trading. No
/// Registry outer, no continuation, no signer anywhere in the frame.
///
/// Three assertions carry the weight, and none of them can pass without the
/// route having run:
///
/// * the root account READ BACK OUT OF THE BANK decodes, through
///   `DirectRootStateV1`, to a `Retiring` tail. It was `Open` before the
///   transaction and this test holds the pre-image, so the transition is
///   observed rather than assumed;
/// * the immutable 232-byte activation header is byte-identical across the
///   transition. The route may move exactly 24 bytes and this says so;
/// * the returned `DCLTDRR1` receipt passes
///   `authenticate_for_request(request_bytes, observed_post_root_digest,
///   trading_program)`, which rejoins every coordinate to the request AND to
///   the digest of the account this test just read. A receipt that decoded but
///   described another poststate fails here.
///
/// Then the same instruction is resubmitted. It refuses as `Root`, because the
/// root digest it names is the one that no longer exists -- which is the same
/// fact as the first three, stated from the other side.
#[tokio::test]
async fn direct_begin_retiring_v1_executes_and_retires_the_root() {
    let artifacts = elves();
    let mut test = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let case = build_case(&mut test, releases, &artifacts, Phase::Retiring);

    let instructions = transaction_instructions(case.instruction(), COMPUTE_LIMIT);
    let replay = transaction_instructions(case.instruction(), COMPUTE_LIMIT - 1);
    let mut addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
    addresses.retain(|key| *key != FOREIGN_ROOT);
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;

    let before = account(&mut context, case.root).await;
    assert_eq!(before.data, case.root_bytes, "staged root prestate");
    assert_eq!(
        DirectRootStateV1::decode(
            before
                .data
                .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
                .expect("Direct tail")
        )
        .expect("prestate Direct tail")
        .phase(),
        DirectRootPhaseV1::Open,
        "the root must be Open before this route runs, or it proves nothing",
    );

    let execution = submit_v0_observed(&mut context, &instructions, addresses.clone(), None, &[])
        .await
        .expect("top-level DCLTDBR1 execution");

    let units = execution.compute_units_consumed;
    assert!(
        units > 0 && units <= u64::from(COMPUTE_LIMIT),
        "begin-retiring consumed {units} against a {COMPUTE_LIMIT} request",
    );
    // Reported, not gated. This route makes no CPI; its cost is dominated by
    // eleven PDA searches whose depths are a lottery this fixture's identities
    // draw once, so a threshold here would describe these keys.
    println!("BEGINRETIRING compute units consumed: {units}");

    let after = account(&mut context, case.root).await;
    assert_eq!(
        after.data.len(),
        case.root_bytes.len(),
        "the route resized the root",
    );
    assert_eq!(after.owner, TRADING_PROGRAM_ID);
    assert_eq!(after.lamports, before.lamports, "the route moved lamports");
    assert_eq!(
        after.data.get(..CAPABILITY_ROOT_HEADER_BYTES_V1),
        before.data.get(..CAPABILITY_ROOT_HEADER_BYTES_V1),
        "the immutable activation header moved",
    );
    let tail = after
        .data
        .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .expect("Direct tail");
    assert_eq!(tail.len(), DIRECT_ROOT_STATE_BYTES_V1);
    let post = DirectRootStateV1::decode(tail).expect("poststate Direct tail");
    assert_eq!(post.phase(), DirectRootPhaseV1::Retiring);
    assert_eq!(post.open_maker_root_count(), 0);
    assert_ne!(after.data, before.data, "the root did not change at all");

    // The Market is read-only on this route and must come out untouched.
    let market = account(&mut context, case.market).await;
    assert_eq!(market.owner, CORE_PROGRAM_ID);
    assert_eq!(
        CoreState::decode(&market.data).expect("post Market").phase,
        Phase::Retiring,
    );

    let (producer, returned) = execution
        .return_data
        .expect("a successful begin-retiring must return its DCLTDRR1 receipt");
    assert_eq!(
        producer, TRADING_PROGRAM_ID,
        "receipt producer substitution"
    );
    let receipt = DirectBeginRetiringReceiptV1::decode(&returned).expect("canonical receipt");
    let authenticated = receipt
        .authenticate_for_request(
            &case.request_bytes,
            hash(&after.data).to_bytes(),
            TRADING_PROGRAM_ID.to_bytes(),
        )
        .expect("the receipt must rejoin the request and the observed poststate");
    assert_eq!(authenticated, receipt);
    assert_eq!(receipt.root, case.root.to_bytes());
    assert_eq!(receipt.market, case.market.to_bytes());
    assert_eq!(receipt.pre_root_digest, hash(&before.data).to_bytes());
    assert_eq!(receipt.post_root_digest, hash(&after.data).to_bytes());
    assert_eq!(receipt.request_digest, hash(&case.request_bytes).to_bytes());
    assert_eq!(receipt.generation, GENERATION);
    assert_eq!(receipt.entry_index, ENTRY_INDEX);

    // Once only. The identical request now names a root digest the chain no
    // longer carries, and the route refuses before it reaches any content.
    let refusal = refused(
        submit_v0_observed(&mut context, &replay, addresses, None, &[]).await,
        "a replayed begin-retiring request",
    );
    assert_eq!(
        refusal_code(&refusal.error),
        Some(TradingSbfError::Root as u32),
        "replay refused as something other than Root: {:#?}",
        refusal.logs,
    );
    let unchanged = account(&mut context, case.root).await;
    assert_eq!(
        unchanged.data, after.data,
        "a refused replay moved the root"
    );
}

/// Five hostile frames, each naming its exact refusal code.
///
/// A substituted root, a signer in the frame, a foreign key at a content
/// coordinate, a request one byte short of its one exact width, and a request
/// whose selector names another action. They share one bank because none of
/// them changes chain state -- every arm refuses, and the last assertion is
/// that the root is byte-identical to the prestate after all five.
///
/// # The signer arm, and why compute is the evidence
///
/// `Accounts::parse` refuses a signer at ANY coordinate, and it does so in its
/// first statement, before the per-index privilege table and before any key is
/// compared. Every refusal in this route carries `Content`, so the discriminant
/// cannot distinguish "refused because it was a signer" from "refused because
/// that key does not belong at that coordinate" -- and the second is true of
/// the substituted key either way, because there is no way to make a PDA sign.
///
/// So the arm is run TWICE with the same substituted key at the same
/// coordinate, once signing and once not, and the claim is that the signing one
/// refuses for STRICTLY LESS compute. That separates the frame gate from the
/// content join by the only evidence a shared discriminant leaves behind.
#[tokio::test]
async fn direct_begin_retiring_v1_refuses_hostile_frames() {
    let artifacts = elves();
    let mut test = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let case = build_case(&mut test, releases, &artifacts, Phase::Retiring);
    let bystander = Keypair::new();

    // A substituted root: byte-identical contents, Trading-owned, at an address
    // the root header's own seeds do not produce.
    let foreign_root = case.instruction_with(
        DIRECT_BEGIN_RETIRING_ROOT_TOP_ACCOUNT_V1,
        AccountMeta::new(FOREIGN_ROOT, false),
    );
    // The same key at the manifest coordinate, signing and not signing.
    let signer = case.instruction_with(
        DIRECT_BEGIN_RETIRING_MANIFEST_RAW_ACCOUNT_V1,
        AccountMeta::new_readonly(bystander.pubkey(), true),
    );
    let bystanding = case.instruction_with(
        DIRECT_BEGIN_RETIRING_MANIFEST_RAW_ACCOUNT_V1,
        AccountMeta::new_readonly(bystander.pubkey(), false),
    );
    // One byte short of the one exact request width.
    let mut narrow = case.request_bytes.to_vec();
    narrow.pop();
    let narrow = case.instruction_with_data(narrow);
    // Exact width, exact magic, a selector that names another action.
    let mut wrong_selector = case.request_bytes;
    *wrong_selector
        .get_mut(DIRECT_BEGIN_RETIRING_SELECTOR_OFFSET_V1)
        .expect("selector byte") ^= 1;
    let wrong_selector = case.instruction_with_data(wrong_selector.to_vec());

    let arms = [
        transaction_instructions(foreign_root, COMPUTE_LIMIT),
        transaction_instructions(signer, COMPUTE_LIMIT),
        transaction_instructions(bystanding, COMPUTE_LIMIT),
        transaction_instructions(narrow, COMPUTE_LIMIT),
        transaction_instructions(wrong_selector, COMPUTE_LIMIT),
    ];
    let mut addresses = Vec::new();
    for arm in &arms {
        addresses.extend(canonical_lookup_addresses(arm, Pubkey::default()));
    }
    addresses.retain(|key| *key != bystander.pubkey());
    addresses.sort_unstable_by_key(Pubkey::to_bytes);
    addresses.dedup();
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;
    let before = account(&mut context, case.root).await;
    assert_eq!(before.data, case.root_bytes);

    // Arm 1 is the only one carrying a signer, so it is the only one whose
    // transaction needs a second signature.
    let mut outcomes = Vec::with_capacity(arms.len());
    for (index, arm) in arms.iter().enumerate() {
        let signers: Vec<&Keypair> = if index == 1 {
            vec![&bystander]
        } else {
            Vec::new()
        };
        outcomes.push(refused(
            submit_v0_observed(&mut context, arm, addresses.clone(), None, &signers).await,
            "a hostile begin-retiring frame",
        ));
    }

    let named = |value: &RefusedExecution, expected: TradingSbfError, label: &str| {
        assert_eq!(
            refusal_code(&value.error),
            Some(expected as u32),
            "{label} refused with the wrong code: {:#?}",
            value.logs,
        );
    };
    let foreign = outcomes.first().expect("foreign-root outcome");
    let signer = outcomes.get(1).expect("signer outcome");
    let bystanding = outcomes.get(2).expect("non-signing outcome");
    let narrow = outcomes.get(3).expect("narrow outcome");
    let selector = outcomes.get(4).expect("selector outcome");

    named(foreign, TradingSbfError::Root, "a substituted root");
    named(signer, TradingSbfError::Content, "a signer in the frame");
    named(
        bystanding,
        TradingSbfError::Content,
        "a foreign manifest coordinate",
    );
    named(narrow, TradingSbfError::Content, "a short request");
    named(selector, TradingSbfError::Content, "a substituted selector");

    assert!(
        signer.compute_units_consumed < bystanding.compute_units_consumed,
        "the signer gate did not refuse before the content join: signing cost {} \
         units and the identical non-signing frame cost {}",
        signer.compute_units_consumed,
        bystanding.compute_units_consumed,
    );

    let after = account(&mut context, case.root).await;
    assert_eq!(
        after.data, before.data,
        "a refused begin-retiring frame moved the root",
    );
}

/// The route authenticates an already-`Retiring` Core Market, and nothing else.
///
/// This is the property the twelve existing unit tests assert against
/// `authenticate_market_bytes` in isolation. Here it is asserted against the
/// entry point, on a bank, with a canonical `Open` Market at the canonical
/// address -- the same twenty coordinates, the same root, the same request
/// except for the market digest it correctly states. The phase is the only
/// difference, and it is enough.
#[tokio::test]
async fn direct_begin_retiring_v1_refuses_a_market_that_is_not_retiring() {
    let artifacts = elves();
    let mut test = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let case = build_case(&mut test, releases, &artifacts, Phase::Open);

    let instructions = transaction_instructions(case.instruction(), COMPUTE_LIMIT);
    let mut addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
    addresses.retain(|key| *key != FOREIGN_ROOT);
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;

    let market = account(&mut context, case.market).await;
    assert_eq!(
        CoreState::decode(&market.data)
            .expect("staged Market")
            .phase,
        Phase::Open,
        "this arm is only about the phase, so the phase has to be the staged one",
    );

    let refusal = refused(
        submit_v0_observed(&mut context, &instructions, addresses, None, &[]).await,
        "an Open Market beginning retirement",
    );
    assert_eq!(
        refusal_code(&refusal.error),
        Some(TradingSbfError::Content as u32),
        "a non-Retiring Market refused with the wrong code: {:#?}",
        refusal.logs,
    );

    let root = account(&mut context, case.root).await;
    assert_eq!(root.data, case.root_bytes, "a refused frame moved the root");
    assert_eq!(
        DirectRootStateV1::decode(
            root.data
                .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
                .expect("Direct tail")
        )
        .expect("Direct tail")
        .phase(),
        DirectRootPhaseV1::Open,
    );
}

/// The membrane the codec publishes is the membrane this file submits.
///
/// Not a restatement of the codec's own unit test: that one counts writables
/// and executables in the abstract, this one holds the twenty metas actually
/// sent to the bank against it, coordinate by coordinate. A frame built by hand
/// that drifted from the table would refuse on chain with `Content` and look
/// exactly like a protocol refusal.
#[tokio::test]
async fn the_submitted_frame_matches_the_published_privilege_table() {
    let artifacts = elves();
    let mut test = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let case = build_case(&mut test, releases, &artifacts, Phase::Retiring);
    let instruction = case.instruction();

    assert_eq!(
        instruction.accounts.len(),
        DIRECT_BEGIN_RETIRING_ACCOUNT_COUNT_V1
    );
    assert_eq!(
        instruction.data.len(),
        DIRECT_BEGIN_RETIRING_REQUEST_BYTES_V1
    );
    // The three the table says are executable, and there is no fourth: a
    // membrane that admitted an extra program account would admit an extra
    // thing to be substituted.
    let executables = [CORE_PROGRAM_ID, TRADING_PROGRAM_ID, REGISTRY_PROGRAM_ID];
    for (index, meta) in instruction.accounts.iter().enumerate() {
        let (writable, executable) =
            direct_begin_retiring_account_privileges_v1(index).expect("published privileges");
        assert!(!meta.is_signer, "coordinate {index} signs");
        assert_eq!(meta.is_writable, writable, "coordinate {index} writability");
        assert_eq!(
            executables.contains(&meta.pubkey),
            executable,
            "coordinate {index} executability",
        );
        assert!(
            instruction
                .accounts
                .get(index.saturating_add(1)..)
                .is_some_and(|suffix| !suffix.iter().any(|other| other.pubkey == meta.pubkey)),
            "coordinate {index} aliases a later coordinate",
        );
    }
    assert_eq!(
        instruction
            .accounts
            .iter()
            .filter(|meta| meta.is_writable)
            .map(|meta| meta.pubkey)
            .collect::<Vec<_>>(),
        vec![case.root],
        "the root is the sole writable coordinate",
    );
}
