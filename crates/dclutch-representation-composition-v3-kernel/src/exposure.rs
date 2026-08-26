//! Authenticated Product-result to Claims-representation exposure boundary.
//!
//! A finalized exposure bundle is the canonical sparse flattening of a
//! representation-composition DAG for execution.  Its `K` ordered rank-one
//! roots are the exhaustive Claims/representation basis; their edges select
//! the `N` Product-result leaves.  Thus acyclicity is structural, row order is
//! canonical, and callers cannot supply a parallel matrix.

use crate::abi::{
    Error, RecordAdmissionV3, Result, array_at, gcd_u64, nonzero_array, put, require_zero, slice,
    u16_at, u32_at, u64_at, validate_record_admission,
};
use crate::{CompositionGraphV3, CompositionNodeKindV3};

/// Exposure-bundle schema version.
pub const COMPOSITION_EXPOSURE_VERSION_V3: u16 = 3;
/// Exposure-bundle magic.
pub const COMPOSITION_EXPOSURE_MAGIC_V3: [u8; 8] = *b"DCRCEX03";
/// Fixed header before ordered roots and sparse edges.
pub const COMPOSITION_EXPOSURE_HEADER_BYTES_V3: usize = 304;
/// Fixed ordered-root width.
pub const COMPOSITION_EXPOSURE_ROW_BYTES_V3: usize = 56;
/// Fixed sparse-edge width.
pub const COMPOSITION_EXPOSURE_TERM_BYTES_V3: usize = 16;
/// Minimum Product-result width in this execution profile.
pub const MIN_COMPOSITION_PRODUCT_WIDTH_V3: u32 = 1;
/// Maximum Product-result width in this execution profile.
pub const MAX_COMPOSITION_PRODUCT_WIDTH_V3: u32 = 512;
/// Maximum Claims/representation width in this execution profile.
pub const MAX_COMPOSITION_REPRESENTATION_WIDTH_V3: u32 = 256;
/// Maximum sparse edges in this execution profile.
pub const MAX_COMPOSITION_EXPOSURE_TERMS_V3: u32 = 65_536;

/// Schema preimage for finalized exposure bundles.
pub const COMPOSITION_EXPOSURE_SCHEMA_PREIMAGE_V3: &[u8] =
    b"dclutch/schema/product-representation-exposure-bundle-v3";
/// SHA-256 of [`COMPOSITION_EXPOSURE_SCHEMA_PREIMAGE_V3`].
pub const COMPOSITION_EXPOSURE_SCHEMA_ID_V3: [u8; 32] = [
    0xc8, 0xbf, 0x29, 0xb9, 0x97, 0x67, 0x94, 0xa7, 0x7d, 0x32, 0xbe, 0xd9, 0xd7, 0xfc, 0x93, 0x3d,
    0xcb, 0xfc, 0x78, 0x75, 0x91, 0x0c, 0x99, 0xc8, 0x0d, 0xe7, 0x18, 0xc3, 0xc0, 0x10, 0x07, 0x5a,
];
/// Capacity-profile preimage. These bounds are executable, not ontology.
pub const COMPOSITION_EXPOSURE_CAPACITY_PREIMAGE_V3: &[u8] = b"dclutch/capacity/product-representation-exposure-v3/product512/representation256/terms65536/u128";
/// SHA-256 of [`COMPOSITION_EXPOSURE_CAPACITY_PREIMAGE_V3`].
pub const COMPOSITION_EXPOSURE_CAPACITY_ID_V3: [u8; 32] = [
    0x44, 0x0b, 0x9a, 0x61, 0x16, 0x31, 0xa2, 0x3e, 0x68, 0x74, 0xaa, 0x94, 0x54, 0x07, 0xe2, 0x35,
    0x7a, 0xea, 0xab, 0x3f, 0xea, 0x4d, 0xd0, 0xd8, 0xc7, 0x31, 0x00, 0x9b, 0xdc, 0x83, 0x63, 0x9a,
];

/// Exposure-header byte layout.
pub struct CompositionExposureLayoutV3;

impl CompositionExposureLayoutV3 {
    /// Magic offset.
    pub const MAGIC: usize = 0;
    /// Version offset.
    pub const VERSION: usize = 8;
    /// Reserved header offset.
    pub const RESERVED_HEADER: usize = 10;
    /// Logical Market offset.
    pub const MARKET: usize = 16;
    /// Product-owned result-domain offset.
    pub const RESULT_DOMAIN: usize = 48;
    /// Selected release-set offset.
    pub const RELEASE_SET: usize = 80;
    /// Product-owned terminal-result basis offset.
    pub const PRODUCT_BASIS: usize = 112;
    /// Claims-owned representation basis offset.
    pub const REPRESENTATION_BASIS: usize = 144;
    /// Stable source composition-graph identity offset.
    pub const GRAPH_ID: usize = 176;
    /// Executable capacity-profile offset.
    pub const CAPACITY_PROFILE: usize = 208;
    /// Product terminal-result width offset.
    pub const PRODUCT_WIDTH: usize = 240;
    /// Claims/representation width offset.
    pub const REPRESENTATION_WIDTH: usize = 244;
    /// Ordered root count offset.
    pub const ROW_COUNT: usize = 248;
    /// Total sparse-edge count offset.
    pub const TERM_COUNT: usize = 252;
    /// Reserved tail offset.
    pub const RESERVED_TAIL: usize = 256;
}

/// Ordered exposure-root byte layout.
pub struct CompositionExposureRowLayoutV3;

impl CompositionExposureRowLayoutV3 {
    /// Stable composition-node identity offset.
    pub const NODE_ID: usize = 0;
    /// Claims/representation coordinate offset.
    pub const REPRESENTATION_COORDINATE: usize = 32;
    /// Canonical node rank offset; exact one for every row.
    pub const RANK: usize = 36;
    /// First sparse-edge index offset.
    pub const FIRST_TERM: usize = 40;
    /// Sparse-edge count offset.
    pub const TERM_COUNT: usize = 44;
    /// Positive normalized row denominator offset.
    pub const DENOMINATOR: usize = 48;
}

/// Sparse exposure-edge byte layout.
pub struct CompositionExposureTermLayoutV3;

impl CompositionExposureTermLayoutV3 {
    /// Product-result coordinate offset.
    pub const PRODUCT_COORDINATE: usize = 0;
    /// Reserved offset.
    pub const RESERVED: usize = 4;
    /// Positive numerator offset.
    pub const NUMERATOR: usize = 8;
}

/// One canonical sparse exposure edge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositionExposureTermV3 {
    /// Product-result coordinate selected by this edge.
    pub product_coordinate: u32,
    /// Positive exact coefficient numerator.
    pub numerator: u64,
}

/// One ordered Claims-basis root supplied to the atomic encoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositionExposureRowInputV3<'a> {
    /// Stable node identity from the source composition DAG.
    pub node_id: [u8; 32],
    /// Positive normalized common denominator.
    pub denominator: u64,
    /// Strictly ordered nonzero Product-result edges.
    pub terms: &'a [CompositionExposureTermV3],
}

/// Exact exposure-bundle input supplied to the atomic encoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositionExposureInputV3<'a> {
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Product-owned result domain.
    pub result_domain: [u8; 32],
    /// Immutable selected release set.
    pub release_set: [u8; 32],
    /// Product-owned terminal-result basis.
    pub product_basis: [u8; 32],
    /// Claims-owned representation basis.
    pub representation_basis: [u8; 32],
    /// Stable source composition-graph identity.
    pub graph_id: [u8; 32],
    /// Product terminal-result width `N`.
    pub product_width: u32,
    /// Canonically ordered Claims roots; their count is `K`.
    pub rows: &'a [CompositionExposureRowInputV3<'a>],
}

/// Independently authenticated identities and widths joined by an adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositionExposureExpectedV3 {
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Product-owned result domain.
    pub result_domain: [u8; 32],
    /// Immutable selected release set.
    pub release_set: [u8; 32],
    /// Product-owned terminal-result basis.
    pub product_basis: [u8; 32],
    /// Claims-owned representation basis.
    pub representation_basis: [u8; 32],
    /// Stable source composition-graph identity.
    pub graph_id: [u8; 32],
    /// Independently authenticated Product width `N`.
    pub product_width: u32,
    /// Independently authenticated Claims width `K`.
    pub representation_width: u32,
}

/// One decoded row root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositionExposureRowV3 {
    node_id: [u8; 32],
    representation_coordinate: u32,
    first_term: u32,
    term_count: u32,
    denominator: u64,
}

impl CompositionExposureRowV3 {
    /// Stable source-DAG node identity.
    pub const fn node_id(self) -> [u8; 32] {
        self.node_id
    }
    /// Canonical Claims coordinate.
    pub const fn representation_coordinate(self) -> u32 {
        self.representation_coordinate
    }
    /// Sparse-edge count.
    pub const fn term_count(self) -> u32 {
        self.term_count
    }

    /// Positive normalized denominator.
    pub const fn denominator(self) -> u64 {
        self.denominator
    }
}

/// Hostile-decoded, finalized Product-to-Claims exposure bundle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositionExposureBundleV3<'a> {
    bytes: &'a [u8],
    bundle_id: [u8; 32],
    bundle_digest: [u8; 32],
    market: [u8; 32],
    result_domain: [u8; 32],
    release_set: [u8; 32],
    product_basis: [u8; 32],
    representation_basis: [u8; 32],
    graph_id: [u8; 32],
    product_width: u32,
    representation_width: u32,
    term_count: u32,
}

impl<'a> CompositionExposureBundleV3<'a> {
    /// Decode and validate exact finalized bytes plus record admission.
    pub fn decode(input: &'a [u8], admission: RecordAdmissionV3) -> Result<Self> {
        validate_record_admission(admission, admission.selected_id, admission.finalized_digest)?;
        if input.len() < COMPOSITION_EXPOSURE_HEADER_BYTES_V3 {
            return Err(Error::InvalidLength);
        }
        if array_at::<8>(input, CompositionExposureLayoutV3::MAGIC)?
            != COMPOSITION_EXPOSURE_MAGIC_V3
        {
            return Err(Error::InvalidMagic);
        }
        if u16_at(input, CompositionExposureLayoutV3::VERSION)? != COMPOSITION_EXPOSURE_VERSION_V3 {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(input, CompositionExposureLayoutV3::RESERVED_HEADER, 6)?;
        require_zero(input, CompositionExposureLayoutV3::RESERVED_TAIL, 48)?;
        if array_at::<32>(input, CompositionExposureLayoutV3::CAPACITY_PROFILE)?
            != COMPOSITION_EXPOSURE_CAPACITY_ID_V3
        {
            return Err(Error::CapacityExceeded);
        }
        let value = Self {
            bytes: input,
            bundle_id: admission.selected_id,
            bundle_digest: admission.finalized_digest,
            market: nonzero_array(input, CompositionExposureLayoutV3::MARKET)?,
            result_domain: nonzero_array(input, CompositionExposureLayoutV3::RESULT_DOMAIN)?,
            release_set: nonzero_array(input, CompositionExposureLayoutV3::RELEASE_SET)?,
            product_basis: nonzero_array(input, CompositionExposureLayoutV3::PRODUCT_BASIS)?,
            representation_basis: nonzero_array(
                input,
                CompositionExposureLayoutV3::REPRESENTATION_BASIS,
            )?,
            graph_id: nonzero_array(input, CompositionExposureLayoutV3::GRAPH_ID)?,
            product_width: u32_at(input, CompositionExposureLayoutV3::PRODUCT_WIDTH)?,
            representation_width: u32_at(input, CompositionExposureLayoutV3::REPRESENTATION_WIDTH)?,
            term_count: u32_at(input, CompositionExposureLayoutV3::TERM_COUNT)?,
        };
        if u32_at(input, CompositionExposureLayoutV3::ROW_COUNT)? != value.representation_width {
            return Err(Error::AmbiguousRoot);
        }
        value.validate()?;
        Ok(value)
    }

    /// Exact authenticated bytes.
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }
    /// Finalized bundle content identity.
    pub const fn bundle_id(self) -> [u8; 32] {
        self.bundle_id
    }
    /// Finalized digest of the exact authenticated bundle bytes.
    pub const fn bundle_digest(self) -> [u8; 32] {
        self.bundle_digest
    }
    /// Logical Market.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }
    /// Product-owned result domain.
    pub const fn result_domain(self) -> [u8; 32] {
        self.result_domain
    }
    /// Selected release set.
    pub const fn release_set(self) -> [u8; 32] {
        self.release_set
    }
    /// Product-owned terminal-result basis.
    pub const fn product_basis(self) -> [u8; 32] {
        self.product_basis
    }
    /// Claims-owned representation basis.
    pub const fn representation_basis(self) -> [u8; 32] {
        self.representation_basis
    }
    /// Stable source composition-graph identity.
    pub const fn graph_id(self) -> [u8; 32] {
        self.graph_id
    }
    /// Product terminal-result width `N`.
    pub const fn product_width(self) -> u32 {
        self.product_width
    }
    /// Claims/representation width `K`.
    pub const fn representation_width(self) -> u32 {
        self.representation_width
    }
    /// Total sparse edge count.
    pub const fn term_count(self) -> u32 {
        self.term_count
    }

    /// Least common denominator across every canonical Claims root.
    pub fn common_denominator(self) -> Result<u64> {
        let mut common = 1_u128;
        let mut index = 0_u32;
        while index < self.representation_width {
            let denominator = u128::from(self.row(index)?.denominator);
            let divisor = gcd_u128(common, denominator);
            common = common
                .checked_div(divisor)
                .and_then(|value| value.checked_mul(denominator))
                .ok_or(Error::ArithmeticOverflow)?;
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        u64::try_from(common).map_err(|_| Error::ArithmeticOverflow)
    }

    /// Require exact independently authenticated identities and dimensions.
    pub fn verify_for(self, expected: CompositionExposureExpectedV3) -> Result<Self> {
        if self.market != expected.market
            || self.result_domain != expected.result_domain
            || self.release_set != expected.release_set
            || self.product_basis != expected.product_basis
            || self.representation_basis != expected.representation_basis
            || self.graph_id != expected.graph_id
        {
            return Err(Error::ContentAdmission);
        }
        if self.product_width != expected.product_width
            || self.representation_width != expected.representation_width
        {
            return Err(Error::InvalidOutcome);
        }
        Ok(self)
    }

    /// Join every ordered exposure root to the canonical native DAG basis.
    ///
    /// The composition graph owns the exhaustive `K` native basis. Every
    /// exposure row must name the graph's unique rank-zero native node at the
    /// same coordinate. Product coordinates remain confined to the exposure's
    /// independently authenticated `N` and cannot be reinterpreted as graph
    /// coordinates.
    pub fn verify_composition_graph(self, graph: CompositionGraphV3<'_>) -> Result<Self> {
        if self.graph_id != graph.graph_id() || self.representation_width != graph.outcome_count() {
            return Err(Error::CompositionMismatch);
        }
        let mut coordinate = 0_u32;
        while coordinate < self.representation_width {
            let row = self.row(coordinate)?;
            let mut matches = 0_u32;
            let mut node_index = 0_u32;
            while node_index < graph.node_count() {
                let node = graph.node(node_index)?;
                if node.kind() == CompositionNodeKindV3::Native
                    && node.native_outcome() == coordinate
                {
                    if node.rank() != 0 || node.id() != row.node_id {
                        return Err(Error::CompositionMismatch);
                    }
                    matches = matches.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
                }
                node_index = node_index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
            }
            if matches != 1 {
                return Err(Error::CompositionMismatch);
            }
            coordinate = coordinate.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(self)
    }

    /// Read one ordered Claims root.
    pub fn row(self, index: u32) -> Result<CompositionExposureRowV3> {
        if index >= self.representation_width {
            return Err(Error::InvalidOutcome);
        }
        let offset = table_offset(
            COMPOSITION_EXPOSURE_HEADER_BYTES_V3,
            index,
            COMPOSITION_EXPOSURE_ROW_BYTES_V3,
        )?;
        let bytes = slice(self.bytes, offset, COMPOSITION_EXPOSURE_ROW_BYTES_V3)?;
        if u32_at(bytes, CompositionExposureRowLayoutV3::RANK)? != 1 {
            return Err(Error::InvalidNode);
        }
        Ok(CompositionExposureRowV3 {
            node_id: nonzero_array(bytes, CompositionExposureRowLayoutV3::NODE_ID)?,
            representation_coordinate: u32_at(
                bytes,
                CompositionExposureRowLayoutV3::REPRESENTATION_COORDINATE,
            )?,
            first_term: u32_at(bytes, CompositionExposureRowLayoutV3::FIRST_TERM)?,
            term_count: u32_at(bytes, CompositionExposureRowLayoutV3::TERM_COUNT)?,
            denominator: u64_at(bytes, CompositionExposureRowLayoutV3::DENOMINATOR)?,
        })
    }

    /// Read one sparse Product-result edge from an ordered row.
    pub fn row_term(
        self,
        row: CompositionExposureRowV3,
        index: u32,
    ) -> Result<CompositionExposureTermV3> {
        if index >= row.term_count {
            return Err(Error::InvalidOutcome);
        }
        let term_index = row
            .first_term
            .checked_add(index)
            .ok_or(Error::ArithmeticOverflow)?;
        self.term(term_index)
    }

    /// Translate one exact Product payout partition into `K` Claims payouts.
    ///
    /// The output changes only after every row was checked for exact integer
    /// divisibility and `u64` bounds.
    pub fn translate_product_payouts(
        self,
        product_payouts: &[u64],
        scratch: &mut [u64],
        output: &mut [u64],
    ) -> Result<()> {
        let product_width =
            usize::try_from(self.product_width).map_err(|_| Error::InvalidLength)?;
        let representation_width =
            usize::try_from(self.representation_width).map_err(|_| Error::InvalidLength)?;
        if product_payouts.len() != product_width
            || scratch.len() != representation_width
            || output.len() != representation_width
        {
            return Err(Error::InvalidLength);
        }
        scratch.fill(0);
        let mut row_index = 0_u32;
        while row_index < self.representation_width {
            let row = self.row(row_index)?;
            let mut numerator = 0_u128;
            let mut term_index = 0_u32;
            while term_index < row.term_count {
                let term = self.row_term(row, term_index)?;
                let coordinate =
                    usize::try_from(term.product_coordinate).map_err(|_| Error::InvalidOutcome)?;
                let payout = *product_payouts
                    .get(coordinate)
                    .ok_or(Error::InvalidOutcome)?;
                numerator = numerator
                    .checked_add(
                        u128::from(term.numerator)
                            .checked_mul(u128::from(payout))
                            .ok_or(Error::ArithmeticOverflow)?,
                    )
                    .ok_or(Error::ArithmeticOverflow)?;
                term_index = term_index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
            }
            let denominator = u128::from(row.denominator);
            if !numerator.is_multiple_of(denominator) {
                return Err(Error::NonIntegralTranslation);
            }
            let translated =
                u64::try_from(numerator / denominator).map_err(|_| Error::ArithmeticOverflow)?;
            let coordinate = usize::try_from(row_index).map_err(|_| Error::InvalidOutcome)?;
            *scratch.get_mut(coordinate).ok_or(Error::InvalidOutcome)? = translated;
            row_index = row_index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        output.copy_from_slice(scratch);
        Ok(())
    }

    fn validate(self) -> Result<()> {
        if self.product_width < MIN_COMPOSITION_PRODUCT_WIDTH_V3
            || self.product_width > MAX_COMPOSITION_PRODUCT_WIDTH_V3
            || self.representation_width == 0
            || self.representation_width > MAX_COMPOSITION_REPRESENTATION_WIDTH_V3
            || self.term_count == 0
            || self.term_count > MAX_COMPOSITION_EXPOSURE_TERMS_V3
        {
            return Err(Error::CapacityExceeded);
        }
        if self.bytes.len()
            != composition_exposure_bytes_v3(self.representation_width, self.term_count)?
        {
            return Err(Error::InvalidLength);
        }
        let mut term_cursor = 0_u32;
        let mut row_index = 0_u32;
        while row_index < self.representation_width {
            let row = self.row(row_index)?;
            if row.representation_coordinate != row_index
                || row.first_term != term_cursor
                || row.term_count == 0
                || row.denominator == 0
            {
                return Err(Error::NonCanonical);
            }
            let mut prior_row = 0_u32;
            while prior_row < row_index {
                if self.row(prior_row)?.node_id == row.node_id {
                    return Err(Error::DuplicateOrUnorderedNode);
                }
                prior_row = prior_row.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
            }
            let mut normalization = row.denominator;
            let mut prior_product = None;
            let mut term_index = 0_u32;
            while term_index < row.term_count {
                let term = self.row_term(row, term_index)?;
                if term.product_coordinate >= self.product_width
                    || term.numerator == 0
                    || prior_product.is_some_and(|prior| term.product_coordinate <= prior)
                {
                    return Err(Error::NonCanonicalPayoff);
                }
                normalization = gcd_u64(normalization, term.numerator);
                prior_product = Some(term.product_coordinate);
                term_index = term_index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
            }
            if normalization != 1 {
                return Err(Error::NonCanonicalPayoff);
            }
            term_cursor = term_cursor
                .checked_add(row.term_count)
                .ok_or(Error::ArithmeticOverflow)?;
            row_index = row_index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        if term_cursor != self.term_count {
            return Err(Error::InvalidLength);
        }
        Ok(())
    }

    fn term(self, index: u32) -> Result<CompositionExposureTermV3> {
        if index >= self.term_count {
            return Err(Error::InvalidOutcome);
        }
        let rows_bytes = usize::try_from(self.representation_width)
            .map_err(|_| Error::InvalidLength)?
            .checked_mul(COMPOSITION_EXPOSURE_ROW_BYTES_V3)
            .ok_or(Error::InvalidLength)?;
        let base = COMPOSITION_EXPOSURE_HEADER_BYTES_V3
            .checked_add(rows_bytes)
            .ok_or(Error::InvalidLength)?;
        let offset = table_offset(base, index, COMPOSITION_EXPOSURE_TERM_BYTES_V3)?;
        let bytes = slice(self.bytes, offset, COMPOSITION_EXPOSURE_TERM_BYTES_V3)?;
        require_zero(bytes, CompositionExposureTermLayoutV3::RESERVED, 4)?;
        Ok(CompositionExposureTermV3 {
            product_coordinate: u32_at(bytes, CompositionExposureTermLayoutV3::PRODUCT_COORDINATE)?,
            numerator: u64_at(bytes, CompositionExposureTermLayoutV3::NUMERATOR)?,
        })
    }
}

/// Return the exact bundle width for validated dimensions.
pub fn composition_exposure_bytes_v3(representation_width: u32, term_count: u32) -> Result<usize> {
    if representation_width == 0
        || representation_width > MAX_COMPOSITION_REPRESENTATION_WIDTH_V3
        || term_count == 0
        || term_count > MAX_COMPOSITION_EXPOSURE_TERMS_V3
    {
        return Err(Error::CapacityExceeded);
    }
    COMPOSITION_EXPOSURE_HEADER_BYTES_V3
        .checked_add(
            usize::try_from(representation_width)
                .map_err(|_| Error::InvalidLength)?
                .checked_mul(COMPOSITION_EXPOSURE_ROW_BYTES_V3)
                .ok_or(Error::InvalidLength)?,
        )
        .and_then(|value| {
            value.checked_add(
                usize::try_from(term_count)
                    .ok()?
                    .checked_mul(COMPOSITION_EXPOSURE_TERM_BYTES_V3)?,
            )
        })
        .ok_or(Error::InvalidLength)
}

/// Encode a canonical exposure bundle atomically into caller-owned buffers.
pub fn encode_composition_exposure_v3_atomic(
    input: CompositionExposureInputV3<'_>,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    let representation_width =
        u32::try_from(input.rows.len()).map_err(|_| Error::CapacityExceeded)?;
    let mut term_count = 0_u32;
    for row in input.rows {
        term_count = term_count
            .checked_add(u32::try_from(row.terms.len()).map_err(|_| Error::CapacityExceeded)?)
            .ok_or(Error::ArithmeticOverflow)?;
    }
    let length = composition_exposure_bytes_v3(representation_width, term_count)?;
    if scratch.len() != length || output.len() != length {
        return Err(Error::InvalidLength);
    }
    scratch.fill(0);
    put(
        scratch,
        CompositionExposureLayoutV3::MAGIC,
        &COMPOSITION_EXPOSURE_MAGIC_V3,
    )?;
    put(
        scratch,
        CompositionExposureLayoutV3::VERSION,
        &COMPOSITION_EXPOSURE_VERSION_V3.to_le_bytes(),
    )?;
    for (offset, value) in [
        (CompositionExposureLayoutV3::MARKET, input.market),
        (
            CompositionExposureLayoutV3::RESULT_DOMAIN,
            input.result_domain,
        ),
        (CompositionExposureLayoutV3::RELEASE_SET, input.release_set),
        (
            CompositionExposureLayoutV3::PRODUCT_BASIS,
            input.product_basis,
        ),
        (
            CompositionExposureLayoutV3::REPRESENTATION_BASIS,
            input.representation_basis,
        ),
        (CompositionExposureLayoutV3::GRAPH_ID, input.graph_id),
        (
            CompositionExposureLayoutV3::CAPACITY_PROFILE,
            COMPOSITION_EXPOSURE_CAPACITY_ID_V3,
        ),
    ] {
        put(scratch, offset, &value)?;
    }
    for (offset, value) in [
        (
            CompositionExposureLayoutV3::PRODUCT_WIDTH,
            input.product_width,
        ),
        (
            CompositionExposureLayoutV3::REPRESENTATION_WIDTH,
            representation_width,
        ),
        (CompositionExposureLayoutV3::ROW_COUNT, representation_width),
        (CompositionExposureLayoutV3::TERM_COUNT, term_count),
    ] {
        put(scratch, offset, &value.to_le_bytes())?;
    }
    let mut term_cursor = 0_u32;
    for (row_index, row) in input.rows.iter().enumerate() {
        let row_index_u32 = u32::try_from(row_index).map_err(|_| Error::CapacityExceeded)?;
        let offset = table_offset(
            COMPOSITION_EXPOSURE_HEADER_BYTES_V3,
            row_index_u32,
            COMPOSITION_EXPOSURE_ROW_BYTES_V3,
        )?;
        put(
            scratch,
            offset + CompositionExposureRowLayoutV3::NODE_ID,
            &row.node_id,
        )?;
        put(
            scratch,
            offset + CompositionExposureRowLayoutV3::REPRESENTATION_COORDINATE,
            &row_index_u32.to_le_bytes(),
        )?;
        put(
            scratch,
            offset + CompositionExposureRowLayoutV3::RANK,
            &1_u32.to_le_bytes(),
        )?;
        put(
            scratch,
            offset + CompositionExposureRowLayoutV3::FIRST_TERM,
            &term_cursor.to_le_bytes(),
        )?;
        let row_term_count = u32::try_from(row.terms.len()).map_err(|_| Error::CapacityExceeded)?;
        put(
            scratch,
            offset + CompositionExposureRowLayoutV3::TERM_COUNT,
            &row_term_count.to_le_bytes(),
        )?;
        put(
            scratch,
            offset + CompositionExposureRowLayoutV3::DENOMINATOR,
            &row.denominator.to_le_bytes(),
        )?;
        for term in row.terms {
            let rows_bytes = input
                .rows
                .len()
                .checked_mul(COMPOSITION_EXPOSURE_ROW_BYTES_V3)
                .ok_or(Error::InvalidLength)?;
            let terms_base = COMPOSITION_EXPOSURE_HEADER_BYTES_V3
                .checked_add(rows_bytes)
                .ok_or(Error::InvalidLength)?;
            let term_offset =
                table_offset(terms_base, term_cursor, COMPOSITION_EXPOSURE_TERM_BYTES_V3)?;
            put(
                scratch,
                term_offset + CompositionExposureTermLayoutV3::PRODUCT_COORDINATE,
                &term.product_coordinate.to_le_bytes(),
            )?;
            put(
                scratch,
                term_offset + CompositionExposureTermLayoutV3::NUMERATOR,
                &term.numerator.to_le_bytes(),
            )?;
            term_cursor = term_cursor
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?;
        }
    }
    let admission = RecordAdmissionV3 {
        selected_id: [1; 32],
        finalized_id: [1; 32],
        recomputed_digest: [2; 32],
        finalized_digest: [2; 32],
        record_authenticated: true,
    };
    CompositionExposureBundleV3::decode(scratch, admission)?;
    output.copy_from_slice(scratch);
    Ok(())
}

fn table_offset(base: usize, index: u32, stride: usize) -> Result<usize> {
    base.checked_add(
        usize::try_from(index)
            .map_err(|_| Error::InvalidLength)?
            .checked_mul(stride)
            .ok_or(Error::InvalidLength)?,
    )
    .ok_or(Error::InvalidLength)
}

const fn gcd_u128(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}
