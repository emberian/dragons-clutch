//! Reference-only Product admission agreement and substitution tests.

use dclutch_product_runtime_v2::{
    ContentId, PortfolioInputV2, ResultDomainInputV2, compile_portfolio_v2,
    compile_result_domain_v2, portfolio_record_bytes, result_domain_record_bytes,
};
use dclutch_product_runtime_v2_admission::*;

fn id(byte: u8) -> ContentId {
    ContentId::new([byte; 32]).expect("identity")
}

fn coordinate(schema: [u8; 32], digest: u8, raw: u8, staging: u8) -> FinalizedRecordCoordinateV2 {
    FinalizedRecordCoordinateV2 {
        schema_id: ContentId::new(schema).expect("schema"),
        content_digest: id(digest),
        raw_account: id(raw),
        staging_account: id(staging),
    }
}

#[test]
fn receipt_contains_only_record_coordinates_and_admission_joins_exact_ids() {
    let cuts: Vec<i128> = (-130_i128..130).collect();
    let mut domain = vec![0_u8; result_domain_record_bytes(cuts.len()).expect("domain width")];
    compile_result_domain_v2(
        ResultDomainInputV2 {
            product_id: id(1),
            coordinate_domain_id: id(2),
            result_unit_id: id(3),
            liability_basis_id: id(4),
            representation_release_id: id(5),
            mapping_release_id: id(6),
            cut_denominator: 10,
            cuts: &cuts,
        },
        &mut domain,
    )
    .expect("domain");
    let coefficients = vec![1_u64; 262];
    let mut portfolio =
        vec![0_u8; portfolio_record_bytes(coefficients.len()).expect("portfolio width")];
    compile_portfolio_v2(
        PortfolioInputV2 {
            product_id: id(1),
            result_domain_id: id(7),
            claim_basis_id: id(8),
            liability_basis_id: id(4),
            representation_release_id: id(5),
            denominator: 3,
            coefficients: &coefficients,
        },
        &mut portfolio,
    )
    .expect("portfolio");
    let product = ProductRecordV2::new(id(1), id(7), id(9));
    let mut product_bytes = [0_u8; PRODUCT_RECORD_BYTES_V2];
    product.encode_into(&mut product_bytes).expect("product");
    let receipt = AdmissionReceiptV2 {
        product: coordinate(PRODUCT_RECORD_SCHEMA_ID_V2, 10, 11, 12),
        result_domain: coordinate(RESULT_DOMAIN_SCHEMA_ID_V2, 7, 13, 14),
        portfolio: coordinate(PORTFOLIO_SCHEMA_ID_V2, 9, 15, 16),
    };
    let mut receipt_bytes = [0_u8; ADMISSION_RECEIPT_BYTES_V2];
    receipt.encode_into(&mut receipt_bytes).expect("receipt");
    let decoded = AdmissionReceiptV2::decode(&receipt_bytes).expect("receipt decode");
    let projection = admit_authenticated_records_v2(decoded, &product_bytes, &domain, &portfolio)
        .expect("admission");
    assert_eq!(projection.join.outcome_count, 262);
    assert_eq!(projection.join.liability_basis_id, id(4));
    assert_eq!(projection.product_record_digest, id(10));
    assert_eq!(projection.portfolio_record_digest, id(9));
}

#[test]
fn same_width_product_domain_and_receipt_schema_substitutions_refuse() {
    let cuts = [0_i128];
    let mut domain = vec![0_u8; result_domain_record_bytes(1).expect("domain width")];
    compile_result_domain_v2(
        ResultDomainInputV2 {
            product_id: id(1),
            coordinate_domain_id: id(2),
            result_unit_id: id(3),
            liability_basis_id: id(4),
            representation_release_id: id(5),
            mapping_release_id: id(6),
            cut_denominator: 1,
            cuts: &cuts,
        },
        &mut domain,
    )
    .expect("domain");
    let coefficients = [1_u64, 1, 1];
    let mut portfolio = vec![0_u8; portfolio_record_bytes(3).expect("portfolio width")];
    compile_portfolio_v2(
        PortfolioInputV2 {
            product_id: id(1),
            result_domain_id: id(7),
            claim_basis_id: id(8),
            liability_basis_id: id(4),
            representation_release_id: id(5),
            denominator: 1,
            coefficients: &coefficients,
        },
        &mut portfolio,
    )
    .expect("portfolio");
    let mut product_bytes = [0_u8; PRODUCT_RECORD_BYTES_V2];
    ProductRecordV2::new(id(1), id(7), id(9))
        .encode_into(&mut product_bytes)
        .expect("product");
    let receipt = AdmissionReceiptV2 {
        product: coordinate(PRODUCT_RECORD_SCHEMA_ID_V2, 10, 11, 12),
        result_domain: coordinate(RESULT_DOMAIN_SCHEMA_ID_V2, 7, 13, 14),
        portfolio: coordinate(PORTFOLIO_SCHEMA_ID_V2, 9, 15, 16),
    };
    assert!(admit_authenticated_records_v2(receipt, &product_bytes, &domain, &portfolio).is_ok());

    let mut other_domain = domain.clone();
    other_domain
        .get_mut(32..64)
        .expect("product identity")
        .fill(17);
    assert!(
        admit_authenticated_records_v2(receipt, &product_bytes, &other_domain, &portfolio).is_err()
    );

    let wrong_schema = AdmissionReceiptV2 {
        result_domain: coordinate(PRODUCT_RECORD_SCHEMA_ID_V2, 7, 13, 14),
        ..receipt
    };
    let mut receipt_bytes = [0_u8; ADMISSION_RECEIPT_BYTES_V2];
    assert!(wrong_schema.encode_into(&mut receipt_bytes).is_err());
}
