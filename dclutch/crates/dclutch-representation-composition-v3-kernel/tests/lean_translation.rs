//! Lean-owned fixed coordinates, pinned, and their agreement with safe Rust.
//!
//! `abi`, `graph` and `translation` now derive every constant and byte offset
//! from `generated_abi.rs`, so asserting the two against each other would
//! compare a name with itself. What derivation cannot give away is whether
//! Lean still says the numbers this wire committed to, so each is pinned
//! against its literal; the witness and hostile-corpus tests below are what
//! check that safe Rust agrees with Lean about the bytes.

#[allow(missing_docs)]
#[path = "../src/generated_abi.rs"]
mod generated;

use dclutch_representation_composition_v3_kernel::{
    COMPOSITION_DESCRIPTOR_BYTES_V3, CanonicalTranslationInputV3, CompositionDescriptorInputV3,
    CompositionDescriptorV3, CompositionEdgeInputV3, CompositionGraphInputV3,
    CompositionNodeInputV3, CompositionNodeKindV3, Error, RecordAdmissionV3, SparseTermV3,
    composition_graph_bytes_v3, composition_translation_bytes_v3, decode_composition_bundle_v3,
    encode_canonical_translation_v3_atomic, encode_composition_descriptor_v3_atomic,
    encode_composition_graph_v3_atomic,
};

const GRAPH_ID: [u8; 32] = [40; 32];
const GRAPH_DIGEST: [u8; 32] = [41; 32];
const ROOT_ID: [u8; 32] = [30; 32];
const TRANSLATION_ID: [u8; 32] = [50; 32];
const TRANSLATION_DIGEST: [u8; 32] = [51; 32];
const DESCRIPTOR_ID: [u8; 32] = [90; 32];
const DESCRIPTOR_DIGEST: [u8; 32] = [91; 32];

fn admitted(selected_id: [u8; 32], digest: [u8; 32]) -> RecordAdmissionV3 {
    RecordAdmissionV3 {
        selected_id,
        finalized_id: selected_id,
        recomputed_digest: digest,
        finalized_digest: digest,
        record_authenticated: true,
    }
}

fn descriptor_input() -> CompositionDescriptorInputV3 {
    CompositionDescriptorInputV3 {
        market: [1; 32],
        result_domain: [2; 32],
        release_set: [3; 32],
        native_basis: [4; 32],
        graph_id: GRAPH_ID,
        graph_digest: GRAPH_DIGEST,
        root_id: ROOT_ID,
        translation_id: TRANSLATION_ID,
        translation_digest: TRANSLATION_DIGEST,
        outcome_count: 3,
        node_count: 4,
        edge_count: 3,
        term_count: 6,
        root_denominator: 2,
    }
}

fn nodes() -> [CompositionNodeInputV3; 4] {
    [
        CompositionNodeInputV3 {
            id: [10; 32],
            rank: 0,
            first_edge: 0,
            edge_count: 0,
            first_term: 0,
            term_count: 1,
            kind: CompositionNodeKindV3::Native,
            native_outcome: 0,
            recipe_divisor: 1,
            flattened_denominator: 1,
        },
        CompositionNodeInputV3 {
            id: [11; 32],
            rank: 0,
            first_edge: 0,
            edge_count: 0,
            first_term: 1,
            term_count: 1,
            kind: CompositionNodeKindV3::Native,
            native_outcome: 2,
            recipe_divisor: 1,
            flattened_denominator: 1,
        },
        CompositionNodeInputV3 {
            id: [20; 32],
            rank: 1,
            first_edge: 0,
            edge_count: 2,
            first_term: 2,
            term_count: 2,
            kind: CompositionNodeKindV3::Compose,
            native_outcome: 0,
            recipe_divisor: 1,
            flattened_denominator: 1,
        },
        CompositionNodeInputV3 {
            id: ROOT_ID,
            rank: 2,
            first_edge: 2,
            edge_count: 1,
            first_term: 4,
            term_count: 2,
            kind: CompositionNodeKindV3::Compose,
            native_outcome: 0,
            recipe_divisor: 2,
            flattened_denominator: 2,
        },
    ]
}

fn edges() -> [CompositionEdgeInputV3; 3] {
    [
        CompositionEdgeInputV3 {
            child_id: [10; 32],
            child_index: 0,
            coefficient: 1,
        },
        CompositionEdgeInputV3 {
            child_id: [11; 32],
            child_index: 1,
            coefficient: 2,
        },
        CompositionEdgeInputV3 {
            child_id: [20; 32],
            child_index: 2,
            coefficient: 3,
        },
    ]
}

fn terms() -> [SparseTermV3; 6] {
    [
        SparseTermV3 {
            outcome: 0,
            numerator: 1,
        },
        SparseTermV3 {
            outcome: 2,
            numerator: 1,
        },
        SparseTermV3 {
            outcome: 0,
            numerator: 1,
        },
        SparseTermV3 {
            outcome: 2,
            numerator: 2,
        },
        SparseTermV3 {
            outcome: 0,
            numerator: 3,
        },
        SparseTermV3 {
            outcome: 2,
            numerator: 6,
        },
    ]
}

fn rust_witness() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let mut descriptor_scratch = [0_u8; COMPOSITION_DESCRIPTOR_BYTES_V3];
    let mut descriptor = [0_u8; COMPOSITION_DESCRIPTOR_BYTES_V3];
    encode_composition_descriptor_v3_atomic(
        descriptor_input(),
        &mut descriptor_scratch,
        &mut descriptor,
    )
    .expect("safe descriptor encoder");

    let nodes = nodes();
    let edges = edges();
    let terms = terms();
    let graph_len = composition_graph_bytes_v3(4, 3, 6).expect("safe graph width");
    let mut graph_scratch = vec![0_u8; graph_len];
    let mut graph = vec![0_u8; graph_len];
    encode_composition_graph_v3_atomic(
        CompositionGraphInputV3 {
            graph_id: GRAPH_ID,
            root_id: ROOT_ID,
            outcome_count: 3,
            nodes: &nodes,
            edges: &edges,
            terms: &terms,
        },
        &mut graph_scratch,
        &mut graph,
    )
    .expect("safe graph encoder");

    let root_terms = [terms[4], terms[5]];
    let translation_len = composition_translation_bytes_v3(2).expect("safe translation width");
    let mut translation_scratch = vec![0_u8; translation_len];
    let mut translation = vec![0_u8; translation_len];
    encode_canonical_translation_v3_atomic(
        CanonicalTranslationInputV3 {
            graph_id: GRAPH_ID,
            root_id: ROOT_ID,
            outcome_count: 3,
            denominator: 2,
            terms: &root_terms,
        },
        &mut translation_scratch,
        &mut translation,
    )
    .expect("safe translation encoder");
    (descriptor.to_vec(), graph, translation)
}

fn decode(descriptor: &[u8], graph: &[u8], translation: &[u8]) -> Result<(), Error> {
    decode_composition_bundle_v3(
        descriptor,
        admitted(DESCRIPTOR_ID, DESCRIPTOR_DIGEST),
        graph,
        admitted(GRAPH_ID, GRAPH_DIGEST),
        translation,
        admitted(TRANSLATION_ID, TRANSLATION_DIGEST),
    )
    .map(|_| ())
}

#[test]
fn lean_constants_are_the_pinned_composition_coordinates() {
    assert_eq!(generated::COMPOSITION_SCHEMA_VERSION_LEAN_V3, 3);
    assert_eq!(generated::COMPOSITION_MIN_OUTCOMES_LEAN_V3, 2);
    assert_eq!(generated::COMPOSITION_MAX_OUTCOMES_LEAN_V3, 256);
    assert_eq!(generated::COMPOSITION_MAX_NODES_LEAN_V3, 32);
    assert_eq!(generated::COMPOSITION_MAX_EDGES_LEAN_V3, 96);
    assert_eq!(generated::COMPOSITION_MAX_TERMS_LEAN_V3, 2048);
    assert_eq!(generated::COMPOSITION_DESCRIPTOR_BYTES_LEAN_V3, 368);
    assert_eq!(generated::COMPOSITION_GRAPH_HEADER_BYTES_LEAN_V3, 112);
    assert_eq!(generated::COMPOSITION_NODE_BYTES_LEAN_V3, 80);
    assert_eq!(generated::COMPOSITION_EDGE_BYTES_LEAN_V3, 48);
    assert_eq!(generated::COMPOSITION_TERM_BYTES_LEAN_V3, 16);
    assert_eq!(generated::COMPOSITION_TRANSLATION_HEADER_BYTES_LEAN_V3, 128);
    assert_eq!(generated::COMPOSITION_CAPACITY_PROFILE_PREIMAGE_LEAN_V3, b"dclutch/capacity/representation-composition-v3/outcomes256/nodes32/edges96/terms2048/u128");
    assert_eq!(
        generated::COMPOSITION_CAPACITY_PROFILE_ID_LEAN_V3,
        [
            0x48, 0xaa, 0xa1, 0xf4, 0x37, 0xff, 0xda, 0xc9, 0xbf, 0x14, 0xc9, 0xd8, 0xc8, 0xc4,
            0x9c, 0xf3, 0xf7, 0x1e, 0x93, 0x9e, 0x30, 0x39, 0x79, 0x4b, 0xf7, 0xc4, 0x11, 0xa8,
            0xff, 0x8d, 0xb8, 0x78
        ]
    );
    assert_eq!(
        generated::COMPOSITION_DESCRIPTOR_SCHEMA_PREIMAGE_LEAN_V3,
        b"dclutch/schema/representation-composition-descriptor-v3"
    );
    assert_eq!(
        generated::COMPOSITION_DESCRIPTOR_SCHEMA_ID_LEAN_V3,
        [
            0xfa, 0x76, 0x41, 0xfb, 0x0c, 0x60, 0xc1, 0x74, 0xe4, 0x7a, 0x45, 0x69, 0x99, 0x6a,
            0xcc, 0x5d, 0x12, 0x6a, 0x6c, 0x6d, 0xb7, 0xb4, 0xa5, 0xa9, 0x2f, 0x23, 0x86, 0xb5,
            0x49, 0xd9, 0x12, 0x88
        ]
    );
    assert_eq!(
        generated::COMPOSITION_GRAPH_SCHEMA_PREIMAGE_LEAN_V3,
        b"dclutch/schema/representation-composition-graph-v3"
    );
    assert_eq!(
        generated::COMPOSITION_GRAPH_SCHEMA_ID_LEAN_V3,
        [
            0xb3, 0xc5, 0xc7, 0x7b, 0x58, 0x0a, 0x29, 0x6d, 0xf5, 0xf7, 0x59, 0x70, 0x4b, 0x99,
            0x9b, 0xfb, 0x79, 0xc6, 0xc2, 0x39, 0x6c, 0x4c, 0x39, 0xb2, 0xf4, 0xc5, 0x78, 0xc8,
            0x72, 0x11, 0x57, 0x84
        ]
    );
    assert_eq!(
        generated::COMPOSITION_TRANSLATION_SCHEMA_PREIMAGE_LEAN_V3,
        b"dclutch/schema/representation-composition-translation-v3"
    );
    assert_eq!(
        generated::COMPOSITION_TRANSLATION_SCHEMA_ID_LEAN_V3,
        [
            0xd2, 0xc1, 0x0c, 0x1f, 0xe6, 0xd8, 0xfc, 0x09, 0x42, 0x10, 0xca, 0xad, 0x45, 0xd7,
            0x00, 0x34, 0x76, 0xe5, 0x98, 0x8b, 0xe5, 0xa0, 0x69, 0xe8, 0x0c, 0x71, 0xec, 0x30,
            0x0c, 0x2a, 0xe6, 0x41
        ]
    );
    assert_eq!(
        generated::COMPOSITION_DESCRIPTOR_MAGIC_LEAN_V3,
        *b"DCRCDS03"
    );
    assert_eq!(generated::COMPOSITION_GRAPH_MAGIC_LEAN_V3, *b"DCRCDG03");
    assert_eq!(
        generated::COMPOSITION_TRANSLATION_MAGIC_LEAN_V3,
        *b"DCRCDT03"
    );

    assert_eq!(
        [
            generated::COMPOSITION_DESCRIPTOR_MAGIC_OFFSET_V3,
            generated::COMPOSITION_DESCRIPTOR_VERSION_OFFSET_V3,
            generated::COMPOSITION_DESCRIPTOR_RESERVED_HEADER_OFFSET_V3,
            generated::COMPOSITION_DESCRIPTOR_MARKET_OFFSET_V3,
            generated::COMPOSITION_DESCRIPTOR_RESULT_DOMAIN_OFFSET_V3,
            generated::COMPOSITION_DESCRIPTOR_RELEASE_SET_OFFSET_V3,
            generated::COMPOSITION_DESCRIPTOR_NATIVE_BASIS_OFFSET_V3,
            generated::COMPOSITION_DESCRIPTOR_GRAPH_ID_OFFSET_V3,
            generated::COMPOSITION_DESCRIPTOR_GRAPH_DIGEST_OFFSET_V3,
            generated::COMPOSITION_DESCRIPTOR_ROOT_ID_OFFSET_V3,
            generated::COMPOSITION_DESCRIPTOR_TRANSLATION_ID_OFFSET_V3,
            generated::COMPOSITION_DESCRIPTOR_TRANSLATION_DIGEST_OFFSET_V3,
            generated::COMPOSITION_DESCRIPTOR_CAPACITY_PROFILE_OFFSET_V3,
            generated::COMPOSITION_DESCRIPTOR_OUTCOME_COUNT_OFFSET_V3,
            generated::COMPOSITION_DESCRIPTOR_NODE_COUNT_OFFSET_V3,
            generated::COMPOSITION_DESCRIPTOR_EDGE_COUNT_OFFSET_V3,
            generated::COMPOSITION_DESCRIPTOR_TERM_COUNT_OFFSET_V3,
            generated::COMPOSITION_DESCRIPTOR_ROOT_DENOMINATOR_OFFSET_V3,
            generated::COMPOSITION_DESCRIPTOR_RESERVED_TAIL_OFFSET_V3,
        ],
        [
            0, 8, 10, 16, 48, 80, 112, 144, 176, 208, 240, 272, 304, 336, 340, 344, 348, 352, 360
        ]
    );
    assert_eq!(
        [
            generated::COMPOSITION_GRAPH_MAGIC_OFFSET_V3,
            generated::COMPOSITION_GRAPH_VERSION_OFFSET_V3,
            generated::COMPOSITION_GRAPH_RESERVED_HEADER_OFFSET_V3,
            generated::COMPOSITION_GRAPH_ID_OFFSET_V3,
            generated::COMPOSITION_GRAPH_ROOT_ID_OFFSET_V3,
            generated::COMPOSITION_GRAPH_OUTCOME_COUNT_OFFSET_V3,
            generated::COMPOSITION_GRAPH_NODE_COUNT_OFFSET_V3,
            generated::COMPOSITION_GRAPH_EDGE_COUNT_OFFSET_V3,
            generated::COMPOSITION_GRAPH_TERM_COUNT_OFFSET_V3,
            generated::COMPOSITION_GRAPH_ROOT_INDEX_OFFSET_V3,
            generated::COMPOSITION_GRAPH_RESERVED_TAIL_OFFSET_V3,
        ],
        [0, 8, 10, 16, 48, 80, 84, 88, 92, 96, 100]
    );
    assert_eq!(
        [
            generated::COMPOSITION_NODE_ID_OFFSET_V3,
            generated::COMPOSITION_NODE_RANK_OFFSET_V3,
            generated::COMPOSITION_NODE_FIRST_EDGE_OFFSET_V3,
            generated::COMPOSITION_NODE_EDGE_COUNT_OFFSET_V3,
            generated::COMPOSITION_NODE_FIRST_TERM_OFFSET_V3,
            generated::COMPOSITION_NODE_TERM_COUNT_OFFSET_V3,
            generated::COMPOSITION_NODE_KIND_OFFSET_V3,
            generated::COMPOSITION_NODE_RESERVED_KIND_OFFSET_V3,
            generated::COMPOSITION_NODE_NATIVE_OUTCOME_OFFSET_V3,
            generated::COMPOSITION_NODE_RESERVED_SCALAR_OFFSET_V3,
            generated::COMPOSITION_NODE_RECIPE_DIVISOR_OFFSET_V3,
            generated::COMPOSITION_NODE_FLATTENED_DENOMINATOR_OFFSET_V3,
        ],
        [0, 32, 36, 40, 44, 48, 52, 53, 56, 60, 64, 72]
    );
    assert_eq!(
        [
            generated::COMPOSITION_EDGE_CHILD_ID_OFFSET_V3,
            generated::COMPOSITION_EDGE_CHILD_INDEX_OFFSET_V3,
            generated::COMPOSITION_EDGE_RESERVED_OFFSET_V3,
            generated::COMPOSITION_EDGE_COEFFICIENT_OFFSET_V3,
        ],
        [0, 32, 36, 40]
    );
    assert_eq!(
        [
            generated::COMPOSITION_TERM_OUTCOME_OFFSET_V3,
            generated::COMPOSITION_TERM_RESERVED_OFFSET_V3,
            generated::COMPOSITION_TERM_NUMERATOR_OFFSET_V3,
        ],
        [0, 4, 8]
    );
    assert_eq!(
        [
            generated::COMPOSITION_TRANSLATION_MAGIC_OFFSET_V3,
            generated::COMPOSITION_TRANSLATION_VERSION_OFFSET_V3,
            generated::COMPOSITION_TRANSLATION_RESERVED_HEADER_OFFSET_V3,
            generated::COMPOSITION_TRANSLATION_GRAPH_ID_OFFSET_V3,
            generated::COMPOSITION_TRANSLATION_ROOT_ID_OFFSET_V3,
            generated::COMPOSITION_TRANSLATION_OUTCOME_COUNT_OFFSET_V3,
            generated::COMPOSITION_TRANSLATION_TERM_COUNT_OFFSET_V3,
            generated::COMPOSITION_TRANSLATION_DENOMINATOR_OFFSET_V3,
            generated::COMPOSITION_TRANSLATION_RESERVED_TAIL_OFFSET_V3,
        ],
        [0, 8, 10, 16, 48, 80, 84, 88, 96]
    );
}

#[test]
fn lean_witness_is_byte_identical_to_safe_rust_and_conserves_exactly() {
    let (descriptor, graph, translation) = rust_witness();
    assert_eq!(
        descriptor,
        generated::COMPOSITION_DESCRIPTOR_WITNESS_LEAN_V3
    );
    assert_eq!(graph, generated::COMPOSITION_GRAPH_WITNESS_LEAN_V3);
    assert_eq!(
        translation,
        generated::COMPOSITION_TRANSLATION_WITNESS_LEAN_V3
    );

    let bundle = decode_composition_bundle_v3(
        &descriptor,
        admitted(DESCRIPTOR_ID, DESCRIPTOR_DIGEST),
        &graph,
        admitted(GRAPH_ID, GRAPH_DIGEST),
        &translation,
        admitted(TRANSLATION_ID, TRANSLATION_DIGEST),
    )
    .expect("Lean witness is admitted by safe Rust");
    let mut scratch = [u64::MAX; 3];
    let mut output = [u64::MAX; 3];
    bundle
        .translation()
        .materialize_exact(2, &mut scratch, &mut output)
        .expect("checked exact materialization");
    assert_eq!(output, [3, 0, 6]);
    assert_eq!(bundle.translation().verify_conservation(2, &output), Ok(()));
}

#[test]
fn lean_hostile_corpus_is_refused_by_the_safe_rust_decoder() {
    let (descriptor, graph, translation) = rust_witness();
    assert_eq!(
        CompositionDescriptorV3::decode(
            &generated::COMPOSITION_DESCRIPTOR_RESERVED_REFUSAL_LEAN_V3,
            admitted(DESCRIPTOR_ID, DESCRIPTOR_DIGEST),
        ),
        Err(Error::NonCanonical)
    );
    assert_eq!(
        decode(
            &descriptor,
            &generated::COMPOSITION_GRAPH_CYCLE_REFUSAL_LEAN_V3,
            &translation,
        ),
        Err(Error::InvalidEdge)
    );
    assert_eq!(
        decode(
            &descriptor,
            &generated::COMPOSITION_GRAPH_DUPLICATE_NODE_REFUSAL_LEAN_V3,
            &translation,
        ),
        Err(Error::DuplicateOrUnorderedNode)
    );
    assert_eq!(
        decode(
            &descriptor,
            &generated::COMPOSITION_GRAPH_AMBIGUOUS_ROOT_REFUSAL_LEAN_V3,
            &translation,
        ),
        Err(Error::AmbiguousRoot)
    );
    assert_eq!(
        decode(
            &descriptor,
            &graph,
            &generated::COMPOSITION_TRANSLATION_MISMATCH_REFUSAL_LEAN_V3,
        ),
        Err(Error::TranslationMismatch)
    );
    assert_eq!(
        decode(
            &descriptor,
            &graph,
            &generated::COMPOSITION_TRANSLATION_RESERVED_REFUSAL_LEAN_V3,
        ),
        Err(Error::NonCanonical)
    );
}
