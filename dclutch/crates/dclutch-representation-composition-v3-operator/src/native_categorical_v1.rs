//! Canonical Product-native basis composition publication.
//!
//! Native Claims positions already carry one coordinate per categorical
//! Product outcome. Their execution exposure is therefore the identity map
//! `K = N`, at exact denominator one. This compiler derives every stable graph
//! identity from the authenticated Market, result-domain, release, and
//! ProductBasis facts; callers cannot supply a parallel graph name, node name,
//! translation name, width, or scale.
//!
//! No Rational execution descriptor is emitted here. A native Position has no
//! Rational receipt Mint or one aggregate receipt recipe, and inventing either
//! would create a second semantic truth.

use dclutch_product::payoff::{
    price_gate_v1::verify_price_gate_v1,
    runtime_v3::{
        BasisKindV3, ProductBasisV3, SEMANTIC_BASIS_CONTENT_DOMAIN_V3, semantic_basis_preimage_v3,
    },
};
use dclutch_product::{PortfolioV2, ResultDomainV2, join_product_v2};
use dclutch_product::admission::ProductRecordV2;
use dclutch_claims::composition::{
    COMPOSITION_DESCRIPTOR_BYTES_V3, COMPOSITION_DESCRIPTOR_SCHEMA_ID_V3,
    COMPOSITION_EXPOSURE_SCHEMA_ID_V3, COMPOSITION_GRAPH_SCHEMA_ID_V3,
    COMPOSITION_TRANSLATION_SCHEMA_ID_V3, CanonicalTranslationInputV3,
    CompositionDescriptorInputV3, CompositionEdgeInputV3, CompositionExposureInputV3,
    CompositionExposureRowInputV3, CompositionExposureTermV3, CompositionGraphInputV3,
    CompositionNodeInputV3, CompositionNodeKindV3, SparseTermV3, composition_exposure_bytes_v3,
    composition_graph_bytes_v3, composition_translation_bytes_v3,
    encode_canonical_translation_v3_atomic, encode_composition_descriptor_v3_atomic,
    encode_composition_exposure_v3_atomic, encode_composition_graph_v3_atomic,
};
use solana_program::hash::{hash, hashv};

use crate::{Error, PublicationTargetV3, Result, validate_publication_candidates_v3};

const GRAPH_ID_DOMAIN_V1: &[u8] = b"dclutch/native-categorical-composition/graph/v1";
const ROOT_ID_DOMAIN_V1: &[u8] = b"dclutch/native-categorical-composition/root/v1";
const LEAF_ID_DOMAIN_V1: &[u8] = b"dclutch/native-categorical-composition/leaf/v1";
const TRANSLATION_ID_DOMAIN_V1: &[u8] = b"dclutch/native-categorical-composition/translation/v1";
const BASIS_GRAPH_ID_DOMAIN_V1: &[u8] = b"dclutch/native-basis-composition/graph/v1";
const BASIS_ROOT_ID_DOMAIN_V1: &[u8] = b"dclutch/native-basis-composition/root/v1";
const BASIS_LEAF_ID_DOMAIN_V1: &[u8] = b"dclutch/native-basis-composition/leaf/v1";
const BASIS_TRANSLATION_ID_DOMAIN_V1: &[u8] = b"dclutch/native-basis-composition/translation/v1";

/// Exact semantic-owner bodies used to derive one native basis bundle.
///
/// A spline basis carries the exact `DCLTPGT1` body named by its authenticated
/// digest. Exempt categorical and graded bases must not carry one.
#[derive(Clone, Copy, Debug)]
pub struct NativeBasisCompositionInputV1<'a> {
    /// Canonical Core Market PDA selected for terminal payout.
    pub market: [u8; 32],
    /// Immutable activated release set.
    pub release_set: [u8; 32],
    /// Exact Product root record bytes.
    pub product_record_bytes: &'a [u8],
    /// Exact Product result-domain record bytes.
    pub result_domain_bytes: &'a [u8],
    /// Exact Product portfolio record bytes.
    pub portfolio_bytes: &'a [u8],
    /// Exact linked ProductBasisV3 record bytes.
    pub product_basis_bytes: &'a [u8],
    /// Exact linked price-gate body, present only when the basis names it.
    pub price_gate_bytes: Option<&'a [u8]>,
}

/// Exact semantic-owner bodies used to derive one native categorical bundle.
///
/// The four record bodies are pre-publication candidates. Their Product
/// children and ProductBasis are re-decoded and content-joined here before any
/// composition byte is emitted.
#[derive(Clone, Copy, Debug)]
pub struct NativeCategoricalCompositionInputV1<'a> {
    /// Canonical Core Market PDA selected for terminal payout.
    pub market: [u8; 32],
    /// Immutable activated release set.
    pub release_set: [u8; 32],
    /// Exact Product root record bytes.
    pub product_record_bytes: &'a [u8],
    /// Exact Product result-domain record bytes.
    pub result_domain_bytes: &'a [u8],
    /// Exact Product portfolio record bytes.
    pub portfolio_bytes: &'a [u8],
    /// Exact linked ProductBasisV3 record bytes.
    pub product_basis_bytes: &'a [u8],
}

/// Four canonical Registry bodies required by native basis terminal payout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeBasisCompositionRecordsV1 {
    descriptor: Vec<u8>,
    graph: Vec<u8>,
    translation: Vec<u8>,
    exposure: Vec<u8>,
    graph_id: [u8; 32],
    root_id: [u8; 32],
    translation_id: [u8; 32],
    representation_basis: [u8; 32],
    width: u32,
    basis_kind: BasisKindV3,
    payout_scale: u64,
}

impl NativeBasisCompositionRecordsV1 {
    /// Exact composition-descriptor bytes.
    pub fn descriptor(&self) -> &[u8] {
        &self.descriptor
    }

    /// Exact canonical graph bytes.
    pub fn graph(&self) -> &[u8] {
        &self.graph
    }

    /// Exact canonical root-translation bytes.
    pub fn translation(&self) -> &[u8] {
        &self.translation
    }

    /// Exact Product-to-native-Claims identity exposure bytes.
    pub fn exposure(&self) -> &[u8] {
        &self.exposure
    }

    /// Domain-separated stable graph identity.
    pub const fn graph_id(&self) -> [u8; 32] {
        self.graph_id
    }

    /// Domain-separated sole graph-root identity.
    pub const fn root_id(&self) -> [u8; 32] {
        self.root_id
    }

    /// Domain-separated canonical-translation identity.
    pub const fn translation_id(&self) -> [u8; 32] {
        self.translation_id
    }

    /// Semantic native liability-basis identity.
    pub const fn representation_basis(&self) -> [u8; 32] {
        self.representation_basis
    }

    /// Exact Product and Claims width `K = N`.
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Authenticated ProductBasisV3 evaluator family.
    pub const fn basis_kind(&self) -> BasisKindV3 {
        self.basis_kind
    }

    /// Authenticated exact ProductBasisV3 payout partition scale.
    pub const fn payout_scale(&self) -> u64 {
        self.payout_scale
    }

    /// Canonically ordered immutable Registry publication targets.
    pub fn publication_targets(&self) -> [PublicationTargetV3<'_>; 4] {
        [
            PublicationTargetV3 {
                schema_id: COMPOSITION_DESCRIPTOR_SCHEMA_ID_V3,
                bytes: &self.descriptor,
            },
            PublicationTargetV3 {
                schema_id: COMPOSITION_GRAPH_SCHEMA_ID_V3,
                bytes: &self.graph,
            },
            PublicationTargetV3 {
                schema_id: COMPOSITION_TRANSLATION_SCHEMA_ID_V3,
                bytes: &self.translation,
            },
            PublicationTargetV3 {
                schema_id: COMPOSITION_EXPOSURE_SCHEMA_ID_V3,
                bytes: &self.exposure,
            },
        ]
    }
}

/// Byte-compatible categorical result returned by the legacy strict wrapper.
pub type NativeCategoricalCompositionRecordsV1 = NativeBasisCompositionRecordsV1;

#[derive(Clone, Copy)]
struct IdentityDomainsV1 {
    graph: &'static [u8],
    root: &'static [u8],
    leaf: &'static [u8],
    translation: &'static [u8],
}

const CATEGORICAL_IDENTITY_DOMAINS_V1: IdentityDomainsV1 = IdentityDomainsV1 {
    graph: GRAPH_ID_DOMAIN_V1,
    root: ROOT_ID_DOMAIN_V1,
    leaf: LEAF_ID_DOMAIN_V1,
    translation: TRANSLATION_ID_DOMAIN_V1,
};

const BASIS_IDENTITY_DOMAINS_V1: IdentityDomainsV1 = IdentityDomainsV1 {
    graph: BASIS_GRAPH_ID_DOMAIN_V1,
    root: BASIS_ROOT_ID_DOMAIN_V1,
    leaf: BASIS_LEAF_ID_DOMAIN_V1,
    translation: BASIS_TRANSLATION_ID_DOMAIN_V1,
};

#[derive(Clone, Copy)]
struct IdentityFactsV1 {
    domains: IdentityDomainsV1,
    market: [u8; 32],
    result_domain: [u8; 32],
    release_set: [u8; 32],
    product_basis: [u8; 32],
    representation_basis: [u8; 32],
    representation_release: [u8; 32],
    mapping_release: [u8; 32],
    width: u32,
    payout_scale: u64,
}

impl IdentityFactsV1 {
    fn graph_id(self) -> [u8; 32] {
        let width = self.width.to_le_bytes();
        let payout_scale = self.payout_scale.to_le_bytes();
        hashv(&[
            self.domains.graph,
            &self.market,
            &self.result_domain,
            &self.release_set,
            &self.product_basis,
            &self.representation_basis,
            &self.representation_release,
            &self.mapping_release,
            &width,
            &payout_scale,
        ])
        .to_bytes()
    }
}

fn root_id(domain: &[u8], graph_id: [u8; 32]) -> [u8; 32] {
    hashv(&[domain, &graph_id]).to_bytes()
}

fn leaf_id(domain: &[u8], graph_id: [u8; 32], coordinate: u32) -> [u8; 32] {
    hashv(&[domain, &graph_id, &coordinate.to_le_bytes()]).to_bytes()
}

fn translation_id(domain: &[u8], graph_id: [u8; 32], root_id: [u8; 32]) -> [u8; 32] {
    hashv(&[domain, &graph_id, &root_id]).to_bytes()
}

/// Compile one canonical identity exposure for a native categorical Product.
pub fn compile_native_categorical_composition_v1(
    input: NativeCategoricalCompositionInputV1<'_>,
) -> Result<NativeCategoricalCompositionRecordsV1> {
    let product_basis = ProductBasisV3::decode(input.product_basis_bytes)
        .map_err(Error::ProductPayoffRuntimeCodec)?;
    // A categorical basis carries exactly two admissible payout scales -- the
    // legacy `1` and the refunding ordinary-region count -- and the record's
    // own decoder is the authority on which it is. Restating "scale must be 1"
    // here would have made this operator a second, quieter author of the
    // economics, refusing every refunding market with a cross-record error
    // that named nothing.
    if product_basis.kind() != BasisKindV3::CategoricalQ1
        || (product_basis.payout_scale() != 1 && !product_basis.refunds_on_failure())
    {
        return Err(Error::CrossRecord);
    }
    compile_native_basis_composition_v1(NativeBasisCompositionInputV1 {
        market: input.market,
        release_set: input.release_set,
        product_record_bytes: input.product_record_bytes,
        result_domain_bytes: input.result_domain_bytes,
        portfolio_bytes: input.portfolio_bytes,
        product_basis_bytes: input.product_basis_bytes,
        price_gate_bytes: None,
    })
}

/// Compile one canonical identity exposure for an admitted ProductBasisV3.
///
/// The Product coordinates and native Claims coordinates remain the exact
/// identity map `K = N`; the selected basis controls their payoff semantics and
/// exact payout scale. Spline bases are admitted only alongside the canonical
/// price-gate body named by the basis itself.
pub fn compile_native_basis_composition_v1(
    input: NativeBasisCompositionInputV1<'_>,
) -> Result<NativeBasisCompositionRecordsV1> {
    if input.market == [0; 32] || input.release_set == [0; 32] {
        return Err(Error::CrossRecord);
    }

    let product = ProductRecordV2::decode(input.product_record_bytes)
        .map_err(Error::ProductRuntimeAdmission)?;
    let result_domain =
        ResultDomainV2::decode(input.result_domain_bytes).map_err(Error::ProductRuntime)?;
    let portfolio = PortfolioV2::decode(input.portfolio_bytes).map_err(Error::ProductRuntime)?;
    let result_domain_digest = hash(input.result_domain_bytes).to_bytes();
    let portfolio_digest = hash(input.portfolio_bytes).to_bytes();
    if product.result_domain_digest().to_bytes() != result_domain_digest
        || product.portfolio_digest().to_bytes() != portfolio_digest
        || product.product_id() != result_domain.product_id()
    {
        return Err(Error::Product);
    }
    let joined = join_product_v2(
        dclutch_product::ContentId::new(result_domain_digest)
            .map_err(|_| Error::Product)?,
        dclutch_product::ContentId::new(portfolio_digest).map_err(|_| Error::Product)?,
        result_domain,
        portfolio,
    )
    .map_err(Error::ProductRuntime)?;
    if joined.product_id != product.product_id() {
        return Err(Error::Product);
    }

    let product_basis = ProductBasisV3::decode(input.product_basis_bytes)
        .map_err(Error::ProductPayoffRuntimeCodec)?;
    product_basis
        .admit_selection_v3()
        .map_err(Error::ProductPayoffRuntimeCodec)?;
    let price_gate_digest = product_basis.price_gate_certificate_digest_v3();
    match (price_gate_digest == [0; 32], input.price_gate_bytes) {
        (true, None) => {}
        (true, Some(_)) | (false, None) => return Err(Error::ProductBasis),
        (false, Some(price_gate)) => {
            let expected = price_gate_digest;
            if hash(price_gate).to_bytes() != expected {
                return Err(Error::CrossRecord);
            }
            let degree = match product_basis.kind() {
                BasisKindV3::SplineDegree2To3 { degree, .. } => degree,
                _ => return Err(Error::ProductBasis),
            };
            verify_price_gate_v1(
                &product_basis,
                product_basis.knot_denominator(),
                product_basis.payout_scale(),
                degree,
                product_basis.basis_width(),
                price_gate,
            )
            .map_err(Error::ProductPayoffRuntimeCodec)?;
        }
    }
    let product_basis_digest = hash(input.product_basis_bytes).to_bytes();
    let semantic = semantic_basis_preimage_v3(input.product_basis_bytes)
        .map_err(Error::ProductPayoffRuntimeCodec)?;
    let representation_basis = hashv(&[
        SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
        semantic.prefix(),
        semantic.suffix(),
    ])
    .to_bytes();
    if product_basis.product_id() != joined.product_id.to_bytes()
        || product_basis.result_domain_id() != result_domain_digest
        || product_basis.basis_width() != joined.outcome_count
        || representation_basis != joined.liability_basis_id.to_bytes()
    {
        return Err(Error::CrossRecord);
    }

    let width = joined.outcome_count;
    let node_count = width.checked_add(1).ok_or(Error::Arithmetic)?;
    let term_count = width.checked_mul(2).ok_or(Error::Arithmetic)?;
    let domains = if product_basis.kind() == BasisKindV3::CategoricalQ1 {
        CATEGORICAL_IDENTITY_DOMAINS_V1
    } else {
        BASIS_IDENTITY_DOMAINS_V1
    };
    let facts = IdentityFactsV1 {
        domains,
        market: input.market,
        result_domain: result_domain_digest,
        release_set: input.release_set,
        product_basis: product_basis_digest,
        representation_basis,
        representation_release: result_domain.representation_release_id().to_bytes(),
        mapping_release: result_domain.mapping_release_id().to_bytes(),
        width,
        payout_scale: product_basis.payout_scale(),
    };
    let graph_id = facts.graph_id();
    let root_id = root_id(domains.root, graph_id);
    let translation_id = translation_id(domains.translation, graph_id, root_id);
    if [graph_id, root_id, translation_id]
        .into_iter()
        .any(|identity| identity == [0; 32])
    {
        return Err(Error::Composition);
    }

    let width_usize = usize::try_from(width).map_err(|_| Error::Arithmetic)?;
    let mut leaf_ids_by_outcome = Vec::with_capacity(width_usize);
    for coordinate in 0..width {
        let id = leaf_id(domains.leaf, graph_id, coordinate);
        if id == [0; 32] {
            return Err(Error::Composition);
        }
        leaf_ids_by_outcome.push(id);
    }
    let mut ordered_leaves = leaf_ids_by_outcome
        .iter()
        .copied()
        .enumerate()
        .map(|(coordinate, id)| {
            u32::try_from(coordinate)
                .map(|coordinate| (id, coordinate))
                .map_err(|_| Error::Arithmetic)
        })
        .collect::<Result<Vec<_>>>()?;
    ordered_leaves.sort_unstable_by_key(|(id, _)| *id);
    if ordered_leaves
        .windows(2)
        .any(|pair| matches!(pair, [(left, _), (right, _)] if left == right))
    {
        return Err(Error::Composition);
    }

    let mut nodes = Vec::with_capacity(width_usize.checked_add(1).ok_or(Error::Arithmetic)?);
    let mut edges = Vec::with_capacity(width_usize);
    let mut terms = Vec::with_capacity(width_usize.checked_mul(2).ok_or(Error::Arithmetic)?);
    for (index, (id, outcome)) in ordered_leaves.iter().copied().enumerate() {
        let index = u32::try_from(index).map_err(|_| Error::Arithmetic)?;
        nodes.push(CompositionNodeInputV3 {
            id,
            rank: 0,
            first_edge: 0,
            edge_count: 0,
            first_term: index,
            term_count: 1,
            kind: CompositionNodeKindV3::Native,
            native_outcome: outcome,
            recipe_divisor: 1,
            flattened_denominator: 1,
        });
        terms.push(SparseTermV3 {
            outcome,
            numerator: 1,
        });
        edges.push(CompositionEdgeInputV3 {
            child_id: id,
            child_index: index,
            coefficient: 1,
        });
    }
    nodes.push(CompositionNodeInputV3 {
        id: root_id,
        rank: 1,
        first_edge: 0,
        edge_count: width,
        first_term: width,
        term_count: width,
        kind: CompositionNodeKindV3::Compose,
        native_outcome: 0,
        recipe_divisor: 1,
        flattened_denominator: 1,
    });
    let root_term_start = terms.len();
    for outcome in 0..width {
        terms.push(SparseTermV3 {
            outcome,
            numerator: 1,
        });
    }

    let graph_bytes = composition_graph_bytes_v3(node_count, width, term_count)
        .map_err(Error::RepresentationComposition)?;
    let mut graph_scratch = vec![0; graph_bytes];
    let mut graph = vec![0; graph_bytes];
    encode_composition_graph_v3_atomic(
        CompositionGraphInputV3 {
            graph_id,
            root_id,
            outcome_count: width,
            nodes: &nodes,
            edges: &edges,
            terms: &terms,
        },
        &mut graph_scratch,
        &mut graph,
    )
    .map_err(Error::RepresentationComposition)?;

    let root_terms = terms.get(root_term_start..).ok_or(Error::Arithmetic)?;
    let translation_bytes =
        composition_translation_bytes_v3(width).map_err(Error::RepresentationComposition)?;
    let mut translation_scratch = vec![0; translation_bytes];
    let mut translation = vec![0; translation_bytes];
    encode_canonical_translation_v3_atomic(
        CanonicalTranslationInputV3 {
            graph_id,
            root_id,
            outcome_count: width,
            denominator: 1,
            terms: root_terms,
        },
        &mut translation_scratch,
        &mut translation,
    )
    .map_err(Error::RepresentationComposition)?;

    let mut descriptor_scratch = [0; COMPOSITION_DESCRIPTOR_BYTES_V3];
    let mut descriptor = [0; COMPOSITION_DESCRIPTOR_BYTES_V3];
    encode_composition_descriptor_v3_atomic(
        CompositionDescriptorInputV3 {
            market: input.market,
            result_domain: result_domain_digest,
            release_set: input.release_set,
            native_basis: representation_basis,
            graph_id,
            graph_digest: hash(&graph).to_bytes(),
            root_id,
            translation_id,
            translation_digest: hash(&translation).to_bytes(),
            outcome_count: width,
            node_count,
            edge_count: width,
            term_count,
            root_denominator: 1,
        },
        &mut descriptor_scratch,
        &mut descriptor,
    )
    .map_err(Error::RepresentationComposition)?;

    let row_terms = (0..width)
        .map(|coordinate| {
            [CompositionExposureTermV3 {
                product_coordinate: coordinate,
                numerator: 1,
            }]
        })
        .collect::<Vec<_>>();
    let rows = leaf_ids_by_outcome
        .iter()
        .copied()
        .zip(row_terms.iter())
        .map(|(node_id, terms)| CompositionExposureRowInputV3 {
            node_id,
            denominator: 1,
            terms,
        })
        .collect::<Vec<_>>();
    let exposure_bytes =
        composition_exposure_bytes_v3(width, width).map_err(Error::RepresentationComposition)?;
    let mut exposure_scratch = vec![0; exposure_bytes];
    let mut exposure = vec![0; exposure_bytes];
    encode_composition_exposure_v3_atomic(
        CompositionExposureInputV3 {
            market: input.market,
            result_domain: result_domain_digest,
            release_set: input.release_set,
            product_basis: product_basis_digest,
            representation_basis,
            graph_id,
            product_width: width,
            rows: &rows,
        },
        &mut exposure_scratch,
        &mut exposure,
    )
    .map_err(Error::RepresentationComposition)?;

    validate_publication_candidates_v3(
        input.product_basis_bytes,
        &descriptor,
        &graph,
        &translation,
        &exposure,
    )?;
    Ok(NativeBasisCompositionRecordsV1 {
        descriptor: descriptor.to_vec(),
        graph,
        translation,
        exposure,
        graph_id,
        root_id,
        translation_id,
        representation_basis,
        width,
        basis_kind: product_basis.kind(),
        payout_scale: product_basis.payout_scale(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]

    use super::*;
    use dclutch_product::payoff::{
        price_gate_v1::{
            PRICE_GATE_ATOM_COUNT_OFFSET_V1, PRICE_GATE_DEGREE_OFFSET_V1,
            PRICE_GATE_DENOMINATORS_OFFSET_V1, PRICE_GATE_MAGIC_OFFSET_V1, PRICE_GATE_MAGIC_V1,
            PRICE_GATE_MASS_OFFSET_V1, PRICE_GATE_NUMERATORS_OFFSET_V1,
            PRICE_GATE_PRICES_OFFSET_V1, PRICE_GATE_PROFILE_OFFSET_V1, PRICE_GATE_PROFILE_V1,
            PRICE_GATE_REQUEST_BYTES_V1, PRICE_GATE_SCALE_OFFSET_V1, PRICE_GATE_SCHEMA_VERSION_V1,
            PRICE_GATE_VERSION_OFFSET_V1, PRICE_GATE_WEIGHTS_OFFSET_V1, PRICE_GATE_WIDTH_OFFSET_V1,
        },
        runtime_v3::{BasisInputV3, basis_record_bytes_v3, compile_basis_v3},
    };
    use dclutch_product::{
        ContentId, portfolio_record_bytes, result_domain_record_bytes,
    };
    use dclutch_product::admission::PRODUCT_RECORD_BYTES_V2;
    use dclutch_product_runtime_v2_operator::{
        ProductCompilationInputV2, compile_product_records_v2,
        spline_basis_v3::{
            SplineProductCompilationInputV3, compile_spline_product_records_v3,
            spline_basis_output_bytes_v3,
        },
    };
    use dclutch_claims::composition::{
        COMPOSITION_EXPOSURE_HEADER_BYTES_V3, COMPOSITION_EXPOSURE_ROW_BYTES_V3,
        COMPOSITION_EXPOSURE_TERM_BYTES_V3, COMPOSITION_GRAPH_HEADER_BYTES_V3,
        COMPOSITION_NODE_BYTES_V3, CompositionExposureRowLayoutV3, CompositionExposureTermLayoutV3,
        DescriptorLayoutV3, GraphLayoutV3, NodeLayoutV3,
    };
    use solana_program::pubkey::Pubkey;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn content(value: u8) -> ContentId {
        ContentId::new(id(value)).expect("nonzero identity")
    }

    fn basis(
        width: u32,
        product_id: [u8; 32],
        result_domain: [u8; 32],
        payout_scale: u64,
    ) -> Vec<u8> {
        let mut bytes = vec![
            0;
            basis_record_bytes_v3(BasisKindV3::CategoricalQ1, width as usize, 0, 0)
                .expect("basis width")
        ];
        compile_basis_v3(
            BasisInputV3 {
                kind: BasisKindV3::CategoricalQ1,
                product_id,
                result_domain_id: result_domain,
                coordinate_domain_id: id(3),
                result_unit_id: id(4),
                evaluator_release_id: id(5),
                basis_width: width,
                payout_scale,
                knot_denominator: 1,
                knots: &[],
                terms: &[],
                failure_payouts: &[],
                // Exempt by proof: degree 0 and 1 need no price gate,
                // and a digest offered alongside one is refused.
                price_gate_certificate_digest: [0_u8; 32],
            },
            &mut bytes,
        )
        .expect("categorical basis");
        bytes
    }

    fn semantic_basis(bytes: &[u8]) -> [u8; 32] {
        let semantic = semantic_basis_preimage_v3(bytes).expect("semantic basis preimage");
        hashv(&[
            SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
            semantic.prefix(),
            semantic.suffix(),
        ])
        .to_bytes()
    }

    struct Fixture {
        product: Vec<u8>,
        domain: Vec<u8>,
        portfolio: Vec<u8>,
        basis: Vec<u8>,
    }

    struct CubicFixture {
        product: Vec<u8>,
        domain: Vec<u8>,
        portfolio: Vec<u8>,
        basis: Vec<u8>,
        gate: [u8; PRICE_GATE_REQUEST_BYTES_V1],
    }

    impl CubicFixture {
        fn new() -> Self {
            const SCALE: u64 = 11;
            let mut gate = [0_u8; PRICE_GATE_REQUEST_BYTES_V1];
            gate[PRICE_GATE_MAGIC_OFFSET_V1..PRICE_GATE_MAGIC_OFFSET_V1 + 8]
                .copy_from_slice(&PRICE_GATE_MAGIC_V1);
            gate[PRICE_GATE_VERSION_OFFSET_V1..PRICE_GATE_VERSION_OFFSET_V1 + 2]
                .copy_from_slice(&PRICE_GATE_SCHEMA_VERSION_V1.to_le_bytes());
            gate[PRICE_GATE_PROFILE_OFFSET_V1..PRICE_GATE_PROFILE_OFFSET_V1 + 2]
                .copy_from_slice(&PRICE_GATE_PROFILE_V1.to_le_bytes());
            gate[PRICE_GATE_SCALE_OFFSET_V1..PRICE_GATE_SCALE_OFFSET_V1 + 4]
                .copy_from_slice(&u32::try_from(SCALE).expect("scale").to_le_bytes());
            gate[PRICE_GATE_MASS_OFFSET_V1..PRICE_GATE_MASS_OFFSET_V1 + 8]
                .copy_from_slice(&1_u64.to_le_bytes());
            gate[PRICE_GATE_DEGREE_OFFSET_V1] = 3;
            gate[PRICE_GATE_WIDTH_OFFSET_V1] = 4;
            gate[PRICE_GATE_ATOM_COUNT_OFFSET_V1] = 1;
            for (claim, payout) in [1_u64, 4, 4, 2].iter().enumerate() {
                let offset = PRICE_GATE_PRICES_OFFSET_V1 + claim * 8;
                gate[offset..offset + 8].copy_from_slice(&payout.to_le_bytes());
            }
            gate[PRICE_GATE_WEIGHTS_OFFSET_V1..PRICE_GATE_WEIGHTS_OFFSET_V1 + 8]
                .copy_from_slice(&1_u64.to_le_bytes());
            gate[PRICE_GATE_NUMERATORS_OFFSET_V1..PRICE_GATE_NUMERATORS_OFFSET_V1 + 8]
                .copy_from_slice(&3_i64.to_le_bytes());
            gate[PRICE_GATE_DENOMINATORS_OFFSET_V1..PRICE_GATE_DENOMINATORS_OFFSET_V1 + 4]
                .copy_from_slice(&2_u32.to_le_bytes());

            let cuts = [1_i128, 2];
            let coefficients = [1_u64; 4];
            let knots = [0_i128, 0, 0, 0, 3, 3, 3, 3];
            let failure = [0_u64, 0, 0, SCALE];
            let input = SplineProductCompilationInputV3 {
                product_id: content(1),
                coordinate_domain_id: content(3),
                result_unit_id: content(4),
                claim_basis_id: content(8),
                representation_release_id: content(6),
                mapping_release_id: content(7),
                cut_denominator: 1,
                cuts: &cuts,
                portfolio_denominator: 1,
                coefficients: &coefficients,
                evaluator_release_id: content(5),
                degree: 3,
                interior_multiplicity: false,
                payout_scale: SCALE,
                knot_denominator: 1,
                knots: &knots,
                failure_payouts: &failure,
                price_gate_certificate: &gate,
            };
            let mut product = vec![0; PRODUCT_RECORD_BYTES_V2];
            let mut domain = vec![0; result_domain_record_bytes(2).expect("domain width")];
            let mut portfolio = vec![0; portfolio_record_bytes(4).expect("portfolio width")];
            let mut basis = vec![0; spline_basis_output_bytes_v3(input).expect("basis width")];
            compile_spline_product_records_v3(
                Pubkey::new_from_array(id(90)),
                input,
                &mut product,
                &mut domain,
                &mut portfolio,
                &mut basis,
            )
            .expect("cubic Product graph");
            Self {
                product,
                domain,
                portfolio,
                basis,
                gate,
            }
        }

        fn input(&self) -> NativeBasisCompositionInputV1<'_> {
            NativeBasisCompositionInputV1 {
                market: id(80),
                release_set: id(81),
                product_record_bytes: &self.product,
                result_domain_bytes: &self.domain,
                portfolio_bytes: &self.portfolio,
                product_basis_bytes: &self.basis,
                price_gate_bytes: Some(&self.gate),
            }
        }
    }

    impl Fixture {
        fn new(width: u32, payout_scale: u64) -> Self {
            let cut_count =
                usize::try_from(width.checked_sub(2).expect("minimum width")).expect("cut count");
            let cuts = (0..cut_count)
                .map(|value| i128::try_from(value).expect("cut"))
                .collect::<Vec<_>>();
            let coefficients = vec![1_u64; usize::try_from(width).expect("width")];
            let prebasis = basis(width, id(1), id(2), payout_scale);
            let native_basis = semantic_basis(&prebasis);
            let mut product = vec![0; PRODUCT_RECORD_BYTES_V2];
            let mut domain = vec![0; result_domain_record_bytes(cut_count).expect("domain width")];
            let mut portfolio =
                vec![0; portfolio_record_bytes(coefficients.len()).expect("portfolio width")];
            compile_product_records_v2(
                Pubkey::new_from_array(id(90)),
                ProductCompilationInputV2 {
                    product_id: content(1),
                    coordinate_domain_id: content(3),
                    result_unit_id: content(4),
                    claim_basis_id: ContentId::new(native_basis).expect("claim basis"),
                    liability_basis_id: ContentId::new(native_basis).expect("liability basis"),
                    representation_release_id: content(6),
                    mapping_release_id: content(7),
                    cut_denominator: 1,
                    cuts: &cuts,
                    portfolio_denominator: 1,
                    coefficients: &coefficients,
                },
                &mut product,
                &mut domain,
                &mut portfolio,
            )
            .expect("Product graph");
            let domain_id = hash(&domain).to_bytes();
            let basis = basis(width, id(1), domain_id, payout_scale);
            assert_eq!(semantic_basis(&basis), native_basis);
            Self {
                product,
                domain,
                portfolio,
                basis,
            }
        }

        fn input(&self) -> NativeCategoricalCompositionInputV1<'_> {
            NativeCategoricalCompositionInputV1 {
                market: id(80),
                release_set: id(81),
                product_record_bytes: &self.product,
                result_domain_bytes: &self.domain,
                portfolio_bytes: &self.portfolio,
                product_basis_bytes: &self.basis,
            }
        }
    }

    fn u32_at(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("u32 field"))
    }

    fn u64_at(bytes: &[u8], offset: usize) -> u64 {
        u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("u64 field"))
    }

    #[test]
    fn native_identity_is_canonical_even_when_leaf_hash_order_differs() {
        let fixture = Fixture::new(4, 1);
        let output = compile_native_categorical_composition_v1(fixture.input()).expect("compiler");
        assert_eq!(output.width(), 4);
        assert_eq!(output.payout_scale(), 1);
        assert_eq!(output.basis_kind(), BasisKindV3::CategoricalQ1);
        assert_eq!(
            output.publication_targets().map(|target| target.schema_id),
            [
                COMPOSITION_DESCRIPTOR_SCHEMA_ID_V3,
                COMPOSITION_GRAPH_SCHEMA_ID_V3,
                COMPOSITION_TRANSLATION_SCHEMA_ID_V3,
                COMPOSITION_EXPOSURE_SCHEMA_ID_V3,
            ]
        );
        assert_eq!(
            u32_at(output.graph(), GraphLayoutV3::NODE_COUNT),
            output.width() + 1
        );
        let graph_leaf_ids = (0..output.width())
            .map(|index| {
                let start = COMPOSITION_GRAPH_HEADER_BYTES_V3
                    + usize::try_from(index).expect("index") * COMPOSITION_NODE_BYTES_V3;
                output.graph()[start + NodeLayoutV3::ID..start + NodeLayoutV3::ID + 32]
                    .try_into()
                    .expect("node id")
            })
            .collect::<Vec<[u8; 32]>>();
        assert!(graph_leaf_ids.windows(2).all(|pair| pair[0] < pair[1]));
        assert_ne!(
            graph_leaf_ids,
            (0..4)
                .map(|coordinate| {
                    leaf_id(
                        CATEGORICAL_IDENTITY_DOMAINS_V1.leaf,
                        output.graph_id(),
                        coordinate,
                    )
                })
                .collect::<Vec<_>>()
        );

        let terms_start = COMPOSITION_EXPOSURE_HEADER_BYTES_V3
            + usize::try_from(output.width()).expect("width") * COMPOSITION_EXPOSURE_ROW_BYTES_V3;
        for coordinate in 0..output.width() {
            let row = COMPOSITION_EXPOSURE_HEADER_BYTES_V3
                + usize::try_from(coordinate).expect("coordinate")
                    * COMPOSITION_EXPOSURE_ROW_BYTES_V3;
            assert_eq!(
                &output.exposure()[row + CompositionExposureRowLayoutV3::NODE_ID
                    ..row + CompositionExposureRowLayoutV3::NODE_ID + 32],
                &leaf_id(
                    CATEGORICAL_IDENTITY_DOMAINS_V1.leaf,
                    output.graph_id(),
                    coordinate,
                )
            );
            assert_eq!(
                u32_at(
                    output.exposure(),
                    row + CompositionExposureRowLayoutV3::REPRESENTATION_COORDINATE,
                ),
                coordinate
            );
            assert_eq!(
                u64_at(
                    output.exposure(),
                    row + CompositionExposureRowLayoutV3::DENOMINATOR,
                ),
                1
            );
            let term = terms_start
                + usize::try_from(coordinate).expect("coordinate")
                    * COMPOSITION_EXPOSURE_TERM_BYTES_V3;
            assert_eq!(
                u32_at(
                    output.exposure(),
                    term + CompositionExposureTermLayoutV3::PRODUCT_COORDINATE,
                ),
                coordinate
            );
            assert_eq!(
                u64_at(
                    output.exposure(),
                    term + CompositionExposureTermLayoutV3::NUMERATOR,
                ),
                1
            );
        }
    }

    #[test]
    fn cubic_scale_eleven_is_one_admitted_identity_mapping() {
        let fixture = CubicFixture::new();
        let output = compile_native_basis_composition_v1(fixture.input()).expect("cubic compiler");
        assert_eq!(output.width(), 4);
        assert_eq!(output.payout_scale(), 11);
        assert_eq!(
            output.basis_kind(),
            BasisKindV3::SplineDegree2To3 {
                degree: 3,
                interior_multiplicity: false,
            }
        );
        assert_eq!(
            u64_at(output.descriptor(), DescriptorLayoutV3::ROOT_DENOMINATOR),
            1
        );
        let terms_start = COMPOSITION_EXPOSURE_HEADER_BYTES_V3
            + usize::try_from(output.width()).expect("width") * COMPOSITION_EXPOSURE_ROW_BYTES_V3;
        for coordinate in 0..output.width() {
            let row = COMPOSITION_EXPOSURE_HEADER_BYTES_V3
                + usize::try_from(coordinate).expect("coordinate")
                    * COMPOSITION_EXPOSURE_ROW_BYTES_V3;
            assert_eq!(
                u64_at(
                    output.exposure(),
                    row + CompositionExposureRowLayoutV3::DENOMINATOR,
                ),
                1
            );
            let term = terms_start
                + usize::try_from(coordinate).expect("coordinate")
                    * COMPOSITION_EXPOSURE_TERM_BYTES_V3;
            assert_eq!(
                u32_at(
                    output.exposure(),
                    term + CompositionExposureTermLayoutV3::PRODUCT_COORDINATE,
                ),
                coordinate
            );
            assert_eq!(
                u64_at(
                    output.exposure(),
                    term + CompositionExposureTermLayoutV3::NUMERATOR,
                ),
                1
            );
        }

        assert_eq!(
            compile_native_categorical_composition_v1(NativeCategoricalCompositionInputV1 {
                market: id(80),
                release_set: id(81),
                product_record_bytes: &fixture.product,
                result_domain_bytes: &fixture.domain,
                portfolio_bytes: &fixture.portfolio,
                product_basis_bytes: &fixture.basis,
            })
            .err(),
            Some(Error::CrossRecord)
        );
    }

    #[test]
    fn cubic_gate_and_link_substitutions_refuse() {
        let fixture = CubicFixture::new();
        let mut missing = fixture.input();
        missing.price_gate_bytes = None;
        assert_eq!(
            compile_native_basis_composition_v1(missing).err(),
            Some(Error::ProductBasis)
        );

        let mut forged_gate = fixture.gate;
        forged_gate[PRICE_GATE_PRICES_OFFSET_V1] ^= 1;
        let mut forged = fixture.input();
        forged.price_gate_bytes = Some(&forged_gate);
        assert_eq!(
            compile_native_basis_composition_v1(forged).err(),
            Some(Error::CrossRecord)
        );

        let other = CubicFixture::new();
        let mut substituted_basis = other.basis;
        substituted_basis[32] ^= 1;
        let mut substituted = fixture.input();
        substituted.product_basis_bytes = &substituted_basis;
        assert_eq!(
            compile_native_basis_composition_v1(substituted).err(),
            Some(Error::CrossRecord)
        );
    }

    #[test]
    fn substitutions_scale_width_and_noncanonical_node_order_refuse() {
        let fixture = Fixture::new(4, 1);
        let output = compile_native_categorical_composition_v1(fixture.input()).expect("compiler");

        let mut substituted = fixture.input();
        substituted.market = id(82);
        let other = compile_native_categorical_composition_v1(substituted).expect("other market");
        assert_ne!(output.graph_id(), other.graph_id());
        assert_ne!(output.descriptor(), other.descriptor());

        let mut bad_domain = fixture.domain.clone();
        bad_domain[0] ^= 1;
        let mut substituted = fixture.input();
        substituted.result_domain_bytes = &bad_domain;
        assert_eq!(
            compile_native_categorical_composition_v1(substituted).err(),
            Some(Error::ProductRuntime(
                dclutch_product::Error::InvalidMagic
            ))
        );

        let mut scaled_basis = fixture.basis.clone();
        scaled_basis[160..168].copy_from_slice(&2_u64.to_le_bytes());
        let mut scaled = fixture.input();
        scaled.product_basis_bytes = &scaled_basis;
        assert_eq!(
            compile_native_categorical_composition_v1(scaled).err(),
            Some(Error::ProductPayoffRuntimeCodec(
                dclutch_product::payoff::runtime_v3::Error::NonCanonicalReserved
            ))
        );
        let too_wide = Fixture::new(32, 1);
        assert_eq!(
            compile_native_categorical_composition_v1(too_wide.input()).err(),
            Some(Error::RepresentationComposition(
                dclutch_claims::composition::Error::CapacityExceeded
            ))
        );

        let mut graph = output.graph().to_vec();
        let first = COMPOSITION_GRAPH_HEADER_BYTES_V3;
        let second = first + COMPOSITION_NODE_BYTES_V3;
        let (prefix, suffix) = graph.split_at_mut(second);
        prefix[first..first + COMPOSITION_NODE_BYTES_V3]
            .swap_with_slice(&mut suffix[..COMPOSITION_NODE_BYTES_V3]);
        let mut descriptor = output.descriptor().to_vec();
        descriptor[DescriptorLayoutV3::GRAPH_DIGEST..DescriptorLayoutV3::GRAPH_DIGEST + 32]
            .copy_from_slice(&hash(&graph).to_bytes());
        assert_eq!(
            validate_publication_candidates_v3(
                &fixture.basis,
                &descriptor,
                &graph,
                output.translation(),
                output.exposure(),
            )
            .err(),
            Some(Error::RepresentationComposition(
                dclutch_claims::composition::Error::NonCanonical
            ))
        );
    }
}
