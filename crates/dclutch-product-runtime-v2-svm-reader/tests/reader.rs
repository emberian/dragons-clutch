//! Hostile borrowed-account tests for the independent Product graph reader.

use dclutch_product_payoff_v2_codec::{
    registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
    runtime_v3::{
        BasisInputV3, BasisKindV3, BasisShapeV3, BasisTermV3, SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
        basis_record_bytes_v3, compile_basis_v3, semantic_basis_preimage_v3,
    },
};
use dclutch_product_runtime_v2::{
    ContentId, PortfolioInputV2, ResultDomainInputV2, compile_portfolio_v2,
    compile_result_domain_v2, portfolio_record_bytes, result_domain_record_bytes,
};
use dclutch_product_runtime_v2_admission::{
    ADMISSION_RECEIPT_BYTES_V2, AdmissionReceiptV2, FinalizedRecordCoordinateV2,
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_BYTES_V2, PRODUCT_RECORD_SCHEMA_ID_V2, ProductRecordV2,
    RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_product_runtime_v2_svm_reader::representation_v3::{
    RepresentationRuntimeContextV3, RepresentationRuntimeFrameV3,
    authenticate_product_representation_v3,
};
use dclutch_product_runtime_v2_svm_reader::*;
use dclutch_rational_representation_v2_kernel::{
    DESCRIPTOR_COEFFICIENT_BYTES, DESCRIPTOR_HEADER_BYTES, DESCRIPTOR_MAGIC_V3, GRAPH_HEADER_BYTES,
    GRAPH_MAGIC_V2, GRAPH_NODE_BYTES, REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
    REPRESENTATION_GRAPH_SCHEMA_RELEASE_ID_V2, SCALAR_BYTES, SCHEMA_VERSION_V2,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    pubkey::Pubkey,
    rent::Rent,
};
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

struct RuntimeV3Backing {
    product: RecordBacking,
    domain: RecordBacking,
    portfolio: RecordBacking,
    basis: RecordBacking,
}

struct RepresentationV3Backing {
    runtime: RuntimeV3Backing,
    descriptor: RecordBacking,
    graph: RecordBacking,
    context: RepresentationRuntimeContextV3,
}

#[derive(Clone, Copy)]
struct RuntimeV3Snapshot {
    runtime: AuthenticatedProductRuntimeV2,
    linked_basis_record: AuthenticatedRecordV2,
    semantic_basis_id: ContentId,
    basis_kind: BasisKindV3,
    basis_width: u32,
    payout_scale: u64,
    evaluator_release_id: ContentId,
    linked_basis_raw_writable: bool,
    linked_basis_staging_writable: bool,
    linked_basis_body_digest: ContentId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RepresentationV3Snapshot {
    descriptor_record: AuthenticatedRecordV2,
    graph_record: AuthenticatedRecordV2,
    representation_authority: Pubkey,
    basis_width: u32,
    descriptor_id: [u8; 32],
    graph_id: [u8; 32],
    market_id: [u8; 32],
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

fn semantic_basis_id(bytes: &[u8]) -> ContentId {
    let semantic = semantic_basis_preimage_v3(bytes).expect("semantic basis");
    ContentId::new(
        hashv(&[
            SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
            semantic.prefix(),
            semantic.suffix(),
        ])
        .to_bytes(),
    )
    .expect("semantic digest")
}

fn basis_bytes(
    kind: BasisKindV3,
    product_id: ContentId,
    result_domain_id: ContentId,
    changed_semantics: bool,
) -> Vec<u8> {
    let knots = [0_i128, 10_i128];
    let terms = [BasisTermV3 {
        claim_index: 0,
        shape: BasisShapeV3::RampUp { left: 0, right: 1 },
        amplitude: if changed_semantics { 99 } else { 100 },
    }];
    let failure_payouts = [0_u64, 100_u64];
    let (basis_width, payout_scale, knot_denominator, active_knots, active_terms, failure) =
        match kind {
            BasisKindV3::CategoricalQ1 => (4, 1, 1, &[][..], &[][..], &[][..]),
            BasisKindV3::GradedExactComplement => {
                (2, 100, 1, &knots[..], &terms[..], &failure_payouts[..])
            }
        };
    let categorical_width = if changed_semantics && kind == BasisKindV3::CategoricalQ1 {
        5
    } else {
        basis_width
    };
    let input = BasisInputV3 {
        kind,
        product_id: product_id.to_bytes(),
        result_domain_id: result_domain_id.to_bytes(),
        coordinate_domain_id: id(2).to_bytes(),
        result_unit_id: id(3).to_bytes(),
        evaluator_release_id: id(8).to_bytes(),
        basis_width: categorical_width,
        payout_scale,
        knot_denominator,
        knots: active_knots,
        terms: active_terms,
        failure_payouts: failure,
    };
    let width = basis_record_bytes_v3(
        kind,
        usize::try_from(input.basis_width).expect("basis width"),
        input.knots.len(),
        input.terms.len(),
    )
    .expect("basis record width");
    let mut bytes = vec![0_u8; width];
    compile_basis_v3(input, &mut bytes).expect("basis");
    bytes
}

fn compiled_runtime_v3(kind: BasisKindV3, product_byte: u8) -> RuntimeV3Backing {
    let product_id = id(product_byte);
    let provisional_basis = basis_bytes(kind, id(0xf1), id(0xf2), false);
    let liability_basis_id = semantic_basis_id(&provisional_basis);
    let cuts = [-10_i128, 10_i128];
    let mut domain = vec![0_u8; result_domain_record_bytes(cuts.len()).expect("domain width")];
    compile_result_domain_v2(
        ResultDomainInputV2 {
            product_id,
            coordinate_domain_id: id(2),
            result_unit_id: id(3),
            liability_basis_id,
            representation_release_id: id(5),
            mapping_release_id: id(6),
            cut_denominator: 10,
            cuts: &cuts,
        },
        &mut domain,
    )
    .expect("domain");
    let domain = record(RESULT_DOMAIN_SCHEMA_ID_V2, domain);
    let coefficients = [7_u64; 4];
    let mut portfolio =
        vec![0_u8; portfolio_record_bytes(coefficients.len()).expect("portfolio width")];
    compile_portfolio_v2(
        PortfolioInputV2 {
            product_id,
            result_domain_id: domain.coordinate.content_digest,
            claim_basis_id: id(7),
            liability_basis_id,
            representation_release_id: id(5),
            denominator: 9,
            coefficients: &coefficients,
        },
        &mut portfolio,
    )
    .expect("portfolio");
    let portfolio = record(PORTFOLIO_SCHEMA_ID_V2, portfolio);
    let mut product = vec![0_u8; PRODUCT_RECORD_BYTES_V2];
    ProductRecordV2::new(
        product_id,
        domain.coordinate.content_digest,
        portfolio.coordinate.content_digest,
    )
    .encode_into(&mut product)
    .expect("Product record");
    let product = record(PRODUCT_RECORD_SCHEMA_ID_V2, product);
    let basis = record(
        GRADED_BASIS_RECORD_SCHEMA_ID_V3,
        basis_bytes(kind, product_id, domain.coordinate.content_digest, false),
    );
    RuntimeV3Backing {
        product,
        domain,
        portfolio,
        basis,
    }
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    output
        .get_mut(offset..offset + value.len())
        .expect("fixture offset")
        .copy_from_slice(value);
}

fn put_u32(output: &mut [u8], offset: usize, value: u32) {
    put(output, offset, &value.to_le_bytes());
}

fn put_u64(output: &mut [u8], offset: usize, value: u64) {
    put(output, offset, &value.to_le_bytes());
}

fn representation_graph_v3() -> Vec<u8> {
    const WIDTH: u32 = 4;
    let width = usize::try_from(WIDTH).expect("fixture width");
    let mut output = vec![0_u8; GRAPH_HEADER_BYTES + GRAPH_NODE_BYTES + width * SCALAR_BYTES];
    put(&mut output, 0, &GRAPH_MAGIC_V2);
    put(&mut output, 8, &SCHEMA_VERSION_V2.to_le_bytes());
    put(&mut output, 16, &[0x71; 32]);
    put(&mut output, 48, &[0x72; 32]);
    put_u32(&mut output, 80, WIDTH);
    put_u32(&mut output, 84, 1);
    put_u32(&mut output, 88, 0);
    put_u64(&mut output, 96, 1);
    put(&mut output, GRAPH_HEADER_BYTES, &[0x72; 32]);
    put_u32(&mut output, GRAPH_HEADER_BYTES + 32, 0);
    put_u32(&mut output, GRAPH_HEADER_BYTES + 36, 0);
    put_u32(&mut output, GRAPH_HEADER_BYTES + 40, 0);
    *output
        .get_mut(GRAPH_HEADER_BYTES + 44)
        .expect("node-kind byte") = 0;
    put_u64(&mut output, GRAPH_HEADER_BYTES + 48, 0);
    put_u64(&mut output, GRAPH_HEADER_BYTES + GRAPH_NODE_BYTES, 1);
    output
}

fn representation_descriptor_v3(
    graph_digest: [u8; 32],
    context: RepresentationRuntimeContextV3,
) -> Vec<u8> {
    const WIDTH: u32 = 4;
    let width = usize::try_from(WIDTH).expect("fixture width");
    let mut output = vec![0_u8; DESCRIPTOR_HEADER_BYTES + width * DESCRIPTOR_COEFFICIENT_BYTES];
    put(&mut output, 0, &DESCRIPTOR_MAGIC_V3);
    put(&mut output, 8, &3_u16.to_le_bytes());
    put(&mut output, 16, &[0x71; 32]);
    put(&mut output, 48, &graph_digest);
    put(&mut output, 80, &[0x72; 32]);
    put(&mut output, 112, &context.market.to_bytes());
    put(&mut output, 144, &context.release_set.to_bytes());
    put(&mut output, 176, &context.receipt_mint.to_bytes());
    put(&mut output, 208, &context.token_program.to_bytes());
    put_u32(&mut output, 240, WIDTH);
    put_u64(&mut output, 248, 10);
    put_u64(&mut output, DESCRIPTOR_HEADER_BYTES, 10);
    output
}

fn compiled_representation_v3() -> RepresentationV3Backing {
    let runtime = compiled_runtime_v3(BasisKindV3::CategoricalQ1, 0x61);
    let context = RepresentationRuntimeContextV3 {
        claims_program: Pubkey::new_from_array([0x81; 32]),
        market: Pubkey::new_from_array([0x82; 32]),
        release_set: Pubkey::new_from_array([0x83; 32]),
        claims_basis_id: semantic_basis_id(&runtime.basis.raw.data),
        claims_width: 4,
        receipt_mint: Pubkey::new_from_array([0x84; 32]),
        token_program: Pubkey::new_from_array([0x85; 32]),
    };
    let graph_bytes = representation_graph_v3();
    let graph_digest = hash(&graph_bytes).to_bytes();
    let descriptor_bytes = representation_descriptor_v3(graph_digest, context);
    RepresentationV3Backing {
        runtime,
        descriptor: record(
            REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
            descriptor_bytes,
        ),
        graph: record(REPRESENTATION_GRAPH_SCHEMA_RELEASE_ID_V2, graph_bytes),
        context,
    }
}

fn authenticate_representation_v3(
    backing: &mut RepresentationV3Backing,
) -> Result<RepresentationV3Snapshot> {
    let product_raw = backing.runtime.product.raw.info();
    let product_staging = backing.runtime.product.staging.info();
    let domain_raw = backing.runtime.domain.raw.info();
    let domain_staging = backing.runtime.domain.staging.info();
    let portfolio_raw = backing.runtime.portfolio.raw.info();
    let portfolio_staging = backing.runtime.portfolio.staging.info();
    let basis_raw = backing.runtime.basis.raw.info();
    let basis_staging = backing.runtime.basis.staging.info();
    let descriptor_raw = backing.descriptor.raw.info();
    let descriptor_staging = backing.descriptor.staging.info();
    let graph_raw = backing.graph.raw.info();
    let graph_staging = backing.graph.staging.info();
    let authenticated = authenticate_product_representation_v3(
        &REGISTRY,
        &Rent::default(),
        backing.runtime.product.coordinate.content_digest,
        backing.descriptor.coordinate.content_digest,
        backing.context,
        RepresentationRuntimeFrameV3 {
            product: ProductRuntimeFrameV3 {
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
                linked_basis: FinalizedRecordFrameV2 {
                    raw: &basis_raw,
                    staging: &basis_staging,
                },
            },
            descriptor: FinalizedRecordFrameV2 {
                raw: &descriptor_raw,
                staging: &descriptor_staging,
            },
            graph: FinalizedRecordFrameV2 {
                raw: &graph_raw,
                staging: &graph_staging,
            },
        },
    )?;
    Ok(RepresentationV3Snapshot {
        descriptor_record: authenticated.descriptor_record,
        graph_record: authenticated.graph_record,
        representation_authority: authenticated.representation_authority,
        basis_width: authenticated.admission.basis_width(),
        descriptor_id: authenticated.admission.descriptor_id(),
        graph_id: authenticated.admission.graph_id(),
        market_id: authenticated.admission.market_id(),
    })
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

fn authenticate_v3(runtime: &mut RuntimeV3Backing) -> Result<RuntimeV3Snapshot> {
    let product_raw = runtime.product.raw.info();
    let product_staging = runtime.product.staging.info();
    let domain_raw = runtime.domain.raw.info();
    let domain_staging = runtime.domain.staging.info();
    let portfolio_raw = runtime.portfolio.raw.info();
    let portfolio_staging = runtime.portfolio.staging.info();
    let basis_raw = runtime.basis.raw.info();
    let basis_staging = runtime.basis.staging.info();
    let authenticated = authenticate_product_runtime_v3(
        &REGISTRY,
        &Rent::default(),
        runtime.product.coordinate.content_digest,
        ProductRuntimeFrameV3 {
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
            linked_basis: FinalizedRecordFrameV2 {
                raw: &basis_raw,
                staging: &basis_staging,
            },
        },
    )?;
    let linked_basis_body_digest = content_id(
        hash(
            &authenticated
                .linked_basis_raw
                .try_borrow_data()
                .map_err(|_| Error::Borrow)?,
        )
        .to_bytes(),
    );
    Ok(RuntimeV3Snapshot {
        runtime: authenticated.runtime,
        linked_basis_record: authenticated.linked_basis_record,
        semantic_basis_id: authenticated.semantic_basis_id,
        basis_kind: authenticated.basis_kind,
        basis_width: authenticated.basis_width,
        payout_scale: authenticated.payout_scale,
        evaluator_release_id: authenticated.evaluator_release_id,
        linked_basis_raw_writable: authenticated.linked_basis_raw.is_writable,
        linked_basis_staging_writable: authenticated.linked_basis_staging.is_writable,
        linked_basis_body_digest,
    })
}

fn authenticate_v3_continuation(runtime: &mut RuntimeV3Backing) -> Result<RuntimeV3Snapshot> {
    let product_raw = runtime.product.raw.info();
    let product_staging = runtime.product.staging.info();
    let domain_raw = runtime.domain.raw.info();
    let domain_staging = runtime.domain.staging.info();
    let portfolio_raw = runtime.portfolio.raw.info();
    let portfolio_staging = runtime.portfolio.staging.info();
    let basis_raw = runtime.basis.raw.info();
    let basis_staging = runtime.basis.staging.info();
    let authenticated_v2 = authenticate_product_runtime_v2(
        &REGISTRY,
        &Rent::default(),
        runtime.product.coordinate.content_digest,
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
    )?;
    let authenticated = authenticate_product_basis_v3(
        &REGISTRY,
        &Rent::default(),
        authenticated_v2,
        FinalizedRecordFrameV2 {
            raw: &basis_raw,
            staging: &basis_staging,
        },
    )?;
    let linked_basis_body_digest = content_id(
        hash(
            &authenticated
                .linked_basis_raw
                .try_borrow_data()
                .map_err(|_| Error::Borrow)?,
        )
        .to_bytes(),
    );
    Ok(RuntimeV3Snapshot {
        runtime: authenticated.runtime,
        linked_basis_record: authenticated.linked_basis_record,
        semantic_basis_id: authenticated.semantic_basis_id,
        basis_kind: authenticated.basis_kind,
        basis_width: authenticated.basis_width,
        payout_scale: authenticated.payout_scale,
        evaluator_release_id: authenticated.evaluator_release_id,
        linked_basis_raw_writable: authenticated.linked_basis_raw.is_writable,
        linked_basis_staging_writable: authenticated.linked_basis_staging.is_writable,
        linked_basis_body_digest,
    })
}

fn content_id(bytes: [u8; 32]) -> ContentId {
    ContentId::new(bytes).expect("nonzero digest")
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

#[test]
fn v3_authenticates_both_canonical_basis_kinds_and_exposes_read_only_coordinate() {
    for (kind, expected_width, expected_scale) in [
        (BasisKindV3::CategoricalQ1, 4, 1),
        (BasisKindV3::GradedExactComplement, 2, 100),
    ] {
        let mut backing = compiled_runtime_v3(kind, 0x21);
        let expected_basis_digest = backing.basis.coordinate.content_digest;
        let authenticated = authenticate_v3(&mut backing).expect("V3 graph");
        assert_eq!(authenticated.basis_kind, kind);
        assert_eq!(authenticated.basis_width, expected_width);
        assert_eq!(authenticated.payout_scale, expected_scale);
        assert_eq!(authenticated.evaluator_release_id, id(8));
        assert_eq!(
            authenticated.semantic_basis_id,
            authenticated.runtime.liability_basis_id
        );
        assert_eq!(
            authenticated.linked_basis_record.schema_id,
            ContentId::new(GRADED_BASIS_RECORD_SCHEMA_ID_V3).expect("schema")
        );
        assert_eq!(
            authenticated.linked_basis_record.content_digest,
            expected_basis_digest
        );
        assert!(!authenticated.linked_basis_raw_writable);
        assert!(!authenticated.linked_basis_staging_writable);
        assert_eq!(
            authenticated.linked_basis_body_digest,
            expected_basis_digest
        );
    }
}

#[test]
fn v3_basis_only_continuation_matches_full_graph_without_redecoding_runtime() {
    for kind in [
        BasisKindV3::CategoricalQ1,
        BasisKindV3::GradedExactComplement,
    ] {
        let mut full = compiled_runtime_v3(kind, 0x29);
        let mut continued = compiled_runtime_v3(kind, 0x29);
        let full = authenticate_v3(&mut full).expect("full V3 graph");
        let continued =
            authenticate_v3_continuation(&mut continued).expect("basis-only continuation");
        assert_eq!(continued.runtime, full.runtime);
        assert_eq!(continued.linked_basis_record, full.linked_basis_record);
        assert_eq!(continued.semantic_basis_id, full.semantic_basis_id);
        assert_eq!(continued.basis_kind, full.basis_kind);
        assert_eq!(continued.basis_width, full.basis_width);
        assert_eq!(continued.payout_scale, full.payout_scale);
        assert_eq!(continued.evaluator_release_id, full.evaluator_release_id);
        assert_eq!(
            continued.linked_basis_body_digest,
            full.linked_basis_body_digest
        );
    }
}

#[test]
fn v3_equivalent_semantics_do_not_pin_one_raw_encoding_in_product() {
    for kind in [
        BasisKindV3::CategoricalQ1,
        BasisKindV3::GradedExactComplement,
    ] {
        let mut first = compiled_runtime_v3(kind, 0x31);
        let mut second = compiled_runtime_v3(kind, 0x32);
        assert_ne!(first.basis.raw.data, second.basis.raw.data);
        assert_ne!(
            first.basis.coordinate.content_digest,
            second.basis.coordinate.content_digest
        );
        let first = authenticate_v3(&mut first).expect("first encoding");
        let second = authenticate_v3(&mut second).expect("relinked encoding");
        assert_eq!(first.semantic_basis_id, second.semantic_basis_id);
        assert_ne!(
            first.linked_basis_record.content_digest,
            second.linked_basis_record.content_digest
        );
    }
}

#[test]
fn v3_refuses_schema_raw_and_product_link_substitution() {
    let mut valid = compiled_runtime_v3(BasisKindV3::GradedExactComplement, 0x41);
    let mut foreign = compiled_runtime_v3(BasisKindV3::GradedExactComplement, 0x42);
    core::mem::swap(&mut valid.basis, &mut foreign.basis);
    assert!(matches!(
        authenticate_v3(&mut valid),
        Err(Error::LinkedBasisComposition)
    ));

    let mut wrong_schema = compiled_runtime_v3(BasisKindV3::CategoricalQ1, 0x43);
    wrong_schema.basis = record([0xa5; 32], wrong_schema.basis.raw.data.clone());
    assert!(matches!(
        authenticate_v3(&mut wrong_schema),
        Err(Error::LinkedBasisRecord)
    ));

    let mut wrong_raw = compiled_runtime_v3(BasisKindV3::CategoricalQ1, 0x44);
    *wrong_raw.basis.raw.data.last_mut().expect("basis byte") ^= 1;
    assert!(matches!(
        authenticate_v3(&mut wrong_raw),
        Err(Error::LinkedBasisRecord)
    ));
}

#[test]
fn v3_refuses_canonical_wrong_semantic_basis() {
    for kind in [
        BasisKindV3::CategoricalQ1,
        BasisKindV3::GradedExactComplement,
    ] {
        let mut backing = compiled_runtime_v3(kind, 0x51);
        let changed = basis_bytes(
            kind,
            id(0x51),
            backing.domain.coordinate.content_digest,
            true,
        );
        backing.basis = record(GRADED_BASIS_RECORD_SCHEMA_ID_V3, changed);
        assert!(matches!(
            authenticate_v3(&mut backing),
            Err(Error::LinkedBasisComposition)
        ));
    }
}

#[test]
fn representation_v3_authenticates_product_descriptor_and_selected_graph() {
    let mut backing = compiled_representation_v3();
    let descriptor_digest = backing.descriptor.coordinate.content_digest;
    let graph_digest = backing.graph.coordinate.content_digest;
    let expected_authority = Pubkey::find_program_address(
        &[
            dclutch_rational_representation_v2_kernel::RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
            descriptor_digest.to_bytes().as_slice(),
        ],
        &backing.context.claims_program,
    )
    .0;
    let authenticated =
        authenticate_representation_v3(&mut backing).expect("exact representation graph");
    assert_eq!(
        authenticated.descriptor_record.content_digest,
        descriptor_digest
    );
    assert_eq!(authenticated.graph_record.content_digest, graph_digest);
    assert_eq!(authenticated.representation_authority, expected_authority);
    assert_eq!(authenticated.basis_width, 4);
    assert_eq!(authenticated.descriptor_id, descriptor_digest.to_bytes());
    assert_eq!(authenticated.graph_id, [0x71; 32]);
    assert_eq!(authenticated.market_id, [0x82; 32]);
}

#[test]
fn representation_v3_refuses_claims_width_and_basis_substitution() {
    let mut wrong_width = compiled_representation_v3();
    wrong_width.context.claims_width = 3;
    assert!(matches!(
        authenticate_representation_v3(&mut wrong_width),
        Err(Error::RepresentationComposition)
    ));

    let mut wrong_basis = compiled_representation_v3();
    wrong_basis.context.claims_basis_id = id(0xf4);
    assert!(matches!(
        authenticate_representation_v3(&mut wrong_basis),
        Err(Error::RepresentationComposition)
    ));
}

#[test]
fn representation_v3_refuses_graph_substitution_and_cross_role_alias() {
    let mut substituted = compiled_representation_v3();
    *substituted.graph.raw.data.last_mut().expect("graph byte") ^= 1;
    assert!(matches!(
        authenticate_representation_v3(&mut substituted),
        Err(Error::RepresentationGraphRecord)
    ));

    let mut aliased = compiled_representation_v3();
    aliased.graph.raw.key = aliased.descriptor.raw.key;
    assert!(matches!(
        authenticate_representation_v3(&mut aliased),
        Err(Error::AccountFrame)
    ));
}
