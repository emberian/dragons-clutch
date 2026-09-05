//! The shared REAL-RECORD composition every Structured descriptor test starts
//! from, and the campaign basis it encodes.
//!
//! `K = 3`, coefficients `[2, 3, 5]`, denominator `7`. Pairwise coprime, and
//! coprime to the denominator: that is what makes a one-atom backing skew at
//! any single coordinate impossible to present as a legitimate quantity at
//! another, so `K_i = S * c_i` either holds everywhere or fails visibly at one
//! coordinate.
//!
//! Every record here is encoded by its own kernel's atomic encoder and decoded
//! back through its own admission -- the composition graph, the canonical
//! translation, the composition descriptor and the exposure bundle. Nothing is
//! a hand-filled 32-byte identity standing in for a record, because that is the
//! exact defect these fixtures exist to keep out: five modules in this tree
//! build a Rational descriptor preimage out of `id(11), id(12), id(13)`, and a
//! descriptor assembled that way asserts its own joins.

#![allow(dead_code)]

use dclutch_claims::composition::{
    COMPOSITION_DESCRIPTOR_BYTES_V3, CanonicalTranslationInputV3, CompositionBundleV3,
    CompositionDescriptorInputV3, CompositionEdgeInputV3, CompositionExposureBundleV3,
    CompositionExposureInputV3, CompositionExposureRowInputV3, CompositionExposureTermV3,
    CompositionGraphInputV3, CompositionNodeInputV3, CompositionNodeKindV3, RecordAdmissionV3,
    SparseTermV3, composition_exposure_bytes_v3, composition_graph_bytes_v3,
    composition_translation_bytes_v3, decode_composition_bundle_v3,
    encode_canonical_translation_v3_atomic, encode_composition_descriptor_v3_atomic,
    encode_composition_exposure_v3_atomic, encode_composition_graph_v3_atomic,
};
use dclutch_claims::structured_kernel::{
    StructuredTermsInputV2, StructuredTermsV2, encode_structured_terms_v2,
    structured_terms_bytes_v2,
};
use dclutch_custody::token_svm::TOKEN_2022_PROGRAM_ID;
use dclutch_operator::structured::{
    StructuredRepresentationDescriptorV2, derive_structured_representation_descriptor_v2,
};

use crate::support::{digest, identity, shard_mints, shard_terms, structured_admission};

pub const K: u32 = 3;
pub const PRODUCT_WIDTH: u32 = 3;
/// Pairwise coprime, and coprime to the denominator.
pub const COEFFICIENTS: [u64; 3] = [2, 3, 5];
pub const DENOMINATOR: u64 = 7;

pub const MARKET: u8 = 0x11;
pub const PRODUCT_RECORD: u8 = 0x12;
pub const RESULT_DOMAIN: u8 = 0x13;
pub const RELEASE_SET: u8 = 0x14;
pub const GRAPH_ID: u8 = 0x1b;
pub const RECEIPT_MINT: u8 = 0x1c;
pub const NATIVE_BASIS: u8 = 0x1a;
pub const PRODUCT_BASIS: u8 = 0x19;
pub const ROOT_ID: u8 = 0x20;
pub const TRANSLATION_ID: u8 = 0x31;
pub const AUTHORITY: u8 = 0x51;

/// Every encoded record of one composition, kept alive together because the
/// decoders borrow their bytes.
pub struct Composition {
    pub graph_id: [u8; 32],
    pub descriptor: Vec<u8>,
    pub graph: Vec<u8>,
    pub translation: Vec<u8>,
    pub exposure: Vec<u8>,
}

pub fn admission(id: [u8; 32], digest_value: [u8; 32]) -> RecordAdmissionV3 {
    RecordAdmissionV3 {
        selected_id: id,
        finalized_id: id,
        recomputed_digest: digest_value,
        finalized_digest: digest_value,
        record_authenticated: true,
    }
}

/// One canonical composition whose ROOT payoff is `coefficients / denominator`.
///
/// The root's sparse terms are the numerators and its flattened denominator is
/// the common scale, so the composition and the Structured terms state the same
/// recipe in the same lowest form -- which is what
/// `require_coefficients_are_the_composition_root` checks by cross
/// multiplication.  Passing a different vector here is how the hostile below
/// disagrees with the terms without becoming an invalid record.
pub fn composition(
    root_numerators: [u64; 3],
    root_denominator: u64,
    graph_id: [u8; 32],
) -> Composition {
    composition_for_market(
        root_numerators,
        root_denominator,
        graph_id,
        identity(MARKET),
    )
}

/// The same composition over a CHOSEN Market coordinate.
///
/// Both market-bearing records in the composition closure -- the composition
/// descriptor and the composition exposure -- take the same value here, because
/// the derivation cross-checks them against the Structured terms
/// (`descriptor.rs:128` refuses when `exposure.market() != terms.market()`).
/// Varying the Market therefore means varying it in every record that carries
/// it, which is what makes a two-market comparison a comparison of the SAME
/// closure at two Markets rather than of two disagreeing ones.
pub fn composition_for_market(
    root_numerators: [u64; 3],
    root_denominator: u64,
    graph_id: [u8; 32],
    market: [u8; 32],
) -> Composition {
    let leaves = [identity(0x10), identity(0x11), identity(0x12)];
    let nodes = [
        CompositionNodeInputV3 {
            id: leaves[0],
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
            id: leaves[1],
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
            id: leaves[2],
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
            id: identity(ROOT_ID),
            rank: 1,
            first_edge: 0,
            edge_count: 3,
            first_term: 3,
            term_count: 3,
            kind: CompositionNodeKindV3::Compose,
            native_outcome: 0,
            recipe_divisor: root_denominator,
            flattened_denominator: root_denominator,
        },
    ];
    let edges = [
        CompositionEdgeInputV3 {
            child_id: leaves[0],
            child_index: 0,
            coefficient: root_numerators[0],
        },
        CompositionEdgeInputV3 {
            child_id: leaves[1],
            child_index: 1,
            coefficient: root_numerators[1],
        },
        CompositionEdgeInputV3 {
            child_id: leaves[2],
            child_index: 2,
            coefficient: root_numerators[2],
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
            numerator: root_numerators[0],
        },
        SparseTermV3 {
            outcome: 1,
            numerator: root_numerators[1],
        },
        SparseTermV3 {
            outcome: 2,
            numerator: root_numerators[2],
        },
    ];

    let graph_len = composition_graph_bytes_v3(4, 3, 6).expect("graph width");
    let mut graph_scratch = vec![0_u8; graph_len];
    let mut graph = vec![0_u8; graph_len];
    encode_composition_graph_v3_atomic(
        CompositionGraphInputV3 {
            graph_id,
            root_id: identity(ROOT_ID),
            outcome_count: K,
            nodes: &nodes,
            edges: &edges,
            terms: &terms,
        },
        &mut graph_scratch,
        &mut graph,
    )
    .expect("composition graph");

    let translation_len = composition_translation_bytes_v3(3).expect("translation width");
    let mut translation_scratch = vec![0_u8; translation_len];
    let mut translation = vec![0_u8; translation_len];
    encode_canonical_translation_v3_atomic(
        CanonicalTranslationInputV3 {
            graph_id,
            root_id: identity(ROOT_ID),
            outcome_count: K,
            denominator: root_denominator,
            terms: &terms[3..],
        },
        &mut translation_scratch,
        &mut translation,
    )
    .expect("canonical translation");

    let mut descriptor_scratch = [0_u8; COMPOSITION_DESCRIPTOR_BYTES_V3];
    let mut descriptor = [0_u8; COMPOSITION_DESCRIPTOR_BYTES_V3];
    encode_composition_descriptor_v3_atomic(
        CompositionDescriptorInputV3 {
            market,
            result_domain: identity(RESULT_DOMAIN),
            release_set: identity(RELEASE_SET),
            native_basis: identity(NATIVE_BASIS),
            graph_id,
            graph_digest: digest(&graph),
            root_id: identity(ROOT_ID),
            translation_id: identity(TRANSLATION_ID),
            translation_digest: digest(&translation),
            outcome_count: K,
            node_count: 4,
            edge_count: 3,
            term_count: 6,
            root_denominator,
        },
        &mut descriptor_scratch,
        &mut descriptor,
    )
    .expect("composition descriptor");

    let row_terms = [
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
            node_id: leaves[0],
            denominator: 1,
            terms: &row_terms[0],
        },
        CompositionExposureRowInputV3 {
            node_id: leaves[1],
            denominator: 1,
            terms: &row_terms[1],
        },
        CompositionExposureRowInputV3 {
            node_id: leaves[2],
            denominator: 1,
            terms: &row_terms[2],
        },
    ];
    let exposure_len = composition_exposure_bytes_v3(K, K).expect("exposure width");
    let mut exposure_scratch = vec![0_u8; exposure_len];
    let mut exposure = vec![0_u8; exposure_len];
    encode_composition_exposure_v3_atomic(
        CompositionExposureInputV3 {
            market,
            result_domain: identity(RESULT_DOMAIN),
            release_set: identity(RELEASE_SET),
            product_basis: identity(PRODUCT_BASIS),
            representation_basis: identity(NATIVE_BASIS),
            graph_id,
            product_width: PRODUCT_WIDTH,
            rows: &rows,
        },
        &mut exposure_scratch,
        &mut exposure,
    )
    .expect("composition exposure");

    Composition {
        graph_id,
        descriptor: descriptor.to_vec(),
        graph,
        translation,
        exposure,
    }
}

impl Composition {
    pub fn bundle(&self) -> CompositionBundleV3<'_> {
        decode_composition_bundle_v3(
            &self.descriptor,
            admission(digest(&self.descriptor), digest(&self.descriptor)),
            &self.graph,
            admission(self.graph_id, digest(&self.graph)),
            &self.translation,
            admission(identity(TRANSLATION_ID), digest(&self.translation)),
        )
        .expect("authenticated composition bundle")
    }

    pub fn exposure_bundle(&self) -> CompositionExposureBundleV3<'_> {
        CompositionExposureBundleV3::decode(
            &self.exposure,
            admission(digest(&self.exposure), digest(&self.exposure)),
        )
        .expect("authenticated exposure bundle")
    }

    pub fn exposure_id(&self) -> [u8; 32] {
        digest(&self.exposure)
    }
}

pub fn shard_terms_bytes(exposure_id: [u8; 32]) -> Vec<u8> {
    shard_terms_bytes_scaled(exposure_id, DENOMINATOR)
}

/// The same shard layer at a chosen denominator.
///
/// The shard layer PINS the denominator: `bind_shard_terms` refuses when the
/// two disagree, so a rescaled recipe is a different shard layer, not a
/// presentation choice.
pub fn shard_terms_bytes_scaled(exposure_id: [u8; 32], denominator: u64) -> Vec<u8> {
    shard_terms_bytes_scaled_for_market(exposure_id, denominator, identity(MARKET))
}

/// The same shard layer over a CHOSEN Market coordinate.
pub fn shard_terms_bytes_scaled_for_market(
    exposure_id: [u8; 32],
    denominator: u64,
    market: [u8; 32],
) -> Vec<u8> {
    use dclutch_claims::fractional_kernel::{
        FractionalExposureTermsInputV2, encode_fractional_exposure_terms_v2,
        fractional_exposure_terms_bytes_v2,
    };
    let mints = shard_mints(K as usize);
    let size = fractional_exposure_terms_bytes_v2(K as usize).expect("shard terms width");
    let mut scratch = vec![0_u8; size];
    let mut output = vec![0_u8; size];
    encode_fractional_exposure_terms_v2(
        FractionalExposureTermsInputV2 {
            market,
            product_record: identity(PRODUCT_RECORD),
            result_domain: identity(RESULT_DOMAIN),
            release_set: identity(RELEASE_SET),
            token_program: TOKEN_2022_PROGRAM_ID,
            token_behavior: identity(0x16),
            exposure_id,
            product_basis: identity(PRODUCT_BASIS),
            representation_basis: identity(NATIVE_BASIS),
            graph_id: identity(GRAPH_ID),
            product_width: PRODUCT_WIDTH,
            denominator,
            shard_mints: &mints,
        },
        &mut scratch,
        &mut output,
    )
    .expect("encode shard terms");
    output
}

pub fn terms_bytes(exposure_id: [u8; 32], coefficients: &[u64]) -> Vec<u8> {
    terms_bytes_for_market(exposure_id, coefficients, identity(MARKET))
}

/// The same Structured terms over a CHOSEN Market coordinate.
///
/// The Market travels through the encoder's own named `market` field, so this
/// helper makes no claim about a byte offset and cannot be silently wrong about
/// which field it is substituting.
pub fn terms_bytes_for_market(
    exposure_id: [u8; 32],
    coefficients: &[u64],
    market: [u8; 32],
) -> Vec<u8> {
    let size = structured_terms_bytes_v2(coefficients.len()).expect("terms width");
    let mut scratch = vec![0_u8; size];
    let mut output = vec![0_u8; size];
    encode_structured_terms_v2(
        StructuredTermsInputV2 {
            market,
            product_record: identity(PRODUCT_RECORD),
            result_domain: identity(RESULT_DOMAIN),
            release_set: identity(RELEASE_SET),
            token_program: TOKEN_2022_PROGRAM_ID,
            token_behavior: identity(0x17),
            // The shard layer this names must be the one at the SAME Market,
            // or the two records disagree and the derivation refuses for a
            // reason that has nothing to do with the question being asked.
            shard_terms: digest(&shard_terms_bytes_scaled_for_market(
                exposure_id,
                DENOMINATOR,
                market,
            )),
            shard_exposure: exposure_id,
            receipt_mint: identity(RECEIPT_MINT),
            graph_id: identity(GRAPH_ID),
            denominator: DENOMINATOR,
            coefficients,
        },
        &mut scratch,
        &mut output,
    )
    .expect("encode terms");
    output
}

pub fn decode_terms<'a>(bytes: &'a [u8], shard_bytes: &'a [u8]) -> StructuredTermsV2<'a> {
    StructuredTermsV2::decode(bytes, structured_admission(bytes), shard_terms(shard_bytes))
        .expect("decode terms")
}

/// The canonical derivation every artifact test starts from.
pub fn derived_descriptor() -> StructuredRepresentationDescriptorV2 {
    derived_descriptor_for_market(identity(MARKET))
}

/// The canonical derivation at a CHOSEN Market coordinate.
///
/// Every record in the closure that carries a Market -- composition descriptor,
/// composition exposure, shard terms, Structured terms -- is built at the same
/// one. Nothing else about the closure changes: the coefficients, denominator,
/// graph, release set, receipt Mint and Token program are the fixture's.
pub fn derived_descriptor_for_market(market: [u8; 32]) -> StructuredRepresentationDescriptorV2 {
    let composition = composition_for_market(COEFFICIENTS, DENOMINATOR, identity(GRAPH_ID), market);
    let exposure_id = composition.exposure_id();
    let terms_source = terms_bytes_for_market(exposure_id, &COEFFICIENTS, market);
    let shard_source = shard_terms_bytes_scaled_for_market(exposure_id, DENOMINATOR, market);
    let terms = decode_terms(&terms_source, &shard_source);
    derive_structured_representation_descriptor_v2(
        terms,
        composition.bundle(),
        composition.exposure_bundle(),
    )
    .expect("derived descriptor")
}
