//! Canonical per-Market Structured publication closure.
//!
//! This is the host producer for the immutable records that must exist before
//! the selected V6 `ActivateReceipt` route can be assembled.  It deliberately
//! uses the composition, fractional-terms, Structured-terms, and descriptor
//! semantic owners; no descriptor preimage is hand-written here.

use dclutch_custody::token_svm::{TOKEN_2022_PROGRAM_ID, TokenBehaviorSelectionV2};
use dclutch_product::admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_product::payoff::registry_v3::{
    GRADED_BASIS_RECORD_SCHEMA_ID_V3, PRICE_GATE_RECORD_SCHEMA_ID_V1,
};
use dclutch_registry::record::{ContentDigest, RecordKeyV1, RecordPdaSeedsV1, SchemaReleaseId};
use sha2::{Digest as _, Sha256};

use dclutch_claims::{
    composition::{
        COMPOSITION_DESCRIPTOR_BYTES_V3, CanonicalTranslationInputV3, CompositionDescriptorInputV3,
        CompositionEdgeInputV3, CompositionExposureInputV3, CompositionExposureRowInputV3,
        CompositionExposureTermV3, CompositionGraphInputV3, CompositionNodeInputV3,
        CompositionNodeKindV3, RecordAdmissionV3, SparseTermV3, composition_exposure_bytes_v3,
        composition_graph_bytes_v3, composition_translation_bytes_v3, decode_composition_bundle_v3,
        encode_canonical_translation_v3_atomic, encode_composition_descriptor_v3_atomic,
        encode_composition_exposure_v3_atomic, encode_composition_graph_v3_atomic,
    },
    fractional_kernel::{
        FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2, FractionalExposureTermsAdmissionV2,
        FractionalExposureTermsInputV2, FractionalExposureTermsV2,
        encode_fractional_exposure_terms_v2, fractional_exposure_terms_bytes_v2,
    },
    rational::RationalReceiptMintSeedsV2,
    structured_kernel::{
        STRUCTURED_TERMS_SCHEMA_ID_V2, StructuredTermsAdmissionV2, StructuredTermsInputV2,
        encode_structured_terms_v2, structured_terms_bytes_v2,
    },
};
use dclutch_operator::{
    representation_composition::{
        PublicationTargetV3,
        native_categorical_v1::{
            NativeBasisCompositionInputV1, compile_native_basis_composition_v1,
        },
    },
    structured::{
        StructuredRepresentationDescriptorV2, derive_structured_representation_descriptor_v2,
    },
};
use solana_program::hash::{hash, hashv};
use solana_sdk::{pubkey::Pubkey, signature::Keypair};

use crate::{
    Error, Result,
    campaign::CampaignTerminalEvidenceV1,
    model::TransactionEvidence,
    plan::{hex32, pubkey},
    rpc::Rpc,
    runtime::{PublishedRecord, publish_record},
};

const GRAPH_DOMAIN_V1: &[u8] = b"dclutch:structured-claims-graph:v1";
const ROOT_DOMAIN_V1: &[u8] = b"dclutch:structured-claims-root:v1";
const TRANSLATION_DOMAIN_V1: &[u8] = b"dclutch:structured-claims-translation:v1";
const LEAF_DOMAIN_V1: &[u8] = b"dclutch:structured-claims-leaf:v1";
const PLACEHOLDER_SHARD_DOMAIN_V1: &[u8] = b"dclutch:structured-claims-placeholder-shard:v1";

/// Exact founding bodies and release facts a Structured publication binds.
///
/// The four Product bodies must have been authenticated as finalized Registry
/// records by the caller.  This compiler deliberately derives every identity
/// it needs from those bodies; it never accepts a caller-supplied digest for a
/// Product or basis fact.
#[derive(Clone, Debug)]
pub(crate) struct StructuredPublicationInputV1 {
    pub(crate) market: Pubkey,
    pub(crate) release_set: [u8; 32],
    pub(crate) claims_program: Pubkey,
    pub(crate) product_record_body: Vec<u8>,
    pub(crate) result_domain_body: Vec<u8>,
    pub(crate) portfolio_body: Vec<u8>,
    pub(crate) product_basis_body: Vec<u8>,
    pub(crate) price_gate_body: Option<Vec<u8>>,
    /// The selected token-behavior record body for this release.
    pub(crate) token_behavior_body: Vec<u8>,
    /// Product coordinates selected in strictly increasing canonical order.
    pub(crate) product_coordinates: Vec<u32>,
    /// Shard atoms backing one whole native claim.
    pub(crate) denominator: u64,
    /// Canonical Portfolio denominator used by the composition root.  This is
    /// distinct from the Structured shard denominator: the latter may scale a
    /// whole-number Portfolio up to the minimum fractional shard unit.
    pub(crate) composition_denominator: u64,
    /// Canonical nonzero Portfolio numerators in the same sparse order.
    pub(crate) composition_coefficients: Vec<u64>,
    /// Receipt recipe coefficients in the same order as `product_coordinates`.
    pub(crate) coefficients: Vec<u64>,
}

/// Complete immutable closure and its derived execution descriptor.
#[derive(Clone, Debug)]
pub(crate) struct StructuredPublicationClosureV1 {
    pub(crate) composition_descriptor: Vec<u8>,
    pub(crate) graph: Vec<u8>,
    pub(crate) translation: Vec<u8>,
    pub(crate) exposure: Vec<u8>,
    pub(crate) shard_terms: Vec<u8>,
    pub(crate) structured_terms: Vec<u8>,
    pub(crate) representation_descriptor: StructuredRepresentationDescriptorV2,
}

impl StructuredPublicationClosureV1 {
    /// Canonical Registry records in dependency order.
    pub(crate) fn publication_targets(&self) -> Vec<PublicationTargetV3<'_>> {
        vec![
            PublicationTargetV3 {
                schema_id: dclutch_claims::composition::COMPOSITION_DESCRIPTOR_SCHEMA_ID_V3,
                bytes: &self.composition_descriptor,
            },
            PublicationTargetV3 {
                schema_id: dclutch_claims::composition::COMPOSITION_GRAPH_SCHEMA_ID_V3,
                bytes: &self.graph,
            },
            PublicationTargetV3 {
                schema_id: dclutch_claims::composition::COMPOSITION_TRANSLATION_SCHEMA_ID_V3,
                bytes: &self.translation,
            },
            PublicationTargetV3 {
                schema_id: dclutch_claims::composition::COMPOSITION_EXPOSURE_SCHEMA_ID_V3,
                bytes: &self.exposure,
            },
            PublicationTargetV3 {
                schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
                bytes: &self.shard_terms,
            },
            PublicationTargetV3 {
                schema_id: STRUCTURED_TERMS_SCHEMA_ID_V2,
                bytes: &self.structured_terms,
            },
            PublicationTargetV3 {
                schema_id:
                    dclutch_claims::rational_kernel::REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
                bytes: &self.representation_descriptor.preimage,
            },
        ]
    }
}

/// Join same-finalized founding and selected-release observations into the
/// only compiler input accepted by this host lane.
///
/// The caller supplies the explicit economic recipe (selected Product
/// coordinates, denominator, and coefficients); all content identities and
/// token-behavior bytes come from finalized authenticated records.  A slot
/// mismatch refuses instead of composing a route from two chain moments.
pub(crate) fn structured_publication_input_from_finalized_v1(
    market: Pubkey,
    claims_program: Pubkey,
    artifacts: &crate::structured_activation::StructuredActivateReceiptArtifactsV1,
    founding: StructuredFoundingBodiesV1,
    product_coordinates: Vec<u32>,
    denominator: u64,
    composition_denominator: u64,
    composition_coefficients: Vec<u64>,
    coefficients: Vec<u64>,
) -> Result<StructuredPublicationInputV1> {
    if artifacts.slot != founding.slot {
        return Err(Error::new(
            "Structured publication selected release and founding facts were not finalized at one slot",
        ));
    }
    Ok(StructuredPublicationInputV1 {
        market,
        release_set: artifacts.token_behavior_selection.release_set(),
        claims_program,
        product_record_body: founding.product,
        result_domain_body: founding.result_domain,
        portfolio_body: founding.portfolio,
        product_basis_body: founding.product_basis,
        price_gate_body: founding.price_gate,
        token_behavior_body: artifacts.config.body.clone(),
        product_coordinates,
        denominator,
        composition_denominator,
        composition_coefficients,
        coefficients,
    })
}

/// Compile one complete Structured closure from exact, authenticated founding facts.
pub(crate) fn compile_structured_publication_closure_v1(
    input: &StructuredPublicationInputV1,
) -> Result<StructuredPublicationClosureV1> {
    let width = u32::try_from(input.coefficients.len())
        .map_err(|_| Error::new("Structured coefficient width overflow"))?;
    if input.market == Pubkey::default() || input.release_set == [0; 32] {
        return Err(Error::new(
            "Structured publication requires a nonzero Market and release set",
        ));
    }
    if input.claims_program == Pubkey::default() {
        return Err(Error::new(
            "Structured publication requires a Claims program",
        ));
    }
    if !(1..=3).contains(&width)
        || input.product_coordinates.len() != input.coefficients.len()
        || input.composition_coefficients.len() != input.coefficients.len()
        || input.denominator <= 1
        || input.coefficients.iter().any(|value| *value == 0)
    {
        return Err(Error::new(
            "Structured publication requires one through three nonzero coefficients, equally many Product coordinates, and denominator above one",
        ));
    }
    if input.token_behavior_body.is_empty() {
        return Err(Error::new(
            "Structured publication omitted the finalized token-behavior body",
        ));
    }
    let token_behavior_selection = TokenBehaviorSelectionV2::decode(&input.token_behavior_body)
        .map_err(|error| Error::new(format!("Structured token behavior: {error:?}")))?;
    if token_behavior_selection.release_set() != input.release_set
        || token_behavior_selection.token_program() != TOKEN_2022_PROGRAM_ID
    {
        return Err(Error::new(
            "Structured publication token behavior differs from its release or Token-2022",
        ));
    }
    let native = compile_native_basis_composition_v1(NativeBasisCompositionInputV1 {
        market: input.market.to_bytes(),
        release_set: input.release_set,
        product_record_bytes: &input.product_record_body,
        result_domain_bytes: &input.result_domain_body,
        portfolio_bytes: &input.portfolio_body,
        product_basis_bytes: &input.product_basis_body,
        price_gate_bytes: input.price_gate_body.as_deref(),
    })
    .map_err(|error| Error::new(format!("Structured founding facts: {error:?}")))?;
    let product_width = native.width();
    let mut previous = None;
    for coordinate in &input.product_coordinates {
        if *coordinate >= product_width || previous.is_some_and(|prior| prior >= *coordinate) {
            return Err(Error::new(
                "Structured publication Product coordinates must be in-range and strictly increasing",
            ));
        }
        previous = Some(*coordinate);
    }
    let market = input.market.to_bytes();
    let product_record = hash(&input.product_record_body).to_bytes();
    let result_domain = hash(&input.result_domain_body).to_bytes();
    let product_basis = hash(&input.product_basis_body).to_bytes();
    let representation_basis = native.representation_basis();
    let token_behavior = hash(&input.token_behavior_body).to_bytes();
    let denominator = input.composition_denominator.to_le_bytes();
    let coordinates = coordinates_bytes(&input.product_coordinates);
    let coefficients = coefficients_bytes(&input.composition_coefficients);
    let graph_id = hashv(&[
        GRAPH_DOMAIN_V1,
        &market,
        &input.release_set,
        &product_basis,
        &representation_basis,
        &denominator,
        &coordinates,
        &coefficients,
    ])
    .to_bytes();
    let root_id = hashv(&[ROOT_DOMAIN_V1, &graph_id]).to_bytes();
    let translation_id = hashv(&[TRANSLATION_DOMAIN_V1, &graph_id, &root_id]).to_bytes();
    let leaves = (0..width)
        .map(|index| hashv(&[LEAF_DOMAIN_V1, &graph_id, &index.to_le_bytes()]).to_bytes())
        .collect::<Vec<_>>();
    let mut nodes = Vec::with_capacity(leaves.len().saturating_add(1));
    for (index, id) in leaves.iter().enumerate() {
        let coordinate =
            u32::try_from(index).map_err(|_| Error::new("Structured leaf coordinate overflow"))?;
        nodes.push(CompositionNodeInputV3 {
            id: *id,
            rank: 0,
            first_edge: 0,
            edge_count: 0,
            first_term: coordinate,
            term_count: 1,
            kind: CompositionNodeKindV3::Native,
            native_outcome: coordinate,
            recipe_divisor: 1,
            flattened_denominator: 1,
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
        recipe_divisor: input.composition_denominator,
        flattened_denominator: input.composition_denominator,
    });
    let mut edges = Vec::with_capacity(leaves.len());
    for (index, id) in leaves.iter().enumerate() {
        let coordinate =
            u32::try_from(index).map_err(|_| Error::new("Structured edge coordinate overflow"))?;
        let coefficient = *input.coefficients.get(index).ok_or_else(|| {
            Error::new("Structured coefficient width changed while compiling the graph")
        })?;
        edges.push(CompositionEdgeInputV3 {
            child_id: *id,
            child_index: coordinate,
            coefficient,
        });
    }
    let mut terms = (0..width)
        .map(|index| SparseTermV3 {
            outcome: index,
            numerator: 1,
        })
        .collect::<Vec<_>>();
    terms.extend(
        input
            .composition_coefficients
            .iter()
            .enumerate()
            .map(|(index, numerator)| SparseTermV3 {
                outcome: u32::try_from(index).unwrap_or(u32::MAX),
                numerator: *numerator,
            }),
    );
    let graph_len = composition_graph_bytes_v3(
        u32::try_from(nodes.len())
            .map_err(|_| Error::new("Structured graph node width overflow"))?,
        width,
        u32::try_from(terms.len())
            .map_err(|_| Error::new("Structured graph term width overflow"))?,
    )
    .map_err(|error| Error::new(format!("Structured graph width: {error:?}")))?;
    let mut graph_scratch = vec![0; graph_len];
    let mut graph = vec![0; graph_len];
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
    .map_err(|error| Error::new(format!("Structured graph: {error:?}")))?;
    let trans_len = composition_translation_bytes_v3(width)
        .map_err(|error| Error::new(format!("Structured translation width: {error:?}")))?;
    let mut trans_scratch = vec![0; trans_len];
    let mut translation = vec![0; trans_len];
    encode_canonical_translation_v3_atomic(
        CanonicalTranslationInputV3 {
            graph_id,
            root_id,
            outcome_count: width,
            denominator: input.composition_denominator,
            terms: &terms[usize::try_from(width)
                .map_err(|_| Error::new("Structured width conversion failed"))?..],
        },
        &mut trans_scratch,
        &mut translation,
    )
    .map_err(|error| Error::new(format!("Structured translation: {error:?}")))?;
    let mut descriptor_scratch = [0; COMPOSITION_DESCRIPTOR_BYTES_V3];
    let mut composition_descriptor = [0; COMPOSITION_DESCRIPTOR_BYTES_V3];
    encode_composition_descriptor_v3_atomic(
        CompositionDescriptorInputV3 {
            market,
            result_domain: result_domain,
            release_set: input.release_set,
            native_basis: representation_basis,
            graph_id,
            graph_digest: hash(&graph).to_bytes(),
            root_id,
            translation_id,
            translation_digest: hash(&translation).to_bytes(),
            outcome_count: width,
            node_count: u32::try_from(nodes.len())
                .map_err(|_| Error::new("Structured graph node count overflow"))?,
            edge_count: width,
            term_count: u32::try_from(terms.len())
                .map_err(|_| Error::new("Structured graph term count overflow"))?,
            root_denominator: input.composition_denominator,
        },
        &mut descriptor_scratch,
        &mut composition_descriptor,
    )
    .map_err(|error| Error::new(format!("Structured composition descriptor: {error:?}")))?;
    let row_terms = input
        .product_coordinates
        .iter()
        .map(|product_coordinate| {
            [CompositionExposureTermV3 {
                product_coordinate: *product_coordinate,
                numerator: 1,
            }]
        })
        .collect::<Vec<_>>();
    let rows = leaves
        .iter()
        .enumerate()
        .map(|(index, id)| CompositionExposureRowInputV3 {
            node_id: *id,
            denominator: 1,
            terms: row_terms[index].as_slice(),
        })
        .collect::<Vec<_>>();
    let exposure_len = composition_exposure_bytes_v3(width, width)
        .map_err(|error| Error::new(format!("Structured exposure width: {error:?}")))?;
    let mut exposure_scratch = vec![0; exposure_len];
    let mut exposure = vec![0; exposure_len];
    encode_composition_exposure_v3_atomic(
        CompositionExposureInputV3 {
            market,
            result_domain: result_domain,
            release_set: input.release_set,
            product_basis: product_basis,
            representation_basis: representation_basis,
            graph_id,
            product_width: product_width,
            rows: &rows,
        },
        &mut exposure_scratch,
        &mut exposure,
    )
    .map_err(|error| Error::new(format!("Structured exposure: {error:?}")))?;
    let exposure_id = hash(&exposure).to_bytes();
    let receipt = Pubkey::find_program_address(
        &RationalReceiptMintSeedsV2::new(exposure_id, market, input.release_set)
            .map_err(|error| Error::new(format!("Structured receipt seeds: {error:?}")))?
            .as_slices(),
        &input.claims_program,
    )
    .0
    .to_bytes();
    let placeholders = (0..width)
        .map(|index| {
            hashv(&[
                PLACEHOLDER_SHARD_DOMAIN_V1,
                &exposure_id,
                &index.to_le_bytes(),
            ])
            .to_bytes()
        })
        .collect::<Vec<_>>();
    let shard_len = fractional_exposure_terms_bytes_v2(placeholders.len())
        .map_err(|error| Error::new(format!("Structured shard terms width: {error:?}")))?;
    let mut shard_scratch = vec![0; shard_len];
    let mut shard_terms = vec![0; shard_len];
    encode_fractional_exposure_terms_v2(
        FractionalExposureTermsInputV2 {
            market,
            product_record: product_record,
            result_domain: result_domain,
            release_set: input.release_set,
            token_program: TOKEN_2022_PROGRAM_ID,
            token_behavior: token_behavior,
            exposure_id,
            product_basis: product_basis,
            representation_basis: representation_basis,
            graph_id,
            product_width: product_width,
            denominator: input.denominator,
            shard_mints: &placeholders,
        },
        &mut shard_scratch,
        &mut shard_terms,
    )
    .map_err(|error| Error::new(format!("Structured shard terms: {error:?}")))?;
    let terms_len = structured_terms_bytes_v2(placeholders.len())
        .map_err(|error| Error::new(format!("Structured terms width: {error:?}")))?;
    let mut terms_scratch = vec![0; terms_len];
    let mut structured_terms = vec![0; terms_len];
    encode_structured_terms_v2(
        StructuredTermsInputV2 {
            market,
            product_record: product_record,
            result_domain: result_domain,
            release_set: input.release_set,
            token_program: TOKEN_2022_PROGRAM_ID,
            token_behavior: token_behavior,
            shard_terms: hash(&shard_terms).to_bytes(),
            shard_exposure: exposure_id,
            receipt_mint: receipt,
            graph_id,
            denominator: input.denominator,
            coefficients: &input.coefficients,
        },
        &mut terms_scratch,
        &mut structured_terms,
    )
    .map_err(|error| Error::new(format!("Structured terms: {error:?}")))?;
    let admission = |id| RecordAdmissionV3 {
        selected_id: id,
        finalized_id: id,
        recomputed_digest: id,
        finalized_digest: id,
        record_authenticated: true,
    };
    let bundle = decode_composition_bundle_v3(
        &composition_descriptor,
        admission(hash(&composition_descriptor).to_bytes()),
        &graph,
        admission(hash(&graph).to_bytes()),
        &translation,
        admission(hash(&translation).to_bytes()),
    )
    .map_err(|error| Error::new(format!("Structured composition admission: {error:?}")))?;
    let shard = FractionalExposureTermsV2::decode(
        &shard_terms,
        FractionalExposureTermsAdmissionV2 {
            selected_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            finalized_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            selected_terms_id: hash(&shard_terms).to_bytes(),
            finalized_terms_id: hash(&shard_terms).to_bytes(),
            recomputed_terms_digest: hash(&shard_terms).to_bytes(),
            finalized_terms_digest: hash(&shard_terms).to_bytes(),
            record_authenticated: true,
        },
    )
    .map_err(|error| Error::new(format!("Structured shard terms admission: {error:?}")))?;
    let terms = dclutch_claims::structured_kernel::StructuredTermsV2::decode(
        &structured_terms,
        StructuredTermsAdmissionV2 {
            selected_schema_id: STRUCTURED_TERMS_SCHEMA_ID_V2,
            finalized_schema_id: STRUCTURED_TERMS_SCHEMA_ID_V2,
            selected_terms_id: hash(&structured_terms).to_bytes(),
            finalized_terms_id: hash(&structured_terms).to_bytes(),
            recomputed_terms_digest: hash(&structured_terms).to_bytes(),
            finalized_terms_digest: hash(&structured_terms).to_bytes(),
            record_authenticated: true,
        },
        shard,
    )
    .map_err(|error| Error::new(format!("Structured terms admission: {error:?}")))?;
    let exposure_bundle = dclutch_claims::composition::CompositionExposureBundleV3::decode(
        &exposure,
        admission(exposure_id),
    )
    .map_err(|error| Error::new(format!("Structured exposure admission: {error:?}")))?;
    let representation_descriptor =
        derive_structured_representation_descriptor_v2(terms, bundle, exposure_bundle)
            .map_err(|error| Error::new(format!("Structured descriptor derivation: {error:?}")))?;
    Ok(StructuredPublicationClosureV1 {
        composition_descriptor: composition_descriptor.to_vec(),
        graph,
        translation,
        exposure,
        shard_terms,
        structured_terms,
        representation_descriptor,
    })
}

/// Finalized Product records consumed by the per-Market Structured compiler.
///
/// These bodies originated at content-addressed Registry coordinates recorded
/// in the sealed campaign report.  They are private to this module so no
/// caller can construct a trusted input without passing through
/// [`hydrate_structured_founding_bodies_v1`].
#[derive(Clone, Debug)]
pub(crate) struct StructuredFoundingBodiesV1 {
    pub(crate) slot: u64,
    pub(crate) product: Vec<u8>,
    pub(crate) result_domain: Vec<u8>,
    pub(crate) portfolio: Vec<u8>,
    pub(crate) product_basis: Vec<u8>,
    pub(crate) price_gate: Option<Vec<u8>>,
}

#[derive(Clone, Debug)]
struct FoundingRecordExpectationV1 {
    label: &'static str,
    schema: [u8; 32],
    digest: [u8; 32],
    raw: Pubkey,
    staging: Pubkey,
}

fn founding_record_expectations_v1(
    registry: Pubkey,
    evidence: &CampaignTerminalEvidenceV1,
) -> Result<Vec<FoundingRecordExpectationV1>> {
    let requested = [
        ("product_record", PRODUCT_RECORD_SCHEMA_ID_V2, false),
        ("result_domain_record", RESULT_DOMAIN_SCHEMA_ID_V2, false),
        ("portfolio_record", PORTFOLIO_SCHEMA_ID_V2, false),
        (
            "linked_liability_basis_record",
            GRADED_BASIS_RECORD_SCHEMA_ID_V3,
            false,
        ),
        ("price_gate_record", PRICE_GATE_RECORD_SCHEMA_ID_V1, true),
    ];
    requested
        .into_iter()
        .filter_map(|(label, schema, optional)| match evidence.accounts.get(label) {
            Some(row) => Some(Ok((label, schema, row))),
            None if optional => None,
            None => Some(Err(Error::new(format!(
                "Structured publication report omitted {label}"
            )))),
        })
        .map(|row| {
            let (label, schema, report) = row?;
            let digest = hex32(&report.data_sha256).map_err(|error| {
                Error::new(format!("Structured publication report {label} digest: {error}"))
            })?;
            let (raw, staging) = record_coordinates_v1(registry, schema, digest)?;
            if pubkey(&report.address)? != raw {
                return Err(Error::new(format!(
                    "Structured publication report {label} address differs from its schema and digest"
                )));
            }
            Ok(FoundingRecordExpectationV1 {
                label,
                schema,
                digest,
                raw,
                staging,
            })
        })
        .collect()
}

fn founding_bodies_from_snapshot_v1(
    registry: Pubkey,
    expectations: &[FoundingRecordExpectationV1],
    live: &[dclutch_operator::ObservedAccount],
) -> Result<StructuredFoundingBodiesV1> {
    if live.len() != expectations.len() * 2 {
        return Err(Error::new(
            "Structured publication founding snapshot changed record width",
        ));
    }
    let mut bodies = std::collections::BTreeMap::<&str, Vec<u8>>::new();
    for (index, expected) in expectations.iter().enumerate() {
        let raw = live.get(index * 2).ok_or_else(|| {
            Error::new("Structured publication founding snapshot omitted a raw record")
        })?;
        let staging = live.get(index * 2 + 1).ok_or_else(|| {
            Error::new("Structured publication founding snapshot omitted a staging cursor")
        })?;
        if raw.key != expected.raw
            || staging.key != expected.staging
            || hash(&raw.data).to_bytes() != expected.digest
        {
            return Err(Error::new(format!(
                "Structured publication finalized {} differs from its campaign report",
                expected.label
            )));
        }
        dclutch_operator::observation::authenticate_finalized_record(
            registry,
            raw,
            &dclutch_operator::observation::FinalizedRecordProof {
                schema_release_id: expected.schema,
                staging_cursor: staging.clone(),
            },
        )
        .map_err(|error| {
            Error::new(format!(
                "Structured publication finalized {} record refused: {error:?}",
                expected.label
            ))
        })?;
        bodies.insert(expected.label, raw.data.clone());
    }
    let mut required = |label| {
        bodies.remove(label).ok_or_else(|| {
            Error::new(format!(
                "Structured publication lost {label} after authentication"
            ))
        })
    };
    Ok(StructuredFoundingBodiesV1 {
        slot: 0,
        product: required("product_record")?,
        result_domain: required("result_domain_record")?,
        portfolio: required("portfolio_record")?,
        product_basis: required("linked_liability_basis_record")?,
        price_gate: bodies.remove("price_gate_record"),
    })
}

/// Acquire selected-release and Product founding records in one finalized
/// `getMultipleAccounts` snapshot, then produce a compiler input.  This is the
/// only path that binds both sides without gambling that two RPC calls land at
/// the same slot.
pub(crate) fn hydrate_structured_publication_input_same_slot_v1(
    rpc: &mut Rpc,
    registry: Pubkey,
    market_input_bytes: &[u8],
    evidence: &CampaignTerminalEvidenceV1,
    minimum_slot: u64,
    market: Pubkey,
    claims_program: Pubkey,
    product_coordinates: Vec<u32>,
    denominator: u64,
    composition_denominator: u64,
    composition_coefficients: Vec<u64>,
    coefficients: Vec<u64>,
) -> Result<StructuredPublicationInputV1> {
    let selected = crate::structured_activation::selected_activate_receipt_record_expectations_v1(
        market_input_bytes,
    )?;
    let founding = founding_record_expectations_v1(registry, evidence)?;
    let mut addresses = selected
        .iter()
        .map(|record| {
            crate::structured_activation::record_coordinates_v1(
                registry,
                record.schema,
                &record.body,
            )
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flat_map(|(raw, staging)| [raw, staging])
        .collect::<Vec<_>>();
    addresses.extend(
        founding
            .iter()
            .flat_map(|record| [record.raw, record.staging]),
    );
    let (observation, live) =
        rpc.finalized_observed_accounts_admitting_vacant(&addresses, minimum_slot)?;
    let selected_width = selected.len() * 2;
    let artifacts =
        crate::structured_activation::hydrate_selected_activate_receipt_artifacts_from_snapshot_v1(
            registry,
            &selected,
            observation.slot,
            live.get(..selected_width).ok_or_else(|| {
                Error::new("Structured publication snapshot omitted selected records")
            })?,
        )?;
    let mut founding_bodies = founding_bodies_from_snapshot_v1(
        registry,
        &founding,
        live.get(selected_width..).ok_or_else(|| {
            Error::new("Structured publication snapshot omitted founding records")
        })?,
    )?;
    founding_bodies.slot = observation.slot;
    structured_publication_input_from_finalized_v1(
        market,
        claims_program,
        &artifacts,
        founding_bodies,
        product_coordinates,
        denominator,
        composition_denominator,
        composition_coefficients,
        coefficients,
    )
}

/// Re-acquire every Product-side fact used by Structured lowering from the
/// report-named Registry records and authenticate it at one finalized slot.
///
/// The report's address and byte digest are checked before the Registry
/// authenticator sees a body.  The authenticator then proves the raw PDA,
/// schema, owner, rent, and paired vacant cursor.  A stale report, alternate
/// raw record, or same-schema substituted body cannot become compiler input.
pub(crate) fn hydrate_structured_founding_bodies_v1(
    rpc: &mut Rpc,
    registry: Pubkey,
    evidence: &CampaignTerminalEvidenceV1,
    minimum_slot: u64,
) -> Result<StructuredFoundingBodiesV1> {
    let requested = [
        ("product_record", PRODUCT_RECORD_SCHEMA_ID_V2, false),
        ("result_domain_record", RESULT_DOMAIN_SCHEMA_ID_V2, false),
        ("portfolio_record", PORTFOLIO_SCHEMA_ID_V2, false),
        (
            "linked_liability_basis_record",
            GRADED_BASIS_RECORD_SCHEMA_ID_V3,
            false,
        ),
        ("price_gate_record", PRICE_GATE_RECORD_SCHEMA_ID_V1, true),
    ];
    let mut supplied = Vec::new();
    for (label, schema, optional) in requested {
        match evidence.accounts.get(label) {
            Some(row) => supplied.push((label, schema, row)),
            None if optional => {}
            None => {
                return Err(Error::new(format!(
                    "Structured publication report omitted {label}"
                )));
            }
        }
    }
    let coordinates = supplied
        .iter()
        .map(|(label, schema, row)| {
            let digest = hex32(&row.data_sha256).map_err(|error| {
                Error::new(format!("Structured publication report {label} digest: {error}"))
            })?;
            let (raw, staging) = record_coordinates_v1(registry, *schema, digest)?;
            if pubkey(&row.address)? != raw {
                return Err(Error::new(format!(
                    "Structured publication report {label} address differs from its schema and digest"
                )));
            }
            Ok((raw, staging))
        })
        .collect::<Result<Vec<_>>>()?;
    let addresses = coordinates
        .iter()
        .flat_map(|(raw, staging)| [*raw, *staging])
        .collect::<Vec<_>>();
    let (observation, live) =
        rpc.finalized_observed_accounts_admitting_vacant(&addresses, minimum_slot)?;
    if live.len() != addresses.len() {
        return Err(Error::new(
            "Structured publication founding snapshot changed record width",
        ));
    }
    let mut bodies = std::collections::BTreeMap::<&str, Vec<u8>>::new();
    for (index, (label, schema, report)) in supplied.iter().enumerate() {
        let raw = live.get(index * 2).ok_or_else(|| {
            Error::new("Structured publication founding snapshot omitted a raw record")
        })?;
        let (expected_raw, expected_staging) = coordinates[index];
        if raw.key != expected_raw || sha256_hex_v1(&raw.data) != report.data_sha256 {
            return Err(Error::new(format!(
                "Structured publication finalized {label} differs from its campaign report"
            )));
        }
        let staging = live.get(index * 2 + 1).ok_or_else(|| {
            Error::new("Structured publication founding snapshot omitted a staging cursor")
        })?;
        if staging.key != expected_staging {
            return Err(Error::new(format!(
                "Structured publication finalized {label} staging coordinate differs from its content"
            )));
        }
        dclutch_operator::observation::authenticate_finalized_record(
            registry,
            raw,
            &dclutch_operator::observation::FinalizedRecordProof {
                schema_release_id: *schema,
                staging_cursor: staging.clone(),
            },
        )
        .map_err(|error| {
            Error::new(format!(
                "Structured publication finalized {label} record refused: {error:?}"
            ))
        })?;
        bodies.insert(*label, raw.data.clone());
    }
    let mut required = |label| {
        bodies.remove(label).ok_or_else(|| {
            Error::new(format!(
                "Structured publication lost {label} after authentication"
            ))
        })
    };
    Ok(StructuredFoundingBodiesV1 {
        slot: observation.slot,
        product: required("product_record")?,
        result_domain: required("result_domain_record")?,
        portfolio: required("portfolio_record")?,
        product_basis: required("linked_liability_basis_record")?,
        price_gate: bodies.remove("price_gate_record"),
    })
}

pub(crate) fn record_coordinates_v1(
    registry: Pubkey,
    schema: [u8; 32],
    digest: [u8; 32],
) -> Result<(Pubkey, Pubkey)> {
    let key = RecordKeyV1::new(
        SchemaReleaseId::new(schema)
            .map_err(|error| Error::new(format!("Structured publication schema: {error:?}")))?,
        ContentDigest::new(digest)
            .map_err(|error| Error::new(format!("Structured publication digest: {error:?}")))?,
    );
    let address = |seeds: RecordPdaSeedsV1| {
        Pubkey::find_program_address(
            &[
                seeds.domain(),
                seeds.schema_release_id().as_bytes(),
                seeds.expected_digest().as_bytes(),
            ],
            &registry,
        )
        .0
    };
    Ok((
        address(key.raw_record_pda_seeds()),
        address(key.staging_cursor_pda_seeds()),
    ))
}

fn sha256_hex_v1(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// One published, same-finalized-slot Structured record closure.
///
/// A publication helper verifies every ladder separately; this extra snapshot
/// is deliberately one observation across the full seven-record closure so an
/// activation builder never joins records that finalised at different slots.
#[derive(Clone, Debug)]
pub(crate) struct PublishedStructuredClosureV1 {
    pub(crate) slot: u64,
    pub(crate) records: [PublishedRecord; 7],
}

/// Publish the complete per-Market Structured closure, then re-acquire and
/// authenticate every raw record and its vacant staging cursor at one finalized
/// slot.  `closure` is expected to come from
/// [`compile_structured_publication_closure_v1`]; this function checks that
/// expectation again by binding each returned coordinate and raw bytes to the
/// target that caused it.
pub(crate) fn publish_structured_publication_closure_v1(
    rpc: &mut Rpc,
    registry: Pubkey,
    payer: &Keypair,
    closure: &StructuredPublicationClosureV1,
    minimum_slot: u64,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<PublishedStructuredClosureV1> {
    let targets = closure.publication_targets();
    let mut published = Vec::with_capacity(targets.len());
    for target in &targets {
        let record = publish_record(
            rpc,
            registry,
            payer,
            target.schema_id,
            target.bytes,
            None,
            transactions,
        )?;
        if record.schema != target.schema_id || record.digest != hash(target.bytes).to_bytes() {
            return Err(Error::new(
                "Structured publication returned a coordinate outside its exact target",
            ));
        }
        published.push(record);
    }
    let records: [PublishedRecord; 7] = published.try_into().map_err(|_| {
        Error::new("Structured publication target count differs from its seven-record closure")
    })?;
    let publication_slot = transactions
        .last()
        .map(|transaction| transaction.slot)
        .unwrap_or(minimum_slot)
        .max(minimum_slot);
    let addresses = records
        .iter()
        .flat_map(|record| [record.raw, record.staging])
        .collect::<Vec<_>>();
    let (observation, accounts) =
        rpc.finalized_observed_accounts_admitting_vacant(&addresses, publication_slot)?;
    if accounts.len() != addresses.len() {
        return Err(Error::new(
            "Structured publication finalized snapshot changed record width",
        ));
    }
    for (index, target) in targets.iter().enumerate() {
        let raw = accounts.get(index * 2).ok_or_else(|| {
            Error::new("Structured publication finalized snapshot omitted a raw record")
        })?;
        let staging = accounts.get(index * 2 + 1).ok_or_else(|| {
            Error::new("Structured publication finalized snapshot omitted a staging cursor")
        })?;
        let record = records[index];
        if raw.key != record.raw || staging.key != record.staging || raw.data != target.bytes {
            return Err(Error::new(
                "Structured publication finalized record differs from its exact target",
            ));
        }
        dclutch_operator::observation::authenticate_finalized_record(
            registry,
            raw,
            &dclutch_operator::observation::FinalizedRecordProof {
                schema_release_id: target.schema_id,
                staging_cursor: staging.clone(),
            },
        )
        .map_err(|error| {
            Error::new(format!(
                "Structured publication finalized record authentication refused: {error:?}"
            ))
        })?;
    }
    Ok(PublishedStructuredClosureV1 {
        slot: observation.slot,
        records,
    })
}

fn coordinates_bytes(values: &[u32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn coefficients_bytes(values: &[u64]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_market_refuses_before_any_founding_body_is_accepted() {
        let input = StructuredPublicationInputV1 {
            market: Pubkey::default(),
            release_set: [1; 32],
            claims_program: Pubkey::new_from_array([2; 32]),
            product_record_body: Vec::new(),
            result_domain_body: Vec::new(),
            portfolio_body: Vec::new(),
            product_basis_body: Vec::new(),
            price_gate_body: None,
            token_behavior_body: vec![1],
            product_coordinates: vec![0],
            denominator: 2,
            composition_denominator: 1,
            composition_coefficients: vec![1],
            coefficients: vec![1],
        };
        let error = compile_structured_publication_closure_v1(&input)
            .expect_err("zero Market must refuse before any publication target exists");
        assert_eq!(
            error.to_string(),
            "Structured publication requires a nonzero Market and release set"
        );
    }
}
