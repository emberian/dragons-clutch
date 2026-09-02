//! Byte agreement between Lean-owned DCE5 coordinates and the safe Rust kernel.

#![allow(clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

#[allow(missing_docs)]
#[path = "../src/generated_v4_abi.rs"]
mod generated;

use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{
        HEADER_BYTES as BASE_HEADER_BYTES, ROUTE_BYTES, RouteKindV3,
        encode::{EffectGeometryV3, RouteInputV3, encode_effect_program_v3_atomic},
    },
    v4::{
        BORROWED_RANGE_BYTES_V4, BorrowedRangePolicyV4, BorrowedRangeV4, ErrorV4, HEADER_BYTES_V4,
        ProgramV4, RequestCoordinateV4, SEMANTIC_RANGE_ROUTE_V4, encode_program_v4_atomic,
    },
};

const ROUTE_COUNT: usize = 5;
const BASE_BYTES: usize = BASE_HEADER_BYTES + ROUTE_COUNT * ROUTE_BYTES;
const DEALER_SUCCESSOR_BYTES: usize = HEADER_BYTES_V4 + 2 * BORROWED_RANGE_BYTES_V4 + BASE_BYTES;
const ZERO_SUCCESSOR_BYTES: usize = HEADER_BYTES_V4 + BASE_BYTES;

fn base_program() -> [u8; BASE_BYTES] {
    let routes = [
        RouteInputV3 {
            role: FixedRole::Custody,
            kind: RouteKindV3::Once,
            enable_common_scalar: None,
            witness_range_common_scalar: None,
            receipt_dependency: None,
            fixed_account_start: 0,
            fixed_account_count: 1,
            item_account_start: 0,
            item_account_count: 0,
            fixed_request: &[],
            item_request: &[],
        },
        RouteInputV3 {
            role: FixedRole::Custody,
            kind: RouteKindV3::Once,
            enable_common_scalar: None,
            witness_range_common_scalar: None,
            receipt_dependency: None,
            fixed_account_start: 1,
            fixed_account_count: 1,
            item_account_start: 0,
            item_account_count: 0,
            fixed_request: &[],
            item_request: &[],
        },
        RouteInputV3 {
            role: FixedRole::Custody,
            kind: RouteKindV3::Once,
            enable_common_scalar: None,
            witness_range_common_scalar: None,
            receipt_dependency: None,
            fixed_account_start: 2,
            fixed_account_count: 1,
            item_account_start: 0,
            item_account_count: 0,
            fixed_request: &[],
            item_request: &[],
        },
        RouteInputV3 {
            role: FixedRole::Custody,
            kind: RouteKindV3::Once,
            enable_common_scalar: None,
            witness_range_common_scalar: None,
            receipt_dependency: None,
            fixed_account_start: 3,
            fixed_account_count: 1,
            item_account_start: 0,
            item_account_count: 0,
            fixed_request: &[],
            item_request: &[],
        },
        RouteInputV3 {
            role: FixedRole::Claims,
            kind: RouteKindV3::Once,
            enable_common_scalar: None,
            witness_range_common_scalar: None,
            receipt_dependency: None,
            fixed_account_start: 4,
            fixed_account_count: 1,
            item_account_start: 0,
            item_account_count: 0,
            fixed_request: &[],
            item_request: &[],
        },
    ];
    let mut scratch = [0_u8; BASE_BYTES];
    let mut output = [0_u8; BASE_BYTES];
    encode_effect_program_v3_atomic(
        EffectGeometryV3 {
            fixed_accounts: 5,
            item_account_stride: 0,
            common_scalars: 2,
            item_scalar_stride: 0,
            common_identities: 1,
            item_identity_stride: 0,
        },
        &routes,
        &[],
        &[],
        &mut scratch,
        &mut output,
    )
    .expect("safe DCE4 base encoder");
    output
}

fn dealer_ranges() -> [BorrowedRangeV4; 2] {
    [
        BorrowedRangeV4::new(
            SEMANTIC_RANGE_ROUTE_V4,
            RequestCoordinateV4::Fixed(384),
            RequestCoordinateV4::ProductTailAffine { base: 0, stride: 8 },
        ),
        BorrowedRangeV4::new(
            4,
            RequestCoordinateV4::ProductTailAffine {
                base: 384,
                stride: 8,
            },
            RequestCoordinateV4::CommonScalar(1),
        ),
    ]
}

fn concatenate(header: &[u8], table: &[u8], base: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(header.len() + table.len() + base.len());
    output.extend_from_slice(header);
    output.extend_from_slice(table);
    output.extend_from_slice(base);
    output
}

#[test]
fn lean_constants_and_layouts_are_the_pinned_dce5_coordinates() {
    // `v4` derives every one of these from `generated`, so comparing the two
    // would compare a name with itself. What derivation cannot give away is
    // whether Lean still says the numbers this wire committed to, so each is
    // pinned against its literal.
    assert_eq!(generated::EFFECT_V4_MAGIC_LEAN, *b"DCE5");
    assert_eq!(generated::EFFECT_V4_VERSION_LEAN, 5);
    assert_eq!(generated::EFFECT_V4_DISJOINT_EXACT_COVERAGE_POLICY_LEAN, 0);
    assert_eq!(
        generated::EFFECT_V4_IDENTICAL_REUSE_EXACT_COVERAGE_POLICY_LEAN,
        1
    );
    assert_eq!(generated::EFFECT_V4_SEMANTIC_RANGE_ROUTE_LEAN, u16::MAX);
    assert_eq!(generated::EFFECT_V4_FIXED_COORDINATE_KIND_LEAN, 0);
    assert_eq!(generated::EFFECT_V4_COMMON_SCALAR_COORDINATE_KIND_LEAN, 1);
    assert_eq!(
        generated::EFFECT_V4_PRODUCT_TAIL_AFFINE_COORDINATE_KIND_LEAN,
        2
    );
    assert_eq!(generated::EFFECT_V4_MAX_EXTENSION_LEAN, 63);
    assert_eq!(generated::EFFECT_V4_HEADER_BYTES_LEAN, 24);
    assert_eq!(generated::EFFECT_V4_DYNAMIC_SPAN_BYTES_LEAN, 16);
    assert_eq!(generated::EFFECT_V4_BORROWED_RANGE_BYTES_LEAN, 16);
    assert_eq!(
        generated::EFFECT_V4_SCHEMA_RELEASE_PREIMAGE_LEAN,
        b"dclutch/schema/effect-program-v5-scalar-spans-and-borrowed-ranges-v2-tail-affine-semantic"
    );
    // Proven to be that preimage's SHA-256 by
    // `effect_v4_lean_generator_fresh::effect_v4_schema_id_is_the_exact_sha256_preimage`;
    // pinned here too because a finalized-record identity moving is a release
    // event, not a refactor.
    assert_eq!(
        generated::EFFECT_V4_SCHEMA_RELEASE_ID_LEAN,
        [
            0x28, 0xe4, 0xa6, 0xc2, 0x95, 0x9d, 0x49, 0x76, 0x12, 0x35, 0xb7, 0x79, 0x9a, 0xa4,
            0xee, 0xcf, 0x28, 0x45, 0x05, 0x29, 0xb2, 0xa5, 0x0c, 0xb9, 0x2b, 0x77, 0x69, 0x6d,
            0x2f, 0xfe, 0xd4, 0x8c,
        ]
    );
    assert_eq!(
        [
            generated::EFFECT_V4_MAGIC_OFFSET,
            generated::EFFECT_V4_VERSION_OFFSET,
            generated::EFFECT_V4_POLICY_OFFSET,
            generated::EFFECT_V4_SPAN_COUNT_OFFSET,
            generated::EFFECT_V4_RANGE_COUNT_OFFSET,
            generated::EFFECT_V4_RESERVED_HEADER_OFFSET,
            generated::EFFECT_V4_BASE_BYTES_OFFSET,
            generated::EFFECT_V4_SEMANTIC_PREFIX_BYTES_OFFSET,
            generated::EFFECT_V4_RESERVED_TAIL_OFFSET,
        ],
        [0, 4, 5, 6, 8, 10, 12, 16, 20]
    );
    assert_eq!(
        [
            generated::EFFECT_V4_SPAN_ROUTE_OFFSET,
            generated::EFFECT_V4_SPAN_SELECTOR_COMMON_SCALAR_OFFSET,
            generated::EFFECT_V4_SPAN_BASE_FIXED_ACCOUNT_COUNT_OFFSET,
            generated::EFFECT_V4_SPAN_RESERVED_OFFSET,
            generated::EFFECT_V4_SPAN_ALLOWED_EXTENSIONS_OFFSET,
        ],
        [0, 2, 4, 6, 8]
    );
    assert_eq!(
        [
            generated::EFFECT_V4_RANGE_ROUTE_OFFSET,
            generated::EFFECT_V4_RANGE_OFFSET_KIND_OFFSET,
            generated::EFFECT_V4_RANGE_LENGTH_KIND_OFFSET,
            generated::EFFECT_V4_RANGE_OFFSET_VALUE_OFFSET,
            generated::EFFECT_V4_RANGE_LENGTH_VALUE_OFFSET,
            generated::EFFECT_V4_RANGE_RESERVED_OFFSET,
        ],
        [0, 2, 3, 4, 8, 12]
    );
}

#[test]
fn lean_zero_table_is_the_unique_safe_fixed_topology_envelope() {
    let base = base_program();
    let mut scratch = [0_u8; ZERO_SUCCESSOR_BYTES];
    let mut output = [0_u8; ZERO_SUCCESSOR_BYTES];
    encode_program_v4_atomic(
        &base,
        BorrowedRangePolicyV4::DisjointExactCoverage,
        1,
        &[],
        &[],
        &mut scratch,
        &mut output,
    )
    .expect("safe zero-table DCE5 encoder");
    assert_eq!(
        output.get(..HEADER_BYTES_V4),
        Some(generated::EFFECT_V4_ZERO_TABLE_HEADER_WITNESS.as_slice())
    );
    let decoded = ProgramV4::decode(&output).expect("safe zero-table decoder");
    assert_eq!(decoded.span_count(), 0);
    assert_eq!(decoded.range_count(), 0);
    assert_eq!(decoded.base().bytes(), base);

    let hostile = concatenate(&generated::EFFECT_V4_ZERO_SPAN_COUNT_REFUSAL, &[], &base);
    assert_eq!(ProgramV4::decode(&hostile), Err(ErrorV4::Wire));
    let hostile = concatenate(&generated::EFFECT_V4_HEADER_RESERVED_REFUSAL, &[], &base);
    assert_eq!(ProgramV4::decode(&hostile), Err(ErrorV4::Wire));
}

#[test]
fn lean_dealer_ranges_match_safe_encoding_and_exact_request_partition() {
    let base = base_program();
    let ranges = dealer_ranges();
    let mut scratch = [0_u8; DEALER_SUCCESSOR_BYTES];
    let mut output = [0_u8; DEALER_SUCCESSOR_BYTES];
    encode_program_v4_atomic(
        &base,
        BorrowedRangePolicyV4::DisjointExactCoverage,
        384,
        &[],
        &ranges,
        &mut scratch,
        &mut output,
    )
    .expect("safe Dealer affine-range encoder");
    assert_eq!(
        output.get(..HEADER_BYTES_V4),
        Some(generated::EFFECT_V4_DEALER_HEADER_WITNESS.as_slice())
    );
    assert_eq!(
        output.get(HEADER_BYTES_V4..HEADER_BYTES_V4 + 2 * BORROWED_RANGE_BYTES_V4),
        Some(generated::EFFECT_V4_DEALER_RANGE_TABLE_WITNESS.as_slice())
    );
    let decoded = ProgramV4::decode(&output).expect("safe Dealer range decoder");
    let identities = [[1_u8; 32]];
    for tail_count in [1_u32, 16, 256] {
        let scalars = [0_u64, 640];
        let request_bytes = 384 + usize::try_from(tail_count).expect("small tail") * 8 + 640;
        assert_eq!(
            decoded.validate_request_coverage(request_bytes, tail_count, &scalars, &identities,),
            Ok(())
        );
        let child = decoded
            .resolved_borrowed_range_for_tail(4, 0, tail_count, &scalars)
            .expect("safe exact child range");
        assert_eq!(
            child.source_offset(),
            384 + usize::try_from(tail_count).expect("small tail") * 8
        );
        assert_eq!(child.len(), 640);
    }
}

fn decoded_with_table(base: &[u8], table: &[u8]) -> Vec<u8> {
    concatenate(&generated::EFFECT_V4_DEALER_HEADER_WITNESS, table, base)
}

#[test]
fn lean_hostile_range_corpus_refuses_zero_stride_reorder_overlap_and_gap() {
    let base = base_program();
    let zero_stride = decoded_with_table(&base, &generated::EFFECT_V4_AFFINE_ZERO_STRIDE_REFUSAL);
    assert_eq!(ProgramV4::decode(&zero_stride), Err(ErrorV4::RangeTable));

    let identities = [[1_u8; 32]];
    let scalars = [0_u64, 640];
    for hostile_table in [
        generated::EFFECT_V4_REVERSED_RANGES_REFUSAL.as_slice(),
        generated::EFFECT_V4_CHILD_OVERLAP_REFUSAL.as_slice(),
        generated::EFFECT_V4_CHILD_GAP_REFUSAL.as_slice(),
    ] {
        let hostile = decoded_with_table(&base, hostile_table);
        let decoded = ProgramV4::decode(&hostile).expect("structurally valid hostile table");
        assert_eq!(
            decoded.validate_request_coverage(384 + 16 * 8 + 640, 16, &scalars, &identities),
            Err(ErrorV4::RequestCoverage)
        );
    }
}
