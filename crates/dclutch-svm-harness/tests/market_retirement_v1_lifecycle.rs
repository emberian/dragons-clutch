#![allow(dead_code)]

include!("resolution_core_v3_lifecycle.rs");

use dclutch_claims_svm::liability_basis_state_v2::{
    LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_MARKET_SEED_V2,
    LiabilityBasisMarketInputV2, encode_liability_basis_market_into_v2,
    liability_basis_vector_width_v2,
};
use dclutch_market_retirement_v1_operator::{
    MarketRetirementOperatorErrorV1, MarketRetirementSnapshotV1, build_market_retirement_v1,
};
use dclutch_registry_svm::continuation_v1::{
    REGISTRY_CONTINUATION_REQUEST_BYTES_V1, RegistryContinuationAdmissionSeedsV1,
    RegistryContinuationRequestV1,
};
use dclutch_rent_contract::lifecycle_v2::{
    LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleAccountIdV2, LifecycleRentCreditV2,
};
use spl_token_interface::state::Account as SplAccount;

const CLAIMS_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x76; 32]);
const TRADING_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x77; 32]);
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

fn joined_activation(
    core: ArtifactReleaseV1,
    claims: ArtifactReleaseV1,
    trading: ArtifactReleaseV1,
    resolution: ArtifactReleaseV1,
    custody: ArtifactReleaseV1,
) -> ([u8; 32], Vec<u8>) {
    let release_set = ExecutionReleaseSetV1::new(
        binding(core),
        binding(claims),
        binding(trading),
        binding(resolution),
        binding(custody),
    )
    .expect("joined execution release set");
    let release_set_id = hash(&release_set.to_bytes()).to_bytes();
    let content = id(release_set_id);
    let mut bytes = vec![0; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut bytes, content).expect("joined activation cache");
    for (role, selected) in [
        (ExecutionRoleV1::Core, core),
        (ExecutionRoleV1::Claims, claims),
        (ExecutionRoleV1::Trading, trading),
        (ExecutionRoleV1::Resolution, resolution),
        (ExecutionRoleV1::Custody, custody),
    ] {
        activate_execution_role_into_v1(
            &mut bytes,
            content,
            &release_set,
            role,
            &activation_input(selected),
        )
        .expect("activate joined role");
    }
    ActivatedExecutionReleaseSetV1::decode(&bytes).expect("complete joined activation cache");
    (release_set_id, bytes)
}

fn set_account(context: &mut ProgramTestContext, key: Pubkey, account: Account) {
    context.set_account(&key, &AccountSharedData::from(account));
}

async fn joined_fixture() -> (JoinedFixture, ProgramTestContext) {
    let mut base = fixture(true);
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    let claims_elf = fs::read(directory.join("dclutch_claims_sbf.so")).expect("Claims ELF");
    let trading_elf = fs::read(directory.join("dclutch_trading_sbf.so")).expect("Trading ELF");
    let rent_elf = fs::read(directory.join("dclutch_rent_sbf.so")).expect("Rent ELF");
    let elves = artifacts();
    let test = base.test.as_mut().expect("unstarted ProgramTest");
    add_program(test, "dclutch_claims_sbf", CLAIMS_PROGRAM_ID, &claims_elf);
    add_program(
        test,
        "dclutch_trading_sbf",
        TRADING_PROGRAM_ID,
        &trading_elf,
    );
    add_program(test, "dclutch_rent_sbf", RENT_PROGRAM_ID, &rent_elf);

    let core_release = release(CORE_PROGRAM_ID, [0x41; 32], &elves.core);
    let claims_release = release(CLAIMS_PROGRAM_ID, [0x43; 32], &claims_elf);
    let trading_release = release(TRADING_PROGRAM_ID, [0x46; 32], &trading_elf);
    let resolution_release = release(
        RESOLUTION_PROGRAM_ID,
        RESOLUTION_CONTROLLER_RELEASE_ID_V4,
        &elves.resolution,
    );
    let custody_release = release(CUSTODY_PROGRAM_ID, [0x42; 32], &elves.custody);
    let rent_release = release(RENT_PROGRAM_ID, [0x44; 32], &rent_elf);
    let registry_release = release(REGISTRY_PROGRAM_ID, [0x45; 32], &elves.registry);
    let (release_set, activation_data) = joined_activation(
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
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
        &CORE_PROGRAM_ID,
    )
    .0;
    let infrastructure =
        ProtocolInfrastructureProfileV1::new(binding(registry_release), binding(rent_release))
            .expect("immutable infrastructure profile");
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
        rent_beneficiary: CoreIdentity::new(rent_credit.to_bytes()).expect("RentCredit"),
        terminal_receipt: None,
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
            SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V2,
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
    let funding = [0_u16, 1, 2].map(|entry| funding_key(market, manifest_id, manifest, entry));

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
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    const RETIREMENT_CLAIM_COUNT: u32 = 5;
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
            product_instance_id: identity.product_record.to_bytes(),
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
        protocol_account(CLAIMS_PROGRAM_ID, aggregate_bytes),
    );

    base.release_set = release_set;
    base.activation = activation;
    base.infrastructure = infrastructure_profile;
    base.registry_artifact = registry_artifact;
    base.market = market;
    base.source = source;
    base.funding = funding;
    base.certificate = certificate;
    base.closure = closure;
    base.rent_credit = rent_credit;
    base.replay = replay;
    base.vault = vault;
    base.custody_authority = custody_authority;
    (
        JoinedFixture {
            base,
            claims_programdata: programdata(CLAIMS_PROGRAM_ID),
            trading_programdata: programdata(TRADING_PROGRAM_ID),
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

struct RetirementPlan {
    instruction: Instruction,
    direct_instruction: Instruction,
    activation_cache_digest: CoreContentId,
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
        AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false),
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
    MarketRetirementSnapshotV1 {
        market: required_observed(context, fixture.base.market).await,
        rent_credit: required_observed(context, fixture.base.rent_credit).await,
        activation_cache: required_observed(context, fixture.base.activation).await,
        registry_program: required_observed(context, REGISTRY_PROGRAM_ID).await,
        core_program: required_observed(context, CORE_PROGRAM_ID).await,
        core_programdata: required_observed(context, fixture.base.core_programdata).await,
        claims_program: required_observed(context, CLAIMS_PROGRAM_ID).await,
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
    }
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
    let mut create_instructions = Vec::with_capacity(5);
    create_instructions.push(transfer(
        &payer,
        &fixture.base.source,
        create.source_top_up_lamports,
    ));
    for (funding, top_up) in fixture
        .base
        .funding
        .into_iter()
        .zip(create.funding_top_up_lamports)
    {
        create_instructions.push(transfer(&payer, &funding, top_up));
    }
    create_instructions.push(create.instruction.clone());
    let mut substituted_system = create_instructions.clone();
    substituted_system
        .last_mut()
        .expect("CreateFund instruction")
        .accounts[17]
        .pubkey = sysvar::rent::ID;
    assert!(
        submit(context, &substituted_system).await.is_err(),
        "a substituted System program must refuse after all four top-ups"
    );
    assert_eq!(
        open_rollback_snapshot(context, &fixture.base).await,
        before_create,
        "late CreateFund refusal rolls the Market, Source, three Funds, Custody, and RentCredit back"
    );
    submit(context, &create_instructions)
        .await
        .expect("create exact same-Market Source and three pending Funds");
    for funding in fixture.base.funding {
        assert_eq!(
            FundingStateV1::decode(
                &observed(context, funding)
                    .await
                    .expect("created same-Market Funding")
                    .data,
            )
            .expect("Funding state")
            .status(),
            FundingStatus::Pending,
        );
    }

    let verify =
        build_resolution_verify_fund_ready_v3(&verify_snapshot(context, &fixture.base).await)
            .expect("chain-derived same-Market VerifyFundReady");
    validate_resolution_verify_fund_ready_report_v3(&verify)
        .expect("exact same-Market VerifyFundReady");
    let before_verify = open_rollback_snapshot(context, &fixture.base).await;
    let mut read_only_beneficiary = verify.instruction.clone();
    read_only_beneficiary.accounts[16].is_writable = false;
    assert!(
        submit(context, &[read_only_beneficiary]).await.is_err(),
        "a read-only immutable beneficiary must refuse"
    );
    assert_eq!(
        open_rollback_snapshot(context, &fixture.base).await,
        before_verify,
        "VerifyFundReady privilege refusal rolls every funding ledger back"
    );
    submit(context, &[verify.instruction])
        .await
        .expect("activate exact same-Market three-ledger funding");

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
        trading_program: TRADING_PROGRAM_ID,
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
    .expect("chain-derived Core provider execution");
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
        .expect("Core consumes the authenticated provider result");

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
    let closure_rent = Rent::default().minimum_balance(SOURCE_CLOSURE_RECEIPT_BYTES_V2);
    let payer = context.payer.pubkey();
    submit(
        &mut context,
        &[transfer(&payer, &fixture.base.closure, closure_rent)],
    )
    .await
    .expect("prepay Source closure receipt");
    let close = build_resolution_close_fund_v3(&close_snapshot(&mut context, &fixture.base).await)
        .expect("chain-derived CloseFund");
    validate_resolution_close_fund_report_v3(&close).expect("exact CloseFund report");
    submit(&mut context, &[close.instruction])
        .await
        .expect("Resolution closes Source subtree first");

    let plan = retirement_instruction(&mut context, &fixture).await;
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
    assert_eq!(fixture.base.funding.len(), 3, "Resolution has three funds");
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
