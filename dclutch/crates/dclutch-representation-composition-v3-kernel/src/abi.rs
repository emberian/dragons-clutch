//! Descriptor ABI and shared hostile-decoding utilities.

use crate::generated_abi as gen_abi;

use core::convert::TryInto;

/// Implemented composition schema version.
pub const COMPOSITION_SCHEMA_VERSION_V3: u16 = gen_abi::COMPOSITION_SCHEMA_VERSION_LEAN_V3;
/// Minimum exhaustive Product-native result-domain width.
pub const MIN_COMPOSITION_OUTCOMES_V3: u32 = gen_abi::COMPOSITION_MIN_OUTCOMES_LEAN_V3;
/// Maximum Product-native width in this executable capacity profile.
pub const MAX_COMPOSITION_OUTCOMES_V3: u32 = gen_abi::COMPOSITION_MAX_OUTCOMES_LEAN_V3;
/// Maximum graph nodes in this executable capacity profile.
pub const MAX_COMPOSITION_NODES_V3: u32 = gen_abi::COMPOSITION_MAX_NODES_LEAN_V3;
/// Maximum graph edges in this executable capacity profile.
pub const MAX_COMPOSITION_EDGES_V3: u32 = gen_abi::COMPOSITION_MAX_EDGES_LEAN_V3;
/// Maximum sparse terms across every node in this executable capacity profile.
pub const MAX_COMPOSITION_TERMS_V3: u32 = gen_abi::COMPOSITION_MAX_TERMS_LEAN_V3;

/// Capacity-profile preimage. These maxima are executable bounds, not ontology.
pub const CAPACITY_PROFILE_PREIMAGE_V3: &[u8] =
    gen_abi::COMPOSITION_CAPACITY_PROFILE_PREIMAGE_LEAN_V3;
/// SHA-256 of [`CAPACITY_PROFILE_PREIMAGE_V3`].
pub const CAPACITY_PROFILE_ID_V3: [u8; 32] = gen_abi::COMPOSITION_CAPACITY_PROFILE_ID_LEAN_V3;

/// Descriptor schema preimage.
pub const COMPOSITION_DESCRIPTOR_SCHEMA_PREIMAGE_V3: &[u8] =
    gen_abi::COMPOSITION_DESCRIPTOR_SCHEMA_PREIMAGE_LEAN_V3;
/// SHA-256 of [`COMPOSITION_DESCRIPTOR_SCHEMA_PREIMAGE_V3`].
pub const COMPOSITION_DESCRIPTOR_SCHEMA_ID_V3: [u8; 32] =
    gen_abi::COMPOSITION_DESCRIPTOR_SCHEMA_ID_LEAN_V3;
/// Graph schema preimage.
pub const COMPOSITION_GRAPH_SCHEMA_PREIMAGE_V3: &[u8] =
    gen_abi::COMPOSITION_GRAPH_SCHEMA_PREIMAGE_LEAN_V3;
/// SHA-256 of [`COMPOSITION_GRAPH_SCHEMA_PREIMAGE_V3`].
pub const COMPOSITION_GRAPH_SCHEMA_ID_V3: [u8; 32] = gen_abi::COMPOSITION_GRAPH_SCHEMA_ID_LEAN_V3;
/// Translation schema preimage.
pub const COMPOSITION_TRANSLATION_SCHEMA_PREIMAGE_V3: &[u8] =
    gen_abi::COMPOSITION_TRANSLATION_SCHEMA_PREIMAGE_LEAN_V3;
/// SHA-256 of [`COMPOSITION_TRANSLATION_SCHEMA_PREIMAGE_V3`].
pub const COMPOSITION_TRANSLATION_SCHEMA_ID_V3: [u8; 32] =
    gen_abi::COMPOSITION_TRANSLATION_SCHEMA_ID_LEAN_V3;

/// Descriptor magic.
pub const COMPOSITION_DESCRIPTOR_MAGIC_V3: [u8; 8] = gen_abi::COMPOSITION_DESCRIPTOR_MAGIC_LEAN_V3;
/// Graph magic.
pub const COMPOSITION_GRAPH_MAGIC_V3: [u8; 8] = gen_abi::COMPOSITION_GRAPH_MAGIC_LEAN_V3;
/// Translation magic.
pub const COMPOSITION_TRANSLATION_MAGIC_V3: [u8; 8] =
    gen_abi::COMPOSITION_TRANSLATION_MAGIC_LEAN_V3;
/// Exact fixed descriptor width.
pub const COMPOSITION_DESCRIPTOR_BYTES_V3: usize = gen_abi::COMPOSITION_DESCRIPTOR_BYTES_LEAN_V3;
/// Exact fixed graph header before node, edge, and term tables.
pub const COMPOSITION_GRAPH_HEADER_BYTES_V3: usize =
    gen_abi::COMPOSITION_GRAPH_HEADER_BYTES_LEAN_V3;
/// Exact fixed translation header before canonical sparse terms.
pub const COMPOSITION_TRANSLATION_HEADER_BYTES_V3: usize =
    gen_abi::COMPOSITION_TRANSLATION_HEADER_BYTES_LEAN_V3;

/// Descriptor byte-layout authority.
pub struct DescriptorLayoutV3;

impl DescriptorLayoutV3 {
    /// Magic offset.
    pub const MAGIC: usize = gen_abi::COMPOSITION_DESCRIPTOR_MAGIC_OFFSET_V3;
    /// Schema-version offset.
    pub const VERSION: usize = gen_abi::COMPOSITION_DESCRIPTOR_VERSION_OFFSET_V3;
    /// Reserved header offset.
    pub const RESERVED_HEADER: usize = gen_abi::COMPOSITION_DESCRIPTOR_RESERVED_HEADER_OFFSET_V3;
    /// Immutable Core Market identity offset.
    pub const MARKET: usize = gen_abi::COMPOSITION_DESCRIPTOR_MARKET_OFFSET_V3;
    /// Exhaustive Product result-domain identity offset.
    pub const RESULT_DOMAIN: usize = gen_abi::COMPOSITION_DESCRIPTOR_RESULT_DOMAIN_OFFSET_V3;
    /// Immutable execution release-set identity offset.
    pub const RELEASE_SET: usize = gen_abi::COMPOSITION_DESCRIPTOR_RELEASE_SET_OFFSET_V3;
    /// Exhaustive native liability-basis identity offset.
    pub const NATIVE_BASIS: usize = gen_abi::COMPOSITION_DESCRIPTOR_NATIVE_BASIS_OFFSET_V3;
    /// Stable representation graph identity offset.
    pub const GRAPH_ID: usize = gen_abi::COMPOSITION_DESCRIPTOR_GRAPH_ID_OFFSET_V3;
    /// Exact finalized graph-content digest offset.
    pub const GRAPH_DIGEST: usize = gen_abi::COMPOSITION_DESCRIPTOR_GRAPH_DIGEST_OFFSET_V3;
    /// Sole canonical graph-root identity offset.
    pub const ROOT_ID: usize = gen_abi::COMPOSITION_DESCRIPTOR_ROOT_ID_OFFSET_V3;
    /// Stable canonical-translation identity offset.
    pub const TRANSLATION_ID: usize = gen_abi::COMPOSITION_DESCRIPTOR_TRANSLATION_ID_OFFSET_V3;
    /// Exact finalized translation-content digest offset.
    pub const TRANSLATION_DIGEST: usize =
        gen_abi::COMPOSITION_DESCRIPTOR_TRANSLATION_DIGEST_OFFSET_V3;
    /// Explicit executable-capacity profile identity offset.
    pub const CAPACITY_PROFILE: usize = gen_abi::COMPOSITION_DESCRIPTOR_CAPACITY_PROFILE_OFFSET_V3;
    /// Native result-domain width offset.
    pub const OUTCOME_COUNT: usize = gen_abi::COMPOSITION_DESCRIPTOR_OUTCOME_COUNT_OFFSET_V3;
    /// Exact node-count offset.
    pub const NODE_COUNT: usize = gen_abi::COMPOSITION_DESCRIPTOR_NODE_COUNT_OFFSET_V3;
    /// Exact edge-count offset.
    pub const EDGE_COUNT: usize = gen_abi::COMPOSITION_DESCRIPTOR_EDGE_COUNT_OFFSET_V3;
    /// Exact total sparse-term count offset.
    pub const TERM_COUNT: usize = gen_abi::COMPOSITION_DESCRIPTOR_TERM_COUNT_OFFSET_V3;
    /// Canonical root common denominator offset.
    pub const ROOT_DENOMINATOR: usize = gen_abi::COMPOSITION_DESCRIPTOR_ROOT_DENOMINATOR_OFFSET_V3;
    /// Reserved tail offset.
    pub const RESERVED_TAIL: usize = gen_abi::COMPOSITION_DESCRIPTOR_RESERVED_TAIL_OFFSET_V3;
}

/// Stable hostile-decode, topology, arithmetic, or translation refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A fixed or count-derived byte width differed.
    InvalidLength,
    /// Magic bytes selected another schema.
    InvalidMagic,
    /// The schema version is unsupported.
    UnsupportedVersion,
    /// Reserved bytes, enum fields, or zero-only coordinates were noncanonical.
    NonCanonical,
    /// A required identity was zero.
    ZeroIdentity,
    /// Finalized Record selection, digest, or owner/PDA authentication differed.
    ContentAdmission,
    /// A descriptor count exceeded this explicit executable capacity profile.
    CapacityExceeded,
    /// Product outcome width or a sparse outcome coordinate was invalid.
    InvalidOutcome,
    /// Node identities or canonical `(rank,id)` order were invalid.
    DuplicateOrUnorderedNode,
    /// An edge selected a missing, future, duplicate, zero-weight, or unordered child.
    InvalidEdge,
    /// A node kind, rank, arity, divisor, or native coordinate was invalid.
    InvalidNode,
    /// More than one root existed or a node was not reachable from the sole root.
    AmbiguousRoot,
    /// Sparse terms were empty, zero, duplicate, unordered, or not gcd-normalized.
    NonCanonicalPayoff,
    /// Checked `u64`/`u128` sum, product, LCM, offset, or narrowing overflowed.
    ArithmeticOverflow,
    /// Committed node flattening differed from direct child-edge composition.
    CompositionMismatch,
    /// Translation header or sparse bytes differed from the canonical graph root.
    TranslationMismatch,
    /// Exact quantity translation would require rounding.
    NonIntegralTranslation,
    /// Supplied native quantities differed from the exact canonical translation.
    ConservationMismatch,
}

/// Result alias for this total kernel.
pub type Result<T> = core::result::Result<T, Error>;

/// Finalized-record evidence supplied by the small physical adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecordAdmissionV3 {
    /// Content identity selected by the immutable parent descriptor.
    pub selected_id: [u8; 32],
    /// Content identity observed in finalized Record coordinates.
    pub finalized_id: [u8; 32],
    /// SHA-256 recomputed over the exact bytes by the adapter.
    pub recomputed_digest: [u8; 32],
    /// Content digest stored in the finalized Record identity.
    pub finalized_digest: [u8; 32],
    /// Record owner, raw/staging PDA, finality, and rent were authenticated.
    pub record_authenticated: bool,
}

/// Exact descriptor fields encoded without any mutable balance or payout fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositionDescriptorInputV3 {
    /// Logical Core Market owning the native liability basis.
    pub market: [u8; 32],
    /// Product-owned exhaustive result-domain identity.
    pub result_domain: [u8; 32],
    /// Immutable selected execution release set.
    pub release_set: [u8; 32],
    /// Product/Claims-owned exhaustive native liability-basis identity.
    pub native_basis: [u8; 32],
    /// Stable graph identity.
    pub graph_id: [u8; 32],
    /// SHA-256 of exact finalized graph bytes.
    pub graph_digest: [u8; 32],
    /// Sole canonical graph-root identity.
    pub root_id: [u8; 32],
    /// Stable canonical-translation identity.
    pub translation_id: [u8; 32],
    /// SHA-256 of exact finalized translation bytes.
    pub translation_digest: [u8; 32],
    /// Product-native outcome width.
    pub outcome_count: u32,
    /// Exact graph node count.
    pub node_count: u32,
    /// Exact graph edge count.
    pub edge_count: u32,
    /// Exact sparse-term count across every graph node.
    pub term_count: u32,
    /// Canonical root common denominator.
    pub root_denominator: u64,
}

/// Hostile-decoded immutable composition descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositionDescriptorV3 {
    descriptor_id: [u8; 32],
    fields: CompositionDescriptorInputV3,
}

impl CompositionDescriptorV3 {
    /// Decode exact descriptor bytes after finalized-record admission.
    pub fn decode(input: &[u8], admission: RecordAdmissionV3) -> Result<Self> {
        if input.len() != COMPOSITION_DESCRIPTOR_BYTES_V3 {
            return Err(Error::InvalidLength);
        }
        if array_at::<8>(input, DescriptorLayoutV3::MAGIC)? != COMPOSITION_DESCRIPTOR_MAGIC_V3 {
            return Err(Error::InvalidMagic);
        }
        if u16_at(input, DescriptorLayoutV3::VERSION)? != COMPOSITION_SCHEMA_VERSION_V3 {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(input, DescriptorLayoutV3::RESERVED_HEADER, 6)?;
        require_zero(input, DescriptorLayoutV3::RESERVED_TAIL, 8)?;
        validate_record_admission(admission, admission.selected_id, admission.finalized_digest)?;
        let fields = CompositionDescriptorInputV3 {
            market: nonzero_array(input, DescriptorLayoutV3::MARKET)?,
            result_domain: nonzero_array(input, DescriptorLayoutV3::RESULT_DOMAIN)?,
            release_set: nonzero_array(input, DescriptorLayoutV3::RELEASE_SET)?,
            native_basis: nonzero_array(input, DescriptorLayoutV3::NATIVE_BASIS)?,
            graph_id: nonzero_array(input, DescriptorLayoutV3::GRAPH_ID)?,
            graph_digest: nonzero_array(input, DescriptorLayoutV3::GRAPH_DIGEST)?,
            root_id: nonzero_array(input, DescriptorLayoutV3::ROOT_ID)?,
            translation_id: nonzero_array(input, DescriptorLayoutV3::TRANSLATION_ID)?,
            translation_digest: nonzero_array(input, DescriptorLayoutV3::TRANSLATION_DIGEST)?,
            outcome_count: u32_at(input, DescriptorLayoutV3::OUTCOME_COUNT)?,
            node_count: u32_at(input, DescriptorLayoutV3::NODE_COUNT)?,
            edge_count: u32_at(input, DescriptorLayoutV3::EDGE_COUNT)?,
            term_count: u32_at(input, DescriptorLayoutV3::TERM_COUNT)?,
            root_denominator: u64_at(input, DescriptorLayoutV3::ROOT_DENOMINATOR)?,
        };
        if array_at::<32>(input, DescriptorLayoutV3::CAPACITY_PROFILE)? != CAPACITY_PROFILE_ID_V3 {
            return Err(Error::CapacityExceeded);
        }
        validate_descriptor_fields(fields)?;
        Ok(Self {
            descriptor_id: admission.selected_id,
            fields,
        })
    }

    /// Finalized descriptor content identity.
    pub const fn descriptor_id(self) -> [u8; 32] {
        self.descriptor_id
    }

    /// Exact immutable fields.
    pub const fn fields(self) -> CompositionDescriptorInputV3 {
        self.fields
    }

    /// Logical Core Market identity.
    pub const fn market(self) -> [u8; 32] {
        self.fields.market
    }

    /// Product-owned exhaustive result-domain identity.
    pub const fn result_domain(self) -> [u8; 32] {
        self.fields.result_domain
    }

    /// Immutable execution release set.
    pub const fn release_set(self) -> [u8; 32] {
        self.fields.release_set
    }

    /// Exhaustive native liability-basis identity.
    pub const fn native_basis(self) -> [u8; 32] {
        self.fields.native_basis
    }

    /// Stable graph identity.
    pub const fn graph_id(self) -> [u8; 32] {
        self.fields.graph_id
    }

    /// Finalized graph digest.
    pub const fn graph_digest(self) -> [u8; 32] {
        self.fields.graph_digest
    }

    /// Sole graph root identity.
    pub const fn root_id(self) -> [u8; 32] {
        self.fields.root_id
    }

    /// Stable translation identity.
    pub const fn translation_id(self) -> [u8; 32] {
        self.fields.translation_id
    }

    /// Finalized translation digest.
    pub const fn translation_digest(self) -> [u8; 32] {
        self.fields.translation_digest
    }

    /// Product-native outcome width.
    pub const fn outcome_count(self) -> u32 {
        self.fields.outcome_count
    }

    /// Exact graph node count.
    pub const fn node_count(self) -> u32 {
        self.fields.node_count
    }

    /// Exact graph edge count.
    pub const fn edge_count(self) -> u32 {
        self.fields.edge_count
    }

    /// Exact total graph sparse-term count.
    pub const fn term_count(self) -> u32 {
        self.fields.term_count
    }

    /// Canonical root denominator.
    pub const fn root_denominator(self) -> u64 {
        self.fields.root_denominator
    }
}

/// Encode exact descriptor bytes atomically into caller-owned buffers.
pub fn encode_composition_descriptor_v3_atomic(
    input: CompositionDescriptorInputV3,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    if scratch.len() != COMPOSITION_DESCRIPTOR_BYTES_V3
        || output.len() != COMPOSITION_DESCRIPTOR_BYTES_V3
    {
        return Err(Error::InvalidLength);
    }
    validate_descriptor_fields(input)?;
    scratch.fill(0);
    put(
        scratch,
        DescriptorLayoutV3::MAGIC,
        &COMPOSITION_DESCRIPTOR_MAGIC_V3,
    )?;
    put(
        scratch,
        DescriptorLayoutV3::VERSION,
        &COMPOSITION_SCHEMA_VERSION_V3.to_le_bytes(),
    )?;
    for (offset, value) in [
        (DescriptorLayoutV3::MARKET, input.market),
        (DescriptorLayoutV3::RESULT_DOMAIN, input.result_domain),
        (DescriptorLayoutV3::RELEASE_SET, input.release_set),
        (DescriptorLayoutV3::NATIVE_BASIS, input.native_basis),
        (DescriptorLayoutV3::GRAPH_ID, input.graph_id),
        (DescriptorLayoutV3::GRAPH_DIGEST, input.graph_digest),
        (DescriptorLayoutV3::ROOT_ID, input.root_id),
        (DescriptorLayoutV3::TRANSLATION_ID, input.translation_id),
        (
            DescriptorLayoutV3::TRANSLATION_DIGEST,
            input.translation_digest,
        ),
        (DescriptorLayoutV3::CAPACITY_PROFILE, CAPACITY_PROFILE_ID_V3),
    ] {
        put(scratch, offset, &value)?;
    }
    for (offset, value) in [
        (DescriptorLayoutV3::OUTCOME_COUNT, input.outcome_count),
        (DescriptorLayoutV3::NODE_COUNT, input.node_count),
        (DescriptorLayoutV3::EDGE_COUNT, input.edge_count),
        (DescriptorLayoutV3::TERM_COUNT, input.term_count),
    ] {
        put(scratch, offset, &value.to_le_bytes())?;
    }
    put(
        scratch,
        DescriptorLayoutV3::ROOT_DENOMINATOR,
        &input.root_denominator.to_le_bytes(),
    )?;
    output.copy_from_slice(scratch);
    Ok(())
}

fn validate_descriptor_fields(input: CompositionDescriptorInputV3) -> Result<()> {
    if [
        input.market,
        input.result_domain,
        input.release_set,
        input.native_basis,
        input.graph_id,
        input.graph_digest,
        input.root_id,
        input.translation_id,
        input.translation_digest,
    ]
    .iter()
    .any(is_zero)
    {
        return Err(Error::ZeroIdentity);
    }
    if input.outcome_count < MIN_COMPOSITION_OUTCOMES_V3
        || input.outcome_count > MAX_COMPOSITION_OUTCOMES_V3
    {
        return Err(Error::InvalidOutcome);
    }
    if input.node_count == 0
        || input.node_count > MAX_COMPOSITION_NODES_V3
        || input.edge_count > MAX_COMPOSITION_EDGES_V3
        || input.term_count == 0
        || input.term_count > MAX_COMPOSITION_TERMS_V3
    {
        return Err(Error::CapacityExceeded);
    }
    if input.root_denominator == 0 {
        return Err(Error::NonCanonicalPayoff);
    }
    Ok(())
}

pub(crate) fn validate_record_admission(
    admission: RecordAdmissionV3,
    selected_id: [u8; 32],
    selected_digest: [u8; 32],
) -> Result<()> {
    if !admission.record_authenticated
        || is_zero(&selected_id)
        || is_zero(&selected_digest)
        || admission.selected_id != selected_id
        || admission.finalized_id != selected_id
        || admission.recomputed_digest != selected_digest
        || admission.finalized_digest != selected_digest
    {
        return Err(Error::ContentAdmission);
    }
    Ok(())
}

pub(crate) fn require_zero(input: &[u8], offset: usize, length: usize) -> Result<()> {
    if slice(input, offset, length)?
        .iter()
        .any(|value| *value != 0)
    {
        return Err(Error::NonCanonical);
    }
    Ok(())
}

pub(crate) fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

pub(crate) fn nonzero_array(input: &[u8], offset: usize) -> Result<[u8; 32]> {
    let value = array_at::<32>(input, offset)?;
    if is_zero(&value) {
        return Err(Error::ZeroIdentity);
    }
    Ok(value)
}

pub(crate) fn u16_at(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array_at(input, offset)?))
}

pub(crate) fn u32_at(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(array_at(input, offset)?))
}

pub(crate) fn u64_at(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(array_at(input, offset)?))
}

pub(crate) fn array_at<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    slice(input, offset, N)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

pub(crate) fn slice(input: &[u8], offset: usize, length: usize) -> Result<&[u8]> {
    input
        .get(offset..offset.checked_add(length).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)
}

pub(crate) fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    output
        .get_mut(
            offset
                ..offset
                    .checked_add(value.len())
                    .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

pub(crate) fn gcd_u64(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

pub(crate) fn gcd_u128(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}
