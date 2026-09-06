#![allow(dead_code)]

include!("resolution_core_v3_lifecycle.rs");

use dclutch_claims::liability_basis_state_v2::{
    LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_MARKET_SEED_V2,
    LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, LiabilityBasisMarketInputV2,
    LiabilityBasisMarketViewV2, LiabilityBasisPositionInputV2,
    encode_liability_basis_market_into_v2, encode_liability_basis_position_into_v2,
    liability_basis_vector_width_v2,
};
use dclutch_claims::protocol_position_v2::{
    FailureEscrowV1, ProtocolPositionActionV2, ProtocolPositionAdmissionEvidenceV2,
    ProtocolPositionAdmissionV2, ProtocolPositionOwnerKindV2, ProtocolPositionPresenceV2,
    ProtocolPositionRequestV2, ProtocolPositionSeedsV2, failure_escrow_v1,
};
use dclutch_market_retirement_v1_operator::{
    CHECKPOINT_RETIREMENT_CUSTODY_SUFFIX_BYTES_V1, CHECKPOINT_RETIREMENT_FINISH_BYTES_V1,
    CHECKPOINT_RETIREMENT_PREPARE_CORE_BYTES_V1, MarketRetirementOperatorErrorV1,
    MarketRetirementSnapshotV1, ResolutionRetirementReceiptFactsV3,
    build_checkpoint_market_retirement_v1, build_market_retirement_v1,
    terminal_stage_order_v1::{
        TerminalStageOrderErrorV1, TerminalStageV1, authenticate_terminal_stage_order_v1,
    },
};
use dclutch_product::payoff::runtime_v3::{
    BasisInputV3, BasisKindV3, ProductBasisV3, basis_record_bytes_v3, compile_basis_v3,
    semantic_basis_id_v3,
};
use dclutch_registry::svm::continuation_v1::{
    REGISTRY_CONTINUATION_REQUEST_BYTES_V1, RegistryContinuationAdmissionSeedsV1,
    RegistryContinuationRequestV1,
};
// `RefundAuthority` used to arrive through the `include!` above, on the back of
// the V1 record's own import. That record is deleted; the wallet type is not.
use dclutch_market::rent::RefundAuthority;
use dclutch_market::rent::lifecycle_v2::{
    LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleAccountIdV2, LifecycleRentCreditV2,
};
use spl_token_interface::state::{Account as SplAccount, AccountState};

/// The discriminant a seated refunding failure column refuses with at the
/// Claims handoff.
///
/// The accusation is the Claims closure's own -- "a nonzero aggregate supply
/// prevented closure" -- and, MEASURED, that is also what the transaction
/// reports: `0x5503` reaches the caller through Core's CPI rather than Core's
/// own `ChildCpi` (`programs/dclutch-core-sbf/src/retire_v1.rs:585`), which is
/// what the `map_err` there would have suggested. The child's code is the one
/// on the wire, so an operator reading a failed retirement reads the Claims
/// band and goes to the right program. Derived from the enum rather than
/// written: a band move must break this reading rather than relabel it.
const CHECKPOINT_PREPARE_SEATED_RESIDUE_REFUSAL_V1: u32 =
    dclutch_claims_sbf::market_closure_v1::ClaimsMarketClosureSbfErrorV1::Liability as u32;

/// Runtime width of this campaign's Claims aggregate.
const RETIREMENT_CLAIM_COUNT: u32 = 5;

/// This campaign's OWN Claims and Trading programs, and its own release set.
///
/// The base campaign gained five real roles of its own on 2026-09-02
/// (`JOINED_CLAIMS_PROGRAM_ID` 0x78, `JOINED_TRADING_PROGRAM_ID` 0x79). These ids stay
/// distinct from those deliberately: a joined fixture that shared the base's
/// release set would share its Market address too, and the base's `fixture()`
/// account and this campaign's `set_account` would then be two authors of one
/// account. Two release sets, two Market addresses, one activation builder.
const JOINED_CLAIMS_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x76; 32]);
const JOINED_TRADING_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x77; 32]);

const CLAIMS_REVISION: u64 = 11;
const CUSTODY_REVISION: u64 = 2;

struct JoinedFixture {
    base: Fixture,
    claims_programdata: Pubkey,
    trading_programdata: Pubkey,
    rent_programdata: Pubkey,
    refund_wallet: Pubkey,
    claims_aggregate: Pubkey,
    infrastructure_profile: Pubkey,
    registry_artifact: RecordPair,
    rent_artifact: RecordPair,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct JoinedSnapshot {
    market: Option<Account>,
    rent_credit: Option<Account>,
    claims_aggregate: Option<Account>,
    custody_replay: Option<Account>,
    hoard_vault: Option<Account>,
    source_receipt: Option<Account>,
    refund_wallet: Option<Account>,
}

fn set_account(context: &mut ProgramTestContext, key: Pubkey, account: Account) {
    context.set_account(&key, &AccountSharedData::from(account));
}

async fn joined_fixture() -> (JoinedFixture, ProgramTestContext) {
    let mut base = fixture(MarketPrestateV1::Terminal);
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    let rent_elf = fs::read(directory.join("dclutch_rent_sbf.so")).expect("Rent ELF");
    let elves = artifacts();
    let test = base.test.as_mut().expect("unstarted ProgramTest");
    let claims_elf = fs::read(directory.join("dclutch_claims_sbf.so")).expect("Claims ELF");
    let trading_elf = fs::read(directory.join("dclutch_trading_sbf.so")).expect("Trading ELF");
    add_program(
        test,
        "dclutch_claims_sbf",
        JOINED_CLAIMS_PROGRAM_ID,
        &claims_elf,
    );
    add_program(
        test,
        "dclutch_trading_sbf",
        JOINED_TRADING_PROGRAM_ID,
        &trading_elf,
    );
    add_program(test, "dclutch_rent_sbf", RENT_PROGRAM_ID, &rent_elf);

    let core_release = release(CORE_PROGRAM_ID, [0x41; 32], &elves.core);
    let claims_release = release(JOINED_CLAIMS_PROGRAM_ID, [0x43; 32], &claims_elf);
    let trading_release = release(JOINED_TRADING_PROGRAM_ID, [0x46; 32], &trading_elf);
    let resolution_release = release(
        RESOLUTION_PROGRAM_ID,
        RESOLUTION_CONTROLLER_RELEASE_ID_V7,
        &elves.resolution,
    );
    let custody_release = release(CUSTODY_PROGRAM_ID, [0x42; 32], &elves.custody);
    let rent_release = release(RENT_PROGRAM_ID, [0x44; 32], &rent_elf);
    let registry_release = release(REGISTRY_PROGRAM_ID, [0x45; 32], &elves.registry);
    let (release_set, activation_data) = activation(
        core_release,
        claims_release,
        trading_release,
        resolution_release,
        custody_release,
    );
    let activation = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    test.add_account(
        activation,
        protocol_account(REGISTRY_PROGRAM_ID, activation_data),
    );
    let registry_artifact = add_record(
        test,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        registry_release.to_bytes().to_vec(),
    );
    let rent_artifact = add_record(
        test,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        rent_release.to_bytes().to_vec(),
    );
    let infrastructure_profile = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
        &CORE_PROGRAM_ID,
    )
    .0;
    // Registry moved across the succession and Rent did not: the predecessor
    // Registry id names the distinct release this profile succeeded, while
    // Rent holds the same id on both sides of it.
    let predecessor_registry_release = release(REGISTRY_PROGRAM_ID, [0xb5; 32], &elves.registry);
    let infrastructure = ProtocolInfrastructureProfileV2::new(
        binding(registry_release),
        binding(rent_release),
        artifact_id(predecessor_registry_release),
        artifact_id(rent_release),
    )
    .expect("infrastructure succession profile");
    test.add_account(
        infrastructure_profile,
        protocol_account(CORE_PROGRAM_ID, infrastructure.to_bytes().to_vec()),
    );

    let mut context = base
        .test
        .take()
        .expect("unstarted ProgramTest")
        .start_with_context()
        .await;
    let old_state = CoreState::decode(
        &observed(&mut context, base.market)
            .await
            .expect("old fixture Market")
            .data,
    )
    .expect("old Core state");
    let mut identity = old_state.identity;
    identity.selected_release_set = CoreIdentity::new(release_set).expect("joined release set");
    identity.market_id = CoreIdentity::new([0xff; 32]).expect("placeholder Market");
    let market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(identity).as_slices(),
        &CORE_PROGRAM_ID,
    )
    .0;
    identity.market_id = CoreIdentity::new(market.to_bytes()).expect("joined Market");
    let refund_wallet = Pubkey::new_from_array([0xd1; 32]);
    let (rent_credit, rent_bump) = Pubkey::find_program_address(
        &[
            LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
            market.as_ref(),
            &GENERATION.to_le_bytes(),
        ],
        &RENT_PROGRAM_ID,
    );
    let rent_credit_value = LifecycleRentCreditV2::new(
        RefundAuthority::new(refund_wallet.to_bytes()).expect("refund wallet"),
        LifecycleAccountIdV2::new(market.to_bytes()).expect("Market"),
        LifecycleAccountIdV2::new(release_set).expect("release set"),
        GENERATION,
        rent_bump,
    )
    .expect("lifecycle RentCredit");
    let state = CoreState {
        phase: Phase::Founding,
        readiness: Readiness::Prepaid,
        terminal_winner: 0,
        identity,
        outstanding_capabilities: 0,
        principal_cap_sets: u64::MAX,
        rent_beneficiary: CoreIdentity::new(rent_credit.to_bytes()).expect("RentCredit"),
        terminal_receipt: None,
        bumps: StateBumpsV1::UNRECORDED,
    };
    set_account(
        &mut context,
        market,
        protocol_account(
            CORE_PROGRAM_ID,
            state.encode().expect("Core state").to_vec(),
        ),
    );
    set_account(
        &mut context,
        refund_wallet,
        Account {
            lamports: 1_000_000,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    set_account(
        &mut context,
        rent_credit,
        protocol_account(RENT_PROGRAM_ID, rent_credit_value.to_bytes().to_vec()),
    );

    let (source, _) = Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
            market.as_ref(),
            &GENERATION.to_le_bytes(),
        ],
        &RESOLUTION_PROGRAM_ID,
    );
    let activation_receipt = Pubkey::find_program_address(
        &[
            FUNDING_ACTIVATION_RECEIPT_PDA_DOMAIN_V1,
            market.as_ref(),
            &GENERATION.to_le_bytes(),
        ],
        &RESOLUTION_PROGRAM_ID,
    )
    .0;
    let certificate = Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            source.as_ref(),
            &[1],
            &TERMINAL_SEQUENCE.to_le_bytes(),
        ],
        &RESOLUTION_PROGRAM_ID,
    )
    .0;
    let closure = Pubkey::find_program_address(
        &[
            SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V3,
            source.as_ref(),
            &(TERMINAL_SEQUENCE + 1).to_le_bytes(),
        ],
        &RESOLUTION_PROGRAM_ID,
    )
    .0;
    let manifest_account = observed(&mut context, base.capability_manifest.raw)
        .await
        .expect("capability manifest");
    let manifest_id = CapabilityContentId::new(hash(&manifest_account.data).to_bytes())
        .expect("manifest identity");
    let manifest = CapabilityManifestV1::decode(&manifest_account.data).expect("manifest");
    // The Resolution subset of the BASE fixture's manifest, taken from the
    // fixture rather than restated. It used to be the literal `0b111`, which
    // was correct only while every row in the manifest was a Resolution row;
    // the base manifest carries the Direct capability now, at whatever row its
    // kind_id sorts to, and a literal here selected the Direct row and dropped
    // a Resolution one. The failure was an operator `Funding` refusal on the
    // same-lineage CreateFund, which is a long way from the line that caused it.
    let resolution_funding_mask = base.resolution_selected_mask;
    let funding = funding_key(market, manifest_id, manifest, resolution_funding_mask);
    let funding_width = funding_ledger_bytes_v2(3).expect("three-row FundingLedgerV2 width");
    let mut funding_data = vec![0_u8; funding_width];
    FundingLedgerV2::initialize(
        &mut funding_data,
        manifest_id,
        manifest,
        resolution_funding_mask,
        funded_rent_rate(funding_width),
    )
    .expect("pre-Market pending Resolution subset ledger");
    let funding_principal = FundingLedgerV2::decode(&funding_data)
        .and_then(|ledger| ledger.authenticate(manifest_id, manifest))
        .and_then(|ledger| ledger.remaining_native_lamports_total())
        .expect("bounded aggregate Resolution principal");
    set_account(
        &mut context,
        funding,
        Account {
            lamports: Rent::default().minimum_balance(funding_width) + funding_principal,
            data: funding_data,
            owner: RESOLUTION_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let replay_request = custody_request(
        release_set,
        market,
        base.realm,
        base.mint,
        context.payer.pubkey(),
        rent_credit,
        OperationV1::InitializeReplay,
    );
    let replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::from_request(replay_request).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let custody_authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::from_request(replay_request).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let vault = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            market.to_bytes(),
            release_set,
            market.to_bytes(),
            CompartmentV1::HoardPrincipal,
        )
        .as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let claims_aggregate = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, market.as_ref()],
        &JOINED_CLAIMS_PROGRAM_ID,
    )
    .0;
    let mut aggregate_bytes = vec![
        0;
        liability_basis_vector_width_v2(
            LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
            RETIREMENT_CLAIM_COUNT,
        )
        .expect("runtime aggregate width")
    ];
    encode_liability_basis_market_into_v2(
        LiabilityBasisMarketInputV2 {
            revision: CLAIMS_REVISION,
            logical_market: market.to_bytes(),
            release_set,
            registry_program: REGISTRY_PROGRAM_ID.to_bytes(),
            // SEEDED THE WAY CLAIMS WRITES IT, which is the Core state's
            // product INSTANCE and not its record. Seeding it from
            // `product_record` made this fixture a mirror of the retirement
            // operator's own reader rather than of the program that owns the
            // field, so the two agreed about a value no chain ever carries.
            product_instance_id: identity.product_id.to_bytes(),
            basis_id: [0xd2; 32],
            realm_id: base.realm,
            custody_context: market.to_bytes(),
            generation: GENERATION,
        },
        &[0, 0, 0, 0, 0],
        &mut aggregate_bytes,
    )
    .expect("empty Claims aggregate");
    set_account(
        &mut context,
        claims_aggregate,
        protocol_account(JOINED_CLAIMS_PROGRAM_ID, aggregate_bytes),
    );

    base.release_set = release_set;
    base.activation = activation;
    base.infrastructure = infrastructure_profile;
    base.registry_artifact = registry_artifact;
    base.market = market;
    base.source = source;
    base.funding = funding;
    base.activation_receipt = activation_receipt;
    base.certificate = certificate;
    base.closure = closure;
    base.rent_credit = rent_credit;
    base.replay = replay;
    base.vault = vault;
    base.custody_authority = custody_authority;
    (
        JoinedFixture {
            base,
            claims_programdata: programdata(JOINED_CLAIMS_PROGRAM_ID),
            trading_programdata: programdata(JOINED_TRADING_PROGRAM_ID),
            rent_programdata: programdata(RENT_PROGRAM_ID),
            refund_wallet,
            claims_aggregate,
            infrastructure_profile,
            registry_artifact,
            rent_artifact,
        },
        context,
    )
}

async fn seed_exact_retirement_prestate(context: &mut ProgramTestContext, fixture: &JoinedFixture) {
    let market_account = observed(context, fixture.base.market)
        .await
        .expect("prebuilt Market");
    let mut market = CoreState::decode(&market_account.data).expect("prebuilt Core Market");
    market.phase = Phase::Retiring;
    market.readiness = Readiness::Consumed;
    market.outstanding_capabilities = 0;
    market.terminal_winner = 0;
    market.terminal_receipt =
        Some(CoreIdentity::new(fixture.base.certificate.to_bytes()).expect("terminal certificate"));
    set_account(
        context,
        fixture.base.market,
        Account {
            data: market.encode().expect("Retiring Core Market").to_vec(),
            ..market_account
        },
    );

    let source_refund_lamports = 1_u64;
    let closure = SourceClosureReceiptV3 {
        market: fixture.base.market.to_bytes(),
        source_state: fixture.base.source.to_bytes(),
        source_material: market.identity.resolution_policy.to_bytes(),
        capability_manifest: market.identity.capability_manifest.to_bytes(),
        terminal_certificate: fixture.base.certificate.to_bytes(),
        receipt_account: fixture.base.closure.to_bytes(),
        beneficiary: fixture.base.rent_credit.to_bytes(),
        source_state_digest: [0xa1; 32],
        terminal_certificate_digest: [0xa2; 32],
        funding_set_digest: [0xa3; 32],
        generation: market.identity.generation,
        terminal_sequence: TERMINAL_SEQUENCE,
        selector: market.terminal_winner,
        source_refund_lamports,
        ledger_remaining_native_principal: 0,
        ledger_rent_lamports: 0,
        ledger_lamport_surplus: 0,
        refund_lamports: source_refund_lamports,
        closed_at: u64::try_from(TERMINAL_TIME).expect("positive terminal time"),
    };
    set_account(
        context,
        fixture.base.closure,
        protocol_account(
            RESOLUTION_PROGRAM_ID,
            closure.to_bytes().expect("Source closure receipt").to_vec(),
        ),
    );

    let replay = CustodyReplayV1 {
        caller_role: CallerRoleV1::Core,
        release_set: fixture.base.release_set,
        market: fixture.base.market.to_bytes(),
        realm: market.identity.realm_id.to_bytes(),
        context: fixture.base.market.to_bytes(),
        caller_program: CORE_PROGRAM_ID.to_bytes(),
        rent_refund: fixture.base.rent_credit.to_bytes(),
        open_vault_count: 1,
        next_revision: CUSTODY_REVISION,
        generation: market.identity.generation,
        last_request_digest: [0xb1; 32],
        last_poststate_commitment: [0xb2; 32],
    };
    set_account(
        context,
        fixture.base.replay,
        protocol_account(
            CUSTODY_PROGRAM_ID,
            replay.to_bytes().expect("Custody replay").to_vec(),
        ),
    );

    let mut vault_data = vec![0_u8; SplAccount::LEN];
    SplAccount::pack(
        SplAccount {
            mint: fixture.base.mint,
            owner: fixture.base.custody_authority,
            amount: 0,
            delegate: COption::None,
            state: AccountState::Initialized,
            is_native: COption::None,
            delegated_amount: 0,
            close_authority: COption::None,
        },
        &mut vault_data,
    )
    .expect("empty Hoard vault");
    set_account(
        context,
        fixture.base.vault,
        protocol_account(Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID), vault_data),
    );
}

struct RetirementPlan {
    instruction: Instruction,
    direct_instruction: Instruction,
    activation_cache_digest: CoreContentId,
    resolution_facts: ResolutionRetirementReceiptFactsV3,
    expected_refund_delta: u64,
}

fn hostile_registry_retirement_continuation(
    fixture: &JoinedFixture,
    activation_cache_digest: CoreContentId,
    mut core: Instruction,
) -> Instruction {
    let roles = [
        ExecutionRoleV1::Core,
        ExecutionRoleV1::Claims,
        ExecutionRoleV1::Resolution,
        ExecutionRoleV1::Custody,
    ];
    let continuation = RegistryContinuationRequestV1::new(
        id(fixture.base.release_set),
        activation_cache_digest,
        id(hash(&core.data).to_bytes()),
        u32::try_from(core.data.len()).expect("bounded Core retirement instruction"),
        ExecutionRoleV1::Core,
        &roles,
    )
    .expect("Registry continuation header");
    let batch = continuation
        .role_batch_request()
        .expect("canonical retirement role batch");
    let seeds = RegistryContinuationAdmissionSeedsV1::new(
        continuation,
        fixture.base.activation.to_bytes(),
        id(hash(&batch.to_bytes()).to_bytes()),
    )
    .expect("continuation admission seeds");
    let release = seeds.release_set();
    let cache = seeds.activation_cache();
    let batch_digest = seeds.batch_request_digest();
    let mask = seeds.role_mask();
    let role = seeds.continuation_role();
    let continuation_digest = seeds.continuation_digest();
    let admission = Pubkey::find_program_address(
        &[
            seeds.domain(),
            release.as_slice(),
            cache.as_slice(),
            batch_digest.as_slice(),
            mask.as_slice(),
            role.as_slice(),
            continuation_digest.as_slice(),
        ],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    core.accounts
        .push(AccountMeta::new_readonly(admission, false));

    let mut accounts = Vec::with_capacity(10 + core.accounts.len());
    accounts.extend_from_slice(&[
        AccountMeta::new_readonly(fixture.base.activation, false),
        AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.base.core_programdata, false),
        AccountMeta::new_readonly(JOINED_CLAIMS_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.claims_programdata, false),
        AccountMeta::new_readonly(RESOLUTION_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.base.resolution_programdata, false),
        AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.base.custody_programdata, false),
        AccountMeta::new_readonly(admission, false),
    ]);
    accounts.extend(core.accounts);
    let mut data = Vec::with_capacity(REGISTRY_CONTINUATION_REQUEST_BYTES_V1 + core.data.len());
    data.extend_from_slice(&continuation.to_bytes());
    data.extend_from_slice(&core.data);
    Instruction {
        program_id: REGISTRY_PROGRAM_ID,
        accounts,
        data,
    }
}

async fn retirement_operator_snapshot(
    context: &mut ProgramTestContext,
    fixture: &JoinedFixture,
) -> MarketRetirementSnapshotV1 {
    let escrow = failure_escrow_v1(
        JOINED_CLAIMS_PROGRAM_ID,
        fixture.base.market.to_bytes(),
        fixture.claims_aggregate,
        RETIREMENT_CLAIM_COUNT,
    )
    .expect("this campaign's width seats a failure coordinate");
    MarketRetirementSnapshotV1 {
        market: required_observed(context, fixture.base.market).await,
        rent_credit: required_observed(context, fixture.base.rent_credit).await,
        activation_cache: required_observed(context, fixture.base.activation).await,
        registry_program: required_observed(context, REGISTRY_PROGRAM_ID).await,
        core_program: required_observed(context, CORE_PROGRAM_ID).await,
        core_programdata: required_observed(context, fixture.base.core_programdata).await,
        claims_program: required_observed(context, JOINED_CLAIMS_PROGRAM_ID).await,
        claims_programdata: required_observed(context, fixture.claims_programdata).await,
        resolution_program: required_observed(context, RESOLUTION_PROGRAM_ID).await,
        resolution_programdata: required_observed(context, fixture.base.resolution_programdata)
            .await,
        custody_program: required_observed(context, CUSTODY_PROGRAM_ID).await,
        custody_programdata: required_observed(context, fixture.base.custody_programdata).await,
        rent_program: required_observed(context, RENT_PROGRAM_ID).await,
        source_receipt: required_observed(context, fixture.base.closure).await,
        claims_aggregate: required_observed(context, fixture.claims_aggregate).await,
        custody_replay: required_observed(context, fixture.base.replay).await,
        hoard_vault: required_observed(context, fixture.base.vault).await,
        custody_authority: observed_or_vacant(context, fixture.base.custody_authority).await,
        collateral_mint: required_observed(context, fixture.base.mint).await,
        collateral_token_program: required_observed(
            context,
            Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID),
        )
        .await,
        realm_raw: required_observed(context, fixture.base.realm_record.raw).await,
        realm_staging: observed_or_vacant(context, fixture.base.realm_record.staging).await,
        infrastructure_profile: required_observed(context, fixture.infrastructure_profile).await,
        registry_artifact_raw: required_observed(context, fixture.registry_artifact.raw).await,
        registry_artifact_staging: observed_or_vacant(context, fixture.registry_artifact.staging)
            .await,
        registry_programdata: required_observed(context, fixture.base.registry_programdata).await,
        rent_artifact_raw: required_observed(context, fixture.rent_artifact.raw).await,
        rent_artifact_staging: observed_or_vacant(context, fixture.rent_artifact.staging).await,
        rent_programdata: required_observed(context, fixture.rent_programdata).await,
        rent_sysvar: required_observed(context, sysvar::rent::ID).await,
        refund_wallet: required_observed(context, fixture.refund_wallet).await,
        // Derived rather than declared, and OBSERVED rather than assumed: on a
        // categorical prestate the three addresses hold nothing, `observed`
        // returns `None`, and the plan is the exact thirty-five-account one
        // that shipped. On the refunding prestate below they are live and the
        // plan grows the escrow tail decision 0025's burn needs.
        failure_escrow_position: observed_or_none(context, escrow.position).await,
        failure_escrow_admission: observed_or_none(context, escrow.admission).await,
        linked_basis_record: observed_or_none(context, linked_basis_record(fixture.base.market))
            .await,
    }
}

/// This campaign's linked `ProductBasisV3` record address.
///
/// Not a PDA of any program: the harness plants the record itself, and the
/// closure's join to it is CONTENT-addressed -- the aggregate's `basis_id` is
/// the record's own semantic identity -- so the address is free and the bytes
/// are not.
fn linked_basis_record(market: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"dclutch/harness/linked-basis/v1", market.as_ref()],
        &REGISTRY_PROGRAM_ID,
    )
    .0
}

/// One observation, or `None` when the address holds nothing at all.
///
/// `observed_or_vacant` is the wrong shape for the escrow tail: a vacant
/// observation still occupies a key in the snapshot's alias sweep and would
/// claim the Market HAS an escrow whose accounts are empty. Absence is `None`.
async fn observed_or_none(
    context: &mut ProgramTestContext,
    key: Pubkey,
) -> Option<dclutch_market_retirement_v1_operator::ObservedAccount> {
    observed(context, key)
        .await
        .map(|account| into_observed(key, account))
}

async fn retirement_instruction(
    context: &mut ProgramTestContext,
    fixture: &JoinedFixture,
) -> RetirementPlan {
    let snapshot = retirement_operator_snapshot(context, fixture).await;
    let activation_cache_digest = id(hash(&snapshot.activation_cache.data).to_bytes());
    let report = build_market_retirement_v1(&snapshot).expect("chain-derived aggregate retirement");
    RetirementPlan {
        instruction: report.instruction,
        direct_instruction: report.direct_instruction,
        activation_cache_digest,
        resolution_facts: report.resolution_facts,
        expected_refund_delta: report.expected_refund_delta,
    }
}

async fn joined_snapshot(
    context: &mut ProgramTestContext,
    fixture: &JoinedFixture,
) -> JoinedSnapshot {
    JoinedSnapshot {
        market: observed(context, fixture.base.market).await,
        rent_credit: observed(context, fixture.base.rent_credit).await,
        claims_aggregate: observed(context, fixture.claims_aggregate).await,
        custody_replay: observed(context, fixture.base.replay).await,
        hoard_vault: observed(context, fixture.base.vault).await,
        source_receipt: observed(context, fixture.base.closure).await,
        refund_wallet: observed(context, fixture.refund_wallet).await,
    }
}

async fn execute_same_lineage_funding_and_open(
    context: &mut ProgramTestContext,
    fixture: &JoinedFixture,
) {
    let payer = context.payer.pubkey();
    let before_create = open_rollback_snapshot(context, &fixture.base).await;
    let create = build_resolution_create_fund_v3(&create_snapshot(context, &fixture.base).await)
        .expect("chain-derived same-Market CreateFund");
    validate_resolution_create_fund_report_v3(&create).expect("exact same-Market CreateFund");
    let mut create_instructions = Vec::with_capacity(2);
    create_instructions.push(transfer(
        &payer,
        &fixture.base.source,
        create.source_top_up_lamports,
    ));
    create_instructions.push(create.instruction.clone());
    let mut substituted_system = create_instructions.clone();
    substituted_system
        .last_mut()
        .expect("CreateFund instruction")
        .accounts[15]
        .pubkey = sysvar::rent::ID;
    assert!(
        submit(context, &substituted_system).await.is_err(),
        "a substituted System program must refuse after the Source top-up"
    );
    assert_eq!(
        open_rollback_snapshot(context, &fixture.base).await,
        before_create,
        "late CreateFund refusal rolls the Market, Source, subset ledger, Custody, and RentCredit back"
    );
    submit(context, &create_instructions)
        .await
        .expect("create the exact same-Market Source against the pending subset ledger");
    assert_funding_ledger_status(context, &fixture.base, FundingLedgerStatusV2::Pending).await;

    let activation = build_resolution_activate_fund_v1(&ResolutionActivateFundSnapshotV1 {
        pending: verify_snapshot(context, &fixture.base).await,
        system_program: required_observed(context, system_program::ID).await,
    })
    .expect("chain-derived direct same-Market activation");
    let mut activation_instructions = Vec::with_capacity(2);
    if activation.receipt_top_up_lamports != 0 {
        activation_instructions.push(transfer(
            &payer,
            &fixture.base.activation_receipt,
            activation.receipt_top_up_lamports,
        ));
    }
    activation_instructions.push(activation.instruction);
    let before_activation = open_rollback_snapshot(context, &fixture.base).await;
    let mut read_only_beneficiary = activation_instructions.clone();
    read_only_beneficiary
        .last_mut()
        .expect("direct activation instruction")
        .accounts[13]
        .is_writable = false;
    assert!(
        submit(context, &read_only_beneficiary).await.is_err(),
        "a read-only activation beneficiary must refuse"
    );
    assert_eq!(
        open_rollback_snapshot(context, &fixture.base).await,
        before_activation,
        "direct activation privilege refusal rolls its receipt top-up and every funding mutation back"
    );
    submit(context, &activation_instructions)
        .await
        .expect("activate the exact same-Market three-row subset ledger");
    assert_funding_ledger_status(context, &fixture.base, FundingLedgerStatusV2::Active).await;

    let verify =
        build_resolution_verify_fund_ready_v3(&verify_snapshot(context, &fixture.base).await)
            .expect("chain-derived no-CPI same-Market funding Accept");
    validate_resolution_verify_fund_ready_report_v3(&verify)
        .expect("exact same-Market funding Accept");
    let before_accept = open_rollback_snapshot(context, &fixture.base).await;
    let mut writable_beneficiary = verify.instruction.clone();
    writable_beneficiary.accounts[14].is_writable = true;
    assert!(
        submit(context, &[writable_beneficiary]).await.is_err(),
        "surplus writable Accept beneficiary must refuse"
    );
    assert_eq!(
        open_rollback_snapshot(context, &fixture.base).await,
        before_accept,
        "funding Accept privilege refusal preserves the activated ledger"
    );
    submit(context, &[verify.instruction])
        .await
        .expect("accept the durable activation receipt into Core readiness");

    let before_open = open_rollback_snapshot(context, &fixture.base).await;
    let mut substituted_admission =
        open_instruction(context, &fixture.base, payer, OperationV1::InitializeReplay).await;
    substituted_admission
        .accounts
        .last_mut()
        .expect("Registry continuation admission")
        .pubkey = Pubkey::new_unique();
    assert!(
        submit(context, &[substituted_admission]).await.is_err(),
        "a substituted Registry continuation admission must refuse"
    );
    assert_eq!(
        open_rollback_snapshot(context, &fixture.base).await,
        before_open,
        "late Registry refusal rolls Market and all Custody creation back"
    );
    for operation in [OperationV1::InitializeReplay, OperationV1::OpenVault] {
        let instruction = open_instruction(context, &fixture.base, payer, operation).await;
        submit(context, &[instruction])
            .await
            .expect("transaction-produce the same-Market Custody replay and Hoard vault");
    }
    let market = CoreState::decode(
        &observed(context, fixture.base.market)
            .await
            .expect("opened same-Market")
            .data,
    )
    .expect("Core Market state");
    assert_eq!(market.phase, Phase::Open);
    assert_eq!(market.readiness, Readiness::Consumed);
    let replay = CustodyReplayV1::decode(
        &observed(context, fixture.base.replay)
            .await
            .expect("transaction-created Custody replay")
            .data,
    )
    .expect("Custody replay state");
    assert_eq!(replay.next_revision, CUSTODY_REVISION);
    assert_eq!(replay.open_vault_count, 1);
}

async fn execute_same_lineage_real_provider(
    context: &mut ProgramTestContext,
    fixture: &JoinedFixture,
) {
    execute_same_lineage_funding_and_open(context, fixture).await;
    let encoded_vaa =
        pyth_provider::initialize_real_providers(context, fixture.base.provider).await;
    let mut clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("ProgramTest Clock");
    clock.slot = clock.slot.max(1);
    clock.unix_timestamp = TERMINAL_TIME;
    context.set_sysvar(&clock);

    let payer = context.payer.pubkey();
    let post_update_body = pyth_provider::RECEIVER_POST_UPDATE
        .get(8..)
        .expect("Receiver PostUpdate body")
        .to_vec();
    let submit_intent = ProviderSubmitIntentV3 {
        submitter: payer,
        refund_recipient: fixture.base.rent_credit,
        update_account: fixture.base.update.pubkey(),
        reclaim_after_unix_seconds: TERMINAL_TIME + 20,
        post_update_body: post_update_body.clone(),
    };
    let submit_report = build_provider_submit_v3(
        &provider_submit_snapshot(context, &fixture.base, encoded_vaa).await,
        provider_submit_deployment(&fixture.base),
        &submit_intent,
    )
    .expect("chain-derived real-provider submission");
    let lifecycle_rent = context
        .banks_client
        .get_rent()
        .await
        .expect("chain Rent")
        .minimum_balance(PROVIDER_UPDATE_LIFECYCLE_BYTES_V3);
    let before_profile_refusals =
        provider_rollback_snapshot(context, &fixture.base, submit_report.lifecycle).await;
    let mut substituted_profile = submit_report.instruction.clone();
    substituted_profile.accounts[7].pubkey = fixture.base.realm_record.raw;
    assert!(
        pyth_provider::submit(
            context,
            &[
                transfer(&payer, &submit_report.lifecycle, lifecycle_rent),
                substituted_profile,
            ],
            &[&fixture.base.update],
        )
        .await
        .is_err(),
        "a substituted immutable infrastructure profile must refuse"
    );
    assert_eq!(
        provider_rollback_snapshot(context, &fixture.base, submit_report.lifecycle).await,
        before_profile_refusals,
        "profile substitution rolls back provider, Source, funding, Market, Custody, and RentCredit"
    );
    let mut substituted_registry_artifact = submit_report.instruction.clone();
    substituted_registry_artifact.accounts[10].pubkey = fixture.rent_artifact.raw;
    assert!(
        pyth_provider::submit(
            context,
            &[
                transfer(&payer, &submit_report.lifecycle, lifecycle_rent),
                substituted_registry_artifact,
            ],
            &[&fixture.base.update],
        )
        .await
        .is_err(),
        "a different finalized infrastructure artifact must refuse"
    );
    assert_eq!(
        provider_rollback_snapshot(context, &fixture.base, submit_report.lifecycle).await,
        before_profile_refusals,
        "Registry-artifact substitution preserves every provider and Market resource"
    );
    pyth_provider::submit(
        context,
        &[
            transfer(&payer, &submit_report.lifecycle, lifecycle_rent),
            submit_report.instruction,
        ],
        &[&fixture.base.update],
    )
    .await
    .expect("captured Receiver accepts the provider update");

    let resolver = Keypair::new();
    let resolver_rent = context
        .banks_client
        .get_rent()
        .await
        .expect("chain Rent")
        .minimum_balance(0);
    submit(
        context,
        &[
            transfer(
                &payer,
                &fixture.base.certificate,
                Rent::default().minimum_balance(RESOLUTION_CERTIFICATE_BYTES_V2),
            ),
            transfer(&payer, &resolver.pubkey(), resolver_rent),
        ],
    )
    .await
    .expect("prepay terminal certificate and distinct resolver");
    let execute_deployment = ProviderExecuteDeploymentV3 {
        trading_program: JOINED_TRADING_PROGRAM_ID,
        trading_programdata: fixture.trading_programdata,
        ..provider_execute_deployment(&fixture.base)
    };
    let execute_report = build_provider_execute_v3(
        &provider_execute_snapshot(context, &fixture.base, submit_report.lifecycle).await,
        execute_deployment,
        &ProviderExecuteIntentV3 {
            resolver: resolver.pubkey(),
            terminal_sequence: TERMINAL_SEQUENCE,
            post_update_body,
        },
    )
    .expect("chain-derived direct Resolution provider execution");
    let before_inactive_role_substitution =
        provider_rollback_snapshot(context, &fixture.base, submit_report.lifecycle).await;
    let mut substituted_inactive_trading = execute_report.instruction.clone();
    substituted_inactive_trading.accounts[13].pubkey = CUSTODY_PROGRAM_ID;
    assert!(
        pyth_provider::submit(context, &[substituted_inactive_trading], &[&resolver])
            .await
            .is_err(),
        "inactive Trading deployment identity remains release-bound"
    );
    assert_eq!(
        provider_rollback_snapshot(context, &fixture.base, submit_report.lifecycle).await,
        before_inactive_role_substitution,
        "inactive-role substitution rolls provider, Source, Market, funding, Custody, and RentCredit back"
    );
    pyth_provider::submit(context, &[execute_report.instruction], &[&resolver])
        .await
        .expect("Resolution persists the authenticated provider result");
    let accept = build_resolution_admit_terminal_v3(&admit_snapshot(context, &fixture.base).await)
        .expect("chain-derived no-CPI terminal Accept");
    submit(context, &[accept.instruction])
        .await
        .expect("Core accepts the durable provider certificate");

    let market = CoreState::decode(
        &observed(context, fixture.base.market)
            .await
            .expect("provider-terminal Market")
            .data,
    )
    .expect("Core Market state");
    assert_eq!(market.phase, Phase::Terminal);
    let source = SourceResolutionStateV2::decode(
        &observed(context, fixture.base.source)
            .await
            .expect("provider-resolved Source")
            .data,
    )
    .expect("Source state");
    assert_eq!(source.phase(), SourceResolutionPhaseV1::Resolved);
    let lifecycle = ProviderUpdateLifecycleV3::decode(
        &observed(context, submit_report.lifecycle)
            .await
            .expect("consumed provider lifecycle")
            .data,
    )
    .expect("provider lifecycle");
    assert_eq!(lifecycle.status, ProviderUpdateStatusV3::Consumed);
}

#[tokio::test]
async fn joined_retirement_is_atomic_through_rent_close_last() {
    let (fixture, mut context) = joined_fixture().await;
    execute_same_lineage_real_provider(&mut context, &fixture).await;
    let mut clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("ProgramTest Clock");
    clock.unix_timestamp = TERMINAL_TIME + 1;
    context.set_sysvar(&clock);

    submit(&mut context, &[begin_retiring_instruction(&fixture.base)])
        .await
        .expect("permissionless BeginRetiring");
    let closure_rent = Rent::default().minimum_balance(SOURCE_CLOSURE_RECEIPT_BYTES_V3);
    let payer = context.payer.pubkey();
    submit(
        &mut context,
        &[transfer(&payer, &fixture.base.closure, closure_rent)],
    )
    .await
    .expect("prepay Source closure receipt");
    let close =
        build_resolution_direct_close_fund_v1(&close_snapshot(&mut context, &fixture.base).await)
            .expect("chain-derived direct CloseFund");
    let expected_resolution_facts = close.expected_retirement_facts;
    submit(&mut context, &[close.instruction.clone()])
        .await
        .expect("Resolution closes Source subtree first");
    let closure = SourceClosureReceiptV3::decode(
        &observed(&mut context, fixture.base.closure)
            .await
            .expect("V3 Source closure receipt")
            .data,
    )
    .expect("exhaustive V3 Source closure receipt");
    assert_exhaustive_closure_receipt(closure, &close);

    let plan = retirement_instruction(&mut context, &fixture).await;
    assert_eq!(
        plan.resolution_facts, expected_resolution_facts,
        "the retirement builder must carry every Resolution-owned closure component unchanged"
    );
    let chain_snapshot = retirement_operator_snapshot(&mut context, &fixture).await;
    let mut stale_observation = chain_snapshot.clone();
    stale_observation.claims_aggregate.observation.slot += 1;
    assert_eq!(
        build_market_retirement_v1(&stale_observation),
        Err(MarketRetirementOperatorErrorV1::Observation),
        "one stale child observation cannot enter the packet"
    );
    let mut stale_release = chain_snapshot.clone();
    stale_release.activation_cache.data[0] ^= 1;
    assert_eq!(
        build_market_retirement_v1(&stale_release),
        Err(MarketRetirementOperatorErrorV1::Release),
        "a stale or substituted release cache must refuse"
    );
    let mut swapped_child = chain_snapshot.clone();
    swapped_child.claims_aggregate = chain_snapshot.source_receipt.clone();
    assert_eq!(
        build_market_retirement_v1(&swapped_child),
        Err(MarketRetirementOperatorErrorV1::Frame),
        "a Resolution receipt cannot alias the Claims aggregate"
    );
    let mut nonempty_custody = chain_snapshot.clone();
    let mut token = SplAccount::unpack(&nonempty_custody.hoard_vault.data).expect("Hoard vault");
    token.amount = 1;
    SplAccount::pack(token, &mut nonempty_custody.hoard_vault.data).expect("hostile token state");
    assert_eq!(
        build_market_retirement_v1(&nonempty_custody),
        Err(MarketRetirementOperatorErrorV1::Custody),
        "partial Custody settlement cannot retire"
    );
    let report = build_market_retirement_v1(&chain_snapshot).expect("retirement report");
    assert_eq!(report.claim_count, 5, "Claims width is runtime-derived");
    assert_ne!(
        plan.resolution_facts.funding_set_digest, [0; 32],
        "retirement carries the exact three-row Resolution funding prestate digest"
    );
    assert_eq!(plan.direct_instruction.accounts.len(), 35);
    assert_eq!(plan.direct_instruction.data.len(), 2_152);
    assert_eq!(plan.instruction.accounts.len(), 46);
    assert_eq!(plan.instruction.data.len(), 2_280);
    let before = joined_snapshot(&mut context, &fixture).await;

    assert!(
        submit(&mut context, std::slice::from_ref(&plan.direct_instruction))
            .await
            .is_err(),
        "direct Core retirement without a Registry continuation must refuse"
    );
    assert_eq!(
        joined_snapshot(&mut context, &fixture).await,
        before,
        "direct-route refusal preserves every byte and lamport"
    );

    let mut missing_source = plan.direct_instruction.clone();
    missing_source.accounts.remove(13);
    let missing_source = hostile_registry_retirement_continuation(
        &fixture,
        plan.activation_cache_digest,
        missing_source,
    );
    assert!(
        submit(&mut context, &[missing_source]).await.is_err(),
        "missing Resolution receipt must refuse"
    );
    assert_eq!(
        joined_snapshot(&mut context, &fixture).await,
        before,
        "missing receipt preserves every byte and lamport"
    );

    let mut substituted_source = plan.direct_instruction.clone();
    substituted_source.accounts[13].pubkey = fixture.base.certificate;
    let substituted_source = hostile_registry_retirement_continuation(
        &fixture,
        plan.activation_cache_digest,
        substituted_source,
    );
    assert!(
        submit(&mut context, &[substituted_source]).await.is_err(),
        "substituted Resolution-owned certificate must not stand in for closure evidence"
    );
    assert_eq!(
        joined_snapshot(&mut context, &fixture).await,
        before,
        "receipt substitution preserves every byte and lamport"
    );

    let mut reordered = plan.direct_instruction.clone();
    const CLOSE_VAULT_OFFSET: usize = 72 + 480 + 256;
    const CUSTODY_BYTES: usize = 672;
    let first = reordered.data[CLOSE_VAULT_OFFSET..CLOSE_VAULT_OFFSET + CUSTODY_BYTES].to_vec();
    let second = reordered.data
        [CLOSE_VAULT_OFFSET + CUSTODY_BYTES..CLOSE_VAULT_OFFSET + 2 * CUSTODY_BYTES]
        .to_vec();
    reordered.data[CLOSE_VAULT_OFFSET..CLOSE_VAULT_OFFSET + CUSTODY_BYTES].copy_from_slice(&second);
    reordered.data[CLOSE_VAULT_OFFSET + CUSTODY_BYTES..CLOSE_VAULT_OFFSET + 2 * CUSTODY_BYTES]
        .copy_from_slice(&first);
    let reordered =
        hostile_registry_retirement_continuation(&fixture, plan.activation_cache_digest, reordered);
    assert!(
        submit(&mut context, &[reordered]).await.is_err(),
        "ordered CloseVault/CloseReplay evidence cannot be reversed"
    );
    assert_eq!(
        joined_snapshot(&mut context, &fixture).await,
        before,
        "ordered-evidence refusal preserves every byte and lamport"
    );

    let mut late_rent_refusal = plan.direct_instruction.clone();
    late_rent_refusal.accounts[34].pubkey = Pubkey::new_from_array([0xf1; 32]);
    let late_rent_refusal = hostile_registry_retirement_continuation(
        &fixture,
        plan.activation_cache_digest,
        late_rent_refusal,
    );
    assert!(
        submit(&mut context, &[late_rent_refusal]).await.is_err(),
        "substituted Core-derived Rent close signer must refuse after child closure work"
    );
    assert_eq!(
        joined_snapshot(&mut context, &fixture).await,
        before,
        "late Rent refusal rolls Claims, Custody, Market, credit, and wallet bytes/lamports back"
    );

    let wallet_before = before
        .refund_wallet
        .as_ref()
        .expect("refund wallet prestate")
        .lamports;
    submit(&mut context, &[plan.instruction])
        .await
        .expect("ordered Resolution -> Claims -> Custody -> Market -> Rent retirement");
    let after = joined_snapshot(&mut context, &fixture).await;
    assert!(after.market.is_none(), "Core Market is closed");
    assert!(
        after.rent_credit.is_none(),
        "Rent closes lifecycle credit last"
    );
    assert!(
        after.claims_aggregate.is_none(),
        "Claims aggregate is closed"
    );
    assert!(after.custody_replay.is_none(), "Custody replay is closed");
    assert!(after.hoard_vault.is_none(), "empty Hoard vault is closed");
    assert_eq!(after.source_receipt, before.source_receipt);
    assert_eq!(
        after
            .refund_wallet
            .expect("immutable refund wallet")
            .lamports,
        wallet_before + plan.expected_refund_delta,
        "the sole immutable wallet receives exact credit + Claims + Custody + Market lamports"
    );
}

/// The four checkpoint routes' extents, one per route.
///
/// Every hostile submits the same instruction as the honest route it attacks,
/// so a class has one extent and the hostiles check it too -- a hostile nobody
/// could submit refuses nothing, which is the second reason to convert them.
///
/// These are DATA-bound frames, and the figures say so: 35 metas each, of which
/// 33 become one-byte indexes, but the requests are 744 to 864 bytes and no
/// table touches those. The aggregate retirement was already split into four
/// transactions to get here; the table is what carries the split under the
/// packet, and there is no third lever left if a request grows.
const PREPARE_EXTENT: PacketExtentV1 = PacketExtentV1 {
    legacy_bytes: 2_101,
    v0_bytes: 1_083,
    static_keys: 2,
    loaded_addresses: 34,
};

const CLOSE_VAULT_EXTENT: PacketExtentV1 = PacketExtentV1 {
    legacy_bytes: 2_157,
    v0_bytes: 1_139,
    static_keys: 2,
    loaded_addresses: 34,
};

const CLOSE_REPLAY_EXTENT: PacketExtentV1 = PacketExtentV1 {
    legacy_bytes: 2_157,
    v0_bytes: 1_139,
    static_keys: 2,
    loaded_addresses: 34,
};

const FINISH_EXTENT: PacketExtentV1 = PacketExtentV1 {
    legacy_bytes: 2_037,
    v0_bytes: 1_019,
    static_keys: 2,
    loaded_addresses: 34,
};

/// The substituted-wallet hostile is 32 bytes narrower than the honest finish:
/// the substitution collapses two coordinates onto one address, so the frame
/// carries one fewer unique key.
const FINISH_SUBSTITUTED_EXTENT: PacketExtentV1 = PacketExtentV1 {
    legacy_bytes: 2_005,
    v0_bytes: 1_018,
    static_keys: 2,
    loaded_addresses: 33,
};

/// The unique account LOCKS one instruction resolves.
///
/// Bytes are deliberately not measured here any more. Every checkpoint route is
/// now submitted as a v0 message over the chain's own frozen table and pins its
/// exact extent at the submission, so a second byte model computed here against
/// a synthetic table would be a parallel authority for a number the campaign
/// observes directly.
///
/// What survives is the wall a packet fix cannot move. The runtime's 64-lock
/// limit counts unique addresses whether they arrive inline or through a table,
/// so a frame that fits the packet can still be unsubmittable, and a document
/// that reports "fits" on a 63-key frame is reporting the wrong wall.
fn unique_account_locks(instruction: &Instruction, payer: Pubkey) -> usize {
    std::iter::once(payer)
        .chain(std::iter::once(instruction.program_id))
        .chain(instruction.accounts.iter().map(|meta| meta.pubkey))
        .collect::<std::collections::BTreeSet<_>>()
        .len()
}

/// THE WALL, on real ELFs, with a discriminant instead of an `is_err()`.
///
/// PROGRAMS-17B convicted the closure's `Liability` conjunct from the source
/// and from a HOST message: cohort-16.1's refusal arrived as a `BeginRetiring`
/// preflight sentence and no transaction was ever built, so the ON-CHAIN
/// refusal every reading since has named had no witness at all. This is its
/// first. "Nothing fired" and "my instrument was disconnected" log the same
/// way, and the whole of decision 0025's shape A is a repair to a refusal
/// nobody had seen refuse.
///
/// The residue is planted AFTER the plan is built, deliberately. Three authors
/// carry the same conjunct -- the operator's retirement builder
/// (`market-retirement-v1-operator`, which refuses `Claims`), the
/// `BeginRetiring` preflight, and the Claims program -- and only the third one
/// is the chain. Building against a zero-supply aggregate and then giving the
/// chain the column is how this test asks the third one instead of the first.
/// Nothing else about the packet moves: the revision the request expects is
/// unchanged, so the closure reaches its supply loop rather than refusing on
/// identity first.
#[tokio::test]
async fn a_seated_failure_column_refuses_the_claims_handoff_by_name() {
    let (fixture, mut context) = joined_fixture().await;
    seed_exact_retirement_prestate(&mut context, &fixture).await;
    let snapshot = retirement_operator_snapshot(&mut context, &fixture).await;
    let plan = build_checkpoint_market_retirement_v1(&snapshot)
        .expect("checkpointed plan against an empty aggregate");
    let (checkpoint_table, checkpoint_addresses) = frozen_route_lookup_table(
        &mut context,
        &[
            plan.prepare.clone(),
            plan.close_vault.clone(),
            plan.close_replay.clone(),
            plan.finish.clone(),
        ],
    )
    .await;

    // The state terminal settlement leaves on a refunding market: every
    // ordinary coordinate paid and gone, the failure column standing at the
    // quantity the founding seated in an escrow no key opens.
    const SEATED_RESIDUE: u64 = 166_666_667;
    let empty = required_observed(&mut context, fixture.claims_aggregate).await;
    let view = LiabilityBasisMarketViewV2::decode(&empty.data).expect("planted aggregate");
    let failure = u32::try_from(
        dclutch_product::economic_slice::refunding_failure_index(view.claim_count)
            .expect("a width that seats a failure coordinate"),
    )
    .expect("failure selector");
    let mut supplies = vec![0_u64; view.claim_count as usize];
    supplies[failure as usize] = SEATED_RESIDUE;
    let mut seated_bytes = vec![
        0;
        liability_basis_vector_width_v2(
            LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
            view.claim_count,
        )
        .expect("aggregate width")
    ];
    encode_liability_basis_market_into_v2(
        LiabilityBasisMarketInputV2 {
            revision: view.revision,
            logical_market: view.logical_market,
            release_set: view.release_set,
            registry_program: view.registry_program,
            product_instance_id: view.product_instance_id,
            basis_id: view.basis_id,
            realm_id: view.realm_id,
            custody_context: view.custody_context,
            generation: view.generation,
        },
        &supplies,
        &mut seated_bytes,
    )
    .expect("aggregate carrying the seated residue");
    let raw = observed(&mut context, fixture.claims_aggregate)
        .await
        .expect("raw aggregate");
    set_account(
        &mut context,
        fixture.claims_aggregate,
        Account {
            data: seated_bytes,
            ..raw
        },
    );

    // The host mirror refuses too, and it now names WHICH: a supply standing at
    // a coordinate this closure will not discharge, with no escrow tail to
    // discharge it. `Claims` used to be the same code eleven conjuncts carried,
    // so this assertion could only say "it refused"; it says what.
    let seated_snapshot = retirement_operator_snapshot(&mut context, &fixture).await;
    assert_eq!(
        build_checkpoint_market_retirement_v1(&seated_snapshot),
        Err(MarketRetirementOperatorErrorV1::UnescrowedSupply),
        "a seated residue with no escrow tail is an undischargeable supply, by name"
    );

    let before = joined_snapshot(&mut context, &fixture).await;
    let refusal = submit_recorded_over_table_v0(
        &mut context,
        std::slice::from_ref(&plan.prepare),
        &[],
        "retirement checkpoint: prepare against a seated refunding failure column",
        checkpoint_table,
        &checkpoint_addresses,
        PREPARE_EXTENT,
    )
    .await
    .expect_err("a nonzero failure supply must refuse the Claims handoff");
    let code = match &refusal {
        BanksClientError::TransactionError(TransactionError::InstructionError(
            _,
            InstructionError::Custom(code),
        )) => *code,
        other => panic!("expected a custom program error, got {other:?}"),
    };
    assert_eq!(
        code, CHECKPOINT_PREPARE_SEATED_RESIDUE_REFUSAL_V1,
        "the seated residue must refuse by the discriminant this lane names, not merely refuse"
    );
    assert_eq!(
        joined_snapshot(&mut context, &fixture).await,
        before,
        "the refusal is byte and lamport atomic: no checkpoint, no handoff, no refund"
    );
}

/// Failure-coordinate units cohort-16.1's founding seated in its escrow.
const SEATED_RESIDUE_V1: u64 = 166_666_667;

/// Plant the state a REFUNDING Market is in when terminal settlement is done.
///
/// Every ordinary coordinate paid and gone; the failure column standing at the
/// quantity the founding seated, in the Position the MARKET derives and nobody
/// holds a key to; that Position's admission beside it; and the linked
/// `ProductBasisV3` record that says the column is owed nothing under every
/// certificate.
///
/// The aggregate's `basis_id` is rewritten to the record's own semantic
/// identity rather than the record being built to match a declared id, because
/// that is the direction the join runs: `semantic_basis_id_v3` is the content
/// address a founding commits to, and the closure re-derives it from the bytes
/// it is handed. A test that declared both independently would pass while
/// proving nothing about the join.
///
/// Returns the escrow's addresses and the rent its pair holds.
async fn seed_refunding_failure_escrow_v1(
    context: &mut ProgramTestContext,
    fixture: &JoinedFixture,
) -> (FailureEscrowV1, u64) {
    let escrow = failure_escrow_v1(
        JOINED_CLAIMS_PROGRAM_ID,
        fixture.base.market.to_bytes(),
        fixture.claims_aggregate,
        RETIREMENT_CLAIM_COUNT,
    )
    .expect("a width that seats a failure coordinate");
    assert_eq!(
        escrow.failure_selector,
        RETIREMENT_CLAIM_COUNT - 1,
        "the failure coordinate is the kernel's, not this test's"
    );

    // A categorical basis that refunds on failure: `payout_scale` is
    // `basis_width - 1`, which is the sole author's rule and the only thing
    // that distinguishes this record from the legacy `Q = 1` shape.
    let mut basis_bytes = vec![
        0;
        basis_record_bytes_v3(
            BasisKindV3::CategoricalQ1,
            RETIREMENT_CLAIM_COUNT as usize,
            0,
            0
        )
        .expect("categorical record width")
    ];
    compile_basis_v3(
        BasisInputV3 {
            kind: BasisKindV3::CategoricalQ1,
            product_id: [0xb1; 32],
            result_domain_id: [0xb2; 32],
            coordinate_domain_id: [0xb3; 32],
            result_unit_id: [0xb4; 32],
            evaluator_release_id: [0xb5; 32],
            basis_width: RETIREMENT_CLAIM_COUNT,
            payout_scale: u64::from(RETIREMENT_CLAIM_COUNT - 1),
            knot_denominator: 1,
            knots: &[],
            terms: &[],
            failure_payouts: &[],
            price_gate_certificate_digest: [0; 32],
        },
        &mut basis_bytes,
    )
    .expect("a refunding categorical basis record");
    let semantic_basis_id = semantic_basis_id_v3(&basis_bytes).expect("semantic basis identity");
    assert!(
        ProductBasisV3::decode(&basis_bytes)
            .expect("record decodes")
            .refunds_on_failure(),
        "the fixture is only a fixture for this decision if the record refunds"
    );
    set_account(
        context,
        linked_basis_record(fixture.base.market),
        protocol_account(REGISTRY_PROGRAM_ID, basis_bytes),
    );

    // The aggregate terminal settlement leaves behind.
    let raw = observed(context, fixture.claims_aggregate)
        .await
        .expect("planted aggregate");
    let view = LiabilityBasisMarketViewV2::decode(&raw.data).expect("aggregate decodes");
    let mut supplies = vec![0_u64; RETIREMENT_CLAIM_COUNT as usize];
    supplies[escrow.failure_selector as usize] = SEATED_RESIDUE_V1;
    let mut aggregate_bytes = vec![
        0;
        liability_basis_vector_width_v2(
            LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
            RETIREMENT_CLAIM_COUNT,
        )
        .expect("aggregate width")
    ];
    encode_liability_basis_market_into_v2(
        LiabilityBasisMarketInputV2 {
            revision: view.revision,
            logical_market: view.logical_market,
            release_set: view.release_set,
            registry_program: view.registry_program,
            product_instance_id: view.product_instance_id,
            basis_id: semantic_basis_id,
            realm_id: view.realm_id,
            custody_context: view.custody_context,
            generation: view.generation,
        },
        &supplies,
        &mut aggregate_bytes,
    )
    .expect("aggregate carrying the seated residue");
    set_account(
        context,
        fixture.claims_aggregate,
        Account {
            data: aggregate_bytes,
            ..raw
        },
    );

    // The escrow's own Position, holding the column and nothing else.
    let mut position_bytes = vec![
        0;
        liability_basis_vector_width_v2(
            LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
            RETIREMENT_CLAIM_COUNT,
        )
        .expect("position width")
    ];
    encode_liability_basis_position_into_v2(
        LiabilityBasisPositionInputV2 {
            revision: 1,
            market_account: fixture.claims_aggregate.to_bytes(),
            owner: escrow.owner.to_bytes(),
            basis_id: semantic_basis_id,
        },
        &supplies,
        &mut position_bytes,
    )
    .expect("the escrow's seated Position");
    set_account(
        context,
        escrow.position,
        protocol_account(JOINED_CLAIMS_PROGRAM_ID, position_bytes),
    );

    // Its admission record, in the shape a founding writes: a ClaimsCapability
    // owner at this Market's failure descriptor and coordinate.
    let admission_request = ProtocolPositionRequestV2::new(ProtocolPositionRequestV2 {
        action: ProtocolPositionActionV2::Admit,
        owner_kind: ProtocolPositionOwnerKindV2::ClaimsCapability,
        presence: ProtocolPositionPresenceV2::Vacant,
        release_set: fixture.base.release_set,
        market: fixture.base.market.to_bytes(),
        position_owner: escrow.owner.to_bytes(),
        parent_request_digest: [0xc1; 32],
        rent_credit: fixture.base.rent_credit.to_bytes(),
        rent_program: RENT_PROGRAM_ID.to_bytes(),
        generation: GENERATION,
        expected_market_revision: CLAIMS_REVISION,
        expected_position_revision: 0,
        observed_position_lamports: 1,
        observed_admission_lamports: 1,
        position_rent_principal: 1,
        admission_rent_principal: 1,
        capability_descriptor: fixture.base.market.to_bytes(),
        capability_outcome: escrow.failure_selector,
    })
    .expect("escrow admission request");
    let admission = ProtocolPositionAdmissionV2::new(
        admission_request,
        ProtocolPositionAdmissionEvidenceV2 {
            product_record_digest: [0xc2; 32],
            semantic_basis_id,
            linked_basis_record_digest: [0xc3; 32],
            request_digest: [0xc4; 32],
            claims_program: JOINED_CLAIMS_PROGRAM_ID.to_bytes(),
            trading_program: JOINED_TRADING_PROGRAM_ID.to_bytes(),
            capability_descriptor: fixture.base.market.to_bytes(),
            capability_outcome: escrow.failure_selector,
            outcome_count: RETIREMENT_CLAIM_COUNT,
        },
    )
    .expect("escrow admission");
    set_account(
        context,
        escrow.admission,
        protocol_account(
            JOINED_CLAIMS_PROGRAM_ID,
            admission
                .to_state_bytes()
                .expect("admission bytes")
                .to_vec(),
        ),
    );
    let rent = required_observed(context, escrow.position).await.lamports
        + required_observed(context, escrow.admission).await.lamports;
    (escrow, rent)
}

/// The four extents a refunding retirement's packets occupy.
///
/// Three more accounts than the categorical walk on every packet -- the escrow
/// Position, its admission and the linked basis record -- and all four carry
/// them, because `aggregate_retirement_journal.rs` requires the operations of
/// one retirement to present an identical frame and the three suffix packets
/// never read what they carry. The request bytes do not move at all: shape A is
/// a FRAME change, not an ABI change, which is why a categorical retirement's
/// four extents above are untouched.
const REFUNDING_PREPARE_EXTENT: PacketExtentV1 = PacketExtentV1 {
    legacy_bytes: 2_200,
    v0_bytes: 1_089,
    static_keys: 2,
    loaded_addresses: 37,
};

const REFUNDING_CLOSE_VAULT_EXTENT: PacketExtentV1 = PacketExtentV1 {
    legacy_bytes: 2_256,
    v0_bytes: 1_145,
    static_keys: 2,
    loaded_addresses: 37,
};

const REFUNDING_CLOSE_REPLAY_EXTENT: PacketExtentV1 = PacketExtentV1 {
    legacy_bytes: 2_256,
    v0_bytes: 1_145,
    static_keys: 2,
    loaded_addresses: 37,
};

const REFUNDING_FINISH_EXTENT: PacketExtentV1 = PacketExtentV1 {
    legacy_bytes: 2_136,
    v0_bytes: 1_025,
    static_keys: 2,
    loaded_addresses: 37,
};

/// THE WALL, DISSOLVED: a refunding Market retires, on real ELFs, end to end.
///
/// This is the same fixture, the same four packets and the same assertions as
/// `checkpointed_retirement_is_packet_bounded_resumable_and_conserving`, with
/// ONE difference in the prestate: the failure column is standing in the escrow
/// the founding seated, and the frame carries the three accounts that discharge
/// it. Without them this exact state refuses `0x5503`
/// (`a_seated_failure_column_refuses_the_claims_handoff_by_name`, which still
/// builds the thirty-five-account plan against a zero aggregate and is the
/// negative control for this test).
///
/// What it proves, in the order the chain proves it:
///
/// - the builder ADMITS a seated column now, where it refused `Claims`;
/// - the four packets present one identical thirty-eight-account frame;
/// - `prepare` burns the column, closes the escrow's Position and its
///   admission, and hands their rent to the checkpoint -- so the aggregate's
///   own supply loop, the conjunct that had been the wall, passes on the
///   post-state of the burn rather than being relaxed;
/// - the escrow's two accounts are GONE, not merely empty: a residue admitted
///   without being burned would strand them and their rent forever;
/// - the three remaining packets run unchanged and the Market reaches Retired;
/// - and the refund wallet receives the escrow's rent along with everything
///   else, to the lamport.
#[tokio::test]
async fn a_refunding_market_retires_once_the_closure_burns_its_failure_column() {
    let (fixture, mut context) = joined_fixture().await;
    seed_exact_retirement_prestate(&mut context, &fixture).await;
    let (escrow, escrow_rent) = seed_refunding_failure_escrow_v1(&mut context, &fixture).await;

    let snapshot = retirement_operator_snapshot(&mut context, &fixture).await;
    let plan = build_checkpoint_market_retirement_v1(&snapshot)
        .expect("a seated failure column no longer forecloses the plan");
    assert_eq!(
        plan.burned_failure_units, SEATED_RESIDUE_V1,
        "the plan says exactly what the closure will burn"
    );
    assert_eq!(plan.failure_escrow_rent_lamports, escrow_rent);
    for instruction in [
        &plan.prepare,
        &plan.close_vault,
        &plan.close_replay,
        &plan.finish,
    ] {
        assert_eq!(
            instruction.accounts.len(),
            38,
            "every packet of one retirement presents the same frame"
        );
    }
    assert_eq!(
        plan.prepare.accounts[35].pubkey, escrow.position,
        "the escrow Position is the first trailing account"
    );
    assert_eq!(plan.prepare.accounts[36].pubkey, escrow.admission);
    assert_eq!(
        plan.prepare.accounts[37].pubkey,
        linked_basis_record(fixture.base.market)
    );
    assert!(
        plan.prepare.accounts[35].is_writable && plan.prepare.accounts[36].is_writable,
        "the pair the closure closes is writable"
    );
    assert!(
        !plan.prepare.accounts[37].is_writable,
        "the record is read, never written"
    );

    let (table, addresses) = frozen_route_lookup_table(
        &mut context,
        &[
            plan.prepare.clone(),
            plan.close_vault.clone(),
            plan.close_replay.clone(),
            plan.finish.clone(),
        ],
    )
    .await;
    let census: Vec<usize> = [
        &plan.prepare,
        &plan.close_vault,
        &plan.close_replay,
        &plan.finish,
    ]
    .iter()
    .map(|instruction| unique_account_locks(instruction, context.payer.pubkey()))
    .collect();

    let wallet_before = required_observed(&mut context, fixture.refund_wallet)
        .await
        .lamports;
    let checkpoint_lamports_before = required_observed(&mut context, fixture.claims_aggregate)
        .await
        .lamports;

    submit_recorded_over_table_v0(
        &mut context,
        std::slice::from_ref(&plan.prepare),
        &[],
        "refunding retirement: prepare burns the failure column",
        table,
        &addresses,
        REFUNDING_PREPARE_EXTENT,
    )
    .await
    .expect("the closure discharges the column it used to refuse");

    let checkpoint = required_observed(&mut context, fixture.claims_aggregate).await;
    assert_eq!(
        checkpoint.owner, CORE_PROGRAM_ID,
        "the aggregate became Core's retirement checkpoint"
    );
    assert_eq!(
        checkpoint.lamports,
        checkpoint_lamports_before + escrow_rent,
        "the escrow's rent went to the checkpoint, not to a fourth account"
    );
    assert!(
        observed(&mut context, escrow.position).await.is_none(),
        "the escrow's Position is CLOSED, not merely emptied"
    );
    assert!(
        observed(&mut context, escrow.admission).await.is_none(),
        "the escrow's admission is closed with it"
    );
    assert!(
        observed(&mut context, linked_basis_record(fixture.base.market))
            .await
            .is_some(),
        "the record is evidence, not a resource this act consumes"
    );

    submit_recorded_over_table_v0(
        &mut context,
        std::slice::from_ref(&plan.close_vault),
        &[],
        "refunding retirement: close vault",
        table,
        &addresses,
        REFUNDING_CLOSE_VAULT_EXTENT,
    )
    .await
    .expect("the empty Hoard vault closes");
    submit_recorded_over_table_v0(
        &mut context,
        std::slice::from_ref(&plan.close_replay),
        &[],
        "refunding retirement: close replay",
        table,
        &addresses,
        REFUNDING_CLOSE_REPLAY_EXTENT,
    )
    .await
    .expect("the Custody replay cursor closes");
    submit_recorded_over_table_v0(
        &mut context,
        std::slice::from_ref(&plan.finish),
        &[],
        "refunding retirement: finish",
        table,
        &addresses,
        REFUNDING_FINISH_EXTENT,
    )
    .await
    .expect("Core, the checkpoint and the lifecycle credit close last");

    let after = joined_snapshot(&mut context, &fixture).await;
    assert!(after.market.is_none(), "Core Market is closed");
    assert!(
        after.rent_credit.is_none(),
        "the lifecycle credit is closed"
    );
    assert!(
        after.claims_aggregate.is_none(),
        "the retirement checkpoint is closed"
    );
    assert!(after.custody_replay.is_none(), "Custody replay is closed");
    assert!(after.hoard_vault.is_none(), "empty Hoard vault is closed");
    assert_eq!(
        after
            .refund_wallet
            .expect("immutable refund wallet")
            .lamports,
        wallet_before + plan.expected_refund_delta,
        "the refund wallet receives the escrow's rent along with everything else, to the lamport"
    );
    assert!(
        plan.expected_refund_delta > escrow_rent,
        "and the escrow's rent is inside that number rather than beside it"
    );
    assert!(
        census.iter().all(|keys| *keys <= 64),
        "three more accounts per packet stays under the runtime lock wall: {census:?}"
    );
}

/// WHAT THIS CAMPAIGN COVERS OF THE TERMINAL SEQUENCE, AND WHAT IT DOES NOT.
///
/// This file drives a market to `Retired` on real ELFs, and for three cohorts
/// that fact was read as coverage of the whole retirement. It is not. The
/// terminal sequence is six protocol mutations, declared once in
/// `dclutch_market_retirement_v1_operator::terminal_stage_order_v1`; this
/// campaign drives the LAST one, `AggregateRetirement`, decomposed into its four
/// checkpoint packets, and it drives it against a prestate
/// `seed_exact_retirement_prestate` writes directly: `outstanding_capabilities =
/// 0` and a `SourceClosureReceiptV3` set into the account rather than produced
/// by `ResolutionCloseFund`.
///
/// So the two stages whose ORDER the ruling fixes -- `DirectCloseCapability`,
/// which preserves the Resolution dependency funding ledger, and
/// `ResolutionCloseFund`, which closes it -- are not in this walk at all. This
/// campaign agreed with the wrong order because it could not see it, and the
/// pair was executed for the first time on devnet, in cohort-17, on a market
/// that can never be repaired.
///
/// This test is the boundary said out loud. It reads the ONE declaration rather
/// than restating an order, holds the ruled adjacency, refuses the inverted one
/// by name, and pins the two prestate facts that make this campaign blind to the
/// pair. Covering the pair on real ELFs is OWED: it needs a fixture with a live
/// Direct capability child and both physical funding ledgers, which this
/// fixture has never had.
#[tokio::test]
async fn the_checkpoint_campaign_drives_the_last_declared_stage_and_not_the_ruled_pair() {
    assert_eq!(
        TerminalStageV1::ORDERED.last().copied(),
        Some(TerminalStageV1::AggregateRetirement),
        "the four packets below are the last declared stage"
    );
    assert!(
        TerminalStageV1::DirectCloseCapability.ordinal()
            < TerminalStageV1::ResolutionCloseFund.ordinal(),
        "the stage that preserves the dependency ledger runs before its owner closes it"
    );
    let inverted = [
        TerminalStageV1::CoreBeginRetiring,
        TerminalStageV1::DirectBeginRetiring,
        TerminalStageV1::ResolutionCloseFund,
        TerminalStageV1::DirectCloseCapability,
        TerminalStageV1::RetirementReplayHandoff,
        TerminalStageV1::AggregateRetirement,
    ];
    assert_eq!(
        authenticate_terminal_stage_order_v1(&inverted),
        Err(TerminalStageOrderErrorV1::ResolutionCloseFundBeforeDirectClose),
        "the order this campaign could not have caught refuses at the declaration"
    );
    authenticate_terminal_stage_order_v1(&TerminalStageV1::ORDERED).expect("the ruled order");

    // The two prestate facts that make this campaign blind to the pair, read
    // off the fixture rather than asserted about it.
    let (fixture, mut context) = joined_fixture().await;
    seed_exact_retirement_prestate(&mut context, &fixture).await;
    let market = CoreState::decode(
        &required_observed(&mut context, fixture.base.market)
            .await
            .data,
    )
    .expect("seeded Core Market");
    assert_eq!(
        market.outstanding_capabilities, 0,
        "no capability child stands, so DirectCloseCapability has nothing to close here"
    );
    assert_eq!(market.phase, Phase::Retiring);
    let closure = required_observed(&mut context, fixture.base.closure).await;
    assert_eq!(
        closure.owner, RESOLUTION_PROGRAM_ID,
        "the Source closure receipt is SEEDED at this owner, not produced by ResolutionCloseFund"
    );
}

/// A Position that is not this Market's escrow refuses by this discriminant.
///
/// `0x5010` is the code the complete-set gate, the signed-delta waist and the
/// founding all already raise for "the named Position is not this Market's
/// escrow". The closure's burn is the fourth route and it makes the same
/// accusation, so an operator reading a failed retirement reaches one page.
const CLOSURE_BURN_WRONG_ESCROW_REFUSAL_V1: u32 =
    dclutch_claims_sbf::ClaimsSbfError::FailureEscrow as u32;

/// A linked basis record that is not the one this Market was founded on, or one
/// whose Market does not refund on failure.
const CLOSURE_BURN_SUBSTITUTED_BASIS_REFUSAL_V1: u32 =
    dclutch_claims_sbf::market_closure_v1::ClaimsMarketClosureSbfErrorV1::Basis as u32;

/// The four hostiles decision 0025's fourth addendum names for the burn, on
/// real ELFs, each by discriminant.
///
/// Every one of them is submitted against the SAME prestate the honest walk
/// runs on, and the honest packet is submitted last from the same fixture as
/// the positive control -- so a hostile that refused because the setup was
/// broken would take the control down with it.
///
/// What each one is:
///
/// - **Not the derived escrow.** The frame's trailing Position is swapped for a
///   canonical Position at another owner. The residue rule is an equality
///   against THE escrow's balance and nothing else, so an index that merely
///   holds the right number is not a licence.
/// - **A substituted basis record.** A valid `ProductBasisV3` that this Market
///   was not founded on. The join is content-addressed -- the aggregate's
///   `basis_id` IS the record's semantic identity -- so a substitution cannot
///   be made to agree. This is also the categorical case: a record that does
///   not refund on failure licenses no burn.
/// - **A stranger holding part of the column.** The aggregate carries one unit
///   more at the failure coordinate than the escrow holds. That unit is in
///   hands that can be paid, so it is an outstanding liability rather than a
///   residue and the closure refuses the conjunct it always refused.
/// - **An escrow holding a tradeable claim too.** Its holder is keyless but its
///   claim is not worthless, and burning the whole Position would destroy a
///   payable one.
#[tokio::test]
async fn the_closure_burn_refuses_its_four_hostiles_by_discriminant() {
    let (fixture, mut context) = joined_fixture().await;
    seed_exact_retirement_prestate(&mut context, &fixture).await;
    let (escrow, _) = seed_refunding_failure_escrow_v1(&mut context, &fixture).await;
    let snapshot = retirement_operator_snapshot(&mut context, &fixture).await;
    let plan = build_checkpoint_market_retirement_v1(&snapshot).expect("the honest plan");

    // A canonical Position at another owner, planted so the hostile frame names
    // a REAL Claims-owned Position rather than a vacant address -- otherwise the
    // frame's privilege sweep would refuse before the escrow rule was reached.
    let decoy_owner = Pubkey::new_unique();
    let decoy = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(fixture.claims_aggregate.to_bytes(), decoy_owner.to_bytes())
            .expect("decoy seeds")
            .as_slices(),
        &JOINED_CLAIMS_PROGRAM_ID,
    )
    .0;
    let honest_position = required_observed(&mut context, escrow.position).await;
    let mut decoy_bytes = honest_position.data.clone();
    encode_liability_basis_position_into_v2(
        LiabilityBasisPositionInputV2 {
            revision: 1,
            market_account: fixture.claims_aggregate.to_bytes(),
            owner: decoy_owner.to_bytes(),
            basis_id: LiabilityBasisMarketViewV2::decode(
                &required_observed(&mut context, fixture.claims_aggregate)
                    .await
                    .data,
            )
            .expect("aggregate")
            .basis_id,
        },
        &{
            let mut balances = vec![0_u64; RETIREMENT_CLAIM_COUNT as usize];
            balances[escrow.failure_selector as usize] = SEATED_RESIDUE_V1;
            balances
        },
        &mut decoy_bytes,
    )
    .expect("a canonical Position at another owner");
    set_account(
        &mut context,
        decoy,
        protocol_account(JOINED_CLAIMS_PROGRAM_ID, decoy_bytes),
    );

    // THE BUILDER REFUSES THE DECOY BEFORE THE CHAIN DOES. The decoy is a
    // canonical protocol-Position pair under its OWN recorded owner holding
    // exactly the residue at exactly the failure coordinate, so every rule
    // stated against the Position's own bytes admits it; what refuses it is
    // re-deriving the owner off the aggregate the way Claims does. Until
    // 2026-09-06 this snapshot compiled a plan and the operator learned the
    // difference by submitting it.
    let mut decoy_snapshot = snapshot.clone();
    decoy_snapshot.failure_escrow_position = observed_or_none(&mut context, decoy).await;
    assert_eq!(
        build_checkpoint_market_retirement_v1(&decoy_snapshot),
        Err(MarketRetirementOperatorErrorV1::Claims),
        "a canonical Position at another owner is not this Market's derived escrow"
    );

    let mut wrong_escrow = plan.prepare.clone();
    wrong_escrow.accounts[35].pubkey = decoy;
    let (table, addresses) =
        frozen_route_lookup_table(&mut context, &[plan.prepare.clone(), wrong_escrow.clone()])
            .await;

    let before = joined_snapshot(&mut context, &fixture).await;
    assert_eq!(
        refused_burn_hostile(
            &mut context,
            &wrong_escrow,
            "burn hostile: not the derived escrow",
            table,
            &addresses
        )
        .await,
        CLOSURE_BURN_WRONG_ESCROW_REFUSAL_V1,
        "a Position that is not this Market's escrow refuses 0x5010"
    );

    // A valid categorical record this Market was not founded on.
    let mut foreign_basis = vec![
        0;
        basis_record_bytes_v3(
            BasisKindV3::CategoricalQ1,
            RETIREMENT_CLAIM_COUNT as usize,
            0,
            0
        )
        .expect("record width")
    ];
    compile_basis_v3(
        BasisInputV3 {
            kind: BasisKindV3::CategoricalQ1,
            product_id: [0xe1; 32],
            result_domain_id: [0xe2; 32],
            coordinate_domain_id: [0xe3; 32],
            result_unit_id: [0xe4; 32],
            evaluator_release_id: [0xe5; 32],
            basis_width: RETIREMENT_CLAIM_COUNT,
            payout_scale: 1,
            knot_denominator: 1,
            knots: &[],
            terms: &[],
            failure_payouts: &[],
            price_gate_certificate_digest: [0; 32],
        },
        &mut foreign_basis,
    )
    .expect("a NON-refunding categorical record");
    let record_address = linked_basis_record(fixture.base.market);
    let honest_record = required_observed(&mut context, record_address).await;
    set_account(
        &mut context,
        record_address,
        protocol_account(REGISTRY_PROGRAM_ID, foreign_basis),
    );
    assert_eq!(
        refused_burn_hostile(
            &mut context,
            &plan.prepare,
            "burn hostile: substituted basis record",
            table,
            &addresses,
        )
        .await,
        CLOSURE_BURN_SUBSTITUTED_BASIS_REFUSAL_V1,
        "a record this Market was not founded on licenses no burn"
    );
    set_account(
        &mut context,
        record_address,
        Account {
            lamports: honest_record.lamports,
            data: honest_record.data.clone(),
            owner: honest_record.owner,
            executable: honest_record.executable,
            rent_epoch: 0,
        },
    );

    // A stranger holding one unit of the column.
    let honest_aggregate = required_observed(&mut context, fixture.claims_aggregate).await;
    let view = LiabilityBasisMarketViewV2::decode(&honest_aggregate.data).expect("aggregate");
    let mut stranded = vec![0_u64; RETIREMENT_CLAIM_COUNT as usize];
    stranded[escrow.failure_selector as usize] = SEATED_RESIDUE_V1 + 1;
    let mut stranded_bytes = honest_aggregate.data.clone();
    encode_liability_basis_market_into_v2(
        LiabilityBasisMarketInputV2 {
            revision: view.revision,
            logical_market: view.logical_market,
            release_set: view.release_set,
            registry_program: view.registry_program,
            product_instance_id: view.product_instance_id,
            basis_id: view.basis_id,
            realm_id: view.realm_id,
            custody_context: view.custody_context,
            generation: view.generation,
        },
        &stranded,
        &mut stranded_bytes,
    )
    .expect("an aggregate a stranger holds part of");
    set_account(
        &mut context,
        fixture.claims_aggregate,
        Account {
            lamports: honest_aggregate.lamports,
            data: stranded_bytes,
            owner: honest_aggregate.owner,
            executable: honest_aggregate.executable,
            rent_epoch: 0,
        },
    );
    assert_eq!(
        refused_burn_hostile(
            &mut context,
            &plan.prepare,
            "burn hostile: a stranger holds part of the column",
            table,
            &addresses,
        )
        .await,
        CHECKPOINT_PREPARE_SEATED_RESIDUE_REFUSAL_V1,
        "a column only partly in the escrow is an outstanding liability"
    );
    set_account(
        &mut context,
        fixture.claims_aggregate,
        Account {
            lamports: honest_aggregate.lamports,
            data: honest_aggregate.data.clone(),
            owner: honest_aggregate.owner,
            executable: honest_aggregate.executable,
            rent_epoch: 0,
        },
    );

    // An escrow holding a tradeable claim beside the residue.
    let mut mixed = vec![0_u64; RETIREMENT_CLAIM_COUNT as usize];
    mixed[0] = 7;
    mixed[escrow.failure_selector as usize] = SEATED_RESIDUE_V1;
    let mut mixed_bytes = honest_position.data.clone();
    encode_liability_basis_position_into_v2(
        LiabilityBasisPositionInputV2 {
            revision: 1,
            market_account: fixture.claims_aggregate.to_bytes(),
            owner: escrow.owner.to_bytes(),
            basis_id: view.basis_id,
        },
        &mixed,
        &mut mixed_bytes,
    )
    .expect("an escrow holding an ordinary claim too");
    set_account(
        &mut context,
        escrow.position,
        Account {
            lamports: honest_position.lamports,
            data: mixed_bytes,
            owner: honest_position.owner,
            executable: honest_position.executable,
            rent_epoch: 0,
        },
    );
    assert_eq!(
        refused_burn_hostile(
            &mut context,
            &plan.prepare,
            "burn hostile: the escrow holds a tradeable claim",
            table,
            &addresses,
        )
        .await,
        CHECKPOINT_PREPARE_SEATED_RESIDUE_REFUSAL_V1,
        "an escrow holding a payable claim is not a pure residue"
    );
    set_account(
        &mut context,
        escrow.position,
        Account {
            lamports: honest_position.lamports,
            data: honest_position.data.clone(),
            owner: honest_position.owner,
            executable: honest_position.executable,
            rent_epoch: 0,
        },
    );

    assert_eq!(
        joined_snapshot(&mut context, &fixture).await,
        before,
        "every hostile is byte and lamport atomic"
    );

    // THE POSITIVE CONTROL, from the same fixture and the same instruction: the
    // four refusals above are the hostiles, not the setup.
    submit_recorded_over_table_v0(
        &mut context,
        std::slice::from_ref(&plan.prepare),
        &[],
        "burn hostiles: the honest packet still burns",
        table,
        &addresses,
        REFUNDING_PREPARE_EXTENT,
    )
    .await
    .expect("the honest prepare packet burns the column");
    assert!(
        observed(&mut context, escrow.position).await.is_none(),
        "and it closes the escrow it emptied"
    );
}

/// Submit one hostile prepare packet and return the discriminant it refused
/// with, insisting it refused at all.
async fn refused_burn_hostile(
    context: &mut ProgramTestContext,
    instruction: &Instruction,
    label: &'static str,
    table: Pubkey,
    addresses: &[Pubkey],
) -> u32 {
    // Three of these hostiles submit BYTE-IDENTICAL instructions against
    // different account states, so without a fresh blockhash the second one
    // returns `AlreadyProcessed` and the assertion below would be reading the
    // runtime's dedup rather than the program's refusal.
    let clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("ProgramTest Clock");
    context
        .warp_to_slot(clock.slot.checked_add(1).expect("bounded fixture slot"))
        .expect("advance the hostile's blockhash");
    let refusal = submit_recorded_over_table_v0(
        context,
        std::slice::from_ref(instruction),
        &[],
        label,
        table,
        addresses,
        REFUNDING_PREPARE_EXTENT,
    )
    .await
    .expect_err(label);
    custom_program_error_code(refusal)
}

/// The custom discriminant a refused transaction carries, or a panic naming
/// what it carried instead.
fn custom_program_error_code(error: BanksClientError) -> u32 {
    match error {
        BanksClientError::TransactionError(TransactionError::InstructionError(
            _,
            InstructionError::Custom(code),
        ))
        | BanksClientError::SimulationError {
            err: TransactionError::InstructionError(_, InstructionError::Custom(code)),
            ..
        } => code,
        other => panic!("expected a custom program error, got {other:?}"),
    }
}

#[tokio::test]
async fn checkpointed_retirement_is_packet_bounded_resumable_and_conserving() {
    let (fixture, mut context) = joined_fixture().await;
    seed_exact_retirement_prestate(&mut context, &fixture).await;
    let snapshot = retirement_operator_snapshot(&mut context, &fixture).await;
    let plan = build_checkpoint_market_retirement_v1(&snapshot)
        .expect("checkpointed aggregate-retirement plan");
    assert_eq!(
        plan.prepare.data.len(),
        CHECKPOINT_RETIREMENT_PREPARE_CORE_BYTES_V1
    );
    assert_eq!(
        plan.close_vault.data.len(),
        CHECKPOINT_RETIREMENT_CUSTODY_SUFFIX_BYTES_V1
    );
    assert_eq!(
        plan.close_replay.data.len(),
        CHECKPOINT_RETIREMENT_CUSTODY_SUFFIX_BYTES_V1
    );
    assert_eq!(
        plan.finish.data.len(),
        CHECKPOINT_RETIREMENT_FINISH_BYTES_V1
    );
    assert_eq!(plan.prepare.accounts.len(), 35);
    assert_eq!(plan.close_vault.accounts.len(), 35);
    assert_eq!(plan.close_replay.accounts.len(), 35);
    assert_eq!(plan.finish.accounts.len(), 35);

    let census = [
        ("prepare", &plan.prepare),
        ("close-vault", &plan.close_vault),
        ("close-replay", &plan.close_replay),
        ("finish", &plan.finish),
    ]
    .map(|(name, instruction)| {
        let keys = unique_account_locks(instruction, context.payer.pubkey());
        println!(
            "checkpoint-retirement {name}: metas={} unique_locks={keys} data_bytes={}",
            instruction.accounts.len(),
            instruction.data.len(),
        );
        keys
    });

    // The chain's own frozen table, built before the chain runs.
    //
    // All four routes are 35-meta frames and all four were over the packet
    // maximum as legacy messages -- 2,005 to 2,157, up to 925 over -- so not one
    // of the twelve transactions below could have been submitted anywhere.
    // ProgramTest submits no packet, which is why they all passed.
    //
    // ONE table for the chain, not one per route. The four frames share their
    // coordinates, and four tables would be four rents for one market's
    // retirement. Building it up front is also what a real controller must do:
    // the addresses have to be finalized before the first submission can
    // resolve them.
    let (checkpoint_table, checkpoint_addresses) = frozen_route_lookup_table(
        &mut context,
        &[
            plan.prepare.clone(),
            plan.close_vault.clone(),
            plan.close_replay.clone(),
            plan.finish.clone(),
        ],
    )
    .await;

    let before = joined_snapshot(&mut context, &fixture).await;
    let claims_prestate = before
        .claims_aggregate
        .clone()
        .expect("Claims aggregate prestate");
    let mut substituted_claims_owner = claims_prestate.clone();
    substituted_claims_owner.owner = system_program::ID;
    set_account(
        &mut context,
        fixture.claims_aggregate,
        substituted_claims_owner,
    );
    let substituted_owner_snapshot = joined_snapshot(&mut context, &fixture).await;
    assert!(
        submit_recorded_over_table_v0(&mut context, std::slice::from_ref(&plan.prepare), &[], "retirement checkpoint: prepare against a Claims aggregate reassigned to the System program", checkpoint_table, &checkpoint_addresses, PREPARE_EXTENT)
            .await
            .is_err(),
        "Claims handoff refuses a substituted aggregate owner"
    );
    assert_eq!(
        joined_snapshot(&mut context, &fixture).await,
        substituted_owner_snapshot,
        "owner refusal cannot partially reassign or refund the aggregate"
    );
    set_account(&mut context, fixture.claims_aggregate, claims_prestate);
    assert!(
        submit_recorded_over_table_v0(
            &mut context,
            std::slice::from_ref(&plan.close_vault),
            &[],
            "retirement checkpoint: close-vault suffix before the Claims handoff",
            checkpoint_table,
            &checkpoint_addresses,
            CLOSE_VAULT_EXTENT
        )
        .await
        .is_err(),
        "a suffix cannot mint authority before Claims handoff"
    );
    assert_eq!(joined_snapshot(&mut context, &fixture).await, before);

    submit_recorded_over_table_v0(
        &mut context,
        std::slice::from_ref(&plan.prepare),
        &[],
        "retirement checkpoint: prepare hands the Claims aggregate to Core",
        checkpoint_table,
        &checkpoint_addresses,
        PREPARE_EXTENT,
    )
    .await
    .expect("Claims handoff and ClaimsClosed checkpoint");
    let prepared = required_observed(&mut context, fixture.claims_aggregate).await;
    let prepared_account = observed(&mut context, fixture.claims_aggregate)
        .await
        .expect("raw ClaimsClosed checkpoint");
    assert_eq!(prepared.owner, CORE_PROGRAM_ID);
    assert_eq!(prepared.data.len(), 256);
    assert_eq!(
        prepared.lamports,
        before
            .claims_aggregate
            .as_ref()
            .expect("Claims aggregate prestate")
            .lamports,
        "Claims refund stays in the exact handed-off account"
    );
    assert_eq!(
        required_observed(&mut context, fixture.base.rent_credit)
            .await
            .lamports,
        before
            .rent_credit
            .as_ref()
            .expect("RentCredit prestate")
            .lamports,
        "Claims handoff never relabels its retained refund as RentCredit"
    );
    let prepared_snapshot = joined_snapshot(&mut context, &fixture).await;
    assert!(
        submit_recorded_over_table_v0(
            &mut context,
            std::slice::from_ref(&plan.prepare),
            &[],
            "retirement checkpoint: prepare replayed against a ClaimsClosed checkpoint",
            checkpoint_table,
            &checkpoint_addresses,
            PREPARE_EXTENT
        )
        .await
        .is_err(),
        "ClaimsClosed cannot replay prepare"
    );
    assert_eq!(
        joined_snapshot(&mut context, &fixture).await,
        prepared_snapshot,
        "prepare replay refusal is byte/lamport atomic"
    );
    let mut substituted_checkpoint_owner = prepared_account.clone();
    substituted_checkpoint_owner.owner = JOINED_CLAIMS_PROGRAM_ID;
    set_account(
        &mut context,
        fixture.claims_aggregate,
        substituted_checkpoint_owner,
    );
    let substituted_checkpoint_snapshot = joined_snapshot(&mut context, &fixture).await;
    assert!(
        submit_recorded_over_table_v0(
            &mut context,
            std::slice::from_ref(&plan.close_vault),
            &[],
            "retirement checkpoint: close-vault against a checkpoint reassigned away from Core",
            checkpoint_table,
            &checkpoint_addresses,
            CLOSE_VAULT_EXTENT
        )
        .await
        .is_err(),
        "a suffix refuses a checkpoint reassigned away from Core"
    );
    assert_eq!(
        joined_snapshot(&mut context, &fixture).await,
        substituted_checkpoint_snapshot,
        "checkpoint-owner refusal cannot close or refund Custody state"
    );
    set_account(&mut context, fixture.claims_aggregate, prepared_account);
    assert!(
        submit_recorded_over_table_v0(
            &mut context,
            std::slice::from_ref(&plan.close_replay),
            &[],
            "retirement checkpoint: close-replay before the HoardPrincipal vault closes",
            checkpoint_table,
            &checkpoint_addresses,
            CLOSE_REPLAY_EXTENT
        )
        .await
        .is_err(),
        "Custody replay cannot close before the HoardPrincipal vault"
    );
    assert_eq!(
        joined_snapshot(&mut context, &fixture).await,
        prepared_snapshot,
        "phase refusal is byte/lamport atomic"
    );

    submit_recorded_over_table_v0(
        &mut context,
        std::slice::from_ref(&plan.close_vault),
        &[],
        "retirement checkpoint: close-vault closes the HoardPrincipal vault",
        checkpoint_table,
        &checkpoint_addresses,
        CLOSE_VAULT_EXTENT,
    )
    .await
    .expect("HoardPrincipal close and HoardVaultClosed checkpoint");
    assert!(observed(&mut context, fixture.base.vault).await.is_none());
    assert!(observed(&mut context, fixture.base.market).await.is_some());
    assert!(
        observed(&mut context, fixture.base.rent_credit)
            .await
            .is_some()
    );
    let vault_closed_snapshot = joined_snapshot(&mut context, &fixture).await;
    assert!(
        submit_recorded_over_table_v0(
            &mut context,
            std::slice::from_ref(&plan.close_vault),
            &[],
            "retirement checkpoint: close-vault replayed against a HoardVaultClosed checkpoint",
            checkpoint_table,
            &checkpoint_addresses,
            CLOSE_VAULT_EXTENT
        )
        .await
        .is_err(),
        "HoardVaultClosed cannot replay close-vault"
    );
    assert_eq!(
        joined_snapshot(&mut context, &fixture).await,
        vault_closed_snapshot,
        "close-vault replay refusal is byte/lamport atomic"
    );
    submit_recorded_over_table_v0(
        &mut context,
        std::slice::from_ref(&plan.close_replay),
        &[],
        "retirement checkpoint: close-replay closes the Custody replay",
        checkpoint_table,
        &checkpoint_addresses,
        CLOSE_REPLAY_EXTENT,
    )
    .await
    .expect("Custody replay close and CustodyReplayClosed checkpoint");
    assert!(observed(&mut context, fixture.base.replay).await.is_none());
    assert!(observed(&mut context, fixture.base.market).await.is_some());
    assert!(
        observed(&mut context, fixture.base.rent_credit)
            .await
            .is_some()
    );
    let replay_closed_snapshot = joined_snapshot(&mut context, &fixture).await;
    assert!(
        submit_recorded_over_table_v0(
            &mut context,
            std::slice::from_ref(&plan.close_replay),
            &[],
            "retirement checkpoint: close-replay replayed against a CustodyReplayClosed checkpoint",
            checkpoint_table,
            &checkpoint_addresses,
            CLOSE_REPLAY_EXTENT
        )
        .await
        .is_err(),
        "CustodyReplayClosed cannot replay close-replay"
    );
    assert_eq!(
        joined_snapshot(&mut context, &fixture).await,
        replay_closed_snapshot,
        "close-replay replay refusal is byte/lamport atomic"
    );

    let mut substituted_refund = plan.finish.clone();
    substituted_refund
        .accounts
        .iter_mut()
        .find(|meta| meta.pubkey == fixture.refund_wallet)
        .expect("finish refund-wallet meta")
        .pubkey = context.payer.pubkey();
    assert!(
        submit_recorded_over_table_v0(
            &mut context,
            &[substituted_refund],
            &[],
            "retirement checkpoint: finish with the immutable refund wallet substituted",
            checkpoint_table,
            &checkpoint_addresses,
            FINISH_SUBSTITUTED_EXTENT
        )
        .await
        .is_err(),
        "finish refuses substitution of the immutable refund wallet"
    );
    assert_eq!(
        joined_snapshot(&mut context, &fixture).await,
        replay_closed_snapshot,
        "refund substitution cannot close checkpoint, Market, or RentCredit"
    );

    let wallet_before = before
        .refund_wallet
        .as_ref()
        .expect("refund wallet prestate")
        .lamports;
    submit_recorded_over_table_v0(
        &mut context,
        std::slice::from_ref(&plan.finish),
        &[],
        "retirement checkpoint: finish closes checkpoint, Market and RentCredit",
        checkpoint_table,
        &checkpoint_addresses,
        FINISH_EXTENT,
    )
    .await
    .expect("checkpoint then Core then Rent close");
    let after = joined_snapshot(&mut context, &fixture).await;
    assert!(after.claims_aggregate.is_none());
    assert!(after.market.is_none());
    assert!(after.rent_credit.is_none());
    assert!(after.custody_replay.is_none());
    assert!(after.hoard_vault.is_none());
    assert_eq!(after.source_receipt, before.source_receipt);
    assert_eq!(
        after
            .refund_wallet
            .expect("immutable refund wallet")
            .lamports,
        wallet_before + plan.expected_refund_delta,
        "every classified rent lamport reaches the immutable refund wallet exactly once"
    );
    assert!(census.iter().all(|keys| *keys <= 64));
}
