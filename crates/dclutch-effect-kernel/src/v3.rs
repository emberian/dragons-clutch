//! Authenticated runtime-tail physical effects and fixed-role request routes.
//!
//! A V3 program has one fixed body and one item body repeated for an
//! authenticated `u32` count. It owns exact request templates, so dynamic
//! projection patches typed fields without fabricating child wire magic or
//! static tags. All affine widths are checked before a loop, all loops are
//! bounded by decoded counts or the authenticated tail count, and caller
//! outputs change only after the complete projection accepts.

use core::convert::TryInto;

use super::v2::{AccountInput, AccountPermission, FixedRole};

/// Safe, allocation-free typed EffectProgram V3 artifact encoder.
pub mod encode;

/// Canonical runtime-tail effect-program successor magic.
pub const MAGIC: [u8; 4] = *b"DCE4";
/// Finalized-record schema label for the variable ordered receipt-dependency
/// table. The distinct release prevents a singular-dependency artifact from
/// being accepted under plural semantics.
pub const SCHEMA_RELEASE_PREIMAGE: &[u8] =
    b"dclutch/schema/effect-program-v4-ordered-receipt-dependencies-v1";
/// SHA-256 of [`SCHEMA_RELEASE_PREIMAGE`].
pub const SCHEMA_RELEASE_ID: [u8; 32] = [
    0x0b, 0x58, 0x3a, 0xe2, 0xe3, 0x07, 0x35, 0xbf, 0xcd, 0x83, 0x79, 0xe8, 0xf4, 0x42, 0x66, 0x85,
    0x9e, 0xdb, 0x7b, 0xb7, 0x12, 0xe6, 0x80, 0x34, 0x7d, 0x94, 0xc5, 0x79, 0x87, 0xe7, 0x1f, 0x18,
];
/// Canonical runtime-tail effect-program version.
pub const VERSION: u8 = 4;
/// Exact V3 header width.
pub const HEADER_BYTES: usize = 32;
/// Exact fixed-role route width.
pub const ROUTE_BYTES: usize = 32;
/// Exact width of one ordered prior-child receipt dependency entry.
pub const RECEIPT_DEPENDENCY_BYTES: usize = 8;
/// Exact effect-operation width.
pub const OPERATION_BYTES: usize = 24;

const OP_TRANSFER_LAMPORTS: u8 = 0;
const OP_WRITE_SCALAR: u8 = 1;
const OP_WRITE_IDENTITY: u8 = 2;
const OP_REQUIRE_LAMPORTS_EQ: u8 = 3;
const OP_WRITE_REQUEST_U8: u8 = 4;
const OP_WRITE_REQUEST_U16: u8 = 5;
const OP_WRITE_REQUEST_U32: u8 = 6;
const OP_WRITE_REQUEST_U64: u8 = 7;
const OP_WRITE_REQUEST_IDENTITY: u8 = 8;
const OP_WRITE_SCALAR_AFFINE: u8 = 9;
const OP_WRITE_IDENTITY_AFFINE: u8 = 10;
const OP_WRITE_DATA_U8: u8 = 11;
const OP_WRITE_DATA_U16: u8 = 12;
const OP_WRITE_DATA_U32: u8 = 13;
const OP_WRITE_DATA_U8_AFFINE: u8 = 14;
const OP_WRITE_DATA_U16_AFFINE: u8 = 15;
const OP_WRITE_DATA_U32_AFFINE: u8 = 16;

const MODE_ACCOUNT_A_ITEM: u8 = 1 << 0;
const MODE_ACCOUNT_B_ITEM: u8 = 1 << 1;
const MODE_REGISTER_ITEM: u8 = 1 << 2;
const MODE_REQUEST_ITEM: u8 = 1 << 3;
const MODE_MASK: u8 =
    MODE_ACCOUNT_A_ITEM | MODE_ACCOUNT_B_ITEM | MODE_REGISTER_ITEM | MODE_REQUEST_ITEM;

/// Stable hostile-decode or runtime-tail projection refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A descriptor-selected or authenticated content identity was zero.
    ZeroProgramIdentity,
    /// Descriptor selection and authenticated finalized content differed.
    ProgramIdentityMismatch,
    /// Bytes or caller-owned banks did not have their exact checked width.
    InvalidLength,
    /// Magic selected another effect-program family.
    InvalidMagic,
    /// Version or flags selected an unsupported profile.
    UnsupportedProfile,
    /// Reserved header, route, or operation bytes were nonzero.
    NonCanonicalReserved,
    /// The program did not expose an account or register address space.
    EmptyProgram,
    /// A route role, kind, enable mode, or geometry was noncanonical.
    InvalidRoute,
    /// A receipt dependency was forward, cross-item, disabled, or noncanonical.
    InvalidReceiptDependency,
    /// An opcode or fixed/item mode was unsupported.
    UnknownOperation,
    /// Active and inactive operation fields were noncanonical.
    NonCanonicalOperation,
    /// An account, register, route, or request coordinate was out of bounds.
    InvalidCoordinate,
    /// Expanded accounts, registers, aliases, scratch, or outputs differed.
    WidthMismatch,
    /// The authenticated alias partition was forward or cross-item.
    InvalidAlias,
    /// A transfer resolved source and destination to one physical account.
    AliasSelfTransfer,
    /// AccountProfile did not authorize the projected mutation.
    PermissionDenied,
    /// A data write exceeded the authenticated account width.
    DataOutOfBounds,
    /// A debit exceeded its projected source balance.
    InsufficientLamports,
    /// Checked value, width, balance, or offset arithmetic overflowed.
    ArithmeticOverflow,
    /// A required projected post-state was false.
    CheckFailed,
    /// Two resolved account-data or request writes overlapped.
    OverlappingWrites,
    /// A scalar could not be narrowed to the typed child field width.
    NarrowingOverflow,
}

/// Result alias for V3 effect operations.
pub type Result<T> = core::result::Result<T, Error>;

/// Canonical fixed-role invocation geometry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouteKindV3 {
    /// Invoke once with only one fixed account/request frame.
    Once,
    /// Invoke once with a fixed prefix and all authenticated item tails.
    AffineOnce,
    /// Invoke once per authenticated item in canonical item order.
    Each,
}

/// Hostile-decoded fixed-role route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteV3 {
    role: FixedRole,
    kind: RouteKindV3,
    enabled_if_nonzero: bool,
    borrows_witness: bool,
    enable_common_scalar: u16,
    fixed_account_start: u16,
    fixed_account_count: u16,
    item_account_start: u16,
    item_account_count: u16,
    witness_range_common_scalar: u16,
    fixed_request_bytes: u32,
    item_request_bytes: u32,
    receipt_dependency_start: u16,
    receipt_dependency_count: u16,
    compatibility_receipt_dependency: Option<RouteReceiptDependencyV3>,
}

/// One descriptor-authenticated dependency on an exact earlier child receipt.
///
/// Runtime resolution additionally binds the exact producer invocation,
/// selected program, execution context, request kind/digest, and receipt
/// kind/width retained by the common Trading receipt bank.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteReceiptDependencyV3 {
    producer_role: FixedRole,
    producer_route: u16,
    expected_receipt_bytes: u16,
}

impl RouteReceiptDependencyV3 {
    /// Construct one dependency. Full backward/geometry checks occur when the
    /// complete EffectProgram is hostile-decoded.
    pub const fn new(
        producer_role: FixedRole,
        producer_route: u16,
        expected_receipt_bytes: u16,
    ) -> Self {
        Self {
            producer_role,
            producer_route,
            expected_receipt_bytes,
        }
    }

    /// Expected producer role; the adapter resolves its current release-selected program.
    pub const fn producer_role(self) -> FixedRole {
        self.producer_role
    }

    /// Strictly earlier route ordinal.
    pub const fn producer_route(self) -> u16 {
        self.producer_route
    }

    /// Exact producer return-data width appended to the consumer request.
    pub const fn expected_receipt_bytes(self) -> u16 {
        self.expected_receipt_bytes
    }
}

impl RouteV3 {
    /// State-owning child role selected by the authenticated descriptor.
    pub const fn role(self) -> FixedRole {
        self.role
    }

    /// Once, affine-once, or each-item invocation geometry.
    pub const fn kind(self) -> RouteKindV3 {
        self.kind
    }

    /// Fixed-prefix account-frame start.
    pub const fn fixed_account_start(self) -> u16 {
        self.fixed_account_start
    }

    /// Fixed-prefix account-frame count.
    pub const fn fixed_account_count(self) -> u16 {
        self.fixed_account_count
    }

    /// Per-item account-template start.
    pub const fn item_account_start(self) -> u16 {
        self.item_account_start
    }

    /// Per-item account-template count.
    pub const fn item_account_count(self) -> u16 {
        self.item_account_count
    }

    /// Exact fixed request-template width.
    pub const fn fixed_request_bytes(self) -> u32 {
        self.fixed_request_bytes
    }

    /// Exact repeated item request-template width.
    pub const fn item_request_bytes(self) -> u32 {
        self.item_request_bytes
    }

    /// Whether the invocation appends an authenticated slice of the family request.
    pub const fn borrows_witness(self) -> bool {
        self.borrows_witness
    }

    /// Number of exact prior receipts appended in declared table order.
    pub const fn receipt_dependency_count(self) -> u16 {
        self.receipt_dependency_count
    }

    /// Compatibility view for capabilities with zero or one dependency.
    /// Plural consumers must use [`ProgramV3::route_receipt_dependency`].
    pub const fn receipt_dependency(self) -> Option<RouteReceiptDependencyV3> {
        self.compatibility_receipt_dependency
    }

    fn enabled(self, scalars: &[u64]) -> Result<bool> {
        if self.enabled_if_nonzero {
            Ok(*scalars
                .get(usize::from(self.enable_common_scalar))
                .ok_or(Error::InvalidCoordinate)?
                != 0)
        } else {
            Ok(true)
        }
    }
}

/// One adapter-resolved fixed-role invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedInvocationV3 {
    /// State-owning child role.
    pub role: FixedRole,
    /// Authenticated invocation geometry.
    pub kind: RouteKindV3,
    /// Item ordinal for `Each`; absent for once/affine-once.
    pub item: Option<u32>,
    /// Fixed account-frame start.
    pub fixed_account_start: u16,
    /// Fixed account-frame count.
    pub fixed_account_count: u16,
    /// First expanded item account coordinate.
    pub item_account_start: usize,
    /// Accounts from each repeated item subframe.
    pub item_account_count: u16,
    /// Distance between repeated item account subframes.
    pub item_account_stride: u16,
    /// Number of item subframes in this invocation.
    pub repeated_item_count: u32,
    /// Start in the projected flat request bank.
    pub request_offset: usize,
    /// Exact request width for this invocation.
    pub request_len: usize,
    /// Optional exact top-level family-request suffix appended after the IR-owned request.
    pub borrowed_witness: Option<BorrowedWitnessV3>,
    /// Exact ordered prior receipts selected in this invocation's item scope.
    pub receipt_dependencies: ResolvedReceiptDependenciesV3,
    /// Compatibility view for a route with exactly one dependency.
    pub receipt_dependency: Option<ResolvedReceiptDependencyV3>,
}

/// Coordinates of one route's ordered dependency subtable after invocation
/// scope has been resolved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedReceiptDependenciesV3 {
    first: u16,
    count: u16,
    producer_invocation: u32,
    expected_receipt_bytes: u32,
}

impl ResolvedReceiptDependenciesV3 {
    /// Empty dependency view for tests and adapters constructing a route that
    /// is known not to append prior receipts.
    pub const fn empty() -> Self {
        Self {
            first: 0,
            count: 0,
            producer_invocation: 0,
            expected_receipt_bytes: 0,
        }
    }

    /// Number of ordered dependency entries.
    pub const fn len(self) -> u16 {
        self.count
    }

    /// Whether this route appends no prior receipt.
    pub const fn is_empty(self) -> bool {
        self.count == 0
    }

    /// Exact sum of all receipt widths in declared append order.
    pub const fn expected_receipt_bytes(self) -> u32 {
        self.expected_receipt_bytes
    }
}

/// Runtime-resolved receipt dependency for one child invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedReceiptDependencyV3 {
    /// Expected producer role.
    pub producer_role: FixedRole,
    /// Strictly earlier producer route.
    pub producer_route: u16,
    /// Producer invocation: zero for once/affine, same item for each-item.
    pub producer_invocation: u32,
    /// Exact raw return-data width.
    pub expected_receipt_bytes: u16,
}

/// Exact borrowed witness range in the authenticated top-level family request.
///
/// The EffectProgram owns whether a route may borrow a witness and which two
/// common scalar registers provide `(offset, length)`. RequestProfile and the
/// transition own those scalar values. This lets a typed child request append
/// a variable proof without making Product tail width a second proof-count
/// authority or letting the family adapter fabricate child instruction data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BorrowedWitnessV3 {
    source_offset: usize,
    len: usize,
}

impl BorrowedWitnessV3 {
    /// Absolute offset within the family request (after the common hot envelope).
    pub const fn source_offset(self) -> usize {
        self.source_offset
    }

    /// Exact borrowed byte width, which may be zero for an empty proof.
    pub const fn len(self) -> usize {
        self.len
    }

    /// Whether the authenticated suffix is empty, as for a single-leaf proof.
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }

    /// Borrow the exact trailing range, refusing overflow, truncation, or padding.
    pub fn slice(self, family_request: &[u8]) -> Result<&[u8]> {
        let end = self
            .source_offset
            .checked_add(self.len)
            .ok_or(Error::ArithmeticOverflow)?;
        if end != family_request.len() {
            return Err(Error::InvalidCoordinate);
        }
        family_request
            .get(self.source_offset..end)
            .ok_or(Error::InvalidCoordinate)
    }
}

/// One fully register- and item-resolved local effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedEffectV3 {
    /// Conserve lamports between two AccountProfile-authorized coordinates.
    TransferLamports {
        /// Expanded source coordinate.
        source: usize,
        /// Expanded destination coordinate.
        destination: usize,
        /// Exact amount.
        amount: u64,
    },
    /// Write an exact scalar into authenticated account data.
    WriteScalar {
        /// Expanded account coordinate.
        account: usize,
        /// Byte offset.
        offset: u32,
        /// Exact scalar.
        value: u64,
    },
    /// Write an exact identity into authenticated account data.
    WriteIdentity {
        /// Expanded account coordinate.
        account: usize,
        /// Byte offset.
        offset: u32,
        /// Exact identity.
        value: [u8; 32],
    },
    /// Write one scalar narrowed to an exact byte into authenticated account data.
    WriteU8 {
        /// Expanded account coordinate.
        account: usize,
        /// Byte offset.
        offset: u32,
        /// Exact narrowed value.
        value: u8,
    },
    /// Write one scalar narrowed to an exact little-endian `u16`.
    WriteU16 {
        /// Expanded account coordinate.
        account: usize,
        /// Byte offset.
        offset: u32,
        /// Exact narrowed value.
        value: u16,
    },
    /// Write one scalar narrowed to an exact little-endian `u32`.
    WriteU32 {
        /// Expanded account coordinate.
        account: usize,
        /// Byte offset.
        offset: u32,
        /// Exact narrowed value.
        value: u32,
    },
    /// Require one projected lamport balance exactly.
    RequireLamportsEq {
        /// Expanded account coordinate.
        account: usize,
        /// Required value.
        value: u64,
    },
    /// Patch one typed child request field.
    WriteRequest {
        /// Route owning the field.
        route: u16,
        /// Absolute offset in the flat request bank.
        offset: usize,
        /// Typed projected value.
        value: RequestValueV3,
    },
}

/// Typed request-field value after checked narrowing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestValueV3 {
    /// One byte.
    U8(u8),
    /// Two little-endian bytes.
    U16(u16),
    /// Four little-endian bytes.
    U32(u32),
    /// Eight little-endian bytes.
    U64(u64),
    /// One 32-byte identity.
    Identity([u8; 32]),
}

impl RequestValueV3 {
    const fn width(self) -> usize {
        match self {
            Self::U8(_) => 1,
            Self::U16(_) => 2,
            Self::U32(_) => 4,
            Self::U64(_) => 8,
            Self::Identity(_) => 32,
        }
    }
}

/// Borrowed, hostile-decoded runtime-tail effect program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgramV3<'a> {
    route_count: u16,
    receipt_dependency_count: u16,
    fixed_operations: u16,
    item_operations: u16,
    fixed_accounts: u16,
    item_account_stride: u16,
    common_scalars: u16,
    item_scalar_stride: u16,
    common_identities: u16,
    item_identity_stride: u16,
    dependency_start: usize,
    template_start: usize,
    bytes: &'a [u8],
}

impl<'a> ProgramV3<'a> {
    /// Decode only after exact descriptor selection joins authenticated bytes.
    pub fn decode_selected(
        selected_program_id: [u8; 32],
        authenticated_program_id: [u8; 32],
        bytes: &'a [u8],
    ) -> Result<Self> {
        if selected_program_id == [0; 32] || authenticated_program_id == [0; 32] {
            return Err(Error::ZeroProgramIdentity);
        }
        if selected_program_id != authenticated_program_id {
            return Err(Error::ProgramIdentityMismatch);
        }
        Self::decode(bytes)
    }

    /// Hostile-decode and prevalidate one exact V3 program.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        let program = Self::decode_shape(bytes)?;
        program.validate_shape()?;
        Ok(program)
    }

    /// Parse only the fixed header and the derived section offsets.
    ///
    /// Every counter this view exposes is read here, from these bytes, on every
    /// path. `decode` runs this and then `validate_shape`; the sealed path runs
    /// this alone, because `validate_shape` is a pure function of bytes that a
    /// seal has already pinned by their own digest. Nothing that shapes the
    /// view is ever taken from a seal.
    pub(crate) fn decode_shape(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < HEADER_BYTES {
            return Err(Error::InvalidLength);
        }
        if bytes.get(..4) != Some(MAGIC.as_slice()) {
            return Err(Error::InvalidMagic);
        }
        if byte(bytes, 4)? != VERSION || byte(bytes, 5)? != 0 {
            return Err(Error::UnsupportedProfile);
        }
        if bytes.get(26..32) != Some([0_u8; 6].as_slice()) {
            return Err(Error::NonCanonicalReserved);
        }
        let route_count = read_u16(bytes, 6)?;
        let fixed_operations = read_u16(bytes, 8)?;
        let item_operations = read_u16(bytes, 10)?;
        let fixed_accounts = read_u16(bytes, 12)?;
        let item_account_stride = read_u16(bytes, 14)?;
        let common_scalars = read_u16(bytes, 16)?;
        let item_scalar_stride = read_u16(bytes, 18)?;
        let common_identities = read_u16(bytes, 20)?;
        let item_identity_stride = read_u16(bytes, 22)?;
        let receipt_dependency_count = read_u16(bytes, 24)?;
        if fixed_accounts == 0
            || (common_scalars == 0
                && item_scalar_stride == 0
                && common_identities == 0
                && item_identity_stride == 0)
        {
            return Err(Error::EmptyProgram);
        }
        let operations = usize::from(fixed_operations)
            .checked_add(usize::from(item_operations))
            .ok_or(Error::InvalidLength)?;
        let dependency_start = HEADER_BYTES
            .checked_add(
                usize::from(route_count)
                    .checked_mul(ROUTE_BYTES)
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)?;
        let template_start = dependency_start
            .checked_add(
                usize::from(receipt_dependency_count)
                    .checked_mul(RECEIPT_DEPENDENCY_BYTES)
                    .ok_or(Error::InvalidLength)?,
            )
            .and_then(|value| value.checked_add(operations.checked_mul(OPERATION_BYTES)?))
            .ok_or(Error::InvalidLength)?;
        if template_start > bytes.len() {
            return Err(Error::InvalidLength);
        }
        let program = Self {
            route_count,
            receipt_dependency_count,
            fixed_operations,
            item_operations,
            fixed_accounts,
            item_account_stride,
            common_scalars,
            item_scalar_stride,
            common_identities,
            item_identity_stride,
            dependency_start,
            template_start,
            bytes,
        };
        Ok(program)
    }

    /// Sweep every route, receipt dependency and operation of this program.
    ///
    /// This is the expensive half of `decode` and the only half a sealed view
    /// skips. It reads nothing but `self.bytes`, so "it accepted these bytes
    /// once" and "it accepts these bytes now" are the same proposition.
    pub(crate) fn validate_shape(self) -> Result<()> {
        let program = self;
        let bytes = self.bytes;
        let route_count = self.route_count;
        let receipt_dependency_count = self.receipt_dependency_count;
        let fixed_operations = self.fixed_operations;
        let item_operations = self.item_operations;
        let template_start = self.template_start;
        let mut template_bytes = 0_usize;
        let mut dependency_cursor = 0_u16;
        let mut route = 0_u16;
        while route < route_count {
            let decoded = program.route(route)?;
            if decoded.receipt_dependency_start != dependency_cursor {
                return Err(Error::InvalidReceiptDependency);
            }
            decoded.validate(program, route)?;
            dependency_cursor = dependency_cursor
                .checked_add(decoded.receipt_dependency_count)
                .ok_or(Error::InvalidReceiptDependency)?;
            template_bytes = template_bytes
                .checked_add(usize_from_u32(decoded.fixed_request_bytes)?)
                .and_then(|value| {
                    value.checked_add(usize_from_u32(decoded.item_request_bytes).ok()?)
                })
                .ok_or(Error::InvalidLength)?;
            route = route.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        if dependency_cursor != receipt_dependency_count {
            return Err(Error::InvalidReceiptDependency);
        }
        if template_start.checked_add(template_bytes) != Some(bytes.len()) {
            return Err(Error::InvalidLength);
        }
        let mut operation = 0_u16;
        while operation < fixed_operations {
            let decoded = program.operation(false, operation)?;
            decoded.validate(program, false)?;
            program.require_nonoverlap(false, operation, decoded)?;
            operation = operation.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        operation = 0;
        while operation < item_operations {
            let decoded = program.operation(true, operation)?;
            decoded.validate(program, true)?;
            program.require_nonoverlap(true, operation, decoded)?;
            operation = operation.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(())
    }

    /// Fixed account-prefix width.
    pub const fn fixed_account_count(self) -> u16 {
        self.fixed_accounts
    }

    /// Per-item account stride.
    pub const fn item_account_stride(self) -> u16 {
        self.item_account_stride
    }

    /// Common scalar width.
    pub const fn common_scalar_count(self) -> u16 {
        self.common_scalars
    }

    /// Per-item scalar stride.
    pub const fn item_scalar_stride(self) -> u16 {
        self.item_scalar_stride
    }

    /// Common identity width.
    pub const fn common_identity_count(self) -> u16 {
        self.common_identities
    }

    /// Per-item identity stride.
    pub const fn item_identity_stride(self) -> u16 {
        self.item_identity_stride
    }

    /// Route count.
    pub const fn route_count(self) -> u16 {
        self.route_count
    }

    /// Total descriptor-authenticated dependency entries across all routes.
    pub const fn receipt_dependency_count(self) -> u16 {
        self.receipt_dependency_count
    }

    /// Fixed operation count.
    pub const fn fixed_operation_count(self) -> u16 {
        self.fixed_operations
    }

    /// Repeated item operation count.
    pub const fn item_operation_count(self) -> u16 {
        self.item_operations
    }

    /// How many local-effect ordinals write ACCOUNT DATA at `tail_count`.
    ///
    /// The opcode alone decides. `Operation::resolve` maps each opcode to
    /// exactly one `ResolvedEffectV3` variant, and `resolved_data_range`
    /// answers `Some` for exactly the `Write*` variants -- so this counts
    /// precisely the ordinals the runtime-write overlap refusal will record,
    /// and it costs `fixed_operations + item_operations` template decodes
    /// rather than one per ordinal.
    ///
    /// A caller sizing that refusal's scratch bank wants this and not
    /// `item_operations * tail_count + fixed_operations`: a program whose
    /// hundred-odd ordinals are transfers and request writes needs a bank of
    /// two entries, and on an SBF allocator that never frees, the difference
    /// is bytes the rest of the instruction never gets back.
    pub fn data_write_operation_count(self, tail_count: u32) -> Result<usize> {
        let mut fixed_writers = 0_u64;
        let mut index = 0_u16;
        while index < self.fixed_operations {
            if self.operation(false, index)?.writes_account_data() {
                fixed_writers = fixed_writers.checked_add(1).ok_or(Error::InvalidLength)?;
            }
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        let mut item_writers = 0_u64;
        let mut index = 0_u16;
        while index < self.item_operations {
            if self.operation(true, index)?.writes_account_data() {
                item_writers = item_writers.checked_add(1).ok_or(Error::InvalidLength)?;
            }
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        let total = item_writers
            .checked_mul(u64::from(tail_count))
            .and_then(|value| value.checked_add(fixed_writers))
            .ok_or(Error::ArithmeticOverflow)?;
        usize::try_from(total).map_err(|_| Error::ArithmeticOverflow)
    }

    /// Borrow complete canonical bytes.
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Decode one authenticated route.
    pub fn route(self, index: u16) -> Result<RouteV3> {
        if index >= self.route_count {
            return Err(Error::InvalidCoordinate);
        }
        let offset = HEADER_BYTES
            .checked_add(
                usize::from(index)
                    .checked_mul(ROUTE_BYTES)
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)?;
        let mut route = RouteV3::decode(self.bytes, offset)?;
        route.compatibility_receipt_dependency = if route.receipt_dependency_count == 1 {
            Some(self.receipt_dependency(route.receipt_dependency_start)?)
        } else {
            None
        };
        Ok(route)
    }

    /// Decode one dependency in one route's declared append order.
    pub fn route_receipt_dependency(
        self,
        route_index: u16,
        dependency_index: u16,
    ) -> Result<RouteReceiptDependencyV3> {
        let route = self.route(route_index)?;
        if dependency_index >= route.receipt_dependency_count {
            return Err(Error::InvalidCoordinate);
        }
        let absolute = route
            .receipt_dependency_start
            .checked_add(dependency_index)
            .ok_or(Error::InvalidReceiptDependency)?;
        self.receipt_dependency(absolute)
    }

    /// Resolve one dependency of an already resolved invocation.
    pub fn resolved_receipt_dependency(
        self,
        dependencies: ResolvedReceiptDependenciesV3,
        dependency_index: u16,
    ) -> Result<ResolvedReceiptDependencyV3> {
        if dependency_index >= dependencies.count {
            return Err(Error::InvalidCoordinate);
        }
        let absolute = dependencies
            .first
            .checked_add(dependency_index)
            .ok_or(Error::InvalidReceiptDependency)?;
        let dependency = self.receipt_dependency(absolute)?;
        Ok(ResolvedReceiptDependencyV3 {
            producer_role: dependency.producer_role,
            producer_route: dependency.producer_route,
            producer_invocation: dependencies.producer_invocation,
            expected_receipt_bytes: dependency.expected_receipt_bytes,
        })
    }

    /// Borrow the exact fixed and repeated-item request templates owned by one route.
    ///
    /// These are authenticated program bytes, before register projection.  The
    /// accessor deliberately exposes no mutable view: admission layers may
    /// hostile-decode the child ABI skeleton while [`project_atomic`] remains
    /// the sole operation which patches runtime fields.
    pub fn route_template(self, index: u16) -> Result<(&'a [u8], &'a [u8])> {
        let route = self.route(index)?;
        let start = self.route_template_start(index)?;
        let fixed_len = usize_from_u32(route.fixed_request_bytes)?;
        let item_len = usize_from_u32(route.item_request_bytes)?;
        let fixed_end = start.checked_add(fixed_len).ok_or(Error::InvalidLength)?;
        let item_end = fixed_end
            .checked_add(item_len)
            .ok_or(Error::InvalidLength)?;
        Ok((
            self.bytes
                .get(start..fixed_end)
                .ok_or(Error::InvalidLength)?,
            self.bytes
                .get(fixed_end..item_end)
                .ok_or(Error::InvalidLength)?,
        ))
    }

    /// Exact expanded account-vector width.
    pub fn account_count(self, tail_count: u32) -> Result<usize> {
        affine_width(self.fixed_accounts, self.item_account_stride, tail_count)
    }

    /// Exact expanded scalar-bank width.
    pub fn scalar_count(self, tail_count: u32) -> Result<usize> {
        affine_width(self.common_scalars, self.item_scalar_stride, tail_count)
    }

    /// Exact expanded identity-bank width.
    pub fn identity_count(self, tail_count: u32) -> Result<usize> {
        affine_width(
            self.common_identities,
            self.item_identity_stride,
            tail_count,
        )
    }

    /// Exact flat request-bank width across every route.
    pub fn request_bytes(self, tail_count: u32) -> Result<usize> {
        let mut total = 0_usize;
        let mut index = 0_u16;
        while index < self.route_count {
            total = total
                .checked_add(route_expanded_request_bytes(
                    self.route(index)?,
                    tail_count,
                )?)
                .ok_or(Error::ArithmeticOverflow)?;
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(total)
    }

    /// Number of invocations emitted by one enabled route.
    pub fn invocation_count(
        self,
        route_index: u16,
        tail_count: u32,
        scalars: &[u64],
        identities: &[[u8; 32]],
    ) -> Result<u32> {
        self.require_register_widths(tail_count, scalars, identities)?;
        let route = self.route(route_index)?;
        if !route.enabled(scalars)? {
            return Ok(0);
        }
        match route.kind {
            RouteKindV3::Once | RouteKindV3::AffineOnce => Ok(1),
            RouteKindV3::Each => Ok(tail_count),
        }
    }

    /// Resolve one route invocation and exact request/account subframes.
    pub fn resolved_invocation(
        self,
        route_index: u16,
        invocation_index: u32,
        tail_count: u32,
        scalars: &[u64],
        identities: &[[u8; 32]],
    ) -> Result<ResolvedInvocationV3> {
        let count = self.invocation_count(route_index, tail_count, scalars, identities)?;
        if invocation_index >= count {
            return Err(Error::InvalidCoordinate);
        }
        let route = self.route(route_index)?;
        let route_request_start = self.route_request_start(route_index, tail_count)?;
        let tail_accounts_start = usize::from(self.fixed_accounts);
        let borrowed_witness = route.resolve_borrowed_witness(scalars)?;
        let receipt_dependencies = self.resolve_receipt_dependencies(
            route_index,
            invocation_index,
            tail_count,
            scalars,
            identities,
        )?;
        let receipt_dependency = if receipt_dependencies.count == 1 {
            Some(self.resolved_receipt_dependency(receipt_dependencies, 0)?)
        } else {
            None
        };
        match route.kind {
            RouteKindV3::Once => Ok(ResolvedInvocationV3 {
                role: route.role,
                kind: route.kind,
                item: None,
                fixed_account_start: route.fixed_account_start,
                fixed_account_count: route.fixed_account_count,
                item_account_start: tail_accounts_start,
                item_account_count: 0,
                item_account_stride: self.item_account_stride,
                repeated_item_count: 0,
                request_offset: route_request_start,
                request_len: usize_from_u32(route.fixed_request_bytes)?,
                borrowed_witness,
                receipt_dependencies,
                receipt_dependency,
            }),
            RouteKindV3::AffineOnce => Ok(ResolvedInvocationV3 {
                role: route.role,
                kind: route.kind,
                item: None,
                fixed_account_start: route.fixed_account_start,
                fixed_account_count: route.fixed_account_count,
                item_account_start: tail_accounts_start
                    .checked_add(usize::from(route.item_account_start))
                    .ok_or(Error::ArithmeticOverflow)?,
                item_account_count: route.item_account_count,
                item_account_stride: self.item_account_stride,
                repeated_item_count: tail_count,
                request_offset: route_request_start,
                request_len: route_expanded_request_bytes(route, tail_count)?,
                borrowed_witness,
                receipt_dependencies,
                receipt_dependency,
            }),
            RouteKindV3::Each => {
                let item_account_start = item_index(
                    self.fixed_accounts,
                    self.item_account_stride,
                    invocation_index,
                    route.item_account_start,
                )?;
                let request_len = usize_from_u32(route.item_request_bytes)?;
                let request_offset = route_request_start
                    .checked_add(
                        usize::try_from(invocation_index)
                            .map_err(|_| Error::ArithmeticOverflow)?
                            .checked_mul(request_len)
                            .ok_or(Error::ArithmeticOverflow)?,
                    )
                    .ok_or(Error::ArithmeticOverflow)?;
                Ok(ResolvedInvocationV3 {
                    role: route.role,
                    kind: route.kind,
                    item: Some(invocation_index),
                    fixed_account_start: route.fixed_account_start,
                    fixed_account_count: route.fixed_account_count,
                    item_account_start,
                    item_account_count: route.item_account_count,
                    item_account_stride: self.item_account_stride,
                    repeated_item_count: 1,
                    request_offset,
                    request_len,
                    borrowed_witness,
                    receipt_dependencies,
                    receipt_dependency,
                })
            }
        }
    }

    fn resolve_receipt_dependencies(
        self,
        route_index: u16,
        invocation_index: u32,
        tail_count: u32,
        scalars: &[u64],
        identities: &[[u8; 32]],
    ) -> Result<ResolvedReceiptDependenciesV3> {
        let route = self.route(route_index)?;
        let producer_invocation = match route.kind {
            RouteKindV3::Once | RouteKindV3::AffineOnce => 0,
            RouteKindV3::Each => invocation_index,
        };
        let mut dependency_index = 0_u16;
        let mut expected_receipt_bytes = 0_u32;
        while dependency_index < route.receipt_dependency_count {
            let dependency = self.route_receipt_dependency(route_index, dependency_index)?;
            let producer_count =
                self.invocation_count(dependency.producer_route, tail_count, scalars, identities)?;
            if producer_invocation >= producer_count {
                return Err(Error::InvalidReceiptDependency);
            }
            expected_receipt_bytes = expected_receipt_bytes
                .checked_add(u32::from(dependency.expected_receipt_bytes))
                .ok_or(Error::ArithmeticOverflow)?;
            dependency_index = dependency_index
                .checked_add(1)
                .ok_or(Error::InvalidReceiptDependency)?;
        }
        Ok(ResolvedReceiptDependenciesV3 {
            first: route.receipt_dependency_start,
            count: route.receipt_dependency_count,
            producer_invocation,
            expected_receipt_bytes,
        })
    }

    /// Resolve one fixed effect after exact expanded-bank validation.
    pub fn resolved_fixed_effect(
        self,
        index: u16,
        tail_count: u32,
        scalars: &[u64],
        identities: &[[u8; 32]],
    ) -> Result<ResolvedEffectV3> {
        self.require_register_widths(tail_count, scalars, identities)?;
        self.operation(false, index)?
            .resolve(self, None, tail_count, scalars, identities)
    }

    /// Resolve one repeated item effect after exact expanded-bank validation.
    pub fn resolved_item_effect(
        self,
        item: u32,
        index: u16,
        tail_count: u32,
        scalars: &[u64],
        identities: &[[u8; 32]],
    ) -> Result<ResolvedEffectV3> {
        if item >= tail_count {
            return Err(Error::InvalidCoordinate);
        }
        self.require_register_widths(tail_count, scalars, identities)?;
        self.operation(true, index)?
            .resolve(self, Some(item), tail_count, scalars, identities)
    }

    fn operation(self, item_body: bool, index: u16) -> Result<Operation> {
        let count = if item_body {
            self.item_operations
        } else {
            self.fixed_operations
        };
        if index >= count {
            return Err(Error::InvalidCoordinate);
        }
        let ordinal = if item_body {
            usize::from(self.fixed_operations)
                .checked_add(usize::from(index))
                .ok_or(Error::InvalidLength)?
        } else {
            usize::from(index)
        };
        Operation::decode(self.bytes, self.operation_offset(ordinal)?)
    }

    /// Byte offset of the operation table.
    fn operations_start(self) -> Result<usize> {
        HEADER_BYTES
            .checked_add(
                usize::from(self.route_count)
                    .checked_mul(ROUTE_BYTES)
                    .ok_or(Error::InvalidLength)?,
            )
            .and_then(|value| {
                value.checked_add(
                    usize::from(self.receipt_dependency_count)
                        .checked_mul(RECEIPT_DEPENDENCY_BYTES)?,
                )
            })
            .ok_or(Error::InvalidLength)
    }

    /// Byte offset of one operation by its table ordinal.
    fn operation_offset(self, ordinal: usize) -> Result<usize> {
        self.operations_start()?
            .checked_add(
                ordinal
                    .checked_mul(OPERATION_BYTES)
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)
    }

    fn require_register_widths(
        self,
        tail_count: u32,
        scalars: &[u64],
        identities: &[[u8; 32]],
    ) -> Result<()> {
        if scalars.len() == self.scalar_count(tail_count)?
            && identities.len() == self.identity_count(tail_count)?
        {
            Ok(())
        } else {
            Err(Error::WidthMismatch)
        }
    }

    fn route_request_start(self, index: u16, tail_count: u32) -> Result<usize> {
        if index >= self.route_count {
            return Err(Error::InvalidCoordinate);
        }
        let mut start = 0_usize;
        let mut prior = 0_u16;
        while prior < index {
            start = start
                .checked_add(route_expanded_request_bytes(
                    self.route(prior)?,
                    tail_count,
                )?)
                .ok_or(Error::ArithmeticOverflow)?;
            prior = prior.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(start)
    }

    fn route_template_start(self, index: u16) -> Result<usize> {
        if index >= self.route_count {
            return Err(Error::InvalidCoordinate);
        }
        let mut start = self.template_start;
        let mut prior = 0_u16;
        while prior < index {
            let route = self.route(prior)?;
            start = start
                .checked_add(usize_from_u32(route.fixed_request_bytes)?)
                .and_then(|value| value.checked_add(usize_from_u32(route.item_request_bytes).ok()?))
                .ok_or(Error::InvalidLength)?;
            prior = prior.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(start)
    }

    /// Refuse any static write range that overlaps an earlier operation's.
    ///
    /// The pairwise test is inherently quadratic, but re-decoding and
    /// re-validating every earlier operation for each pair made it quadratic in
    /// *full decodes*. Each earlier operation was already decoded and validated
    /// by this same loop before it became a left operand, so a comparison needs
    /// only the opcode plus the one or two fields its ranges are built from.
    /// An operation with no static range cannot overlap anything, so a right
    /// operand without one skips the sweep entirely.
    fn require_nonoverlap(self, item_body: bool, right_index: u16, right: Operation) -> Result<()> {
        let right_data = right.static_data_range();
        let right_request = right.static_request_range();
        if right_data.is_none() && right_request.is_none() {
            return Ok(());
        }
        let base = self.operations_start()?;
        let ordinal_base = if item_body {
            usize::from(self.fixed_operations)
        } else {
            0
        };
        let mut left_index = 0_u16;
        while left_index < right_index {
            let offset = base
                .checked_add(
                    ordinal_base
                        .checked_add(usize::from(left_index))
                        .and_then(|ordinal| ordinal.checked_mul(OPERATION_BYTES))
                        .ok_or(Error::InvalidLength)?,
                )
                .ok_or(Error::InvalidLength)?;
            let opcode = byte(self.bytes, offset)?;
            if let Some((right_account, right_start, right_width)) = right_data
                && is_static_data_write(opcode)
                && read_u16(self.bytes, add(offset, 2)?)? == right_account
                && overlaps(
                    right_start,
                    right_width,
                    read_u32(self.bytes, add(offset, 8)?)?,
                    write_width_of(opcode),
                )?
            {
                return Err(Error::OverlappingWrites);
            }
            if let Some((right_route, right_start, right_width)) = right_request
                && is_request_write(opcode)
                && read_u16(self.bytes, add(offset, 16)?)? == right_route
                && overlaps(
                    right_start,
                    right_width,
                    read_u32(self.bytes, add(offset, 8)?)?,
                    write_width_of(opcode),
                )?
            {
                return Err(Error::OverlappingWrites);
            }
            left_index = left_index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(())
    }
}

const fn is_data_write(opcode: u8) -> bool {
    matches!(
        opcode,
        OP_WRITE_SCALAR
            | OP_WRITE_IDENTITY
            | OP_WRITE_SCALAR_AFFINE
            | OP_WRITE_IDENTITY_AFFINE
            | OP_WRITE_DATA_U8
            | OP_WRITE_DATA_U16
            | OP_WRITE_DATA_U32
            | OP_WRITE_DATA_U8_AFFINE
            | OP_WRITE_DATA_U16_AFFINE
            | OP_WRITE_DATA_U32_AFFINE
    )
}

const fn is_affine_data_write(opcode: u8) -> bool {
    matches!(
        opcode,
        OP_WRITE_SCALAR_AFFINE
            | OP_WRITE_IDENTITY_AFFINE
            | OP_WRITE_DATA_U8_AFFINE
            | OP_WRITE_DATA_U16_AFFINE
            | OP_WRITE_DATA_U32_AFFINE
    )
}

const fn is_static_data_write(opcode: u8) -> bool {
    is_data_write(opcode) && !is_affine_data_write(opcode)
}

const fn is_request_write(opcode: u8) -> bool {
    matches!(
        opcode,
        OP_WRITE_REQUEST_U8
            | OP_WRITE_REQUEST_U16
            | OP_WRITE_REQUEST_U32
            | OP_WRITE_REQUEST_U64
            | OP_WRITE_REQUEST_IDENTITY
    )
}

const fn write_width_of(opcode: u8) -> u32 {
    match opcode {
        OP_WRITE_REQUEST_U8 | OP_WRITE_DATA_U8 | OP_WRITE_DATA_U8_AFFINE => 1,
        OP_WRITE_REQUEST_U16 | OP_WRITE_DATA_U16 | OP_WRITE_DATA_U16_AFFINE => 2,
        OP_WRITE_REQUEST_U32 | OP_WRITE_DATA_U32 | OP_WRITE_DATA_U32_AFFINE => 4,
        OP_WRITE_SCALAR | OP_WRITE_SCALAR_AFFINE | OP_WRITE_REQUEST_U64 => 8,
        OP_WRITE_IDENTITY | OP_WRITE_IDENTITY_AFFINE | OP_WRITE_REQUEST_IDENTITY => 32,
        _ => 0,
    }
}

impl RouteV3 {
    fn decode(bytes: &[u8], offset: usize) -> Result<Self> {
        let role = match byte(bytes, offset)? {
            0 => FixedRole::Core,
            1 => FixedRole::Claims,
            3 => FixedRole::Resolution,
            4 => FixedRole::Custody,
            _ => return Err(Error::InvalidRoute),
        };
        let kind = match byte(bytes, add(offset, 1)?)? {
            0 => RouteKindV3::Once,
            1 => RouteKindV3::AffineOnce,
            2 => RouteKindV3::Each,
            _ => return Err(Error::InvalidRoute),
        };
        let enabled_if_nonzero = match byte(bytes, add(offset, 2)?)? {
            0 => false,
            1 => true,
            _ => return Err(Error::InvalidRoute),
        };
        let borrows_witness = match byte(bytes, add(offset, 3)?)? {
            0 => false,
            1 => true,
            _ => return Err(Error::InvalidRoute),
        };
        if bytes.get(add(offset, 28)?..add(offset, 32)?) != Some([0_u8; 4].as_slice()) {
            return Err(Error::NonCanonicalReserved);
        }
        Ok(Self {
            role,
            kind,
            enabled_if_nonzero,
            borrows_witness,
            enable_common_scalar: read_u16(bytes, add(offset, 4)?)?,
            fixed_account_start: read_u16(bytes, add(offset, 6)?)?,
            fixed_account_count: read_u16(bytes, add(offset, 8)?)?,
            item_account_start: read_u16(bytes, add(offset, 10)?)?,
            item_account_count: read_u16(bytes, add(offset, 12)?)?,
            witness_range_common_scalar: read_u16(bytes, add(offset, 14)?)?,
            fixed_request_bytes: read_u32(bytes, add(offset, 16)?)?,
            item_request_bytes: read_u32(bytes, add(offset, 20)?)?,
            receipt_dependency_start: read_u16(bytes, add(offset, 24)?)?,
            receipt_dependency_count: read_u16(bytes, add(offset, 26)?)?,
            compatibility_receipt_dependency: None,
        })
    }

    fn validate(self, program: ProgramV3<'_>, route_index: u16) -> Result<()> {
        let fixed_end = self
            .fixed_account_start
            .checked_add(self.fixed_account_count)
            .ok_or(Error::InvalidRoute)?;
        let item_end = self
            .item_account_start
            .checked_add(self.item_account_count)
            .ok_or(Error::InvalidRoute)?;
        let witness_registers_valid = if self.borrows_witness {
            self.witness_range_common_scalar
                .checked_add(1)
                .is_some_and(|last| last < program.common_scalars)
                && self.kind != RouteKindV3::Each
        } else {
            self.witness_range_common_scalar == 0
        };
        let dependency_end = self
            .receipt_dependency_start
            .checked_add(self.receipt_dependency_count)
            .ok_or(Error::InvalidReceiptDependency)?;
        let mut dependency_valid = dependency_end <= program.receipt_dependency_count;
        let mut dependency_index = 0_u16;
        while dependency_valid && dependency_index < self.receipt_dependency_count {
            let dependency = program.route_receipt_dependency(route_index, dependency_index)?;
            dependency_valid = dependency.expected_receipt_bytes != 0
                && dependency.producer_route < route_index
                && program
                    .route(dependency.producer_route)
                    .is_ok_and(|producer| {
                        producer.role == dependency.producer_role && producer.kind == self.kind
                    });
            let mut prior = 0_u16;
            while dependency_valid && prior < dependency_index {
                dependency_valid = program
                    .route_receipt_dependency(route_index, prior)
                    .is_ok_and(|existing| existing.producer_route != dependency.producer_route);
                prior = prior
                    .checked_add(1)
                    .ok_or(Error::InvalidReceiptDependency)?;
            }
            dependency_index = dependency_index
                .checked_add(1)
                .ok_or(Error::InvalidReceiptDependency)?;
        }
        if fixed_end > program.fixed_accounts
            || item_end > program.item_account_stride
            || (!self.enabled_if_nonzero && self.enable_common_scalar != 0)
            || (self.enabled_if_nonzero && self.enable_common_scalar >= program.common_scalars)
            || !witness_registers_valid
            || !dependency_valid
        {
            return Err(if dependency_valid {
                Error::InvalidRoute
            } else {
                Error::InvalidReceiptDependency
            });
        }
        match self.kind {
            RouteKindV3::Once
                if self.item_account_start == 0
                    && self.item_account_count == 0
                    && self.item_request_bytes == 0 =>
            {
                Ok(())
            }
            RouteKindV3::AffineOnce => Ok(()),
            RouteKindV3::Each if self.fixed_request_bytes == 0 && self.item_request_bytes != 0 => {
                Ok(())
            }
            _ => Err(Error::InvalidRoute),
        }
    }

    fn resolve_borrowed_witness(self, scalars: &[u64]) -> Result<Option<BorrowedWitnessV3>> {
        if !self.borrows_witness {
            return Ok(None);
        }
        let offset_register = usize::from(self.witness_range_common_scalar);
        let length_register = offset_register
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let source_offset = usize::try_from(
            *scalars
                .get(offset_register)
                .ok_or(Error::InvalidCoordinate)?,
        )
        .map_err(|_| Error::ArithmeticOverflow)?;
        let len = usize::try_from(
            *scalars
                .get(length_register)
                .ok_or(Error::InvalidCoordinate)?,
        )
        .map_err(|_| Error::ArithmeticOverflow)?;
        source_offset
            .checked_add(len)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(Some(BorrowedWitnessV3 { source_offset, len }))
    }
}

impl ProgramV3<'_> {
    fn receipt_dependency(self, index: u16) -> Result<RouteReceiptDependencyV3> {
        if index >= self.receipt_dependency_count {
            return Err(Error::InvalidCoordinate);
        }
        let offset = self
            .dependency_start
            .checked_add(
                usize::from(index)
                    .checked_mul(RECEIPT_DEPENDENCY_BYTES)
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)?;
        if byte(self.bytes, add(offset, 1)?)? != 0
            || self.bytes.get(add(offset, 6)?..add(offset, 8)?) != Some([0_u8; 2].as_slice())
        {
            return Err(Error::NonCanonicalReserved);
        }
        let producer_role = match byte(self.bytes, offset)? {
            0 => FixedRole::Core,
            1 => FixedRole::Claims,
            3 => FixedRole::Resolution,
            4 => FixedRole::Custody,
            _ => return Err(Error::InvalidReceiptDependency),
        };
        Ok(RouteReceiptDependencyV3 {
            producer_role,
            producer_route: read_u16(self.bytes, add(offset, 2)?)?,
            expected_receipt_bytes: read_u16(self.bytes, add(offset, 4)?)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Operation {
    opcode: u8,
    mode: u8,
    account_a: u16,
    account_b: u16,
    register: u16,
    data_offset: u32,
    extra: u32,
    route: u16,
}

impl Operation {
    fn decode(bytes: &[u8], offset: usize) -> Result<Self> {
        if bytes.get(add(offset, 18)?..add(offset, 24)?) != Some([0_u8; 6].as_slice()) {
            return Err(Error::NonCanonicalReserved);
        }
        Ok(Self {
            opcode: byte(bytes, offset)?,
            mode: byte(bytes, add(offset, 1)?)?,
            account_a: read_u16(bytes, add(offset, 2)?)?,
            account_b: read_u16(bytes, add(offset, 4)?)?,
            register: read_u16(bytes, add(offset, 6)?)?,
            data_offset: read_u32(bytes, add(offset, 8)?)?,
            extra: read_u32(bytes, add(offset, 12)?)?,
            route: read_u16(bytes, add(offset, 16)?)?,
        })
    }

    fn validate(self, program: ProgramV3<'_>, item_body: bool) -> Result<()> {
        if self.mode & !MODE_MASK != 0 || (!item_body && self.mode != 0) {
            return Err(Error::NonCanonicalOperation);
        }
        let account_a_item = self.mode & MODE_ACCOUNT_A_ITEM != 0;
        let account_b_item = self.mode & MODE_ACCOUNT_B_ITEM != 0;
        let register_item = self.mode & MODE_REGISTER_ITEM != 0;
        let request_item = self.mode & MODE_REQUEST_ITEM != 0;
        if item_body && self.is_data_write() && !self.is_affine_data_write() && !account_a_item {
            return Err(Error::NonCanonicalOperation);
        }
        if self.is_affine_data_write()
            && (!item_body || account_a_item || !register_item || request_item)
        {
            return Err(Error::NonCanonicalOperation);
        }
        if item_body && self.is_request_write() != request_item {
            return Err(Error::NonCanonicalOperation);
        }
        if !item_body && request_item {
            return Err(Error::NonCanonicalOperation);
        }
        let identity = matches!(
            self.opcode,
            OP_WRITE_IDENTITY | OP_WRITE_REQUEST_IDENTITY | OP_WRITE_IDENTITY_AFFINE
        );
        let scalar = matches!(
            self.opcode,
            OP_TRANSFER_LAMPORTS
                | OP_WRITE_SCALAR
                | OP_WRITE_SCALAR_AFFINE
                | OP_WRITE_DATA_U8
                | OP_WRITE_DATA_U16
                | OP_WRITE_DATA_U32
                | OP_WRITE_DATA_U8_AFFINE
                | OP_WRITE_DATA_U16_AFFINE
                | OP_WRITE_DATA_U32_AFFINE
                | OP_REQUIRE_LAMPORTS_EQ
                | OP_WRITE_REQUEST_U8
                | OP_WRITE_REQUEST_U16
                | OP_WRITE_REQUEST_U32
                | OP_WRITE_REQUEST_U64
        );
        if !identity && !scalar {
            return Err(Error::UnknownOperation);
        }
        validate_coordinate(
            self.account_a,
            account_a_item,
            program.fixed_accounts,
            program.item_account_stride,
        )?;
        if self.opcode == OP_TRANSFER_LAMPORTS {
            validate_coordinate(
                self.account_b,
                account_b_item,
                program.fixed_accounts,
                program.item_account_stride,
            )?;
        } else if self.account_b != 0 || account_b_item {
            return Err(Error::NonCanonicalOperation);
        }
        validate_coordinate(
            self.register,
            register_item,
            if identity {
                program.common_identities
            } else {
                program.common_scalars
            },
            if identity {
                program.item_identity_stride
            } else {
                program.item_scalar_stride
            },
        )?;
        if self.is_request_write() {
            if self.route >= program.route_count || self.extra != 0 {
                return Err(Error::InvalidCoordinate);
            }
            let route = program.route(self.route)?;
            let width = self.write_width();
            let request_bound = if request_item {
                route.item_request_bytes
            } else {
                route.fixed_request_bytes
            };
            if self
                .data_offset
                .checked_add(width)
                .filter(|end| *end <= request_bound)
                .is_none()
            {
                return Err(Error::InvalidCoordinate);
            }
        } else {
            if self.route != 0 {
                return Err(Error::NonCanonicalOperation);
            }
            if self.is_affine_data_write() {
                if self.extra < self.write_width() {
                    return Err(Error::NonCanonicalOperation);
                }
            } else if self.extra != 0 {
                return Err(Error::NonCanonicalOperation);
            }
        }
        if matches!(self.opcode, OP_TRANSFER_LAMPORTS | OP_REQUIRE_LAMPORTS_EQ)
            && self.data_offset != 0
        {
            return Err(Error::NonCanonicalOperation);
        }
        Ok(())
    }

    const fn is_data_write(self) -> bool {
        is_data_write(self.opcode)
    }

    const fn is_affine_data_write(self) -> bool {
        is_affine_data_write(self.opcode)
    }

    const fn is_request_write(self) -> bool {
        is_request_write(self.opcode)
    }

    const fn write_width(self) -> u32 {
        write_width_of(self.opcode)
    }

    fn static_data_range(self) -> Option<(u16, u32, u32)> {
        (self.is_data_write() && !self.is_affine_data_write()).then_some((
            self.account_a,
            self.data_offset,
            self.write_width(),
        ))
    }

    fn static_request_range(self) -> Option<(u16, u32, u32)> {
        self.is_request_write()
            .then_some((self.route, self.data_offset, self.write_width()))
    }

    /// Whether [`Self::resolve`] yields a variant that writes account data.
    ///
    /// The list is the `Write*` arms of `resolve`'s opcode match, and nothing
    /// else: a lamport transfer, a lamport assertion and a request write all
    /// answer `false`. It sits beside `resolve` so the two cannot drift out of
    /// sight of each other, and
    /// `every_opcode_agrees_on_whether_it_writes_account_data` holds them to it
    /// over the whole opcode space rather than over the ones someone thought
    /// of.
    const fn writes_account_data(self) -> bool {
        matches!(
            self.opcode,
            OP_WRITE_SCALAR
                | OP_WRITE_IDENTITY
                | OP_WRITE_SCALAR_AFFINE
                | OP_WRITE_IDENTITY_AFFINE
                | OP_WRITE_DATA_U8
                | OP_WRITE_DATA_U16
                | OP_WRITE_DATA_U32
                | OP_WRITE_DATA_U8_AFFINE
                | OP_WRITE_DATA_U16_AFFINE
                | OP_WRITE_DATA_U32_AFFINE
        )
    }

    fn resolve(
        self,
        program: ProgramV3<'_>,
        item: Option<u32>,
        tail_count: u32,
        scalars: &[u64],
        identities: &[[u8; 32]],
    ) -> Result<ResolvedEffectV3> {
        let account_a = expanded_index(
            self.account_a,
            self.mode & MODE_ACCOUNT_A_ITEM != 0,
            item,
            program.fixed_accounts,
            program.item_account_stride,
        )?;
        let account_b = || {
            expanded_index(
                self.account_b,
                self.mode & MODE_ACCOUNT_B_ITEM != 0,
                item,
                program.fixed_accounts,
                program.item_account_stride,
            )
        };
        let scalar = || {
            let index = expanded_index(
                self.register,
                self.mode & MODE_REGISTER_ITEM != 0,
                item,
                program.common_scalars,
                program.item_scalar_stride,
            )?;
            scalars.get(index).copied().ok_or(Error::InvalidCoordinate)
        };
        let identity = || {
            let index = expanded_index(
                self.register,
                self.mode & MODE_REGISTER_ITEM != 0,
                item,
                program.common_identities,
                program.item_identity_stride,
            )?;
            identities
                .get(index)
                .copied()
                .ok_or(Error::InvalidCoordinate)
        };
        match self.opcode {
            OP_TRANSFER_LAMPORTS => Ok(ResolvedEffectV3::TransferLamports {
                source: account_a,
                destination: account_b()?,
                amount: scalar()?,
            }),
            OP_WRITE_SCALAR => Ok(ResolvedEffectV3::WriteScalar {
                account: account_a,
                offset: self.data_offset,
                value: scalar()?,
            }),
            OP_WRITE_IDENTITY => Ok(ResolvedEffectV3::WriteIdentity {
                account: account_a,
                offset: self.data_offset,
                value: identity()?,
            }),
            OP_WRITE_SCALAR_AFFINE => Ok(ResolvedEffectV3::WriteScalar {
                account: account_a,
                offset: self.affine_data_offset(item)?,
                value: scalar()?,
            }),
            OP_WRITE_IDENTITY_AFFINE => Ok(ResolvedEffectV3::WriteIdentity {
                account: account_a,
                offset: self.affine_data_offset(item)?,
                value: identity()?,
            }),
            OP_WRITE_DATA_U8 | OP_WRITE_DATA_U8_AFFINE => Ok(ResolvedEffectV3::WriteU8 {
                account: account_a,
                offset: if self.is_affine_data_write() {
                    self.affine_data_offset(item)?
                } else {
                    self.data_offset
                },
                value: u8::try_from(scalar()?).map_err(|_| Error::NarrowingOverflow)?,
            }),
            OP_WRITE_DATA_U16 | OP_WRITE_DATA_U16_AFFINE => Ok(ResolvedEffectV3::WriteU16 {
                account: account_a,
                offset: if self.is_affine_data_write() {
                    self.affine_data_offset(item)?
                } else {
                    self.data_offset
                },
                value: u16::try_from(scalar()?).map_err(|_| Error::NarrowingOverflow)?,
            }),
            OP_WRITE_DATA_U32 | OP_WRITE_DATA_U32_AFFINE => Ok(ResolvedEffectV3::WriteU32 {
                account: account_a,
                offset: if self.is_affine_data_write() {
                    self.affine_data_offset(item)?
                } else {
                    self.data_offset
                },
                value: u32::try_from(scalar()?).map_err(|_| Error::NarrowingOverflow)?,
            }),
            OP_REQUIRE_LAMPORTS_EQ => Ok(ResolvedEffectV3::RequireLamportsEq {
                account: account_a,
                value: scalar()?,
            }),
            OP_WRITE_REQUEST_U8
            | OP_WRITE_REQUEST_U16
            | OP_WRITE_REQUEST_U32
            | OP_WRITE_REQUEST_U64
            | OP_WRITE_REQUEST_IDENTITY => {
                let request_item = self.mode & MODE_REQUEST_ITEM != 0;
                let offset = program.request_write_offset(
                    self.route,
                    request_item,
                    item,
                    tail_count,
                    self.data_offset,
                )?;
                let value = match self.opcode {
                    OP_WRITE_REQUEST_U8 => RequestValueV3::U8(
                        u8::try_from(scalar()?).map_err(|_| Error::NarrowingOverflow)?,
                    ),
                    OP_WRITE_REQUEST_U16 => RequestValueV3::U16(
                        u16::try_from(scalar()?).map_err(|_| Error::NarrowingOverflow)?,
                    ),
                    OP_WRITE_REQUEST_U32 => RequestValueV3::U32(
                        u32::try_from(scalar()?).map_err(|_| Error::NarrowingOverflow)?,
                    ),
                    OP_WRITE_REQUEST_U64 => RequestValueV3::U64(scalar()?),
                    OP_WRITE_REQUEST_IDENTITY => RequestValueV3::Identity(identity()?),
                    _ => return Err(Error::UnknownOperation),
                };
                Ok(ResolvedEffectV3::WriteRequest {
                    route: self.route,
                    offset,
                    value,
                })
            }
            _ => Err(Error::UnknownOperation),
        }
    }

    fn affine_data_offset(self, item: Option<u32>) -> Result<u32> {
        let item = item.ok_or(Error::InvalidCoordinate)?;
        let offset = item
            .checked_mul(self.extra)
            .and_then(|relative| self.data_offset.checked_add(relative))
            .ok_or(Error::ArithmeticOverflow)?;
        offset
            .checked_add(self.write_width())
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(offset)
    }
}

impl ProgramV3<'_> {
    fn request_write_offset(
        self,
        route_index: u16,
        item_space: bool,
        item: Option<u32>,
        tail_count: u32,
        local_offset: u32,
    ) -> Result<usize> {
        let route = self.route(route_index)?;
        let mut offset = self.route_request_start(route_index, tail_count)?;
        if item_space {
            let item = item.ok_or(Error::InvalidCoordinate)?;
            if item >= tail_count {
                return Err(Error::InvalidCoordinate);
            }
            offset = offset
                .checked_add(usize_from_u32(route.fixed_request_bytes)?)
                .and_then(|value| {
                    value.checked_add(
                        usize::try_from(item)
                            .ok()?
                            .checked_mul(usize_from_u32(route.item_request_bytes).ok()?)?,
                    )
                })
                .ok_or(Error::ArithmeticOverflow)?;
        }
        offset
            .checked_add(usize_from_u32(local_offset)?)
            .ok_or(Error::ArithmeticOverflow)
    }
}

/// Caller-owned banks for failure-atomic V3 projection.
pub struct ProjectionV3<'a> {
    /// Exact TransitionVM scalar output.
    pub scalars: &'a [u64],
    /// Exact TransitionVM identity output.
    pub identities: &'a [[u8; 32]],
    /// Canonical representative coordinate for every expanded account.
    pub aliases: &'a [usize],
    /// Immutable physical account facts.
    pub accounts: &'a [AccountInput],
    /// Mutation permissions derived from authenticated AccountProfile.
    pub permissions: &'a [AccountPermission],
    /// Caller scratch lamports; may change on refusal.
    pub scratch_lamports: &'a mut [u64],
    /// Candidate lamports; changed only on success.
    pub output_lamports: &'a mut [u64],
    /// Caller request bank; may change on refusal.
    ///
    /// Unlike the lamport pair, this is one bank and not two. Every write a
    /// projection makes lands here, and the routes are initialized here before
    /// the first fallible effect runs, so a refusal can leave partial route
    /// bytes behind. A caller that needs the previous contents to survive a
    /// refusal must keep its own copy; every first-party caller allocates this
    /// bank fresh per projection and discards it on refusal, and on SBF the
    /// second bank was a verbatim end-of-projection copy charged in full
    /// against a heap whose allocator never frees.
    pub requests: &'a mut [u8],
}

/// Project all local effects and typed request routes atomically.
pub fn project_atomic(
    program: ProgramV3<'_>,
    tail_count: u32,
    mut projection: ProjectionV3<'_>,
) -> Result<()> {
    program.require_register_widths(tail_count, projection.scalars, projection.identities)?;
    let accounts = program.account_count(tail_count)?;
    let request_bytes = program.request_bytes(tail_count)?;
    if projection.aliases.len() != accounts
        || projection.accounts.len() != accounts
        || projection.permissions.len() != accounts
        || projection.scratch_lamports.len() != accounts
        || projection.output_lamports.len() != accounts
        || projection.requests.len() != request_bytes
    {
        return Err(Error::WidthMismatch);
    }
    validate_aliases(
        program,
        projection.aliases,
        projection.accounts,
        projection.permissions,
    )?;
    validate_runtime_write_nonoverlap(
        program,
        tail_count,
        projection.scalars,
        projection.identities,
        projection.aliases,
    )?;
    initialize_requests(program, tail_count, projection.requests)?;
    for (output, input) in projection
        .scratch_lamports
        .iter_mut()
        .zip(projection.accounts)
    {
        *output = input.lamports;
    }
    let mut fixed = 0_u16;
    while fixed < program.fixed_operations {
        let effect = program.resolved_fixed_effect(
            fixed,
            tail_count,
            projection.scalars,
            projection.identities,
        )?;
        project_effect(effect, &mut projection)?;
        fixed = fixed.checked_add(1).ok_or(Error::InvalidLength)?;
    }
    let mut item = 0_u32;
    while item < tail_count {
        let mut operation = 0_u16;
        while operation < program.item_operations {
            let effect = program.resolved_item_effect(
                item,
                operation,
                tail_count,
                projection.scalars,
                projection.identities,
            )?;
            project_effect(effect, &mut projection)?;
            operation = operation.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        item = item.checked_add(1).ok_or(Error::InvalidLength)?;
    }
    for (index, output) in projection.output_lamports.iter_mut().enumerate() {
        *output = *projection
            .scratch_lamports
            .get(*projection.aliases.get(index).ok_or(Error::InvalidAlias)?)
            .ok_or(Error::InvalidAlias)?;
    }
    Ok(())
}

pub(super) fn initialize_requests(
    program: ProgramV3<'_>,
    tail_count: u32,
    output: &mut [u8],
) -> Result<()> {
    let mut route_index = 0_u16;
    while route_index < program.route_count {
        let route = program.route(route_index)?;
        let template_start = program.route_template_start(route_index)?;
        let fixed_width = usize_from_u32(route.fixed_request_bytes)?;
        let item_width = usize_from_u32(route.item_request_bytes)?;
        let request_start = program.route_request_start(route_index, tail_count)?;
        copy_range(
            output,
            request_start,
            program.bytes,
            template_start,
            fixed_width,
        )?;
        let item_template_start = template_start
            .checked_add(fixed_width)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut item = 0_u32;
        while item < tail_count {
            let destination = request_start
                .checked_add(fixed_width)
                .and_then(|value| {
                    value.checked_add(usize::try_from(item).ok()?.checked_mul(item_width)?)
                })
                .ok_or(Error::ArithmeticOverflow)?;
            copy_range(
                output,
                destination,
                program.bytes,
                item_template_start,
                item_width,
            )?;
            item = item.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        route_index = route_index.checked_add(1).ok_or(Error::InvalidLength)?;
    }
    Ok(())
}

pub(super) fn project_effect(
    effect: ResolvedEffectV3,
    projection: &mut ProjectionV3<'_>,
) -> Result<()> {
    match effect {
        ResolvedEffectV3::TransferLamports {
            source,
            destination,
            amount,
        } => {
            let source = representative(projection.aliases, source)?;
            let destination = representative(projection.aliases, destination)?;
            if source == destination {
                return Err(Error::AliasSelfTransfer);
            }
            if !projection
                .permissions
                .get(source)
                .ok_or(Error::InvalidCoordinate)?
                .may_debit_lamports()
                || !projection
                    .permissions
                    .get(destination)
                    .ok_or(Error::InvalidCoordinate)?
                    .may_credit_lamports()
            {
                return Err(Error::PermissionDenied);
            }
            let source_before = *projection
                .scratch_lamports
                .get(source)
                .ok_or(Error::InvalidCoordinate)?;
            let destination_before = *projection
                .scratch_lamports
                .get(destination)
                .ok_or(Error::InvalidCoordinate)?;
            let source_after = source_before
                .checked_sub(amount)
                .ok_or(Error::InsufficientLamports)?;
            let destination_after = destination_before
                .checked_add(amount)
                .ok_or(Error::ArithmeticOverflow)?;
            *projection
                .scratch_lamports
                .get_mut(source)
                .ok_or(Error::InvalidCoordinate)? = source_after;
            *projection
                .scratch_lamports
                .get_mut(destination)
                .ok_or(Error::InvalidCoordinate)? = destination_after;
            Ok(())
        }
        ResolvedEffectV3::WriteScalar {
            account, offset, ..
        } => validate_data_write(account, offset, 8, projection),
        ResolvedEffectV3::WriteIdentity {
            account, offset, ..
        } => validate_data_write(account, offset, 32, projection),
        ResolvedEffectV3::WriteU8 {
            account, offset, ..
        } => validate_data_write(account, offset, 1, projection),
        ResolvedEffectV3::WriteU16 {
            account, offset, ..
        } => validate_data_write(account, offset, 2, projection),
        ResolvedEffectV3::WriteU32 {
            account, offset, ..
        } => validate_data_write(account, offset, 4, projection),
        ResolvedEffectV3::RequireLamportsEq { account, value } => {
            let account = representative(projection.aliases, account)?;
            if projection.scratch_lamports.get(account).copied() == Some(value) {
                Ok(())
            } else {
                Err(Error::CheckFailed)
            }
        }
        ResolvedEffectV3::WriteRequest { offset, value, .. } => {
            write_request(projection.requests, offset, value)
        }
    }
}

fn validate_data_write(
    account: usize,
    offset: u32,
    width: usize,
    projection: &ProjectionV3<'_>,
) -> Result<()> {
    let account = representative(projection.aliases, account)?;
    if !projection
        .permissions
        .get(account)
        .ok_or(Error::InvalidCoordinate)?
        .may_write_data()
    {
        return Err(Error::PermissionDenied);
    }
    let start = usize_from_u32(offset)?;
    let end = start.checked_add(width).ok_or(Error::DataOutOfBounds)?;
    if end
        <= projection
            .accounts
            .get(account)
            .ok_or(Error::InvalidCoordinate)?
            .data_len
    {
        Ok(())
    } else {
        Err(Error::DataOutOfBounds)
    }
}

fn write_request(output: &mut [u8], offset: usize, value: RequestValueV3) -> Result<()> {
    let end = offset
        .checked_add(value.width())
        .ok_or(Error::ArithmeticOverflow)?;
    let destination = output
        .get_mut(offset..end)
        .ok_or(Error::InvalidCoordinate)?;
    match value {
        RequestValueV3::U8(value) => destination.copy_from_slice(&[value]),
        RequestValueV3::U16(value) => destination.copy_from_slice(&value.to_le_bytes()),
        RequestValueV3::U32(value) => destination.copy_from_slice(&value.to_le_bytes()),
        RequestValueV3::U64(value) => destination.copy_from_slice(&value.to_le_bytes()),
        RequestValueV3::Identity(value) => destination.copy_from_slice(&value),
    }
    Ok(())
}

fn validate_aliases(
    program: ProgramV3<'_>,
    aliases: &[usize],
    accounts: &[AccountInput],
    permissions: &[AccountPermission],
) -> Result<()> {
    for coordinate in 0..aliases.len() {
        let resolved = representative(aliases, coordinate)?;
        if representative(aliases, resolved)? != resolved
            || !alias_region_accepts(program, coordinate, resolved)?
            || accounts.get(coordinate) != accounts.get(resolved)
            || permissions.get(coordinate) != permissions.get(resolved)
        {
            return Err(Error::InvalidAlias);
        }
    }
    Ok(())
}

fn alias_region_accepts(
    program: ProgramV3<'_>,
    coordinate: usize,
    representative: usize,
) -> Result<bool> {
    let fixed = usize::from(program.fixed_accounts);
    if coordinate < fixed {
        return Ok(representative < fixed);
    }
    if representative < fixed {
        return Ok(true);
    }
    let stride = usize::from(program.item_account_stride);
    if stride == 0 {
        return Err(Error::InvalidAlias);
    }
    let item = coordinate.checked_sub(fixed).ok_or(Error::InvalidAlias)? / stride;
    let representative_item = representative
        .checked_sub(fixed)
        .ok_or(Error::InvalidAlias)?
        / stride;
    Ok(item == representative_item)
}

pub(super) fn representative(aliases: &[usize], coordinate: usize) -> Result<usize> {
    let resolved = *aliases.get(coordinate).ok_or(Error::InvalidAlias)?;
    if resolved <= coordinate && resolved < aliases.len() {
        Ok(resolved)
    } else {
        Err(Error::InvalidAlias)
    }
}

fn validate_runtime_write_nonoverlap(
    program: ProgramV3<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    aliases: &[usize],
) -> Result<()> {
    let total = u64::from(program.item_operations)
        .checked_mul(u64::from(tail_count))
        .and_then(|value| value.checked_add(u64::from(program.fixed_operations)))
        .ok_or(Error::ArithmeticOverflow)?;
    let mut right = 0_u64;
    while right < total {
        if let Some((right_account, right_start, right_width)) = resolved_data_range(
            resolved_by_ordinal(program, right, tail_count, scalars, identities)?,
            aliases,
        )? {
            let mut left = 0_u64;
            while left < right {
                let write_overlap = if let Some((left_account, left_start, left_width)) =
                    resolved_data_range(
                        resolved_by_ordinal(program, left, tail_count, scalars, identities)?,
                        aliases,
                    )? {
                    left_account == right_account
                        && overlaps(left_start, left_width, right_start, right_width)?
                } else {
                    false
                };
                if write_overlap {
                    return Err(Error::OverlappingWrites);
                }
                left = left.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
            }
        }
        right = right.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(())
}

fn resolved_by_ordinal(
    program: ProgramV3<'_>,
    ordinal: u64,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
) -> Result<ResolvedEffectV3> {
    if ordinal < u64::from(program.fixed_operations) {
        return program.resolved_fixed_effect(
            u16::try_from(ordinal).map_err(|_| Error::InvalidCoordinate)?,
            tail_count,
            scalars,
            identities,
        );
    }
    if program.item_operations == 0 {
        return Err(Error::InvalidCoordinate);
    }
    let tail_ordinal = ordinal
        .checked_sub(u64::from(program.fixed_operations))
        .ok_or(Error::InvalidCoordinate)?;
    let item = tail_ordinal / u64::from(program.item_operations);
    let operation = tail_ordinal % u64::from(program.item_operations);
    program.resolved_item_effect(
        u32::try_from(item).map_err(|_| Error::InvalidCoordinate)?,
        u16::try_from(operation).map_err(|_| Error::InvalidCoordinate)?,
        tail_count,
        scalars,
        identities,
    )
}

pub(super) fn resolved_data_range(
    effect: ResolvedEffectV3,
    aliases: &[usize],
) -> Result<Option<(usize, u32, u32)>> {
    match effect {
        ResolvedEffectV3::WriteScalar {
            account, offset, ..
        } => Ok(Some((representative(aliases, account)?, offset, 8))),
        ResolvedEffectV3::WriteIdentity {
            account, offset, ..
        } => Ok(Some((representative(aliases, account)?, offset, 32))),
        ResolvedEffectV3::WriteU8 {
            account, offset, ..
        } => Ok(Some((representative(aliases, account)?, offset, 1))),
        ResolvedEffectV3::WriteU16 {
            account, offset, ..
        } => Ok(Some((representative(aliases, account)?, offset, 2))),
        ResolvedEffectV3::WriteU32 {
            account, offset, ..
        } => Ok(Some((representative(aliases, account)?, offset, 4))),
        _ => Ok(None),
    }
}

fn route_expanded_request_bytes(route: RouteV3, tail_count: u32) -> Result<usize> {
    let width = u64::from(route.item_request_bytes)
        .checked_mul(u64::from(tail_count))
        .and_then(|value| value.checked_add(u64::from(route.fixed_request_bytes)))
        .ok_or(Error::ArithmeticOverflow)?;
    usize::try_from(width).map_err(|_| Error::ArithmeticOverflow)
}

fn affine_width(common: u16, stride: u16, count: u32) -> Result<usize> {
    let width = u64::from(stride)
        .checked_mul(u64::from(count))
        .and_then(|value| value.checked_add(u64::from(common)))
        .ok_or(Error::ArithmeticOverflow)?;
    usize::try_from(width).map_err(|_| Error::ArithmeticOverflow)
}

fn item_index(common: u16, stride: u16, item: u32, local: u16) -> Result<usize> {
    expanded_index(local, true, Some(item), common, stride)
}

fn expanded_index(
    local: u16,
    item_space: bool,
    item: Option<u32>,
    common: u16,
    stride: u16,
) -> Result<usize> {
    if !item_space {
        return if local < common {
            Ok(usize::from(local))
        } else {
            Err(Error::InvalidCoordinate)
        };
    }
    if local >= stride {
        return Err(Error::InvalidCoordinate);
    }
    let item = item.ok_or(Error::InvalidCoordinate)?;
    let value = u64::from(item)
        .checked_mul(u64::from(stride))
        .and_then(|value| value.checked_add(u64::from(common)))
        .and_then(|value| value.checked_add(u64::from(local)))
        .ok_or(Error::ArithmeticOverflow)?;
    usize::try_from(value).map_err(|_| Error::ArithmeticOverflow)
}

fn validate_coordinate(coordinate: u16, item_space: bool, common: u16, stride: u16) -> Result<()> {
    let bound = if item_space { stride } else { common };
    if coordinate < bound {
        Ok(())
    } else {
        Err(Error::InvalidCoordinate)
    }
}

pub(super) fn overlaps(left: u32, left_width: u32, right: u32, right_width: u32) -> Result<bool> {
    let left_end = left
        .checked_add(left_width)
        .ok_or(Error::ArithmeticOverflow)?;
    let right_end = right
        .checked_add(right_width)
        .ok_or(Error::ArithmeticOverflow)?;
    Ok(left < right_end && right < left_end)
}

fn copy_range(
    destination: &mut [u8],
    destination_start: usize,
    source: &[u8],
    source_start: usize,
    width: usize,
) -> Result<()> {
    let destination_end = destination_start
        .checked_add(width)
        .ok_or(Error::ArithmeticOverflow)?;
    let source_end = source_start
        .checked_add(width)
        .ok_or(Error::ArithmeticOverflow)?;
    destination
        .get_mut(destination_start..destination_end)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(
            source
                .get(source_start..source_end)
                .ok_or(Error::InvalidLength)?,
        );
    Ok(())
}

fn usize_from_u32(value: u32) -> Result<usize> {
    usize::try_from(value).map_err(|_| Error::ArithmeticOverflow)
}

fn add(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right).ok_or(Error::InvalidLength)
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    let end = offset.checked_add(2).ok_or(Error::InvalidLength)?;
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..end)
            .ok_or(Error::InvalidLength)?
            .try_into()
            .map_err(|_| Error::InvalidLength)?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let end = offset.checked_add(4).ok_or(Error::InvalidLength)?;
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..end)
            .ok_or(Error::InvalidLength)?
            .try_into()
            .map_err(|_| Error::InvalidLength)?,
    ))
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec;
    use std::vec::Vec;

    use super::*;

    fn put(output: &mut [u8], offset: usize, bytes: &[u8]) {
        let end = offset.checked_add(bytes.len()).expect("fixture width");
        output
            .get_mut(offset..end)
            .expect("fixture slice")
            .copy_from_slice(bytes);
    }

    #[allow(clippy::too_many_arguments)]
    fn route(
        role: u8,
        kind: u8,
        fixed_account_start: u16,
        fixed_account_count: u16,
        item_account_start: u16,
        item_account_count: u16,
        fixed_request_bytes: u32,
        item_request_bytes: u32,
    ) -> [u8; ROUTE_BYTES] {
        let mut output = [0_u8; ROUTE_BYTES];
        output[0] = role;
        output[1] = kind;
        put(&mut output, 6, &fixed_account_start.to_le_bytes());
        put(&mut output, 8, &fixed_account_count.to_le_bytes());
        put(&mut output, 10, &item_account_start.to_le_bytes());
        put(&mut output, 12, &item_account_count.to_le_bytes());
        put(&mut output, 16, &fixed_request_bytes.to_le_bytes());
        put(&mut output, 20, &item_request_bytes.to_le_bytes());
        output
    }

    fn operation(
        opcode: u8,
        mode: u8,
        account_a: u16,
        account_b: u16,
        register: u16,
        data_offset: u32,
        route: u16,
    ) -> [u8; OPERATION_BYTES] {
        operation_with_extra(
            opcode,
            mode,
            account_a,
            account_b,
            register,
            data_offset,
            0,
            route,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn operation_with_extra(
        opcode: u8,
        mode: u8,
        account_a: u16,
        account_b: u16,
        register: u16,
        data_offset: u32,
        extra: u32,
        route: u16,
    ) -> [u8; OPERATION_BYTES] {
        let mut output = [0_u8; OPERATION_BYTES];
        output[0] = opcode;
        output[1] = mode;
        put(&mut output, 2, &account_a.to_le_bytes());
        put(&mut output, 4, &account_b.to_le_bytes());
        put(&mut output, 6, &register.to_le_bytes());
        put(&mut output, 8, &data_offset.to_le_bytes());
        put(&mut output, 12, &extra.to_le_bytes());
        put(&mut output, 16, &route.to_le_bytes());
        output
    }

    fn affine_data_program(scalar_stride: u32, identity_base: u32) -> Vec<u8> {
        let operations = [
            operation_with_extra(
                OP_WRITE_SCALAR_AFFINE,
                MODE_REGISTER_ITEM,
                0,
                0,
                0,
                4,
                scalar_stride,
                0,
            ),
            operation_with_extra(
                OP_WRITE_IDENTITY_AFFINE,
                MODE_REGISTER_ITEM,
                0,
                0,
                0,
                identity_base,
                40,
                0,
            ),
        ];
        let mut output = vec![0_u8; HEADER_BYTES + operations.len() * OPERATION_BYTES];
        put(&mut output, 0, &MAGIC);
        *output.get_mut(4).expect("version") = VERSION;
        for (offset, value) in [(10, 2_u16), (12, 1), (18, 1), (22, 1)] {
            put(&mut output, offset, &value.to_le_bytes());
        }
        let mut cursor = HEADER_BYTES;
        for value in operations {
            put(&mut output, cursor, &value);
            cursor += OPERATION_BYTES;
        }
        output
    }

    fn canonical() -> Vec<u8> {
        let routes = [route(1, 1, 0, 1, 0, 1, 8, 8), route(4, 2, 0, 1, 1, 1, 0, 8)];
        let operations = [
            operation(OP_WRITE_REQUEST_U32, 0, 0, 0, 0, 0, 0),
            operation(
                OP_TRANSFER_LAMPORTS,
                MODE_ACCOUNT_A_ITEM | MODE_REGISTER_ITEM,
                0,
                0,
                1,
                0,
                0,
            ),
            operation(
                OP_WRITE_REQUEST_U64,
                MODE_REGISTER_ITEM | MODE_REQUEST_ITEM,
                0,
                0,
                1,
                0,
                0,
            ),
            operation(
                OP_WRITE_REQUEST_U64,
                MODE_REGISTER_ITEM | MODE_REQUEST_ITEM,
                0,
                0,
                1,
                0,
                1,
            ),
        ];
        let templates = [
            b"CLAIMFIX".as_slice(),
            b"CLAIMITM".as_slice(),
            b"CUSTITEM".as_slice(),
        ];
        let len = HEADER_BYTES
            + routes.len() * ROUTE_BYTES
            + operations.len() * OPERATION_BYTES
            + templates.iter().map(|value| value.len()).sum::<usize>();
        let mut output = vec![0_u8; len];
        put(&mut output, 0, &MAGIC);
        *output.get_mut(4).expect("version") = VERSION;
        for (offset, value) in [
            (6, 2_u16),
            (8, 1),
            (10, 3),
            (12, 1),
            (14, 2),
            (16, 1),
            (18, 2),
            (20, 1),
            (22, 1),
        ] {
            put(&mut output, offset, &value.to_le_bytes());
        }
        let mut cursor = HEADER_BYTES;
        for value in routes {
            put(&mut output, cursor, &value);
            cursor += ROUTE_BYTES;
        }
        for value in operations {
            put(&mut output, cursor, &value);
            cursor += OPERATION_BYTES;
        }
        for value in templates {
            put(&mut output, cursor, value);
            cursor += value.len();
        }
        output
    }

    /// The opcode predicate that sizes the runtime-write scratch bank has to
    /// agree with what `resolve` ACTUALLY produces, over the whole opcode
    /// space -- not over the opcodes someone remembered when they wrote it. A
    /// disagreement is silent in both directions: too small a bank refuses a
    /// valid program, too large a bank spends heap that an allocator which
    /// never frees does not give back.
    ///
    /// So this asks `resolve` and `resolved_data_range`, which are the two
    /// functions whose behaviour the predicate is a claim about, rather than
    /// restating the predicate.
    #[test]
    fn every_opcode_agrees_on_whether_it_writes_account_data() {
        let bytes = canonical();
        let program = ProgramV3::decode(&bytes).expect("canonical program");
        let scalars = [0_u64; 5];
        let identities = [[0_u8; 32]; 3];
        let aliases = core::array::from_fn::<_, 8, _>(|index| index);
        let mut resolvable = 0_usize;
        for opcode in 0..=u8::MAX {
            let operation = Operation {
                opcode,
                mode: 0,
                account_a: 0,
                account_b: 0,
                register: 0,
                route: 0,
                data_offset: 0,
                extra: 0,
            };
            let Ok(resolved) = operation.resolve(program, Some(0), 1, &scalars, &identities) else {
                // An opcode this program cannot resolve records no range, so
                // the predicate must not reserve an entry for it.
                assert!(
                    !operation.writes_account_data(),
                    "opcode {opcode} is claimed to write data but does not resolve"
                );
                continue;
            };
            resolvable = resolvable.checked_add(1).expect("small");
            assert_eq!(
                operation.writes_account_data(),
                resolved_data_range(resolved, &aliases)
                    .expect("range")
                    .is_some(),
                "opcode {opcode} disagrees with what it resolves to"
            );
        }
        // Every declared opcode resolves under this fixture, so the agreement
        // above is over the whole opcode set and not over an accidental
        // handful.
        assert_eq!(resolvable, 17);
    }

    #[test]
    fn affine_and_each_requests_project_atomically() {
        let bytes = canonical();
        let program = ProgramV3::decode(&bytes).expect("program");
        assert_eq!(
            program.route_template(0),
            Ok((b"CLAIMFIX".as_slice(), b"CLAIMITM".as_slice()))
        );
        assert_eq!(
            program.route_template(1),
            Ok((&[][..], b"CUSTITEM".as_slice()))
        );
        assert_eq!(program.route_template(2), Err(Error::InvalidCoordinate));
        let scalars = [9_u64, 0, 3, 1, 4];
        let identities = [[0_u8; 32]; 3];
        let accounts = [
            AccountInput {
                lamports: 10,
                data_len: 0,
            },
            AccountInput {
                lamports: 3,
                data_len: 0,
            },
            AccountInput {
                lamports: 0,
                data_len: 0,
            },
            AccountInput {
                lamports: 4,
                data_len: 0,
            },
            AccountInput {
                lamports: 0,
                data_len: 0,
            },
        ];
        let permissions = [
            AccountPermission::lamport_receiver(),
            AccountPermission::new(true, false, false),
            AccountPermission::read_only(),
            AccountPermission::new(true, false, false),
            AccountPermission::read_only(),
        ];
        let aliases = [0, 1, 2, 3, 4];
        let mut scratch_lamports = [0_u64; 5];
        let mut output_lamports = [99_u64; 5];
        let mut requests = [0x55_u8; 40];
        project_atomic(
            program,
            2,
            ProjectionV3 {
                scalars: &scalars,
                identities: &identities,
                aliases: &aliases,
                accounts: &accounts,
                permissions: &permissions,
                scratch_lamports: &mut scratch_lamports,
                output_lamports: &mut output_lamports,
                requests: &mut requests,
            },
        )
        .expect("projection");
        assert_eq!(output_lamports, [17, 0, 0, 0, 0]);
        // Seeded 0x55 and asserted in full: the single bank is initialized and
        // written across its whole declared width, so no caller byte survives
        // into a projected request.
        assert_eq!(&requests[0..4], &9_u32.to_le_bytes());
        assert_eq!(&requests[4..8], b"MFIX");
        assert_eq!(&requests[8..16], &3_u64.to_le_bytes());
        assert_eq!(&requests[16..24], &4_u64.to_le_bytes());
        assert_eq!(&requests[24..32], &3_u64.to_le_bytes());
        assert_eq!(&requests[32..40], &4_u64.to_le_bytes());
        assert_eq!(
            program
                .resolved_invocation(0, 0, 2, &scalars, &identities)
                .expect("claims")
                .request_len,
            24
        );
        assert_eq!(program.invocation_count(1, 2, &scalars, &identities), Ok(2));
        assert_eq!(
            program
                .resolved_invocation(1, 1, 2, &scalars, &identities)
                .expect("custody")
                .request_offset,
            32
        );
    }

    #[test]
    fn borrowed_witness_is_an_exact_authenticated_suffix() {
        let mut core = route(0, 0, 0, 1, 0, 0, 8, 0);
        core[3] = 1;
        put(&mut core, 14, &0_u16.to_le_bytes());
        let mut bytes = vec![0_u8; HEADER_BYTES + ROUTE_BYTES + 8];
        put(&mut bytes, 0, &MAGIC);
        *bytes.get_mut(4).expect("version") = VERSION;
        for (offset, value) in [(6, 1_u16), (12, 1), (16, 2)] {
            put(&mut bytes, offset, &value.to_le_bytes());
        }
        put(&mut bytes, HEADER_BYTES, &core);
        put(&mut bytes, HEADER_BYTES + ROUTE_BYTES, b"CORE_REQ");

        let program = ProgramV3::decode(&bytes).expect("borrowed-witness program");
        let invocation = program
            .resolved_invocation(0, 0, 0, &[8, 4], &[])
            .expect("invocation");
        let witness = invocation.borrowed_witness.expect("borrowed witness");
        assert_eq!(invocation.request_len, 8);
        assert_eq!(program.request_bytes(0), Ok(8));
        assert_eq!(witness.source_offset(), 8);
        assert_eq!(witness.len(), 4);
        assert_eq!(witness.slice(b"12345678PROO"), Ok(b"PROO".as_slice()));
        assert_eq!(
            witness.slice(b"12345678PROOF"),
            Err(Error::InvalidCoordinate)
        );

        let mut invalid_register_pair = bytes.clone();
        put(
            &mut invalid_register_pair,
            HEADER_BYTES + 14,
            &1_u16.to_le_bytes(),
        );
        assert_eq!(
            ProgramV3::decode(&invalid_register_pair),
            Err(Error::InvalidRoute)
        );
        let mut invalid_each = bytes;
        *invalid_each.get_mut(HEADER_BYTES + 1).expect("route kind") = 2;
        assert_eq!(ProgramV3::decode(&invalid_each), Err(Error::InvalidRoute));
    }

    fn receipt_dependency_program(kind: u8) -> Vec<u8> {
        let producer = route(
            4,
            kind,
            0,
            1,
            0,
            u16::from(kind == 2),
            u32::from(kind != 2) * 8,
            u32::from(kind == 2) * 8,
        );
        let mut consumer = route(
            0,
            kind,
            0,
            1,
            0,
            u16::from(kind == 2),
            u32::from(kind != 2) * 8,
            u32::from(kind == 2) * 8,
        );
        put(&mut consumer, 24, &0_u16.to_le_bytes());
        put(&mut consumer, 26, &1_u16.to_le_bytes());
        let dependency = HEADER_BYTES + 2 * ROUTE_BYTES;
        let mut bytes = vec![0_u8; HEADER_BYTES + 2 * ROUTE_BYTES + RECEIPT_DEPENDENCY_BYTES + 16];
        put(&mut bytes, 0, &MAGIC);
        *bytes.get_mut(4).expect("version") = VERSION;
        for (offset, value) in [
            (6, 2_u16),
            (12, 1),
            (14, u16::from(kind == 2)),
            (16, 1),
            (24, 1),
        ] {
            put(&mut bytes, offset, &value.to_le_bytes());
        }
        put(&mut bytes, HEADER_BYTES, &producer);
        put(&mut bytes, HEADER_BYTES + ROUTE_BYTES, &consumer);
        *bytes.get_mut(dependency).expect("producer role") = 4;
        put(&mut bytes, dependency + 2, &0_u16.to_le_bytes());
        put(&mut bytes, dependency + 4, &384_u16.to_le_bytes());
        put(
            &mut bytes,
            dependency + RECEIPT_DEPENDENCY_BYTES,
            b"PRODUCERCONSUMER",
        );
        bytes
    }

    #[test]
    fn receipt_dependency_is_exact_backward_and_same_item() {
        let bytes = receipt_dependency_program(0);
        let program = ProgramV3::decode(&bytes).expect("dependency program");
        assert_eq!(
            program
                .resolved_invocation(1, 0, 0, &[1], &[])
                .expect("consumer")
                .receipt_dependency,
            Some(ResolvedReceiptDependencyV3 {
                producer_role: FixedRole::Custody,
                producer_route: 0,
                producer_invocation: 0,
                expected_receipt_bytes: 384,
            })
        );

        let each = receipt_dependency_program(2);
        let program = ProgramV3::decode(&each).expect("each dependency");
        assert_eq!(
            program
                .resolved_invocation(1, 1, 2, &[1], &[])
                .expect("item one")
                .receipt_dependency
                .expect("dependency")
                .producer_invocation,
            1
        );
    }

    #[test]
    fn legacy_24_byte_route_profile_is_not_accepted() {
        const LEGACY_ROUTE_BYTES: usize = 24;

        let legacy_route = route(0, 0, 0, 1, 0, 0, 8, 0);
        let mut bytes = vec![0_u8; HEADER_BYTES + LEGACY_ROUTE_BYTES + 8];
        put(&mut bytes, 0, &MAGIC);
        *bytes.get_mut(4).expect("version") = VERSION;
        for (offset, value) in [(6, 1_u16), (12, 1), (16, 1)] {
            put(&mut bytes, offset, &value.to_le_bytes());
        }
        put(
            &mut bytes,
            HEADER_BYTES,
            legacy_route
                .get(..LEGACY_ROUTE_BYTES)
                .expect("legacy route"),
        );
        put(&mut bytes, HEADER_BYTES + LEGACY_ROUTE_BYTES, b"CORE_REQ");

        assert!(ProgramV3::decode(&bytes).is_err());
    }

    #[test]
    fn receipt_dependency_refuses_forward_role_width_geometry_and_disabled_source() {
        let canonical = receipt_dependency_program(0);
        let consumer = HEADER_BYTES + ROUTE_BYTES;
        let dependency = HEADER_BYTES + 2 * ROUTE_BYTES;

        let mut forward = canonical.clone();
        put(&mut forward, dependency + 2, &1_u16.to_le_bytes());
        assert_eq!(
            ProgramV3::decode(&forward),
            Err(Error::InvalidReceiptDependency)
        );

        let mut wrong_role = canonical.clone();
        *wrong_role.get_mut(dependency).expect("producer role") = 1;
        assert_eq!(
            ProgramV3::decode(&wrong_role),
            Err(Error::InvalidReceiptDependency)
        );

        let mut zero_width = canonical.clone();
        put(&mut zero_width, dependency + 4, &0_u16.to_le_bytes());
        assert_eq!(
            ProgramV3::decode(&zero_width),
            Err(Error::InvalidReceiptDependency)
        );

        let mut cross_geometry = canonical.clone();
        *cross_geometry.get_mut(consumer + 1).expect("consumer kind") = 2;
        assert_eq!(
            ProgramV3::decode(&cross_geometry),
            Err(Error::InvalidReceiptDependency)
        );

        let mut noncanonical_absent = canonical.clone();
        put(
            &mut noncanonical_absent,
            consumer + 26,
            &0_u16.to_le_bytes(),
        );
        assert_eq!(
            ProgramV3::decode(&noncanonical_absent),
            Err(Error::InvalidReceiptDependency)
        );

        let mut disabled = canonical;
        *disabled.get_mut(HEADER_BYTES + 2).expect("producer enable") = 1;
        put(&mut disabled, HEADER_BYTES + 4, &0_u16.to_le_bytes());
        let program = ProgramV3::decode(&disabled).expect("statically valid disabled producer");
        assert_eq!(
            program.resolved_invocation(1, 0, 0, &[0], &[]),
            Err(Error::InvalidReceiptDependency)
        );
    }

    #[test]
    fn short_buffer_narrowing_and_cross_item_alias_preserve_outputs() {
        let canonical = canonical();
        let program = ProgramV3::decode(&canonical).expect("program");
        let identities = [[0_u8; 32]; 3];
        let accounts = [
            AccountInput {
                lamports: 10,
                data_len: 0,
            },
            AccountInput {
                lamports: 3,
                data_len: 0,
            },
            AccountInput {
                lamports: 0,
                data_len: 0,
            },
            AccountInput {
                lamports: 3,
                data_len: 0,
            },
            AccountInput {
                lamports: 0,
                data_len: 0,
            },
        ];
        let permissions = [
            AccountPermission::lamport_receiver(),
            AccountPermission::new(true, false, false),
            AccountPermission::read_only(),
            AccountPermission::new(true, false, false),
            AccountPermission::read_only(),
        ];
        for hostile in 0..3 {
            let scalars = if hostile == 1 {
                [u64::from(u32::MAX) + 1, 0, 3, 1, 3]
            } else {
                [9, 0, 3, 1, 3]
            };
            let aliases = if hostile == 2 {
                [0, 1, 2, 1, 4]
            } else {
                [0, 1, 2, 3, 4]
            };
            let mut scratch_lamports = [0_u64; 5];
            let mut output_lamports = [99_u64; 5];
            let before_lamports = output_lamports;
            let mut requests = [0x55_u8; 40];
            let before_requests = requests;
            let request_bank = if hostile == 0 {
                &mut requests[..39]
            } else {
                &mut requests[..]
            };
            assert!(
                project_atomic(
                    program,
                    2,
                    ProjectionV3 {
                        scalars: &scalars,
                        identities: &identities,
                        aliases: &aliases,
                        accounts: &accounts,
                        permissions: &permissions,
                        scratch_lamports: &mut scratch_lamports,
                        output_lamports: &mut output_lamports,
                        requests: request_bank,
                    },
                )
                .is_err()
            );
            assert_eq!(output_lamports, before_lamports);
            // The candidate lamport bank is still written only on success.
            //
            // The request bank is now single, so it is no longer atomic in
            // general: `initialize_requests` writes it before the first
            // fallible effect runs. It survives these three refusals for a
            // reason worth stating exactly — a declared-width mismatch, an
            // out-of-range register and a cross-item alias are all raised
            // *before* that first write. A refusal from inside the effect loop
            // may leave partial route bytes behind, which
            // `partial_request_bank_survives_a_refusal_inside_the_effect_loop`
            // pins.
            assert_eq!(requests, before_requests);
        }
    }

    /// The single request bank is NOT failure-atomic once projection begins.
    ///
    /// This pins the exact cost of collapsing the old scratch/output request
    /// pair into one bank. The lamport candidate is still written only on
    /// success; the request bank is initialized with the declared route
    /// templates before the first fallible effect runs, so a refusal raised
    /// inside the effect loop leaves those template bytes behind and the
    /// caller's previous contents are gone. Every first-party caller allocates
    /// this bank per projection and discards it on refusal, which is why the
    /// weaker contract is affordable — a caller that needs the old bytes must
    /// keep its own copy.
    #[test]
    fn partial_request_bank_survives_a_refusal_inside_the_effect_loop() {
        let bytes = canonical();
        let program = ProgramV3::decode(&bytes).expect("program");
        let scalars = [9_u64, 0, 3, 1, 4];
        let identities = [[0_u8; 32]; 3];
        let accounts = [
            AccountInput {
                lamports: 10,
                data_len: 0,
            },
            AccountInput {
                lamports: 3,
                data_len: 0,
            },
            AccountInput {
                lamports: 0,
                data_len: 0,
            },
            AccountInput {
                lamports: 4,
                data_len: 0,
            },
            AccountInput {
                lamports: 0,
                data_len: 0,
            },
        ];
        // The one difference from `affine_and_each_requests_project_atomically`:
        // coordinate 1 may no longer be debited, so the projection refuses from
        // inside `project_effect` rather than from any width or alias check.
        let permissions = [
            AccountPermission::lamport_receiver(),
            AccountPermission::read_only(),
            AccountPermission::read_only(),
            AccountPermission::new(true, false, false),
            AccountPermission::read_only(),
        ];
        let aliases = [0, 1, 2, 3, 4];
        let mut scratch_lamports = [0_u64; 5];
        let mut output_lamports = [99_u64; 5];
        let before_lamports = output_lamports;
        let mut requests = [0x55_u8; 40];
        assert!(
            project_atomic(
                program,
                2,
                ProjectionV3 {
                    scalars: &scalars,
                    identities: &identities,
                    aliases: &aliases,
                    accounts: &accounts,
                    permissions: &permissions,
                    scratch_lamports: &mut scratch_lamports,
                    output_lamports: &mut output_lamports,
                    requests: &mut requests,
                },
            )
            .is_err()
        );
        assert_eq!(output_lamports, before_lamports);
        assert_ne!(requests, [0x55_u8; 40]);
        assert_eq!(&requests[4..8], b"MFIX");
    }

    #[test]
    fn hostile_route_operation_and_program_identity_refuse() {
        let canonical = canonical();
        for offset in [
            5,
            HEADER_BYTES + 3,
            HEADER_BYTES + 18,
            HEADER_BYTES + 2 * ROUTE_BYTES + 18,
        ] {
            let mut hostile = canonical.clone();
            *hostile.get_mut(offset).expect("hostile byte") ^= 1;
            assert!(ProgramV3::decode(&hostile).is_err());
        }
        assert_eq!(
            ProgramV3::decode_selected([1; 32], [2; 32], &canonical),
            Err(Error::ProgramIdentityMismatch)
        );
    }

    #[test]
    fn affine_fixed_account_writes_resolve_runtime_offsets() {
        let bytes = affine_data_program(40, 12);
        let program = ProgramV3::decode(&bytes).expect("affine program");
        let scalars = [11_u64, 22];
        let identities = [[3_u8; 32], [4_u8; 32]];
        assert_eq!(
            program.resolved_item_effect(0, 0, 2, &scalars, &identities),
            Ok(ResolvedEffectV3::WriteScalar {
                account: 0,
                offset: 4,
                value: 11,
            })
        );
        assert_eq!(
            program.resolved_item_effect(1, 0, 2, &scalars, &identities),
            Ok(ResolvedEffectV3::WriteScalar {
                account: 0,
                offset: 44,
                value: 22,
            })
        );
        assert_eq!(
            program.resolved_item_effect(1, 1, 2, &scalars, &identities),
            Ok(ResolvedEffectV3::WriteIdentity {
                account: 0,
                offset: 52,
                value: [4; 32],
            })
        );

        let accounts = [AccountInput {
            lamports: 7,
            data_len: 84,
        }];
        let permissions = [AccountPermission::new(false, false, true)];
        let aliases = [0_usize];
        let mut scratch_lamports = [0_u64];
        let mut output_lamports = [99_u64];
        let mut requests = [];
        project_atomic(
            program,
            2,
            ProjectionV3 {
                scalars: &scalars,
                identities: &identities,
                aliases: &aliases,
                accounts: &accounts,
                permissions: &permissions,
                scratch_lamports: &mut scratch_lamports,
                output_lamports: &mut output_lamports,
                requests: &mut requests,
            },
        )
        .expect("exact affine account bounds");
        assert_eq!(output_lamports, [7]);
    }

    #[test]
    fn affine_writes_refuse_bad_stride_overlap_and_bounds_atomically() {
        assert_eq!(
            ProgramV3::decode(&affine_data_program(7, 12)),
            Err(Error::NonCanonicalOperation)
        );
        assert_eq!(
            ProgramV3::decode(&affine_data_program(40, 8))
                .expect("cross-operation overlap is runtime-resolved")
                .resolved_item_effect(0, 1, 2, &[11, 22], &[[3; 32], [4; 32]]),
            Ok(ResolvedEffectV3::WriteIdentity {
                account: 0,
                offset: 8,
                value: [3; 32],
            })
        );

        for (bytes, tail_count, expected) in [
            (affine_data_program(40, 8), 2, Error::OverlappingWrites),
            (affine_data_program(40, 12), 3, Error::WidthMismatch),
        ] {
            let program = ProgramV3::decode(&bytes).expect("structural program");
            let scalars = [11_u64, 22];
            let identities = [[3_u8; 32], [4_u8; 32]];
            let accounts = [AccountInput {
                lamports: 7,
                data_len: 84,
            }];
            let permissions = [AccountPermission::new(false, false, true)];
            let aliases = [0_usize];
            let mut scratch_lamports = [55_u64];
            let mut output_lamports = [99_u64];
            let before_output = output_lamports;
            let mut requests = [];
            assert_eq!(
                project_atomic(
                    program,
                    tail_count,
                    ProjectionV3 {
                        scalars: &scalars,
                        identities: &identities,
                        aliases: &aliases,
                        accounts: &accounts,
                        permissions: &permissions,
                        scratch_lamports: &mut scratch_lamports,
                        output_lamports: &mut output_lamports,
                        requests: &mut requests,
                    },
                ),
                Err(expected)
            );
            assert_eq!(output_lamports, before_output);
        }

        let bytes = affine_data_program(40, 12);
        let program = ProgramV3::decode(&bytes).expect("program");
        let scalars = [11_u64, 22, 33];
        let identities = [[3_u8; 32], [4_u8; 32], [5_u8; 32]];
        let accounts = [AccountInput {
            lamports: 7,
            data_len: 84,
        }];
        let permissions = [AccountPermission::new(false, false, true)];
        let aliases = [0_usize];
        let mut scratch_lamports = [55_u64];
        let mut output_lamports = [99_u64];
        let before_output = output_lamports;
        let mut requests = [];
        assert_eq!(
            project_atomic(
                program,
                3,
                ProjectionV3 {
                    scalars: &scalars,
                    identities: &identities,
                    aliases: &aliases,
                    accounts: &accounts,
                    permissions: &permissions,
                    scratch_lamports: &mut scratch_lamports,
                    output_lamports: &mut output_lamports,
                    requests: &mut requests,
                },
            ),
            Err(Error::DataOutOfBounds)
        );
        assert_eq!(output_lamports, before_output);
    }

    fn typed_data_program(operations: &[[u8; OPERATION_BYTES]], accounts: u16) -> Vec<u8> {
        let mut output = vec![0_u8; HEADER_BYTES + operations.len() * OPERATION_BYTES];
        put(&mut output, 0, &MAGIC);
        *output.get_mut(4).expect("version") = VERSION;
        put(
            &mut output,
            8,
            &u16::try_from(operations.len())
                .expect("operation count")
                .to_le_bytes(),
        );
        put(&mut output, 12, &accounts.to_le_bytes());
        put(&mut output, 16, &3_u16.to_le_bytes());
        let mut cursor = HEADER_BYTES;
        for operation in operations {
            put(&mut output, cursor, operation);
            cursor += OPERATION_BYTES;
        }
        output
    }

    #[test]
    fn typed_account_writes_narrow_and_resolve_exact_widths() {
        let operations = [
            operation(OP_WRITE_DATA_U8, 0, 0, 0, 0, 1, 0),
            operation(OP_WRITE_DATA_U16, 0, 0, 0, 1, 2, 0),
            operation(OP_WRITE_DATA_U32, 0, 0, 0, 2, 4, 0),
        ];
        let bytes = typed_data_program(&operations, 1);
        let program = ProgramV3::decode(&bytes).expect("typed data program");
        let scalars = [u64::from(u8::MAX), u64::from(u16::MAX), u64::from(u32::MAX)];
        assert_eq!(
            program.resolved_fixed_effect(0, 0, &scalars, &[]),
            Ok(ResolvedEffectV3::WriteU8 {
                account: 0,
                offset: 1,
                value: u8::MAX,
            })
        );
        assert_eq!(
            program.resolved_fixed_effect(1, 0, &scalars, &[]),
            Ok(ResolvedEffectV3::WriteU16 {
                account: 0,
                offset: 2,
                value: u16::MAX,
            })
        );
        assert_eq!(
            program.resolved_fixed_effect(2, 0, &scalars, &[]),
            Ok(ResolvedEffectV3::WriteU32 {
                account: 0,
                offset: 4,
                value: u32::MAX,
            })
        );
        let accounts = [AccountInput {
            lamports: 17,
            data_len: 8,
        }];
        let permissions = [AccountPermission::new(false, false, true)];
        let aliases = [0_usize];
        let mut scratch_lamports = [0_u64];
        let mut output_lamports = [99_u64];
        project_atomic(
            program,
            0,
            ProjectionV3 {
                scalars: &scalars,
                identities: &[],
                aliases: &aliases,
                accounts: &accounts,
                permissions: &permissions,
                scratch_lamports: &mut scratch_lamports,
                output_lamports: &mut output_lamports,
                requests: &mut [],
            },
        )
        .expect("typed writes preflight");
        assert_eq!(output_lamports, [17]);
    }

    #[test]
    fn typed_account_writes_refuse_narrowing_overlap_permissions_and_bounds_atomically() {
        let narrowing = typed_data_program(&[operation(OP_WRITE_DATA_U8, 0, 0, 0, 0, 0, 0)], 1);
        let program = ProgramV3::decode(&narrowing).expect("narrowing program");
        assert_eq!(
            program.resolved_fixed_effect(0, 0, &[u64::from(u8::MAX) + 1, 0, 0], &[]),
            Err(Error::NarrowingOverflow)
        );

        let overlap = typed_data_program(
            &[
                operation(OP_WRITE_DATA_U8, 0, 0, 0, 0, 0, 0),
                operation(OP_WRITE_DATA_U16, 0, 0, 0, 1, 0, 0),
            ],
            1,
        );
        assert_eq!(ProgramV3::decode(&overlap), Err(Error::OverlappingWrites));

        let aliased = typed_data_program(
            &[
                operation(OP_WRITE_DATA_U8, 0, 0, 0, 0, 0, 0),
                operation(OP_WRITE_DATA_U16, 0, 1, 0, 1, 0, 0),
            ],
            2,
        );
        let program = ProgramV3::decode(&aliased).expect("structurally disjoint accounts");
        let accounts = [
            AccountInput {
                lamports: 7,
                data_len: 4,
            },
            AccountInput {
                lamports: 7,
                data_len: 4,
            },
        ];
        let writable = [
            AccountPermission::new(false, false, true),
            AccountPermission::new(false, false, true),
        ];
        let aliases = [0_usize, 0];
        let mut scratch = [0_u64; 2];
        let mut output = [91_u64; 2];
        let before = output;
        assert_eq!(
            project_atomic(
                program,
                0,
                ProjectionV3 {
                    scalars: &[1, 2, 0],
                    identities: &[],
                    aliases: &aliases,
                    accounts: &accounts,
                    permissions: &writable,
                    scratch_lamports: &mut scratch,
                    output_lamports: &mut output,
                    requests: &mut [],
                },
            ),
            Err(Error::OverlappingWrites)
        );
        assert_eq!(output, before);

        for (permission, data_len, expected) in [
            (AccountPermission::read_only(), 4, Error::PermissionDenied),
            (
                AccountPermission::new(false, false, true),
                0,
                Error::DataOutOfBounds,
            ),
        ] {
            let program = ProgramV3::decode(&narrowing).expect("typed program");
            let accounts = [AccountInput {
                lamports: 7,
                data_len,
            }];
            let permissions = [permission];
            let aliases = [0_usize];
            let mut scratch = [0_u64];
            let mut output = [99_u64];
            let before = output;
            assert_eq!(
                project_atomic(
                    program,
                    0,
                    ProjectionV3 {
                        scalars: &[1, 0, 0],
                        identities: &[],
                        aliases: &aliases,
                        accounts: &accounts,
                        permissions: &permissions,
                        scratch_lamports: &mut scratch,
                        output_lamports: &mut output,
                        requests: &mut [],
                    },
                ),
                Err(expected)
            );
            assert_eq!(output, before);
        }
    }
}
