//! Real-ELF ProgramTest campaign for the authenticated Product payoff adapter.

use std::{env, fs, path::PathBuf};

use dclutch_core_contract::ContentId;
use dclutch_product_payoff_svm::{
    CertificateKindV1, PAYOFF_CERTIFICATE_BYTES_V1, PAYOFF_CERTIFICATE_PDA_DOMAIN_V1,
    PRODUCT_PAYOFF_ADAPTER_RELEASE_ID_V1, PRODUCT_PAYOFF_SCHEMA_RELEASE_ID_V1, PayoffCertificateV1,
    PayoffRequestV1,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactReleaseV1, ArtifactUpgradePolicyV1,
};
use dclutch_release_set_contract::ProgramIdentityV1;
use solana_account::{Account, AccountSharedData};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::Signer;
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_transaction::Transaction;

const PROGRAM_ID: Pubkey = Pubkey::new_from_array([84_u8; 32]);
const REGISTRY_ID: Pubkey = Pubkey::new_from_array([85_u8; 32]);

struct FinalizedRecord {
    raw: Pubkey,
    staging: Pubkey,
    digest: [u8; 32],
}

struct Fixture {
    context: ProgramTestContext,
    product: FinalizedRecord,
    malformed_product: FinalizedRecord,
    artifact: FinalizedRecord,
    false_artifact: FinalizedRecord,
    programdata: Pubkey,
    elf_digest: [u8; 32],
}

fn require_sbf() -> PathBuf {
    let output = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    let artifact = output.join("dclutch_product_payoff_sbf.so");
    assert!(artifact.is_file(), "missing real Product payoff ELF");
    artifact
}

fn product_bytes() -> Vec<u8> {
    let mut bytes = vec![0_u8; 432];
    bytes[0..8].copy_from_slice(b"DCLTPAY1");
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10] = 5;
    bytes[11] = 4;
    for (offset, value) in [(16, 8101_u64), (24, 7001), (32, 9), (40, 100)] {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
    for (index, knot) in [0_u64, 25, 50, 75, 100].into_iter().enumerate() {
        let offset = 48 + index * 8;
        bytes[offset..offset + 8].copy_from_slice(&knot.to_le_bytes());
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
        let offset = 176 + index * 16;
        bytes[offset..offset + 4].copy_from_slice(&[tag, left, peak, right]);
        bytes[offset + 8..offset + 16].copy_from_slice(&amplitude.to_le_bytes());
    }
    bytes
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

fn immutable_programdata(elf: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0_u8; 45 + elf.len()];
    bytes[0..4].copy_from_slice(&3_u32.to_le_bytes());
    bytes[4..12].copy_from_slice(&0_u64.to_le_bytes());
    bytes[12] = 0;
    bytes[45..].copy_from_slice(elf);
    bytes
}

async fn fixture() -> Fixture {
    let elf_path = require_sbf();
    let elf = fs::read(elf_path).expect("read Product payoff ELF");
    let elf_digest = hash(&elf).to_bytes();
    let programdata =
        Pubkey::find_program_address(&[PROGRAM_ID.as_ref()], &bpf_loader_upgradeable::ID).0;
    let release = ArtifactReleaseV1::new(
        ProgramIdentityV1::new(PROGRAM_ID.to_bytes()).expect("program"),
        ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("loader"),
        programdata.to_bytes(),
        ContentId::new(PRODUCT_PAYOFF_ADAPTER_RELEASE_ID_V1).expect("semantic release"),
        elf_digest,
        0,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("artifact release");
    let false_release = ArtifactReleaseV1::new(
        ProgramIdentityV1::new(PROGRAM_ID.to_bytes()).expect("program"),
        ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("loader"),
        programdata.to_bytes(),
        ContentId::new(PRODUCT_PAYOFF_ADAPTER_RELEASE_ID_V1).expect("semantic release"),
        [7_u8; 32],
        0,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("false artifact release remains structurally canonical");

    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    test.add_upgradeable_program_to_genesis("dclutch_product_payoff_sbf", &PROGRAM_ID);
    let programdata_bytes = immutable_programdata(&elf);
    test.add_account(
        programdata,
        Account {
            lamports: Rent::default().minimum_balance(programdata_bytes.len()),
            data: programdata_bytes,
            owner: bpf_loader_upgradeable::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    let product = add_finalized_record(
        &mut test,
        PRODUCT_PAYOFF_SCHEMA_RELEASE_ID_V1,
        product_bytes(),
    );
    let mut malformed = product_bytes();
    malformed[96] = 1;
    let malformed_product =
        add_finalized_record(&mut test, PRODUCT_PAYOFF_SCHEMA_RELEASE_ID_V1, malformed);
    let artifact = add_finalized_record(
        &mut test,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        release.to_bytes().to_vec(),
    );
    let false_artifact = add_finalized_record(
        &mut test,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        false_release.to_bytes().to_vec(),
    );
    let context = test.start_with_context().await;
    Fixture {
        context,
        product,
        malformed_product,
        artifact,
        false_artifact,
        programdata,
        elf_digest,
    }
}

fn certificate_address(request: PayoffRequestV1) -> Pubkey {
    let kind = [match request.kind() {
        CertificateKindV1::Evaluation => 0,
        CertificateKindV1::Liability => 1,
    }];
    Pubkey::find_program_address(
        &[
            PAYOFF_CERTIFICATE_PDA_DOMAIN_V1,
            REGISTRY_ID.as_ref(),
            &request.product_record_digest(),
            &request.artifact_release_digest(),
            &kind,
            &request.query().to_le_bytes(),
        ],
        &PROGRAM_ID,
    )
    .0
}

fn instruction(
    payer: Pubkey,
    certificate: Pubkey,
    product: &FinalizedRecord,
    artifact: &FinalizedRecord,
    programdata: Pubkey,
    data: Vec<u8>,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(certificate, false),
            AccountMeta::new_readonly(product.raw, false),
            AccountMeta::new_readonly(product.staging, false),
            AccountMeta::new_readonly(artifact.raw, false),
            AccountMeta::new_readonly(artifact.staging, false),
            AccountMeta::new_readonly(PROGRAM_ID, false),
            AccountMeta::new_readonly(programdata, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data,
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
async fn real_elf_emits_exact_evaluation_and_liability_certificates() {
    let mut fixture = fixture().await;
    let payer = fixture.context.payer.pubkey();
    let evaluation = PayoffRequestV1::new(
        CertificateKindV1::Evaluation,
        fixture.product.digest,
        fixture.artifact.digest,
        37,
    )
    .expect("evaluation request");
    let evaluation_address = certificate_address(evaluation);
    let evaluation_cu = submit(
        &mut fixture.context,
        instruction(
            payer,
            evaluation_address,
            &fixture.product,
            &fixture.artifact,
            fixture.programdata,
            evaluation.to_bytes().to_vec(),
        ),
    )
    .await
    .expect("evaluation transaction");
    let evaluation_account = fixture
        .context
        .banks_client
        .get_account(evaluation_address)
        .await
        .expect("query evaluation")
        .expect("evaluation certificate");
    let expected_evaluation = PayoffCertificateV1::evaluation(
        REGISTRY_ID.to_bytes(),
        fixture.product.digest,
        fixture.artifact.digest,
        8101,
        7001,
        9,
        100,
        37,
        17,
        37,
    )
    .expect("exact evaluation");
    assert_eq!(evaluation_account.owner, PROGRAM_ID);
    assert_eq!(evaluation_account.data, expected_evaluation.to_bytes());
    assert_eq!(evaluation_account.data.len(), PAYOFF_CERTIFICATE_BYTES_V1);
    let idempotent_cu = submit(
        &mut fixture.context,
        instruction(
            payer,
            evaluation_address,
            &fixture.product,
            &fixture.artifact,
            fixture.programdata,
            evaluation.to_bytes().to_vec(),
        ),
    )
    .await
    .expect("idempotent evaluation transaction");
    let repeated_evaluation = fixture
        .context
        .banks_client
        .get_account(evaluation_address)
        .await
        .expect("query repeated evaluation")
        .expect("repeated evaluation certificate");
    assert_eq!(repeated_evaluation, evaluation_account);

    let liability = PayoffRequestV1::new(
        CertificateKindV1::Liability,
        fixture.product.digest,
        fixture.artifact.digest,
        36,
    )
    .expect("liability request");
    let liability_address = certificate_address(liability);
    let liability_cu = submit(
        &mut fixture.context,
        instruction(
            payer,
            liability_address,
            &fixture.product,
            &fixture.artifact,
            fixture.programdata,
            liability.to_bytes().to_vec(),
        ),
    )
    .await
    .expect("liability transaction");
    let liability_account = fixture
        .context
        .banks_client
        .get_account(liability_address)
        .await
        .expect("query liability")
        .expect("liability certificate");
    let expected_liability = PayoffCertificateV1::liability(
        REGISTRY_ID.to_bytes(),
        fixture.product.digest,
        fixture.artifact.digest,
        8101,
        7001,
        9,
        100,
        36,
        37,
    )
    .expect("exact liability");
    assert_eq!(liability_account.data, expected_liability.to_bytes());
    assert!(!expected_liability.collateralized());
    eprintln!(
        "product payoff exact output: elf={} evaluation_certificate={} liability_certificate={} evaluation_cu={} idempotent_cu={} liability_cu={}",
        hex(fixture.elf_digest),
        hex(hash(&evaluation_account.data).to_bytes()),
        hex(hash(&liability_account.data).to_bytes()),
        evaluation_cu,
        idempotent_cu,
        liability_cu,
    );
}

#[tokio::test]
async fn real_elf_refuses_hostile_records_queries_and_wires_with_rollback() {
    let mut fixture = fixture().await;
    let payer = fixture.context.payer.pubkey();
    let canonical = PayoffRequestV1::new(
        CertificateKindV1::Evaluation,
        fixture.product.digest,
        fixture.artifact.digest,
        37,
    )
    .expect("canonical request");
    let canonical_address = certificate_address(canonical);
    submit(
        &mut fixture.context,
        instruction(
            payer,
            canonical_address,
            &fixture.product,
            &fixture.artifact,
            fixture.programdata,
            canonical.to_bytes().to_vec(),
        ),
    )
    .await
    .expect("canonical certificate");
    let before = fixture
        .context
        .banks_client
        .get_account(canonical_address)
        .await
        .expect("query before")
        .expect("certificate before");

    let mut truncated = canonical.to_bytes().to_vec();
    truncated.pop();
    refused(
        &mut fixture.context,
        instruction(
            payer,
            canonical_address,
            &fixture.product,
            &fixture.artifact,
            fixture.programdata,
            truncated,
        ),
    )
    .await;
    let after = fixture
        .context
        .banks_client
        .get_account(canonical_address)
        .await
        .expect("query after")
        .expect("certificate after");
    assert_eq!(after, before, "wire refusal preserves exact certificate");

    let mut occupied = before.clone();
    *occupied
        .data
        .get_mut(PAYOFF_CERTIFICATE_BYTES_V1 - 1)
        .expect("certificate last byte") = 1;
    fixture.context.set_account(
        &canonical_address,
        &AccountSharedData::from(occupied.clone()),
    );
    refused(
        &mut fixture.context,
        instruction(
            payer,
            canonical_address,
            &fixture.product,
            &fixture.artifact,
            fixture.programdata,
            canonical.to_bytes().to_vec(),
        ),
    )
    .await;
    let occupied_after = fixture
        .context
        .banks_client
        .get_account(canonical_address)
        .await
        .expect("query occupied certificate")
        .expect("occupied certificate remains");
    assert_eq!(occupied_after, occupied);

    let out_of_domain = PayoffRequestV1::new(
        CertificateKindV1::Evaluation,
        fixture.product.digest,
        fixture.artifact.digest,
        101,
    )
    .expect("out-of-domain request wire");
    let out_of_domain_address = certificate_address(out_of_domain);
    refused(
        &mut fixture.context,
        instruction(
            payer,
            out_of_domain_address,
            &fixture.product,
            &fixture.artifact,
            fixture.programdata,
            out_of_domain.to_bytes().to_vec(),
        ),
    )
    .await;
    assert!(
        fixture
            .context
            .banks_client
            .get_account(out_of_domain_address)
            .await
            .expect("query OOD")
            .is_none(),
        "evaluation refusal creates no certificate"
    );

    let malformed = PayoffRequestV1::new(
        CertificateKindV1::Evaluation,
        fixture.malformed_product.digest,
        fixture.artifact.digest,
        37,
    )
    .expect("malformed Product request");
    let malformed_address = certificate_address(malformed);
    refused(
        &mut fixture.context,
        instruction(
            payer,
            malformed_address,
            &fixture.malformed_product,
            &fixture.artifact,
            fixture.programdata,
            malformed.to_bytes().to_vec(),
        ),
    )
    .await;
    assert!(
        fixture
            .context
            .banks_client
            .get_account(malformed_address)
            .await
            .expect("query malformed")
            .is_none()
    );

    let false_release = PayoffRequestV1::new(
        CertificateKindV1::Evaluation,
        fixture.product.digest,
        fixture.false_artifact.digest,
        37,
    )
    .expect("false release request");
    let false_release_address = certificate_address(false_release);
    refused(
        &mut fixture.context,
        instruction(
            payer,
            false_release_address,
            &fixture.product,
            &fixture.false_artifact,
            fixture.programdata,
            false_release.to_bytes().to_vec(),
        ),
    )
    .await;
    assert!(
        fixture
            .context
            .banks_client
            .get_account(false_release_address)
            .await
            .expect("query false release")
            .is_none()
    );

    let live_staging = AccountSharedData::from(Account {
        lamports: 1,
        data: Vec::new(),
        owner: system_program::ID,
        executable: false,
        rent_epoch: 0,
    });
    fixture
        .context
        .set_account(&fixture.product.staging, &live_staging);
    let live = PayoffRequestV1::new(
        CertificateKindV1::Liability,
        fixture.product.digest,
        fixture.artifact.digest,
        37,
    )
    .expect("live-staging request");
    let live_address = certificate_address(live);
    refused(
        &mut fixture.context,
        instruction(
            payer,
            live_address,
            &fixture.product,
            &fixture.artifact,
            fixture.programdata,
            live.to_bytes().to_vec(),
        ),
    )
    .await;
    assert!(
        fixture
            .context
            .banks_client
            .get_account(live_address)
            .await
            .expect("query live staging")
            .is_none()
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
