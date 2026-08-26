//! Protected scalar-selected account spans and borrowed request ranges.
//!
//! This allocation-free successor wraps one canonical V3 program. It permits
//! finite descriptor-owned extensions of fixed child frames and exact ranges
//! of the digest-authenticated family request to be appended to selected child
//! requests. Fixed values or authenticated common scalar registers own every
//! coordinate. Admission must prove that every selected scalar is derived and
//! protected by the selected Transition.

use super::v3::{Error as ErrorV3, ProgramV3, ResolvedEffectV3, ResolvedInvocationV3};

/// Distinct successor magic.
pub const MAGIC_V4: [u8; 4] = *b"DCE5";
/// Successor wire version. Canonical V3 uses wire version four.
pub const VERSION_V4: u8 = 5;
/// Finalized-record schema label.
pub const SCHEMA_RELEASE_PREIMAGE_V4: &[u8] =
    b"dclutch/schema/effect-program-v5-scalar-spans-and-borrowed-ranges-v1";
/// SHA-256 of [`SCHEMA_RELEASE_PREIMAGE_V4`].
pub const SCHEMA_RELEASE_ID_V4: [u8; 32] = [
    0x18, 0x4f, 0x83, 0x50, 0x9b, 0x14, 0x23, 0xdb, 0xf7, 0xb2, 0x9d, 0x60, 0xd2, 0xbe, 0x63, 0x09,
    0xa5, 0x44, 0x7c, 0x4f, 0x87, 0xc3, 0xca, 0x54, 0x70, 0x9c, 0xfb, 0xda, 0x17, 0x10, 0x12, 0x8b,
];
/// Exact successor header width.
pub const HEADER_BYTES_V4: usize = 24;
/// Exact width of one dynamic-span declaration.
pub const DYNAMIC_SPAN_BYTES_V4: usize = 16;
/// Exact width of one borrowed-range declaration.
pub const BORROWED_RANGE_BYTES_V4: usize = 16;

const MAX_EXTENSION_V4: u16 = 63;
const COORDINATE_FIXED: u8 = 0;
const COORDINATE_COMMON_SCALAR: u8 = 1;

/// Stable hostile-decode or runtime-resolution refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorV4 {
    /// Width, magic, version, policy, or reserved bytes differed.
    Wire,
    /// Embedded V3 program refused.
    BaseProgram,
    /// Span declarations were unordered, duplicated, or out of range.
    SpanTable,
    /// A selected span was unavailable or outside its finite set.
    SpanSelection,
    /// Borrowed-range declarations were noncanonical or out of range.
    RangeTable,
    /// A protected range coordinate could not be resolved.
    RangeSelection,
    /// Prefix and ranges did not satisfy the declared coverage policy.
    RequestCoverage,
    /// Checked width or coordinate arithmetic overflowed.
    Arithmetic,
}

impl From<ErrorV3> for ErrorV4 {
    fn from(_: ErrorV3) -> Self {
        Self::BaseProgram
    }
}

/// Result alias for successor programs.
pub type ResultV4<T> = core::result::Result<T, ErrorV4>;

/// Explicit request-range overlap and coverage policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum BorrowedRangePolicyV4 {
    /// Prefix and ranges form one disjoint, ordered, exhaustive partition.
    DisjointExactCoverage = 0,
    /// Exact duplicate ranges may be consumed by multiple routes; distinct
    /// ranges still form one disjoint, ordered, exhaustive partition.
    IdenticalReuseExactCoverage = 1,
}

impl BorrowedRangePolicyV4 {
    fn decode(value: u8) -> ResultV4<Self> {
        match value {
            0 => Ok(Self::DisjointExactCoverage),
            1 => Ok(Self::IdenticalReuseExactCoverage),
            _ => Err(ErrorV4::Wire),
        }
    }
}

/// A range coordinate fixed by the artifact or read from an authenticated
/// common scalar register.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestCoordinateV4 {
    /// Exact artifact-owned byte coordinate.
    Fixed(u32),
    /// Exact common scalar register protected by admission.
    CommonScalar(u16),
}

impl RequestCoordinateV4 {
    fn decode(kind: u8, value: u32) -> ResultV4<Self> {
        match kind {
            COORDINATE_FIXED => Ok(Self::Fixed(value)),
            COORDINATE_COMMON_SCALAR => Ok(Self::CommonScalar(
                u16::try_from(value).map_err(|_| ErrorV4::RangeTable)?,
            )),
            _ => Err(ErrorV4::RangeTable),
        }
    }

    fn encode(self) -> (u8, u32) {
        match self {
            Self::Fixed(value) => (COORDINATE_FIXED, value),
            Self::CommonScalar(value) => (COORDINATE_COMMON_SCALAR, u32::from(value)),
        }
    }

    fn validate(self, common_scalar_count: u16) -> ResultV4<()> {
        if matches!(self, Self::CommonScalar(index) if index >= common_scalar_count) {
            return Err(ErrorV4::RangeTable);
        }
        Ok(())
    }

    fn resolve(self, scalars: &[u64]) -> ResultV4<usize> {
        match self {
            Self::Fixed(value) => usize::try_from(value).map_err(|_| ErrorV4::Arithmetic),
            Self::CommonScalar(index) => usize::try_from(
                *scalars
                    .get(usize::from(index))
                    .ok_or(ErrorV4::RangeSelection)?,
            )
            .map_err(|_| ErrorV4::RangeSelection),
        }
    }
}

/// One descriptor-owned finite extension set for one route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DynamicFixedSpanV4 {
    route: u16,
    selector_common_scalar: u16,
    base_fixed_account_count: u16,
    allowed_extensions: u64,
}

impl DynamicFixedSpanV4 {
    /// Construct one span declaration. Full joins occur on encode/decode.
    pub const fn new(
        route: u16,
        selector_common_scalar: u16,
        base_fixed_account_count: u16,
        allowed_extensions: u64,
    ) -> Self {
        Self {
            route,
            selector_common_scalar,
            base_fixed_account_count,
            allowed_extensions,
        }
    }

    /// Route whose fixed account frame is extended.
    pub const fn route(self) -> u16 {
        self.route
    }

    /// Protected common scalar selecting the extension.
    pub const fn selector_common_scalar(self) -> u16 {
        self.selector_common_scalar
    }

    /// Fixed accounts already declared by the embedded V3 route.
    pub const fn base_fixed_account_count(self) -> u16 {
        self.base_fixed_account_count
    }

    /// Bit `n` admits exactly extension `n`.
    pub const fn allowed_extensions(self) -> u64 {
        self.allowed_extensions
    }

    fn decode(bytes: &[u8], offset: usize) -> ResultV4<Self> {
        if slice(bytes, offset + 6, 2)?.iter().any(|value| *value != 0) {
            return Err(ErrorV4::Wire);
        }
        Ok(Self {
            route: read_u16(bytes, offset)?,
            selector_common_scalar: read_u16(bytes, offset + 2)?,
            base_fixed_account_count: read_u16(bytes, offset + 4)?,
            allowed_extensions: read_u64(bytes, offset + 8)?,
        })
    }

    fn selected(self, scalars: &[u64]) -> ResultV4<u16> {
        let selected = *scalars
            .get(usize::from(self.selector_common_scalar))
            .ok_or(ErrorV4::SpanSelection)?;
        let selected = u16::try_from(selected).map_err(|_| ErrorV4::SpanSelection)?;
        if selected > MAX_EXTENSION_V4
            || self.allowed_extensions & (1_u64 << u32::from(selected)) == 0
        {
            return Err(ErrorV4::SpanSelection);
        }
        Ok(selected)
    }
}

/// One source-request range appended to one selected child route.
///
/// The table is encoded in resolved source order, not route order. Under the
/// reuse policy, exact duplicates are then ordered by strictly increasing
/// route so the same proof can feed two phases without accepting ambiguity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BorrowedRangeV4 {
    route: u16,
    offset: RequestCoordinateV4,
    len: RequestCoordinateV4,
}

impl BorrowedRangeV4 {
    /// Construct one descriptor-owned borrowed range.
    pub const fn new(route: u16, offset: RequestCoordinateV4, len: RequestCoordinateV4) -> Self {
        Self { route, offset, len }
    }

    /// Consumer route.
    pub const fn route(self) -> u16 {
        self.route
    }

    /// Protected source offset coordinate.
    pub const fn offset(self) -> RequestCoordinateV4 {
        self.offset
    }

    /// Protected byte-length coordinate.
    pub const fn len(self) -> RequestCoordinateV4 {
        self.len
    }

    fn decode(bytes: &[u8], offset: usize) -> ResultV4<Self> {
        if slice(bytes, offset + 12, 4)?
            .iter()
            .any(|value| *value != 0)
        {
            return Err(ErrorV4::Wire);
        }
        Ok(Self {
            route: read_u16(bytes, offset)?,
            offset: RequestCoordinateV4::decode(
                read_u8(bytes, offset + 2)?,
                read_u32(bytes, offset + 4)?,
            )?,
            len: RequestCoordinateV4::decode(
                read_u8(bytes, offset + 3)?,
                read_u32(bytes, offset + 8)?,
            )?,
        })
    }

    fn resolve(self, scalars: &[u64]) -> ResultV4<ResolvedBorrowedRangeV4> {
        let source_offset = self.offset.resolve(scalars)?;
        let len = self.len.resolve(scalars)?;
        if len == 0 {
            return Err(ErrorV4::RequestCoverage);
        }
        source_offset.checked_add(len).ok_or(ErrorV4::Arithmetic)?;
        Ok(ResolvedBorrowedRangeV4 { source_offset, len })
    }
}

/// One exact nonempty range in the authenticated family request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedBorrowedRangeV4 {
    source_offset: usize,
    len: usize,
}

impl ResolvedBorrowedRangeV4 {
    /// Absolute byte offset within the family request.
    pub const fn source_offset(self) -> usize {
        self.source_offset
    }

    /// Exact nonzero byte width.
    pub const fn len(self) -> usize {
        self.len
    }

    /// Ranges are canonically nonempty; this accessor mirrors slice APIs.
    pub const fn is_empty(self) -> bool {
        false
    }

    /// Borrow this exact range, refusing overflow or truncation. It need not
    /// end at the family-request boundary.
    pub fn slice(self, family_request: &[u8]) -> ResultV4<&[u8]> {
        let end = self
            .source_offset
            .checked_add(self.len)
            .ok_or(ErrorV4::Arithmetic)?;
        family_request
            .get(self.source_offset..end)
            .ok_or(ErrorV4::RequestCoverage)
    }
}

/// One resolved V3 invocation plus its exact appended-range count.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedInvocationV4 {
    /// Embedded V3 invocation after fixed-account span shifts.
    pub invocation: ResolvedInvocationV3,
    borrowed_range_count: u16,
}

impl ResolvedInvocationV4 {
    /// Exact number of V4 borrowed ranges appended in route-local order.
    pub const fn borrowed_range_count(self) -> u16 {
        self.borrowed_range_count
    }
}

/// Borrowed canonical V3 program plus protected successor geometry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgramV4<'a> {
    bytes: &'a [u8],
    base: ProgramV3<'a>,
    policy: BorrowedRangePolicyV4,
    span_count: u16,
    range_count: u16,
    semantic_prefix_bytes: u32,
    range_start: usize,
}

impl<'a> ProgramV4<'a> {
    /// Hostile-decode the exact successor program and every embedded join.
    pub fn decode(bytes: &'a [u8]) -> ResultV4<Self> {
        if bytes.len() < HEADER_BYTES_V4
            || bytes.get(..4) != Some(MAGIC_V4.as_slice())
            || read_u8(bytes, 4)? != VERSION_V4
            || read_u16(bytes, 10)? != 0
            || slice(bytes, 20, 4)?.iter().any(|value| *value != 0)
        {
            return Err(ErrorV4::Wire);
        }
        let policy = BorrowedRangePolicyV4::decode(read_u8(bytes, 5)?)?;
        let span_count = read_u16(bytes, 6)?;
        let range_count = read_u16(bytes, 8)?;
        let base_bytes = usize::try_from(read_u32(bytes, 12)?).map_err(|_| ErrorV4::Wire)?;
        let semantic_prefix_bytes = read_u32(bytes, 16)?;
        if (span_count == 0 && range_count == 0) || base_bytes == 0 || semantic_prefix_bytes == 0 {
            return Err(ErrorV4::Wire);
        }
        let range_start = HEADER_BYTES_V4
            .checked_add(
                usize::from(span_count)
                    .checked_mul(DYNAMIC_SPAN_BYTES_V4)
                    .ok_or(ErrorV4::Arithmetic)?,
            )
            .ok_or(ErrorV4::Arithmetic)?;
        let base_start = range_start
            .checked_add(
                usize::from(range_count)
                    .checked_mul(BORROWED_RANGE_BYTES_V4)
                    .ok_or(ErrorV4::Arithmetic)?,
            )
            .ok_or(ErrorV4::Arithmetic)?;
        let base_end = base_start
            .checked_add(base_bytes)
            .ok_or(ErrorV4::Arithmetic)?;
        if base_end != bytes.len() {
            return Err(ErrorV4::Wire);
        }
        let base = ProgramV3::decode(slice(bytes, base_start, base_bytes)?)?;
        let value = Self {
            bytes,
            base,
            policy,
            span_count,
            range_count,
            semantic_prefix_bytes,
            range_start,
        };
        value.validate_span_table()?;
        value.validate_range_table()?;
        Ok(value)
    }

    /// Borrow the complete canonical successor bytes.
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Borrow the embedded exact V3 program.
    pub const fn base(self) -> ProgramV3<'a> {
        self.base
    }

    /// Explicit range overlap and coverage policy.
    pub const fn borrowed_range_policy(self) -> BorrowedRangePolicyV4 {
        self.policy
    }

    /// Leading bytes owned by the family semantic header/transition.
    pub const fn semantic_prefix_bytes(self) -> u32 {
        self.semantic_prefix_bytes
    }

    /// Number of dynamic-span declarations.
    pub const fn span_count(self) -> u16 {
        self.span_count
    }

    /// Number of source-ordered borrowed-range declarations.
    pub const fn range_count(self) -> u16 {
        self.range_count
    }

    /// Decode one ordered dynamic-span declaration.
    pub fn span(self, index: u16) -> ResultV4<DynamicFixedSpanV4> {
        if index >= self.span_count {
            return Err(ErrorV4::SpanTable);
        }
        let offset = HEADER_BYTES_V4
            .checked_add(
                usize::from(index)
                    .checked_mul(DYNAMIC_SPAN_BYTES_V4)
                    .ok_or(ErrorV4::Arithmetic)?,
            )
            .ok_or(ErrorV4::Arithmetic)?;
        DynamicFixedSpanV4::decode(self.bytes, offset)
    }

    /// Decode one source-ordered borrowed-range declaration.
    pub fn borrowed_range(self, index: u16) -> ResultV4<BorrowedRangeV4> {
        if index >= self.range_count {
            return Err(ErrorV4::RangeTable);
        }
        let offset = self
            .range_start
            .checked_add(
                usize::from(index)
                    .checked_mul(BORROWED_RANGE_BYTES_V4)
                    .ok_or(ErrorV4::Arithmetic)?,
            )
            .ok_or(ErrorV4::Arithmetic)?;
        BorrowedRangeV4::decode(self.bytes, offset)
    }

    /// Exact expanded account-vector width under selected finite spans.
    pub fn account_count(self, tail_count: u32, scalars: &[u64]) -> ResultV4<usize> {
        self.require_scalar_width(tail_count, scalars)?;
        self.base
            .account_count(tail_count)?
            .checked_add(usize::from(self.total_extension(scalars)?))
            .ok_or(ErrorV4::Arithmetic)
    }

    /// Resolve one child invocation after applying all earlier span shifts.
    pub fn resolved_invocation(
        self,
        route_index: u16,
        invocation_index: u32,
        tail_count: u32,
        scalars: &[u64],
        identities: &[[u8; 32]],
    ) -> ResultV4<ResolvedInvocationV4> {
        self.require_scalar_width(tail_count, scalars)?;
        let mut invocation = self.base.resolved_invocation(
            route_index,
            invocation_index,
            tail_count,
            scalars,
            identities,
        )?;
        invocation.fixed_account_start = invocation
            .fixed_account_start
            .checked_add(self.extension_before_route(route_index, scalars)?)
            .ok_or(ErrorV4::Arithmetic)?;
        if let Some(span) = self.span_for_route(route_index)? {
            invocation.fixed_account_count = invocation
                .fixed_account_count
                .checked_add(span.selected(scalars)?)
                .ok_or(ErrorV4::Arithmetic)?;
        }
        invocation.item_account_start = invocation
            .item_account_start
            .checked_add(usize::from(self.total_extension(scalars)?))
            .ok_or(ErrorV4::Arithmetic)?;
        Ok(ResolvedInvocationV4 {
            invocation,
            borrowed_range_count: self.borrowed_range_count_for_route(route_index)?,
        })
    }

    /// Prove the prefix and resolved ranges exhaust the family request under
    /// the declared overlap policy. Every consuming route must execute exactly
    /// once, preventing a disabled or repeated route from dropping/duplicating
    /// a range accidentally.
    pub fn validate_request_coverage(
        self,
        family_request_len: usize,
        tail_count: u32,
        scalars: &[u64],
        identities: &[[u8; 32]],
    ) -> ResultV4<()> {
        self.require_scalar_width(tail_count, scalars)?;
        let mut cursor =
            usize::try_from(self.semantic_prefix_bytes).map_err(|_| ErrorV4::Arithmetic)?;
        if cursor > family_request_len {
            return Err(ErrorV4::RequestCoverage);
        }
        let mut previous: Option<(ResolvedBorrowedRangeV4, u16)> = None;
        let mut index = 0_u16;
        while index < self.range_count {
            let declaration = self.borrowed_range(index)?;
            if self
                .base
                .invocation_count(declaration.route, tail_count, scalars, identities)?
                != 1
            {
                return Err(ErrorV4::RequestCoverage);
            }
            let resolved = declaration.resolve(scalars)?;
            let end = resolved
                .source_offset
                .checked_add(resolved.len)
                .ok_or(ErrorV4::Arithmetic)?;
            if resolved.source_offset == cursor {
                cursor = end;
            } else if self.policy == BorrowedRangePolicyV4::IdenticalReuseExactCoverage
                && previous.is_some_and(|(prior, prior_route)| {
                    prior == resolved && prior_route < declaration.route
                })
            {
                // Exact reuse neither advances nor fragments coverage.
            } else {
                return Err(ErrorV4::RequestCoverage);
            }
            if cursor > family_request_len {
                return Err(ErrorV4::RequestCoverage);
            }
            previous = Some((resolved, declaration.route));
            index = index.checked_add(1).ok_or(ErrorV4::Arithmetic)?;
        }
        if cursor != family_request_len {
            return Err(ErrorV4::RequestCoverage);
        }
        Ok(())
    }

    /// Number of borrowed ranges appended by one route.
    pub fn borrowed_range_count_for_route(self, route: u16) -> ResultV4<u16> {
        if route >= self.base.route_count() {
            return Err(ErrorV4::RangeTable);
        }
        let mut count = 0_u16;
        let mut index = 0_u16;
        while index < self.range_count {
            if self.borrowed_range(index)?.route == route {
                count = count.checked_add(1).ok_or(ErrorV4::Arithmetic)?;
            }
            index = index.checked_add(1).ok_or(ErrorV4::Arithmetic)?;
        }
        Ok(count)
    }

    /// Resolve the `ordinal`th range consumed by one route. Route-local order
    /// is inherited from the source-ordered canonical table.
    pub fn resolved_borrowed_range(
        self,
        route: u16,
        ordinal: u16,
        scalars: &[u64],
    ) -> ResultV4<ResolvedBorrowedRangeV4> {
        if route >= self.base.route_count() {
            return Err(ErrorV4::RangeTable);
        }
        let mut seen = 0_u16;
        let mut index = 0_u16;
        while index < self.range_count {
            let declaration = self.borrowed_range(index)?;
            if declaration.route == route {
                if seen == ordinal {
                    return declaration.resolve(scalars);
                }
                seen = seen.checked_add(1).ok_or(ErrorV4::Arithmetic)?;
            }
            index = index.checked_add(1).ok_or(ErrorV4::Arithmetic)?;
        }
        Err(ErrorV4::RangeSelection)
    }

    /// Validate a half-open global route execution window. This makes one
    /// authenticated program resumable without inventing phase-local programs.
    pub fn validate_route_window(self, start: u16, end: u16) -> ResultV4<()> {
        if start >= end || end > self.base.route_count() {
            return Err(ErrorV4::RangeSelection);
        }
        Ok(())
    }

    /// Resolve and shift one fixed-body local effect.
    pub fn resolved_fixed_effect(
        self,
        index: u16,
        tail_count: u32,
        scalars: &[u64],
        identities: &[[u8; 32]],
    ) -> ResultV4<ResolvedEffectV3> {
        let effect = self
            .base
            .resolved_fixed_effect(index, tail_count, scalars, identities)?;
        self.shift_effect(effect, scalars)
    }

    /// Resolve and shift one repeated-item local effect.
    pub fn resolved_item_effect(
        self,
        item: u32,
        index: u16,
        tail_count: u32,
        scalars: &[u64],
        identities: &[[u8; 32]],
    ) -> ResultV4<ResolvedEffectV3> {
        let effect = self
            .base
            .resolved_item_effect(item, index, tail_count, scalars, identities)?;
        self.shift_effect(effect, scalars)
    }

    fn validate_span_table(self) -> ResultV4<()> {
        let mut previous_route = None;
        let mut index = 0_u16;
        while index < self.span_count {
            let span = self.span(index)?;
            let route = self.base.route(span.route)?;
            if previous_route.is_some_and(|previous| previous >= span.route)
                || span.route >= self.base.route_count()
                || span.selector_common_scalar >= self.base.common_scalar_count()
                || span.base_fixed_account_count != route.fixed_account_count()
                || span.allowed_extensions == 0
            {
                return Err(ErrorV4::SpanTable);
            }
            previous_route = Some(span.route);
            index = index.checked_add(1).ok_or(ErrorV4::Arithmetic)?;
        }
        Ok(())
    }

    fn validate_range_table(self) -> ResultV4<()> {
        let mut route = 0_u16;
        while route < self.base.route_count() {
            if self.base.route(route)?.borrows_witness() {
                return Err(ErrorV4::RangeTable);
            }
            route = route.checked_add(1).ok_or(ErrorV4::Arithmetic)?;
        }
        let mut index = 0_u16;
        while index < self.range_count {
            let declaration = self.borrowed_range(index)?;
            if declaration.route >= self.base.route_count() {
                return Err(ErrorV4::RangeTable);
            }
            declaration
                .offset
                .validate(self.base.common_scalar_count())?;
            declaration.len.validate(self.base.common_scalar_count())?;
            if declaration.len == RequestCoordinateV4::Fixed(0) {
                return Err(ErrorV4::RangeTable);
            }
            index = index.checked_add(1).ok_or(ErrorV4::Arithmetic)?;
        }
        Ok(())
    }

    fn shift_effect(self, effect: ResolvedEffectV3, scalars: &[u64]) -> ResultV4<ResolvedEffectV3> {
        Ok(match effect {
            ResolvedEffectV3::TransferLamports {
                source,
                destination,
                amount,
            } => ResolvedEffectV3::TransferLamports {
                source: self.shift_account(source, scalars)?,
                destination: self.shift_account(destination, scalars)?,
                amount,
            },
            ResolvedEffectV3::WriteScalar {
                account,
                offset,
                value,
            } => ResolvedEffectV3::WriteScalar {
                account: self.shift_account(account, scalars)?,
                offset,
                value,
            },
            ResolvedEffectV3::WriteIdentity {
                account,
                offset,
                value,
            } => ResolvedEffectV3::WriteIdentity {
                account: self.shift_account(account, scalars)?,
                offset,
                value,
            },
            ResolvedEffectV3::WriteU8 {
                account,
                offset,
                value,
            } => ResolvedEffectV3::WriteU8 {
                account: self.shift_account(account, scalars)?,
                offset,
                value,
            },
            ResolvedEffectV3::WriteU16 {
                account,
                offset,
                value,
            } => ResolvedEffectV3::WriteU16 {
                account: self.shift_account(account, scalars)?,
                offset,
                value,
            },
            ResolvedEffectV3::WriteU32 {
                account,
                offset,
                value,
            } => ResolvedEffectV3::WriteU32 {
                account: self.shift_account(account, scalars)?,
                offset,
                value,
            },
            ResolvedEffectV3::RequireLamportsEq { account, value } => {
                ResolvedEffectV3::RequireLamportsEq {
                    account: self.shift_account(account, scalars)?,
                    value,
                }
            }
            ResolvedEffectV3::WriteRequest {
                route,
                offset,
                value,
            } => ResolvedEffectV3::WriteRequest {
                route,
                offset,
                value,
            },
        })
    }

    fn shift_account(self, account: usize, scalars: &[u64]) -> ResultV4<usize> {
        if account >= usize::from(self.base.fixed_account_count()) {
            return account
                .checked_add(usize::from(self.total_extension(scalars)?))
                .ok_or(ErrorV4::Arithmetic);
        }
        let coordinate = u16::try_from(account).map_err(|_| ErrorV4::Arithmetic)?;
        let mut shift = 0_u16;
        let mut index = 0_u16;
        while index < self.span_count {
            let span = self.span(index)?;
            let route = self.base.route(span.route)?;
            let end = route
                .fixed_account_start()
                .checked_add(span.base_fixed_account_count)
                .ok_or(ErrorV4::Arithmetic)?;
            if coordinate >= end {
                shift = shift
                    .checked_add(span.selected(scalars)?)
                    .ok_or(ErrorV4::Arithmetic)?;
            }
            index = index.checked_add(1).ok_or(ErrorV4::Arithmetic)?;
        }
        account
            .checked_add(usize::from(shift))
            .ok_or(ErrorV4::Arithmetic)
    }

    fn span_for_route(self, route: u16) -> ResultV4<Option<DynamicFixedSpanV4>> {
        let mut index = 0_u16;
        while index < self.span_count {
            let span = self.span(index)?;
            if span.route == route {
                return Ok(Some(span));
            }
            index = index.checked_add(1).ok_or(ErrorV4::Arithmetic)?;
        }
        Ok(None)
    }

    fn extension_before_route(self, route: u16, scalars: &[u64]) -> ResultV4<u16> {
        let mut total = 0_u16;
        let mut index = 0_u16;
        while index < self.span_count {
            let span = self.span(index)?;
            if span.route >= route {
                break;
            }
            total = total
                .checked_add(span.selected(scalars)?)
                .ok_or(ErrorV4::Arithmetic)?;
            index = index.checked_add(1).ok_or(ErrorV4::Arithmetic)?;
        }
        Ok(total)
    }

    fn total_extension(self, scalars: &[u64]) -> ResultV4<u16> {
        self.extension_before_route(self.base.route_count(), scalars)
    }

    fn require_scalar_width(self, tail_count: u32, scalars: &[u64]) -> ResultV4<()> {
        if self.base.scalar_count(tail_count)? != scalars.len() {
            return Err(ErrorV4::SpanSelection);
        }
        Ok(())
    }
}

/// Encode one exact successor program atomically around canonical V3 bytes.
pub fn encode_program_v4_atomic(
    base_program: &[u8],
    policy: BorrowedRangePolicyV4,
    semantic_prefix_bytes: u32,
    spans: &[DynamicFixedSpanV4],
    ranges: &[BorrowedRangeV4],
    scratch: &mut [u8],
    output: &mut [u8],
) -> ResultV4<()> {
    if (spans.is_empty() && ranges.is_empty()) || semantic_prefix_bytes == 0 {
        return Err(ErrorV4::Wire);
    }
    let span_bytes = spans
        .len()
        .checked_mul(DYNAMIC_SPAN_BYTES_V4)
        .ok_or(ErrorV4::Arithmetic)?;
    let range_bytes = ranges
        .len()
        .checked_mul(BORROWED_RANGE_BYTES_V4)
        .ok_or(ErrorV4::Arithmetic)?;
    let expected = HEADER_BYTES_V4
        .checked_add(span_bytes)
        .and_then(|value| value.checked_add(range_bytes))
        .and_then(|value| value.checked_add(base_program.len()))
        .ok_or(ErrorV4::Arithmetic)?;
    if scratch.len() != expected || output.len() != expected {
        return Err(ErrorV4::Wire);
    }
    ProgramV3::decode(base_program)?;
    scratch.fill(0);
    put(scratch, 0, &MAGIC_V4)?;
    put(scratch, 4, &[VERSION_V4, policy as u8])?;
    put(
        scratch,
        6,
        &u16::try_from(spans.len())
            .map_err(|_| ErrorV4::Arithmetic)?
            .to_le_bytes(),
    )?;
    put(
        scratch,
        8,
        &u16::try_from(ranges.len())
            .map_err(|_| ErrorV4::Arithmetic)?
            .to_le_bytes(),
    )?;
    put(
        scratch,
        12,
        &u32::try_from(base_program.len())
            .map_err(|_| ErrorV4::Arithmetic)?
            .to_le_bytes(),
    )?;
    put(scratch, 16, &semantic_prefix_bytes.to_le_bytes())?;
    for (index, span) in spans.iter().copied().enumerate() {
        let offset = HEADER_BYTES_V4
            .checked_add(
                index
                    .checked_mul(DYNAMIC_SPAN_BYTES_V4)
                    .ok_or(ErrorV4::Arithmetic)?,
            )
            .ok_or(ErrorV4::Arithmetic)?;
        put(scratch, offset, &span.route.to_le_bytes())?;
        put(
            scratch,
            offset + 2,
            &span.selector_common_scalar.to_le_bytes(),
        )?;
        put(
            scratch,
            offset + 4,
            &span.base_fixed_account_count.to_le_bytes(),
        )?;
        put(scratch, offset + 8, &span.allowed_extensions.to_le_bytes())?;
    }
    let range_start = HEADER_BYTES_V4
        .checked_add(span_bytes)
        .ok_or(ErrorV4::Arithmetic)?;
    for (index, range) in ranges.iter().copied().enumerate() {
        let offset = range_start
            .checked_add(
                index
                    .checked_mul(BORROWED_RANGE_BYTES_V4)
                    .ok_or(ErrorV4::Arithmetic)?,
            )
            .ok_or(ErrorV4::Arithmetic)?;
        let (offset_kind, offset_value) = range.offset.encode();
        let (length_kind, length_value) = range.len.encode();
        put(scratch, offset, &range.route.to_le_bytes())?;
        put(scratch, offset + 2, &[offset_kind, length_kind])?;
        put(scratch, offset + 4, &offset_value.to_le_bytes())?;
        put(scratch, offset + 8, &length_value.to_le_bytes())?;
    }
    let base_start = range_start
        .checked_add(range_bytes)
        .ok_or(ErrorV4::Arithmetic)?;
    put(scratch, base_start, base_program)?;
    ProgramV4::decode(scratch)?;
    output.copy_from_slice(scratch);
    Ok(())
}

fn read_u8(bytes: &[u8], offset: usize) -> ResultV4<u8> {
    bytes.get(offset).copied().ok_or(ErrorV4::Wire)
}

fn read_u16(bytes: &[u8], offset: usize) -> ResultV4<u16> {
    slice(bytes, offset, 2)?
        .try_into()
        .map(u16::from_le_bytes)
        .map_err(|_| ErrorV4::Wire)
}

fn read_u32(bytes: &[u8], offset: usize) -> ResultV4<u32> {
    slice(bytes, offset, 4)?
        .try_into()
        .map(u32::from_le_bytes)
        .map_err(|_| ErrorV4::Wire)
}

fn read_u64(bytes: &[u8], offset: usize) -> ResultV4<u64> {
    slice(bytes, offset, 8)?
        .try_into()
        .map(u64::from_le_bytes)
        .map_err(|_| ErrorV4::Wire)
}

fn slice(bytes: &[u8], offset: usize, width: usize) -> ResultV4<&[u8]> {
    bytes
        .get(offset..offset.checked_add(width).ok_or(ErrorV4::Arithmetic)?)
        .ok_or(ErrorV4::Wire)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> ResultV4<()> {
    output
        .get_mut(offset..offset.checked_add(value.len()).ok_or(ErrorV4::Arithmetic)?)
        .ok_or(ErrorV4::Wire)?
        .copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        v2::FixedRole,
        v3::encode::{
            AccountCoordinateV3, EffectGeometryV3, EffectInstructionV3, RouteInputV3,
            ScalarCoordinateV3, encode_effect_program_v3_atomic,
        },
        v3::{HEADER_BYTES, OPERATION_BYTES, ROUTE_BYTES, RouteKindV3},
    };

    const BASE_BYTES: usize = HEADER_BYTES + 3 * ROUTE_BYTES + OPERATION_BYTES;
    const SUCCESSOR_BYTES: usize =
        HEADER_BYTES_V4 + DYNAMIC_SPAN_BYTES_V4 + 2 * BORROWED_RANGE_BYTES_V4 + BASE_BYTES;

    fn base_program() -> [u8; BASE_BYTES] {
        let routes = [
            RouteInputV3 {
                role: FixedRole::Custody,
                kind: RouteKindV3::Once,
                enable_common_scalar: None,
                witness_range_common_scalar: None,
                receipt_dependency: None,
                fixed_account_start: 0,
                fixed_account_count: 5,
                item_account_start: 0,
                item_account_count: 0,
                fixed_request: &[],
                item_request: &[],
            },
            RouteInputV3 {
                role: FixedRole::Claims,
                kind: RouteKindV3::Once,
                enable_common_scalar: None,
                witness_range_common_scalar: None,
                receipt_dependency: None,
                fixed_account_start: 5,
                fixed_account_count: 20,
                item_account_start: 0,
                item_account_count: 0,
                fixed_request: &[],
                item_request: &[],
            },
            RouteInputV3 {
                role: FixedRole::Core,
                kind: RouteKindV3::Once,
                enable_common_scalar: None,
                witness_range_common_scalar: None,
                receipt_dependency: None,
                fixed_account_start: 25,
                fixed_account_count: 1,
                item_account_start: 0,
                item_account_count: 0,
                fixed_request: &[],
                item_request: &[],
            },
        ];
        let operation = EffectInstructionV3::write_u64(
            AccountCoordinateV3::fixed(25),
            0,
            ScalarCoordinateV3::common(0),
        );
        let mut scratch = [0; BASE_BYTES];
        let mut output = [0; BASE_BYTES];
        encode_effect_program_v3_atomic(
            EffectGeometryV3 {
                fixed_accounts: 26,
                item_account_stride: 0,
                common_scalars: 6,
                item_scalar_stride: 0,
                common_identities: 1,
                item_identity_stride: 0,
            },
            &routes,
            &[operation],
            &[],
            &mut scratch,
            &mut output,
        )
        .expect("base program");
        output
    }

    fn series_successor(policy: BorrowedRangePolicyV4, routes: [u16; 2]) -> [u8; SUCCESSOR_BYTES] {
        let base = base_program();
        let span = DynamicFixedSpanV4::new(1, 0, 20, (1_u64 << 1) | (1_u64 << 2));
        let ranges = [
            BorrowedRangeV4::new(
                routes[0],
                RequestCoordinateV4::Fixed(128),
                RequestCoordinateV4::CommonScalar(1),
            ),
            BorrowedRangeV4::new(
                routes[1],
                RequestCoordinateV4::Fixed(128),
                RequestCoordinateV4::CommonScalar(1),
            ),
        ];
        let mut scratch = [0; SUCCESSOR_BYTES];
        let mut output = [0; SUCCESSOR_BYTES];
        encode_program_v4_atomic(
            &base,
            policy,
            128,
            &[span],
            &ranges,
            &mut scratch,
            &mut output,
        )
        .expect("successor");
        output
    }

    #[test]
    fn selected_span_shifts_later_accounts_exactly() {
        let bytes = series_successor(BorrowedRangePolicyV4::IdenticalReuseExactCoverage, [1, 2]);
        let program = ProgramV4::decode(&bytes).expect("decode");
        for (selected, expected_count, expected_local) in
            [(1_u64, 27_usize, 26_usize), (2_u64, 28_usize, 27_usize)]
        {
            let scalars = [selected, 32, 0, 0, 0, 0];
            let identities = [[1; 32]];
            assert_eq!(program.account_count(0, &scalars), Ok(expected_count));
            let invocation = program
                .resolved_invocation(1, 0, 0, &scalars, &identities)
                .expect("invocation");
            assert_eq!(invocation.invocation.fixed_account_start, 5);
            assert_eq!(
                invocation.invocation.fixed_account_count,
                20 + u16::try_from(selected).expect("small")
            );
            assert_eq!(invocation.borrowed_range_count(), 1);
            assert_eq!(
                program.resolved_fixed_effect(0, 0, &scalars, &identities),
                Ok(ResolvedEffectV3::WriteScalar {
                    account: expected_local,
                    offset: 0,
                    value: selected,
                })
            );
        }
    }

    #[test]
    fn zero_base_span_omits_or_materializes_one_optional_child_frame() {
        const OPTIONAL_BASE_BYTES: usize = HEADER_BYTES + 2 * ROUTE_BYTES + OPERATION_BYTES;
        const OPTIONAL_BYTES: usize = HEADER_BYTES_V4 + DYNAMIC_SPAN_BYTES_V4 + OPTIONAL_BASE_BYTES;
        let routes = [
            RouteInputV3 {
                role: FixedRole::Custody,
                kind: RouteKindV3::Once,
                enable_common_scalar: Some(0),
                witness_range_common_scalar: None,
                receipt_dependency: None,
                fixed_account_start: 0,
                fixed_account_count: 0,
                item_account_start: 0,
                item_account_count: 0,
                fixed_request: &[],
                item_request: &[],
            },
            RouteInputV3 {
                role: FixedRole::Claims,
                kind: RouteKindV3::Once,
                enable_common_scalar: None,
                witness_range_common_scalar: None,
                receipt_dependency: None,
                fixed_account_start: 0,
                fixed_account_count: 20,
                item_account_start: 0,
                item_account_count: 0,
                fixed_request: &[],
                item_request: &[],
            },
        ];
        let effect = EffectInstructionV3::write_u64(
            AccountCoordinateV3::fixed(20),
            0,
            ScalarCoordinateV3::common(0),
        );
        let mut base_scratch = [0; OPTIONAL_BASE_BYTES];
        let mut base = [0; OPTIONAL_BASE_BYTES];
        encode_effect_program_v3_atomic(
            EffectGeometryV3 {
                fixed_accounts: 21,
                item_account_stride: 0,
                common_scalars: 1,
                item_scalar_stride: 0,
                common_identities: 1,
                item_identity_stride: 0,
            },
            &routes,
            &[effect],
            &[],
            &mut base_scratch,
            &mut base,
        )
        .expect("optional base");
        let span = DynamicFixedSpanV4::new(0, 0, 0, (1_u64 << 0) | (1_u64 << 14));
        let mut scratch = [0; OPTIONAL_BYTES];
        let mut output = [0; OPTIONAL_BYTES];
        encode_program_v4_atomic(
            &base,
            BorrowedRangePolicyV4::DisjointExactCoverage,
            1,
            &[span],
            &[],
            &mut scratch,
            &mut output,
        )
        .expect("optional successor");
        let program = ProgramV4::decode(&output).expect("decode");
        assert_eq!(program.account_count(0, &[0]), Ok(21));
        assert_eq!(program.account_count(0, &[14]), Ok(35));
        assert_eq!(
            program
                .resolved_invocation(0, 0, 0, &[14], &[[1; 32]])
                .expect("enabled")
                .invocation
                .fixed_account_count,
            14
        );
        assert_eq!(
            program.resolved_fixed_effect(0, 0, &[14], &[[1; 32]]),
            Ok(ResolvedEffectV3::WriteScalar {
                account: 34,
                offset: 0,
                value: 14,
            })
        );
    }

    #[test]
    fn one_proof_range_can_feed_two_ordered_series_phases() {
        let bytes = series_successor(BorrowedRangePolicyV4::IdenticalReuseExactCoverage, [1, 2]);
        let program = ProgramV4::decode(&bytes).expect("decode");
        for proof_count in [1_u64, 16, 32] {
            let proof_bytes = proof_count.checked_mul(32).expect("bounded");
            let scalars = [1, proof_bytes, 0, 0, 0, 0];
            let identities = [[1; 32]];
            let request_len = usize::try_from(128 + proof_bytes).expect("bounded");
            assert_eq!(
                program.validate_request_coverage(request_len, 0, &scalars, &identities),
                Ok(())
            );
            let first = program
                .resolved_borrowed_range(1, 0, &scalars)
                .expect("first proof");
            let second = program
                .resolved_borrowed_range(2, 0, &scalars)
                .expect("second proof");
            assert_eq!(first, second);
            assert_eq!(first.source_offset(), 128);
            assert_eq!(first.len(), usize::try_from(proof_bytes).expect("bounded"));
            let request = [7_u8; 1152];
            let exact_request = request.get(..request_len).expect("bounded request");
            assert_eq!(first.slice(exact_request).map(<[u8]>::len), Ok(first.len()));
        }
        assert_eq!(program.validate_route_window(0, 2), Ok(()));
        assert_eq!(program.validate_route_window(2, 3), Ok(()));
        assert_eq!(
            program.validate_route_window(2, 2),
            Err(ErrorV4::RangeSelection)
        );
    }

    #[test]
    fn overlap_gap_order_and_unlisted_span_refuse() {
        let identities = [[1; 32]];
        let scalars = [1, 32, 0, 0, 0, 0];
        let disjoint = series_successor(BorrowedRangePolicyV4::DisjointExactCoverage, [1, 2]);
        assert_eq!(
            ProgramV4::decode(&disjoint)
                .expect("decode")
                .validate_request_coverage(160, 0, &scalars, &identities),
            Err(ErrorV4::RequestCoverage)
        );
        let reversed = series_successor(BorrowedRangePolicyV4::IdenticalReuseExactCoverage, [2, 1]);
        assert_eq!(
            ProgramV4::decode(&reversed)
                .expect("decode")
                .validate_request_coverage(160, 0, &scalars, &identities),
            Err(ErrorV4::RequestCoverage)
        );
        let program = ProgramV4::decode(&disjoint).expect("decode");
        assert_eq!(
            program.account_count(0, &[0, 32, 0, 0, 0, 0]),
            Err(ErrorV4::SpanSelection)
        );
        assert_eq!(
            program.account_count(0, &[3, 32, 0, 0, 0, 0]),
            Err(ErrorV4::SpanSelection)
        );
        assert_eq!(
            program.resolved_borrowed_range(1, 1, &scalars),
            Err(ErrorV4::RangeSelection)
        );
    }

    #[test]
    fn encoder_refusal_preserves_output_and_reserved_bytes_refuse() {
        let base = base_program();
        let span = DynamicFixedSpanV4::new(1, 0, 20, (1_u64 << 1) | (1_u64 << 2));
        let range = BorrowedRangeV4::new(
            1,
            RequestCoordinateV4::Fixed(128),
            RequestCoordinateV4::Fixed(320),
        );
        let mut scratch = [7; SUCCESSOR_BYTES];
        let mut output = [9; SUCCESSOR_BYTES];
        let before = output;
        assert_eq!(
            encode_program_v4_atomic(
                &base,
                BorrowedRangePolicyV4::DisjointExactCoverage,
                128,
                &[span],
                &[range],
                &mut scratch,
                &mut output,
            ),
            Err(ErrorV4::Wire)
        );
        assert_eq!(output, before);

        let mut hostile =
            series_successor(BorrowedRangePolicyV4::IdenticalReuseExactCoverage, [1, 2]);
        hostile[20] = 1;
        assert_eq!(ProgramV4::decode(&hostile), Err(ErrorV4::Wire));
        let range_reserved = HEADER_BYTES_V4 + DYNAMIC_SPAN_BYTES_V4 + 12;
        let mut hostile =
            series_successor(BorrowedRangePolicyV4::IdenticalReuseExactCoverage, [1, 2]);
        *hostile
            .get_mut(range_reserved)
            .expect("range reserved coordinate") = 1;
        assert_eq!(ProgramV4::decode(&hostile), Err(ErrorV4::Wire));
    }
}
