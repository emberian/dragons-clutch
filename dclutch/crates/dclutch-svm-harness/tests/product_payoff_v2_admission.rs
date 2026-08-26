//! Real-ELF campaign for exact-rational Product evidence and Market admission.
//!
//! The Product evaluator and admission deployments execute the same checked
//! multiprogram ELF under distinct program identities.  The Resolution
//! deployment is the independently built Resolution ELF.  This campaign does
//! not treat either fixture bytes or a native processor as execution evidence.

use std::{env, fs, path::PathBuf};

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityManifestV1,
    CompartmentFundingV1, FundingAmountsV1, FundingQuoteV1, MANIFEST_HEADER_BYTES,
    MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_core_contract::{ContentId as CoreId, MarketIdentity, MarketRoot, Phase};
use dclutch_market_contract::market::{CategoricalMarketV1, CategoricalSettlementSummaryV1};
use dclutch_product_admission_contract::{
    AdmissionRoleV1, PAYOFF_ADMISSION_RECEIPT_BYTES_V1, PAYOFF_ADMISSION_RECEIPT_PDA_DOMAIN_V1,
    PRODUCT_PAYOFF_ADMISSION_KIND_ID_V1, PRODUCT_PAYOFF_ADMISSION_RECEIPT_DERIVATION_ID_V1,
    PRODUCT_PAYOFF_ADMISSION_RECEIPT_SCHEMA_ID_V1, PRODUCT_PAYOFF_ADMISSION_RELEASE_ID_V1,
    PRODUCT_PAYOFF_BINDING_SCHEMA_ID_V1, PayoffAdmissionReceiptV1, PayoffAdmissionRequestV1,
    PayoffBindingV1,
};
use dclutch_product_contract::{
    ContentId as ProductId,
    capacity::CapacityProfileId,
    product::{InstanceV1, InstanceV1Input},
    result_domain::{FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1, FiniteResultDomainV1},
};
use dclutch_product_payoff_v2_codec::{
    ABI_BYTES_V2, KNOT_BYTES_V2, KNOTS_OFFSET_V2, MAGIC_V2, TERMS_OFFSET_V2, VERSION_V2,
};
use dclutch_product_payoff_v2_svm::{
    CertificateKindV2, PAYOFF_CERTIFICATE_BYTES_V2, PAYOFF_CERTIFICATE_PDA_DOMAIN_V2,
    PRODUCT_PAYOFF_ADAPTER_RELEASE_ID_V2, PRODUCT_PAYOFF_SCHEMA_RELEASE_ID_V2, PayoffCertificateV2,
    PayoffRequestV2,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactReleaseV1, ArtifactUpgradePolicyV1,
};
use dclutch_release_set_contract::ProgramIdentityV1;
use dclutch_resolution_codec::RESOLUTION_CONTROLLER_RELEASE_ID_V3;
use solana_account::{Account, AccountSharedData};
use solana_program::{
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::Signer;
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_transaction::Transaction;

const PAYOFF_PROGRAM_ID: Pubkey = Pubkey::new_from_array([181; 32]);
const ADMISSION_PROGRAM_ID: Pubkey = Pubkey::new_from_array([182; 32]);
const RESOLUTION_PROGRAM_ID: Pubkey = Pubkey::new_from_array([183; 32]);
const REGISTRY_ID: Pubkey = Pubkey::new_from_array([184; 32]);
const GENERATION: u64 = 7;
// Adapter-owned PDA/schema constants are repeated only as hostile-client
// projections. The executing ELF remains their semantic owner and refuses any
// mismatch.
const MARKET_PDA_DOMAIN_V1: &[u8] = b"dclutch/market-root/v1";
const PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0x96, 0x20, 0xbc, 0xd9, 0xf3, 0x1a, 0x01, 0xca, 0x6f, 0x42, 0x09, 0x1c, 0x84, 0x57, 0x9d, 0x9a,
    0xcc, 0x48, 0x41, 0x27, 0xc0, 0x8d, 0x86, 0xac, 0xc4, 0x0f, 0xdd, 0x5a, 0x4c, 0xab, 0x1f, 0x14,
];
const FINITE_RESULT_DOMAIN_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0x37, 0x3d, 0x8d, 0xf3, 0x60, 0x73, 0xe8, 0x45, 0x54, 0xed, 0xa9, 0x89, 0x11, 0xb8, 0x3a, 0x9c,
    0x13, 0xcb, 0x07, 0x74, 0x54, 0x8f, 0x68, 0x0c, 0xba, 0x66, 0x29, 0x13, 0xdd, 0x66, 0x0e, 0x14,
];

#[derive(Clone, Copy)]
struct FinalizedRecord {
    raw: Pubkey,
    staging: Pubkey,
    digest: [u8; 32],
}

struct Fixture {
    context: ProgramTestContext,
    payoff: FinalizedRecord,
    binding: FinalizedRecord,
    manifest: FinalizedRecord,
    instance: FinalizedRecord,
    domain: FinalizedRecord,
    payoff_artifact: FinalizedRecord,
    admission_artifact: FinalizedRecord,
    resolution_artifact: FinalizedRecord,
    market: Pubkey,
    founding_market: Vec<u8>,
    open_market: Vec<u8>,
    payoff_programdata: Pubkey,
    admission_programdata: Pubkey,
    resolution_programdata: Pubkey,
    product_elf_digest: [u8; 32],
    resolution_elf_digest: [u8; 32],
}

fn require_sbf(name: &str) -> PathBuf {
    let output = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    let artifact = output.join(name);
    assert!(
        artifact.is_file(),
        "missing real ELF: {}",
        artifact.display()
    );
    artifact
}

fn core_id(bytes: [u8; 32]) -> CoreId {
    CoreId::new(bytes).expect("Core content identity")
}

fn product_id(bytes: [u8; 32]) -> ProductId {
    ProductId::new(bytes).expect("Product content identity")
}

fn zero_quote() -> FundingQuoteV1 {
    let none = CompartmentFundingV1::not_applicable();
    FundingQuoteV1::new(
        FundingAmountsV1::new(none, none, none, none, none, none, none).expect("amounts"),
        None,
    )
    .expect("zero quote")
}

fn payoff_bytes() -> Vec<u8> {
    let mut bytes = vec![0_u8; ABI_BYTES_V2];
    bytes[..8].copy_from_slice(&MAGIC_V2);
    bytes[8..10].copy_from_slice(&VERSION_V2.to_le_bytes());
    bytes[10] = 5;
    bytes[11] = 4;
    for (offset, value) in [(16, 81_u64), (24, 70), (32, 11), (40, 100), (48, 2)] {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
    for (index, knot) in [-100_i128, -50, 0, 50, 100].into_iter().enumerate() {
        let offset = KNOTS_OFFSET_V2 + index * KNOT_BYTES_V2;
        bytes[offset..offset + 16].copy_from_slice(&knot.to_le_bytes());
    }
    for (index, (tag, left, peak, right, amplitude)) in [
        (0_u8, 0_u8, 0_u8, 0_u8, 2_u64),
        (1, 0, 0, 4, 10),
        (2, 0, 0, 4, 5),
        (3, 1, 2, 3, 20),
    ]
    .into_iter()
    .enumerate()
    {
        let offset = TERMS_OFFSET_V2 + index * 16;
        bytes[offset..offset + 4].copy_from_slice(&[tag, left, peak, right]);
        bytes[offset + 8..offset + 16].copy_from_slice(&amplitude.to_le_bytes());
    }
    bytes
}

fn programdata_address(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn immutable_programdata(elf: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0_u8; 45 + elf.len()];
    bytes[..4].copy_from_slice(&3_u32.to_le_bytes());
    bytes[4..12].copy_from_slice(&0_u64.to_le_bytes());
    bytes[12] = 0;
    bytes[45..].copy_from_slice(elf);
    bytes
}

fn add_upgradeable_program(
    test: &mut ProgramTest,
    name: &'static str,
    program: Pubkey,
    elf: &[u8],
) -> Pubkey {
    test.add_upgradeable_program_to_genesis(name, &program);
    let programdata = programdata_address(program);
    let data = immutable_programdata(elf);
    test.add_account(
        programdata,
        Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner: bpf_loader_upgradeable::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    programdata
}

fn artifact(program: Pubkey, semantic_release: [u8; 32], elf: &[u8]) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        ProgramIdentityV1::new(program.to_bytes()).expect("program"),
        ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("loader"),
        programdata_address(program).to_bytes(),
        core_id(semantic_release),
        hash(elf).to_bytes(),
        0,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("artifact release")
}

fn record_addresses(schema: [u8; 32], digest: [u8; 32]) -> (Pubkey, Pubkey) {
    (
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &REGISTRY_ID).0,
        Pubkey::find_program_address(
            &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
            &REGISTRY_ID,
        )
        .0,
    )
}

fn add_finalized_record(
    test: &mut ProgramTest,
    schema: [u8; 32],
    data: Vec<u8>,
) -> FinalizedRecord {
    let digest = hash(&data).to_bytes();
    let (raw, staging) = record_addresses(schema, digest);
    test.add_account(
        raw,
        Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner: REGISTRY_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    test.add_account(staging, Account::new(0, 0, &system_program::ID));
    FinalizedRecord {
        raw,
        staging,
        digest,
    }
}

fn market_bytes(root: MarketRoot) -> Vec<u8> {
    let market =
        CategoricalMarketV1::<4>::new(root, 0, [0; 4], CategoricalSettlementSummaryV1::empty())
            .expect("categorical Market");
    let mut bytes = vec![0_u8; CategoricalMarketV1::<4>::encoded_len().expect("width")];
    market.encode(&mut bytes).expect("Market encode");
    bytes
}

async fn fixture() -> Fixture {
    let product_elf = fs::read(require_sbf("dclutch_product_evidence_sbf.so"))
        .expect("read Product evidence ELF");
    let resolution_elf =
        fs::read(require_sbf("dclutch_resolution_proof_sbf.so")).expect("read Resolution ELF");
    let payoff_release = artifact(
        PAYOFF_PROGRAM_ID,
        PRODUCT_PAYOFF_ADAPTER_RELEASE_ID_V2,
        &product_elf,
    );
    let admission_release = artifact(
        ADMISSION_PROGRAM_ID,
        PRODUCT_PAYOFF_ADMISSION_RELEASE_ID_V1,
        &product_elf,
    );
    let resolution_release = artifact(
        RESOLUTION_PROGRAM_ID,
        RESOLUTION_CONTROLLER_RELEASE_ID_V3,
        &resolution_elf,
    );
    let payoff_data = payoff_bytes();
    let payoff_digest = hash(&payoff_data).to_bytes();
    let domain =
        FiniteResultDomainV1::new(product_id([45; 32]), product_id([46; 32]), 2, &[-50, 0])
            .expect("finite result domain");
    let domain_data = domain.to_bytes().to_vec();
    let domain_id = hashv(&[FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1, &[0], &domain_data]).to_bytes();
    let capacity_id = [41; 32];
    let claim_basis_id = [44; 32];
    let instance = InstanceV1::new(InstanceV1Input {
        terms_id: product_id([42; 32]),
        occurrence_id: product_id([43; 32]),
        claim_basis_id: product_id(claim_basis_id),
        result_domain_id: product_id(domain_id),
        capacity_profile_id: CapacityProfileId::new(product_id(capacity_id)),
        partition_cell_count: 4,
    })
    .expect("Product instance");
    let instance_data = instance.to_bytes().to_vec();
    let instance_id = hash(&instance_data).to_bytes();
    let payoff_artifact_data = payoff_release.to_bytes().to_vec();
    let admission_artifact_data = admission_release.to_bytes().to_vec();
    let resolution_artifact_data = resolution_release.to_bytes().to_vec();
    let binding = PayoffBindingV1::new(
        instance_id,
        domain_id,
        payoff_digest,
        PAYOFF_PROGRAM_ID.to_bytes(),
        hash(&payoff_artifact_data).to_bytes(),
        RESOLUTION_PROGRAM_ID.to_bytes(),
        hash(&resolution_artifact_data).to_bytes(),
        ADMISSION_PROGRAM_ID.to_bytes(),
        hash(&admission_artifact_data).to_bytes(),
        81,
        70,
        11,
        100,
        13,
    )
    .expect("payoff binding");
    let binding_data = binding.to_bytes().to_vec();
    let binding_digest = hash(&binding_data).to_bytes();
    let entry = CapabilityEntryV1::new(
        core_id(PRODUCT_PAYOFF_ADMISSION_KIND_ID_V1),
        core_id(PRODUCT_PAYOFF_ADMISSION_RELEASE_ID_V1),
        core_id(binding_digest),
        core_id(capacity_id),
        core_id(PRODUCT_PAYOFF_ADMISSION_RECEIPT_SCHEMA_ID_V1),
        core_id(PRODUCT_PAYOFF_ADMISSION_RECEIPT_DERIVATION_ID_V1),
        ActivationPolicy::RequiredAtFounding,
        0,
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        zero_quote(),
    )
    .expect("capability entry");
    let mut manifest_data = vec![0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&[entry], &mut manifest_data).expect("manifest");
    let manifest_digest = hash(&manifest_data).to_bytes();
    let identity = MarketIdentity::new(
        core_id([47; 32]),
        core_id(instance_id),
        core_id(claim_basis_id),
        core_id([14; 32]),
        core_id(manifest_digest),
        GENERATION,
    );
    let founding_root = MarketRoot::founding(identity, [48; 32]).expect("founding root");
    let founding_market = market_bytes(founding_root);
    let mut open_root = founding_root;
    open_root
        .transition_phase(GENERATION, Phase::Open)
        .expect("open root");
    let open_market = market_bytes(open_root);
    let market_digest = hash(&identity.to_bytes()).to_bytes();
    let market =
        Pubkey::find_program_address(&[MARKET_PDA_DOMAIN_V1, &market_digest], &REGISTRY_ID).0;

    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    let payoff_programdata = add_upgradeable_program(
        &mut test,
        "dclutch_product_evidence_sbf",
        PAYOFF_PROGRAM_ID,
        &product_elf,
    );
    let admission_programdata = add_upgradeable_program(
        &mut test,
        "dclutch_product_evidence_sbf",
        ADMISSION_PROGRAM_ID,
        &product_elf,
    );
    let resolution_programdata = add_upgradeable_program(
        &mut test,
        "dclutch_resolution_proof_sbf",
        RESOLUTION_PROGRAM_ID,
        &resolution_elf,
    );
    let payoff = add_finalized_record(&mut test, PRODUCT_PAYOFF_SCHEMA_RELEASE_ID_V2, payoff_data);
    let domain = add_finalized_record(
        &mut test,
        FINITE_RESULT_DOMAIN_SCHEMA_RELEASE_ID_V1,
        domain_data,
    );
    let instance = add_finalized_record(
        &mut test,
        PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1,
        instance_data,
    );
    let binding =
        add_finalized_record(&mut test, PRODUCT_PAYOFF_BINDING_SCHEMA_ID_V1, binding_data);
    let manifest = add_finalized_record(
        &mut test,
        dclutch_capability_contract::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        manifest_data,
    );
    let payoff_artifact = add_finalized_record(
        &mut test,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        payoff_artifact_data,
    );
    let admission_artifact = add_finalized_record(
        &mut test,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        admission_artifact_data,
    );
    let resolution_artifact = add_finalized_record(
        &mut test,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        resolution_artifact_data,
    );
    test.add_account(
        market,
        Account {
            lamports: Rent::default().minimum_balance(founding_market.len()),
            data: founding_market.clone(),
            owner: REGISTRY_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    let context = test.start_with_context().await;
    Fixture {
        context,
        payoff,
        binding,
        manifest,
        instance,
        domain,
        payoff_artifact,
        admission_artifact,
        resolution_artifact,
        market,
        founding_market,
        open_market,
        payoff_programdata,
        admission_programdata,
        resolution_programdata,
        product_elf_digest: hash(&product_elf).to_bytes(),
        resolution_elf_digest: hash(&resolution_elf).to_bytes(),
    }
}

fn payoff_certificate_address(request: PayoffRequestV2) -> Pubkey {
    let role = [match request.kind() {
        CertificateKindV2::Evaluation => 0,
        CertificateKindV2::Liability => 1,
    }];
    let query = hashv(&[
        &request.result_numerator().to_le_bytes(),
        &request.result_denominator().to_le_bytes(),
        &request.available().to_le_bytes(),
    ])
    .to_bytes();
    Pubkey::find_program_address(
        &[
            PAYOFF_CERTIFICATE_PDA_DOMAIN_V2,
            REGISTRY_ID.as_ref(),
            &request.product_record_digest(),
            &request.artifact_release_digest(),
            &role,
            &query,
        ],
        &PAYOFF_PROGRAM_ID,
    )
    .0
}

fn admission_receipt_address(request: PayoffAdmissionRequestV1, market: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[
            PAYOFF_ADMISSION_RECEIPT_PDA_DOMAIN_V1,
            market.as_ref(),
            &request.expected_generation().to_le_bytes(),
            &[request.role().byte()],
            &request.binding_digest(),
            &request.payoff_certificate_digest(),
            &request.resolution_certificate_digest(),
        ],
        &ADMISSION_PROGRAM_ID,
    )
    .0
}

fn evaluator_instruction(
    fixture: &Fixture,
    payer: Pubkey,
    certificate: Pubkey,
    request: PayoffRequestV2,
) -> Instruction {
    Instruction {
        program_id: PAYOFF_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(certificate, false),
            AccountMeta::new_readonly(fixture.payoff.raw, false),
            AccountMeta::new_readonly(fixture.payoff.staging, false),
            AccountMeta::new_readonly(fixture.payoff_artifact.raw, false),
            AccountMeta::new_readonly(fixture.payoff_artifact.staging, false),
            AccountMeta::new_readonly(PAYOFF_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.payoff_programdata, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: request.to_bytes().to_vec(),
    }
}

fn admission_instruction(
    fixture: &Fixture,
    payer: Pubkey,
    payoff_certificate: Pubkey,
    receipt: Pubkey,
    request: PayoffAdmissionRequestV1,
) -> Instruction {
    Instruction {
        program_id: ADMISSION_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(receipt, false),
            AccountMeta::new_readonly(payoff_certificate, false),
            AccountMeta::new_readonly(fixture.market, false),
            AccountMeta::new_readonly(fixture.manifest.raw, false),
            AccountMeta::new_readonly(fixture.manifest.staging, false),
            AccountMeta::new_readonly(fixture.binding.raw, false),
            AccountMeta::new_readonly(fixture.binding.staging, false),
            AccountMeta::new_readonly(fixture.instance.raw, false),
            AccountMeta::new_readonly(fixture.instance.staging, false),
            AccountMeta::new_readonly(fixture.domain.raw, false),
            AccountMeta::new_readonly(fixture.domain.staging, false),
            AccountMeta::new_readonly(fixture.payoff.raw, false),
            AccountMeta::new_readonly(fixture.payoff.staging, false),
            AccountMeta::new_readonly(fixture.payoff_artifact.raw, false),
            AccountMeta::new_readonly(fixture.payoff_artifact.staging, false),
            AccountMeta::new_readonly(PAYOFF_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.payoff_programdata, false),
            AccountMeta::new_readonly(fixture.admission_artifact.raw, false),
            AccountMeta::new_readonly(fixture.admission_artifact.staging, false),
            AccountMeta::new_readonly(ADMISSION_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.admission_programdata, false),
            AccountMeta::new_readonly(fixture.resolution_artifact.raw, false),
            AccountMeta::new_readonly(fixture.resolution_artifact.staging, false),
            AccountMeta::new_readonly(RESOLUTION_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.resolution_programdata, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: request.to_bytes().to_vec(),
    }
}

async fn submit(
    context: &mut ProgramTestContext,
    instruction: Instruction,
) -> Result<u64, BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await?;
    processed.result?;
    processed
        .metadata
        .map(|metadata| metadata.compute_units_consumed)
        .ok_or(BanksClientError::ClientError(
            "missing transaction metadata",
        ))
}

async fn refused(context: &mut ProgramTestContext, instruction: Instruction) {
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
    assert!(
        context
            .banks_client
            .process_transaction(transaction)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn real_elf_joins_liability_evidence_to_market_admission_and_refuses_replay_attacks() {
    let mut fixture = fixture().await;
    let payer = fixture.context.payer.pubkey();
    let payoff_request =
        PayoffRequestV2::liability(fixture.payoff.digest, fixture.payoff_artifact.digest, 37)
            .expect("liability request");
    let payoff_certificate = payoff_certificate_address(payoff_request);
    let payoff_instruction =
        evaluator_instruction(&fixture, payer, payoff_certificate, payoff_request);
    let payoff_cu = submit(&mut fixture.context, payoff_instruction)
        .await
        .expect("real-ELF payoff evidence");
    let payoff_account = fixture
        .context
        .banks_client
        .get_account(payoff_certificate)
        .await
        .expect("query payoff")
        .expect("payoff certificate");
    let payoff = PayoffCertificateV2::decode(&payoff_account.data).expect("decode payoff");
    assert_eq!(payoff_account.owner, PAYOFF_PROGRAM_ID);
    assert_eq!(payoff_account.data.len(), PAYOFF_CERTIFICATE_BYTES_V2);
    assert!(payoff.collateralized());
    assert_eq!(payoff.liability_bound(), 37);

    let admission_request = PayoffAdmissionRequestV1::new(
        AdmissionRoleV1::Liability,
        GENERATION,
        fixture.binding.digest,
        hash(&payoff_account.data).to_bytes(),
        [0; 32],
    )
    .expect("admission request");
    let receipt = admission_receipt_address(admission_request, fixture.market);
    let canonical_admission_instruction = admission_instruction(
        &fixture,
        payer,
        payoff_certificate,
        receipt,
        admission_request,
    );
    let admission_cu = submit(&mut fixture.context, canonical_admission_instruction)
        .await
        .expect("real-ELF Market admission");
    let receipt_account = fixture
        .context
        .banks_client
        .get_account(receipt)
        .await
        .expect("query receipt")
        .expect("admission receipt");
    let admitted = PayoffAdmissionReceiptV1::decode(&receipt_account.data).expect("decode receipt");
    assert_eq!(receipt_account.owner, ADMISSION_PROGRAM_ID);
    assert_eq!(
        receipt_account.data.len(),
        PAYOFF_ADMISSION_RECEIPT_BYTES_V1
    );
    assert_eq!(admitted.role(), AdmissionRoleV1::Liability);
    assert_eq!(admitted.liability_bound(), 37);

    let replay_instruction = admission_instruction(
        &fixture,
        payer,
        payoff_certificate,
        receipt,
        admission_request,
    );
    let replay_cu = submit(&mut fixture.context, replay_instruction)
        .await
        .expect("exact idempotent replay");
    let replayed = fixture
        .context
        .banks_client
        .get_account(receipt)
        .await
        .expect("query replay")
        .expect("replayed receipt");
    assert_eq!(replayed, receipt_account);

    let underfunded_request =
        PayoffRequestV2::liability(fixture.payoff.digest, fixture.payoff_artifact.digest, 36)
            .expect("underfunded liability request");
    let underfunded_certificate = payoff_certificate_address(underfunded_request);
    let underfunded_payoff_instruction = evaluator_instruction(
        &fixture,
        payer,
        underfunded_certificate,
        underfunded_request,
    );
    submit(&mut fixture.context, underfunded_payoff_instruction)
        .await
        .expect("underfunded evidence is still exact evidence");
    let underfunded_payoff_account = fixture
        .context
        .banks_client
        .get_account(underfunded_certificate)
        .await
        .expect("query underfunded payoff")
        .expect("underfunded payoff certificate");
    let underfunded_payoff =
        PayoffCertificateV2::decode(&underfunded_payoff_account.data).expect("decode underfunded");
    assert!(!underfunded_payoff.collateralized());
    let underfunded_admission = PayoffAdmissionRequestV1::new(
        AdmissionRoleV1::Liability,
        GENERATION,
        fixture.binding.digest,
        hash(&underfunded_payoff_account.data).to_bytes(),
        [0; 32],
    )
    .expect("underfunded admission request");
    let underfunded_receipt = admission_receipt_address(underfunded_admission, fixture.market);
    let underfunded_admission_instruction = admission_instruction(
        &fixture,
        payer,
        underfunded_certificate,
        underfunded_receipt,
        underfunded_admission,
    );
    refused(&mut fixture.context, underfunded_admission_instruction).await;
    assert!(
        fixture
            .context
            .banks_client
            .get_account(underfunded_receipt)
            .await
            .expect("query underfunded receipt")
            .is_none()
    );

    fixture.context.set_account(
        &fixture.market,
        &AccountSharedData::from(Account {
            lamports: Rent::default().minimum_balance(fixture.open_market.len()),
            data: fixture.open_market.clone(),
            owner: REGISTRY_ID,
            executable: false,
            rent_epoch: 0,
        }),
    );
    let wrong_phase_instruction = admission_instruction(
        &fixture,
        payer,
        payoff_certificate,
        receipt,
        admission_request,
    );
    refused(&mut fixture.context, wrong_phase_instruction).await;
    let after_wrong_phase = fixture
        .context
        .banks_client
        .get_account(receipt)
        .await
        .expect("query rollback")
        .expect("receipt preserved");
    assert_eq!(after_wrong_phase, receipt_account);

    fixture.context.set_account(
        &fixture.market,
        &AccountSharedData::from(Account {
            lamports: Rent::default().minimum_balance(fixture.founding_market.len()),
            data: fixture.founding_market.clone(),
            owner: REGISTRY_ID,
            executable: false,
            rent_epoch: 0,
        }),
    );
    let substituted = PayoffAdmissionRequestV1::new(
        AdmissionRoleV1::Liability,
        GENERATION,
        [99; 32],
        admission_request.payoff_certificate_digest(),
        [0; 32],
    )
    .expect("hostile binding substitution");
    let substituted_receipt = admission_receipt_address(substituted, fixture.market);
    let substitution_instruction = admission_instruction(
        &fixture,
        payer,
        payoff_certificate,
        substituted_receipt,
        substituted,
    );
    refused(&mut fixture.context, substitution_instruction).await;
    assert!(
        fixture
            .context
            .banks_client
            .get_account(substituted_receipt)
            .await
            .expect("query hostile receipt")
            .is_none()
    );

    eprintln!(
        "product V2 admission evidence: product_elf={} resolution_elf={} payoff_certificate={} receipt={} payoff_cu={} admission_cu={} replay_cu={}",
        hex(fixture.product_elf_digest),
        hex(fixture.resolution_elf_digest),
        hex(hash(&payoff_account.data).to_bytes()),
        hex(hash(&receipt_account.data).to_bytes()),
        payoff_cu,
        admission_cu,
        replay_cu,
    );
}

fn hex(bytes: [u8; 32]) -> String {
    use core::fmt::Write;
    let mut output = String::with_capacity(64);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("String write");
    }
    output
}
