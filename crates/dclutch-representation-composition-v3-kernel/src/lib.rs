#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Bounded acyclic representation composition over one native liability basis.
//!
//! Finalized descriptors own the sole Market, result-domain, release-set, and
//! native-basis identities. Graph nodes compose only nonnegative exact rational
//! quantities of earlier nodes. Every node commits a canonical sparse native
//! flattening which this kernel independently recomputes from its edges. The
//! root translation is byte-identical to the root node's sparse term table.
//!
//! This crate neither resolves a Product nor owns balances. Product resolution
//! remains Product/Core authority; Claims and Token adapters remain the sole
//! physical economic writers.

mod abi;
mod exposure;
mod graph;
mod translation;

pub use abi::{
    CAPACITY_PROFILE_ID_V3, CAPACITY_PROFILE_PREIMAGE_V3, COMPOSITION_DESCRIPTOR_BYTES_V3,
    COMPOSITION_DESCRIPTOR_MAGIC_V3, COMPOSITION_DESCRIPTOR_SCHEMA_ID_V3,
    COMPOSITION_DESCRIPTOR_SCHEMA_PREIMAGE_V3, COMPOSITION_GRAPH_HEADER_BYTES_V3,
    COMPOSITION_GRAPH_MAGIC_V3, COMPOSITION_GRAPH_SCHEMA_ID_V3,
    COMPOSITION_GRAPH_SCHEMA_PREIMAGE_V3, COMPOSITION_SCHEMA_VERSION_V3,
    COMPOSITION_TRANSLATION_HEADER_BYTES_V3, COMPOSITION_TRANSLATION_MAGIC_V3,
    COMPOSITION_TRANSLATION_SCHEMA_ID_V3, COMPOSITION_TRANSLATION_SCHEMA_PREIMAGE_V3,
    CompositionDescriptorInputV3, CompositionDescriptorV3, DescriptorLayoutV3, Error,
    MAX_COMPOSITION_EDGES_V3, MAX_COMPOSITION_NODES_V3, MAX_COMPOSITION_OUTCOMES_V3,
    MAX_COMPOSITION_TERMS_V3, MIN_COMPOSITION_OUTCOMES_V3, RecordAdmissionV3, Result,
    encode_composition_descriptor_v3_atomic,
};
pub use exposure::{
    COMPOSITION_EXPOSURE_CAPACITY_ID_V3, COMPOSITION_EXPOSURE_CAPACITY_PREIMAGE_V3,
    COMPOSITION_EXPOSURE_HEADER_BYTES_V3, COMPOSITION_EXPOSURE_MAGIC_V3,
    COMPOSITION_EXPOSURE_ROW_BYTES_V3, COMPOSITION_EXPOSURE_SCHEMA_ID_V3,
    COMPOSITION_EXPOSURE_SCHEMA_PREIMAGE_V3, COMPOSITION_EXPOSURE_TERM_BYTES_V3,
    COMPOSITION_EXPOSURE_VERSION_V3, CompositionExposureBundleV3, CompositionExposureExpectedV3,
    CompositionExposureInputV3, CompositionExposureLayoutV3, CompositionExposureRowInputV3,
    CompositionExposureRowLayoutV3, CompositionExposureRowV3, CompositionExposureTermLayoutV3,
    CompositionExposureTermV3, MAX_COMPOSITION_EXPOSURE_TERMS_V3, MAX_COMPOSITION_PRODUCT_WIDTH_V3,
    MAX_COMPOSITION_REPRESENTATION_WIDTH_V3, MIN_COMPOSITION_PRODUCT_WIDTH_V3,
    composition_exposure_bytes_v3, encode_composition_exposure_v3_atomic,
};
pub use graph::{
    COMPOSITION_EDGE_BYTES_V3, COMPOSITION_NODE_BYTES_V3, COMPOSITION_TERM_BYTES_V3,
    CompositionEdgeInputV3, CompositionGraphInputV3, CompositionGraphNodeV3, CompositionGraphV3,
    CompositionNodeInputV3, CompositionNodeKindV3, EdgeLayoutV3, GraphLayoutV3, NodeLayoutV3,
    SparseTermV3, TermLayoutV3, composition_graph_bytes_v3, encode_composition_graph_v3_atomic,
};
pub use translation::{
    CanonicalTranslationInputV3, CanonicalTranslationV3, CompositionBundleV3, TranslationLayoutV3,
    composition_translation_bytes_v3, decode_composition_bundle_v3,
    encode_canonical_translation_v3_atomic,
};
