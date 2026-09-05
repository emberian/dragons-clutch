//! Structured V2's founding-time lowering, built here so the campaign's
//! descriptor is DERIVED rather than hand-written.
//!
//! # Why this module exists
//!
//! Decision 0011 §3c/§3d: under Option A, Structured authors no artifacts. The
//! one genuinely new host-side object it needs is the Rational execution
//! descriptor, and
//! [`derive_structured_representation_descriptor_v2`] is it. Before this
//! module, `rational_representation_v2_program_test.rs` filled the descriptor
//! preimage by hand — the SIXTH such producer in the tree, and the only one
//! that ever reached a real ELF. A hand-filled preimage asserts its own joins:
//! it can name a graph nobody composed, a root nobody rooted, and coefficients
//! that are not the recipe, and every one of those defects reads as a passing
//! test. Every field written below is read out of an already-decoded
//! composition record or out of the immutable Structured terms, and the two
//! are joined before a byte is written.
//!
//! # The identity the executing fixture used to conflate
//!
//! `RepresentationDescriptorV2::graph_id()` names the **exposure bundle**, not
//! the source composition graph (0011 §3d). The fixture this module replaces
//! wrote one constant, `[0x31; 32]`, into BOTH the request header's `graph_id`
//! (which the chain uses as `CompositionExposureBundleV3`'s selected record
//! identity) and the exposure record's own `graph_id` field (which names the
//! source DAG). Those are two different records. The derivation refuses that
//! conflation outright — `StructuredTermsV2::require_distinct_identities`
//! proves `shard_exposure != graph_id`, so a lowering that equates them cannot
//! produce a descriptor at all — which is why this module carries
//! [`StructuredBasis::exposure_id`] and [`StructuredBasis::source_graph_id`]
//! as separate values.
//!
//! # The shard layer's Mints are placeholders, and that is structural
//!
//! `StructuredTermsV2::bind_shard_terms` joins a `FractionalExposureTermsV2`
//! record that carries `K` shard Mint identities. Under Option A the shard
//! Mints are Rational's descriptor-keyed PDAs — derived from a `descriptor_id`
//! that does not exist until this derivation finishes, which itself consumes
//! the shard-terms digest. The dependency is a cycle, so the shard layer names
//! its own Mints and the only rule the kernel enforces across the two is the
//! rank rule: the receipt Mint may never alias a shard Mint. That is recorded
//! here rather than papered over.

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
use dclutch_claims::fractional_kernel::{
    FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2, FractionalExposureTermsAdmissionV2,
    FractionalExposureTermsInputV2, FractionalExposureTermsV2, encode_fractional_exposure_terms_v2,
    fractional_exposure_terms_bytes_v2,
};
use dclutch_claims::structured_kernel::{
    STRUCTURED_TERMS_SCHEMA_ID_V2, StructuredTermsAdmissionV2, StructuredTermsInputV2,
    StructuredTermsV2, encode_structured_terms_v2, structured_terms_bytes_v2,
};
use dclutch_operator::structured::{
    StructuredRepresentationDescriptorV2, derive_structured_representation_descriptor_v2,
};
use solana_program::hash::hash;

fn digest(bytes: &[u8]) -> [u8; 32] {
    hash(bytes).to_bytes()
}

/// A finalized record admitted by its own content identity.
fn admission(id: [u8; 32], record_digest: [u8; 32]) -> RecordAdmissionV3 {
    RecordAdmissionV3 {
        selected_id: id,
        finalized_id: id,
        recomputed_digest: record_digest,
        finalized_digest: record_digest,
        record_authenticated: true,
    }
}

/// Everything one Structured product's lowering reads that is not derived.
///
/// The two graph identities are deliberately separate fields; see the module
/// doc for the conflation that made them one.
#[derive(Clone)]
pub struct StructuredBasis {
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Finalized Product root record identity.
    pub product_record: [u8; 32],
    /// Product-owned result domain.
    pub result_domain: [u8; 32],
    /// Immutable selected release set.
    pub release_set: [u8; 32],
    /// Product-owned terminal-result basis.
    pub product_basis: [u8; 32],
    /// Claims-owned representation basis.
    pub representation_basis: [u8; 32],
    /// The finalized EXPOSURE record's selected identity — what the request
    /// header carries as `graph_id` and what the descriptor names.
    pub exposure_id: [u8; 32],
    /// The SOURCE composition graph's identity — a different record.
    pub source_graph_id: [u8; 32],
    /// The composition graph's rank-`K` root node identity.
    pub root_id: [u8; 32],
    /// The canonical translation record's identity.
    pub translation_id: [u8; 32],
    /// The receipt Mint every issued receipt is minted from.
    pub receipt_mint: [u8; 32],
    /// The Token program that owns every Mint in the instrument.
    pub token_program: [u8; 32],
    /// Selected token-behavior profile for the shard layer.
    pub shard_token_behavior: [u8; 32],
    /// Selected token-behavior profile for the receipt layer.
    pub receipt_token_behavior: [u8; 32],
    /// The shard layer's own Mint identities — see the module doc.
    pub shard_mints: Vec<[u8; 32]>,
    /// `c_i`: shard atoms of coordinate `i` backing one receipt atom.
    pub coefficients: Vec<u64>,
    /// Shard atoms backing one whole native claim.
    pub denominator: u64,
    /// Product terminal-result width `N`.
    pub product_width: u32,
}

impl StructuredBasis {
    /// Representation width `K`.
    pub fn representation_width(&self) -> u32 {
        u32::try_from(self.coefficients.len()).expect("representation width")
    }
}

/// One encoded composition, kept alive together because the decoders borrow.
pub struct Composition {
    pub descriptor: Vec<u8>,
    pub graph: Vec<u8>,
    pub translation: Vec<u8>,
    graph_id: [u8; 32],
    translation_id: [u8; 32],
}

impl Composition {
    pub fn bundle(&self) -> CompositionBundleV3<'_> {
        decode_composition_bundle_v3(
            &self.descriptor,
            admission(digest(&self.descriptor), digest(&self.descriptor)),
            &self.graph,
            admission(self.graph_id, digest(&self.graph)),
            &self.translation,
            admission(self.translation_id, digest(&self.translation)),
        )
        .expect("authenticated composition bundle")
    }
}

/// Build one canonical composition whose ROOT payoff is
/// `root_numerators / root_denominator` over `K` native leaves.
///
/// The root's sparse terms ARE the numerators and its flattened denominator is
/// the common scale, so the composition and the Structured terms state the same
/// recipe in the same lowest form. Passing a different vector is how a hostile
/// disagrees with the terms without ceasing to be a valid record.
pub fn composition(
    basis: &StructuredBasis,
    root_numerators: &[u64],
    root_denominator: u64,
) -> Composition {
    let width = basis.representation_width();
    assert_eq!(root_numerators.len(), basis.coefficients.len());
    let leaves: Vec<[u8; 32]> = (0..root_numerators.len())
        .map(|index| {
            let mut value = [0_u8; 32];
            value[0] = 0xc0;
            value[1] = u8::try_from(index).expect("leaf index");
            value[31] = 0xa5;
            value
        })
        .collect();
    let mut nodes: Vec<CompositionNodeInputV3> = leaves
        .iter()
        .enumerate()
        .map(|(index, id)| CompositionNodeInputV3 {
            id: *id,
            rank: 0,
            first_edge: 0,
            edge_count: 0,
            first_term: u32::try_from(index).expect("leaf term index"),
            term_count: 1,
            kind: CompositionNodeKindV3::Native,
            native_outcome: u32::try_from(index).expect("native outcome"),
            recipe_divisor: 1,
            flattened_denominator: 1,
        })
        .collect();
    nodes.push(CompositionNodeInputV3 {
        id: basis.root_id,
        rank: 1,
        first_edge: 0,
        edge_count: width,
        first_term: width,
        term_count: width,
        kind: CompositionNodeKindV3::Compose,
        native_outcome: 0,
        recipe_divisor: root_denominator,
        flattened_denominator: root_denominator,
    });
    let edges: Vec<CompositionEdgeInputV3> = leaves
        .iter()
        .enumerate()
        .map(|(index, id)| CompositionEdgeInputV3 {
            child_id: *id,
            child_index: u32::try_from(index).expect("child index"),
            coefficient: *root_numerators.get(index).expect("root numerator"),
        })
        .collect();
    let mut terms: Vec<SparseTermV3> = (0..root_numerators.len())
        .map(|index| SparseTermV3 {
            outcome: u32::try_from(index).expect("leaf outcome"),
            numerator: 1,
        })
        .collect();
    terms.extend((0..root_numerators.len()).map(|index| SparseTermV3 {
        outcome: u32::try_from(index).expect("root outcome"),
        numerator: *root_numerators.get(index).expect("root numerator"),
    }));

    let node_count = u32::try_from(nodes.len()).expect("node count");
    let term_count = u32::try_from(terms.len()).expect("term count");
    let graph_length =
        composition_graph_bytes_v3(node_count, width, term_count).expect("graph width");
    let mut graph_scratch = vec![0_u8; graph_length];
    let mut graph = vec![0_u8; graph_length];
    encode_composition_graph_v3_atomic(
        CompositionGraphInputV3 {
            graph_id: basis.source_graph_id,
            root_id: basis.root_id,
            outcome_count: width,
            nodes: &nodes,
            edges: &edges,
            terms: &terms,
        },
        &mut graph_scratch,
        &mut graph,
    )
    .expect("composition graph");

    let translation_length = composition_translation_bytes_v3(width).expect("translation width");
    let mut translation_scratch = vec![0_u8; translation_length];
    let mut translation = vec![0_u8; translation_length];
    encode_canonical_translation_v3_atomic(
        CanonicalTranslationInputV3 {
            graph_id: basis.source_graph_id,
            root_id: basis.root_id,
            outcome_count: width,
            denominator: root_denominator,
            terms: terms.get(root_numerators.len()..).expect("root terms"),
        },
        &mut translation_scratch,
        &mut translation,
    )
    .expect("canonical translation");

    let mut descriptor_scratch = [0_u8; COMPOSITION_DESCRIPTOR_BYTES_V3];
    let mut descriptor = [0_u8; COMPOSITION_DESCRIPTOR_BYTES_V3];
    encode_composition_descriptor_v3_atomic(
        CompositionDescriptorInputV3 {
            market: basis.market,
            result_domain: basis.result_domain,
            release_set: basis.release_set,
            native_basis: basis.representation_basis,
            graph_id: basis.source_graph_id,
            graph_digest: digest(&graph),
            root_id: basis.root_id,
            translation_id: basis.translation_id,
            translation_digest: digest(&translation),
            outcome_count: width,
            node_count,
            edge_count: width,
            term_count,
            root_denominator,
        },
        &mut descriptor_scratch,
        &mut descriptor,
    )
    .expect("composition descriptor");

    Composition {
        descriptor: descriptor.to_vec(),
        graph,
        translation,
        graph_id: basis.source_graph_id,
        translation_id: basis.translation_id,
    }
}

/// The finalized Product-to-Claims exposure record the chain holds.
///
/// `row_numerators` is the sparse weight each representation coordinate places
/// on its own Product coordinate; the canonical instrument uses `1` everywhere,
/// and a hostile disagrees at exactly one coordinate.
pub fn exposure_bytes(basis: &StructuredBasis, row_numerators: &[u64]) -> Vec<u8> {
    let width = basis.representation_width();
    assert_eq!(row_numerators.len(), basis.coefficients.len());
    let node_ids: Vec<[u8; 32]> = (0..row_numerators.len())
        .map(|index| {
            let mut value = [0_u8; 32];
            value[0] = 0xc0;
            value[1] = u8::try_from(index).expect("row index");
            value[31] = 0xa5;
            value
        })
        .collect();
    let row_terms: Vec<[CompositionExposureTermV3; 1]> = row_numerators
        .iter()
        .enumerate()
        .map(|(index, numerator)| {
            [CompositionExposureTermV3 {
                product_coordinate: u32::try_from(index).expect("product coordinate"),
                numerator: *numerator,
            }]
        })
        .collect();
    let rows: Vec<CompositionExposureRowInputV3<'_>> = node_ids
        .iter()
        .enumerate()
        .map(|(index, node_id)| CompositionExposureRowInputV3 {
            node_id: *node_id,
            denominator: 1,
            terms: row_terms.get(index).expect("row terms").as_slice(),
        })
        .collect();
    let length = composition_exposure_bytes_v3(width, width).expect("exposure record width");
    let mut scratch = vec![0_u8; length];
    let mut output = vec![0_u8; length];
    encode_composition_exposure_v3_atomic(
        CompositionExposureInputV3 {
            market: basis.market,
            result_domain: basis.result_domain,
            release_set: basis.release_set,
            product_basis: basis.product_basis,
            representation_basis: basis.representation_basis,
            graph_id: basis.source_graph_id,
            product_width: basis.product_width,
            rows: &rows,
        },
        &mut scratch,
        &mut output,
    )
    .expect("canonical exposure record");
    output
}

/// The shard layer the Structured terms bind to.
pub fn shard_terms_bytes(basis: &StructuredBasis) -> Vec<u8> {
    let size =
        fractional_exposure_terms_bytes_v2(basis.shard_mints.len()).expect("shard terms width");
    let mut scratch = vec![0_u8; size];
    let mut output = vec![0_u8; size];
    encode_fractional_exposure_terms_v2(
        FractionalExposureTermsInputV2 {
            market: basis.market,
            product_record: basis.product_record,
            result_domain: basis.result_domain,
            release_set: basis.release_set,
            token_program: basis.token_program,
            token_behavior: basis.shard_token_behavior,
            exposure_id: basis.exposure_id,
            product_basis: basis.product_basis,
            representation_basis: basis.representation_basis,
            graph_id: basis.source_graph_id,
            product_width: basis.product_width,
            denominator: basis.denominator,
            shard_mints: &basis.shard_mints,
        },
        &mut scratch,
        &mut output,
    )
    .expect("encode shard terms");
    output
}

pub fn shard_terms(bytes: &[u8]) -> FractionalExposureTermsV2<'_> {
    let content = digest(bytes);
    FractionalExposureTermsV2::decode(
        bytes,
        FractionalExposureTermsAdmissionV2 {
            selected_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            finalized_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            selected_terms_id: content,
            finalized_terms_id: content,
            recomputed_terms_digest: content,
            finalized_terms_digest: content,
            record_authenticated: true,
        },
    )
    .expect("decode shard terms")
}

/// The immutable Structured terms one founding finalizes.
pub fn terms_bytes(basis: &StructuredBasis, coefficients: &[u64]) -> Vec<u8> {
    let size = structured_terms_bytes_v2(coefficients.len()).expect("terms width");
    let mut scratch = vec![0_u8; size];
    let mut output = vec![0_u8; size];
    encode_structured_terms_v2(
        StructuredTermsInputV2 {
            market: basis.market,
            product_record: basis.product_record,
            result_domain: basis.result_domain,
            release_set: basis.release_set,
            token_program: basis.token_program,
            token_behavior: basis.receipt_token_behavior,
            shard_terms: digest(&shard_terms_bytes(basis)),
            shard_exposure: basis.exposure_id,
            receipt_mint: basis.receipt_mint,
            graph_id: basis.source_graph_id,
            denominator: basis.denominator,
            coefficients,
        },
        &mut scratch,
        &mut output,
    )
    .expect("encode Structured terms");
    output
}

pub fn decode_terms<'a>(bytes: &'a [u8], shard_bytes: &'a [u8]) -> StructuredTermsV2<'a> {
    let content = digest(bytes);
    StructuredTermsV2::decode(
        bytes,
        StructuredTermsAdmissionV2 {
            selected_schema_id: STRUCTURED_TERMS_SCHEMA_ID_V2,
            finalized_schema_id: STRUCTURED_TERMS_SCHEMA_ID_V2,
            selected_terms_id: content,
            finalized_terms_id: content,
            recomputed_terms_digest: content,
            finalized_terms_digest: content,
            record_authenticated: true,
        },
        shard_terms(shard_bytes),
    )
    .expect("decode Structured terms")
}

/// One Structured product's lowering: every record, and the derived descriptor.
pub struct StructuredLowering {
    /// Exact bytes of the finalized exposure record the chain holds.
    pub exposure: Vec<u8>,
    /// The derived Rational execution descriptor.
    pub descriptor: StructuredRepresentationDescriptorV2,
    /// The immutable Structured terms this lowering read.
    pub terms: Vec<u8>,
    /// The shard layer those terms bind to.
    pub shard_terms: Vec<u8>,
}

/// Derive one Structured product's Rational execution descriptor.
///
/// Everything the descriptor carries comes out of `composition`/`exposure`/
/// `terms`, and the derivation refuses if the three disagree. Nothing here
/// hand-fills a preimage field.
pub fn lower(basis: &StructuredBasis) -> StructuredLowering {
    let composition = composition(basis, &basis.coefficients, basis.denominator);
    let exposure = exposure_bytes(basis, &vec![1; basis.coefficients.len()]);
    let terms_source = terms_bytes(basis, &basis.coefficients);
    let shard_source = shard_terms_bytes(basis);
    let terms = decode_terms(&terms_source, &shard_source);
    let exposure_bundle = CompositionExposureBundleV3::decode(
        &exposure,
        admission(basis.exposure_id, digest(&exposure)),
    )
    .expect("authenticated exposure bundle");
    let descriptor = derive_structured_representation_descriptor_v2(
        terms,
        composition.bundle(),
        exposure_bundle,
    )
    .expect("derived Structured representation descriptor");
    StructuredLowering {
        exposure,
        descriptor,
        terms: terms_source,
        shard_terms: shard_source,
    }
}

/// Try the derivation against a composition whose root is `root_numerators`.
///
/// The campaign uses this to make
/// `require_coefficients_are_the_composition_root` — the join the LIVE chain
/// route lost when `authenticate_exposure` replaced `authenticate_graph`
/// (0011 §3d) — an executed refusal rather than a claim. `root_numerators` must
/// still state a CANONICAL composition (the graph encoder refuses a root whose
/// numerators share a factor with its denominator), so the disagreement is a
/// recipe disagreement and not a malformed record.
pub fn lower_against_root(
    basis: &StructuredBasis,
    root_numerators: &[u64],
) -> core::result::Result<StructuredRepresentationDescriptorV2, dclutch_operator::structured::Error>
{
    let composition = composition(basis, root_numerators, basis.denominator);
    let exposure = exposure_bytes(basis, &vec![1; basis.coefficients.len()]);
    let terms_source = terms_bytes(basis, &basis.coefficients);
    let shard_source = shard_terms_bytes(basis);
    let terms = decode_terms(&terms_source, &shard_source);
    let exposure_bundle = CompositionExposureBundleV3::decode(
        &exposure,
        admission(basis.exposure_id, digest(&exposure)),
    )
    .expect("authenticated exposure bundle");
    derive_structured_representation_descriptor_v2(terms, composition.bundle(), exposure_bundle)
}

/// Decode Structured terms whose receipt Mint ALIASES coordinate zero's shard Mint.
///
/// The physical form of the rank rule, enforced by
/// `StructuredTermsV2::bind_shard_terms`: a receipt can never be backed by
/// itself. This is the founding-time half of the campaign's
/// receipt-backed-by-receipt hostile; the chain-side half substitutes the
/// receipt Mint into a coordinate's asset row and account frame, where it is
/// refused for a different and independent reason (a shard Mint is a
/// descriptor-keyed PDA and the receipt Mint is not).
pub fn decode_terms_with_receipt_aliasing_a_shard_mint(
    basis: &StructuredBasis,
) -> core::result::Result<(), dclutch_claims::structured_kernel::Error> {
    let mut aliased = basis.clone();
    aliased.receipt_mint = *basis.shard_mints.first().expect("shard Mint");
    let terms_source = terms_bytes(&aliased, &aliased.coefficients);
    let shard_source = shard_terms_bytes(&aliased);
    let content = digest(&terms_source);
    StructuredTermsV2::decode(
        &terms_source,
        StructuredTermsAdmissionV2 {
            selected_schema_id: STRUCTURED_TERMS_SCHEMA_ID_V2,
            finalized_schema_id: STRUCTURED_TERMS_SCHEMA_ID_V2,
            selected_terms_id: content,
            finalized_terms_id: content,
            recomputed_terms_digest: content,
            finalized_terms_digest: content,
            record_authenticated: true,
        },
        shard_terms(&shard_source),
    )
    .map(|_| ())
}
