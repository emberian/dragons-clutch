//! Compact descriptor-bound request projection with an independent row count.
//!
//! V1's fixed-prefix projector remains the sole owner of fixed request bytes.
//! V4 adds one canonical row program which is repeated exactly `K` times,
//! where the artifact commits the expected `K` and names a protected scalar
//! populated from independently authenticated immutable config state. Product
//! tail width is deliberately unrelated. Row-local outputs are translated
//! after the protected prefix, so no row can alias a prefix or another row.

use core::convert::TryInto;

use super::{
    ProjectionRegisterKindV1, ProjectionRegisterSpaceV1, ProjectionRegistersV1, ProjectionTargetV1,
    RequestProfileV1, project_atomic,
};

pub use super::generated_v4::{
    REQUEST_PROFILE_V4_ARTIFACT_PROFILE, REQUEST_PROFILE_V4_HEADER_BYTES, REQUEST_PROFILE_V4_MAGIC,
    REQUEST_PROFILE_V4_MAX_BYTES, REQUEST_PROFILE_V4_MAX_ROW_BYTES, REQUEST_PROFILE_V4_MAX_ROWS,
    REQUEST_PROFILE_V4_ROW_OPERATION_BYTES, REQUEST_PROFILE_V4_SCHEMA_RELEASE_ID,
    REQUEST_PROFILE_V4_SCHEMA_RELEASE_PREIMAGE, REQUEST_PROFILE_V4_SCHEMA_VERSION,
};
use super::generated_v4::{
    REQUEST_PROFILE_V4_ARTIFACT_PROFILE_OFFSET, REQUEST_PROFILE_V4_EMBEDDED_V1_BYTES_OFFSET,
    REQUEST_PROFILE_V4_EXPECTED_ROW_COUNT_OFFSET, REQUEST_PROFILE_V4_MAGIC_OFFSET,
    REQUEST_PROFILE_V4_OP_PROJECT_IDENTITY, REQUEST_PROFILE_V4_OP_PROJECT_U8,
    REQUEST_PROFILE_V4_OP_PROJECT_U16, REQUEST_PROFILE_V4_OP_PROJECT_U32,
    REQUEST_PROFILE_V4_OP_PROJECT_U64, REQUEST_PROFILE_V4_OP_REQUIRE_U8,
    REQUEST_PROFILE_V4_OP_REQUIRE_U16, REQUEST_PROFILE_V4_OP_REQUIRE_U32,
    REQUEST_PROFILE_V4_OP_REQUIRE_U64, REQUEST_PROFILE_V4_OP_REQUIRE_ZERO,
    REQUEST_PROFILE_V4_ORDERED_KEY_OFFSET_OFFSET, REQUEST_PROFILE_V4_ORDERED_KEY_SCALAR_OFFSET,
    REQUEST_PROFILE_V4_PROTECTED_IDENTITIES_OFFSET, REQUEST_PROFILE_V4_PROTECTED_SCALARS_OFFSET,
    REQUEST_PROFILE_V4_REQUEST_ROW_COUNT_OFFSET_OFFSET, REQUEST_PROFILE_V4_RESERVED_OFFSET,
    REQUEST_PROFILE_V4_ROW_BYTES_OFFSET, REQUEST_PROFILE_V4_ROW_COUNT_SCALAR_OFFSET,
    REQUEST_PROFILE_V4_ROW_IDENTITY_STRIDE_OFFSET, REQUEST_PROFILE_V4_ROW_IMMEDIATE_OFFSET,
    REQUEST_PROFILE_V4_ROW_OPCODE_OFFSET, REQUEST_PROFILE_V4_ROW_OPERATION_COUNT_OFFSET,
    REQUEST_PROFILE_V4_ROW_REQUEST_OFFSET_OFFSET, REQUEST_PROFILE_V4_ROW_RESERVED_BYTE_OFFSET,
    REQUEST_PROFILE_V4_ROW_RESERVED_HEAD_OFFSET, REQUEST_PROFILE_V4_ROW_RESERVED_TAIL_OFFSET,
    REQUEST_PROFILE_V4_ROW_SCALAR_STRIDE_OFFSET, REQUEST_PROFILE_V4_ROW_TARGET_KIND_OFFSET,
    REQUEST_PROFILE_V4_ROW_TARGET_OFFSET, REQUEST_PROFILE_V4_SCHEMA_VERSION_OFFSET,
    REQUEST_PROFILE_V4_TARGET_IDENTITY, REQUEST_PROFILE_V4_TARGET_NONE,
    REQUEST_PROFILE_V4_TARGET_SCALAR,
};

const EMBEDDED_V1_BYTES_OFFSET: usize = REQUEST_PROFILE_V4_EMBEDDED_V1_BYTES_OFFSET;
const EXPECTED_ROW_COUNT_OFFSET: usize = REQUEST_PROFILE_V4_EXPECTED_ROW_COUNT_OFFSET;
const ROW_BYTES_OFFSET: usize = REQUEST_PROFILE_V4_ROW_BYTES_OFFSET;
const REQUEST_ROW_COUNT_OFFSET_OFFSET: usize = REQUEST_PROFILE_V4_REQUEST_ROW_COUNT_OFFSET_OFFSET;
const ORDERED_KEY_OFFSET_OFFSET: usize = REQUEST_PROFILE_V4_ORDERED_KEY_OFFSET_OFFSET;
const PROTECTED_SCALARS_OFFSET: usize = REQUEST_PROFILE_V4_PROTECTED_SCALARS_OFFSET;
const ROW_SCALAR_STRIDE_OFFSET: usize = REQUEST_PROFILE_V4_ROW_SCALAR_STRIDE_OFFSET;
const PROTECTED_IDENTITIES_OFFSET: usize = REQUEST_PROFILE_V4_PROTECTED_IDENTITIES_OFFSET;
const ROW_IDENTITY_STRIDE_OFFSET: usize = REQUEST_PROFILE_V4_ROW_IDENTITY_STRIDE_OFFSET;
const ROW_COUNT_SCALAR_OFFSET: usize = REQUEST_PROFILE_V4_ROW_COUNT_SCALAR_OFFSET;
const ORDERED_KEY_SCALAR_OFFSET: usize = REQUEST_PROFILE_V4_ORDERED_KEY_SCALAR_OFFSET;
const ROW_OPERATION_COUNT_OFFSET: usize = REQUEST_PROFILE_V4_ROW_OPERATION_COUNT_OFFSET;
const RESERVED_OFFSET: usize = REQUEST_PROFILE_V4_RESERVED_OFFSET;

const OP_REQUIRE_U8: u8 = REQUEST_PROFILE_V4_OP_REQUIRE_U8;
const OP_REQUIRE_U16: u8 = REQUEST_PROFILE_V4_OP_REQUIRE_U16;
const OP_REQUIRE_U32: u8 = REQUEST_PROFILE_V4_OP_REQUIRE_U32;
const OP_REQUIRE_U64: u8 = REQUEST_PROFILE_V4_OP_REQUIRE_U64;
const OP_REQUIRE_ZERO: u8 = REQUEST_PROFILE_V4_OP_REQUIRE_ZERO;
const OP_PROJECT_U8: u8 = REQUEST_PROFILE_V4_OP_PROJECT_U8;
const OP_PROJECT_U16: u8 = REQUEST_PROFILE_V4_OP_PROJECT_U16;
const OP_PROJECT_U32: u8 = REQUEST_PROFILE_V4_OP_PROJECT_U32;
const OP_PROJECT_U64: u8 = REQUEST_PROFILE_V4_OP_PROJECT_U64;
const OP_PROJECT_IDENTITY: u8 = REQUEST_PROFILE_V4_OP_PROJECT_IDENTITY;

const TARGET_NONE: u8 = REQUEST_PROFILE_V4_TARGET_NONE;
const TARGET_SCALAR: u8 = REQUEST_PROFILE_V4_TARGET_SCALAR;
const TARGET_IDENTITY: u8 = REQUEST_PROFILE_V4_TARGET_IDENTITY;

const OP_OPCODE_OFFSET: usize = REQUEST_PROFILE_V4_ROW_OPCODE_OFFSET;
const OP_RESERVED_HEAD_OFFSET: usize = REQUEST_PROFILE_V4_ROW_RESERVED_HEAD_OFFSET;
const OP_REQUEST_OFFSET_OFFSET: usize = REQUEST_PROFILE_V4_ROW_REQUEST_OFFSET_OFFSET;
const OP_TARGET_KIND_OFFSET: usize = REQUEST_PROFILE_V4_ROW_TARGET_KIND_OFFSET;
const OP_RESERVED_BYTE_OFFSET: usize = REQUEST_PROFILE_V4_ROW_RESERVED_BYTE_OFFSET;
const OP_TARGET_OFFSET: usize = REQUEST_PROFILE_V4_ROW_TARGET_OFFSET;
const OP_IMMEDIATE_OFFSET: usize = REQUEST_PROFILE_V4_ROW_IMMEDIATE_OFFSET;
const OP_RESERVED_TAIL_OFFSET: usize = REQUEST_PROFILE_V4_ROW_RESERVED_TAIL_OFFSET;

/// Stable hostile-decode, projection, or arithmetic refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Selected and independently authenticated identities differed or were zero.
    ProgramIdentityMismatch,
    /// Header, embedded program, request, rows, or banks had another exact width.
    InvalidLength,
    /// Magic, version, artifact profile, or reserved bytes were noncanonical.
    InvalidHeader,
    /// Embedded RequestProfile V1 refused.
    InvalidEmbeddedProfile,
    /// Immutable/authenticated row counts differed or exceeded the bound.
    RowCountMismatch,
    /// Row program opcode, active fields, or byte coverage was noncanonical.
    InvalidRowProgram,
    /// A request or output register coordinate was outside its declared space.
    InvalidCoordinate,
    /// A row output targeted the same row-local register more than once.
    DuplicateProjection,
    /// Ordered row keys were duplicated or not strictly increasing.
    NonCanonicalRowOrder,
    /// A required row literal or zero range differed.
    CheckFailed,
    /// Checked width or register arithmetic overflowed.
    ArithmeticOverflow,
}

/// Result alias.
pub type Result<T> = core::result::Result<T, Error>;

/// Exact immutable row-program geometry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RowProgramGeometryV4 {
    /// Descriptor-derived row count committed by the finalized artifact.
    pub expected_row_count: u32,
    /// Exact byte width of one row.
    pub row_bytes: u32,
    /// Fixed-prefix offset of the request's redundant row-count field.
    pub request_row_count_offset: u32,
    /// Row-local offset of the strictly ordered unique `u32` key.
    pub ordered_key_offset: u32,
    /// Scalar registers protected from every row-local projection.
    pub protected_scalars: u16,
    /// Scalar registers allocated to each row.
    pub row_scalar_stride: u16,
    /// Identity registers protected from every row-local projection.
    pub protected_identities: u16,
    /// Identity registers allocated to each row.
    pub row_identity_stride: u16,
    /// Protected scalar populated from authenticated immutable config with `K`.
    pub row_count_common_scalar: u16,
    /// Row-local scalar which receives the ordered key.
    pub ordered_key_row_scalar: u16,
}

/// Caller-owned banks used for failure-atomic V4 projection.
pub struct ProjectionRegistersV4<'a> {
    /// Immutable scalar input after AccountProfile projection.
    pub input_scalars: &'a [u64],
    /// Immutable identity input after AccountProfile projection.
    pub input_identities: &'a [[u8; 32]],
    /// First scratch bank used by embedded-prefix and row projection.
    pub scratch_scalars: &'a mut [u64],
    /// First scratch identity bank.
    pub scratch_identities: &'a mut [[u8; 32]],
    /// Second candidate bank preserving atomic output on row refusal.
    pub candidate_scalars: &'a mut [u64],
    /// Second candidate identity bank.
    pub candidate_identities: &'a mut [[u8; 32]],
    /// Scalar output, changed only after every prefix and row check succeeds.
    pub output_scalars: &'a mut [u64],
    /// Identity output, changed only after every prefix and row check succeeds.
    pub output_identities: &'a mut [[u8; 32]],
}

/// Row-local scalar or identity destination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RowTargetV4 {
    Scalar(u16),
    Identity(u16),
}

/// One typed canonical row-program operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RowInstructionV4 {
    opcode: u8,
    request_offset: u32,
    target: Option<RowTargetV4>,
    immediate: u64,
}

impl RowInstructionV4 {
    /// Require one exact byte.
    pub const fn require_u8(request_offset: u32, value: u8) -> Self {
        Self::require(OP_REQUIRE_U8, request_offset, value as u64)
    }

    /// Require one exact little-endian `u16`.
    pub const fn require_u16(request_offset: u32, value: u16) -> Self {
        Self::require(OP_REQUIRE_U16, request_offset, value as u64)
    }

    /// Require one exact little-endian `u32`.
    pub const fn require_u32(request_offset: u32, value: u32) -> Self {
        Self::require(OP_REQUIRE_U32, request_offset, value as u64)
    }

    /// Require one exact little-endian `u64`.
    pub const fn require_u64(request_offset: u32, value: u64) -> Self {
        Self::require(OP_REQUIRE_U64, request_offset, value)
    }

    /// Require one nonempty exact zero range.
    pub const fn require_zero(request_offset: u32, bytes: u32) -> Self {
        Self::require(OP_REQUIRE_ZERO, request_offset, bytes as u64)
    }

    /// Project one byte into a row-local scalar.
    pub const fn project_u8(request_offset: u32, scalar: u16) -> Self {
        Self::project(OP_PROJECT_U8, request_offset, RowTargetV4::Scalar(scalar))
    }

    /// Project one little-endian `u16` into a row-local scalar.
    pub const fn project_u16(request_offset: u32, scalar: u16) -> Self {
        Self::project(OP_PROJECT_U16, request_offset, RowTargetV4::Scalar(scalar))
    }

    /// Project one little-endian `u32` into a row-local scalar.
    pub const fn project_u32(request_offset: u32, scalar: u16) -> Self {
        Self::project(OP_PROJECT_U32, request_offset, RowTargetV4::Scalar(scalar))
    }

    /// Project one little-endian `u64` into a row-local scalar.
    pub const fn project_u64(request_offset: u32, scalar: u16) -> Self {
        Self::project(OP_PROJECT_U64, request_offset, RowTargetV4::Scalar(scalar))
    }

    /// Project one exact 32-byte identity into a row-local identity.
    pub const fn project_identity(request_offset: u32, identity: u16) -> Self {
        Self::project(
            OP_PROJECT_IDENTITY,
            request_offset,
            RowTargetV4::Identity(identity),
        )
    }

    const fn require(opcode: u8, request_offset: u32, immediate: u64) -> Self {
        Self {
            opcode,
            request_offset,
            target: None,
            immediate,
        }
    }

    const fn project(opcode: u8, request_offset: u32, target: RowTargetV4) -> Self {
        Self {
            opcode,
            request_offset,
            target: Some(target),
            immediate: 0,
        }
    }
}

/// Borrowed hostile-decoded compact repeated-row profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestProfileV4<'a> {
    embedded: RequestProfileV1<'a>,
    geometry: RowProgramGeometryV4,
    row_operation_count: u16,
    bytes: &'a [u8],
    operation_start: usize,
}

impl<'a> RequestProfileV4<'a> {
    /// Decode only after descriptor selection joins authenticated raw bytes.
    pub fn decode_selected(
        selected_program_id: [u8; 32],
        authenticated_program_id: [u8; 32],
        bytes: &'a [u8],
    ) -> Result<Self> {
        if selected_program_id == [0; 32]
            || authenticated_program_id == [0; 32]
            || selected_program_id != authenticated_program_id
        {
            return Err(Error::ProgramIdentityMismatch);
        }
        Self::decode(bytes)
    }

    /// Hostile-decode and prevalidate one complete V4 artifact.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < REQUEST_PROFILE_V4_HEADER_BYTES
            || bytes.len() > REQUEST_PROFILE_V4_MAX_BYTES
            || bytes.get(REQUEST_PROFILE_V4_MAGIC_OFFSET..REQUEST_PROFILE_V4_SCHEMA_VERSION_OFFSET)
                != Some(REQUEST_PROFILE_V4_MAGIC.as_slice())
            || read_u16(bytes, REQUEST_PROFILE_V4_SCHEMA_VERSION_OFFSET)?
                != REQUEST_PROFILE_V4_SCHEMA_VERSION
            || read_u16(bytes, REQUEST_PROFILE_V4_ARTIFACT_PROFILE_OFFSET)?
                != REQUEST_PROFILE_V4_ARTIFACT_PROFILE
            || !all_zero(bytes, RESERVED_OFFSET, 18)?
        {
            return Err(Error::InvalidHeader);
        }
        let embedded_bytes = usize::try_from(read_u32(bytes, EMBEDDED_V1_BYTES_OFFSET)?)
            .map_err(|_| Error::InvalidLength)?;
        let row_operation_count = read_u16(bytes, ROW_OPERATION_COUNT_OFFSET)?;
        let operation_start = REQUEST_PROFILE_V4_HEADER_BYTES
            .checked_add(embedded_bytes)
            .ok_or(Error::ArithmeticOverflow)?;
        let expected = usize::from(row_operation_count)
            .checked_mul(REQUEST_PROFILE_V4_ROW_OPERATION_BYTES)
            .and_then(|body| operation_start.checked_add(body))
            .ok_or(Error::ArithmeticOverflow)?;
        if row_operation_count == 0 || bytes.len() != expected {
            return Err(Error::InvalidLength);
        }
        let embedded = RequestProfileV1::decode(
            bytes
                .get(REQUEST_PROFILE_V4_HEADER_BYTES..operation_start)
                .ok_or(Error::InvalidLength)?,
        )
        .map_err(|_| Error::InvalidEmbeddedProfile)?;
        let geometry = RowProgramGeometryV4 {
            expected_row_count: read_u32(bytes, EXPECTED_ROW_COUNT_OFFSET)?,
            row_bytes: read_u32(bytes, ROW_BYTES_OFFSET)?,
            request_row_count_offset: read_u32(bytes, REQUEST_ROW_COUNT_OFFSET_OFFSET)?,
            ordered_key_offset: read_u32(bytes, ORDERED_KEY_OFFSET_OFFSET)?,
            protected_scalars: read_u16(bytes, PROTECTED_SCALARS_OFFSET)?,
            row_scalar_stride: read_u16(bytes, ROW_SCALAR_STRIDE_OFFSET)?,
            protected_identities: read_u16(bytes, PROTECTED_IDENTITIES_OFFSET)?,
            row_identity_stride: read_u16(bytes, ROW_IDENTITY_STRIDE_OFFSET)?,
            row_count_common_scalar: read_u16(bytes, ROW_COUNT_SCALAR_OFFSET)?,
            ordered_key_row_scalar: read_u16(bytes, ORDERED_KEY_SCALAR_OFFSET)?,
        };
        validate_geometry(embedded, geometry)?;
        let profile = Self {
            embedded,
            geometry,
            row_operation_count,
            bytes,
            operation_start,
        };
        profile.validate_operations()?;
        Ok(profile)
    }

    /// Embedded fixed-prefix projector and full register geometry.
    pub const fn request_profile(self) -> RequestProfileV1<'a> {
        self.embedded
    }

    /// Exact immutable row geometry.
    pub const fn row_geometry(self) -> RowProgramGeometryV4 {
        self.geometry
    }

    /// Exact complete request width.
    pub fn request_bytes(self) -> Result<usize> {
        complete_request_bytes(self.embedded, self.geometry)
    }

    /// Borrow the complete finalized content preimage.
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Validate and project fixed prefix plus all exact rows atomically.
    pub fn project_atomic(
        self,
        complete_request: &[u8],
        registers: ProjectionRegistersV4<'_>,
    ) -> Result<()> {
        let scalar_count = self
            .embedded
            .scalar_count(0)
            .map_err(|_| Error::InvalidEmbeddedProfile)?;
        let identity_count = self
            .embedded
            .identity_count(0)
            .map_err(|_| Error::InvalidEmbeddedProfile)?;
        if complete_request.len() != self.request_bytes()?
            || !scalar_banks_match(&registers, scalar_count)
            || !identity_banks_match(&registers, identity_count)
        {
            return Err(Error::InvalidLength);
        }
        let authenticated_count = u32::try_from(
            *registers
                .input_scalars
                .get(usize::from(self.geometry.row_count_common_scalar))
                .ok_or(Error::InvalidCoordinate)?,
        )
        .map_err(|_| Error::RowCountMismatch)?;
        if authenticated_count != self.geometry.expected_row_count
            || read_u32(
                complete_request,
                usize::try_from(self.geometry.request_row_count_offset)
                    .map_err(|_| Error::InvalidCoordinate)?,
            )? != authenticated_count
        {
            return Err(Error::RowCountMismatch);
        }
        let prefix_bytes = usize::try_from(self.embedded.fixed_request_bytes())
            .map_err(|_| Error::InvalidLength)?;
        let prefix = complete_request
            .get(..prefix_bytes)
            .ok_or(Error::InvalidLength)?;
        project_atomic(
            self.embedded,
            0,
            prefix,
            ProjectionRegistersV1 {
                input_scalars: registers.input_scalars,
                input_identities: registers.input_identities,
                scratch_scalars: registers.scratch_scalars,
                scratch_identities: registers.scratch_identities,
                output_scalars: registers.candidate_scalars,
                output_identities: registers.candidate_identities,
            },
        )
        .map_err(|_| Error::InvalidEmbeddedProfile)?;
        registers
            .scratch_scalars
            .copy_from_slice(registers.candidate_scalars);
        registers
            .scratch_identities
            .copy_from_slice(registers.candidate_identities);

        let mut prior_key = None;
        let mut row = 0_u32;
        while row < authenticated_count {
            let row_bytes = self.row_bytes(complete_request, row)?;
            let key = read_u32(
                row_bytes,
                usize::try_from(self.geometry.ordered_key_offset)
                    .map_err(|_| Error::InvalidCoordinate)?,
            )?;
            if prior_key.is_some_and(|prior| key <= prior) {
                return Err(Error::NonCanonicalRowOrder);
            }
            prior_key = Some(key);
            let mut operation = 0_u16;
            while operation < self.row_operation_count {
                self.operation(operation)?.apply(
                    self.geometry,
                    row,
                    row_bytes,
                    registers.scratch_scalars,
                    registers.scratch_identities,
                )?;
                operation = operation.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
            }
            row = row.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        registers
            .output_scalars
            .copy_from_slice(registers.scratch_scalars);
        registers
            .output_identities
            .copy_from_slice(registers.scratch_identities);
        Ok(())
    }

    fn row_bytes(self, request: &[u8], row: u32) -> Result<&[u8]> {
        let fixed = usize::try_from(self.embedded.fixed_request_bytes())
            .map_err(|_| Error::InvalidLength)?;
        let stride = usize::try_from(self.geometry.row_bytes).map_err(|_| Error::InvalidLength)?;
        let start = usize::try_from(row)
            .map_err(|_| Error::InvalidLength)?
            .checked_mul(stride)
            .and_then(|offset| fixed.checked_add(offset))
            .ok_or(Error::ArithmeticOverflow)?;
        request
            .get(start..start.checked_add(stride).ok_or(Error::ArithmeticOverflow)?)
            .ok_or(Error::InvalidLength)
    }

    fn operation(self, index: u16) -> Result<RowOperationV4> {
        if index >= self.row_operation_count {
            return Err(Error::InvalidCoordinate);
        }
        let offset = usize::from(index)
            .checked_mul(REQUEST_PROFILE_V4_ROW_OPERATION_BYTES)
            .and_then(|value| self.operation_start.checked_add(value))
            .ok_or(Error::ArithmeticOverflow)?;
        RowOperationV4::decode(self.bytes, offset)
    }

    fn validate_operations(self) -> Result<()> {
        let mut ordered_key_projected = false;
        let mut operation = 0_u16;
        while operation < self.row_operation_count {
            let current = self.operation(operation)?;
            current.validate(self.geometry)?;
            if current.opcode == OP_PROJECT_U32
                && current.request_offset == self.geometry.ordered_key_offset
                && current.target == Some(RowTargetV4::Scalar(self.geometry.ordered_key_row_scalar))
            {
                ordered_key_projected = true;
            }
            if let Some(target) = current.target {
                let mut prior = 0_u16;
                while prior < operation {
                    if self.operation(prior)?.target == Some(target) {
                        return Err(Error::DuplicateProjection);
                    }
                    prior = prior.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
                }
            }
            operation = operation.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        if !ordered_key_projected {
            return Err(Error::InvalidRowProgram);
        }
        let mut byte = 0_u32;
        while byte < self.geometry.row_bytes {
            let mut coverage = 0_u16;
            operation = 0;
            while operation < self.row_operation_count {
                if self.operation(operation)?.covers(byte)? {
                    coverage = coverage.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
                }
                operation = operation.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
            }
            if coverage != 1 {
                return Err(Error::InvalidRowProgram);
            }
            byte = byte.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(())
    }
}

/// Encode one complete V4 wrapper atomically.
pub fn encode_request_profile_v4_atomic(
    embedded_v1: &[u8],
    geometry: RowProgramGeometryV4,
    row_instructions: &[RowInstructionV4],
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    let embedded =
        RequestProfileV1::decode(embedded_v1).map_err(|_| Error::InvalidEmbeddedProfile)?;
    validate_geometry(embedded, geometry)?;
    let operation_count =
        u16::try_from(row_instructions.len()).map_err(|_| Error::InvalidLength)?;
    let expected = row_instructions
        .len()
        .checked_mul(REQUEST_PROFILE_V4_ROW_OPERATION_BYTES)
        .and_then(|body| {
            REQUEST_PROFILE_V4_HEADER_BYTES
                .checked_add(embedded_v1.len())?
                .checked_add(body)
        })
        .ok_or(Error::ArithmeticOverflow)?;
    if row_instructions.is_empty()
        || expected > REQUEST_PROFILE_V4_MAX_BYTES
        || scratch.len() != expected
        || output.len() != expected
    {
        return Err(Error::InvalidLength);
    }
    scratch.fill(0);
    put(scratch, 0, &REQUEST_PROFILE_V4_MAGIC)?;
    put(scratch, 8, &REQUEST_PROFILE_V4_SCHEMA_VERSION.to_le_bytes())?;
    put(
        scratch,
        10,
        &REQUEST_PROFILE_V4_ARTIFACT_PROFILE.to_le_bytes(),
    )?;
    put(
        scratch,
        EMBEDDED_V1_BYTES_OFFSET,
        &u32::try_from(embedded_v1.len())
            .map_err(|_| Error::InvalidLength)?
            .to_le_bytes(),
    )?;
    for (offset, value) in [
        (EXPECTED_ROW_COUNT_OFFSET, geometry.expected_row_count),
        (ROW_BYTES_OFFSET, geometry.row_bytes),
        (
            REQUEST_ROW_COUNT_OFFSET_OFFSET,
            geometry.request_row_count_offset,
        ),
        (ORDERED_KEY_OFFSET_OFFSET, geometry.ordered_key_offset),
    ] {
        put(scratch, offset, &value.to_le_bytes())?;
    }
    for (offset, value) in [
        (PROTECTED_SCALARS_OFFSET, geometry.protected_scalars),
        (ROW_SCALAR_STRIDE_OFFSET, geometry.row_scalar_stride),
        (PROTECTED_IDENTITIES_OFFSET, geometry.protected_identities),
        (ROW_IDENTITY_STRIDE_OFFSET, geometry.row_identity_stride),
        (ROW_COUNT_SCALAR_OFFSET, geometry.row_count_common_scalar),
        (ORDERED_KEY_SCALAR_OFFSET, geometry.ordered_key_row_scalar),
        (ROW_OPERATION_COUNT_OFFSET, operation_count),
    ] {
        put(scratch, offset, &value.to_le_bytes())?;
    }
    let operation_start = REQUEST_PROFILE_V4_HEADER_BYTES
        .checked_add(embedded_v1.len())
        .ok_or(Error::ArithmeticOverflow)?;
    put(scratch, REQUEST_PROFILE_V4_HEADER_BYTES, embedded_v1)?;
    for (index, instruction) in row_instructions.iter().copied().enumerate() {
        let offset = index
            .checked_mul(REQUEST_PROFILE_V4_ROW_OPERATION_BYTES)
            .and_then(|value| operation_start.checked_add(value))
            .ok_or(Error::ArithmeticOverflow)?;
        encode_row_instruction(instruction, scratch, offset)?;
    }
    RequestProfileV4::decode(scratch)?;
    output.copy_from_slice(scratch);
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RowOperationV4 {
    opcode: u8,
    request_offset: u32,
    target: Option<RowTargetV4>,
    immediate: u64,
}

impl RowOperationV4 {
    fn decode(bytes: &[u8], offset: usize) -> Result<Self> {
        if !all_zero(bytes, add(offset, OP_RESERVED_HEAD_OFFSET)?, 3)?
            || byte(bytes, add(offset, OP_RESERVED_BYTE_OFFSET)?)? != 0
            || !all_zero(bytes, add(offset, OP_RESERVED_TAIL_OFFSET)?, 4)?
        {
            return Err(Error::InvalidRowProgram);
        }
        let target = match byte(bytes, add(offset, OP_TARGET_KIND_OFFSET)?)? {
            TARGET_NONE => None,
            TARGET_SCALAR => Some(RowTargetV4::Scalar(read_u16(
                bytes,
                add(offset, OP_TARGET_OFFSET)?,
            )?)),
            TARGET_IDENTITY => Some(RowTargetV4::Identity(read_u16(
                bytes,
                add(offset, OP_TARGET_OFFSET)?,
            )?)),
            _ => return Err(Error::InvalidRowProgram),
        };
        Ok(Self {
            opcode: byte(bytes, add(offset, OP_OPCODE_OFFSET)?)?,
            request_offset: read_u32(bytes, add(offset, OP_REQUEST_OFFSET_OFFSET)?)?,
            target,
            immediate: read_u64(bytes, add(offset, OP_IMMEDIATE_OFFSET)?)?,
        })
    }

    fn validate(self, geometry: RowProgramGeometryV4) -> Result<()> {
        let width = self.width()?;
        if self
            .request_offset
            .checked_add(width)
            .is_none_or(|end| end > geometry.row_bytes)
        {
            return Err(Error::InvalidCoordinate);
        }
        match (self.opcode, self.target, self.immediate) {
            (OP_REQUIRE_U8, None, value) if u8::try_from(value).is_ok() => {}
            (OP_REQUIRE_U16, None, value) if u16::try_from(value).is_ok() => {}
            (OP_REQUIRE_U32, None, value) if u32::try_from(value).is_ok() => {}
            (OP_REQUIRE_U64, None, _) => {}
            (OP_REQUIRE_ZERO, None, value) if value != 0 && u32::try_from(value).is_ok() => {}
            (
                OP_PROJECT_U8 | OP_PROJECT_U16 | OP_PROJECT_U32 | OP_PROJECT_U64,
                Some(RowTargetV4::Scalar(index)),
                0,
            ) if index < geometry.row_scalar_stride => {}
            (OP_PROJECT_IDENTITY, Some(RowTargetV4::Identity(index)), 0)
                if index < geometry.row_identity_stride => {}
            _ => return Err(Error::InvalidRowProgram),
        }
        Ok(())
    }

    fn width(self) -> Result<u32> {
        match self.opcode {
            OP_REQUIRE_U8 | OP_PROJECT_U8 => Ok(1),
            OP_REQUIRE_U16 | OP_PROJECT_U16 => Ok(2),
            OP_REQUIRE_U32 | OP_PROJECT_U32 => Ok(4),
            OP_REQUIRE_U64 | OP_PROJECT_U64 => Ok(8),
            OP_PROJECT_IDENTITY => Ok(32),
            OP_REQUIRE_ZERO => u32::try_from(self.immediate).map_err(|_| Error::InvalidRowProgram),
            _ => Err(Error::InvalidRowProgram),
        }
    }

    fn covers(self, byte: u32) -> Result<bool> {
        Ok(byte >= self.request_offset
            && byte
                < self
                    .request_offset
                    .checked_add(self.width()?)
                    .ok_or(Error::ArithmeticOverflow)?)
    }

    fn apply(
        self,
        geometry: RowProgramGeometryV4,
        row: u32,
        request: &[u8],
        scalars: &mut [u64],
        identities: &mut [[u8; 32]],
    ) -> Result<()> {
        let offset = usize::try_from(self.request_offset).map_err(|_| Error::InvalidCoordinate)?;
        match self.opcode {
            OP_REQUIRE_U8 if u64::from(byte(request, offset)?) == self.immediate => Ok(()),
            OP_REQUIRE_U16 if u64::from(read_u16(request, offset)?) == self.immediate => Ok(()),
            OP_REQUIRE_U32 if u64::from(read_u32(request, offset)?) == self.immediate => Ok(()),
            OP_REQUIRE_U64 if read_u64(request, offset)? == self.immediate => Ok(()),
            OP_REQUIRE_ZERO
                if all_zero(
                    request,
                    offset,
                    usize::try_from(self.immediate).map_err(|_| Error::InvalidLength)?,
                )? =>
            {
                Ok(())
            }
            OP_PROJECT_U8 => {
                self.write_scalar(geometry, row, scalars, u64::from(byte(request, offset)?))
            }
            OP_PROJECT_U16 => self.write_scalar(
                geometry,
                row,
                scalars,
                u64::from(read_u16(request, offset)?),
            ),
            OP_PROJECT_U32 => self.write_scalar(
                geometry,
                row,
                scalars,
                u64::from(read_u32(request, offset)?),
            ),
            OP_PROJECT_U64 => self.write_scalar(geometry, row, scalars, read_u64(request, offset)?),
            OP_PROJECT_IDENTITY => {
                self.write_identity(geometry, row, identities, read_array(request, offset)?)
            }
            _ => Err(Error::CheckFailed),
        }
    }

    fn write_scalar(
        self,
        geometry: RowProgramGeometryV4,
        row: u32,
        output: &mut [u64],
        value: u64,
    ) -> Result<()> {
        let Some(RowTargetV4::Scalar(local)) = self.target else {
            return Err(Error::InvalidRowProgram);
        };
        let index = row_register_index(
            geometry.protected_scalars,
            geometry.row_scalar_stride,
            row,
            local,
        )?;
        *output.get_mut(index).ok_or(Error::InvalidCoordinate)? = value;
        Ok(())
    }

    fn write_identity(
        self,
        geometry: RowProgramGeometryV4,
        row: u32,
        output: &mut [[u8; 32]],
        value: [u8; 32],
    ) -> Result<()> {
        let Some(RowTargetV4::Identity(local)) = self.target else {
            return Err(Error::InvalidRowProgram);
        };
        let index = row_register_index(
            geometry.protected_identities,
            geometry.row_identity_stride,
            row,
            local,
        )?;
        *output.get_mut(index).ok_or(Error::InvalidCoordinate)? = value;
        Ok(())
    }
}

fn validate_geometry(embedded: RequestProfileV1<'_>, geometry: RowProgramGeometryV4) -> Result<()> {
    if geometry.expected_row_count == 0
        || geometry.expected_row_count > REQUEST_PROFILE_V4_MAX_ROWS
        || geometry.row_bytes == 0
        || geometry.row_bytes > REQUEST_PROFILE_V4_MAX_ROW_BYTES
        || geometry.row_scalar_stride == 0
        || geometry.row_identity_stride == 0
        || geometry.row_count_common_scalar >= geometry.protected_scalars
        || geometry.ordered_key_row_scalar >= geometry.row_scalar_stride
        || embedded.item_request_bytes() != 0
        || embedded.item_scalar_stride() != 0
        || embedded.item_identity_stride() != 0
    {
        return Err(Error::InvalidLength);
    }
    let fixed = embedded.fixed_request_bytes();
    if geometry
        .request_row_count_offset
        .checked_add(4)
        .is_none_or(|end| end > fixed)
        || geometry
            .ordered_key_offset
            .checked_add(4)
            .is_none_or(|end| end > geometry.row_bytes)
    {
        return Err(Error::InvalidCoordinate);
    }
    let expected_scalars = register_count(
        geometry.protected_scalars,
        geometry.row_scalar_stride,
        geometry.expected_row_count,
    )?;
    let expected_identities = register_count(
        geometry.protected_identities,
        geometry.row_identity_stride,
        geometry.expected_row_count,
    )?;
    if embedded
        .scalar_count(0)
        .map_err(|_| Error::InvalidEmbeddedProfile)?
        != expected_scalars
        || embedded
            .identity_count(0)
            .map_err(|_| Error::InvalidEmbeddedProfile)?
            != expected_identities
        || embedded
            .writes_register(ProjectionTargetV1 {
                kind: ProjectionRegisterKindV1::Scalar,
                space: ProjectionRegisterSpaceV1::Common,
                index: geometry.row_count_common_scalar,
            })
            .map_err(|_| Error::InvalidEmbeddedProfile)?
    {
        return Err(Error::InvalidEmbeddedProfile);
    }
    let mut scalar = geometry.protected_scalars;
    while usize::from(scalar) < expected_scalars {
        if embedded
            .writes_register(ProjectionTargetV1 {
                kind: ProjectionRegisterKindV1::Scalar,
                space: ProjectionRegisterSpaceV1::Common,
                index: scalar,
            })
            .map_err(|_| Error::InvalidEmbeddedProfile)?
        {
            return Err(Error::InvalidEmbeddedProfile);
        }
        scalar = scalar.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    let mut identity = geometry.protected_identities;
    while usize::from(identity) < expected_identities {
        if embedded
            .writes_register(ProjectionTargetV1 {
                kind: ProjectionRegisterKindV1::Identity,
                space: ProjectionRegisterSpaceV1::Common,
                index: identity,
            })
            .map_err(|_| Error::InvalidEmbeddedProfile)?
        {
            return Err(Error::InvalidEmbeddedProfile);
        }
        identity = identity.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    complete_request_bytes(embedded, geometry)?;
    Ok(())
}

fn complete_request_bytes(
    embedded: RequestProfileV1<'_>,
    geometry: RowProgramGeometryV4,
) -> Result<usize> {
    usize::try_from(geometry.expected_row_count)
        .map_err(|_| Error::InvalidLength)?
        .checked_mul(usize::try_from(geometry.row_bytes).map_err(|_| Error::InvalidLength)?)
        .and_then(|rows| {
            usize::try_from(embedded.fixed_request_bytes())
                .ok()?
                .checked_add(rows)
        })
        .ok_or(Error::ArithmeticOverflow)
}

fn register_count(prefix: u16, stride: u16, rows: u32) -> Result<usize> {
    usize::try_from(rows)
        .map_err(|_| Error::InvalidLength)?
        .checked_mul(usize::from(stride))
        .and_then(|value| usize::from(prefix).checked_add(value))
        .ok_or(Error::ArithmeticOverflow)
}

fn row_register_index(prefix: u16, stride: u16, row: u32, local: u16) -> Result<usize> {
    if local >= stride {
        return Err(Error::InvalidCoordinate);
    }
    usize::try_from(row)
        .map_err(|_| Error::InvalidCoordinate)?
        .checked_mul(usize::from(stride))
        .and_then(|value| usize::from(prefix).checked_add(value))
        .and_then(|value| value.checked_add(usize::from(local)))
        .ok_or(Error::ArithmeticOverflow)
}

fn scalar_banks_match(registers: &ProjectionRegistersV4<'_>, expected: usize) -> bool {
    registers.input_scalars.len() == expected
        && registers.scratch_scalars.len() == expected
        && registers.candidate_scalars.len() == expected
        && registers.output_scalars.len() == expected
}

fn identity_banks_match(registers: &ProjectionRegistersV4<'_>, expected: usize) -> bool {
    registers.input_identities.len() == expected
        && registers.scratch_identities.len() == expected
        && registers.candidate_identities.len() == expected
        && registers.output_identities.len() == expected
}

fn encode_row_instruction(
    instruction: RowInstructionV4,
    output: &mut [u8],
    offset: usize,
) -> Result<()> {
    put_byte(output, add(offset, OP_OPCODE_OFFSET)?, instruction.opcode)?;
    put(
        output,
        add(offset, OP_REQUEST_OFFSET_OFFSET)?,
        &instruction.request_offset.to_le_bytes(),
    )?;
    let (kind, target) = match instruction.target {
        None => (TARGET_NONE, 0),
        Some(RowTargetV4::Scalar(value)) => (TARGET_SCALAR, value),
        Some(RowTargetV4::Identity(value)) => (TARGET_IDENTITY, value),
    };
    put_byte(output, add(offset, OP_TARGET_KIND_OFFSET)?, kind)?;
    put(
        output,
        add(offset, OP_TARGET_OFFSET)?,
        &target.to_le_bytes(),
    )?;
    put(
        output,
        add(offset, OP_IMMEDIATE_OFFSET)?,
        &instruction.immediate.to_le_bytes(),
    )
}

fn add(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right).ok_or(Error::ArithmeticOverflow)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    output
        .get_mut(
            offset
                ..offset
                    .checked_add(value.len())
                    .ok_or(Error::ArithmeticOverflow)?,
        )
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

fn put_byte(output: &mut [u8], offset: usize, value: u8) -> Result<()> {
    *output.get_mut(offset).ok_or(Error::InvalidLength)? = value;
    Ok(())
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    bytes
        .get(offset..offset.checked_add(N).ok_or(Error::ArithmeticOverflow)?)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn all_zero(bytes: &[u8], offset: usize, width: usize) -> Result<bool> {
    Ok(bytes
        .get(offset..offset.checked_add(width).ok_or(Error::ArithmeticOverflow)?)
        .ok_or(Error::InvalidLength)?
        .iter()
        .all(|value| *value == 0))
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::encode::{
        RequestCoordinateV1, RequestGeometryV1, RequestInstructionV1,
        encode_request_profile_v1_atomic,
    };
    use std::vec;
    use std::vec::Vec;

    const ROWS: u32 = 3;
    const ROW_BYTES: usize = 48;
    const PROTECTED_SCALARS: u16 = 2;
    const PROTECTED_IDENTITIES: u16 = 1;

    fn embedded_profile() -> [u8; 104] {
        let instructions = [
            RequestInstructionV1::require_u64(
                RequestCoordinateV1::fixed(0),
                u64::from_le_bytes(*b"PREFIX04"),
            ),
            RequestInstructionV1::require_u32(RequestCoordinateV1::fixed(8), ROWS),
            RequestInstructionV1::require_zero(RequestCoordinateV1::fixed(12), 4),
        ];
        let mut scratch = [0_u8; 104];
        let mut output = [0_u8; 104];
        encode_request_profile_v1_atomic(
            RequestGeometryV1::new(
                16,
                0,
                PROTECTED_SCALARS + u16::try_from(ROWS).expect("rows") * 2,
                0,
                PROTECTED_IDENTITIES + u16::try_from(ROWS).expect("rows"),
                0,
            ),
            &instructions,
            &[],
            &mut scratch,
            &mut output,
        )
        .expect("embedded V1");
        output
    }

    fn embedded_profile_aliasing_row() -> [u8; 104] {
        let instructions = [
            RequestInstructionV1::require_u64(
                RequestCoordinateV1::fixed(0),
                u64::from_le_bytes(*b"PREFIX04"),
            ),
            RequestInstructionV1::project_u32(
                RequestCoordinateV1::fixed(8),
                crate::encode::ScalarRegisterV1::common(PROTECTED_SCALARS),
            ),
            RequestInstructionV1::require_zero(RequestCoordinateV1::fixed(12), 4),
        ];
        let mut scratch = [0_u8; 104];
        let mut output = [0_u8; 104];
        encode_request_profile_v1_atomic(
            RequestGeometryV1::new(
                16,
                0,
                PROTECTED_SCALARS + u16::try_from(ROWS).expect("rows") * 2,
                0,
                PROTECTED_IDENTITIES + u16::try_from(ROWS).expect("rows"),
                0,
            ),
            &instructions,
            &[],
            &mut scratch,
            &mut output,
        )
        .expect("embedded V1 alias candidate");
        output
    }

    fn geometry() -> RowProgramGeometryV4 {
        RowProgramGeometryV4 {
            expected_row_count: ROWS,
            row_bytes: u32::try_from(ROW_BYTES).expect("row bytes"),
            request_row_count_offset: 8,
            ordered_key_offset: 0,
            protected_scalars: PROTECTED_SCALARS,
            row_scalar_stride: 2,
            protected_identities: PROTECTED_IDENTITIES,
            row_identity_stride: 1,
            row_count_common_scalar: 1,
            ordered_key_row_scalar: 0,
        }
    }

    fn instructions() -> [RowInstructionV4; 4] {
        [
            RowInstructionV4::project_u32(0, 0),
            RowInstructionV4::require_zero(4, 4),
            RowInstructionV4::project_u64(8, 1),
            RowInstructionV4::project_identity(16, 0),
        ]
    }

    fn profile_bytes() -> Vec<u8> {
        let embedded = embedded_profile();
        let bytes = REQUEST_PROFILE_V4_HEADER_BYTES
            + embedded.len()
            + instructions().len() * REQUEST_PROFILE_V4_ROW_OPERATION_BYTES;
        let mut scratch = vec![0_u8; bytes];
        let mut output = vec![0_u8; bytes];
        encode_request_profile_v4_atomic(
            &embedded,
            geometry(),
            &instructions(),
            &mut scratch,
            &mut output,
        )
        .expect("V4 profile");
        output
    }

    fn request(keys: [u32; 3]) -> Vec<u8> {
        let mut output = vec![0_u8; 16 + ROW_BYTES * 3];
        output
            .get_mut(..8)
            .expect("prefix")
            .copy_from_slice(b"PREFIX04");
        output
            .get_mut(8..12)
            .expect("row count")
            .copy_from_slice(&ROWS.to_le_bytes());
        for (row, key) in keys.into_iter().enumerate() {
            let base = 16 + row * ROW_BYTES;
            output
                .get_mut(base..base + 4)
                .expect("row key")
                .copy_from_slice(&key.to_le_bytes());
            output
                .get_mut(base + 8..base + 16)
                .expect("row scalar")
                .copy_from_slice(&(100_u64 + u64::try_from(row).expect("row")).to_le_bytes());
            output
                .get_mut(base + 16..base + 48)
                .expect("row identity")
                .fill(u8::try_from(row).expect("row").checked_add(10).expect("id"));
        }
        output
    }

    fn project(
        profile: RequestProfileV4<'_>,
        request: &[u8],
        output_seed: u64,
    ) -> Result<Vec<u64>> {
        let scalar_count = profile.request_profile().scalar_count(0).expect("scalars");
        let identity_count = profile
            .request_profile()
            .identity_count(0)
            .expect("identities");
        let mut input_scalars = vec![0_u64; scalar_count];
        *input_scalars.get_mut(1).expect("K register") = u64::from(ROWS);
        let input_identities = vec![[0_u8; 32]; identity_count];
        let mut scratch_scalars = vec![0_u64; scalar_count];
        let mut scratch_identities = vec![[0_u8; 32]; identity_count];
        let mut candidate_scalars = vec![0_u64; scalar_count];
        let mut candidate_identities = vec![[0_u8; 32]; identity_count];
        let mut output_scalars = vec![output_seed; scalar_count];
        let mut output_identities = vec![[7_u8; 32]; identity_count];
        profile.project_atomic(
            request,
            ProjectionRegistersV4 {
                input_scalars: &input_scalars,
                input_identities: &input_identities,
                scratch_scalars: &mut scratch_scalars,
                scratch_identities: &mut scratch_identities,
                candidate_scalars: &mut candidate_scalars,
                candidate_identities: &mut candidate_identities,
                output_scalars: &mut output_scalars,
                output_identities: &mut output_identities,
            },
        )?;
        Ok(output_scalars)
    }

    #[test]
    fn compact_program_projects_three_rows_without_flattened_operations() {
        let bytes = profile_bytes();
        let profile =
            RequestProfileV4::decode_selected([9; 32], [9; 32], &bytes).expect("selected profile");
        assert_eq!(profile.row_geometry(), geometry());
        assert_eq!(profile.request_bytes(), Ok(160));
        let output = project(profile, &request([1, 4, 9]), 77).expect("projection");
        assert_eq!(output.get(2), Some(&1));
        assert_eq!(output.get(3), Some(&100));
        assert_eq!(output.get(4), Some(&4));
        assert_eq!(output.get(5), Some(&101));
        assert_eq!(output.get(6), Some(&9));
        assert_eq!(output.get(7), Some(&102));
    }

    #[test]
    fn omission_reordering_duplicate_and_count_substitution_refuse_atomically() {
        let bytes = profile_bytes();
        let profile = RequestProfileV4::decode(&bytes).expect("profile");
        assert_eq!(
            project(profile, &request([1, 1, 9]), 77),
            Err(Error::NonCanonicalRowOrder)
        );
        assert_eq!(
            project(profile, &request([4, 1, 9]), 77),
            Err(Error::NonCanonicalRowOrder)
        );
        let short = request([1, 4, 9]);
        assert_eq!(
            project(
                profile,
                short.get(..short.len() - ROW_BYTES).expect("short"),
                77
            ),
            Err(Error::InvalidLength)
        );
        let mut hostile = request([1, 4, 9]);
        hostile
            .get_mut(8..12)
            .expect("row count")
            .copy_from_slice(&2_u32.to_le_bytes());
        assert_eq!(project(profile, &hostile, 77), Err(Error::RowCountMismatch));
    }

    #[test]
    fn incomplete_overlapping_or_prefix_aliasing_row_program_refuses() {
        let embedded = embedded_profile();
        for hostile in [
            instructions()[..3].to_vec(),
            vec![
                RowInstructionV4::project_u32(0, 0),
                RowInstructionV4::require_zero(0, 8),
                RowInstructionV4::project_u64(8, 1),
                RowInstructionV4::project_identity(16, 0),
            ],
            vec![
                RowInstructionV4::project_u32(0, 0),
                RowInstructionV4::require_zero(4, 4),
                RowInstructionV4::project_u64(8, 2),
                RowInstructionV4::project_identity(16, 0),
            ],
        ] {
            let bytes = REQUEST_PROFILE_V4_HEADER_BYTES
                + embedded.len()
                + hostile.len() * REQUEST_PROFILE_V4_ROW_OPERATION_BYTES;
            let mut scratch = vec![0_u8; bytes];
            let mut output = vec![0_u8; bytes];
            assert!(
                encode_request_profile_v4_atomic(
                    &embedded,
                    geometry(),
                    &hostile,
                    &mut scratch,
                    &mut output,
                )
                .is_err()
            );
            assert!(output.iter().all(|value| *value == 0));
        }

        let aliasing_embedded = embedded_profile_aliasing_row();
        let bytes = REQUEST_PROFILE_V4_HEADER_BYTES
            + aliasing_embedded.len()
            + instructions().len() * REQUEST_PROFILE_V4_ROW_OPERATION_BYTES;
        let mut scratch = vec![0_u8; bytes];
        let mut output = vec![0_u8; bytes];
        assert_eq!(
            encode_request_profile_v4_atomic(
                &aliasing_embedded,
                geometry(),
                &instructions(),
                &mut scratch,
                &mut output,
            ),
            Err(Error::InvalidEmbeddedProfile)
        );
        assert!(output.iter().all(|value| *value == 0));
    }
}
