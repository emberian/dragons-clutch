//! Composition DAG to canonical Fractional/Product basis agreement.

#![allow(clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

mod support;

use dclutch_fractional_claim_contract::FractionalActionV1;
use dclutch_fractional_claim_operator::{
    Error as FractionalError, FractionalClaimsAccountRuleV1,
    build_fractional_composed_artifact_bundle_v1, build_fractional_finalized_artifact_bundle_v1,
    decode_and_check_fractional_composition_v1,
};
use dclutch_representation_composition_v3_kernel::{
    COMPOSITION_DESCRIPTOR_BYTES_V3, COMPOSITION_EDGE_BYTES_V3, COMPOSITION_GRAPH_HEADER_BYTES_V3,
    COMPOSITION_NODE_BYTES_V3, CanonicalTranslationInputV3, CompositionDescriptorInputV3,
    CompositionEdgeInputV3, CompositionGraphInputV3, CompositionNodeInputV3, CompositionNodeKindV3,
    EdgeLayoutV3, Error as CompositionError, NodeLayoutV3, RecordAdmissionV3, SparseTermV3,
    composition_graph_bytes_v3, composition_translation_bytes_v3, decode_composition_bundle_v3,
    encode_canonical_translation_v3_atomic, encode_composition_descriptor_v3_atomic,
    encode_composition_graph_v3_atomic,
};
use sha2::{Digest, Sha256};

use support::FractionalChainFixtureV1;

const GRAPH_ID: [u8; 32] = [80; 32];
const ROOT_ID: [u8; 32] = [81; 32];
const TRANSLATION_ID: [u8; 32] = [82; 32];

struct Corpus {
    descriptor: [u8; COMPOSITION_DESCRIPTOR_BYTES_V3],
    graph: Vec<u8>,
    translation: Vec<u8>,
}

impl Corpus {
    fn admissions(&self) -> (RecordAdmissionV3, RecordAdmissionV3, RecordAdmissionV3) {
        let descriptor_digest = digest(&self.descriptor);
        let graph_digest = digest(&self.graph);
        let translation_digest = digest(&self.translation);
        (
            admitted(descriptor_digest, descriptor_digest),
            admitted(GRAPH_ID, graph_digest),
            admitted(TRANSLATION_ID, translation_digest),
        )
    }

    fn rebind_descriptor(
        &mut self,
        fixture: &FractionalChainFixtureV1,
        result_domain: [u8; 32],
        denominator: u64,
    ) {
        self.descriptor = descriptor(
            fixture,
            result_domain,
            digest(&self.graph),
            digest(&self.translation),
            denominator,
        );
    }
}

fn claims_program_only_frame() -> [FractionalClaimsAccountRuleV1; 1] {
    [FractionalClaimsAccountRuleV1 {
        signer: false,
        writable: false,
        executable: true,
        data_length: 0,
    }]
}

fn fixture() -> FractionalChainFixtureV1 {
    FractionalChainFixtureV1::new(
        FractionalActionV1::Wrap,
        [62; 32],
        &claims_program_only_frame(),
    )
}

fn corpus(fixture: &FractionalChainFixtureV1, coefficients: [u64; 3], denominator: u64) -> Corpus {
    let nodes = [
        native_node([10; 32], 0, 0),
        native_node([11; 32], 1, 1),
        native_node([12; 32], 2, 2),
        CompositionNodeInputV3 {
            id: ROOT_ID,
            rank: 1,
            first_edge: 0,
            edge_count: 3,
            first_term: 3,
            term_count: 3,
            kind: CompositionNodeKindV3::Compose,
            native_outcome: 0,
            recipe_divisor: denominator,
            flattened_denominator: denominator,
        },
    ];
    let edges = [
        edge([10; 32], 0, coefficients[0]),
        edge([11; 32], 1, coefficients[1]),
        edge([12; 32], 2, coefficients[2]),
    ];
    let terms = [
        term(0, 1),
        term(1, 1),
        term(2, 1),
        term(0, coefficients[0]),
        term(1, coefficients[1]),
        term(2, coefficients[2]),
    ];
    let graph_width = composition_graph_bytes_v3(4, 3, 6).expect("graph width");
    let mut graph_scratch = vec![0; graph_width];
    let mut graph = vec![0; graph_width];
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
    .expect("canonical composition graph");

    let root_terms = [terms[3], terms[4], terms[5]];
    let translation_width = composition_translation_bytes_v3(3).expect("translation width");
    let mut translation_scratch = vec![0; translation_width];
    let mut translation = vec![0; translation_width];
    encode_canonical_translation_v3_atomic(
        CanonicalTranslationInputV3 {
            graph_id: GRAPH_ID,
            root_id: ROOT_ID,
            outcome_count: 3,
            denominator,
            terms: &root_terms,
        },
        &mut translation_scratch,
        &mut translation,
    )
    .expect("canonical root translation");

    let descriptor = descriptor(
        fixture,
        fixture.prepare().request_context().result_domain,
        digest(&graph),
        digest(&translation),
        denominator,
    );
    Corpus {
        descriptor,
        graph,
        translation,
    }
}

fn descriptor(
    fixture: &FractionalChainFixtureV1,
    result_domain: [u8; 32],
    graph_digest: [u8; 32],
    translation_digest: [u8; 32],
    denominator: u64,
) -> [u8; COMPOSITION_DESCRIPTOR_BYTES_V3] {
    let prepared = fixture.prepare();
    let mut scratch = [0; COMPOSITION_DESCRIPTOR_BYTES_V3];
    let mut output = [0; COMPOSITION_DESCRIPTOR_BYTES_V3];
    encode_composition_descriptor_v3_atomic(
        CompositionDescriptorInputV3 {
            market: prepared.request_context().market,
            result_domain,
            release_set: prepared.request_context().release_set,
            native_basis: prepared.product_join().claim_basis_id.to_bytes(),
            graph_id: GRAPH_ID,
            graph_digest,
            root_id: ROOT_ID,
            translation_id: TRANSLATION_ID,
            translation_digest,
            outcome_count: 3,
            node_count: 4,
            edge_count: 3,
            term_count: 6,
            root_denominator: denominator,
        },
        &mut scratch,
        &mut output,
    )
    .expect("canonical composition descriptor");
    output
}

fn native_node(id: [u8; 32], outcome: u32, first_term: u32) -> CompositionNodeInputV3 {
    CompositionNodeInputV3 {
        id,
        rank: 0,
        first_edge: 0,
        edge_count: 0,
        first_term,
        term_count: 1,
        kind: CompositionNodeKindV3::Native,
        native_outcome: outcome,
        recipe_divisor: 1,
        flattened_denominator: 1,
    }
}

fn edge(child_id: [u8; 32], child_index: u32, coefficient: u64) -> CompositionEdgeInputV3 {
    CompositionEdgeInputV3 {
        child_id,
        child_index,
        coefficient,
    }
}

const fn term(outcome: u32, numerator: u64) -> SparseTermV3 {
    SparseTermV3 { outcome, numerator }
}

fn admitted(selected: [u8; 32], content_digest: [u8; 32]) -> RecordAdmissionV3 {
    RecordAdmissionV3 {
        selected_id: selected,
        finalized_id: selected,
        recomputed_digest: content_digest,
        finalized_digest: content_digest,
        record_authenticated: true,
    }
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn decode_checked<'a>(
    fixture: &FractionalChainFixtureV1,
    corpus: &'a Corpus,
) -> Result<dclutch_fractional_claim_operator::CheckedFractionalCompositionV1<'a>, FractionalError>
{
    let (descriptor, graph, translation) = corpus.admissions();
    decode_and_check_fractional_composition_v1(
        fixture.prepare(),
        &corpus.descriptor,
        descriptor,
        &corpus.graph,
        graph,
        &corpus.translation,
        translation,
    )
}

#[test]
fn canonical_dag_equals_product_basis_and_changes_no_fractional_artifact_bytes() {
    let fixture = fixture();
    let corpus = corpus(&fixture, [1, 1, 1], 1);
    let witness = decode_checked(&fixture, &corpus).expect("checked composition");
    let composed = build_fractional_composed_artifact_bundle_v1(
        FractionalActionV1::Wrap,
        [62; 32],
        &claims_program_only_frame(),
        witness,
    )
    .expect("composition-gated artifacts");
    let direct = build_fractional_finalized_artifact_bundle_v1(
        FractionalActionV1::Wrap,
        [62; 32],
        &claims_program_only_frame(),
    )
    .expect("ordinary artifacts");
    assert_eq!(composed.descriptor, direct.descriptor);
    assert_eq!(composed.request_profile, direct.request_profile);
    assert_eq!(composed.effect, direct.effect);
    assert_eq!(witness.bundle().translation().denominator(), 1);
}

#[test]
fn cycle_unreachable_and_product_domain_substitution_refuse() {
    let fixture = fixture();

    let mut cycle = corpus(&fixture, [1, 1, 1], 1);
    let edge_offset = COMPOSITION_GRAPH_HEADER_BYTES_V3
        + 4 * COMPOSITION_NODE_BYTES_V3
        + 2 * COMPOSITION_EDGE_BYTES_V3;
    cycle.graph[edge_offset + EdgeLayoutV3::CHILD_ID..edge_offset + EdgeLayoutV3::CHILD_ID + 32]
        .copy_from_slice(&ROOT_ID);
    cycle.graph
        [edge_offset + EdgeLayoutV3::CHILD_INDEX..edge_offset + EdgeLayoutV3::CHILD_INDEX + 4]
        .copy_from_slice(&3_u32.to_le_bytes());
    cycle.rebind_descriptor(
        &fixture,
        fixture.prepare().request_context().result_domain,
        1,
    );
    assert!(matches!(
        decode_checked(&fixture, &cycle),
        Err(FractionalError::Composition)
    ));

    let mut unreachable = corpus(&fixture, [1, 1, 1], 1);
    let root_offset = COMPOSITION_GRAPH_HEADER_BYTES_V3 + 3 * COMPOSITION_NODE_BYTES_V3;
    unreachable.graph
        [root_offset + NodeLayoutV3::EDGE_COUNT..root_offset + NodeLayoutV3::EDGE_COUNT + 4]
        .copy_from_slice(&2_u32.to_le_bytes());
    unreachable.rebind_descriptor(
        &fixture,
        fixture.prepare().request_context().result_domain,
        1,
    );
    assert!(matches!(
        decode_checked(&fixture, &unreachable),
        Err(FractionalError::Composition)
    ));

    let mut substituted = corpus(&fixture, [1, 1, 1], 1);
    substituted.rebind_descriptor(&fixture, [99; 32], 1);
    assert!(matches!(
        decode_checked(&fixture, &substituted),
        Err(FractionalError::Composition)
    ));
}

#[test]
fn coefficient_conservation_nonintegrality_and_overflow_refuse() {
    let fixture = fixture();
    let mismatched = corpus(&fixture, [1, 1, 2], 1);
    assert!(matches!(
        decode_checked(&fixture, &mismatched),
        Err(FractionalError::Composition)
    ));

    let nonintegral = corpus(&fixture, [1, 1, 1], 2);
    let (descriptor, graph, translation) = nonintegral.admissions();
    let bundle = decode_composition_bundle_v3(
        &nonintegral.descriptor,
        descriptor,
        &nonintegral.graph,
        graph,
        &nonintegral.translation,
        translation,
    )
    .expect("valid exact rational DAG");
    let mut scratch = [0_u64; 3];
    let mut output = [77_u64; 3];
    assert_eq!(
        bundle
            .translation()
            .materialize_exact(1, &mut scratch, &mut output),
        Err(CompositionError::NonIntegralTranslation)
    );
    assert_eq!(output, [77; 3]);

    let overflowing = corpus(&fixture, [1, 1, 2], 1);
    let (descriptor, graph, translation) = overflowing.admissions();
    let bundle = decode_composition_bundle_v3(
        &overflowing.descriptor,
        descriptor,
        &overflowing.graph,
        graph,
        &overflowing.translation,
        translation,
    )
    .expect("valid large-quantity translation");
    assert_eq!(
        bundle
            .translation()
            .materialize_exact(u64::MAX, &mut scratch, &mut output),
        Err(CompositionError::ArithmeticOverflow)
    );
    assert_eq!(output, [77; 3]);

    assert_eq!(
        bundle.translation().verify_conservation(1, &[1, 1, 1]),
        Err(CompositionError::ConservationMismatch)
    );
}
