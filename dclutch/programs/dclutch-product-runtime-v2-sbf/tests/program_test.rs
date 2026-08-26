//! Native ProgramTest evidence for runtime-width Product admission.

use dclutch_product_runtime_v2::{ContentId, portfolio_record_bytes, result_domain_record_bytes};
use dclutch_product_runtime_v2_admission::{
    ADMISSION_RECEIPT_BYTES_V2, AdmissionReceiptV2, PRODUCT_RECORD_BYTES_V2,
};
use dclutch_product_runtime_v2_operator::{
    AccountObservationV2, AdmissionStateV2, FinalizedRecordObservationV2,
    ProductCompilationInputV2, build_admission_instruction_v2, compile_product_records_v2,
    derive_admission_receipt_v2,
};
use dclutch_product_runtime_v2_sbf::process_instruction;
use solana_account::Account;
use solana_program::{pubkey::Pubkey, rent::Rent};
use solana_program_test::{ProgramTest, ProgramTestContext, processor};
use solana_sdk::signature::Signer;
use solana_sdk_ids::{native_loader, system_program, sysvar};
use solana_transaction::Transaction;

const ADMISSION_PROGRAM: Pubkey = Pubkey::new_from_array([0xa1; 32]);
const REGISTRY_PROGRAM: Pubkey = Pubkey::new_from_array([0xa2; 32]);
const SLOT: u64 = 77;

#[derive(Clone)]
struct RecordFixture {
    bytes: Vec<u8>,
    raw: Pubkey,
    staging: Pubkey,
}

struct Fixture {
    test: ProgramTest,
    instruction: solana_program::instruction::Instruction,
    receipt: Pubkey,
    expected_receipt: [u8; ADMISSION_RECEIPT_BYTES_V2],
    hostile_product: RecordFixture,
    hostile_domain: RecordFixture,
}

fn id(byte: u8) -> ContentId {
    ContentId::new([byte; 32]).expect("nonzero fixture identity")
}

fn add_account(test: &mut ProgramTest, key: Pubkey, owner: Pubkey, lamports: u64, data: Vec<u8>) {
    test.add_account(
        key,
        Account {
            lamports,
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn record(
    test: &mut ProgramTest,
    coordinate: dclutch_product_runtime_v2_admission::FinalizedRecordCoordinateV2,
    bytes: Vec<u8>,
) -> RecordFixture {
    let raw = Pubkey::new_from_array(coordinate.raw_account.to_bytes());
    let staging = Pubkey::new_from_array(coordinate.staging_account.to_bytes());
    add_account(
        test,
        raw,
        REGISTRY_PROGRAM,
        Rent::default().minimum_balance(bytes.len()),
        bytes.clone(),
    );
    add_account(test, staging, system_program::ID, 7, Vec::new());
    RecordFixture {
        bytes,
        raw,
        staging,
    }
}

fn observe<'a>(
    coordinate: dclutch_product_runtime_v2_admission::FinalizedRecordCoordinateV2,
    bytes: &'a [u8],
) -> FinalizedRecordObservationV2<'a> {
    FinalizedRecordObservationV2 {
        raw: AccountObservationV2 {
            slot: SLOT,
            key: Pubkey::new_from_array(coordinate.raw_account.to_bytes()),
            owner: REGISTRY_PROGRAM,
            lamports: Rent::default().minimum_balance(bytes.len()),
            executable: false,
            data: bytes,
        },
        staging: AccountObservationV2 {
            slot: SLOT,
            key: Pubkey::new_from_array(coordinate.staging_account.to_bytes()),
            owner: system_program::ID,
            lamports: 7,
            executable: false,
            data: &[],
        },
        raw_rent_minimum: Rent::default().minimum_balance(bytes.len()),
    }
}

fn compile(
    product_id: ContentId,
    cuts: &[i128],
    coefficients: &[u64],
) -> (
    dclutch_product_runtime_v2_operator::CompiledProductRecordsV2,
    Vec<u8>,
    Vec<u8>,
    Vec<u8>,
) {
    let mut product = vec![0_u8; PRODUCT_RECORD_BYTES_V2];
    let mut domain =
        vec![0_u8; result_domain_record_bytes(cuts.len()).expect("domain record width")];
    let mut portfolio =
        vec![0_u8; portfolio_record_bytes(coefficients.len()).expect("portfolio record width")];
    let compiled = compile_product_records_v2(
        REGISTRY_PROGRAM,
        ProductCompilationInputV2 {
            product_id,
            coordinate_domain_id: id(2),
            result_unit_id: id(3),
            claim_basis_id: id(4),
            liability_basis_id: id(5),
            representation_release_id: id(6),
            mapping_release_id: id(7),
            cut_denominator: 10,
            cuts,
            portfolio_denominator: 3,
            coefficients,
        },
        &mut product,
        &mut domain,
        &mut portfolio,
    )
    .expect("compile Product records");
    (compiled, product, domain, portfolio)
}

fn fixture(prefer_real_elf: bool) -> Fixture {
    let cuts: Vec<i128> = (-128_i128..128).collect();
    let coefficients = vec![1_u64; 258];
    let (compiled, product_bytes, domain_bytes, portfolio_bytes) =
        compile(id(1), &cuts, &coefficients);
    assert_eq!(compiled.outcome_count, 258);

    let mut test = if prefer_real_elf {
        ProgramTest::new("dclutch_product_runtime_v2_sbf", ADMISSION_PROGRAM, None)
    } else {
        ProgramTest::new(
            "dclutch_product_runtime_v2_sbf",
            ADMISSION_PROGRAM,
            processor!(process_instruction),
        )
    };
    if prefer_real_elf {
        test.prefer_bpf(true);
    }
    test.add_account(
        REGISTRY_PROGRAM,
        Account {
            lamports: 1,
            data: Vec::new(),
            owner: native_loader::ID,
            executable: true,
            rent_epoch: 0,
        },
    );
    let product = record(&mut test, compiled.receipt.product, product_bytes);
    let domain = record(&mut test, compiled.receipt.result_domain, domain_bytes);
    let portfolio = record(&mut test, compiled.receipt.portfolio, portfolio_bytes);
    let receipt = derive_admission_receipt_v2(ADMISSION_PROGRAM, compiled.request);
    let receipt_rent = Rent::default().minimum_balance(ADMISSION_RECEIPT_BYTES_V2);
    add_account(
        &mut test,
        receipt,
        ADMISSION_PROGRAM,
        receipt_rent,
        vec![0; ADMISSION_RECEIPT_BYTES_V2],
    );

    let state = AdmissionStateV2 {
        registry: AccountObservationV2 {
            slot: SLOT,
            key: REGISTRY_PROGRAM,
            owner: native_loader::ID,
            lamports: 1,
            executable: true,
            data: &[],
        },
        receipt_output: AccountObservationV2 {
            slot: SLOT,
            key: receipt,
            owner: ADMISSION_PROGRAM,
            lamports: receipt_rent,
            executable: false,
            data: &[0; ADMISSION_RECEIPT_BYTES_V2],
        },
        rent: AccountObservationV2 {
            slot: SLOT,
            key: sysvar::rent::ID,
            owner: sysvar::ID,
            lamports: 1,
            executable: false,
            data: &[],
        },
        product: observe(compiled.receipt.product, &product.bytes),
        result_domain: observe(compiled.receipt.result_domain, &domain.bytes),
        portfolio: observe(compiled.receipt.portfolio, &portfolio.bytes),
    };
    let plan = build_admission_instruction_v2(ADMISSION_PROGRAM, compiled, state)
        .expect("chain-derived unsigned admission");

    let hostile_cuts: Vec<i128> = (-127_i128..129).collect();
    let (hostile_compiled, hostile_product_bytes, hostile_domain_bytes, _) =
        compile(id(1), &hostile_cuts, &coefficients);
    assert_eq!(hostile_compiled.outcome_count, compiled.outcome_count);
    let hostile_product = record(
        &mut test,
        hostile_compiled.receipt.product,
        hostile_product_bytes,
    );
    let hostile_domain = record(
        &mut test,
        hostile_compiled.receipt.result_domain,
        hostile_domain_bytes,
    );
    Fixture {
        test,
        instruction: plan.instruction,
        receipt,
        expected_receipt: plan.receipt_bytes,
        hostile_product,
        hostile_domain,
    }
}

async fn submit(
    context: &mut ProgramTestContext,
    instruction: solana_program::instruction::Instruction,
) -> Result<(), solana_program_test::BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    context.banks_client.process_transaction(transaction).await
}

async fn receipt_bytes(context: &mut ProgramTestContext, key: Pubkey) -> Vec<u8> {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("receipt lookup")
        .expect("receipt account")
        .data
}

async fn run_runtime_width_campaign(fixture: Fixture) {
    let mut context = fixture.test.start_with_context().await;

    let mut domain_substitution = fixture.instruction.clone();
    let domain_raw_meta = domain_substitution
        .accounts
        .get_mut(4)
        .expect("domain raw meta");
    domain_raw_meta.pubkey = fixture.hostile_domain.raw;
    let domain_staging_meta = domain_substitution
        .accounts
        .get_mut(5)
        .expect("domain staging meta");
    domain_staging_meta.pubkey = fixture.hostile_domain.staging;
    assert!(submit(&mut context, domain_substitution).await.is_err());
    assert!(
        receipt_bytes(&mut context, fixture.receipt)
            .await
            .iter()
            .all(|byte| *byte == 0),
        "late refusal must roll the deliberate receipt mutation back"
    );

    let mut product_substitution = fixture.instruction.clone();
    let product_raw_meta = product_substitution
        .accounts
        .get_mut(2)
        .expect("Product raw meta");
    product_raw_meta.pubkey = fixture.hostile_product.raw;
    let product_staging_meta = product_substitution
        .accounts
        .get_mut(3)
        .expect("Product staging meta");
    product_staging_meta.pubkey = fixture.hostile_product.staging;
    assert!(submit(&mut context, product_substitution).await.is_err());
    assert!(
        receipt_bytes(&mut context, fixture.receipt)
            .await
            .iter()
            .all(|byte| *byte == 0)
    );

    submit(&mut context, fixture.instruction)
        .await
        .expect("valid runtime-width admission commits");
    let committed = receipt_bytes(&mut context, fixture.receipt).await;
    assert_eq!(committed, fixture.expected_receipt);
    let admitted = AdmissionReceiptV2::decode(&committed).expect("reference-only receipt");
    assert_ne!(
        admitted.product.content_digest,
        admitted.result_domain.content_digest
    );
}

#[tokio::test]
async fn admits_258_outcomes_and_rolls_back_same_width_substitutions() {
    run_runtime_width_campaign(fixture(false)).await;
}

#[tokio::test]
#[ignore = "requires cargo-build-sbf output via SBF_OUT_DIR"]
async fn real_elf_admits_258_outcomes_and_preserves_late_rollback() {
    run_runtime_width_campaign(fixture(true)).await;
}
