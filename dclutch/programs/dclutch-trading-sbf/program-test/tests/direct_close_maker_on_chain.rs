//! `DCLTDMC1` on a real bank, on the real Trading ELF: wall 22's missing
//! decrement, driven end to end.
//!
//! # What this file proves
//!
//! The complete sequence the wall could never finish, on chain:
//!
//! * a Direct root with a STANDING maker root begins retiring -- the
//!   intentional flip of the old count gate (cohort-9 review item 1,
//!   amendment 1), proven here on the real ELF rather than in a leaf test;
//! * the maker replay then closes INSIDE Retiring: the count decrements 1 -> 0
//!   through the released transition's own `nonzero` + `sub_into`, the whole
//!   observed balance (principal plus donation) lands on the immutably
//!   recorded `rent_owner`, and the replay account returns to the System
//!   program;
//! * the drained root now passes the exact gate that stopped wall 22: the
//!   RELEASED native-close transition bytecode -- the same
//!   `scalar_eq(count, 0)` the physical close runs -- accepts the drained
//!   tail and refuses the standing one, executed here from the release bytes
//!   the market actually selected;
//! * a second close of the same replay refuses by absence
//!   (`CloseMakerReplayAccount`), which is the whole double-close story;
//! * the refusals name their codes: a debtor's replay refuses
//!   `CloseMakerFeeOutstanding` -- the replay is the sole record of the
//!   FEE-TX2 receivable, so the close must not erase it -- and a replay with
//!   registered live intents refuses `CloseMakerLiveIntents`; a substituted
//!   rent destination refuses `CloseMakerFrame` before anything moves.

use dclutch_account_profile_contract::ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1;
use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    CapabilityEntryV1, CapabilityManifestV1, CompartmentFundingV1,
    ContentId as CapabilityContentId, FundingAmountsV1, FundingQuoteV1, MANIFEST_HEADER_BYTES,
    MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1, SelectedRecordBumpsV1,
    activation_registers_v2::{ACTIVATION_ACTION_SCALAR_V2, ACTIVATION_FIRST_FAMILY_SCALAR_V2},
    set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
    v4::CapabilityProgramV4,
};
use dclutch_core_contract::ContentId as CoreContentId;
use dclutch_direct_codec::{
    begin_retiring_bundle_v1::{
        direct_begin_retiring_account_profile_schema_v1,
        direct_begin_retiring_descriptor_schema_v1, direct_begin_retiring_effect_schema_v1,
    },
    close_maker_bundle_v1::{
        direct_close_maker_account_profile_schema_v1, direct_close_maker_descriptor_schema_v1,
        direct_close_maker_effect_schema_v1,
    },
    close_maker_v1::{
        DIRECT_CLOSE_MAKER_ACCOUNT_COUNT_V1, DIRECT_CLOSE_MAKER_RENT_OWNER_ACCOUNT_V1,
        DIRECT_CLOSE_MAKER_REPLAY_ACCOUNT_V1, DirectCloseMakerReceiptV1, DirectCloseMakerRequestV1,
        direct_close_maker_account_privileges_v1,
    },
    intent_v2::CompactIntentV2,
    ordinary_bundle_v4::DirectInlineOrdinaryHotBundleV4,
    ordinary_geometry_v3::DirectOrdinaryGeometryV3,
    program_set_v4::build_direct_inline_ordinary_lifecycle_program_set_v1,
    retirement_v1::{
        DIRECT_BEGIN_RETIRING_ACCOUNT_COUNT_V1, DirectBeginRetiringRequestV1,
        direct_begin_retiring_account_privileges_v1, direct_begin_retiring_context_v1,
    },
    successor::{
        AuthenticatedIntentReplayV2, DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
        DIRECT_ROOT_STATE_BYTES_V1, DirectCoordinatesV1, DirectExecutionConfigV1,
        DirectRootPhaseV1, DirectRootStateLayoutV1, DirectRootStateV1, MakerReplayFirstUseV1,
        MakerReplayObservationV1, MakerReplaySeedsV1, MakerReplayVacancyV1, NonceConsumptionV2,
        consume_nonce_v2,
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
use dclutch_market_core_codec::CoreEffectActionV1;
use dclutch_market_core_codec::{
    CoreState, Identity as CoreIdentity, MarketCoreStateSeedsV2, MarketIdentity, Phase, Readiness,
    STATE_BYTES, StateBumpsV1,
};
use dclutch_operator::{
    Finality, Observation, ObservedAccount,
    direct_close_maker_v1::{
        DirectCloseMakerClusterV1, DirectCloseMakerPlanErrorV1, DirectCloseMakerPlanV1,
        DirectCloseMakerSnapshotV1, plan_direct_close_maker_v1,
    },
};
use dclutch_record_contract::{ContentDigest, RecordKeyV1, RecordPdaSeedsV1, SchemaReleaseId};
use dclutch_trading_sbf::TradingSbfError;
use dclutch_transition_vm::v2::{
    ProgramV2 as TransitionProgramV2, RegisterInput, RegisterOutput, execute_atomic,
};
use solana_account::Account;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction, InstructionError},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::transaction::TransactionError;
use solana_sdk_ids::{compute_budget, sysvar};

/// The Market generation every identity in this fixture binds.
const GENERATION: u64 = 9;
/// The sole manifest entry this Market's activation selected.
const ENTRY_INDEX: u16 = 0;
/// The Direct config's price scale, as the Hot fixture states it.
const PRICE_SCALE: u64 = 100;
/// The Direct config's per-side fee, as the Hot fixture states it.
const FEE_BPS: u16 = 50;
/// A slot far past any bank this test runs at.
const ACTIVATION_DEADLINE_SLOT: u64 = 1_000_000;
/// Budget requested by the transaction itself, the way a public caller does.
const COMPUTE_LIMIT: u32 = 400_000;

/// The Direct config's fee recipient; required nonzero, never in a frame.
const FEE_RECIPIENT: Pubkey = Pubkey::new_from_array([0xb1; 32]);
/// The clean maker whose replay drains and closes.
const MAKER: Pubkey = Pubkey::new_from_array([0xa1; 32]);
/// The debtor maker: `fee_owed` nonzero, close must refuse by name.
const DEBTOR_MAKER: Pubkey = Pubkey::new_from_array([0xa2; 32]);
/// The maker with a registered live intent still standing.
const LIVE_MAKER: Pubkey = Pubkey::new_from_array([0xa3; 32]);
/// The clean maker's immutably recorded rent owner.
const RENT_OWNER: Pubkey = Pubkey::new_from_array([0xc1; 32]);
/// A plain System wallet that is NOT the recorded rent owner.
const STRANGER_WALLET: Pubkey = Pubkey::new_from_array([0xc2; 32]);
/// Lamports above rent principal on the clean replay: the donation slice.
const DONATION: u64 = 11;
/// The unsettled fee the debtor's replay records.
const DEBT: u64 = 4;

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

/// One installed maker replay and the coordinates its close needs.
struct InstalledReplay {
    address: Pubkey,
    maker: Pubkey,
    rent_owner: Pubkey,
    rent_principal: u64,
    lamports: u64,
}

/// Fabricate one maker replay's exact persisted bytes through the SAME
/// semantic function the fill runs (`consume_nonce_v2` first use), so the
/// fixture cannot drift into a mirror of the layout.
#[allow(clippy::too_many_arguments)]
fn install_replay(
    test: &mut ProgramTest,
    market: Pubkey,
    maker: Pubkey,
    rent_owner: Pubkey,
    donation: u64,
    fee_owed: u64,
    registered: bool,
) -> InstalledReplay {
    let coordinates =
        DirectCoordinatesV1::new(market.to_bytes(), GENERATION).expect("nonzero market coordinate");
    let seeds =
        MakerReplaySeedsV1::new(coordinates, maker.to_bytes()).expect("nonzero maker seeds");
    let (address, bump) = Pubkey::find_program_address(&seeds.as_slices(), &TRADING_PROGRAM_ID);
    let rent_principal = Rent::default()
        .minimum_balance(dclutch_direct_codec::successor::DIRECT_MAKER_REPLAY_BYTES_V1);
    let intent = AuthenticatedIntentReplayV2::from_signed_intent(
        maker.to_bytes(),
        CompactIntentV2 {
            side: 0,
            lifecycle: if registered { 2 } else { 0 },
            outcome: 0,
            market: market.to_bytes(),
            generation: GENERATION,
            nonce: 0,
            valid_from: 0,
            valid_through: u64::MAX,
            maximum_fill: 1,
            limit_price: 1,
            fee_basis_points: FEE_BPS,
            collateral_account: [0x99; 32],
        },
    )
    .expect("replay coordinate");
    let created = consume_nonce_v2(
        DirectRootStateV1::new(),
        MakerReplayObservationV1::Vacant(MakerReplayVacancyV1::new(bump, 0)),
        intent,
        if registered {
            NonceConsumptionV2::Register
        } else {
            NonceConsumptionV2::Inline
        },
        Some(MakerReplayFirstUseV1 {
            rent_owner: rent_owner.to_bytes(),
            rent_principal,
        }),
    )
    .expect("first-use replay creation");
    let maker_root = if fee_owed == 0 {
        created.maker_root
    } else {
        created
            .maker_root
            .record_fee_owed(fee_owed)
            .expect("recorded obligation")
    };
    let bytes = maker_root.encode().expect("replay bytes");
    let lamports = rent_principal + donation;
    test.add_account(
        address,
        Account {
            lamports,
            data: bytes.to_vec(),
            owner: TRADING_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    InstalledReplay {
        address,
        maker,
        rent_owner,
        rent_principal,
        lamports,
    }
}

/// Everything the lifecycle sequence needs to submit both routes and check
/// itself.
struct CloseMakerCase {
    begin_retiring_request: [u8; 320],
    begin_retiring_metas: Vec<AccountMeta>,
    close_metas: Vec<AccountMeta>,
    root: Pubkey,
    root_bytes: Vec<u8>,
    market: Pubkey,
    native_close_transition: Vec<u8>,
    /// The canonical ordinary bundle this release was regenerated from.
    ///
    /// The operator plan builder takes it as a witness and rebuilds the whole
    /// five-entry release from it, so handing it the fixture's own bundle is
    /// what lets the builder re-derive -- rather than be told -- which
    /// descriptor, profile, and effect the close is allowed to use.
    ordinary: DirectInlineOrdinaryHotBundleV4,
    clean: InstalledReplay,
    debtor: InstalledReplay,
    live: InstalledReplay,
}

impl CloseMakerCase {
    fn begin_retiring_instruction(&self) -> Instruction {
        Instruction {
            program_id: TRADING_PROGRAM_ID,
            accounts: self.begin_retiring_metas.clone(),
            data: self.begin_retiring_request.to_vec(),
        }
    }

    /// The close frame for one installed replay.
    fn close_instruction(&self, replay: &InstalledReplay) -> Instruction {
        let mut metas = self.close_metas.clone();
        *metas
            .get_mut(DIRECT_CLOSE_MAKER_REPLAY_ACCOUNT_V1)
            .expect("replay coordinate") = AccountMeta::new(replay.address, false);
        *metas
            .get_mut(DIRECT_CLOSE_MAKER_RENT_OWNER_ACCOUNT_V1)
            .expect("rent owner coordinate") = AccountMeta::new(replay.rent_owner, false);
        Instruction {
            program_id: TRADING_PROGRAM_ID,
            accounts: metas,
            data: DirectCloseMakerRequestV1 {
                market: self.market.to_bytes(),
                maker: replay.maker.to_bytes(),
                generation: GENERATION,
            }
            .to_bytes()
            .expect("canonical close request")
            .to_vec(),
        }
    }

    fn close_instruction_with(
        &self,
        replay: &InstalledReplay,
        index: usize,
        meta: AccountMeta,
    ) -> Instruction {
        let mut instruction = self.close_instruction(replay);
        *instruction
            .accounts
            .get_mut(index)
            .expect("close coordinate") = meta;
        instruction
    }
}

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

/// Build the complete lifecycle world: a `Retiring` Core Market, an `Open`
/// Direct root with THREE maker roots standing, the five-entry release, and
/// every finalized record both routes borrow.
fn build_case(test: &mut ProgramTest, releases: Releases, artifacts: &Elves) -> CloseMakerCase {
    let rent = Rent::default();
    let substrate = fixture_substrate();
    let widths = DirectHotDeploymentWidthsV5::new(
        programdata_v2(substrate, &artifacts.trading).len(),
        programdata_v2(substrate, &artifacts.claims).len(),
        programdata_v2(substrate, &artifacts.core).len(),
    )
    .expect("real Direct deployment widths");

    let fixture = build_direct_hot_artifact_fixture_v5(widths, DirectOrdinaryGeometryV3::CANONICAL)
        .expect("canonical Direct artifact fixture");
    let release = build_direct_inline_ordinary_lifecycle_program_set_v1(
        fixture.bundle,
        DIRECT_HOT_FIXTURE_CAPACITY_PROFILE_V5,
    )
    .expect("five-entry lifecycle ProgramSet");
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
    let begin_descriptor_record = record(
        REGISTRY_PROGRAM_ID,
        direct_begin_retiring_descriptor_schema_v1(),
        release.begin_retiring.descriptor.clone(),
    );
    let begin_profile_record = record(
        REGISTRY_PROGRAM_ID,
        direct_begin_retiring_account_profile_schema_v1(),
        release.begin_retiring.account_profile.clone(),
    );
    let begin_effect_record = record(
        REGISTRY_PROGRAM_ID,
        direct_begin_retiring_effect_schema_v1(),
        release.begin_retiring.effect.clone(),
    );
    let close_descriptor_record = record(
        REGISTRY_PROGRAM_ID,
        direct_close_maker_descriptor_schema_v1(),
        release.close_maker.descriptor.clone(),
    );
    let close_profile_record = record(
        REGISTRY_PROGRAM_ID,
        direct_close_maker_account_profile_schema_v1(),
        release.close_maker.account_profile.clone(),
    );
    let close_effect_record = record(
        REGISTRY_PROGRAM_ID,
        direct_close_maker_effect_schema_v1(),
        release.close_maker.effect.clone(),
    );
    assert_eq!(
        direct_close_maker_account_profile_schema_v1(),
        ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1,
    );
    assert_eq!(
        direct_close_maker_effect_schema_v1(),
        EFFECT_SCHEMA_RELEASE_ID_V2,
    );
    assert_eq!(
        close_descriptor_record.digest,
        release.close_maker.descriptor_id
    );
    assert_eq!(
        close_profile_record.digest,
        release.close_maker.account_profile_id
    );
    assert_eq!(close_effect_record.digest, release.close_maker.effect_id);

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
    assert_eq!(founding_market, market);
    let market_bytes = CoreState {
        phase: Phase::Retiring,
        readiness: Readiness::Consumed,
        terminal_winner: 1,
        identity,
        outstanding_capabilities: 1,
        principal_cap_sets: u64::MAX,
        rent_beneficiary: core_identity([0x76; 32]),
        terminal_receipt: Some(core_identity([0x77; 32])),
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

    // Three standing maker roots: the clean one that will close, the debtor,
    // and the one with a registered live intent.
    let clean = install_replay(test, market, MAKER, RENT_OWNER, DONATION, 0, false);
    let debtor = install_replay(
        test,
        market,
        DEBTOR_MAKER,
        Pubkey::new_from_array([0xc3; 32]),
        0,
        DEBT,
        false,
    );
    let live = install_replay(
        test,
        market,
        LIVE_MAKER,
        Pubkey::new_from_array([0xc4; 32]),
        0,
        0,
        true,
    );

    // The recorded rent owner and the stranger wallet: plain System wallets.
    for wallet in [RENT_OWNER, STRANGER_WALLET] {
        test.add_account(
            wallet,
            Account {
                lamports: 1_000_000,
                data: Vec::new(),
                owner: solana_sdk_ids::system_program::ID,
                executable: false,
                rent_epoch: 0,
            },
        );
    }

    // The composite root: Open, with the three maker roots COUNTED. This is
    // the exact state wall 22 proved unretirable under the old ordering.
    let mut open_tail = DirectRootStateV1::new();
    for _ in 0..3 {
        open_tail = consume_root_count(open_tail);
    }
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
    root_bytes.extend_from_slice(&open_tail.encode());
    let root = Pubkey::find_program_address(&header.seeds().as_slices(), &TRADING_PROGRAM_ID).0;
    test.add_account(
        root,
        Account {
            lamports: rent.minimum_balance(root_bytes.len()),
            data: root_bytes.clone(),
            owner: TRADING_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    for value in [
        &manifest_record,
        &program_set_record,
        &begin_descriptor_record,
        &begin_profile_record,
        &begin_effect_record,
        &close_descriptor_record,
        &close_profile_record,
        &close_effect_record,
        &config_record,
    ] {
        install_record(test, value);
    }

    let begin_retiring_request = DirectBeginRetiringRequestV1 {
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

    // The two routes share their first twenty coordinate ROLES by design; only
    // the artifact records differ (begin-retiring's trio vs the close's).
    let shared = |records: [&Record; 3]| {
        let [descriptor, profile, effect] = records;
        vec![
            (0, root),
            (1, market),
            (2, manifest_record.raw),
            (3, program_set_record.raw),
            (4, program_set_record.staging),
            (5, descriptor.raw),
            (6, descriptor.staging),
            (7, config_record.raw),
            (8, config_record.staging),
            (9, profile.raw),
            (10, profile.staging),
            (11, effect.raw),
            (12, effect.staging),
            (13, releases.activation),
            (14, CORE_PROGRAM_ID),
            (15, releases.core_programdata),
            (16, TRADING_PROGRAM_ID),
            (17, releases.trading_programdata),
            (18, REGISTRY_PROGRAM_ID),
            (19, sysvar::rent::ID),
        ]
    };

    let mut begin_retiring_metas = vec![
        AccountMeta::new_readonly(Pubkey::default(), false);
        DIRECT_BEGIN_RETIRING_ACCOUNT_COUNT_V1
    ];
    for (index, key) in shared([
        &begin_descriptor_record,
        &begin_profile_record,
        &begin_effect_record,
    ]) {
        let (writable, _) =
            direct_begin_retiring_account_privileges_v1(index).expect("coordinate privileges");
        *begin_retiring_metas.get_mut(index).expect("coordinate") = AccountMeta {
            pubkey: key,
            is_signer: false,
            is_writable: writable,
        };
    }

    let mut close_metas = vec![
        AccountMeta::new_readonly(Pubkey::default(), false);
        DIRECT_CLOSE_MAKER_ACCOUNT_COUNT_V1
    ];
    for (index, key) in shared([
        &close_descriptor_record,
        &close_profile_record,
        &close_effect_record,
    ]) {
        let (writable, _) =
            direct_close_maker_account_privileges_v1(index).expect("coordinate privileges");
        *close_metas.get_mut(index).expect("coordinate") = AccountMeta {
            pubkey: key,
            is_signer: false,
            is_writable: writable,
        };
    }
    // The replay and rent-owner coordinates are per-close;
    // `close_instruction` fills them. Left as placeholders here so an
    // unfilled coordinate cannot pass the default-key assertion below.
    *close_metas
        .get_mut(DIRECT_CLOSE_MAKER_REPLAY_ACCOUNT_V1)
        .expect("replay placeholder") = AccountMeta::new(clean.address, false);
    *close_metas
        .get_mut(DIRECT_CLOSE_MAKER_RENT_OWNER_ACCOUNT_V1)
        .expect("rent owner placeholder") = AccountMeta::new(clean.rent_owner, false);

    for metas in [&begin_retiring_metas, &close_metas] {
        for (index, meta) in metas.iter().enumerate() {
            assert_ne!(
                meta.pubkey,
                Pubkey::default(),
                "coordinate {index} was never filled in",
            );
            assert!(!meta.is_signer, "no route here admits a signer");
        }
    }

    CloseMakerCase {
        begin_retiring_request,
        begin_retiring_metas,
        close_metas,
        root,
        root_bytes,
        market,
        native_close_transition: release.native_close.transition.clone(),
        ordinary: release.ordinary,
        clean,
        debtor,
        live,
    }
}

/// Step one root-count increment the way the fill does, without a fill: the
/// tail is `Open` throughout, so this is `consume_nonce_v2`'s count arithmetic
/// restated as the one thing the fixture needs from it.
fn consume_root_count(tail: DirectRootStateV1) -> DirectRootStateV1 {
    let mut bytes = tail.encode();
    let count = u64::from_le_bytes(
        bytes[DirectRootStateLayoutV1::OPEN_MAKER_ROOT_COUNT
            ..DirectRootStateLayoutV1::OPEN_MAKER_ROOT_COUNT + 8]
            .try_into()
            .expect("count word"),
    ) + 1;
    bytes[DirectRootStateLayoutV1::OPEN_MAKER_ROOT_COUNT
        ..DirectRootStateLayoutV1::OPEN_MAKER_ROOT_COUNT + 8]
        .copy_from_slice(&count.to_le_bytes());
    DirectRootStateV1::decode(&bytes).expect("stepped root tail")
}

/// Run the RELEASED native-close transition bytecode over one root tail, the
/// way `CloseCapability` runs it: the release's own `scalar_eq(count, 0)` is
/// the five-place gate's on-release member, and this is it, from the exact
/// bytes the market selected.
fn released_physical_close_gate(transition_bytes: &[u8], tail: &[u8]) -> Result<(), ()> {
    let transition = TransitionProgramV2::decode(transition_bytes).map_err(|_| ())?;
    let scalars_len = usize::from(transition.scalar_count());
    let identities_len = usize::from(transition.identity_count());
    let mut scalars = vec![0_u64; scalars_len];
    let word =
        |offset: usize| u64::from_le_bytes(tail[offset..offset + 8].try_into().expect("tail word"));
    scalars[usize::from(ACTIVATION_ACTION_SCALAR_V2)] = CoreEffectActionV1::CloseCapability as u64;
    scalars[usize::from(ACTIVATION_FIRST_FAMILY_SCALAR_V2)] = word(DirectRootStateLayoutV1::MAGIC);
    scalars[usize::from(ACTIVATION_FIRST_FAMILY_SCALAR_V2) + 1] =
        word(DirectRootStateLayoutV1::VERSION);
    scalars[usize::from(ACTIVATION_FIRST_FAMILY_SCALAR_V2) + 2] =
        word(DirectRootStateLayoutV1::OPEN_MAKER_ROOT_COUNT);
    let identities = vec![[0_u8; 32]; identities_len];
    let mut scratch_scalars = scalars.clone();
    let mut scratch_identities = identities.clone();
    let mut output_scalars = scalars.clone();
    let mut output_identities = identities.clone();
    execute_atomic(
        transition,
        RegisterInput {
            scalars: &scalars,
            identities: &identities,
        },
        RegisterOutput {
            scalars: &mut scratch_scalars,
            identities: &mut scratch_identities,
        },
        RegisterOutput {
            scalars: &mut output_scalars,
            identities: &mut output_identities,
        },
    )
    .map_err(|_| ())
}

async fn account(context: &mut ProgramTestContext, key: Pubkey) -> Account {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account read")
        .expect("live account")
}

async fn account_maybe(context: &mut ProgramTestContext, key: Pubkey) -> Option<Account> {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account read")
}

/// The complete wall-22 sequence: begin retiring OVER standing makers, close
/// one replay inside Retiring, and watch the physical-close gate open.
#[tokio::test]
async fn close_maker_drains_the_count_wall_22_stopped_at() {
    let artifacts = elves();
    let mut test = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let case = build_case(&mut test, releases, &artifacts);

    let begin = transaction_instructions(case.begin_retiring_instruction(), COMPUTE_LIMIT);
    let close_clean = transaction_instructions(case.close_instruction(&case.clean), COMPUTE_LIMIT);
    let close_debtor =
        transaction_instructions(case.close_instruction(&case.debtor), COMPUTE_LIMIT - 1);
    let close_live =
        transaction_instructions(case.close_instruction(&case.live), COMPUTE_LIMIT - 2);
    let reclose = transaction_instructions(case.close_instruction(&case.clean), COMPUTE_LIMIT - 3);
    let mut addresses = Vec::new();
    for arm in [&begin, &close_clean, &close_debtor, &close_live, &reclose] {
        addresses.extend(canonical_lookup_addresses(arm, Pubkey::default()));
    }
    addresses.sort_unstable_by_key(Pubkey::to_bytes);
    addresses.dedup();
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;

    // Prestate: Open, THREE makers standing -- the exact shape wall 22 proved
    // permanently unretirable under the old ordering.
    let before = account(&mut context, case.root).await;
    assert_eq!(before.data, case.root_bytes, "staged root prestate");
    let pre_tail = DirectRootStateV1::decode(
        before
            .data
            .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .expect("Direct tail"),
    )
    .expect("prestate tail");
    assert_eq!(pre_tail.phase(), DirectRootPhaseV1::Open);
    assert_eq!(pre_tail.open_maker_root_count(), 3);
    assert!(
        released_physical_close_gate(
            &case.native_close_transition,
            &before.data[CAPABILITY_ROOT_HEADER_BYTES_V1..],
        )
        .is_err(),
        "the released physical-close gate must refuse an Open, occupied root",
    );

    // Begin retiring OVER the standing makers: the flip, on the real ELF.
    let execution = submit_v0_observed(&mut context, &begin, addresses.clone(), None, &[])
        .await
        .expect("begin-retiring over standing maker roots");
    println!(
        "BEGINRETIRING(over 3 makers) compute units consumed: {}",
        execution.compute_units_consumed
    );
    let retiring = account(&mut context, case.root).await;
    let retiring_tail = DirectRootStateV1::decode(
        retiring
            .data
            .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .expect("Direct tail"),
    )
    .expect("retiring tail");
    assert_eq!(retiring_tail.phase(), DirectRootPhaseV1::Retiring);
    assert_eq!(
        retiring_tail.open_maker_root_count(),
        3,
        "begin-retiring preserves the standing count; only the phase moves",
    );

    // The debtor refuses by name: the replay is the sole record of the
    // receivable, and this close will not erase a debt.
    let refusal = refused(
        submit_v0_observed(&mut context, &close_debtor, addresses.clone(), None, &[]).await,
        "closing the debtor's replay",
    );
    assert_eq!(
        refusal_code(&refusal.error),
        Some(TradingSbfError::CloseMakerFeeOutstanding as u32),
        "the debtor's close refused as something else: {:#?}",
        refusal.logs,
    );

    // Standing registered intents refuse by name too.
    let refusal = refused(
        submit_v0_observed(&mut context, &close_live, addresses.clone(), None, &[]).await,
        "closing the replay with a live registered intent",
    );
    assert_eq!(
        refusal_code(&refusal.error),
        Some(TradingSbfError::CloseMakerLiveIntents as u32),
        "the live-intent close refused as something else: {:#?}",
        refusal.logs,
    );

    // The clean maker's replay closes: the missing decrement, landing.
    let owner_before = account(&mut context, case.clean.rent_owner).await.lamports;
    let execution = submit_v0_observed(&mut context, &close_clean, addresses.clone(), None, &[])
        .await
        .expect("the clean maker's close");
    println!(
        "CLOSEMAKER compute units consumed: {}",
        execution.compute_units_consumed
    );

    let drained = account(&mut context, case.root).await;
    let drained_tail = DirectRootStateV1::decode(
        drained
            .data
            .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .expect("Direct tail"),
    )
    .expect("drained tail");
    assert_eq!(drained_tail.phase(), DirectRootPhaseV1::Retiring);
    assert_eq!(
        drained_tail.open_maker_root_count(),
        2,
        "exactly one closed"
    );
    assert_eq!(
        drained.data.get(..CAPABILITY_ROOT_HEADER_BYTES_V1),
        before.data.get(..CAPABILITY_ROOT_HEADER_BYTES_V1),
        "the immutable activation header moved",
    );

    // The replay account is gone -- returned to the System program, empty --
    // and the WHOLE balance (principal + donation) landed on the recorded
    // rent owner, per the landed Lean plan (`maker_close_refund_conserved`).
    let closed_replay = account_maybe(&mut context, case.clean.address).await;
    assert!(
        closed_replay.is_none()
            || (closed_replay
                .as_ref()
                .is_some_and(|value| value.lamports == 0
                    && value.data.is_empty()
                    && value.owner == solana_sdk_ids::system_program::ID)),
        "the closed replay must be gone or a zeroed System account",
    );
    let owner_after = account(&mut context, case.clean.rent_owner).await.lamports;
    assert_eq!(
        owner_after,
        owner_before + case.clean.lamports,
        "principal plus donation, conserved to the recorded owner",
    );
    assert_eq!(
        case.clean.lamports,
        case.clean.rent_principal + DONATION,
        "the fixture's donation arithmetic drifted",
    );

    // The receipt rejoins the request and the observed poststate.
    let (producer, returned) = execution
        .return_data
        .expect("a successful close must return its DCLTDMX1 receipt");
    assert_eq!(producer, TRADING_PROGRAM_ID);
    let receipt = DirectCloseMakerReceiptV1::decode(&returned).expect("canonical receipt");
    assert_eq!(receipt.market, case.market.to_bytes());
    assert_eq!(receipt.maker, MAKER.to_bytes());
    assert_eq!(receipt.maker_root, case.clean.address.to_bytes());
    assert_eq!(receipt.rent_owner, RENT_OWNER.to_bytes());
    assert_eq!(receipt.post_root_digest, hash(&drained.data).to_bytes());
    assert_eq!(receipt.rent_principal, case.clean.rent_principal);
    assert_eq!(receipt.unclassified_donation, DONATION);
    assert_eq!(receipt.total_credit, case.clean.lamports);
    assert_eq!(receipt.remaining_open_maker_roots, 2);

    // A second close of the same replay refuses by absence.
    let refusal = refused(
        submit_v0_observed(&mut context, &reclose, addresses.clone(), None, &[]).await,
        "re-closing the already-closed replay",
    );
    assert_eq!(
        refusal_code(&refusal.error),
        Some(TradingSbfError::CloseMakerReplayAccount as u32),
        "the double close refused as something else: {:#?}",
        refusal.logs,
    );

    // The physical-close gate, from the RELEASED bytes: still shut at count 2,
    // and open on the fully drained tail -- the exact five-place gate wall 22
    // died on, now reachable.
    assert!(
        released_physical_close_gate(
            &case.native_close_transition,
            &drained.data[CAPABILITY_ROOT_HEADER_BYTES_V1..],
        )
        .is_err(),
        "two makers still stand; the released gate must stay shut",
    );
    let mut fully_drained = drained.data[CAPABILITY_ROOT_HEADER_BYTES_V1..].to_vec();
    fully_drained[DirectRootStateLayoutV1::OPEN_MAKER_ROOT_COUNT
        ..DirectRootStateLayoutV1::OPEN_MAKER_ROOT_COUNT + 8]
        .copy_from_slice(&0_u64.to_le_bytes());
    released_physical_close_gate(&case.native_close_transition, &fully_drained)
        .expect("the released physical-close gate must open once every maker root has closed");
    DirectRootStateV1::decode(&fully_drained)
        .expect("drained tail decodes")
        .require_closable()
        .expect("the drained root satisfies the closability invariant");
}

/// Hostile close frames, each naming its exact refusal code, none moving state.
#[tokio::test]
async fn close_maker_refuses_hostile_frames() {
    let artifacts = elves();
    let mut test = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let case = build_case(&mut test, releases, &artifacts);

    // Begin retiring first, so every arm below refuses for ITS reason rather
    // than the phase's.
    let begin = transaction_instructions(case.begin_retiring_instruction(), COMPUTE_LIMIT);

    // A substituted rent destination: a plain System wallet that is not the
    // recorded owner.
    let stranger_destination = case.close_instruction_with(
        &case.clean,
        DIRECT_CLOSE_MAKER_RENT_OWNER_ACCOUNT_V1,
        AccountMeta::new(STRANGER_WALLET, false),
    );
    // The debtor's replay offered under the clean maker's coordinate: the
    // body's own maker field disagrees with the request.
    let foreign_replay = case.close_instruction_with(
        &case.clean,
        DIRECT_CLOSE_MAKER_REPLAY_ACCOUNT_V1,
        AccountMeta::new(case.debtor.address, false),
    );
    // One byte short of the one exact request width.
    let mut narrow_instruction = case.close_instruction(&case.clean);
    narrow_instruction.data.pop();
    // Exact width, a selector that names another action: the ProgramSet
    // refuses the selection.
    let mut foreign_selector_data = case.close_instruction(&case.clean).data;
    foreign_selector_data[12] ^= 1;
    let mut foreign_selector = case.close_instruction(&case.clean);
    foreign_selector.data = foreign_selector_data;

    let arms = [
        transaction_instructions(stranger_destination, COMPUTE_LIMIT),
        transaction_instructions(foreign_replay, COMPUTE_LIMIT - 1),
        transaction_instructions(narrow_instruction, COMPUTE_LIMIT - 2),
        transaction_instructions(foreign_selector, COMPUTE_LIMIT - 3),
    ];
    let expected: [(u32, &str); 4] = [
        (
            TradingSbfError::CloseMakerFrame as u32,
            "a substituted rent destination",
        ),
        (
            TradingSbfError::CloseMakerReplayAccount as u32,
            "another maker's replay under this coordinate",
        ),
        (TradingSbfError::Content as u32, "a truncated request"),
        (
            TradingSbfError::Content as u32,
            "a selector naming another action",
        ),
    ];

    let mut addresses = canonical_lookup_addresses(&begin, Pubkey::default());
    for arm in &arms {
        addresses.extend(canonical_lookup_addresses(arm, Pubkey::default()));
    }
    addresses.sort_unstable_by_key(Pubkey::to_bytes);
    addresses.dedup();
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;

    submit_v0_observed(&mut context, &begin, addresses.clone(), None, &[])
        .await
        .expect("begin-retiring over standing maker roots");
    let retiring = account(&mut context, case.root).await;

    for (arm, (code, label)) in arms.iter().zip(expected) {
        let refusal = refused(
            submit_v0_observed(&mut context, arm, addresses.clone(), None, &[]).await,
            label,
        );
        assert_eq!(
            refusal_code(&refusal.error),
            Some(code),
            "{label} refused with another code: {:#?}",
            refusal.logs,
        );
    }

    // Nothing moved: the root, all three replays, and both wallets are
    // byte-identical to the post-begin-retiring state.
    let after = account(&mut context, case.root).await;
    assert_eq!(after.data, retiring.data, "a refused close moved the root");
    for replay in [&case.clean, &case.debtor, &case.live] {
        let untouched = account(&mut context, replay.address).await;
        assert_eq!(untouched.lamports, replay.lamports);
        assert_eq!(untouched.owner, TRADING_PROGRAM_ID);
    }
}

/// Read one live account as the operator crate's finalized observation.
///
/// A bank has no commitment ladder, so the finality label here is asserted by
/// the fixture rather than observed. That is honest for what this test is
/// proving -- that the builder's INSTRUCTION is one the real program accepts --
/// and the freshness rules themselves are red-proofed in the builder's own
/// unit tests, where a mixed observation is cheap to construct.
/// An absent account is read as the empty System account it would become, which
/// is what an RPC client sees for a wallet nobody has funded yet. The debtor
/// and live-intent replays have exactly that: a recorded rent owner that has
/// never been credited, because their closes have never been allowed to run.
async fn observed_account(
    context: &mut ProgramTestContext,
    key: Pubkey,
    observation: Observation,
) -> ObservedAccount {
    match account_maybe(context, key).await {
        Some(account) => ObservedAccount {
            observation,
            key,
            owner: account.owner,
            lamports: account.lamports,
            executable: account.executable,
            data: account.data,
        },
        None => ObservedAccount {
            observation,
            key,
            owner: solana_sdk_ids::system_program::ID,
            lamports: 0,
            executable: false,
            data: Vec::new(),
        },
    }
}

/// Gather the exact 22-account graph the close plan builder authenticates.
///
/// The coordinates come from the frame the fixture already built by hand, so
/// this reads what the route reads and nothing else. The replay and rent-owner
/// slots are per-close and are filled from the installed replay.
async fn close_snapshot(
    context: &mut ProgramTestContext,
    case: &CloseMakerCase,
    replay: &InstalledReplay,
) -> DirectCloseMakerSnapshotV1 {
    let observation = Observation {
        slot: 1_000,
        unix_timestamp: 1_788_000_000,
        finality: Finality::Finalized,
    };
    let mut keys = case
        .close_metas
        .iter()
        .map(|meta| meta.pubkey)
        .collect::<Vec<_>>();
    *keys
        .get_mut(DIRECT_CLOSE_MAKER_REPLAY_ACCOUNT_V1)
        .expect("replay coordinate") = replay.address;
    *keys
        .get_mut(DIRECT_CLOSE_MAKER_RENT_OWNER_ACCOUNT_V1)
        .expect("rent owner coordinate") = replay.rent_owner;

    let mut gathered = Vec::with_capacity(DIRECT_CLOSE_MAKER_ACCOUNT_COUNT_V1);
    for key in &keys {
        gathered.push(observed_account(context, *key, observation).await);
    }
    let at = |index: usize| gathered.get(index).expect("gathered coordinate").clone();

    DirectCloseMakerSnapshotV1 {
        // A program-test bank is neither devnet nor mainnet, which is exactly
        // what the owned-loopback arm is for. A builder hard-wired to devnet's
        // genesis could not have been driven from a bank at all.
        cluster: DirectCloseMakerClusterV1::OwnedLoopback,
        genesis_hash: [0x11; 32],
        ordinary_release_witness: case.ordinary,
        root: at(0),
        market: at(1),
        capability_manifest: at(2),
        program_set: at(3),
        program_set_staging: at(4),
        descriptor: at(5),
        descriptor_staging: at(6),
        config: at(7),
        config_staging: at(8),
        account_profile: at(9),
        account_profile_staging: at(10),
        effect: at(11),
        effect_staging: at(12),
        activation_cache: at(13),
        core_program: at(14),
        core_programdata: at(15),
        trading_program: at(16),
        trading_programdata: at(17),
        registry_program: at(18),
        rent_sysvar: at(19),
        maker: replay.maker,
        maker_replay: at(20),
        rent_owner: at(21),
    }
}

/// The operator's close plan builder, driven against the real ELFs.
///
/// The builder's own unit tests prove it agrees with its own fixture. This
/// proves the thing those cannot: that the instruction it emits from a real
/// account graph is one the deployed program accepts, and that the poststate
/// it PREDICTED -- the receipt bytes, the root digest, the beneficiary's
/// balance -- is the poststate the chain actually produced. A builder whose
/// projection drifted from the route by one byte would still be green in a
/// scratchpad and wrong here.
///
/// The two named refusals are checked on the same real graph, and they are
/// checked at PLAN time: no transaction is built, signed, or sent for either.
/// That is the cut-day property the operator half exists to provide.
#[tokio::test]
async fn operator_plan_builder_drives_a_real_close_and_refuses_the_two_by_name() {
    let artifacts = elves();
    let mut test = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let case = build_case(&mut test, releases, &artifacts);

    let begin = transaction_instructions(case.begin_retiring_instruction(), COMPUTE_LIMIT);
    let close_clean = transaction_instructions(case.close_instruction(&case.clean), COMPUTE_LIMIT);
    let mut addresses = Vec::new();
    for arm in [&begin, &close_clean] {
        addresses.extend(canonical_lookup_addresses(arm, Pubkey::default()));
    }
    addresses.sort_unstable_by_key(Pubkey::to_bytes);
    addresses.dedup();
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;

    // The close needs a Retiring root, so the sequence runs for real first.
    submit_v0_observed(&mut context, &begin, addresses.clone(), None, &[])
        .await
        .expect("begin-retiring over standing maker roots");

    // ---- the debtor and the live replay refuse BEFORE a transaction exists --
    let debtor = close_snapshot(&mut context, &case, &case.debtor).await;
    assert_eq!(
        plan_direct_close_maker_v1(&debtor).expect_err("a debtor replay must not plan"),
        DirectCloseMakerPlanErrorV1::FeeOutstanding,
        "the debtor's close must refuse at plan time, mirroring 0x4011",
    );

    let live = close_snapshot(&mut context, &case, &case.live).await;
    assert_eq!(
        plan_direct_close_maker_v1(&live).expect_err("a live replay must not plan"),
        DirectCloseMakerPlanErrorV1::LiveIntents,
        "the live-intent close must refuse at plan time, mirroring 0x4012",
    );

    // ---- the clean replay plans, and the plan is the fixture's own frame ----
    let snapshot = close_snapshot(&mut context, &case, &case.clean).await;
    let report = match plan_direct_close_maker_v1(&snapshot).expect("a clean replay must plan") {
        DirectCloseMakerPlanV1::Submit(report) => report,
        DirectCloseMakerPlanV1::Complete(_) => panic!("a standing replay must not be Complete"),
    };

    // Two independent authors of the same frame agree: the hand-built fixture
    // instruction and the chain-derived one are byte-identical.
    let handbuilt = case.close_instruction(&case.clean);
    assert_eq!(report.instruction.program_id, handbuilt.program_id);
    assert_eq!(report.instruction.data, handbuilt.data);
    assert_eq!(report.instruction.accounts, handbuilt.accounts);
    assert!(
        report
            .instruction
            .accounts
            .iter()
            .all(|meta| !meta.is_signer),
        "the close is permissionless; nothing in its frame may ask to sign",
    );

    let owner_before = account(&mut context, case.clean.rent_owner).await.lamports;
    let close = transaction_instructions(report.instruction.clone(), COMPUTE_LIMIT);

    // ---- submit the BUILDER's instruction, not the fixture's ----
    let execution = submit_v0_observed(&mut context, &close, addresses.clone(), None, &[])
        .await
        .expect("the plan builder's own close instruction");
    println!(
        "CLOSEMAKER(operator-planned) compute units consumed: {}",
        execution.compute_units_consumed
    );

    // The predicted receipt is the receipt the chain produced -- producer and
    // bytes both. This is the projection O-016 is about: the builder never told
    // the chain what to write, and still knew exactly what it would write.
    let (producer, body) = execution.return_data.expect("close receipt return data");
    assert_eq!(producer, report.expected_receipt_producer);
    assert_eq!(
        body,
        report.expected_receipt_body.to_vec(),
        "the predicted receipt bytes are not the ones the route emitted",
    );
    let landed = DirectCloseMakerReceiptV1::decode(&body).expect("landed receipt");
    assert_eq!(landed, report.expected_receipt);

    // The predicted poststate is the poststate.
    let after_root = account(&mut context, case.root).await;
    assert_eq!(
        after_root.data, report.expected_post_root_data,
        "the predicted post-root bytes are not the ones the route wrote",
    );
    let after_tail = DirectRootStateV1::decode(
        after_root
            .data
            .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .expect("Direct tail"),
    )
    .expect("post tail");
    assert_eq!(
        after_tail.open_maker_root_count(),
        report.expected_remaining_open_maker_roots,
    );

    // The money went where the replay said, in the amount the replay said.
    let owner_after = account(&mut context, case.clean.rent_owner).await.lamports;
    assert_eq!(owner_after, report.expected_rent_owner_lamports);
    assert_eq!(owner_after - owner_before, report.total_credit);
    assert_eq!(
        report.rent_principal + report.unclassified_donation,
        report.total_credit,
    );

    // The replay is gone, and the builder now says so instead of planning a
    // second close that could only refuse by absence.
    let drained = account_maybe(&mut context, case.clean.address).await;
    assert!(
        drained
            .as_ref()
            .is_none_or(|account| account.lamports == 0 && account.data.is_empty()),
        "the closed replay must be gone or empty, found {drained:?}",
    );
}
