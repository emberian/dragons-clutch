//! Hostile borrowed-account tests for the independent Product graph reader.

use dclutch_product_runtime_v2::{
    ContentId, PortfolioInputV2, ResultDomainInputV2, compile_portfolio_v2,
    compile_result_domain_v2, portfolio_record_bytes, result_domain_record_bytes,
};
use dclutch_product_runtime_v2_admission::{
    ADMISSION_RECEIPT_BYTES_V2, AdmissionReceiptV2, FinalizedRecordCoordinateV2,
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_BYTES_V2, PRODUCT_RECORD_SCHEMA_ID_V2, ProductRecordV2,
    RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_product_runtime_v2_svm_reader::*;
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use solana_program::{account_info::AccountInfo, hash::hash, pubkey::Pubkey, rent::Rent};
use solana_sdk_ids::system_program;

const REGISTRY: Pubkey = Pubkey::new_from_array([0x91; 32]);

struct BackingAccount {
    key: Pubkey,
    owner: Pubkey,
    lamports: u64,
    data: Vec<u8>,
}

impl BackingAccount {
    fn info(&mut self) -> AccountInfo<'_> {
        AccountInfo::new(
            &self.key,
            false,
            false,
            &mut self.lamports,
            &mut self.data,
            &self.owner,
            false,
        )
    }
}

struct RecordBacking {
    raw: BackingAccount,
    staging: BackingAccount,
    coordinate: FinalizedRecordCoordinateV2,
}

fn id(byte: u8) -> ContentId {
    ContentId::new([byte; 32]).expect("nonzero fixture identity")
}

fn coordinate(schema: [u8; 32], bytes: &[u8]) -> FinalizedRecordCoordinateV2 {
    let digest = hash(bytes).to_bytes();
    let raw =
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &REGISTRY).0;
    let staging =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], &REGISTRY).0;
    FinalizedRecordCoordinateV2 {
        schema_id: ContentId::new(schema).expect("schema"),
        content_digest: ContentId::new(digest).expect("digest"),
        raw_account: ContentId::new(raw.to_bytes()).expect("raw"),
        staging_account: ContentId::new(staging.to_bytes()).expect("staging"),
    }
}

fn record(schema: [u8; 32], bytes: Vec<u8>) -> RecordBacking {
    let coordinate = coordinate(schema, &bytes);
    RecordBacking {
        raw: BackingAccount {
            key: Pubkey::new_from_array(coordinate.raw_account.to_bytes()),
            owner: REGISTRY,
            lamports: Rent::default().minimum_balance(bytes.len()),
            data: bytes,
        },
        staging: BackingAccount {
            key: Pubkey::new_from_array(coordinate.staging_account.to_bytes()),
            owner: system_program::ID,
            lamports: 11,
            data: Vec::new(),
        },
        coordinate,
    }
}

fn compiled_records(cuts: &[i128]) -> (RecordBacking, RecordBacking, RecordBacking) {
    let outcome_count = cuts.len().checked_add(2).expect("outcome count");
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
            cuts,
        },
        &mut domain,
    )
    .expect("domain");
    let domain_record = record(RESULT_DOMAIN_SCHEMA_ID_V2, domain);
    let coefficients = vec![7_u64; outcome_count];
    let mut portfolio =
        vec![0_u8; portfolio_record_bytes(coefficients.len()).expect("portfolio width")];
    compile_portfolio_v2(
        PortfolioInputV2 {
            product_id: id(1),
            result_domain_id: domain_record.coordinate.content_digest,
            claim_basis_id: id(7),
            liability_basis_id: id(4),
            representation_release_id: id(5),
            denominator: 9,
            coefficients: &coefficients,
        },
        &mut portfolio,
    )
    .expect("portfolio");
    let portfolio_record = record(PORTFOLIO_SCHEMA_ID_V2, portfolio);
    let mut product = vec![0_u8; PRODUCT_RECORD_BYTES_V2];
    ProductRecordV2::new(
        id(1),
        domain_record.coordinate.content_digest,
        portfolio_record.coordinate.content_digest,
    )
    .encode_into(&mut product)
    .expect("Product record");
    let product_record = record(PRODUCT_RECORD_SCHEMA_ID_V2, product);
    (product_record, domain_record, portfolio_record)
}

fn authenticate(
    product: &mut RecordBacking,
    domain: &mut RecordBacking,
    portfolio: &mut RecordBacking,
) -> Result<AuthenticatedProductRuntimeV2> {
    let product_raw = product.raw.info();
    let product_staging = product.staging.info();
    let domain_raw = domain.raw.info();
    let domain_staging = domain.staging.info();
    let portfolio_raw = portfolio.raw.info();
    let portfolio_staging = portfolio.staging.info();
    authenticate_product_runtime_v2(
        &REGISTRY,
        &Rent::default(),
        product.coordinate.content_digest,
        ProductRuntimeFrameV2 {
            product: FinalizedRecordFrameV2 {
                raw: &product_raw,
                staging: &product_staging,
            },
            result_domain: FinalizedRecordFrameV2 {
                raw: &domain_raw,
                staging: &domain_staging,
            },
            portfolio: FinalizedRecordFrameV2 {
                raw: &portfolio_raw,
                staging: &portfolio_staging,
            },
        },
    )
}

#[test]
fn independently_authenticates_258_outcomes_before_rechecking_receipt() {
    let cuts: Vec<i128> = (-128_i128..128).collect();
    let (mut product, mut domain, mut portfolio) = compiled_records(&cuts);
    let authenticated = authenticate(&mut product, &mut domain, &mut portfolio).expect("graph");
    assert_eq!(authenticated.outcome_count, 258);
    assert_eq!(authenticated.product_id, id(1));
    assert_eq!(authenticated.claim_basis_id, id(7));
    assert_eq!(authenticated.liability_basis_id, id(4));
    assert_eq!(authenticated.mapping_release_id, id(6));

    let mut receipt = [0_u8; ADMISSION_RECEIPT_BYTES_V2];
    AdmissionReceiptV2 {
        product: product.coordinate,
        result_domain: domain.coordinate,
        portfolio: portfolio.coordinate,
    }
    .encode_into(&mut receipt)
    .expect("receipt");
    assert_eq!(authenticated.recheck_reference_receipt(&receipt), Ok(()));
    receipt
        .get_mut(200)
        .expect("receipt hostile byte")
        .clone_from(&0xff);
    assert_eq!(
        authenticated.recheck_reference_receipt(&receipt),
        Err(Error::ReceiptMismatch)
    );
}

#[test]
fn same_width_domain_substitution_refuses_without_receipt_authority() {
    let cuts: Vec<i128> = (-128_i128..128).collect();
    let hostile_cuts: Vec<i128> = (-127_i128..129).collect();
    let (mut product, mut domain, mut portfolio) = compiled_records(&cuts);
    let (_, mut hostile_domain, _) = compiled_records(&hostile_cuts);
    assert_eq!(domain.raw.data.len(), hostile_domain.raw.data.len());
    assert_eq!(
        authenticate(&mut product, &mut hostile_domain, &mut portfolio),
        Err(Error::ResultDomainRecord)
    );
    assert!(authenticate(&mut product, &mut domain, &mut portfolio).is_ok());
}
