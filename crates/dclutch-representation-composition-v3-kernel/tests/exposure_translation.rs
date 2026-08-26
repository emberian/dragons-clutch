//! Hostile translation corpus for Product-to-Claims exposure bundles.

use dclutch_representation_composition_v3_kernel::{
    COMPOSITION_DESCRIPTOR_BYTES_V3, COMPOSITION_EXPOSURE_HEADER_BYTES_V3,
    COMPOSITION_EXPOSURE_ROW_BYTES_V3, CompositionDescriptorInputV3, CompositionDescriptorV3,
    CompositionEdgeInputV3, CompositionExposureBundleV3, CompositionExposureExecutionExpectedV3,
    CompositionExposureExpectedV3, CompositionExposureInputV3, CompositionExposureLayoutV3,
    CompositionExposureRowInputV3, CompositionExposureRowLayoutV3, CompositionExposureTermV3,
    CompositionGraphInputV3, CompositionGraphV3, CompositionNodeInputV3, CompositionNodeKindV3,
    Error, RecordAdmissionV3, SparseTermV3, composition_exposure_bytes_v3,
    composition_graph_bytes_v3, encode_composition_descriptor_v3_atomic,
    encode_composition_exposure_v3_atomic, encode_composition_graph_v3_atomic,
};

const MARKET: [u8; 32] = [1; 32];
const DOMAIN: [u8; 32] = [2; 32];
const RELEASE: [u8; 32] = [3; 32];
const PRODUCT_BASIS: [u8; 32] = [4; 32];
const REPRESENTATION_BASIS: [u8; 32] = [5; 32];
const GRAPH: [u8; 32] = [6; 32];

fn admission() -> RecordAdmissionV3 {
    RecordAdmissionV3 {
        selected_id: [7; 32],
        finalized_id: [7; 32],
        recomputed_digest: [8; 32],
        finalized_digest: [8; 32],
        record_authenticated: true,
    }
}

fn expected(product_width: u32) -> CompositionExposureExpectedV3 {
    CompositionExposureExpectedV3 {
        market: MARKET,
        result_domain: DOMAIN,
        release_set: RELEASE,
        product_basis: PRODUCT_BASIS,
        representation_basis: REPRESENTATION_BASIS,
        graph_id: GRAPH,
        product_width,
        representation_width: 3,
    }
}

fn execution_expected(product_width: u32) -> CompositionExposureExecutionExpectedV3 {
    CompositionExposureExecutionExpectedV3 {
        market: MARKET,
        result_domain: DOMAIN,
        release_set: RELEASE,
        product_basis: PRODUCT_BASIS,
        representation_basis: REPRESENTATION_BASIS,
        product_width,
        representation_width: 3,
    }
}

fn encode(product_width: u32, rows: &[CompositionExposureRowInputV3<'_>]) -> Vec<u8> {
    let term_count = rows.iter().map(|row| row.terms.len()).sum::<usize>();
    let length = composition_exposure_bytes_v3(
        u32::try_from(rows.len()).expect("row count"),
        u32::try_from(term_count).expect("term count"),
    )
    .expect("bundle width");
    let mut scratch = vec![0; length];
    let mut output = vec![0; length];
    encode_composition_exposure_v3_atomic(
        CompositionExposureInputV3 {
            market: MARKET,
            result_domain: DOMAIN,
            release_set: RELEASE,
            product_basis: PRODUCT_BASIS,
            representation_basis: REPRESENTATION_BASIS,
            graph_id: GRAPH,
            product_width,
            rows,
        },
        &mut scratch,
        &mut output,
    )
    .expect("canonical exposure bundle");
    output
}

fn admitted(selected_id: [u8; 32], digest: [u8; 32]) -> RecordAdmissionV3 {
    RecordAdmissionV3 {
        selected_id,
        finalized_id: selected_id,
        recomputed_digest: digest,
        finalized_digest: digest,
        record_authenticated: true,
    }
}

fn composition_graph() -> (Vec<u8>, CompositionDescriptorV3) {
    const GRAPH_DIGEST: [u8; 32] = [71; 32];
    const ROOT: [u8; 32] = [20; 32];
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
            id: [12; 32],
            rank: 0,
            first_edge: 0,
            edge_count: 0,
            first_term: 2,
            term_count: 1,
            kind: CompositionNodeKindV3::Native,
            native_outcome: 2,
            recipe_divisor: 1,
            flattened_denominator: 1,
        },
        CompositionNodeInputV3 {
            id: ROOT,
            rank: 1,
            first_edge: 0,
            edge_count: 3,
            first_term: 3,
            term_count: 3,
            kind: CompositionNodeKindV3::Compose,
            native_outcome: 0,
            recipe_divisor: 1,
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
            child_id: [12; 32],
            child_index: 2,
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
            outcome: 2,
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
            outcome: 2,
            numerator: 1,
        },
    ];
    let mut descriptor_scratch = [0_u8; COMPOSITION_DESCRIPTOR_BYTES_V3];
    let mut descriptor_bytes = [0_u8; COMPOSITION_DESCRIPTOR_BYTES_V3];
    encode_composition_descriptor_v3_atomic(
        CompositionDescriptorInputV3 {
            market: MARKET,
            result_domain: DOMAIN,
            release_set: RELEASE,
            native_basis: REPRESENTATION_BASIS,
            graph_id: GRAPH,
            graph_digest: GRAPH_DIGEST,
            root_id: ROOT,
            translation_id: [72; 32],
            translation_digest: [73; 32],
            outcome_count: 3,
            node_count: 4,
            edge_count: 3,
            term_count: 6,
            root_denominator: 1,
        },
        &mut descriptor_scratch,
        &mut descriptor_bytes,
    )
    .expect("composition descriptor");
    let descriptor =
        CompositionDescriptorV3::decode(&descriptor_bytes, admitted([70; 32], [74; 32]))
            .expect("admitted descriptor");
    let length = composition_graph_bytes_v3(4, 3, 6).expect("graph width");
    let mut graph_scratch = vec![0_u8; length];
    let mut graph_bytes = vec![0_u8; length];
    encode_composition_graph_v3_atomic(
        CompositionGraphInputV3 {
            graph_id: GRAPH,
            root_id: ROOT,
            outcome_count: 3,
            nodes: &nodes,
            edges: &edges,
            terms: &terms,
        },
        &mut graph_scratch,
        &mut graph_bytes,
    )
    .expect("composition graph");
    (graph_bytes, descriptor)
}

#[test]
fn k3_n1_and_k3_n258_translate_exactly() {
    let n1_terms = [
        [CompositionExposureTermV3 {
            product_coordinate: 0,
            numerator: 1,
        }],
        [CompositionExposureTermV3 {
            product_coordinate: 0,
            numerator: 2,
        }],
        [CompositionExposureTermV3 {
            product_coordinate: 0,
            numerator: 3,
        }],
    ];
    let n1_rows = [
        CompositionExposureRowInputV3 {
            node_id: [10; 32],
            denominator: 1,
            terms: &n1_terms[0],
        },
        CompositionExposureRowInputV3 {
            node_id: [11; 32],
            denominator: 1,
            terms: &n1_terms[1],
        },
        CompositionExposureRowInputV3 {
            node_id: [12; 32],
            denominator: 1,
            terms: &n1_terms[2],
        },
    ];
    let n1 = encode(1, &n1_rows);
    let n1_bundle = CompositionExposureBundleV3::decode(&n1, admission())
        .and_then(|bundle| bundle.verify_for(expected(1)))
        .expect("K3/N1 admission");
    assert_eq!(
        n1_bundle.verify_execution_for(execution_expected(1)),
        Ok(n1_bundle)
    );
    let mut scratch = [99; 3];
    let mut output = [99; 3];
    n1_bundle
        .translate_product_payouts(&[7], &mut scratch, &mut output)
        .expect("exact N1 translation");
    assert_eq!(output, [7, 14, 21]);

    let n258_terms = [
        [CompositionExposureTermV3 {
            product_coordinate: 0,
            numerator: 1,
        }],
        [CompositionExposureTermV3 {
            product_coordinate: 128,
            numerator: 1,
        }],
        [CompositionExposureTermV3 {
            product_coordinate: 257,
            numerator: 1,
        }],
    ];
    let n258_rows = [
        CompositionExposureRowInputV3 {
            node_id: [20; 32],
            denominator: 1,
            terms: &n258_terms[0],
        },
        CompositionExposureRowInputV3 {
            node_id: [21; 32],
            denominator: 1,
            terms: &n258_terms[1],
        },
        CompositionExposureRowInputV3 {
            node_id: [22; 32],
            denominator: 1,
            terms: &n258_terms[2],
        },
    ];
    let n258 = encode(258, &n258_rows);
    let n258_bundle = CompositionExposureBundleV3::decode(&n258, admission())
        .and_then(|bundle| bundle.verify_for(expected(258)))
        .expect("K3/N258 admission");
    let mut payouts = [0_u64; 258];
    payouts[0] = 3;
    payouts[128] = 5;
    payouts[257] = 8;
    n258_bundle
        .translate_product_payouts(&payouts, &mut scratch, &mut output)
        .expect("exact N258 translation");
    assert_eq!(output, [3, 5, 8]);
}

#[test]
fn transplant_rank_cycle_width_and_nonintegral_substitutions_refuse_atomically() {
    let terms = [
        [CompositionExposureTermV3 {
            product_coordinate: 0,
            numerator: 1,
        }],
        [CompositionExposureTermV3 {
            product_coordinate: 1,
            numerator: 1,
        }],
        [CompositionExposureTermV3 {
            product_coordinate: 2,
            numerator: 1,
        }],
    ];
    let rows = [
        CompositionExposureRowInputV3 {
            node_id: [30; 32],
            denominator: 1,
            terms: &terms[0],
        },
        CompositionExposureRowInputV3 {
            node_id: [31; 32],
            denominator: 1,
            terms: &terms[1],
        },
        CompositionExposureRowInputV3 {
            node_id: [32; 32],
            denominator: 1,
            terms: &terms[2],
        },
    ];
    let canonical = encode(3, &rows);
    let bundle = CompositionExposureBundleV3::decode(&canonical, admission()).expect("bundle");
    let mut transplanted = expected(3);
    transplanted.release_set = [99; 32];
    assert_eq!(
        bundle.verify_for(transplanted).err(),
        Some(Error::ContentAdmission)
    );

    let mut rank_cycle = canonical.clone();
    let rank_offset = COMPOSITION_EXPOSURE_HEADER_BYTES_V3 + CompositionExposureRowLayoutV3::RANK;
    rank_cycle
        .get_mut(rank_offset..rank_offset + 4)
        .expect("rank field")
        .copy_from_slice(&0_u32.to_le_bytes());
    assert_eq!(
        CompositionExposureBundleV3::decode(&rank_cycle, admission()).err(),
        Some(Error::InvalidNode)
    );

    let mut duplicate_root = canonical.clone();
    let second = COMPOSITION_EXPOSURE_HEADER_BYTES_V3 + COMPOSITION_EXPOSURE_ROW_BYTES_V3;
    duplicate_root
        .get_mut(second..second + 32)
        .expect("second root")
        .copy_from_slice(&[30; 32]);
    assert_eq!(
        CompositionExposureBundleV3::decode(&duplicate_root, admission()).err(),
        Some(Error::DuplicateOrUnorderedNode)
    );

    let mut width_substitution = canonical.clone();
    width_substitution
        .get_mut(
            CompositionExposureLayoutV3::PRODUCT_WIDTH
                ..CompositionExposureLayoutV3::PRODUCT_WIDTH + 4,
        )
        .expect("Product width field")
        .copy_from_slice(&2_u32.to_le_bytes());
    assert_eq!(
        CompositionExposureBundleV3::decode(&width_substitution, admission()).err(),
        Some(Error::NonCanonicalPayoff)
    );

    let fractional_terms = [
        [CompositionExposureTermV3 {
            product_coordinate: 0,
            numerator: 1,
        }],
        [CompositionExposureTermV3 {
            product_coordinate: 1,
            numerator: 1,
        }],
        [CompositionExposureTermV3 {
            product_coordinate: 2,
            numerator: 1,
        }],
    ];
    let fractional_rows = [
        CompositionExposureRowInputV3 {
            node_id: [40; 32],
            denominator: 2,
            terms: &fractional_terms[0],
        },
        CompositionExposureRowInputV3 {
            node_id: [41; 32],
            denominator: 1,
            terms: &fractional_terms[1],
        },
        CompositionExposureRowInputV3 {
            node_id: [42; 32],
            denominator: 1,
            terms: &fractional_terms[2],
        },
    ];
    let fractional = encode(3, &fractional_rows);
    let fractional_bundle =
        CompositionExposureBundleV3::decode(&fractional, admission()).expect("fractional bundle");
    let mut scratch = [77; 3];
    let mut output = [77; 3];
    assert_eq!(
        fractional_bundle.translate_product_payouts(&[1, 2, 3], &mut scratch, &mut output),
        Err(Error::NonIntegralTranslation)
    );
    assert_eq!(output, [77; 3]);
}

#[test]
fn encoding_is_byte_canonical_and_admission_is_mandatory() {
    let terms = [
        [CompositionExposureTermV3 {
            product_coordinate: 0,
            numerator: 1,
        }],
        [CompositionExposureTermV3 {
            product_coordinate: 1,
            numerator: 1,
        }],
        [CompositionExposureTermV3 {
            product_coordinate: 2,
            numerator: 1,
        }],
    ];
    let rows = [
        CompositionExposureRowInputV3 {
            node_id: [50; 32],
            denominator: 1,
            terms: &terms[0],
        },
        CompositionExposureRowInputV3 {
            node_id: [51; 32],
            denominator: 1,
            terms: &terms[1],
        },
        CompositionExposureRowInputV3 {
            node_id: [52; 32],
            denominator: 1,
            terms: &terms[2],
        },
    ];
    let first = encode(3, &rows);
    let second = encode(3, &rows);
    assert_eq!(first, second);
    let mut unauthenticated = admission();
    unauthenticated.record_authenticated = false;
    assert_eq!(
        CompositionExposureBundleV3::decode(&first, unauthenticated).err(),
        Some(Error::ContentAdmission)
    );
}

#[test]
fn exposure_rows_join_the_unique_native_dag_basis() {
    let (graph_bytes, descriptor) = composition_graph();
    let graph = CompositionGraphV3::decode(&graph_bytes, descriptor, admitted(GRAPH, [71; 32]))
        .expect("admitted composition graph");
    let exposure = CompositionExposureBundleV3::decode(
        &crate_generated::COMPOSITION_EXPOSURE_K3_N1_WITNESS_V3,
        admission(),
    )
    .expect("admitted exposure");
    assert!(exposure.verify_composition_graph(graph).is_ok());

    let mut substituted = crate_generated::COMPOSITION_EXPOSURE_K3_N1_WITNESS_V3;
    *substituted
        .get_mut(COMPOSITION_EXPOSURE_HEADER_BYTES_V3)
        .expect("first exposure node") = 13;
    let substituted =
        CompositionExposureBundleV3::decode(&substituted, admission()).expect("valid substitution");
    assert_eq!(
        substituted.verify_composition_graph(graph).err(),
        Some(Error::CompositionMismatch)
    );
}

#[allow(dead_code, missing_docs)]
#[path = "../src/generated_exposure_abi.rs"]
mod crate_generated;
