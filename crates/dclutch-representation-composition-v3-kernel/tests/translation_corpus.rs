//! Fixed-layout canonical and hostile representation-composition corpus.

use dclutch_representation_composition_v3_kernel::{
    CAPACITY_PROFILE_ID_V3, COMPOSITION_DESCRIPTOR_BYTES_V3, COMPOSITION_DESCRIPTOR_MAGIC_V3,
    COMPOSITION_EDGE_BYTES_V3, COMPOSITION_GRAPH_HEADER_BYTES_V3, COMPOSITION_NODE_BYTES_V3,
    COMPOSITION_TERM_BYTES_V3, COMPOSITION_TRANSLATION_HEADER_BYTES_V3,
    CanonicalTranslationInputV3, CompositionDescriptorInputV3, CompositionEdgeInputV3,
    CompositionGraphInputV3, CompositionNodeInputV3, CompositionNodeKindV3, DescriptorLayoutV3,
    EdgeLayoutV3, Error, GraphLayoutV3, NodeLayoutV3, RecordAdmissionV3, SparseTermV3,
    TermLayoutV3, TranslationLayoutV3, composition_graph_bytes_v3,
    composition_translation_bytes_v3, decode_composition_bundle_v3,
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

struct Corpus {
    descriptor: [u8; COMPOSITION_DESCRIPTOR_BYTES_V3],
    graph: Vec<u8>,
    translation: Vec<u8>,
}

fn canonical_corpus() -> Corpus {
    let mut descriptor_scratch = [0_u8; COMPOSITION_DESCRIPTOR_BYTES_V3];
    let mut descriptor = [0_u8; COMPOSITION_DESCRIPTOR_BYTES_V3];
    encode_composition_descriptor_v3_atomic(
        descriptor_input(),
        &mut descriptor_scratch,
        &mut descriptor,
    )
    .expect("canonical descriptor");

    let nodes = nodes();
    let edges = edges();
    let terms = terms();
    let graph_length = composition_graph_bytes_v3(4, 3, 6).expect("graph width");
    let mut graph_scratch = vec![0_u8; graph_length];
    let mut graph = vec![0_u8; graph_length];
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
    .expect("canonical graph");

    let translation_terms = [terms[4], terms[5]];
    let translation_length = composition_translation_bytes_v3(2).expect("translation width");
    let mut translation_scratch = vec![0_u8; translation_length];
    let mut translation = vec![0_u8; translation_length];
    encode_canonical_translation_v3_atomic(
        CanonicalTranslationInputV3 {
            graph_id: GRAPH_ID,
            root_id: ROOT_ID,
            outcome_count: 3,
            denominator: 2,
            terms: &translation_terms,
        },
        &mut translation_scratch,
        &mut translation,
    )
    .expect("canonical translation");
    Corpus {
        descriptor,
        graph,
        translation,
    }
}

fn decode(corpus: &Corpus) -> Result<(), Error> {
    decode_composition_bundle_v3(
        &corpus.descriptor,
        admitted(DESCRIPTOR_ID, DESCRIPTOR_DIGEST),
        &corpus.graph,
        admitted(GRAPH_ID, GRAPH_DIGEST),
        &corpus.translation,
        admitted(TRANSLATION_ID, TRANSLATION_DIGEST),
    )
    .map(|_| ())
}

fn graph_node_offset(index: usize) -> usize {
    COMPOSITION_GRAPH_HEADER_BYTES_V3 + index * COMPOSITION_NODE_BYTES_V3
}

fn graph_edge_offset(index: usize) -> usize {
    COMPOSITION_GRAPH_HEADER_BYTES_V3
        + 4 * COMPOSITION_NODE_BYTES_V3
        + index * COMPOSITION_EDGE_BYTES_V3
}

fn graph_term_offset(index: usize) -> usize {
    COMPOSITION_GRAPH_HEADER_BYTES_V3
        + 4 * COMPOSITION_NODE_BYTES_V3
        + 3 * COMPOSITION_EDGE_BYTES_V3
        + index * COMPOSITION_TERM_BYTES_V3
}

fn segment(input: &[u8], offset: usize, length: usize) -> &[u8] {
    input
        .get(offset..offset + length)
        .expect("fixed corpus segment")
}

fn segment_mut(input: &mut [u8], offset: usize, length: usize) -> &mut [u8] {
    input
        .get_mut(offset..offset + length)
        .expect("fixed corpus segment")
}

fn byte_mut(input: &mut [u8], offset: usize) -> &mut u8 {
    input.get_mut(offset).expect("fixed corpus byte")
}

#[test]
fn canonical_fixed_layout_and_exact_translation() {
    let corpus = canonical_corpus();
    assert_eq!(
        &corpus.descriptor[DescriptorLayoutV3::MAGIC..DescriptorLayoutV3::MAGIC + 8],
        &COMPOSITION_DESCRIPTOR_MAGIC_V3
    );
    assert_eq!(
        &corpus.descriptor
            [DescriptorLayoutV3::CAPACITY_PROFILE..DescriptorLayoutV3::CAPACITY_PROFILE + 32],
        &CAPACITY_PROFILE_ID_V3
    );
    assert_eq!(corpus.graph.len(), 672);
    assert_eq!(corpus.translation.len(), 160);
    assert_eq!(
        segment(&corpus.graph, graph_term_offset(4), 32),
        segment(
            &corpus.translation,
            COMPOSITION_TRANSLATION_HEADER_BYTES_V3,
            32
        )
    );

    let bundle = decode_composition_bundle_v3(
        &corpus.descriptor,
        admitted(DESCRIPTOR_ID, DESCRIPTOR_DIGEST),
        &corpus.graph,
        admitted(GRAPH_ID, GRAPH_DIGEST),
        &corpus.translation,
        admitted(TRANSLATION_ID, TRANSLATION_DIGEST),
    )
    .expect("admitted bundle");
    assert_eq!(bundle.descriptor().market(), [1; 32]);
    assert_eq!(bundle.graph().root_denominator(), Ok(2));
    let mut scratch = [99_u64; 3];
    let mut output = [88_u64; 3];
    bundle
        .translation()
        .materialize_exact(2, &mut scratch, &mut output)
        .expect("integral root translation");
    assert_eq!(output, [3, 0, 6]);
    assert_eq!(bundle.translation().verify_conservation(2, &output), Ok(()));

    let prior = output;
    assert_eq!(
        bundle
            .translation()
            .materialize_exact(1, &mut scratch, &mut output),
        Err(Error::NonIntegralTranslation)
    );
    assert_eq!(output, prior);
}

#[test]
fn content_admission_and_same_width_substitution_are_refused() {
    let mut corpus = canonical_corpus();
    corpus.descriptor[DescriptorLayoutV3::MARKET] ^= 1;
    let result = decode_composition_bundle_v3(
        &corpus.descriptor,
        RecordAdmissionV3 {
            recomputed_digest: [92; 32],
            ..admitted(DESCRIPTOR_ID, DESCRIPTOR_DIGEST)
        },
        &corpus.graph,
        admitted(GRAPH_ID, GRAPH_DIGEST),
        &corpus.translation,
        admitted(TRANSLATION_ID, TRANSLATION_DIGEST),
    );
    assert!(matches!(result, Err(Error::ContentAdmission)));

    let mut corpus = canonical_corpus();
    *byte_mut(&mut corpus.graph, GraphLayoutV3::GRAPH_ID) ^= 1;
    assert_eq!(decode(&corpus), Err(Error::CompositionMismatch));

    let mut corpus = canonical_corpus();
    let offset = COMPOSITION_TRANSLATION_HEADER_BYTES_V3
        + COMPOSITION_TERM_BYTES_V3
        + TermLayoutV3::NUMERATOR;
    *byte_mut(&mut corpus.translation, offset) = 7;
    assert_eq!(decode(&corpus), Err(Error::TranslationMismatch));

    let mut corpus = canonical_corpus();
    *byte_mut(&mut corpus.translation, TranslationLayoutV3::RESERVED_TAIL) = 1;
    assert_eq!(decode(&corpus), Err(Error::NonCanonical));
}

#[test]
fn cycles_duplicate_nodes_and_root_substitution_are_refused() {
    let mut corpus = canonical_corpus();
    let edge = graph_edge_offset(2);
    segment_mut(&mut corpus.graph, edge + EdgeLayoutV3::CHILD_ID, 32).copy_from_slice(&ROOT_ID);
    segment_mut(&mut corpus.graph, edge + EdgeLayoutV3::CHILD_INDEX, 4)
        .copy_from_slice(&3_u32.to_le_bytes());
    assert_eq!(decode(&corpus), Err(Error::InvalidEdge));

    let mut corpus = canonical_corpus();
    let node0 = graph_node_offset(0);
    let node1 = graph_node_offset(1);
    let duplicate = segment(&corpus.graph, node0 + NodeLayoutV3::ID, 32).to_vec();
    segment_mut(&mut corpus.graph, node1 + NodeLayoutV3::ID, 32).copy_from_slice(&duplicate);
    assert_eq!(decode(&corpus), Err(Error::DuplicateOrUnorderedNode));

    let mut corpus = canonical_corpus();
    let node2 = graph_node_offset(2);
    segment_mut(&mut corpus.graph, node2 + NodeLayoutV3::ID, 32).copy_from_slice(&[10; 32]);
    assert_eq!(decode(&corpus), Err(Error::DuplicateOrUnorderedNode));

    let mut corpus = canonical_corpus();
    segment_mut(&mut corpus.graph, GraphLayoutV3::ROOT_ID, 32).copy_from_slice(&[20; 32]);
    assert_eq!(decode(&corpus), Err(Error::AmbiguousRoot));
}

#[test]
fn disconnected_node_and_noncanonical_recipe_are_refused_atomically() {
    let disconnected_nodes = [
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
            native_outcome: 1,
            recipe_divisor: 1,
            flattened_denominator: 1,
        },
        CompositionNodeInputV3 {
            id: ROOT_ID,
            rank: 1,
            first_edge: 0,
            edge_count: 1,
            first_term: 2,
            term_count: 1,
            kind: CompositionNodeKindV3::Compose,
            native_outcome: 0,
            recipe_divisor: 1,
            flattened_denominator: 1,
        },
    ];
    let disconnected_edges = [CompositionEdgeInputV3 {
        child_id: [10; 32],
        child_index: 0,
        coefficient: 1,
    }];
    let disconnected_terms = [
        SparseTermV3 {
            outcome: 0,
            numerator: 1,
        },
        SparseTermV3 {
            outcome: 1,
            numerator: 1,
        },
        SparseTermV3 {
            outcome: 0,
            numerator: 1,
        },
    ];
    let length = composition_graph_bytes_v3(3, 1, 3).expect("disconnected width");
    let mut scratch = vec![0_u8; length];
    let mut output = vec![0xa5_u8; length];
    let result = encode_composition_graph_v3_atomic(
        CompositionGraphInputV3 {
            graph_id: GRAPH_ID,
            root_id: ROOT_ID,
            outcome_count: 2,
            nodes: &disconnected_nodes,
            edges: &disconnected_edges,
            terms: &disconnected_terms,
        },
        &mut scratch,
        &mut output,
    );
    assert_eq!(result, Err(Error::AmbiguousRoot));
    assert!(output.iter().all(|value| *value == 0xa5));

    let mut noncanonical_edges = edges();
    noncanonical_edges[0].coefficient = 2;
    noncanonical_edges[1].coefficient = 4;
    let mut noncanonical_nodes = nodes();
    noncanonical_nodes[2].recipe_divisor = 2;
    let canonical_terms = terms();
    let length = composition_graph_bytes_v3(4, 3, 6).expect("graph width");
    let mut scratch = vec![0_u8; length];
    let mut output = vec![0x5a_u8; length];
    let result = encode_composition_graph_v3_atomic(
        CompositionGraphInputV3 {
            graph_id: GRAPH_ID,
            root_id: ROOT_ID,
            outcome_count: 3,
            nodes: &noncanonical_nodes,
            edges: &noncanonical_edges,
            terms: &canonical_terms,
        },
        &mut scratch,
        &mut output,
    );
    assert_eq!(result, Err(Error::InvalidNode));
    assert!(output.iter().all(|value| *value == 0x5a));
}

#[test]
fn payoff_omission_reordering_and_reducible_forms_are_refused() {
    let mut corpus = canonical_corpus();
    let root_second = graph_term_offset(5);
    segment_mut(&mut corpus.graph, root_second + TermLayoutV3::OUTCOME, 4)
        .copy_from_slice(&0_u32.to_le_bytes());
    assert_eq!(decode(&corpus), Err(Error::NonCanonicalPayoff));

    let mut corpus = canonical_corpus();
    *byte_mut(&mut corpus.graph, root_second + TermLayoutV3::NUMERATOR) = 5;
    assert_eq!(decode(&corpus), Err(Error::CompositionMismatch));

    let mut corpus = canonical_corpus();
    let root = graph_node_offset(3);
    segment_mut(
        &mut corpus.graph,
        root + NodeLayoutV3::FLATTENED_DENOMINATOR,
        8,
    )
    .copy_from_slice(&4_u64.to_le_bytes());
    let root_first = graph_term_offset(4);
    segment_mut(&mut corpus.graph, root_first + TermLayoutV3::NUMERATOR, 8)
        .copy_from_slice(&6_u64.to_le_bytes());
    segment_mut(&mut corpus.graph, root_second + TermLayoutV3::NUMERATOR, 8)
        .copy_from_slice(&12_u64.to_le_bytes());
    assert_eq!(decode(&corpus), Err(Error::NonCanonicalPayoff));
}

#[test]
fn checked_u128_overflow_is_an_explicit_capacity_refusal() {
    let max = u64::MAX;
    let nodes = [
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
            native_outcome: 1,
            recipe_divisor: 1,
            flattened_denominator: 1,
        },
        CompositionNodeInputV3 {
            id: [20; 32],
            rank: 1,
            first_edge: 0,
            edge_count: 1,
            first_term: 2,
            term_count: 1,
            kind: CompositionNodeKindV3::Compose,
            native_outcome: 0,
            recipe_divisor: max,
            flattened_denominator: max,
        },
        CompositionNodeInputV3 {
            id: [21; 32],
            rank: 1,
            first_edge: 1,
            edge_count: 1,
            first_term: 3,
            term_count: 1,
            kind: CompositionNodeKindV3::Compose,
            native_outcome: 0,
            recipe_divisor: max - 1,
            flattened_denominator: max - 1,
        },
        CompositionNodeInputV3 {
            id: ROOT_ID,
            rank: 2,
            first_edge: 2,
            edge_count: 2,
            first_term: 4,
            term_count: 1,
            kind: CompositionNodeKindV3::Compose,
            native_outcome: 0,
            recipe_divisor: 2,
            flattened_denominator: 1,
        },
    ];
    let edges = [
        CompositionEdgeInputV3 {
            child_id: [10; 32],
            child_index: 0,
            coefficient: 1,
        },
        CompositionEdgeInputV3 {
            child_id: [11; 32],
            child_index: 1,
            coefficient: 1,
        },
        CompositionEdgeInputV3 {
            child_id: [20; 32],
            child_index: 2,
            coefficient: 1,
        },
        CompositionEdgeInputV3 {
            child_id: [21; 32],
            child_index: 3,
            coefficient: 1,
        },
    ];
    let terms = [
        SparseTermV3 {
            outcome: 0,
            numerator: 1,
        },
        SparseTermV3 {
            outcome: 1,
            numerator: 1,
        },
        SparseTermV3 {
            outcome: 0,
            numerator: 1,
        },
        SparseTermV3 {
            outcome: 1,
            numerator: 1,
        },
        SparseTermV3 {
            outcome: 0,
            numerator: 1,
        },
    ];
    let length = composition_graph_bytes_v3(5, 4, 5).expect("overflow graph width");
    let mut scratch = vec![0_u8; length];
    let mut output = vec![0xcc_u8; length];
    assert_eq!(
        encode_composition_graph_v3_atomic(
            CompositionGraphInputV3 {
                graph_id: GRAPH_ID,
                root_id: ROOT_ID,
                outcome_count: 2,
                nodes: &nodes,
                edges: &edges,
                terms: &terms,
            },
            &mut scratch,
            &mut output,
        ),
        Err(Error::ArithmeticOverflow)
    );
    assert!(output.iter().all(|value| *value == 0xcc));
}

#[test]
fn exact_width_capacity_and_reserved_bytes_are_refused() {
    let mut corpus = canonical_corpus();
    corpus.graph.truncate(corpus.graph.len() - 1);
    assert_eq!(decode(&corpus), Err(Error::InvalidLength));

    let mut corpus = canonical_corpus();
    *byte_mut(&mut corpus.graph, GraphLayoutV3::RESERVED_TAIL) = 1;
    assert_eq!(decode(&corpus), Err(Error::NonCanonical));

    assert_eq!(
        composition_graph_bytes_v3(33, 0, 1),
        Err(Error::CapacityExceeded)
    );
    assert_eq!(
        composition_translation_bytes_v3(2_049),
        Err(Error::CapacityExceeded)
    );
}
