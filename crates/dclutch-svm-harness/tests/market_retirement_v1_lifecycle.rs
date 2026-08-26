#![allow(dead_code)]

include!("resolution_core_v3_lifecycle.rs");

use dclutch_claims_svm::{
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_MARKET_SEED_V2,
        LiabilityBasisMarketInputV2, encode_liability_basis_market_into_v2,
        liability_basis_vector_width_v2,
    },
    market_closure_v1::{
        CLAIMS_MARKET_CLOSURE_POST_RESOURCE_DIGEST_DOMAIN_V1,
        CLAIMS_MARKET_CLOSURE_PRE_RESOURCE_DIGEST_DOMAIN_V1, ClaimsMarketClosureReceiptInputV1,
        ClaimsMarketClosureReceiptV1, ClaimsMarketClosureRequestInputV1,
        ClaimsMarketClosureRequestV1,
    },
};
use dclutch_custody_contract::{
    CUSTODY_POSTSTATE_DOMAIN_V1, CUSTODY_REPLAY_BYTES_V1, CustodyReceiptV1, ReceiptEvidenceV1,
};
use dclutch_market_core_codec::{
    RETIREMENT_CUSTODY_RECEIPT_COUNT_V1, RETIREMENT_POST_RESOURCE_DIGEST_DOMAIN_V1,
    RETIREMENT_ROLE_COUNT_V1, RetirementBundleInputV1, RetirementBundleV1,
};
use dclutch_registry_svm::continuation_v1::{
    REGISTRY_CONTINUATION_REQUEST_BYTES_V1, RegistryContinuationAdmissionSeedsV1,
    RegistryContinuationRequestV1,
};
use dclutch_rent_contract::lifecycle_v2::{
    LIFECYCLE_RENT_CORE_CLOSE_AUTHORITY_DOMAIN_V2, LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
    LifecycleAccountIdV2, LifecycleRentCreditV2,
};
use dclutch_token_svm::ACCOUNT_BYTES as TOKEN_ACCOUNT_BYTES;
use solana_account::AccountSharedData;
use solana_program::hash::hashv;
use spl_token_interface::state::{Account as SplAccount, AccountState};

const CLAIMS_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x76; 32]);
const CUSTODY_CONTEXT: [u8; 32] = [0xc7; 32];
const CLAIMS_REVISION: u64 = 11;
const CUSTODY_REVISION: u64 = 17;

struct JoinedFixture {
    base: Fixture,
    claims_programdata: Pubkey,
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
    resolution: ArtifactReleaseV1,
    custody: ArtifactReleaseV1,
) -> ([u8; 32], Vec<u8>) {
    let release_set = ExecutionReleaseSetV1::new(
        binding(core),
        binding(claims),
        binding(core),
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
        (ExecutionRoleV1::Trading, core),
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

fn token_account_data(mint: Pubkey, owner: Pubkey) -> Vec<u8> {
    let mut bytes = vec![0; SplAccount::LEN];
    SplAccount::pack(
        SplAccount {
            mint,
            owner,
            amount: 0,
            delegate: COption::None,
            state: AccountState::Initialized,
            is_native: COption::None,
            delegated_amount: 0,
            close_authority: COption::None,
        },
        &mut bytes,
    )
    .expect("empty Hoard vault");
    bytes
}

fn set_account(context: &mut ProgramTestContext, key: Pubkey, account: Account) {
    context.set_account(&key, &AccountSharedData::from(account));
}

fn active_funding_account(
    market: Pubkey,
    manifest_id: CapabilityContentId,
    manifest: CapabilityManifestV1<'_>,
    entry_index: u16,
) -> (Pubkey, Account) {
    let rent = Rent::default().minimum_balance(FUNDING_STATE_BYTES);
    let custody = FundingCustodyObservationV1::native_only(
        rent.checked_mul(2)
            .and_then(|value| value.checked_add(BOUNTY))
            .expect("bounded funding custody"),
        rent,
    )
    .expect("funding custody");
    let mut state = FundingStateV1::new(manifest_id, manifest, entry_index, custody)
        .expect("pending funding state");
    state
        .activate(manifest_id, manifest, custody, 1)
        .expect("active funding state");
    let key = funding_key(market, manifest_id, manifest, entry_index);
    (
        key,
        Account {
            lamports: rent + BOUNTY,
            data: state.to_bytes().to_vec(),
            owner: RESOLUTION_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    )
}

async fn joined_fixture() -> (JoinedFixture, ProgramTestContext) {
    let mut base = fixture(true);
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    let claims_elf = fs::read(directory.join("dclutch_claims_sbf.so")).expect("Claims ELF");
    let rent_elf = fs::read(directory.join("dclutch_rent_sbf.so")).expect("Rent ELF");
    let elves = artifacts();
    let test = base.test.as_mut().expect("unstarted ProgramTest");
    add_program(test, "dclutch_claims_sbf", CLAIMS_PROGRAM_ID, &claims_elf);
    add_program(test, "dclutch_rent_sbf", RENT_PROGRAM_ID, &rent_elf);

    let core_release = release(CORE_PROGRAM_ID, [0x41; 32], &elves.core);
    let claims_release = release(CLAIMS_PROGRAM_ID, [0x43; 32], &claims_elf);
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
        phase: Phase::Open,
        readiness: Readiness::Consumed,
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

    let material_account = observed(&mut context, base.source_material.raw)
        .await
        .expect("Source material");
    let material = SourceMaterialV2::decode(&material_account.data).expect("Source material V2");
    let material_id = hash(&material_account.data).to_bytes();
    let product_account = observed(&mut context, base.product.raw)
        .await
        .expect("Product record");
    let product_record_id = hash(&product_account.data).to_bytes();
    let domain_account = observed(&mut context, base.domain.raw)
        .await
        .expect("ResultDomain");
    let domain = ResultDomainV2::decode(&domain_account.data).expect("ResultDomain V2");
    let (source, source_bump) = Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
            market.as_ref(),
            &GENERATION.to_le_bytes(),
        ],
        &RESOLUTION_PROGRAM_ID,
    );
    let mut source_value = SourceResolutionStateV2::fresh(
        market.to_bytes(),
        GENERATION,
        source_id(material_id),
        rent_credit.to_bytes(),
        source_bump,
        0,
        0,
    )
    .expect("fresh Source")
    .state();
    let decision = source_value
        .resolve_primary_from_authenticated_domain(
            source_id(material_id),
            material,
            source_id(product_record_id),
            domain,
            source_id([0xb3; 32]),
            -1,
            1,
            GENERATION,
            TERMINAL_TIME,
            TERMINAL_SEQUENCE,
        )
        .expect("resolved Source");
    set_account(
        &mut context,
        source,
        protocol_account(RESOLUTION_PROGRAM_ID, source_value.to_bytes().to_vec()),
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
    let certificate_value = ResolutionCertificateV2 {
        kind: ResolutionCertificateKindV2::ResolutionSuccess,
        market: market.to_bytes(),
        route: [0xb4; 32],
        source_material: material_id,
        product_record_digest: product_record_id,
        provider_evidence: [0xb3; 32],
        funding_allocation: [0; 32],
        receipt_account: certificate.to_bytes(),
        generation: GENERATION,
        attempt_index: 0,
        schedule_index: 0,
        selector: decision.selector(),
        work_paid: 0,
        funding_remaining: 0,
        result_numerator: -1,
        result_denominator: 1,
        observed_at: u64::try_from(TERMINAL_TIME).expect("positive time"),
    };
    set_account(
        &mut context,
        certificate,
        protocol_account(
            RESOLUTION_PROGRAM_ID,
            certificate_value.to_bytes().expect("certificate").to_vec(),
        ),
    );
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
    let mut funding = [Pubkey::default(); 3];
    for (slot, entry) in funding.iter_mut().zip([0_u16, 1, 2]) {
        let (key, account) = active_funding_account(market, manifest_id, manifest, entry);
        *slot = key;
        set_account(&mut context, key, account);
    }

    let replay_request = joined_custody_request(
        release_set,
        market,
        base.realm,
        base.mint,
        rent_credit,
        OperationV1::CloseVault,
        [0; 32],
        CUSTODY_REVISION,
        0,
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
            CUSTODY_CONTEXT,
            CompartmentV1::HoardPrincipal,
        )
        .as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let replay_value = CustodyReplayV1 {
        caller_role: CallerRoleV1::Core,
        release_set,
        market: market.to_bytes(),
        realm: base.realm,
        context: CUSTODY_CONTEXT,
        caller_program: CORE_PROGRAM_ID.to_bytes(),
        rent_refund: rent_credit.to_bytes(),
        open_vault_count: 1,
        next_revision: CUSTODY_REVISION,
        generation: GENERATION,
        last_request_digest: [0xe1; 32],
        last_poststate_commitment: [0xe2; 32],
    };
    set_account(
        &mut context,
        replay,
        protocol_account(
            CUSTODY_PROGRAM_ID,
            replay_value.to_bytes().expect("replay").to_vec(),
        ),
    );
    set_account(
        &mut context,
        vault,
        protocol_account(
            Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID),
            token_account_data(base.mint, custody_authority),
        ),
    );

    let claims_aggregate = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, market.as_ref()],
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    let mut aggregate_bytes =
        vec![
            0;
            liability_basis_vector_width_v2(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, 3)
                .expect("aggregate width")
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
            custody_context: CUSTODY_CONTEXT,
            generation: GENERATION,
        },
        &[0, 0, 0],
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

#[allow(clippy::too_many_arguments)]
fn joined_custody_request(
    release_set: [u8; 32],
    market: Pubkey,
    realm: [u8; 32],
    mint: Pubkey,
    rent_credit: Pubkey,
    operation: OperationV1,
    parent_request_digest: [u8; 32],
    expected_revision: u64,
    transfer_index: u16,
) -> CustodyRequestV1 {
    let close_vault = operation == OperationV1::CloseVault;
    let vault = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            market.to_bytes(),
            release_set,
            CUSTODY_CONTEXT,
            CompartmentV1::HoardPrincipal,
        )
        .as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    CustodyRequestV1 {
        operation,
        caller_role: CallerRoleV1::Core,
        source_compartment: if close_vault {
            CompartmentV1::HoardPrincipal
        } else {
            CompartmentV1::None
        },
        destination_compartment: CompartmentV1::None,
        release_set,
        market: market.to_bytes(),
        realm,
        context: CUSTODY_CONTEXT,
        caller_program: CORE_PROGRAM_ID.to_bytes(),
        semantic: ContextV1 {
            candidate: [0xd3; 32],
            source_owner: [0; 32],
            destination_owner: [0; 32],
            order: [0xd4; 32],
            parent_request_digest,
            order_nonce: 9,
            generation: GENERATION,
            page_index: 2,
            execution_index: 3,
            transfer_index,
        },
        source: if close_vault {
            vault.to_bytes()
        } else {
            [0; 32]
        },
        destination: [0; 32],
        source_vault_context: if close_vault {
            CUSTODY_CONTEXT
        } else {
            [0; 32]
        },
        destination_vault_context: [0; 32],
        mint: if close_vault {
            mint.to_bytes()
        } else {
            [0; 32]
        },
        token_program: if close_vault {
            LEGACY_TOKEN_PROGRAM_ID
        } else {
            [0; 32]
        },
        payer: [0; 32],
        rent_refund: rent_credit.to_bytes(),
        expected_revision,
        resulting_revision: expected_revision + 1,
        amount: 0,
        rent_lamports: if close_vault {
            Rent::default().minimum_balance(TOKEN_ACCOUNT_BYTES)
        } else {
            Rent::default().minimum_balance(CUSTODY_REPLAY_BYTES_V1)
        },
    }
}

fn custody_poststate(
    request_digest: [u8; 32],
    source: Pubkey,
    destination: Pubkey,
    rent_lamports: u64,
) -> [u8; 32] {
    hashv(&[
        CUSTODY_POSTSTATE_DOMAIN_V1,
        &request_digest,
        source.as_ref(),
        destination.as_ref(),
        &0_u64.to_le_bytes(),
        &0_u64.to_le_bytes(),
        &0_u64.to_le_bytes(),
        &0_u64.to_le_bytes(),
        &rent_lamports.to_le_bytes(),
    ])
    .to_bytes()
}

fn caller_authority(
    release_set: [u8; 32],
    market: Pubkey,
    context: [u8; 32],
    request_bytes: &[u8],
) -> Pubkey {
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        release_set,
        market.to_bytes(),
        ExecutionRoleV1::Core,
        context,
        hash(request_bytes).to_bytes(),
    )
    .expect("Core caller authority");
    Pubkey::find_program_address(&seeds.as_slices(), &CORE_PROGRAM_ID).0
}

struct RetirementPlan {
    instruction: Instruction,
    direct_instruction: Instruction,
    activation_cache_digest: CoreContentId,
    expected_refund_delta: u64,
}

fn registry_retirement_continuation(
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

async fn retirement_instruction(
    context: &mut ProgramTestContext,
    fixture: &JoinedFixture,
) -> RetirementPlan {
    let core_request = Request::administrative(
        Action::Retire,
        GENERATION,
        CoreIdentity::new(fixture.base.market.to_bytes()).expect("Market"),
    );
    let core_bytes = core_request.encode().expect("Retire request");
    let parent_digest = hash(&core_bytes).to_bytes();
    let aggregate_account = observed(context, fixture.claims_aggregate)
        .await
        .expect("Claims aggregate");
    let aggregate =
        dclutch_claims_svm::liability_basis_state_v2::LiabilityBasisMarketViewV2::decode(
            &aggregate_account.data,
        )
        .expect("Claims aggregate view");
    let claims = ClaimsMarketClosureRequestV1::new(ClaimsMarketClosureRequestInputV1 {
        release_set: fixture.base.release_set,
        market: fixture.base.market.to_bytes(),
        aggregate: fixture.claims_aggregate.to_bytes(),
        rent_credit: fixture.base.rent_credit.to_bytes(),
        parent_request_digest: parent_digest,
        core_program: CORE_PROGRAM_ID.to_bytes(),
        generation: GENERATION,
        expected_revision: CLAIMS_REVISION,
        resulting_revision: CLAIMS_REVISION + 1,
        claim_count: aggregate.claim_count,
    })
    .expect("Claims close request");
    let claims_bytes = claims.to_bytes();
    let close_vault = joined_custody_request(
        fixture.base.release_set,
        fixture.base.market,
        fixture.base.realm,
        fixture.base.mint,
        fixture.base.rent_credit,
        OperationV1::CloseVault,
        parent_digest,
        CUSTODY_REVISION,
        0,
    );
    let close_vault_bytes = close_vault.to_bytes().expect("CloseVault request");
    let close_replay = joined_custody_request(
        fixture.base.release_set,
        fixture.base.market,
        fixture.base.realm,
        fixture.base.mint,
        fixture.base.rent_credit,
        OperationV1::CloseReplay,
        parent_digest,
        CUSTODY_REVISION + 1,
        1,
    );
    let close_replay_bytes = close_replay.to_bytes().expect("CloseReplay request");

    let source_account = observed(context, fixture.base.closure)
        .await
        .expect("Resolution closure receipt");
    let source = SourceClosureReceiptV2::decode(&source_account.data).expect("Source receipt");
    let market_account = observed(context, fixture.base.market)
        .await
        .expect("Retiring Market");
    let credit_account = observed(context, fixture.base.rent_credit)
        .await
        .expect("lifecycle RentCredit");
    let replay_account = observed(context, fixture.base.replay)
        .await
        .expect("Custody replay");
    let replay = CustodyReplayV1::decode(&replay_account.data).expect("Custody replay state");
    let vault_account = observed(context, fixture.base.vault)
        .await
        .expect("Hoard vault");

    let claims_request_digest = hash(&claims_bytes).to_bytes();
    let claims_pre_digest = hashv(&[
        CLAIMS_MARKET_CLOSURE_PRE_RESOURCE_DIGEST_DOMAIN_V1.as_slice(),
        fixture.claims_aggregate.as_ref(),
        aggregate_account.data.as_slice(),
    ])
    .to_bytes();
    let claims_credit_after = credit_account
        .lamports
        .checked_add(aggregate_account.lamports)
        .expect("Claims refund");
    let claims_post_digest = hashv(&[
        CLAIMS_MARKET_CLOSURE_POST_RESOURCE_DIGEST_DOMAIN_V1.as_slice(),
        fixture.claims_aggregate.as_ref(),
        fixture.base.rent_credit.as_ref(),
        &(CLAIMS_REVISION + 1).to_le_bytes(),
        &aggregate_account.lamports.to_le_bytes(),
        &claims_credit_after.to_le_bytes(),
    ])
    .to_bytes();
    let claims_receipt = ClaimsMarketClosureReceiptV1::new(ClaimsMarketClosureReceiptInputV1 {
        producer: CLAIMS_PROGRAM_ID.to_bytes(),
        release_set: fixture.base.release_set,
        market: fixture.base.market.to_bytes(),
        aggregate: fixture.claims_aggregate.to_bytes(),
        rent_credit: fixture.base.rent_credit.to_bytes(),
        request_digest: claims_request_digest,
        pre_resource_digest: claims_pre_digest,
        post_resource_digest: claims_post_digest,
        generation: GENERATION,
        pre_revision: CLAIMS_REVISION,
        post_revision: CLAIMS_REVISION + 1,
        liability_units: 0,
        refund_lamports: aggregate_account.lamports,
        claim_count: aggregate.claim_count,
    })
    .expect("Claims receipt");
    let claims_receipt_digest = hash(&claims_receipt.to_bytes()).to_bytes();

    let close_vault_digest = hash(&close_vault_bytes).to_bytes();
    let close_vault_poststate = custody_poststate(
        close_vault_digest,
        fixture.base.vault,
        fixture.base.rent_credit,
        vault_account.lamports,
    );
    let replay_after_vault = replay
        .advance(close_vault, close_vault_digest, close_vault_poststate)
        .expect("CloseVault replay transition");
    let replay_after_vault_bytes = replay_after_vault
        .to_bytes()
        .expect("post-CloseVault replay");
    let close_vault_receipt = CustodyReceiptV1::new(
        close_vault,
        close_vault_digest,
        ReceiptEvidenceV1 {
            source_before: 0,
            source_after: 0,
            destination_before: 0,
            destination_after: 0,
            poststate_commitment: close_vault_poststate,
            replay_state_digest: hash(&replay_after_vault_bytes).to_bytes(),
        },
    )
    .expect("CloseVault receipt");
    let close_vault_receipt_digest = hash(
        &close_vault_receipt
            .to_bytes()
            .expect("CloseVault receipt bytes"),
    )
    .to_bytes();

    let close_replay_digest = hash(&close_replay_bytes).to_bytes();
    let close_replay_poststate = custody_poststate(
        close_replay_digest,
        fixture.base.replay,
        fixture.base.rent_credit,
        replay_account.lamports,
    );
    replay_after_vault
        .advance(close_replay, close_replay_digest, close_replay_poststate)
        .expect("CloseReplay transition");
    let close_replay_receipt = CustodyReceiptV1::new(
        close_replay,
        close_replay_digest,
        ReceiptEvidenceV1 {
            source_before: 0,
            source_after: 0,
            destination_before: 0,
            destination_after: 0,
            poststate_commitment: close_replay_poststate,
            replay_state_digest: hash(&[]).to_bytes(),
        },
    )
    .expect("CloseReplay receipt");
    let close_replay_receipt_digest = hash(
        &close_replay_receipt
            .to_bytes()
            .expect("CloseReplay receipt bytes"),
    )
    .to_bytes();

    let final_credit = credit_account
        .lamports
        .checked_add(aggregate_account.lamports)
        .and_then(|value| value.checked_add(vault_account.lamports))
        .and_then(|value| value.checked_add(replay_account.lamports))
        .and_then(|value| value.checked_add(market_account.lamports))
        .expect("complete retirement refund");
    let source_digest = hash(&source_account.data).to_bytes();
    let post_resource_digest = hashv(&[
        RETIREMENT_POST_RESOURCE_DIGEST_DOMAIN_V1.as_slice(),
        &[RETIREMENT_ROLE_COUNT_V1],
        &[RETIREMENT_CUSTODY_RECEIPT_COUNT_V1],
        fixture.base.rent_credit.as_ref(),
        source_digest.as_slice(),
        claims_receipt_digest.as_slice(),
        close_vault_receipt_digest.as_slice(),
        close_replay_receipt_digest.as_slice(),
        market_account.lamports.to_le_bytes().as_slice(),
        aggregate_account.lamports.to_le_bytes().as_slice(),
        vault_account
            .lamports
            .checked_add(replay_account.lamports)
            .expect("Custody refund")
            .to_le_bytes()
            .as_slice(),
        final_credit.to_le_bytes().as_slice(),
    ])
    .to_bytes();
    let rent_close_authority = Pubkey::find_program_address(
        &[
            LIFECYCLE_RENT_CORE_CLOSE_AUTHORITY_DOMAIN_V2,
            fixture.base.rent_credit.as_ref(),
            post_resource_digest.as_slice(),
        ],
        &CORE_PROGRAM_ID,
    )
    .0;
    let bundle = RetirementBundleV1::new(RetirementBundleInputV1 {
        market: fixture.base.market.to_bytes(),
        release_set: fixture.base.release_set,
        rent_credit: fixture.base.rent_credit.to_bytes(),
        source_receipt_account: fixture.base.closure.to_bytes(),
        claims_aggregate: fixture.claims_aggregate.to_bytes(),
        custody_replay: fixture.base.replay.to_bytes(),
        hoard_vault: fixture.base.vault.to_bytes(),
        source_receipt_digest: source_digest,
        claims_request_digest,
        custody_close_vault_request_digest: close_vault_digest,
        custody_close_replay_request_digest: close_replay_digest,
        core_prestate_digest: hash(&market_account.data).to_bytes(),
        generation: GENERATION,
        source_closure_revision: source.terminal_sequence + 1,
        claims_pre_revision: CLAIMS_REVISION,
        claims_post_revision: CLAIMS_REVISION + 1,
        custody_pre_revision: CUSTODY_REVISION,
        custody_middle_revision: CUSTODY_REVISION + 1,
        custody_post_revision: CUSTODY_REVISION + 2,
        expected_core_lamports: market_account.lamports,
    })
    .expect("retirement bundle");

    let claims_authority = caller_authority(
        fixture.base.release_set,
        fixture.base.market,
        parent_digest,
        &claims_bytes,
    );
    let close_vault_authority = caller_authority(
        fixture.base.release_set,
        fixture.base.market,
        CUSTODY_CONTEXT,
        &close_vault_bytes,
    );
    let close_replay_authority = caller_authority(
        fixture.base.release_set,
        fixture.base.market,
        CUSTODY_CONTEXT,
        &close_replay_bytes,
    );
    let mut data = Vec::with_capacity(2_152);
    data.extend_from_slice(&core_bytes);
    data.extend_from_slice(&bundle.to_bytes());
    data.extend_from_slice(&claims_bytes);
    data.extend_from_slice(&close_vault_bytes);
    data.extend_from_slice(&close_replay_bytes);
    assert_eq!(data.len(), 2_152);
    let direct_instruction = Instruction {
        program_id: CORE_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(fixture.base.market, false),
            AccountMeta::new(fixture.base.rent_credit, false),
            AccountMeta::new_readonly(fixture.base.activation, false),
            AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
            AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.base.core_programdata, false),
            AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.claims_programdata, false),
            AccountMeta::new_readonly(RESOLUTION_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.base.resolution_programdata, false),
            AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.base.custody_programdata, false),
            AccountMeta::new_readonly(RENT_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.base.closure, false),
            AccountMeta::new(fixture.claims_aggregate, false),
            AccountMeta::new(fixture.base.replay, false),
            AccountMeta::new(fixture.base.vault, false),
            AccountMeta::new_readonly(fixture.base.custody_authority, false),
            AccountMeta::new_readonly(fixture.base.mint, false),
            AccountMeta::new_readonly(Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID), false),
            AccountMeta::new_readonly(fixture.base.realm_record.raw, false),
            AccountMeta::new_readonly(fixture.base.realm_record.staging, false),
            AccountMeta::new_readonly(claims_authority, false),
            AccountMeta::new_readonly(close_vault_authority, false),
            AccountMeta::new_readonly(close_replay_authority, false),
            AccountMeta::new_readonly(fixture.infrastructure_profile, false),
            AccountMeta::new_readonly(fixture.registry_artifact.raw, false),
            AccountMeta::new_readonly(fixture.registry_artifact.staging, false),
            AccountMeta::new_readonly(programdata(REGISTRY_PROGRAM_ID), false),
            AccountMeta::new_readonly(fixture.rent_artifact.raw, false),
            AccountMeta::new_readonly(fixture.rent_artifact.staging, false),
            AccountMeta::new_readonly(fixture.rent_programdata, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
            AccountMeta::new(fixture.refund_wallet, false),
            AccountMeta::new_readonly(rent_close_authority, false),
        ],
        data,
    };
    let activation = observed(context, fixture.base.activation)
        .await
        .expect("Registry activation cache");
    let activation_cache_digest = id(hash(&activation.data).to_bytes());
    let instruction = registry_retirement_continuation(
        fixture,
        activation_cache_digest,
        direct_instruction.clone(),
    );
    RetirementPlan {
        instruction,
        direct_instruction,
        activation_cache_digest,
        expected_refund_delta: final_credit,
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

#[tokio::test]
async fn joined_retirement_is_atomic_through_rent_close_last() {
    let (fixture, mut context) = joined_fixture().await;
    let mut clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("ProgramTest Clock");
    clock.unix_timestamp = TERMINAL_TIME + 1;
    context.set_sysvar(&clock);

    let admit =
        build_resolution_admit_terminal_v3(&admit_snapshot(&mut context, &fixture.base).await)
            .expect("chain-derived AdmitTerminal");
    submit(&mut context, &[admit.instruction])
        .await
        .expect("Core -> Resolution terminal admission");
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
    let missing_source =
        registry_retirement_continuation(&fixture, plan.activation_cache_digest, missing_source);
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
    let substituted_source = registry_retirement_continuation(
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
        registry_retirement_continuation(&fixture, plan.activation_cache_digest, reordered);
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
    let late_rent_refusal =
        registry_retirement_continuation(&fixture, plan.activation_cache_digest, late_rent_refusal);
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
