//! Family-neutral runtime-width signed-delta Claims batch ABI.
//!
//! One packet carries a canonical table of unique Positions, one exact signed
//! aggregate-supply delta for every runtime outcome, and one already-netted
//! signed delta for each touched `(Position, outcome)` coordinate. Conservation
//! is checked over the whole batch, so split, transfer, and merge effects may
//! coalesce without introducing family-specific rows or duplicate coordinates.

use super::CallerRole;

/// Bytes in the fixed plan header before runtime tables.
pub const SIGNED_DELTA_PLAN_HEADER_BYTES_V3: usize = 240;
/// Bytes in one unique Position-table entry.
pub const SIGNED_DELTA_POSITION_BYTES_V3: usize = 40;
/// Bytes in one signed aggregate or Position delta.
pub const SIGNED_DELTA_BYTES_V3: usize = 16;
/// Bytes in one unique `(Position, outcome)` delta row.
pub const SIGNED_DELTA_ROW_BYTES_V3: usize = 24;
/// Bytes in one fixed success receipt.
pub const SIGNED_DELTA_RECEIPT_BYTES_V3: usize = 376;
/// Canonical plan magic.
pub const SIGNED_DELTA_PLAN_MAGIC_V3: [u8; 8] = *b"DCLSDP03";
/// Canonical receipt magic.
pub const SIGNED_DELTA_RECEIPT_MAGIC_V3: [u8; 8] = *b"DCLSDR03";
/// Implemented wire version.
pub const SIGNED_DELTA_WIRE_VERSION_V3: u16 = 3;

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
const CLAIM_COUNT_OFFSET: usize = 216;
const POSITION_COUNT_OFFSET: usize = 220;
const POSITION_DELTA_COUNT_OFFSET: usize = 224;
const HEADER_TAIL_RESERVED_OFFSET: usize = 228;

const POSITION_OWNER_OFFSET: usize = 0;
const POSITION_REVISION_OFFSET: usize = 32;

const ROW_POSITION_INDEX_OFFSET: usize = 0;
const ROW_OUTCOME_OFFSET: usize = 4;
const ROW_DELTA_OFFSET: usize = 8;

const DELTA_DIRECTION_OFFSET: usize = 0;
const DELTA_RESERVED_OFFSET: usize = 1;
const DELTA_MAGNITUDE_OFFSET: usize = 8;

const RECEIPT_PACKET_DIGEST_OFFSET: usize = 208;
const RECEIPT_TABLE_DIGEST_OFFSET: usize = 240;
const RECEIPT_CLAIMS_PROGRAM_OFFSET: usize = 272;
const RECEIPT_RESOURCE_DIGEST_OFFSET: usize = 304;
const RECEIPT_PRE_MARKET_REVISION_OFFSET: usize = 336;
const RECEIPT_POST_MARKET_REVISION_OFFSET: usize = 344;
const RECEIPT_CLAIM_COUNT_OFFSET: usize = 352;
const RECEIPT_POSITION_COUNT_OFFSET: usize = 356;
const RECEIPT_POSITION_DELTA_COUNT_OFFSET: usize = 360;
const RECEIPT_RESERVED_OFFSET: usize = 364;

/// Stable signed-delta ABI refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignedDeltaErrorV3 {
    /// A packet or table had the wrong exact width.
    InvalidLength,
    /// Magic bytes selected another packet family.
    InvalidMagic,
    /// The wire version is unsupported.
    UnsupportedVersion,
    /// Reserved bytes or a zero delta were not canonical.
    NonCanonical,
    /// A required identity was zero.
    ZeroIdentity,
    /// A role or direction tag was unknown.
    UnknownTag,
    /// A runtime count was zero or overflowed address arithmetic.
    InvalidCount,
    /// Position owners were not strictly ordered or an entry was unused.
    InvalidPositionTable,
    /// A Position index or outcome was outside its runtime table.
    InvalidIndex,
    /// Position-delta coordinates were duplicated or not strictly ordered.
    InvalidCoordinateOrder,
    /// Position deltas did not conserve exactly against the aggregate delta.
    Conservation,
    /// An exact signed total overflowed or did not fit the canonical magnitude.
    Arithmetic,
    /// An optimistic revision could not advance exactly once.
    InvalidRevision,
    /// Receipt facts did not agree with their committed plan.
    ReceiptMismatch,
}

/// Result alias for the signed-delta ABI.
pub type Result<T> = core::result::Result<T, SignedDeltaErrorV3>;

/// Canonical direction for one full-range exact magnitude.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DeltaDirectionV3 {
    /// The sole canonical direction for magnitude zero.
    Neutral = 0,
    /// Add the magnitude to the resource.
    Credit = 1,
    /// Subtract the magnitude from the resource.
    Debit = 2,
}

impl DeltaDirectionV3 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Neutral),
            1 => Ok(Self::Credit),
            2 => Ok(Self::Debit),
            _ => Err(SignedDeltaErrorV3::UnknownTag),
        }
    }
}

/// One canonical signed-magnitude delta over the full `u64` range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignedDeltaV3 {
    direction: DeltaDirectionV3,
    magnitude: u64,
}

impl SignedDeltaV3 {
    /// Construct one canonical exact delta.
    pub const fn new(direction: DeltaDirectionV3, magnitude: u64) -> Result<Self> {
        if (magnitude == 0) != matches!(direction, DeltaDirectionV3::Neutral) {
            return Err(SignedDeltaErrorV3::NonCanonical);
        }
        Ok(Self {
            direction,
            magnitude,
        })
    }

    /// Return the exact direction.
    pub const fn direction(self) -> DeltaDirectionV3 {
        self.direction
    }

    /// Return the exact unsigned magnitude.
    pub const fn magnitude(self) -> u64 {
        self.magnitude
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != SIGNED_DELTA_BYTES_V3 {
            return Err(SignedDeltaErrorV3::InvalidLength);
        }
        require_zero(bytes, DELTA_RESERVED_OFFSET, 7)?;
        Self::new(
            DeltaDirectionV3::decode(byte_at(bytes, DELTA_DIRECTION_OFFSET)?)?,
            u64_at(bytes, DELTA_MAGNITUDE_OFFSET)?,
        )
    }

    fn encode_into(self, output: &mut [u8]) -> Result<()> {
        if output.len() != SIGNED_DELTA_BYTES_V3 {
            return Err(SignedDeltaErrorV3::InvalidLength);
        }
        put(output, DELTA_DIRECTION_OFFSET, &[self.direction as u8])?;
        put(
            output,
            DELTA_MAGNITUDE_OFFSET,
            &self.magnitude.to_le_bytes(),
        )
    }
}

/// One unique Position entry that advances once with the batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignedDeltaPositionV3 {
    owner: [u8; 32],
    expected_revision: u64,
}

impl SignedDeltaPositionV3 {
    /// Construct one canonical Position coordinate.
    pub fn new(owner: [u8; 32], expected_revision: u64) -> Result<Self> {
        if is_zero(owner) {
            return Err(SignedDeltaErrorV3::ZeroIdentity);
        }
        if expected_revision == u64::MAX {
            return Err(SignedDeltaErrorV3::InvalidRevision);
        }
        Ok(Self {
            owner,
            expected_revision,
        })
    }

    /// Return the Position owner identity.
    pub const fn owner(self) -> [u8; 32] {
        self.owner
    }

    /// Return the optimistic pre-revision.
    pub const fn expected_revision(self) -> u64 {
        self.expected_revision
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != SIGNED_DELTA_POSITION_BYTES_V3 {
            return Err(SignedDeltaErrorV3::InvalidLength);
        }
        Self::new(
            array_at(bytes, POSITION_OWNER_OFFSET)?,
            u64_at(bytes, POSITION_REVISION_OFFSET)?,
        )
    }

    fn encode_into(self, output: &mut [u8]) -> Result<()> {
        if output.len() != SIGNED_DELTA_POSITION_BYTES_V3 {
            return Err(SignedDeltaErrorV3::InvalidLength);
        }
        put(output, POSITION_OWNER_OFFSET, &self.owner)?;
        put(
            output,
            POSITION_REVISION_OFFSET,
            &self.expected_revision.to_le_bytes(),
        )
    }
}

/// Construction input for one already-netted Position delta.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionDeltaInputV3 {
    /// Index into the packet's canonical Position table.
    pub position_index: u32,
    /// Runtime outcome coordinate.
    pub outcome: u32,
    /// Exact nonzero net delta at the coordinate.
    pub delta: SignedDeltaV3,
}

/// One canonical unique `(Position, outcome)` delta.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionDeltaV3(PositionDeltaInputV3);

impl PositionDeltaV3 {
    /// Construct one bounds-checked, nonzero coordinate delta.
    pub fn new(input: PositionDeltaInputV3, position_count: u32, claim_count: u32) -> Result<Self> {
        if input.position_index >= position_count || input.outcome >= claim_count {
            return Err(SignedDeltaErrorV3::InvalidIndex);
        }
        if input.delta.direction() == DeltaDirectionV3::Neutral || input.delta.magnitude() == 0 {
            return Err(SignedDeltaErrorV3::NonCanonical);
        }
        Ok(Self(input))
    }

    /// Return the Position-table index.
    pub const fn position_index(self) -> u32 {
        self.0.position_index
    }

    /// Return the runtime outcome coordinate.
    pub const fn outcome(self) -> u32 {
        self.0.outcome
    }

    /// Return the exact already-netted delta.
    pub const fn delta(self) -> SignedDeltaV3 {
        self.0.delta
    }

    fn decode(bytes: &[u8], position_count: u32, claim_count: u32) -> Result<Self> {
        if bytes.len() != SIGNED_DELTA_ROW_BYTES_V3 {
            return Err(SignedDeltaErrorV3::InvalidLength);
        }
        Self::new(
            PositionDeltaInputV3 {
                position_index: u32_at(bytes, ROW_POSITION_INDEX_OFFSET)?,
                outcome: u32_at(bytes, ROW_OUTCOME_OFFSET)?,
                delta: SignedDeltaV3::decode(slice(
                    bytes,
                    ROW_DELTA_OFFSET,
                    SIGNED_DELTA_BYTES_V3,
                )?)?,
            },
            position_count,
            claim_count,
        )
    }

    fn encode_into(self, output: &mut [u8]) -> Result<()> {
        if output.len() != SIGNED_DELTA_ROW_BYTES_V3 {
            return Err(SignedDeltaErrorV3::InvalidLength);
        }
        put(
            output,
            ROW_POSITION_INDEX_OFFSET,
            &self.0.position_index.to_le_bytes(),
        )?;
        put(output, ROW_OUTCOME_OFFSET, &self.0.outcome.to_le_bytes())?;
        self.0
            .delta
            .encode_into(slice_mut(output, ROW_DELTA_OFFSET, SIGNED_DELTA_BYTES_V3)?)
    }
}

/// Immutable header facts used to construct one signed-delta batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignedDeltaPlanInputV3 {
    /// Registry role of the selected caller program.
    pub caller_role: CallerRole,
    /// Immutable current execution release set.
    pub release_set: [u8; 32],
    /// Canonical logical Core Market identity.
    pub market: [u8; 32],
    /// Caller-owned nonzero request identity.
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
    pub claim_count: u32,
}

/// Borrowed, exact-width family-neutral signed-delta plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignedDeltaPlanV3<'a> {
    input: SignedDeltaPlanInputV3,
    position_count: u32,
    position_delta_count: u32,
    positions: &'a [u8],
    aggregate_deltas: &'a [u8],
    position_deltas: &'a [u8],
}

impl<'a> SignedDeltaPlanV3<'a> {
    /// Decode and fully canonicalize one hostile packet without allocation.
    pub fn decode(input: &'a [u8]) -> Result<Self> {
        if input.len() < SIGNED_DELTA_PLAN_HEADER_BYTES_V3 {
            return Err(SignedDeltaErrorV3::InvalidLength);
        }
        exact(input, 0, &SIGNED_DELTA_PLAN_MAGIC_V3)?;
        if u16_at(input, VERSION_OFFSET)? != SIGNED_DELTA_WIRE_VERSION_V3 {
            return Err(SignedDeltaErrorV3::UnsupportedVersion);
        }
        require_zero(input, HEADER_RESERVED_OFFSET, 5)?;
        require_zero(input, HEADER_TAIL_RESERVED_OFFSET, 12)?;
        let claim_count = u32_at(input, CLAIM_COUNT_OFFSET)?;
        let position_count = u32_at(input, POSITION_COUNT_OFFSET)?;
        let position_delta_count = u32_at(input, POSITION_DELTA_COUNT_OFFSET)?;
        let positions_bytes = table_bytes(position_count, SIGNED_DELTA_POSITION_BYTES_V3)?;
        let aggregate_bytes = table_bytes(claim_count, SIGNED_DELTA_BYTES_V3)?;
        let position_delta_bytes = table_bytes(position_delta_count, SIGNED_DELTA_ROW_BYTES_V3)?;
        let aggregate_offset = add(SIGNED_DELTA_PLAN_HEADER_BYTES_V3, positions_bytes)?;
        let position_delta_offset = add(aggregate_offset, aggregate_bytes)?;
        let expected = add(position_delta_offset, position_delta_bytes)?;
        if input.len() != expected {
            return Err(SignedDeltaErrorV3::InvalidLength);
        }
        let value = Self {
            input: SignedDeltaPlanInputV3 {
                caller_role: decode_role(byte_at(input, CALLER_ROLE_OFFSET)?)?,
                release_set: nonzero_array(input, RELEASE_SET_OFFSET)?,
                market: nonzero_array(input, MARKET_OFFSET)?,
                request_id: nonzero_array(input, REQUEST_OFFSET)?,
                product_record_digest: nonzero_array(input, PRODUCT_OFFSET)?,
                semantic_basis_id: nonzero_array(input, BASIS_OFFSET)?,
                linked_basis_record_digest: nonzero_array(input, LINKED_BASIS_RECORD_OFFSET)?,
                expected_market_revision: u64_at(input, MARKET_REVISION_OFFSET)?,
                claim_count,
            },
            position_count,
            position_delta_count,
            positions: slice(input, SIGNED_DELTA_PLAN_HEADER_BYTES_V3, positions_bytes)?,
            aggregate_deltas: slice(input, aggregate_offset, aggregate_bytes)?,
            position_deltas: slice(input, position_delta_offset, position_delta_bytes)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact packet into caller-owned storage.
    pub fn encode_into(
        input: SignedDeltaPlanInputV3,
        positions: &[SignedDeltaPositionV3],
        aggregate_deltas: &[SignedDeltaV3],
        position_deltas: &[PositionDeltaV3],
        output: &mut [u8],
    ) -> Result<()> {
        let position_count =
            u32::try_from(positions.len()).map_err(|_| SignedDeltaErrorV3::InvalidCount)?;
        let position_delta_count =
            u32::try_from(position_deltas.len()).map_err(|_| SignedDeltaErrorV3::InvalidCount)?;
        if aggregate_deltas.len()
            != usize::try_from(input.claim_count).map_err(|_| SignedDeltaErrorV3::InvalidCount)?
        {
            return Err(SignedDeltaErrorV3::InvalidCount);
        }
        let expected = plan_bytes(input.claim_count, position_count, position_delta_count)?;
        if output.len() != expected {
            return Err(SignedDeltaErrorV3::InvalidLength);
        }
        output.fill(0);
        put(output, 0, &SIGNED_DELTA_PLAN_MAGIC_V3)?;
        put(
            output,
            VERSION_OFFSET,
            &SIGNED_DELTA_WIRE_VERSION_V3.to_le_bytes(),
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
        put(output, CLAIM_COUNT_OFFSET, &input.claim_count.to_le_bytes())?;
        put(output, POSITION_COUNT_OFFSET, &position_count.to_le_bytes())?;
        put(
            output,
            POSITION_DELTA_COUNT_OFFSET,
            &position_delta_count.to_le_bytes(),
        )?;
        let mut offset = SIGNED_DELTA_PLAN_HEADER_BYTES_V3;
        for position in positions.iter().copied() {
            position.encode_into(slice_mut(output, offset, SIGNED_DELTA_POSITION_BYTES_V3)?)?;
            offset = add(offset, SIGNED_DELTA_POSITION_BYTES_V3)?;
        }
        for delta in aggregate_deltas.iter().copied() {
            delta.encode_into(slice_mut(output, offset, SIGNED_DELTA_BYTES_V3)?)?;
            offset = add(offset, SIGNED_DELTA_BYTES_V3)?;
        }
        for delta in position_deltas.iter().copied() {
            delta.encode_into(slice_mut(output, offset, SIGNED_DELTA_ROW_BYTES_V3)?)?;
            offset = add(offset, SIGNED_DELTA_ROW_BYTES_V3)?;
        }
        SignedDeltaPlanV3::decode(&*output).map(|_| ())
    }

    fn validate(self) -> Result<()> {
        if self.input.claim_count == 0
            || self.position_count == 0
            || self.position_delta_count == 0
            || self.input.expected_market_revision == u64::MAX
        {
            return Err(SignedDeltaErrorV3::InvalidCount);
        }
        for index in 0..self.position_count {
            let position = self.position(index)?;
            if index != 0 && self.position(index - 1)?.owner() >= position.owner() {
                return Err(SignedDeltaErrorV3::InvalidPositionTable);
            }
            let mut used = false;
            for row_index in 0..self.position_delta_count {
                used |= self.position_delta(row_index)?.position_index() == index;
            }
            if !used {
                return Err(SignedDeltaErrorV3::InvalidPositionTable);
            }
        }
        for index in 0..self.position_delta_count {
            let row = self.position_delta(index)?;
            if index != 0 {
                let previous = self.position_delta(index - 1)?;
                if (previous.position_index(), previous.outcome())
                    >= (row.position_index(), row.outcome())
                {
                    return Err(SignedDeltaErrorV3::InvalidCoordinateOrder);
                }
            }
        }
        for outcome in 0..self.input.claim_count {
            self.validate_conservation(outcome)?;
        }
        Ok(())
    }

    fn validate_conservation(self, outcome: u32) -> Result<()> {
        let mut credits = 0_u128;
        let mut debits = 0_u128;
        for index in 0..self.position_delta_count {
            let row = self.position_delta(index)?;
            if row.outcome() != outcome {
                continue;
            }
            match row.delta().direction() {
                DeltaDirectionV3::Neutral => return Err(SignedDeltaErrorV3::NonCanonical),
                DeltaDirectionV3::Credit => {
                    credits = credits
                        .checked_add(u128::from(row.delta().magnitude()))
                        .ok_or(SignedDeltaErrorV3::Arithmetic)?;
                }
                DeltaDirectionV3::Debit => {
                    debits = debits
                        .checked_add(u128::from(row.delta().magnitude()))
                        .ok_or(SignedDeltaErrorV3::Arithmetic)?;
                }
            }
        }
        let aggregate = self.aggregate_delta(outcome)?;
        let conserved = match credits.cmp(&debits) {
            core::cmp::Ordering::Equal => aggregate.direction() == DeltaDirectionV3::Neutral,
            core::cmp::Ordering::Greater => {
                aggregate.direction() == DeltaDirectionV3::Credit
                    && credits.checked_sub(debits) == Some(u128::from(aggregate.magnitude()))
            }
            core::cmp::Ordering::Less => {
                aggregate.direction() == DeltaDirectionV3::Debit
                    && debits.checked_sub(credits) == Some(u128::from(aggregate.magnitude()))
            }
        };
        if !conserved {
            return Err(SignedDeltaErrorV3::Conservation);
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
    /// Return the semantic liability-basis identity.
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
    /// Return the runtime claim count.
    pub const fn claim_count(self) -> u32 {
        self.input.claim_count
    }
    /// Return the number of unique Position entries.
    pub const fn position_count(self) -> u32 {
        self.position_count
    }
    /// Return the number of unique Position deltas.
    pub const fn position_delta_count(self) -> u32 {
        self.position_delta_count
    }

    /// Decode one Position-table entry.
    pub fn position(self, index: u32) -> Result<SignedDeltaPositionV3> {
        let offset = indexed_offset(index, self.position_count, SIGNED_DELTA_POSITION_BYTES_V3)?;
        SignedDeltaPositionV3::decode(slice(
            self.positions,
            offset,
            SIGNED_DELTA_POSITION_BYTES_V3,
        )?)
    }

    /// Decode one implicit-outcome aggregate delta.
    pub fn aggregate_delta(self, outcome: u32) -> Result<SignedDeltaV3> {
        let offset = indexed_offset(outcome, self.input.claim_count, SIGNED_DELTA_BYTES_V3)?;
        SignedDeltaV3::decode(slice(self.aggregate_deltas, offset, SIGNED_DELTA_BYTES_V3)?)
    }

    /// Decode one unique Position delta.
    pub fn position_delta(self, index: u32) -> Result<PositionDeltaV3> {
        let offset = indexed_offset(index, self.position_delta_count, SIGNED_DELTA_ROW_BYTES_V3)?;
        PositionDeltaV3::decode(
            slice(self.position_deltas, offset, SIGNED_DELTA_ROW_BYTES_V3)?,
            self.position_count,
            self.input.claim_count,
        )
    }

    /// Borrow the exact ordered runtime tables committed by this plan.
    pub const fn table_bytes(self) -> (&'a [u8], &'a [u8], &'a [u8]) {
        (self.positions, self.aggregate_deltas, self.position_deltas)
    }
}

/// Exact fixed receipt for one committed signed-delta batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignedDeltaReceiptV3 {
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
    claim_count: u32,
    position_count: u32,
    position_delta_count: u32,
}

impl SignedDeltaReceiptV3 {
    /// Construct a receipt whose aggregate revision advanced exactly once.
    pub fn new(
        plan: SignedDeltaPlanV3<'_>,
        packet_digest: [u8; 32],
        table_digest: [u8; 32],
        claims_program: [u8; 32],
        post_resource_digest: [u8; 32],
        post_market_revision: u64,
    ) -> Result<Self> {
        for identity in [
            packet_digest,
            table_digest,
            claims_program,
            post_resource_digest,
        ] {
            if is_zero(identity) {
                return Err(SignedDeltaErrorV3::ZeroIdentity);
            }
        }
        let expected_post = plan
            .expected_market_revision()
            .checked_add(1)
            .ok_or(SignedDeltaErrorV3::InvalidRevision)?;
        if post_market_revision != expected_post {
            return Err(SignedDeltaErrorV3::InvalidRevision);
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
            claim_count: plan.claim_count(),
            position_count: plan.position_count(),
            position_delta_count: plan.position_delta_count(),
        })
    }

    /// Decode one exact canonical receipt.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != SIGNED_DELTA_RECEIPT_BYTES_V3 {
            return Err(SignedDeltaErrorV3::InvalidLength);
        }
        exact(input, 0, &SIGNED_DELTA_RECEIPT_MAGIC_V3)?;
        if u16_at(input, VERSION_OFFSET)? != SIGNED_DELTA_WIRE_VERSION_V3 {
            return Err(SignedDeltaErrorV3::UnsupportedVersion);
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
            claim_count: u32_at(input, RECEIPT_CLAIM_COUNT_OFFSET)?,
            position_count: u32_at(input, RECEIPT_POSITION_COUNT_OFFSET)?,
            position_delta_count: u32_at(input, RECEIPT_POSITION_DELTA_COUNT_OFFSET)?,
        };
        if value.pre_market_revision.checked_add(1) != Some(value.post_market_revision)
            || value.claim_count == 0
            || value.position_count == 0
            || value.position_delta_count == 0
        {
            return Err(SignedDeltaErrorV3::InvalidRevision);
        }
        Ok(value)
    }

    /// Encode the exact fixed receipt bytes.
    pub fn to_bytes(self) -> [u8; SIGNED_DELTA_RECEIPT_BYTES_V3] {
        let mut output = [0_u8; SIGNED_DELTA_RECEIPT_BYTES_V3];
        put_infallible(&mut output, 0, &SIGNED_DELTA_RECEIPT_MAGIC_V3);
        put_infallible(
            &mut output,
            VERSION_OFFSET,
            &SIGNED_DELTA_WIRE_VERSION_V3.to_le_bytes(),
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
            RECEIPT_CLAIM_COUNT_OFFSET,
            &self.claim_count.to_le_bytes(),
        );
        put_infallible(
            &mut output,
            RECEIPT_POSITION_COUNT_OFFSET,
            &self.position_count.to_le_bytes(),
        );
        put_infallible(
            &mut output,
            RECEIPT_POSITION_DELTA_COUNT_OFFSET,
            &self.position_delta_count.to_le_bytes(),
        );
        output
    }

    /// Require every plan-owned coordinate to agree with this receipt.
    pub fn validate_plan(self, plan: SignedDeltaPlanV3<'_>) -> Result<()> {
        if self.caller_role != plan.caller_role()
            || self.release_set != plan.release_set()
            || self.market != plan.market()
            || self.request_id != plan.request_id()
            || self.product_record_digest != plan.product_record_digest()
            || self.semantic_basis_id != plan.semantic_basis_id()
            || self.linked_basis_record_digest != plan.linked_basis_record_digest()
            || self.pre_market_revision != plan.expected_market_revision()
            || Some(self.post_market_revision) != plan.expected_market_revision().checked_add(1)
            || self.claim_count != plan.claim_count()
            || self.position_count != plan.position_count()
            || self.position_delta_count != plan.position_delta_count()
        {
            return Err(SignedDeltaErrorV3::ReceiptMismatch);
        }
        Ok(())
    }

    /// Return the exact packet digest.
    pub const fn packet_digest(self) -> [u8; 32] {
        self.packet_digest
    }
    /// Return the digest of all three ordered runtime tables.
    pub const fn table_digest(self) -> [u8; 32] {
        self.table_digest
    }
    /// Return the current Claims program that produced the receipt.
    pub const fn claims_program(self) -> [u8; 32] {
        self.claims_program
    }
    /// Return the digest of the aggregate followed by ordered post Positions.
    pub const fn post_resource_digest(self) -> [u8; 32] {
        self.post_resource_digest
    }
    /// Return the aggregate pre-revision.
    pub const fn pre_market_revision(self) -> u64 {
        self.pre_market_revision
    }
    /// Return the aggregate post-revision.
    pub const fn post_market_revision(self) -> u64 {
        self.post_market_revision
    }
}

/// Return the exact packet width for runtime table counts.
pub fn plan_bytes(
    claim_count: u32,
    position_count: u32,
    position_delta_count: u32,
) -> Result<usize> {
    if claim_count == 0 || position_count == 0 || position_delta_count == 0 {
        return Err(SignedDeltaErrorV3::InvalidCount);
    }
    let positions = table_bytes(position_count, SIGNED_DELTA_POSITION_BYTES_V3)?;
    let aggregates = table_bytes(claim_count, SIGNED_DELTA_BYTES_V3)?;
    let deltas = table_bytes(position_delta_count, SIGNED_DELTA_ROW_BYTES_V3)?;
    add(
        add(
            add(SIGNED_DELTA_PLAN_HEADER_BYTES_V3, positions)?,
            aggregates,
        )?,
        deltas,
    )
}

fn decode_role(value: u8) -> Result<CallerRole> {
    match value {
        0 => Ok(CallerRole::Core),
        2 => Ok(CallerRole::Trading),
        _ => Err(SignedDeltaErrorV3::UnknownTag),
    }
}

fn table_bytes(count: u32, element_bytes: usize) -> Result<usize> {
    usize::try_from(count)
        .ok()
        .and_then(|count| count.checked_mul(element_bytes))
        .ok_or(SignedDeltaErrorV3::InvalidLength)
}

fn indexed_offset(index: u32, count: u32, element_bytes: usize) -> Result<usize> {
    if index >= count {
        return Err(SignedDeltaErrorV3::InvalidIndex);
    }
    usize::try_from(index)
        .ok()
        .and_then(|index| index.checked_mul(element_bytes))
        .ok_or(SignedDeltaErrorV3::InvalidIndex)
}

fn add(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right)
        .ok_or(SignedDeltaErrorV3::InvalidLength)
}

fn exact(input: &[u8], offset: usize, expected: &[u8]) -> Result<()> {
    if slice(input, offset, expected.len())? != expected {
        return Err(SignedDeltaErrorV3::InvalidMagic);
    }
    Ok(())
}

fn require_zero(input: &[u8], offset: usize, length: usize) -> Result<()> {
    if slice(input, offset, length)?.iter().any(|byte| *byte != 0) {
        return Err(SignedDeltaErrorV3::NonCanonical);
    }
    Ok(())
}

fn byte_at(input: &[u8], offset: usize) -> Result<u8> {
    input
        .get(offset)
        .copied()
        .ok_or(SignedDeltaErrorV3::InvalidLength)
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
    if is_zero(value) {
        return Err(SignedDeltaErrorV3::ZeroIdentity);
    }
    Ok(value)
}
fn array_at<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    slice(input, offset, N)?
        .try_into()
        .map_err(|_| SignedDeltaErrorV3::InvalidLength)
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
                    .ok_or(SignedDeltaErrorV3::InvalidLength)?,
        )
        .ok_or(SignedDeltaErrorV3::InvalidLength)
}
fn slice_mut(input: &mut [u8], offset: usize, length: usize) -> Result<&mut [u8]> {
    input
        .get_mut(
            offset
                ..offset
                    .checked_add(length)
                    .ok_or(SignedDeltaErrorV3::InvalidLength)?,
        )
        .ok_or(SignedDeltaErrorV3::InvalidLength)
}
fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    slice_mut(output, offset, value.len())?.copy_from_slice(value);
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
    use super::*;
    use std::vec;

    fn credit(value: u64) -> SignedDeltaV3 {
        SignedDeltaV3::new(DeltaDirectionV3::Credit, value).expect("credit")
    }
    fn debit(value: u64) -> SignedDeltaV3 {
        SignedDeltaV3::new(DeltaDirectionV3::Debit, value).expect("debit")
    }
    fn neutral() -> SignedDeltaV3 {
        SignedDeltaV3::new(DeltaDirectionV3::Neutral, 0).expect("neutral")
    }
    fn header(claim_count: u32) -> SignedDeltaPlanInputV3 {
        SignedDeltaPlanInputV3 {
            caller_role: CallerRole::Trading,
            release_set: [1; 32],
            market: [2; 32],
            request_id: [3; 32],
            product_record_digest: [4; 32],
            semantic_basis_id: [5; 32],
            linked_basis_record_digest: [6; 32],
            expected_market_revision: 7,
            claim_count,
        }
    }
    fn row(
        position_index: u32,
        outcome: u32,
        delta: SignedDeltaV3,
        positions: u32,
        claims: u32,
    ) -> PositionDeltaV3 {
        PositionDeltaV3::new(
            PositionDeltaInputV3 {
                position_index,
                outcome,
                delta,
            },
            positions,
            claims,
        )
        .expect("row")
    }

    #[test]
    fn split_transfer_merge_net_to_one_unique_coordinate() {
        let positions = [
            SignedDeltaPositionV3::new([7; 32], 3).expect("a"),
            SignedDeltaPositionV3::new([8; 32], 4).expect("b"),
        ];
        let aggregates = [credit(5), neutral()];
        let deltas = [row(0, 0, credit(12), 2, 2), row(1, 0, debit(7), 2, 2)];
        let mut bytes = vec![0; plan_bytes(2, 2, 2).expect("width")];
        SignedDeltaPlanV3::encode_into(header(2), &positions, &aggregates, &deltas, &mut bytes)
            .expect("encode");
        let plan = SignedDeltaPlanV3::decode(&bytes).expect("decode");
        assert_eq!(plan.aggregate_delta(0).expect("aggregate"), credit(5));
        assert_eq!(
            plan.position_delta(0).expect("net maker").delta(),
            credit(12)
        );
        assert_eq!(bytes.len(), 240 + 2 * 40 + 2 * 16 + 2 * 24);
    }

    #[test]
    fn full_u64_transfer_conserves_through_u128_totals() {
        let positions = [
            SignedDeltaPositionV3::new([7; 32], 3).expect("a"),
            SignedDeltaPositionV3::new([8; 32], 4).expect("b"),
        ];
        let deltas = [
            row(0, 0, debit(u64::MAX), 2, 1),
            row(1, 0, credit(u64::MAX), 2, 1),
        ];
        let mut bytes = vec![0; plan_bytes(1, 2, 2).expect("width")];
        SignedDeltaPlanV3::encode_into(header(1), &positions, &[neutral()], &deltas, &mut bytes)
            .expect("encode");
        assert!(SignedDeltaPlanV3::decode(&bytes).is_ok());
    }

    #[test]
    fn duplicate_unsorted_and_nonconserving_coordinates_refuse() {
        let positions = [
            SignedDeltaPositionV3::new([7; 32], 3).expect("a"),
            SignedDeltaPositionV3::new([8; 32], 4).expect("b"),
        ];
        let one_position = [SignedDeltaPositionV3::new([7; 32], 3).expect("a")];
        let duplicate = [row(0, 0, credit(1), 1, 1), row(0, 0, debit(1), 1, 1)];
        let mut duplicate_bytes = vec![0; plan_bytes(1, 1, 2).expect("width")];
        assert_eq!(
            SignedDeltaPlanV3::encode_into(
                header(1),
                &one_position,
                &[neutral()],
                &duplicate,
                &mut duplicate_bytes
            ),
            Err(SignedDeltaErrorV3::InvalidCoordinateOrder)
        );
        let unsorted_positions = [positions[1], positions[0]];
        let deltas = [row(0, 0, debit(1), 2, 1), row(1, 0, credit(1), 2, 1)];
        let mut bytes = vec![0; plan_bytes(1, 2, 2).expect("width")];
        assert_eq!(
            SignedDeltaPlanV3::encode_into(
                header(1),
                &unsorted_positions,
                &[neutral()],
                &deltas,
                &mut bytes
            ),
            Err(SignedDeltaErrorV3::InvalidPositionTable)
        );
        let duplicate_positions = [
            SignedDeltaPositionV3::new([7; 32], 3).expect("a"),
            SignedDeltaPositionV3::new([7; 32], 4).expect("duplicate"),
        ];
        assert_eq!(
            SignedDeltaPlanV3::encode_into(
                header(1),
                &duplicate_positions,
                &[neutral()],
                &deltas,
                &mut bytes
            ),
            Err(SignedDeltaErrorV3::InvalidPositionTable)
        );
        assert_eq!(
            SignedDeltaPlanV3::encode_into(
                header(1),
                &positions,
                &[credit(1)],
                &deltas,
                &mut bytes
            ),
            Err(SignedDeltaErrorV3::Conservation)
        );
    }

    #[test]
    fn hostile_wire_and_receipt_substitution_refuse() {
        let positions = [SignedDeltaPositionV3::new([7; 32], 3).expect("a")];
        let deltas = [row(0, 0, credit(1), 1, 1)];
        let mut bytes = vec![0; plan_bytes(1, 1, 1).expect("width")];
        SignedDeltaPlanV3::encode_into(header(1), &positions, &[credit(1)], &deltas, &mut bytes)
            .expect("encode");
        let plan = SignedDeltaPlanV3::decode(&bytes).expect("plan");
        let receipt = SignedDeltaReceiptV3::new(plan, [8; 32], [9; 32], [10; 32], [11; 32], 8)
            .expect("receipt");
        assert_eq!(
            SignedDeltaReceiptV3::decode(&receipt.to_bytes()).expect("decode"),
            receipt
        );
        let mut substituted = bytes.clone();
        *substituted
            .get_mut(HEADER_TAIL_RESERVED_OFFSET)
            .expect("reserved byte") = 1;
        assert_eq!(
            SignedDeltaPlanV3::decode(&substituted),
            Err(SignedDeltaErrorV3::NonCanonical)
        );
        let mut other = header(1);
        other.request_id = [12; 32];
        SignedDeltaPlanV3::encode_into(other, &positions, &[credit(1)], &deltas, &mut substituted)
            .expect("other");
        assert_eq!(
            receipt.validate_plan(SignedDeltaPlanV3::decode(&substituted).expect("other plan")),
            Err(SignedDeltaErrorV3::ReceiptMismatch)
        );
    }
}
