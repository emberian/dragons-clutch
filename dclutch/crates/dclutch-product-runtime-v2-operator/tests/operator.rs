//! Caller-buffer compilation and chain-derived unsigned instruction tests.

use dclutch_product_runtime_v2::{ContentId, portfolio_record_bytes, result_domain_record_bytes};
use dclutch_product_runtime_v2_admission::{ADMISSION_RECEIPT_BYTES_V2, PRODUCT_RECORD_BYTES_V2};
use dclutch_product_runtime_v2_operator::*;
use solana_program::pubkey::Pubkey;
use solana_sdk_ids::{system_program, sysvar};

fn id(byte: u8) -> ContentId {
    ContentId::new([byte; 32]).expect("identity")
}
fn account<'a>(
    slot: u64,
    key: Pubkey,
    owner: Pubkey,
    executable: bool,
    lamports: u64,
    data: &'a [u8],
) -> AccountObservationV2<'a> {
    AccountObservationV2 {
        slot,
        key,
        owner,
        lamports,
        executable,
        data,
    }
}

#[test]
fn compiler_derives_every_child_digest_and_runtime_width() {
    let registry = Pubkey::new_from_array([70; 32]);
    let cuts: Vec<i128> = (-140_i128..140).collect();
    let coefficients = vec![1_u64; 282];
    let mut product = [0_u8; PRODUCT_RECORD_BYTES_V2];
    let mut domain = vec![0_u8; result_domain_record_bytes(cuts.len()).expect("domain")];
    let mut portfolio = vec![0_u8; portfolio_record_bytes(coefficients.len()).expect("portfolio")];
    let report = compile_product_records_v2(
        registry,
        ProductCompilationInputV2 {
            product_id: id(1),
            coordinate_domain_id: id(2),
            result_unit_id: id(3),
            claim_basis_id: id(4),
            liability_basis_id: id(5),
            representation_release_id: id(6),
            mapping_release_id: id(7),
            cut_denominator: 10,
            cuts: &cuts,
            portfolio_denominator: 3,
            coefficients: &coefficients,
        },
        &mut product,
        &mut domain,
        &mut portfolio,
    )
    .expect("compile");
    assert_eq!(report.outcome_count, 282);
    assert_ne!(
        report.request.product_digest,
        report.request.result_domain_digest
    );
    assert_ne!(
        report.request.result_domain_digest,
        report.request.portfolio_digest
    );
}

#[test]
fn late_portfolio_refusal_preserves_all_three_caller_buffers() {
    let registry = Pubkey::new_from_array([70; 32]);
    let cuts = [0_i128];
    let coefficients = [1_u64, 1, 1];
    let mut product = [0xa5_u8; PRODUCT_RECORD_BYTES_V2];
    let mut domain = vec![0xb6_u8; result_domain_record_bytes(cuts.len()).expect("domain")];
    let mut portfolio =
        vec![0xc7_u8; portfolio_record_bytes(coefficients.len()).expect("portfolio")];
    let product_before = product;
    let domain_before = domain.clone();
    let portfolio_before = portfolio.clone();
    assert_eq!(
        compile_product_records_v2(
            registry,
            ProductCompilationInputV2 {
                product_id: id(1),
                coordinate_domain_id: id(2),
                result_unit_id: id(3),
                claim_basis_id: id(4),
                liability_basis_id: id(5),
                representation_release_id: id(6),
                mapping_release_id: id(7),
                cut_denominator: 1,
                cuts: &cuts,
                portfolio_denominator: 0,
                coefficients: &coefficients,
            },
            &mut product,
            &mut domain,
            &mut portfolio,
        ),
        Err(Error::RuntimeProduct)
    );
    assert_eq!(product, product_before);
    assert_eq!(domain, domain_before);
    assert_eq!(portfolio, portfolio_before);
}

#[test]
fn finalized_observations_build_one_unsigned_admission_frame() {
    let registry_key = Pubkey::new_from_array([70; 32]);
    let admission_program = Pubkey::new_from_array([71; 32]);
    let cuts = [0_i128];
    let coefficients = [1_u64, 1, 1];
    let mut product = [0_u8; PRODUCT_RECORD_BYTES_V2];
    let mut domain = vec![0_u8; result_domain_record_bytes(1).expect("domain")];
    let mut portfolio = vec![0_u8; portfolio_record_bytes(3).expect("portfolio")];
    let report = compile_product_records_v2(
        registry_key,
        ProductCompilationInputV2 {
            product_id: id(1),
            coordinate_domain_id: id(2),
            result_unit_id: id(3),
            claim_basis_id: id(4),
            liability_basis_id: id(5),
            representation_release_id: id(6),
            mapping_release_id: id(7),
            cut_denominator: 1,
            cuts: &cuts,
            portfolio_denominator: 1,
            coefficients: &coefficients,
        },
        &mut product,
        &mut domain,
        &mut portfolio,
    )
    .expect("compile");
    let receipt_key = derive_admission_receipt_v2(admission_program, report.request);
    let receipt_data = [0_u8; ADMISSION_RECEIPT_BYTES_V2];
    let empty: [u8; 0] = [];
    let slot = 99;
    let raw = |coordinate: dclutch_product_runtime_v2_admission::FinalizedRecordCoordinateV2,
               data| FinalizedRecordObservationV2 {
        raw: account(
            slot,
            Pubkey::new_from_array(coordinate.raw_account.to_bytes()),
            registry_key,
            false,
            1_000_000,
            data,
        ),
        staging: account(
            slot,
            Pubkey::new_from_array(coordinate.staging_account.to_bytes()),
            system_program::ID,
            false,
            7,
            &empty,
        ),
        raw_rent_minimum: 100,
    };
    let state = AdmissionStateV2 {
        registry: account(
            slot,
            registry_key,
            Pubkey::new_from_array([80; 32]),
            true,
            1,
            &empty,
        ),
        receipt_output: account(
            slot,
            receipt_key,
            admission_program,
            false,
            1_000_000,
            &receipt_data,
        ),
        rent: account(slot, sysvar::rent::ID, sysvar::ID, false, 1, &empty),
        product: raw(report.receipt.product, &product),
        result_domain: raw(report.receipt.result_domain, &domain),
        portfolio: raw(report.receipt.portfolio, &portfolio),
    };
    let plan = build_admission_instruction_v2(admission_program, report, state).expect("plan");
    assert_eq!(plan.instruction.accounts.len(), 9);
    assert_eq!(plan.instruction.data.len(), 112);
    assert_eq!(plan.receipt_bytes.len(), ADMISSION_RECEIPT_BYTES_V2);
    assert!(plan.instruction.accounts.iter().all(|meta| !meta.is_signer));

    let stale = AdmissionStateV2 {
        receipt_output: AccountObservationV2 {
            slot: slot + 1,
            ..state.receipt_output
        },
        ..state
    };
    assert_eq!(
        build_admission_instruction_v2(admission_program, report, stale),
        Err(Error::ObservationMismatch)
    );
}
