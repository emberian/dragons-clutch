//! Canonical runtime-width affine Claims batch ABI.
//!
//! The packet owns only Claims coordinates. An ordered, duplicate-free Position
//! table is followed by affine rows that index it. Every row mutates one outcome
//! and conserves the signed Position deltas against the aggregate supply delta.
//! Full-range `u64` magnitudes use a canonical direction tag instead of an
//! asymmetric signed integer representation.

use super::CallerRole;

/// Bytes in the fixed batch-plan header before the Position table.
pub const AFFINE_BATCH_PLAN_HEADER_BYTES_V2: usize = 240;
/// Bytes in one unique Position-table entry.
pub const AFFINE_BATCH_POSITION_BYTES_V2: usize = 40;
/// Bytes in one ordered affine row.
pub const AFFINE_BATCH_ROW_BYTES_V2: usize = 64;
/// Bytes in one fixed affine-batch receipt.
pub const AFFINE_BATCH_RECEIPT_BYTES_V2: usize = 376;
/// Canonical affine-batch plan magic.
pub const AFFINE_BATCH_PLAN_MAGIC_V2: [u8; 8] = *b"DCLABP02";
/// Canonical affine-batch receipt magic.
pub const AFFINE_BATCH_RECEIPT_MAGIC_V2: [u8; 8] = *b"DCLABR02";
/// Implemented affine-batch wire version.
pub const AFFINE_BATCH_WIRE_VERSION_V2: u16 = 2;

const VERSION_OFFSET: usize = 8;
const CALLER_ROLE_OFFSET: usize = 10;
const HEADER_RESERVED_OFFSET: usize = 11;
const RELEASE_SET_OFFSET: usize = 16;
const MARKET_OFFSET: usize = 48;
const REQUEST_OFFSET: usize = 80;
const PRODUCT_OFFSET: usize = 112;
const BASIS_OFFSET: usize = 144;
const LINKED_BASIS_RECORD_OFFSET: usize = 176;
const MARKET_REVISION_OFFSET: usize = 208;
const OUTCOME_COUNT_OFFSET: usize = 216;
const POSITION_COUNT_OFFSET: usize = 220;
const ROW_COUNT_OFFSET: usize = 224;
const HEADER_TAIL_RESERVED_OFFSET: usize = 228;

const POSITION_OWNER_OFFSET: usize = 0;
const POSITION_REVISION_OFFSET: usize = 32;

const ROW_SOURCE_PRESENT_OFFSET: usize = 0;
const ROW_DESTINATION_PRESENT_OFFSET: usize = 1;
const ROW_RESERVED_OFFSET: usize = 2;
const ROW_OUTCOME_OFFSET: usize = 4;
const ROW_SOURCE_INDEX_OFFSET: usize = 8;
const ROW_DESTINATION_INDEX_OFFSET: usize = 12;
const ROW_AGGREGATE_DELTA_OFFSET: usize = 16;
const ROW_SOURCE_DELTA_OFFSET: usize = 32;
const ROW_DESTINATION_DELTA_OFFSET: usize = 48;

const DELTA_DIRECTION_OFFSET: usize = 0;
const DELTA_RESERVED_OFFSET: usize = 1;
const DELTA_MAGNITUDE_OFFSET: usize = 8;

const RECEIPT_PACKET_DIGEST_OFFSET: usize = 208;
const RECEIPT_TABLE_DIGEST_OFFSET: usize = 240;
const RECEIPT_CLAIMS_PROGRAM_OFFSET: usize = 272;
const RECEIPT_RESOURCE_DIGEST_OFFSET: usize = 304;
const RECEIPT_PRE_MARKET_REVISION_OFFSET: usize = 336;
const RECEIPT_POST_MARKET_REVISION_OFFSET: usize = 344;
const RECEIPT_OUTCOME_COUNT_OFFSET: usize = 352;
const RECEIPT_POSITION_COUNT_OFFSET: usize = 356;
const RECEIPT_ROW_COUNT_OFFSET: usize = 360;
const RECEIPT_RESERVED_OFFSET: usize = 364;

/// Canonical patchable byte coordinates of `AffineBatchPlanV2`.
///
/// The fixed request template contains the header and Position table, while
/// the item template contains exactly one affine row. Generic EffectProgram
/// encoders use these coordinates without becoming a second ABI authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AffineBatchRequestLayoutV2;

impl AffineBatchRequestLayoutV2 {
    /// Selected execution release-set identity in the fixed header.
    pub const RELEASE_SET: usize = RELEASE_SET_OFFSET;
    /// Logical Core Market identity in the fixed header.
    pub const MARKET: usize = MARKET_OFFSET;
    /// Complete parent-request digest in the fixed header.
    pub const REQUEST_DIGEST: usize = REQUEST_OFFSET;
    /// Exact Product record identity in the fixed header.
    pub const PRODUCT_RECORD: usize = PRODUCT_OFFSET;
    /// Exact semantic basis identity in the fixed header.
    pub const SEMANTIC_BASIS: usize = BASIS_OFFSET;
    /// Exact linked basis-record identity in the fixed header.
    pub const LINKED_BASIS_RECORD: usize = LINKED_BASIS_RECORD_OFFSET;
    /// Claims aggregate pre-revision as little-endian `u64`.
    pub const EXPECTED_MARKET_REVISION: usize = MARKET_REVISION_OFFSET;
    /// Product-authenticated outcome count as little-endian `u32`.
    pub const OUTCOME_COUNT: usize = OUTCOME_COUNT_OFFSET;
    /// Runtime row count as little-endian `u32`.
    pub const ROW_COUNT: usize = ROW_COUNT_OFFSET;
    /// Start of the fixed Position table.
    pub const POSITION_TABLE: usize = AFFINE_BATCH_PLAN_HEADER_BYTES_V2;
    /// Width of one fixed Position-table entry.
    pub const POSITION_STRIDE: usize = AFFINE_BATCH_POSITION_BYTES_V2;
    /// Owner identity within one Position-table entry.
    pub const POSITION_OWNER: usize = POSITION_OWNER_OFFSET;
    /// Pre-revision within one Position-table entry.
    pub const POSITION_REVISION: usize = POSITION_REVISION_OFFSET;
    /// Source-presence byte within the repeated row template.
    pub const ROW_SOURCE_PRESENT: usize = ROW_SOURCE_PRESENT_OFFSET;
    /// Destination-presence byte within the repeated row template.
    pub const ROW_DESTINATION_PRESENT: usize = ROW_DESTINATION_PRESENT_OFFSET;
    /// Outcome coordinate within the repeated row template.
    pub const ROW_OUTCOME: usize = ROW_OUTCOME_OFFSET;
    /// Source Position-table index within the repeated row template.
    pub const ROW_SOURCE_INDEX: usize = ROW_SOURCE_INDEX_OFFSET;
    /// Destination Position-table index within the repeated row template.
    pub const ROW_DESTINATION_INDEX: usize = ROW_DESTINATION_INDEX_OFFSET;
    /// Aggregate direction tag within the repeated row template.
    pub const ROW_AGGREGATE_DIRECTION: usize = ROW_AGGREGATE_DELTA_OFFSET + DELTA_DIRECTION_OFFSET;
    /// Aggregate magnitude within the repeated row template.
    pub const ROW_AGGREGATE_MAGNITUDE: usize = ROW_AGGREGATE_DELTA_OFFSET + DELTA_MAGNITUDE_OFFSET;
    /// Source direction tag within the repeated row template.
    pub const ROW_SOURCE_DIRECTION: usize = ROW_SOURCE_DELTA_OFFSET + DELTA_DIRECTION_OFFSET;
    /// Source magnitude within the repeated row template.
    pub const ROW_SOURCE_MAGNITUDE: usize = ROW_SOURCE_DELTA_OFFSET + DELTA_MAGNITUDE_OFFSET;
    /// Destination direction tag within the repeated row template.
    pub const ROW_DESTINATION_DIRECTION: usize =
        ROW_DESTINATION_DELTA_OFFSET + DELTA_DIRECTION_OFFSET;
    /// Destination magnitude within the repeated row template.
    pub const ROW_DESTINATION_MAGNITUDE: usize =
        ROW_DESTINATION_DELTA_OFFSET + DELTA_MAGNITUDE_OFFSET;

    /// Return one Position field's checked absolute offset in the fixed template.
    pub const fn position_field(position: u32, field: usize) -> Option<usize> {
        match (position as usize).checked_mul(Self::POSITION_STRIDE) {
            Some(item) => match Self::POSITION_TABLE.checked_add(item) {
                Some(base) => base.checked_add(field),
                None => None,
            },
            None => None,
        }
    }
}

/// Stable affine-batch ABI refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AffineBatchErrorV2 {
    /// A plan, table, row, or receipt had the wrong exact width.
    InvalidLength,
    /// Magic bytes selected another packet family.
    InvalidMagic,
    /// The wire version is unsupported.
    UnsupportedVersion,
    /// Reserved or inactive fields were not canonical zero bytes.
    NonCanonical,
    /// A required identity was zero.
    ZeroIdentity,
    /// A role, presence, or direction tag was unknown.
    UnknownTag,
    /// Runtime counts were zero or overflowed address arithmetic.
    InvalidCount,
    /// A Position-table owner was duplicated or an entry was unused.
    InvalidPositionTable,
    /// A row index or outcome was outside its runtime table.
    InvalidIndex,
    /// A Position/outcome coordinate was mutated more than once.
    DuplicateCoordinate,
    /// A source and destination aliased in one row.
    AliasedPosition,
    /// A delta had a noncanonical direction or did not conserve exactly.
    InvalidDelta,
    /// An optimistic revision could not advance exactly once.
    InvalidRevision,
    /// Receipt facts did not agree with their committed plan.
    ReceiptMismatch,
}

/// Result alias for the affine-batch ABI.
pub type Result<T> = core::result::Result<T, AffineBatchErrorV2>;

/// Canonical direction for one full-range exact magnitude.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DeltaDirectionV2 {
    /// The sole canonical direction for magnitude zero.
    Neutral = 0,
    /// Add the magnitude to the resource.
    Credit = 1,
    /// Subtract the magnitude from the resource.
    Debit = 2,
}

impl DeltaDirectionV2 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Neutral),
            1 => Ok(Self::Credit),
            2 => Ok(Self::Debit),
            _ => Err(AffineBatchErrorV2::UnknownTag),
        }
    }
}

/// One canonical signed-magnitude delta over the full `u64` quantity range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignedMagnitudeV2 {
    direction: DeltaDirectionV2,
    magnitude: u64,
}

impl SignedMagnitudeV2 {
    /// Construct one canonical exact delta.
    pub const fn new(direction: DeltaDirectionV2, magnitude: u64) -> Result<Self> {
        if (magnitude == 0) != matches!(direction, DeltaDirectionV2::Neutral) {
            return Err(AffineBatchErrorV2::InvalidDelta);
        }
        Ok(Self {
            direction,
            magnitude,
        })
    }

    /// Return the exact direction.
    pub const fn direction(self) -> DeltaDirectionV2 {
        self.direction
    }

    /// Return the exact unsigned magnitude.
    pub const fn magnitude(self) -> u64 {
        self.magnitude
    }

    fn signed_i128(self) -> i128 {
        match self.direction {
            DeltaDirectionV2::Neutral => 0,
            DeltaDirectionV2::Credit => i128::from(self.magnitude),
            DeltaDirectionV2::Debit => -i128::from(self.magnitude),
        }
    }

    fn decode(bytes: &[u8], offset: usize) -> Result<Self> {
        require_zero(bytes, add(offset, DELTA_RESERVED_OFFSET)?, 7)?;
        Self::new(
            DeltaDirectionV2::decode(byte_at(bytes, add(offset, DELTA_DIRECTION_OFFSET)?)?)?,
            u64_at(bytes, add(offset, DELTA_MAGNITUDE_OFFSET)?)?,
        )
    }

    fn encode_into(self, output: &mut [u8], offset: usize) -> Result<()> {
        put(
            output,
            add(offset, DELTA_DIRECTION_OFFSET)?,
            &[self.direction as u8],
        )?;
        put(
            output,
            add(offset, DELTA_MAGNITUDE_OFFSET)?,
            &self.magnitude.to_le_bytes(),
        )
    }
}

/// One unique Position entry referenced by zero or more packet rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AffineBatchPositionV2 {
    owner: [u8; 32],
    expected_revision: u64,
}

impl AffineBatchPositionV2 {
    /// Construct one Position coordinate that can advance exactly once.
    pub fn new(owner: [u8; 32], expected_revision: u64) -> Result<Self> {
        if is_zero(owner) {
            return Err(AffineBatchErrorV2::ZeroIdentity);
        }
        if expected_revision == u64::MAX {
            return Err(AffineBatchErrorV2::InvalidRevision);
        }
        Ok(Self {
            owner,
            expected_revision,
        })
    }

    /// Return the sole Position owner.
    pub const fn owner(self) -> [u8; 32] {
        self.owner
    }

    /// Return the optimistic pre-revision.
    pub const fn expected_revision(self) -> u64 {
        self.expected_revision
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        Self::new(
            array_at(bytes, POSITION_OWNER_OFFSET)?,
            u64_at(bytes, POSITION_REVISION_OFFSET)?,
        )
    }

    fn encode_into(self, output: &mut [u8]) -> Result<()> {
        put(output, POSITION_OWNER_OFFSET, &self.owner)?;
        put(
            output,
            POSITION_REVISION_OFFSET,
            &self.expected_revision.to_le_bytes(),
        )
    }
}

/// Construction input for one ordered affine row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AffineBatchRowInputV2 {
    /// Whether the source Position-table index is active.
    pub source_present: bool,
    /// Whether the destination Position-table index is active.
    pub destination_present: bool,
    /// Runtime outcome coordinate mutated by this row.
    pub outcome: u32,
    /// Source Position-table index; zero when inactive.
    pub source_position_index: u32,
    /// Destination Position-table index; zero when inactive.
    pub destination_position_index: u32,
    /// Exact aggregate-supply delta for this outcome.
    pub aggregate_delta: SignedMagnitudeV2,
    /// Exact source-Position delta; neutral zero when inactive.
    pub source_delta: SignedMagnitudeV2,
    /// Exact destination-Position delta; neutral zero when inactive.
    pub destination_delta: SignedMagnitudeV2,
}

/// One canonical ordered affine row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AffineBatchRowV2(AffineBatchRowInputV2);

impl AffineBatchRowV2 {
    /// Construct and canonicalize one row against runtime counts.
    pub fn new(
        input: AffineBatchRowInputV2,
        outcome_count: u32,
        position_count: u32,
    ) -> Result<Self> {
        if outcome_count == 0 || position_count == 0 || input.outcome >= outcome_count {
            return Err(AffineBatchErrorV2::InvalidIndex);
        }
        if !input.source_present && !input.destination_present {
            return Err(AffineBatchErrorV2::InvalidDelta);
        }
        validate_endpoint(
            input.source_present,
            input.source_position_index,
            input.source_delta,
            position_count,
            DeltaDirectionV2::Debit,
        )?;
        validate_endpoint(
            input.destination_present,
            input.destination_position_index,
            input.destination_delta,
            position_count,
            DeltaDirectionV2::Credit,
        )?;
        if input.source_present
            && input.destination_present
            && input.source_position_index == input.destination_position_index
        {
            return Err(AffineBatchErrorV2::AliasedPosition);
        }
        let position_delta = input
            .source_delta
            .signed_i128()
            .checked_add(input.destination_delta.signed_i128())
            .ok_or(AffineBatchErrorV2::InvalidDelta)?;
        if position_delta != input.aggregate_delta.signed_i128() {
            return Err(AffineBatchErrorV2::InvalidDelta);
        }
        Ok(Self(input))
    }

    /// Return whether the source Position is present.
    pub const fn source_present(self) -> bool {
        self.0.source_present
    }

    /// Return whether the destination Position is present.
    pub const fn destination_present(self) -> bool {
        self.0.destination_present
    }

    /// Return the runtime outcome coordinate.
    pub const fn outcome(self) -> u32 {
        self.0.outcome
    }

    /// Return the source Position-table index.
    pub const fn source_position_index(self) -> u32 {
        self.0.source_position_index
    }

    /// Return the destination Position-table index.
    pub const fn destination_position_index(self) -> u32 {
        self.0.destination_position_index
    }

    /// Return the exact aggregate delta.
    pub const fn aggregate_delta(self) -> SignedMagnitudeV2 {
        self.0.aggregate_delta
    }

    /// Return the exact source delta.
    pub const fn source_delta(self) -> SignedMagnitudeV2 {
        self.0.source_delta
    }

    /// Return the exact destination delta.
    pub const fn destination_delta(self) -> SignedMagnitudeV2 {
        self.0.destination_delta
    }

    fn decode(bytes: &[u8], outcome_count: u32, position_count: u32) -> Result<Self> {
        if bytes.len() != AFFINE_BATCH_ROW_BYTES_V2 {
            return Err(AffineBatchErrorV2::InvalidLength);
        }
        require_zero(bytes, ROW_RESERVED_OFFSET, 2)?;
        Self::new(
            AffineBatchRowInputV2 {
                source_present: bool_at(bytes, ROW_SOURCE_PRESENT_OFFSET)?,
                destination_present: bool_at(bytes, ROW_DESTINATION_PRESENT_OFFSET)?,
                outcome: u32_at(bytes, ROW_OUTCOME_OFFSET)?,
                source_position_index: u32_at(bytes, ROW_SOURCE_INDEX_OFFSET)?,
                destination_position_index: u32_at(bytes, ROW_DESTINATION_INDEX_OFFSET)?,
                aggregate_delta: SignedMagnitudeV2::decode(bytes, ROW_AGGREGATE_DELTA_OFFSET)?,
                source_delta: SignedMagnitudeV2::decode(bytes, ROW_SOURCE_DELTA_OFFSET)?,
                destination_delta: SignedMagnitudeV2::decode(bytes, ROW_DESTINATION_DELTA_OFFSET)?,
            },
            outcome_count,
            position_count,
        )
    }

    fn encode_into(self, output: &mut [u8]) -> Result<()> {
        if output.len() != AFFINE_BATCH_ROW_BYTES_V2 {
            return Err(AffineBatchErrorV2::InvalidLength);
        }
        put(
            output,
            ROW_SOURCE_PRESENT_OFFSET,
            &[u8::from(self.0.source_present)],
        )?;
        put(
            output,
            ROW_DESTINATION_PRESENT_OFFSET,
            &[u8::from(self.0.destination_present)],
        )?;
        put(output, ROW_OUTCOME_OFFSET, &self.0.outcome.to_le_bytes())?;
        put(
            output,
            ROW_SOURCE_INDEX_OFFSET,
            &self.0.source_position_index.to_le_bytes(),
        )?;
        put(
            output,
            ROW_DESTINATION_INDEX_OFFSET,
            &self.0.destination_position_index.to_le_bytes(),
        )?;
        self.0
            .aggregate_delta
            .encode_into(output, ROW_AGGREGATE_DELTA_OFFSET)?;
        self.0
            .source_delta
            .encode_into(output, ROW_SOURCE_DELTA_OFFSET)?;
        self.0
            .destination_delta
            .encode_into(output, ROW_DESTINATION_DELTA_OFFSET)
    }
}

/// Immutable header facts used to construct one affine-batch plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AffineBatchPlanInputV2 {
    /// Registry role of the selected caller program.
    pub caller_role: CallerRole,
    /// Immutable current execution release set.
    pub release_set: [u8; 32],
    /// Canonical logical Core Market identity.
    pub market: [u8; 32],
    /// Caller-owned request identity.
    pub request_id: [u8; 32],
    /// Exact finalized Product-record digest.
    pub product_record_digest: [u8; 32],
    /// Exact semantic LiabilityBasisV2 identity.
    pub semantic_basis_id: [u8; 32],
    /// Exact finalized linked-basis raw-record digest.
    pub linked_basis_record_digest: [u8; 32],
    /// Optimistic aggregate pre-revision.
    pub expected_market_revision: u64,
    /// Runtime claim/outcome width.
    pub outcome_count: u32,
}

/// Borrowed, exact-width affine-batch plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AffineBatchPlanV2<'a> {
    input: AffineBatchPlanInputV2,
    position_count: u32,
    row_count: u32,
    positions: &'a [u8],
    rows: &'a [u8],
}

impl<'a> AffineBatchPlanV2<'a> {
    /// Decode and fully canonicalize one hostile packet without allocation.
    pub fn decode(input: &'a [u8]) -> Result<Self> {
        if input.len() < AFFINE_BATCH_PLAN_HEADER_BYTES_V2 {
            return Err(AffineBatchErrorV2::InvalidLength);
        }
        exact(input, 0, &AFFINE_BATCH_PLAN_MAGIC_V2)?;
        if u16_at(input, VERSION_OFFSET)? != AFFINE_BATCH_WIRE_VERSION_V2 {
            return Err(AffineBatchErrorV2::UnsupportedVersion);
        }
        require_zero(input, HEADER_RESERVED_OFFSET, 5)?;
        require_zero(input, HEADER_TAIL_RESERVED_OFFSET, 12)?;
        let position_count = u32_at(input, POSITION_COUNT_OFFSET)?;
        let row_count = u32_at(input, ROW_COUNT_OFFSET)?;
        let positions_bytes = table_bytes(position_count, AFFINE_BATCH_POSITION_BYTES_V2)?;
        let rows_bytes = table_bytes(row_count, AFFINE_BATCH_ROW_BYTES_V2)?;
        let rows_offset = AFFINE_BATCH_PLAN_HEADER_BYTES_V2
            .checked_add(positions_bytes)
            .ok_or(AffineBatchErrorV2::InvalidLength)?;
        let expected = rows_offset
            .checked_add(rows_bytes)
            .ok_or(AffineBatchErrorV2::InvalidLength)?;
        if input.len() != expected {
            return Err(AffineBatchErrorV2::InvalidLength);
        }
        let value = Self {
            input: AffineBatchPlanInputV2 {
                caller_role: decode_role(byte_at(input, CALLER_ROLE_OFFSET)?)?,
                release_set: nonzero_array(input, RELEASE_SET_OFFSET)?,
                market: nonzero_array(input, MARKET_OFFSET)?,
                request_id: nonzero_array(input, REQUEST_OFFSET)?,
                product_record_digest: nonzero_array(input, PRODUCT_OFFSET)?,
                semantic_basis_id: nonzero_array(input, BASIS_OFFSET)?,
                linked_basis_record_digest: nonzero_array(input, LINKED_BASIS_RECORD_OFFSET)?,
                expected_market_revision: u64_at(input, MARKET_REVISION_OFFSET)?,
                outcome_count: u32_at(input, OUTCOME_COUNT_OFFSET)?,
            },
            position_count,
            row_count,
            positions: slice(input, AFFINE_BATCH_PLAN_HEADER_BYTES_V2, positions_bytes)?,
            rows: slice(input, rows_offset, rows_bytes)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact packet into caller-owned storage.
    pub fn encode_into(
        input: AffineBatchPlanInputV2,
        positions: &[AffineBatchPositionV2],
        rows: &[AffineBatchRowV2],
        output: &mut [u8],
    ) -> Result<()> {
        let position_count =
            u32::try_from(positions.len()).map_err(|_| AffineBatchErrorV2::InvalidCount)?;
        let row_count = u32::try_from(rows.len()).map_err(|_| AffineBatchErrorV2::InvalidCount)?;
        let expected = plan_bytes(position_count, row_count)?;
        if output.len() != expected {
            return Err(AffineBatchErrorV2::InvalidLength);
        }
        output.fill(0);
        put(output, 0, &AFFINE_BATCH_PLAN_MAGIC_V2)?;
        put(
            output,
            VERSION_OFFSET,
            &AFFINE_BATCH_WIRE_VERSION_V2.to_le_bytes(),
        )?;
        put(output, CALLER_ROLE_OFFSET, &[input.caller_role as u8])?;
        for (offset, value) in [
            (RELEASE_SET_OFFSET, input.release_set),
            (MARKET_OFFSET, input.market),
            (REQUEST_OFFSET, input.request_id),
            (PRODUCT_OFFSET, input.product_record_digest),
            (BASIS_OFFSET, input.semantic_basis_id),
            (LINKED_BASIS_RECORD_OFFSET, input.linked_basis_record_digest),
        ] {
            put(output, offset, &value)?;
        }
        put(
            output,
            MARKET_REVISION_OFFSET,
            &input.expected_market_revision.to_le_bytes(),
        )?;
        put(
            output,
            OUTCOME_COUNT_OFFSET,
            &input.outcome_count.to_le_bytes(),
        )?;
        put(output, POSITION_COUNT_OFFSET, &position_count.to_le_bytes())?;
        put(output, ROW_COUNT_OFFSET, &row_count.to_le_bytes())?;
        for (index, position) in positions.iter().copied().enumerate() {
            let offset = table_offset(
                AFFINE_BATCH_PLAN_HEADER_BYTES_V2,
                index,
                AFFINE_BATCH_POSITION_BYTES_V2,
            )?;
            position.encode_into(slice_mut(output, offset, AFFINE_BATCH_POSITION_BYTES_V2)?)?;
        }
        let rows_offset = AFFINE_BATCH_PLAN_HEADER_BYTES_V2
            .checked_add(table_bytes(position_count, AFFINE_BATCH_POSITION_BYTES_V2)?)
            .ok_or(AffineBatchErrorV2::InvalidLength)?;
        for (index, row) in rows.iter().copied().enumerate() {
            let offset = table_offset(rows_offset, index, AFFINE_BATCH_ROW_BYTES_V2)?;
            row.encode_into(slice_mut(output, offset, AFFINE_BATCH_ROW_BYTES_V2)?)?;
        }
        AffineBatchPlanV2::decode(&*output).map(|_| ())
    }

    fn validate(self) -> Result<()> {
        if self.input.outcome_count == 0
            || self.position_count == 0
            || self.row_count == 0
            || self.input.expected_market_revision == u64::MAX
        {
            return Err(AffineBatchErrorV2::InvalidCount);
        }
        for left in 0..self.position_count {
            let position = self.position(left)?;
            let mut used = false;
            for right in 0..self.position_count {
                if left != right && position.owner() == self.position(right)?.owner() {
                    return Err(AffineBatchErrorV2::InvalidPositionTable);
                }
            }
            for row_index in 0..self.row_count {
                let row = self.row(row_index)?;
                used |= row.source_present() && row.source_position_index() == left;
                used |= row.destination_present() && row.destination_position_index() == left;
            }
            if !used {
                return Err(AffineBatchErrorV2::InvalidPositionTable);
            }
        }
        for left in 0..self.row_count {
            let row = self.row(left)?;
            for right in 0..left {
                if rows_duplicate_coordinate(row, self.row(right)?) {
                    return Err(AffineBatchErrorV2::DuplicateCoordinate);
                }
            }
        }
        Ok(())
    }

    /// Return the selected caller role.
    pub const fn caller_role(self) -> CallerRole {
        self.input.caller_role
    }

    /// Return the current release set.
    pub const fn release_set(self) -> [u8; 32] {
        self.input.release_set
    }

    /// Return the logical Core Market identity.
    pub const fn market(self) -> [u8; 32] {
        self.input.market
    }

    /// Return the caller-owned request identity.
    pub const fn request_id(self) -> [u8; 32] {
        self.input.request_id
    }

    /// Return the finalized Product-record digest.
    pub const fn product_record_digest(self) -> [u8; 32] {
        self.input.product_record_digest
    }

    /// Return the semantic LiabilityBasisV2 identity.
    pub const fn semantic_basis_id(self) -> [u8; 32] {
        self.input.semantic_basis_id
    }

    /// Return the finalized linked-basis raw-record digest.
    pub const fn linked_basis_record_digest(self) -> [u8; 32] {
        self.input.linked_basis_record_digest
    }

    /// Return the optimistic aggregate pre-revision.
    pub const fn expected_market_revision(self) -> u64 {
        self.input.expected_market_revision
    }

    /// Return the runtime outcome width.
    pub const fn outcome_count(self) -> u32 {
        self.input.outcome_count
    }

    /// Return the number of unique Position entries.
    pub const fn position_count(self) -> u32 {
        self.position_count
    }

    /// Return the number of ordered affine rows.
    pub const fn row_count(self) -> u32 {
        self.row_count
    }

    /// Decode one indexed Position-table entry.
    pub fn position(self, index: u32) -> Result<AffineBatchPositionV2> {
        let offset = indexed_offset(index, self.position_count, AFFINE_BATCH_POSITION_BYTES_V2)?;
        AffineBatchPositionV2::decode(slice(
            self.positions,
            offset,
            AFFINE_BATCH_POSITION_BYTES_V2,
        )?)
    }

    /// Decode one indexed ordered row.
    pub fn row(self, index: u32) -> Result<AffineBatchRowV2> {
        let offset = indexed_offset(index, self.row_count, AFFINE_BATCH_ROW_BYTES_V2)?;
        AffineBatchRowV2::decode(
            slice(self.rows, offset, AFFINE_BATCH_ROW_BYTES_V2)?,
            self.input.outcome_count,
            self.position_count,
        )
    }

    /// Borrow the exact Position-table and ordered-row bytes committed by the plan.
    pub const fn table_bytes(self) -> (&'a [u8], &'a [u8]) {
        (self.positions, self.rows)
    }
}

/// Exact fixed receipt for one committed affine batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AffineBatchReceiptV2 {
    caller_role: CallerRole,
    release_set: [u8; 32],
    market: [u8; 32],
    request_id: [u8; 32],
    product_record_digest: [u8; 32],
    semantic_basis_id: [u8; 32],
    linked_basis_record_digest: [u8; 32],
    packet_digest: [u8; 32],
    table_digest: [u8; 32],
    claims_program: [u8; 32],
    post_resource_digest: [u8; 32],
    pre_market_revision: u64,
    post_market_revision: u64,
    outcome_count: u32,
    position_count: u32,
    row_count: u32,
}

impl AffineBatchReceiptV2 {
    /// Construct a receipt whose plan-bound aggregate revision advanced once.
    pub fn new(
        plan: AffineBatchPlanV2<'_>,
        packet_digest: [u8; 32],
        table_digest: [u8; 32],
        claims_program: [u8; 32],
        post_resource_digest: [u8; 32],
        post_market_revision: u64,
    ) -> Result<Self> {
        for value in [
            packet_digest,
            table_digest,
            claims_program,
            post_resource_digest,
        ] {
            if is_zero(value) {
                return Err(AffineBatchErrorV2::ZeroIdentity);
            }
        }
        let expected_post = plan
            .expected_market_revision()
            .checked_add(1)
            .ok_or(AffineBatchErrorV2::InvalidRevision)?;
        if post_market_revision != expected_post {
            return Err(AffineBatchErrorV2::InvalidRevision);
        }
        Ok(Self {
            caller_role: plan.caller_role(),
            release_set: plan.release_set(),
            market: plan.market(),
            request_id: plan.request_id(),
            product_record_digest: plan.product_record_digest(),
            semantic_basis_id: plan.semantic_basis_id(),
            linked_basis_record_digest: plan.linked_basis_record_digest(),
            packet_digest,
            table_digest,
            claims_program,
            post_resource_digest,
            pre_market_revision: plan.expected_market_revision(),
            post_market_revision,
            outcome_count: plan.outcome_count(),
            position_count: plan.position_count(),
            row_count: plan.row_count(),
        })
    }

    /// Decode one exact canonical receipt.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != AFFINE_BATCH_RECEIPT_BYTES_V2 {
            return Err(AffineBatchErrorV2::InvalidLength);
        }
        exact(input, 0, &AFFINE_BATCH_RECEIPT_MAGIC_V2)?;
        if u16_at(input, VERSION_OFFSET)? != AFFINE_BATCH_WIRE_VERSION_V2 {
            return Err(AffineBatchErrorV2::UnsupportedVersion);
        }
        require_zero(input, HEADER_RESERVED_OFFSET, 5)?;
        require_zero(input, RECEIPT_RESERVED_OFFSET, 12)?;
        let value = Self {
            caller_role: decode_role(byte_at(input, CALLER_ROLE_OFFSET)?)?,
            release_set: nonzero_array(input, RELEASE_SET_OFFSET)?,
            market: nonzero_array(input, MARKET_OFFSET)?,
            request_id: nonzero_array(input, REQUEST_OFFSET)?,
            product_record_digest: nonzero_array(input, PRODUCT_OFFSET)?,
            semantic_basis_id: nonzero_array(input, BASIS_OFFSET)?,
            linked_basis_record_digest: nonzero_array(input, LINKED_BASIS_RECORD_OFFSET)?,
            packet_digest: nonzero_array(input, RECEIPT_PACKET_DIGEST_OFFSET)?,
            table_digest: nonzero_array(input, RECEIPT_TABLE_DIGEST_OFFSET)?,
            claims_program: nonzero_array(input, RECEIPT_CLAIMS_PROGRAM_OFFSET)?,
            post_resource_digest: nonzero_array(input, RECEIPT_RESOURCE_DIGEST_OFFSET)?,
            pre_market_revision: u64_at(input, RECEIPT_PRE_MARKET_REVISION_OFFSET)?,
            post_market_revision: u64_at(input, RECEIPT_POST_MARKET_REVISION_OFFSET)?,
            outcome_count: u32_at(input, RECEIPT_OUTCOME_COUNT_OFFSET)?,
            position_count: u32_at(input, RECEIPT_POSITION_COUNT_OFFSET)?,
            row_count: u32_at(input, RECEIPT_ROW_COUNT_OFFSET)?,
        };
        if value.pre_market_revision.checked_add(1) != Some(value.post_market_revision)
            || value.outcome_count == 0
            || value.position_count == 0
            || value.row_count == 0
        {
            return Err(AffineBatchErrorV2::InvalidRevision);
        }
        Ok(value)
    }

    /// Encode the exact fixed receipt bytes.
    pub fn to_bytes(self) -> [u8; AFFINE_BATCH_RECEIPT_BYTES_V2] {
        let mut output = [0_u8; AFFINE_BATCH_RECEIPT_BYTES_V2];
        put_infallible(&mut output, 0, &AFFINE_BATCH_RECEIPT_MAGIC_V2);
        put_infallible(
            &mut output,
            VERSION_OFFSET,
            &AFFINE_BATCH_WIRE_VERSION_V2.to_le_bytes(),
        );
        put_infallible(&mut output, CALLER_ROLE_OFFSET, &[self.caller_role as u8]);
        for (offset, value) in [
            (RELEASE_SET_OFFSET, self.release_set),
            (MARKET_OFFSET, self.market),
            (REQUEST_OFFSET, self.request_id),
            (PRODUCT_OFFSET, self.product_record_digest),
            (BASIS_OFFSET, self.semantic_basis_id),
            (LINKED_BASIS_RECORD_OFFSET, self.linked_basis_record_digest),
            (RECEIPT_PACKET_DIGEST_OFFSET, self.packet_digest),
            (RECEIPT_TABLE_DIGEST_OFFSET, self.table_digest),
            (RECEIPT_CLAIMS_PROGRAM_OFFSET, self.claims_program),
            (RECEIPT_RESOURCE_DIGEST_OFFSET, self.post_resource_digest),
        ] {
            put_infallible(&mut output, offset, &value);
        }
        put_infallible(
            &mut output,
            RECEIPT_PRE_MARKET_REVISION_OFFSET,
            &self.pre_market_revision.to_le_bytes(),
        );
        put_infallible(
            &mut output,
            RECEIPT_POST_MARKET_REVISION_OFFSET,
            &self.post_market_revision.to_le_bytes(),
        );
        put_infallible(
            &mut output,
            RECEIPT_OUTCOME_COUNT_OFFSET,
            &self.outcome_count.to_le_bytes(),
        );
        put_infallible(
            &mut output,
            RECEIPT_POSITION_COUNT_OFFSET,
            &self.position_count.to_le_bytes(),
        );
        put_infallible(
            &mut output,
            RECEIPT_ROW_COUNT_OFFSET,
            &self.row_count.to_le_bytes(),
        );
        output
    }

    /// Require every plan-owned coordinate to agree with this receipt.
    pub fn validate_plan(self, plan: AffineBatchPlanV2<'_>) -> Result<()> {
        if self.caller_role != plan.caller_role()
            || self.release_set != plan.release_set()
            || self.market != plan.market()
            || self.request_id != plan.request_id()
            || self.product_record_digest != plan.product_record_digest()
            || self.semantic_basis_id != plan.semantic_basis_id()
            || self.linked_basis_record_digest != plan.linked_basis_record_digest()
            || self.pre_market_revision != plan.expected_market_revision()
            || Some(self.post_market_revision) != plan.expected_market_revision().checked_add(1)
            || self.outcome_count != plan.outcome_count()
            || self.position_count != plan.position_count()
            || self.row_count != plan.row_count()
        {
            return Err(AffineBatchErrorV2::ReceiptMismatch);
        }
        Ok(())
    }

    /// Return the exact packet digest, which commits the ordered rows.
    pub const fn packet_digest(self) -> [u8; 32] {
        self.packet_digest
    }

    /// Return the digest of the unique Position table followed by ordered rows.
    pub const fn table_digest(self) -> [u8; 32] {
        self.table_digest
    }

    /// Return the selected Claims program that produced the receipt.
    pub const fn claims_program(self) -> [u8; 32] {
        self.claims_program
    }

    /// Return the digest of aggregate bytes followed by the ordered post-Position table.
    pub const fn post_resource_digest(self) -> [u8; 32] {
        self.post_resource_digest
    }

    /// Return the optimistic aggregate pre-revision.
    pub const fn pre_market_revision(self) -> u64 {
        self.pre_market_revision
    }

    /// Return the committed aggregate post-revision.
    pub const fn post_market_revision(self) -> u64 {
        self.post_market_revision
    }
}

/// Return the exact packet width for runtime Position and row counts.
pub fn plan_bytes(position_count: u32, row_count: u32) -> Result<usize> {
    if position_count == 0 || row_count == 0 {
        return Err(AffineBatchErrorV2::InvalidCount);
    }
    AFFINE_BATCH_PLAN_HEADER_BYTES_V2
        .checked_add(table_bytes(position_count, AFFINE_BATCH_POSITION_BYTES_V2)?)
        .and_then(|width| {
            table_bytes(row_count, AFFINE_BATCH_ROW_BYTES_V2)
                .ok()
                .and_then(|rows| width.checked_add(rows))
        })
        .ok_or(AffineBatchErrorV2::InvalidLength)
}

fn validate_endpoint(
    present: bool,
    index: u32,
    delta: SignedMagnitudeV2,
    position_count: u32,
    required_direction: DeltaDirectionV2,
) -> Result<()> {
    if present {
        if index >= position_count
            || delta.direction() != required_direction
            || delta.magnitude() == 0
        {
            return Err(AffineBatchErrorV2::InvalidDelta);
        }
    } else if index != 0 || delta.direction() != DeltaDirectionV2::Neutral || delta.magnitude() != 0
    {
        return Err(AffineBatchErrorV2::NonCanonical);
    }
    Ok(())
}

fn rows_duplicate_coordinate(left: AffineBatchRowV2, right: AffineBatchRowV2) -> bool {
    (left.source_present()
        && ((right.source_present()
            && left.source_position_index() == right.source_position_index())
            || (right.destination_present()
                && left.source_position_index() == right.destination_position_index()))
        && left.outcome() == right.outcome())
        || (left.destination_present()
            && ((right.source_present()
                && left.destination_position_index() == right.source_position_index())
                || (right.destination_present()
                    && left.destination_position_index() == right.destination_position_index()))
            && left.outcome() == right.outcome())
}

fn decode_role(value: u8) -> Result<CallerRole> {
    match value {
        0 => Ok(CallerRole::Core),
        2 => Ok(CallerRole::Trading),
        _ => Err(AffineBatchErrorV2::UnknownTag),
    }
}

fn bool_at(input: &[u8], offset: usize) -> Result<bool> {
    match byte_at(input, offset)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(AffineBatchErrorV2::UnknownTag),
    }
}

fn table_bytes(count: u32, element_bytes: usize) -> Result<usize> {
    usize::try_from(count)
        .ok()
        .and_then(|count| count.checked_mul(element_bytes))
        .ok_or(AffineBatchErrorV2::InvalidLength)
}

fn indexed_offset(index: u32, count: u32, element_bytes: usize) -> Result<usize> {
    if index >= count {
        return Err(AffineBatchErrorV2::InvalidIndex);
    }
    usize::try_from(index)
        .ok()
        .and_then(|index| index.checked_mul(element_bytes))
        .ok_or(AffineBatchErrorV2::InvalidIndex)
}

fn table_offset(base: usize, index: usize, element_bytes: usize) -> Result<usize> {
    index
        .checked_mul(element_bytes)
        .and_then(|offset| base.checked_add(offset))
        .ok_or(AffineBatchErrorV2::InvalidLength)
}

fn add(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right)
        .ok_or(AffineBatchErrorV2::InvalidLength)
}

fn exact(input: &[u8], offset: usize, expected: &[u8]) -> Result<()> {
    if slice(input, offset, expected.len())? != expected {
        return Err(AffineBatchErrorV2::InvalidMagic);
    }
    Ok(())
}

fn require_zero(input: &[u8], offset: usize, length: usize) -> Result<()> {
    if slice(input, offset, length)?.iter().any(|byte| *byte != 0) {
        return Err(AffineBatchErrorV2::NonCanonical);
    }
    Ok(())
}

fn byte_at(input: &[u8], offset: usize) -> Result<u8> {
    input
        .get(offset)
        .copied()
        .ok_or(AffineBatchErrorV2::InvalidLength)
}

fn u16_at(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array_at(input, offset)?))
}

fn u32_at(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(array_at(input, offset)?))
}

fn u64_at(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(array_at(input, offset)?))
}

fn nonzero_array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    let value = array_at(input, offset)?;
    if value.iter().all(|byte| *byte == 0) {
        return Err(AffineBatchErrorV2::ZeroIdentity);
    }
    Ok(value)
}

fn array_at<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    slice(input, offset, N)?
        .try_into()
        .map_err(|_| AffineBatchErrorV2::InvalidLength)
}

fn is_zero<const N: usize>(value: [u8; N]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn slice(input: &[u8], offset: usize, length: usize) -> Result<&[u8]> {
    input
        .get(
            offset
                ..offset
                    .checked_add(length)
                    .ok_or(AffineBatchErrorV2::InvalidLength)?,
        )
        .ok_or(AffineBatchErrorV2::InvalidLength)
}

fn slice_mut(input: &mut [u8], offset: usize, length: usize) -> Result<&mut [u8]> {
    input
        .get_mut(
            offset
                ..offset
                    .checked_add(length)
                    .ok_or(AffineBatchErrorV2::InvalidLength)?,
        )
        .ok_or(AffineBatchErrorV2::InvalidLength)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let destination = slice_mut(output, offset, value.len())?;
    destination.copy_from_slice(value);
    Ok(())
}

fn put_infallible(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(destination) = output.get_mut(offset..offset.saturating_add(value.len())) {
        destination.copy_from_slice(value);
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec;

    use super::*;

    fn credit(value: u64) -> SignedMagnitudeV2 {
        SignedMagnitudeV2::new(DeltaDirectionV2::Credit, value).expect("credit")
    }

    fn debit(value: u64) -> SignedMagnitudeV2 {
        SignedMagnitudeV2::new(DeltaDirectionV2::Debit, value).expect("debit")
    }

    fn neutral() -> SignedMagnitudeV2 {
        SignedMagnitudeV2::new(DeltaDirectionV2::Neutral, 0).expect("neutral")
    }

    fn header(outcome_count: u32) -> AffineBatchPlanInputV2 {
        AffineBatchPlanInputV2 {
            caller_role: CallerRole::Trading,
            release_set: [1; 32],
            market: [2; 32],
            request_id: [3; 32],
            product_record_digest: [4; 32],
            semantic_basis_id: [5; 32],
            linked_basis_record_digest: [6; 32],
            expected_market_revision: 7,
            outcome_count,
        }
    }

    fn split_row(index: u32, outcome: u32, quantity: u64, width: u32) -> AffineBatchRowV2 {
        AffineBatchRowV2::new(
            AffineBatchRowInputV2 {
                source_present: false,
                destination_present: true,
                outcome,
                source_position_index: 0,
                destination_position_index: index,
                aggregate_delta: credit(quantity),
                source_delta: neutral(),
                destination_delta: credit(quantity),
            },
            width,
            width,
        )
        .expect("split row")
    }

    #[test]
    fn runtime_width_258_round_trips_without_padding() {
        let width = 258_u32;
        let positions: vec::Vec<_> = (0..width)
            .map(|index| {
                let mut owner = [0_u8; 32];
                owner[..4].copy_from_slice(&(index + 1).to_le_bytes());
                AffineBatchPositionV2::new(owner, u64::from(index)).expect("Position")
            })
            .collect();
        let rows: vec::Vec<_> = (0..width)
            .map(|index| split_row(index, index, u64::MAX, width))
            .collect();
        let mut bytes = vec![0_u8; plan_bytes(width, width).expect("width")];
        AffineBatchPlanV2::encode_into(header(width), &positions, &rows, &mut bytes)
            .expect("encode");
        let plan = AffineBatchPlanV2::decode(&bytes).expect("decode");
        assert_eq!(plan.position_count(), width);
        assert_eq!(plan.row_count(), width);
        assert_eq!(
            plan.row(257).expect("last").aggregate_delta().magnitude(),
            u64::MAX
        );
        assert_eq!(bytes.len(), 240 + 258 * 40 + 258 * 64);
    }

    #[test]
    fn public_request_layout_tracks_header_position_and_row_encoders() {
        let header = header(1);
        let position = AffineBatchPositionV2::new([8; 32], 9).expect("Position");
        let row = split_row(0, 0, 10, 1);
        let mut bytes = vec![0_u8; plan_bytes(1, 1).expect("width")];
        AffineBatchPlanV2::encode_into(header, &[position], &[row], &mut bytes).expect("encode");
        for (offset, expected) in [
            (AffineBatchRequestLayoutV2::RELEASE_SET, header.release_set),
            (AffineBatchRequestLayoutV2::MARKET, header.market),
            (
                AffineBatchRequestLayoutV2::REQUEST_DIGEST,
                header.request_id,
            ),
            (
                AffineBatchRequestLayoutV2::PRODUCT_RECORD,
                header.product_record_digest,
            ),
            (
                AffineBatchRequestLayoutV2::SEMANTIC_BASIS,
                header.semantic_basis_id,
            ),
            (
                AffineBatchRequestLayoutV2::LINKED_BASIS_RECORD,
                header.linked_basis_record_digest,
            ),
        ] {
            assert_eq!(bytes.get(offset..offset + 32), Some(expected.as_slice()));
        }
        assert_eq!(
            bytes.get(
                AffineBatchRequestLayoutV2::EXPECTED_MARKET_REVISION
                    ..AffineBatchRequestLayoutV2::EXPECTED_MARKET_REVISION + 8
            ),
            Some(header.expected_market_revision.to_le_bytes().as_slice())
        );
        assert_eq!(
            bytes.get(
                AffineBatchRequestLayoutV2::OUTCOME_COUNT
                    ..AffineBatchRequestLayoutV2::OUTCOME_COUNT + 4
            ),
            Some(header.outcome_count.to_le_bytes().as_slice())
        );
        assert_eq!(
            bytes.get(
                AffineBatchRequestLayoutV2::ROW_COUNT..AffineBatchRequestLayoutV2::ROW_COUNT + 4
            ),
            Some(1_u32.to_le_bytes().as_slice())
        );
        let owner = AffineBatchRequestLayoutV2::position_field(
            0,
            AffineBatchRequestLayoutV2::POSITION_OWNER,
        )
        .expect("owner offset");
        let revision = AffineBatchRequestLayoutV2::position_field(
            0,
            AffineBatchRequestLayoutV2::POSITION_REVISION,
        )
        .expect("revision offset");
        assert_eq!(
            bytes.get(owner..owner + 32),
            Some(position.owner().as_slice())
        );
        assert_eq!(
            bytes.get(revision..revision + 8),
            Some(position.expected_revision().to_le_bytes().as_slice())
        );
        let row_base = AffineBatchRequestLayoutV2::POSITION_TABLE
            + AffineBatchRequestLayoutV2::POSITION_STRIDE;
        assert_eq!(
            bytes.get(
                row_base + AffineBatchRequestLayoutV2::ROW_OUTCOME
                    ..row_base + AffineBatchRequestLayoutV2::ROW_OUTCOME + 4
            ),
            Some(row.outcome().to_le_bytes().as_slice())
        );
        assert_eq!(
            bytes.get(
                row_base + AffineBatchRequestLayoutV2::ROW_DESTINATION_MAGNITUDE
                    ..row_base + AffineBatchRequestLayoutV2::ROW_DESTINATION_MAGNITUDE + 8
            ),
            Some(row.destination_delta().magnitude().to_le_bytes().as_slice())
        );
        assert!(AffineBatchPlanV2::decode(&bytes).is_ok());
    }

    #[test]
    fn one_position_can_span_distinct_outcomes_but_not_one_twice() {
        let position = [AffineBatchPositionV2::new([9; 32], 3).expect("Position")];
        let rows = [split_row(0, 0, 7, 2), split_row(0, 1, 8, 2)];
        let mut bytes = vec![0_u8; plan_bytes(1, 2).expect("width")];
        AffineBatchPlanV2::encode_into(header(2), &position, &rows, &mut bytes)
            .expect("distinct outcomes");
        let duplicate = [split_row(0, 0, 7, 2), split_row(0, 0, 8, 2)];
        assert_eq!(
            AffineBatchPlanV2::encode_into(header(2), &position, &duplicate, &mut bytes),
            Err(AffineBatchErrorV2::DuplicateCoordinate)
        );
    }

    #[test]
    fn transfer_and_merge_conserve_and_hostile_shapes_refuse() {
        let transfer = AffineBatchRowV2::new(
            AffineBatchRowInputV2 {
                source_present: true,
                destination_present: true,
                outcome: 1,
                source_position_index: 0,
                destination_position_index: 1,
                aggregate_delta: neutral(),
                source_delta: debit(u64::MAX),
                destination_delta: credit(u64::MAX),
            },
            2,
            2,
        );
        assert!(transfer.is_ok());
        let merge = AffineBatchRowV2::new(
            AffineBatchRowInputV2 {
                source_present: true,
                destination_present: false,
                outcome: 0,
                source_position_index: 0,
                destination_position_index: 0,
                aggregate_delta: debit(u64::MAX),
                source_delta: debit(u64::MAX),
                destination_delta: neutral(),
            },
            2,
            2,
        );
        assert!(merge.is_ok());
        assert_eq!(
            SignedMagnitudeV2::new(DeltaDirectionV2::Debit, 0),
            Err(AffineBatchErrorV2::InvalidDelta)
        );
        let nonconserving = AffineBatchRowV2::new(
            AffineBatchRowInputV2 {
                source_present: true,
                destination_present: true,
                outcome: 0,
                source_position_index: 0,
                destination_position_index: 1,
                aggregate_delta: credit(1),
                source_delta: debit(7),
                destination_delta: credit(7),
            },
            2,
            2,
        );
        assert_eq!(nonconserving, Err(AffineBatchErrorV2::InvalidDelta));
    }

    #[test]
    fn duplicate_owner_inactive_fields_and_receipt_substitution_refuse() {
        let positions = [
            AffineBatchPositionV2::new([9; 32], 3).expect("Position"),
            AffineBatchPositionV2::new([9; 32], 4).expect("duplicate Position"),
        ];
        let rows = [split_row(0, 0, 7, 2), split_row(1, 1, 7, 2)];
        let mut bytes = vec![0_u8; plan_bytes(2, 2).expect("width")];
        assert_eq!(
            AffineBatchPlanV2::encode_into(header(2), &positions, &rows, &mut bytes),
            Err(AffineBatchErrorV2::InvalidPositionTable)
        );

        let positions = [AffineBatchPositionV2::new([8; 32], 3).expect("Position")];
        let rows = [split_row(0, 0, 7, 1)];
        let mut bytes = vec![0_u8; plan_bytes(1, 1).expect("width")];
        AffineBatchPlanV2::encode_into(header(1), &positions, &rows, &mut bytes).expect("encode");
        let rows_offset = AFFINE_BATCH_PLAN_HEADER_BYTES_V2 + AFFINE_BATCH_POSITION_BYTES_V2;
        *bytes
            .get_mut(rows_offset + ROW_SOURCE_INDEX_OFFSET)
            .expect("inactive index") = 1;
        assert_eq!(
            AffineBatchPlanV2::decode(&bytes),
            Err(AffineBatchErrorV2::NonCanonical)
        );

        *bytes
            .get_mut(rows_offset + ROW_SOURCE_INDEX_OFFSET)
            .expect("restore") = 0;
        let plan = AffineBatchPlanV2::decode(&bytes).expect("plan");
        let receipt = AffineBatchReceiptV2::new(plan, [6; 32], [7; 32], [8; 32], [9; 32], 8)
            .expect("receipt");
        assert_eq!(
            AffineBatchReceiptV2::decode(&receipt.to_bytes()),
            Ok(receipt)
        );
        let mut hostile_header = header(1);
        hostile_header.semantic_basis_id = [10; 32];
        AffineBatchPlanV2::encode_into(hostile_header, &positions, &rows, &mut bytes)
            .expect("hostile plan");
        assert_eq!(
            receipt.validate_plan(AffineBatchPlanV2::decode(&bytes).expect("decode")),
            Err(AffineBatchErrorV2::ReceiptMismatch)
        );
    }
}
