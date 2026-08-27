//! Byte agreement between Lean-owned fixed coordinates and the safe Rust kernel.

#[allow(missing_docs)]
#[path = "../src/generated_abi.rs"]
mod generated;

use dclutch_representation_composition_v3_kernel::{
    CAPACITY_PROFILE_ID_V3, CAPACITY_PROFILE_PREIMAGE_V3, COMPOSITION_DESCRIPTOR_BYTES_V3,
    COMPOSITION_DESCRIPTOR_MAGIC_V3, COMPOSITION_DESCRIPTOR_SCHEMA_ID_V3,
    COMPOSITION_DESCRIPTOR_SCHEMA_PREIMAGE_V3, COMPOSITION_EDGE_BYTES_V3,
    COMPOSITION_GRAPH_HEADER_BYTES_V3, COMPOSITION_GRAPH_MAGIC_V3, COMPOSITION_GRAPH_SCHEMA_ID_V3,
    COMPOSITION_GRAPH_SCHEMA_PREIMAGE_V3, COMPOSITION_NODE_BYTES_V3, COMPOSITION_SCHEMA_VERSION_V3,
    COMPOSITION_TERM_BYTES_V3, COMPOSITION_TRANSLATION_HEADER_BYTES_V3,
    COMPOSITION_TRANSLATION_MAGIC_V3, COMPOSITION_TRANSLATION_SCHEMA_ID_V3,
    COMPOSITION_TRANSLATION_SCHEMA_PREIMAGE_V3, CanonicalTranslationInputV3,
    CompositionDescriptorInputV3, CompositionDescriptorV3, CompositionEdgeInputV3,
    CompositionGraphInputV3, CompositionNodeInputV3, CompositionNodeKindV3, DescriptorLayoutV3,
    EdgeLayoutV3, Error, GraphLayoutV3, MAX_COMPOSITION_EDGES_V3, MAX_COMPOSITION_NODES_V3,
    MAX_COMPOSITION_OUTCOMES_V3, MAX_COMPOSITION_TERMS_V3, MIN_COMPOSITION_OUTCOMES_V3,
    NodeLayoutV3, RecordAdmissionV3, SparseTermV3, TermLayoutV3, TranslationLayoutV3,
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
fn lean_constants_equal_live_rust_coordinates() {
    assert_eq!(
        generated::COMPOSITION_SCHEMA_VERSION_LEAN_V3,
        COMPOSITION_SCHEMA_VERSION_V3
    );
    assert_eq!(
        generated::COMPOSITION_MIN_OUTCOMES_LEAN_V3,
        MIN_COMPOSITION_OUTCOMES_V3
    );
    assert_eq!(
        generated::COMPOSITION_MAX_OUTCOMES_LEAN_V3,
        MAX_COMPOSITION_OUTCOMES_V3
    );
    assert_eq!(
        generated::COMPOSITION_MAX_NODES_LEAN_V3,
        MAX_COMPOSITION_NODES_V3
    );
    assert_eq!(
        generated::COMPOSITION_MAX_EDGES_LEAN_V3,
        MAX_COMPOSITION_EDGES_V3
    );
    assert_eq!(
        generated::COMPOSITION_MAX_TERMS_LEAN_V3,
        MAX_COMPOSITION_TERMS_V3
    );
    assert_eq!(
        generated::COMPOSITION_DESCRIPTOR_BYTES_LEAN_V3,
        COMPOSITION_DESCRIPTOR_BYTES_V3
    );
    assert_eq!(
        generated::COMPOSITION_GRAPH_HEADER_BYTES_LEAN_V3,
        COMPOSITION_GRAPH_HEADER_BYTES_V3
    );
    assert_eq!(
        generated::COMPOSITION_NODE_BYTES_LEAN_V3,
        COMPOSITION_NODE_BYTES_V3
    );
    assert_eq!(
        generated::COMPOSITION_EDGE_BYTES_LEAN_V3,
        COMPOSITION_EDGE_BYTES_V3
    );
    assert_eq!(
        generated::COMPOSITION_TERM_BYTES_LEAN_V3,
        COMPOSITION_TERM_BYTES_V3
    );
    assert_eq!(
        generated::COMPOSITION_TRANSLATION_HEADER_BYTES_LEAN_V3,
        COMPOSITION_TRANSLATION_HEADER_BYTES_V3
    );
    assert_eq!(
        generated::COMPOSITION_CAPACITY_PROFILE_PREIMAGE_LEAN_V3,
        CAPACITY_PROFILE_PREIMAGE_V3
    );
    assert_eq!(
        generated::COMPOSITION_CAPACITY_PROFILE_ID_LEAN_V3,
        CAPACITY_PROFILE_ID_V3
    );
    assert_eq!(
        generated::COMPOSITION_DESCRIPTOR_SCHEMA_PREIMAGE_LEAN_V3,
        COMPOSITION_DESCRIPTOR_SCHEMA_PREIMAGE_V3
    );
    assert_eq!(
        generated::COMPOSITION_DESCRIPTOR_SCHEMA_ID_LEAN_V3,
        COMPOSITION_DESCRIPTOR_SCHEMA_ID_V3
    );
    assert_eq!(
        generated::COMPOSITION_GRAPH_SCHEMA_PREIMAGE_LEAN_V3,
        COMPOSITION_GRAPH_SCHEMA_PREIMAGE_V3
    );
    assert_eq!(
        generated::COMPOSITION_GRAPH_SCHEMA_ID_LEAN_V3,
        COMPOSITION_GRAPH_SCHEMA_ID_V3
    );
    assert_eq!(
        generated::COMPOSITION_TRANSLATION_SCHEMA_PREIMAGE_LEAN_V3,
        COMPOSITION_TRANSLATION_SCHEMA_PREIMAGE_V3
    );
    assert_eq!(
        generated::COMPOSITION_TRANSLATION_SCHEMA_ID_LEAN_V3,
        COMPOSITION_TRANSLATION_SCHEMA_ID_V3
    );
    assert_eq!(
        generated::COMPOSITION_DESCRIPTOR_MAGIC_LEAN_V3,
        COMPOSITION_DESCRIPTOR_MAGIC_V3
    );
    assert_eq!(
        generated::COMPOSITION_GRAPH_MAGIC_LEAN_V3,
        COMPOSITION_GRAPH_MAGIC_V3
    );
    assert_eq!(
        generated::COMPOSITION_TRANSLATION_MAGIC_LEAN_V3,
        COMPOSITION_TRANSLATION_MAGIC_V3
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
            DescriptorLayoutV3::MAGIC,
            DescriptorLayoutV3::VERSION,
            DescriptorLayoutV3::RESERVED_HEADER,
            DescriptorLayoutV3::MARKET,
            DescriptorLayoutV3::RESULT_DOMAIN,
            DescriptorLayoutV3::RELEASE_SET,
            DescriptorLayoutV3::NATIVE_BASIS,
            DescriptorLayoutV3::GRAPH_ID,
            DescriptorLayoutV3::GRAPH_DIGEST,
            DescriptorLayoutV3::ROOT_ID,
            DescriptorLayoutV3::TRANSLATION_ID,
            DescriptorLayoutV3::TRANSLATION_DIGEST,
            DescriptorLayoutV3::CAPACITY_PROFILE,
            DescriptorLayoutV3::OUTCOME_COUNT,
            DescriptorLayoutV3::NODE_COUNT,
            DescriptorLayoutV3::EDGE_COUNT,
            DescriptorLayoutV3::TERM_COUNT,
            DescriptorLayoutV3::ROOT_DENOMINATOR,
            DescriptorLayoutV3::RESERVED_TAIL,
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
        [
            GraphLayoutV3::MAGIC,
            GraphLayoutV3::VERSION,
            GraphLayoutV3::RESERVED_HEADER,
            GraphLayoutV3::GRAPH_ID,
            GraphLayoutV3::ROOT_ID,
            GraphLayoutV3::OUTCOME_COUNT,
            GraphLayoutV3::NODE_COUNT,
            GraphLayoutV3::EDGE_COUNT,
            GraphLayoutV3::TERM_COUNT,
            GraphLayoutV3::ROOT_INDEX,
            GraphLayoutV3::RESERVED_TAIL,
        ]
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
        [
            NodeLayoutV3::ID,
            NodeLayoutV3::RANK,
            NodeLayoutV3::FIRST_EDGE,
            NodeLayoutV3::EDGE_COUNT,
            NodeLayoutV3::FIRST_TERM,
            NodeLayoutV3::TERM_COUNT,
            NodeLayoutV3::KIND,
            NodeLayoutV3::RESERVED_KIND,
            NodeLayoutV3::NATIVE_OUTCOME,
            NodeLayoutV3::RESERVED_SCALAR,
            NodeLayoutV3::RECIPE_DIVISOR,
            NodeLayoutV3::FLATTENED_DENOMINATOR,
        ]
    );
    assert_eq!(
        [
            generated::COMPOSITION_EDGE_CHILD_ID_OFFSET_V3,
            generated::COMPOSITION_EDGE_CHILD_INDEX_OFFSET_V3,
            generated::COMPOSITION_EDGE_RESERVED_OFFSET_V3,
            generated::COMPOSITION_EDGE_COEFFICIENT_OFFSET_V3,
        ],
        [
            EdgeLayoutV3::CHILD_ID,
            EdgeLayoutV3::CHILD_INDEX,
            EdgeLayoutV3::RESERVED,
            EdgeLayoutV3::COEFFICIENT,
        ]
    );
    assert_eq!(
        [
            generated::COMPOSITION_TERM_OUTCOME_OFFSET_V3,
            generated::COMPOSITION_TERM_RESERVED_OFFSET_V3,
            generated::COMPOSITION_TERM_NUMERATOR_OFFSET_V3,
        ],
        [
            TermLayoutV3::OUTCOME,
            TermLayoutV3::RESERVED,
            TermLayoutV3::NUMERATOR,
        ]
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
        [
            TranslationLayoutV3::MAGIC,
            TranslationLayoutV3::VERSION,
            TranslationLayoutV3::RESERVED_HEADER,
            TranslationLayoutV3::GRAPH_ID,
            TranslationLayoutV3::ROOT_ID,
            TranslationLayoutV3::OUTCOME_COUNT,
            TranslationLayoutV3::TERM_COUNT,
            TranslationLayoutV3::DENOMINATOR,
            TranslationLayoutV3::RESERVED_TAIL,
        ]
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
