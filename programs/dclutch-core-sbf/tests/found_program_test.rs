//! Real-ELF Core Found31 infrastructure and Runtime Product V2 composition.

use std::{env, fs, path::PathBuf, vec, vec::Vec};

use dclutch_capability_contract::{CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, EMPTY_MANIFEST_BYTES};
use dclutch_core_contract::ContentId as CoreContentId;
use dclutch_market_core_codec::{
    Action, CoreState, Identity, MarketCoreStateSeedsV2, MarketIdentity, Phase, Readiness, Request,
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
    ArtifactReleaseIdV1, EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1, ExecutionReleaseSetV1,
    ExecutionRoleBindingV1, ExecutionRoleV1, PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1,
    ProgramIdentityV1, ProtocolInfrastructureProfileV1,
};
use dclutch_rent_contract::{RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority, RentCreditV1};
use dclutch_source_contract::{
    ContentId as SourceContentId, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2, SourceMaterialV2,
};
use solana_account::Account;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::ProgramTest;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_transaction::Transaction;

const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc1; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc2; 32]);
const RENT_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xc3; 32]);
const GENERATION: u64 = 41;

struct Artifacts {
    core: Vec<u8>,
    registry: Vec<u8>,
    rent: Vec<u8>,
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
    source: Record,
    manifest: Record,
    release_set: Record,
    cache: Pubkey,
    core_programdata: Pubkey,
    registry_programdata: Pubkey,
    rent_programdata: Pubkey,
    profile: Pubkey,
    registry_artifact: Record,
    rent_artifact: Record,
    outcome_count: u32,
}

fn artifacts() -> Artifacts {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    Artifacts {
        core: fs::read(directory.join("dclutch_core_sbf.so")).expect("Core ELF"),
        registry: fs::read(directory.join("dclutch_registry_sbf.so")).expect("Registry ELF"),
        rent: fs::read(directory.join("dclutch_rent_sbf.so")).expect("Rent ELF"),
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

fn product_graph() -> (Record, Record, Record, u32, [u8; 32]) {
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
            liability_basis_id: product_id(5),
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
    (
        Record::from_coordinate(report.receipt.product, product.to_vec()),
        Record::from_coordinate(report.receipt.result_domain, domain),
        Record::from_coordinate(report.receipt.portfolio, portfolio),
        report.outcome_count,
        product_id(1).to_bytes(),
    )
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
    let core_release = release(CORE_PROGRAM_ID, &artifacts.core, 0xa0, mutable_authority);
    let registry_release = release(REGISTRY_PROGRAM_ID, &artifacts.registry, 0xa1, None);
    let rent_release = release(RENT_PROGRAM_ID, &artifacts.rent, 0xa2, None);
    let core_binding = binding(core_release);
    let release_set_value = ExecutionReleaseSetV1::new(
        core_binding,
        core_binding,
        core_binding,
        core_binding,
        core_binding,
    )
    .expect("release set");
    let release_set_id =
        CoreContentId::new(hash(&release_set_value.to_bytes()).to_bytes()).expect("release set ID");
    let mut cache_data = vec![0; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut cache_data, release_set_id).expect("cache");
    for role in [
        ExecutionRoleV1::Core,
        ExecutionRoleV1::Claims,
        ExecutionRoleV1::Trading,
        ExecutionRoleV1::Resolution,
        ExecutionRoleV1::Custody,
    ] {
        activate_execution_role_into_v1(
            &mut cache_data,
            release_set_id,
            &release_set_value,
            role,
            &activation_input(core_release),
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

    let (product, domain, portfolio, outcome_count, stable_product_id) = product_graph();
    let realm_value = RealmV1::new(RealmV1Input {
        token_program: [0xb1; 32],
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
    let manifest = Record::new(
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        EMPTY_MANIFEST_BYTES.to_vec(),
    );
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
        &source,
        &manifest,
        &release_set,
        &registry_artifact,
        &rent_artifact,
    ] {
        record.add(&mut test);
    }

    let payer = Keypair::new();
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
    let refund = RefundAuthority::new(payer.pubkey().to_bytes()).expect("refund");
    let (rent_credit, rent_bump) = Pubkey::find_program_address(
        &[RENT_CREDIT_PDA_DOMAIN_V1, &refund.to_bytes()],
        &RENT_PROGRAM_ID,
    );
    let rent_credit_data = RentCreditV1::new(refund, rent_bump).to_bytes().to_vec();
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
    test.add_account(
        market,
        Account {
            lamports: 1,
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
        source,
        manifest,
        release_set,
        cache,
        core_programdata: programdata_address(CORE_PROGRAM_ID),
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
    assert_eq!(market.lamports, 1);
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
    assert_eq!(market.lamports, 1);
}
