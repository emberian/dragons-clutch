//! Real-SVM evidence for the current Resolution terminal-retirement waist.
//!
//! The fixture loads only compiled Registry, Core, and Resolution ELFs. It
//! starts from an authenticated provider-produced terminal Source/certificate
//! boundary, then executes the chain-derived AdmitTerminal, permissionless
//! BeginRetiring, and chain-derived CloseFund instructions. A deliberately
//! stale second retirement instruction proves rollback of the preceding
//! physical close across Core, Source, all three Funds, the closure output,
//! and the immutable RentCredit beneficiary.

use std::{env, fs, path::PathBuf};

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    CapabilityEntryV1, CapabilityFundingDerivationV1, CapabilityManifestV1, CompartmentFundingV1,
    ContentId as CapabilityContentId, FUNDING_STATE_BYTES, FundingAmountsV1,
    FundingCustodyObservationV1, FundingQuoteV1, FundingStateV1, FundingStatus,
    MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_core_contract::ContentId as CoreContentId;
use dclutch_market_core_codec::{
    Action, CoreState, Identity as CoreIdentity, MarketCoreStateSeedsV2, MarketIdentity, Phase,
    Readiness, Request,
};
use dclutch_product_runtime_v2::{
    ContentId as ProductContentId, PortfolioInputV2, ResultDomainInputV2, ResultDomainV2,
    compile_portfolio_v2, compile_result_domain_v2, portfolio_record_bytes,
    result_domain_record_bytes,
};
use dclutch_product_runtime_v2_admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_BYTES_V2, PRODUCT_RECORD_SCHEMA_ID_V2, ProductRecordV2,
    RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetV1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, activate_execution_role_into_v1,
    initialize_activation_cache_v1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1,
    ProgramIdentityV1,
};
use dclutch_rent_contract::{RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority, RentCreditV1};
use dclutch_resolution_codec::{
    RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3, RESOLUTION_CONTROLLER_RELEASE_ID_V4,
    ResolutionCertificateKindV2, ResolutionCertificateV2, SOURCE_CLOSURE_RECEIPT_BYTES_V2,
    SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V2, SourceClosureReceiptV2,
};
use dclutch_resolution_core_v3_operator::{
    Finality, Observation, ObservedAccount, ResolutionAdmitTerminalSnapshotV3,
    ResolutionCloseFundSnapshotV3, ResolutionCreateFundSnapshotV3,
    ResolutionVerifyFundReadySnapshotV3, build_resolution_admit_terminal_v3,
    build_resolution_close_fund_v3, build_resolution_create_fund_v3,
    build_resolution_verify_fund_ready_v3, validate_resolution_close_fund_report_v3,
    validate_resolution_create_fund_report_v3, validate_resolution_verify_fund_ready_report_v3,
};
use dclutch_source_contract::{
    CapacityEnvelope, ContentId as SourceContentId, RECOVERY_POLICY_SCHEMA_ID_V2,
    RecoveryAttemptV2, RecoveryPolicyV2, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
    SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2, SourceCapacityProfileV1, SourceMaterialV2,
    SourceResolutionPhaseV1, SourceResolutionStateV2,
};
use solana_account::Account;
use solana_program::{
    clock::Clock,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::Signer;
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, system_program, sysvar};
use solana_system_interface::instruction::transfer;
use solana_transaction::Transaction;

const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x71; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x72; 32]);
const RESOLUTION_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x73; 32]);
const RENT_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x74; 32]);
const GENERATION: u64 = 7;
const TERMINAL_SEQUENCE: u64 = 1;
const TERMINAL_TIME: i64 = 1_787_431_680;
const BOUNTY: u64 = 7;

struct Elves {
    core: Vec<u8>,
    registry: Vec<u8>,
    resolution: Vec<u8>,
}

#[derive(Clone, Copy)]
struct RecordPair {
    raw: Pubkey,
    staging: Pubkey,
}

struct Fixture {
    test: Option<ProgramTest>,
    market: Pubkey,
    activation: Pubkey,
    core_programdata: Pubkey,
    resolution_programdata: Pubkey,
    source_material: RecordPair,
    capability_manifest: RecordPair,
    recovery_policy: RecordPair,
    product: RecordPair,
    domain: RecordPair,
    portfolio: RecordPair,
    source: Pubkey,
    funding: [Pubkey; 3],
    certificate: Pubkey,
    closure: Pubkey,
    rent_credit: Pubkey,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RetirementSnapshot {
    market: Option<Account>,
    source: Option<Account>,
    funding: [Option<Account>; 3],
    certificate: Option<Account>,
    closure: Option<Account>,
    rent_credit: Option<Account>,
}

fn id(bytes: [u8; 32]) -> CoreContentId {
    CoreContentId::new(bytes).expect("nonzero content identity")
}

fn product_id(bytes: [u8; 32]) -> ProductContentId {
    ProductContentId::new(bytes).expect("nonzero Product identity")
}

fn source_id(bytes: [u8; 32]) -> SourceContentId {
    SourceContentId::new(bytes).expect("nonzero Source identity")
}

fn artifacts() -> Elves {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    Elves {
        core: fs::read(directory.join("dclutch_core_sbf.so")).expect("Core ELF"),
        registry: fs::read(directory.join("dclutch_registry_sbf.so")).expect("Registry ELF"),
        resolution: fs::read(directory.join("dclutch_resolution_proof_sbf.so"))
            .expect("Resolution ELF"),
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
        id(semantic),
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
    let content = id(release_set_id);
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

fn add_record(test: &mut ProgramTest, schema: [u8; 32], data: Vec<u8>) -> RecordPair {
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
    RecordPair { raw, staging }
}

fn add_active_funding(
    test: &mut ProgramTest,
    market: Pubkey,
    manifest_id: CapabilityContentId,
    manifest: CapabilityManifestV1<'_>,
    entry_index: u16,
) -> Pubkey {
    let rent = Rent::default().minimum_balance(FUNDING_STATE_BYTES);
    let custody = FundingCustodyObservationV1::native_only(
        rent.checked_mul(2)
            .and_then(|value| value.checked_add(BOUNTY))
            .expect("bounded funding custody"),
        rent,
    )
    .expect("native funding custody");
    let mut state = FundingStateV1::new(manifest_id, manifest, entry_index, custody)
        .expect("pending FundingState");
    state
        .activate(manifest_id, manifest, custody, 1)
        .expect("active FundingState");
    let key = funding_key(market, manifest_id, manifest, entry_index);
    test.add_account(
        key,
        Account {
            lamports: rent + BOUNTY,
            data: state.to_bytes().to_vec(),
            owner: RESOLUTION_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    key
}

fn funding_key(
    market: Pubkey,
    manifest_id: CapabilityContentId,
    manifest: CapabilityManifestV1<'_>,
    entry_index: u16,
) -> Pubkey {
    let rent = Rent::default().minimum_balance(FUNDING_STATE_BYTES);
    let custody = FundingCustodyObservationV1::native_only(
        rent.checked_mul(2)
            .and_then(|value| value.checked_add(BOUNTY))
            .expect("bounded funding custody"),
        rent,
    )
    .expect("native funding custody");
    let state = FundingStateV1::new(manifest_id, manifest, entry_index, custody)
        .expect("pending FundingState");
    let derivation = CapabilityFundingDerivationV1::new(
        market.to_bytes(),
        GENERATION,
        manifest_id,
        manifest,
        state,
    )
    .expect("funding derivation");
    Pubkey::find_program_address(&derivation.seed_components(), &RESOLUTION_PROGRAM_ID).0
}

fn fixture(preload_terminal: bool) -> Fixture {
    let elves = artifacts();
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    add_program(
        &mut test,
        "dclutch_registry_sbf",
        REGISTRY_PROGRAM_ID,
        &elves.registry,
    );
    add_program(&mut test, "dclutch_core_sbf", CORE_PROGRAM_ID, &elves.core);
    add_program(
        &mut test,
        "dclutch_resolution_proof_sbf",
        RESOLUTION_PROGRAM_ID,
        &elves.resolution,
    );

    let core_release = release(CORE_PROGRAM_ID, [0x41; 32], &elves.core);
    let resolution_release = release(
        RESOLUTION_PROGRAM_ID,
        RESOLUTION_CONTROLLER_RELEASE_ID_V4,
        &elves.resolution,
    );
    let (release_set, activation_data) = activation(core_release, resolution_release);
    let activation = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    test.add_account(
        activation,
        protocol_account(REGISTRY_PROGRAM_ID, activation_data),
    );

    let coordinate_id = [0x81; 32];
    let unit_id = [0x82; 32];
    let product_identity = [0x83; 32];
    let liability_id = [0x84; 32];
    let representation_release = [0x85; 32];
    let mapping_release = [0x86; 32];
    let cuts = [0_i128];
    let coefficients = [1_u64, 1, 1];
    let mut domain_bytes = vec![0; result_domain_record_bytes(cuts.len()).expect("domain width")];
    compile_result_domain_v2(
        ResultDomainInputV2 {
            product_id: product_id(product_identity),
            coordinate_domain_id: product_id(coordinate_id),
            result_unit_id: product_id(unit_id),
            liability_basis_id: product_id(liability_id),
            representation_release_id: product_id(representation_release),
            mapping_release_id: product_id(mapping_release),
            cut_denominator: 1,
            cuts: &cuts,
        },
        &mut domain_bytes,
    )
    .expect("Product result domain");
    let domain_id = hash(&domain_bytes).to_bytes();
    let mut portfolio_bytes =
        vec![0; portfolio_record_bytes(coefficients.len()).expect("portfolio width")];
    compile_portfolio_v2(
        PortfolioInputV2 {
            product_id: product_id(product_identity),
            result_domain_id: product_id(domain_id),
            claim_basis_id: product_id([0x87; 32]),
            liability_basis_id: product_id(liability_id),
            representation_release_id: product_id(representation_release),
            denominator: 1,
            coefficients: &coefficients,
        },
        &mut portfolio_bytes,
    )
    .expect("Product portfolio");
    let portfolio_id = hash(&portfolio_bytes).to_bytes();
    let mut product_bytes = [0; PRODUCT_RECORD_BYTES_V2];
    ProductRecordV2::new(
        product_id(product_identity),
        product_id(domain_id),
        product_id(portfolio_id),
    )
    .encode_into(&mut product_bytes)
    .expect("Product root");
    let product_record_id = hash(&product_bytes).to_bytes();

    let capacity = SourceCapacityProfileV1::new(
        CapacityEnvelope::Measured,
        1,
        1,
        source_id([0x91; 32]),
        source_id([0x92; 32]),
        256,
        0,
    )
    .expect("Source capacity");
    let capacity_id = source_id(hash(&capacity.to_bytes()).to_bytes());
    let recovery_allocation = source_id([0x93; 32]);
    let recovery_policy_value = RecoveryPolicyV2::new(
        capacity_id,
        [
            Some(
                RecoveryAttemptV2::new(
                    source_id([0x94; 32]),
                    source_id([0x95; 32]),
                    TERMINAL_TIME + 20,
                    recovery_allocation,
                )
                .expect("recovery attempt"),
            ),
            None,
            None,
            None,
        ],
        1,
    )
    .expect("recovery policy");
    let recovery_policy_bytes = recovery_policy_value.to_bytes();
    let recovery_policy_id = hash(&recovery_policy_bytes).to_bytes();
    let material_value = SourceMaterialV2::new(
        source_id(product_record_id),
        source_id([0x96; 32]),
        source_id([0x97; 32]),
        source_id([0x98; 32]),
        Some(source_id(recovery_policy_id)),
        source_id([0x99; 32]),
    );
    let material_bytes = material_value.to_bytes();
    let material_id = hash(&material_bytes).to_bytes();

    let funding_rent = Rent::default().minimum_balance(FUNDING_STATE_BYTES);
    let quote = FundingQuoteV1::new(
        FundingAmountsV1::new(
            CompartmentFundingV1::native_lamports(funding_rent).expect("funding rent"),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::native_lamports(BOUNTY).expect("worker bounty"),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
        )
        .expect("typed funding"),
        None,
    )
    .expect("funding quote");
    let entries = [
        (0xa1, recovery_allocation.to_bytes()),
        (0xa2, recovery_policy_id),
        (0xa3, material_id),
    ]
    .map(|(seed, config)| {
        CapabilityEntryV1::new(
            id([seed; 32]),
            id(RESOLUTION_CONTROLLER_RELEASE_ID_V4),
            id(config),
            id([0xa4; 32]),
            id([0xa5; 32]),
            id([0xa6; 32]),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            quote,
        )
        .expect("Resolution funding entry")
    });
    let mut manifest_bytes = vec![0; MANIFEST_HEADER_BYTES + 3 * CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&entries, &mut manifest_bytes).expect("capability manifest");
    let manifest_id_bytes = hash(&manifest_bytes).to_bytes();
    let manifest = CapabilityManifestV1::decode(&manifest_bytes).expect("manifest view");

    let source_material = add_record(
        &mut test,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
        material_bytes.to_vec(),
    );
    let capability_manifest = add_record(
        &mut test,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        manifest_bytes.clone(),
    );
    let recovery_policy = add_record(
        &mut test,
        RECOVERY_POLICY_SCHEMA_ID_V2,
        recovery_policy_bytes.to_vec(),
    );
    let product = add_record(
        &mut test,
        PRODUCT_RECORD_SCHEMA_ID_V2,
        product_bytes.to_vec(),
    );
    let domain = add_record(&mut test, RESULT_DOMAIN_SCHEMA_ID_V2, domain_bytes.clone());
    let portfolio = add_record(&mut test, PORTFOLIO_SCHEMA_ID_V2, portfolio_bytes);

    let refund_authority = RefundAuthority::new([0xb1; 32]).expect("rent refund authority");
    let (rent_credit, rent_credit_bump) = Pubkey::find_program_address(
        &[RENT_CREDIT_PDA_DOMAIN_V1, &refund_authority.to_bytes()],
        &RENT_PROGRAM_ID,
    );
    let rent_credit_value = RentCreditV1::new(refund_authority, rent_credit_bump);
    test.add_account(
        rent_credit,
        protocol_account(RENT_PROGRAM_ID, rent_credit_value.to_bytes().to_vec()),
    );

    let mut identity = MarketIdentity {
        market_id: CoreIdentity::new([0xff; 32]).expect("placeholder Market"),
        realm_id: CoreIdentity::new([0xb2; 32]).expect("Realm"),
        product_record: CoreIdentity::new(product_record_id).expect("Product record"),
        product_id: CoreIdentity::new(product_identity).expect("Product"),
        resolution_policy: CoreIdentity::new(material_id).expect("Source material"),
        capability_manifest: CoreIdentity::new(manifest_id_bytes).expect("manifest"),
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
        phase: if preload_terminal {
            Phase::Open
        } else {
            Phase::Founding
        },
        readiness: if preload_terminal {
            Readiness::Consumed
        } else {
            Readiness::Prepaid
        },
        terminal_winner: 0,
        identity,
        outstanding_capabilities: 0,
        rent_beneficiary: CoreIdentity::new(rent_credit.to_bytes()).expect("RentCredit"),
        terminal_receipt: None,
    };
    test.add_account(
        market,
        protocol_account(
            CORE_PROGRAM_ID,
            state.encode().expect("Core state").to_vec(),
        ),
    );

    let (source, source_bump) = Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
            market.as_ref(),
            &GENERATION.to_le_bytes(),
        ],
        &RESOLUTION_PROGRAM_ID,
    );
    if !preload_terminal {
        test.add_account(
            source,
            Account {
                lamports: 1,
                data: Vec::new(),
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            },
        );
    }
    let manifest_id = CapabilityContentId::new(manifest_id_bytes).expect("manifest identity");
    let funding = if preload_terminal {
        [0_u16, 1, 2]
            .map(|entry| add_active_funding(&mut test, market, manifest_id, manifest, entry))
    } else {
        [0_u16, 1, 2].map(|entry| funding_key(market, manifest_id, manifest, entry))
    };
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
    if preload_terminal {
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
        let result_domain = ResultDomainV2::decode(&domain_bytes).expect("ResultDomain view");
        let decision = source_value
            .resolve_primary_from_authenticated_domain(
                source_id(material_id),
                material_value,
                source_id(product_record_id),
                result_domain,
                source_id([0xb3; 32]),
                -1,
                1,
                GENERATION,
                TERMINAL_TIME,
                TERMINAL_SEQUENCE,
            )
            .expect("provider-authenticated terminal Source projection");
        assert_eq!(decision.selector(), 0);
        test.add_account(
            source,
            protocol_account(RESOLUTION_PROGRAM_ID, source_value.to_bytes().to_vec()),
        );
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
            observed_at: u64::try_from(TERMINAL_TIME).expect("positive terminal time"),
        };
        test.add_account(
            certificate,
            protocol_account(
                RESOLUTION_PROGRAM_ID,
                certificate_value
                    .to_bytes()
                    .expect("terminal certificate")
                    .to_vec(),
            ),
        );
    }
    let closure = Pubkey::find_program_address(
        &[
            SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V2,
            source.as_ref(),
            &(TERMINAL_SEQUENCE + 1).to_le_bytes(),
        ],
        &RESOLUTION_PROGRAM_ID,
    )
    .0;

    Fixture {
        test: Some(test),
        market,
        activation,
        core_programdata: programdata(CORE_PROGRAM_ID),
        resolution_programdata: programdata(RESOLUTION_PROGRAM_ID),
        source_material,
        capability_manifest,
        recovery_policy,
        product,
        domain,
        portfolio,
        source,
        funding,
        certificate,
        closure,
        rent_credit,
    }
}

fn finality() -> Observation {
    Observation {
        slot: 1,
        unix_timestamp: TERMINAL_TIME,
        finality: Finality::Finalized,
    }
}

fn into_observed(key: Pubkey, account: Account) -> ObservedAccount {
    ObservedAccount {
        observation: finality(),
        key,
        owner: account.owner,
        lamports: account.lamports,
        executable: account.executable,
        data: account.data,
    }
}

async fn observed(context: &mut ProgramTestContext, key: Pubkey) -> Option<Account> {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account query")
}

async fn required_observed(context: &mut ProgramTestContext, key: Pubkey) -> ObservedAccount {
    into_observed(
        key,
        observed(context, key)
            .await
            .expect("required account exists"),
    )
}

async fn observed_or_vacant(context: &mut ProgramTestContext, key: Pubkey) -> ObservedAccount {
    match observed(context, key).await {
        Some(account) => into_observed(key, account),
        None => vacant_observed(key),
    }
}

fn vacant_observed(key: Pubkey) -> ObservedAccount {
    ObservedAccount {
        observation: finality(),
        key,
        owner: system_program::ID,
        lamports: 0,
        executable: false,
        data: Vec::new(),
    }
}

async fn submit(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
) -> Result<(), BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let transaction = Transaction::new_signed_with_payer(
        instructions,
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    context.banks_client.process_transaction(transaction).await
}

async fn create_snapshot(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
) -> ResolutionCreateFundSnapshotV3 {
    ResolutionCreateFundSnapshotV3 {
        market: required_observed(context, fixture.market).await,
        activation_cache: required_observed(context, fixture.activation).await,
        registry_program: required_observed(context, REGISTRY_PROGRAM_ID).await,
        core_program: required_observed(context, CORE_PROGRAM_ID).await,
        core_programdata: required_observed(context, fixture.core_programdata).await,
        resolution_program: required_observed(context, RESOLUTION_PROGRAM_ID).await,
        resolution_programdata: required_observed(context, fixture.resolution_programdata).await,
        source_material: required_observed(context, fixture.source_material.raw).await,
        source_material_staging: vacant_observed(fixture.source_material.staging),
        capability_manifest: required_observed(context, fixture.capability_manifest.raw).await,
        capability_manifest_staging: vacant_observed(fixture.capability_manifest.staging),
        source_destination: observed_or_vacant(context, fixture.source).await,
        recovery_destination: observed_or_vacant(context, fixture.funding[0]).await,
        exhaustion_destination: observed_or_vacant(context, fixture.funding[1]).await,
        failure_destination: observed_or_vacant(context, fixture.funding[2]).await,
        rent_sysvar: required_observed(context, sysvar::rent::ID).await,
        system_program: required_observed(context, system_program::ID).await,
        recovery_policy: required_observed(context, fixture.recovery_policy.raw).await,
        recovery_policy_staging: vacant_observed(fixture.recovery_policy.staging),
    }
}

async fn verify_snapshot(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
) -> ResolutionVerifyFundReadySnapshotV3 {
    ResolutionVerifyFundReadySnapshotV3 {
        market: required_observed(context, fixture.market).await,
        activation_cache: required_observed(context, fixture.activation).await,
        registry_program: required_observed(context, REGISTRY_PROGRAM_ID).await,
        core_program: required_observed(context, CORE_PROGRAM_ID).await,
        core_programdata: required_observed(context, fixture.core_programdata).await,
        resolution_program: required_observed(context, RESOLUTION_PROGRAM_ID).await,
        resolution_programdata: required_observed(context, fixture.resolution_programdata).await,
        source_material: required_observed(context, fixture.source_material.raw).await,
        source_material_staging: vacant_observed(fixture.source_material.staging),
        capability_manifest: required_observed(context, fixture.capability_manifest.raw).await,
        capability_manifest_staging: vacant_observed(fixture.capability_manifest.staging),
        source_state: required_observed(context, fixture.source).await,
        recovery_funding: required_observed(context, fixture.funding[0]).await,
        exhaustion_funding: required_observed(context, fixture.funding[1]).await,
        failure_funding: required_observed(context, fixture.funding[2]).await,
        beneficiary: required_observed(context, fixture.rent_credit).await,
        clock_sysvar: required_observed(context, sysvar::clock::ID).await,
        rent_sysvar: required_observed(context, sysvar::rent::ID).await,
        recovery_policy: required_observed(context, fixture.recovery_policy.raw).await,
        recovery_policy_staging: vacant_observed(fixture.recovery_policy.staging),
    }
}

async fn admit_snapshot(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
) -> ResolutionAdmitTerminalSnapshotV3 {
    ResolutionAdmitTerminalSnapshotV3 {
        market: required_observed(context, fixture.market).await,
        activation_cache: required_observed(context, fixture.activation).await,
        registry_program: required_observed(context, REGISTRY_PROGRAM_ID).await,
        core_program: required_observed(context, CORE_PROGRAM_ID).await,
        core_programdata: required_observed(context, fixture.core_programdata).await,
        resolution_program: required_observed(context, RESOLUTION_PROGRAM_ID).await,
        resolution_programdata: required_observed(context, fixture.resolution_programdata).await,
        source_material: required_observed(context, fixture.source_material.raw).await,
        source_material_staging: vacant_observed(fixture.source_material.staging),
        capability_manifest: required_observed(context, fixture.capability_manifest.raw).await,
        capability_manifest_staging: vacant_observed(fixture.capability_manifest.staging),
        source_state: required_observed(context, fixture.source).await,
        recovery_funding: required_observed(context, fixture.funding[0]).await,
        exhaustion_funding: required_observed(context, fixture.funding[1]).await,
        failure_funding: required_observed(context, fixture.funding[2]).await,
        certificate: required_observed(context, fixture.certificate).await,
        rent_sysvar: required_observed(context, sysvar::rent::ID).await,
        product_raw: required_observed(context, fixture.product.raw).await,
        product_staging: vacant_observed(fixture.product.staging),
        result_domain_raw: required_observed(context, fixture.domain.raw).await,
        result_domain_staging: vacant_observed(fixture.domain.staging),
        portfolio_raw: required_observed(context, fixture.portfolio.raw).await,
        portfolio_staging: vacant_observed(fixture.portfolio.staging),
    }
}

fn begin_retiring_instruction(fixture: &Fixture) -> Instruction {
    let request = Request::administrative(
        Action::BeginRetiring,
        GENERATION,
        CoreIdentity::new(fixture.market.to_bytes()).expect("Market"),
    );
    Instruction {
        program_id: CORE_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(fixture.market, false),
            AccountMeta::new_readonly(fixture.activation, false),
            AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
            AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.core_programdata, false),
        ],
        data: request.encode().expect("BeginRetiring request").to_vec(),
    }
}

async fn close_snapshot(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
) -> ResolutionCloseFundSnapshotV3 {
    ResolutionCloseFundSnapshotV3 {
        market: required_observed(context, fixture.market).await,
        activation_cache: required_observed(context, fixture.activation).await,
        registry_program: required_observed(context, REGISTRY_PROGRAM_ID).await,
        core_program: required_observed(context, CORE_PROGRAM_ID).await,
        core_programdata: required_observed(context, fixture.core_programdata).await,
        resolution_program: required_observed(context, RESOLUTION_PROGRAM_ID).await,
        resolution_programdata: required_observed(context, fixture.resolution_programdata).await,
        source_material: required_observed(context, fixture.source_material.raw).await,
        source_material_staging: vacant_observed(fixture.source_material.staging),
        capability_manifest: required_observed(context, fixture.capability_manifest.raw).await,
        capability_manifest_staging: vacant_observed(fixture.capability_manifest.staging),
        source_state: required_observed(context, fixture.source).await,
        recovery_funding: required_observed(context, fixture.funding[0]).await,
        exhaustion_funding: required_observed(context, fixture.funding[1]).await,
        failure_funding: required_observed(context, fixture.funding[2]).await,
        certificate: required_observed(context, fixture.certificate).await,
        closure_destination: required_observed(context, fixture.closure).await,
        beneficiary: required_observed(context, fixture.rent_credit).await,
        clock_sysvar: required_observed(context, sysvar::clock::ID).await,
        rent_sysvar: required_observed(context, sysvar::rent::ID).await,
        system_program: required_observed(context, system_program::ID).await,
        recovery_policy: required_observed(context, fixture.recovery_policy.raw).await,
        recovery_policy_staging: vacant_observed(fixture.recovery_policy.staging),
    }
}

async fn retirement_snapshot(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
) -> RetirementSnapshot {
    RetirementSnapshot {
        market: observed(context, fixture.market).await,
        source: observed(context, fixture.source).await,
        funding: [
            observed(context, fixture.funding[0]).await,
            observed(context, fixture.funding[1]).await,
            observed(context, fixture.funding[2]).await,
        ],
        certificate: observed(context, fixture.certificate).await,
        closure: observed(context, fixture.closure).await,
        rent_credit: observed(context, fixture.rent_credit).await,
    }
}

#[tokio::test]
async fn current_resolution_creates_and_activates_exact_funding() {
    let mut fixture = fixture(false);
    let mut context = fixture
        .test
        .take()
        .expect("unstarted ProgramTest")
        .start_with_context()
        .await;
    let mut clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("ProgramTest Clock");
    clock.slot = clock.slot.max(1);
    clock.unix_timestamp = TERMINAL_TIME;
    context.set_sysvar(&clock);

    let payer = context.payer.pubkey();
    let create_snapshot = create_snapshot(&mut context, &fixture).await;
    assert_eq!(create_snapshot.system_program.key, system_program::ID);
    assert_eq!(create_snapshot.system_program.owner, native_loader::ID);
    assert!(create_snapshot.system_program.executable);
    assert!(!create_snapshot.system_program.data.is_empty());
    let create =
        build_resolution_create_fund_v3(&create_snapshot).expect("chain-derived CreateFund");
    validate_resolution_create_fund_report_v3(&create).expect("exact CreateFund report");
    assert!(create.source_top_up_lamports > 0);
    assert!(
        create
            .funding_top_up_lamports
            .iter()
            .all(|value| *value > 0)
    );

    let mut create_instructions = Vec::with_capacity(5);
    create_instructions.push(transfer(
        &payer,
        &fixture.source,
        create.source_top_up_lamports,
    ));
    for (funding, top_up) in fixture
        .funding
        .into_iter()
        .zip(create.funding_top_up_lamports)
    {
        create_instructions.push(transfer(&payer, &funding, top_up));
    }
    create_instructions.push(create.instruction);
    submit(&mut context, &create_instructions)
        .await
        .expect("prepay and create canonical Source funding");

    let source = SourceResolutionStateV2::decode(
        &observed(&mut context, fixture.source)
            .await
            .expect("created Source")
            .data,
    )
    .expect("Source state");
    assert_eq!(source.phase(), SourceResolutionPhaseV1::Primary);
    for funding in fixture.funding {
        let state = FundingStateV1::decode(
            &observed(&mut context, funding)
                .await
                .expect("created Funding")
                .data,
        )
        .expect("Funding state");
        assert_eq!(state.status(), FundingStatus::Pending);
    }

    let verify =
        build_resolution_verify_fund_ready_v3(&verify_snapshot(&mut context, &fixture).await)
            .expect("chain-derived VerifyFundReady");
    validate_resolution_verify_fund_ready_report_v3(&verify).expect("exact VerifyFundReady report");
    let before_privilege_refusal = retirement_snapshot(&mut context, &fixture).await;
    let mut read_only_beneficiary = verify.instruction.clone();
    read_only_beneficiary
        .accounts
        .get_mut(16)
        .expect("beneficiary account")
        .is_writable = false;
    assert!(
        submit(&mut context, &[read_only_beneficiary])
            .await
            .is_err(),
        "read-only beneficiary privilege must refuse"
    );
    assert_eq!(
        retirement_snapshot(&mut context, &fixture).await,
        before_privilege_refusal,
        "privilege refusal preserves Source, Funds, Core, and RentCredit"
    );

    let beneficiary_before = observed(&mut context, fixture.rent_credit)
        .await
        .expect("RentCredit")
        .lamports;
    submit(&mut context, &[verify.instruction])
        .await
        .expect("activate exact three-ledger Resolution funding");
    let ready = CoreState::decode(
        &observed(&mut context, fixture.market)
            .await
            .expect("Market")
            .data,
    )
    .expect("Core state");
    assert_eq!(ready.phase, Phase::Founding);
    assert_eq!(ready.readiness, Readiness::Ready);
    assert_eq!(
        observed(&mut context, fixture.rent_credit)
            .await
            .expect("RentCredit")
            .lamports,
        beneficiary_before + verify.expected_beneficiary_credit_lamports
    );
    for funding in fixture.funding {
        let state = FundingStateV1::decode(
            &observed(&mut context, funding)
                .await
                .expect("active Funding")
                .data,
        )
        .expect("Funding state");
        assert_eq!(state.status(), FundingStatus::Active);
    }
}

#[tokio::test]
async fn current_resolution_admits_retires_closes_and_rolls_back_late_refusal() {
    let mut fixture = fixture(true);
    let mut context = fixture
        .test
        .take()
        .expect("unstarted ProgramTest")
        .start_with_context()
        .await;
    let mut clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("ProgramTest Clock");
    clock.unix_timestamp = TERMINAL_TIME + 1;
    context.set_sysvar(&clock);

    let admit = build_resolution_admit_terminal_v3(&admit_snapshot(&mut context, &fixture).await)
        .expect("chain-derived AdmitTerminal");
    submit(&mut context, &[admit.instruction])
        .await
        .expect("Core -> Resolution terminal admission");
    let admitted = CoreState::decode(
        &observed(&mut context, fixture.market)
            .await
            .expect("Market")
            .data,
    )
    .expect("Core state");
    assert_eq!(admitted.phase, Phase::Terminal);
    assert_eq!(
        admitted.terminal_receipt.map(|value| value.to_bytes()),
        Some(fixture.certificate.to_bytes())
    );

    submit(&mut context, &[begin_retiring_instruction(&fixture)])
        .await
        .expect("permissionless authenticated BeginRetiring");
    let retiring = CoreState::decode(
        &observed(&mut context, fixture.market)
            .await
            .expect("Market")
            .data,
    )
    .expect("Core state");
    assert_eq!(retiring.phase, Phase::Retiring);

    let closure_rent = Rent::default().minimum_balance(SOURCE_CLOSURE_RECEIPT_BYTES_V2);
    let payer = context.payer.pubkey();
    submit(
        &mut context,
        &[transfer(&payer, &fixture.closure, closure_rent)],
    )
    .await
    .expect("prepay canonical closure receipt");
    let close = build_resolution_close_fund_v3(&close_snapshot(&mut context, &fixture).await)
        .expect("chain-derived CloseFund");
    validate_resolution_close_fund_report_v3(&close).expect("exact CloseFund report");

    let before_refusal = retirement_snapshot(&mut context, &fixture).await;
    let mut substituted_release = begin_retiring_instruction(&fixture);
    substituted_release
        .accounts
        .get_mut(4)
        .expect("Core ProgramData account")
        .pubkey = fixture.resolution_programdata;
    let refusal = submit(
        &mut context,
        &[
            close.instruction.clone(),
            // CloseFund has completed every physical write when this hostile
            // substitution refuses current Core release authentication.
            substituted_release,
        ],
    )
    .await;
    assert!(refusal.is_err(), "substituted Core release must refuse");
    assert_eq!(
        retirement_snapshot(&mut context, &fixture).await,
        before_refusal,
        "SVM rollback restores Core, Source, all three Funds, closure prepayment, certificate, and RentCredit"
    );

    submit(&mut context, &[close.instruction])
        .await
        .expect("Core -> Resolution physical close");
    assert!(observed(&mut context, fixture.source).await.is_none());
    for funding in fixture.funding {
        assert!(observed(&mut context, funding).await.is_none());
    }
    let closure = observed(&mut context, fixture.closure)
        .await
        .expect("Source closure receipt");
    assert_eq!(closure.owner, RESOLUTION_PROGRAM_ID);
    let closure = SourceClosureReceiptV2::decode(&closure.data).expect("closure receipt");
    assert_eq!(closure.market, fixture.market.to_bytes());
    assert_eq!(closure.terminal_certificate, fixture.certificate.to_bytes());
    assert_eq!(closure.beneficiary, fixture.rent_credit.to_bytes());
    let terminal =
        SourceResolutionStateV2::decode(&before_refusal.source.expect("Source prestate").data)
            .expect("terminal Source");
    assert_eq!(terminal.phase(), SourceResolutionPhaseV1::Resolved);
    let system = required_observed(&mut context, system_program::ID).await;
    assert_eq!(system.owner, native_loader::ID);
    assert!(system.executable);
    assert_eq!(
        CoreState::decode(
            &observed(&mut context, fixture.market)
                .await
                .expect("Market")
                .data,
        )
        .expect("Core state")
        .phase,
        Phase::Retiring,
        "CloseFund is a receipt-producing retirement component, not the full Claims/Custody retirement join"
    );
}
