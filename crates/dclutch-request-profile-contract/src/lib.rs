#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Descriptor-selected request-byte validation and register projection.
//!
//! One finalized RequestProfile owns a fixed request prefix and one item
//! template repeated for the Product-authenticated `u32` tail count. It checks
//! exact magic/version/action/reserved fields and projects typed values into
//! caller-owned TransitionVM banks without allocation. Candidate outputs are
//! unchanged on every refusal.

use core::convert::TryInto;

/// Safe, allocation-free typed RequestProfile V1 artifact encoder.
pub mod encode;
/// Descriptor-selected RequestProfile V2 with generic native-signature evidence.
pub mod v2;
/// Descriptor-selected RequestProfile V3 with one exact borrowed child witness.
pub mod v3;

#[rustfmt::skip]
#[allow(missing_docs)]
mod generated;

pub use generated::{
    REQUEST_PROFILE_ARTIFACT_PROFILE_V1 as ARTIFACT_PROFILE,
    REQUEST_PROFILE_HEADER_BYTES_V1 as HEADER_BYTES, REQUEST_PROFILE_MAGIC_V1 as MAGIC,
    REQUEST_PROFILE_MAX_BYTES_V1 as MAX_BYTES,
    REQUEST_PROFILE_OPERATION_BYTES_V1 as OPERATION_BYTES,
    REQUEST_PROFILE_SCHEMA_RELEASE_ID_V1 as SCHEMA_RELEASE_ID,
    REQUEST_PROFILE_SCHEMA_RELEASE_PREIMAGE_V1 as SCHEMA_RELEASE_PREIMAGE,
    REQUEST_PROFILE_SCHEMA_VERSION_V1 as VERSION,
};

use generated::{
    OP_PROJECT_IDENTITY, OP_PROJECT_U8, OP_PROJECT_U16, OP_PROJECT_U32, OP_PROJECT_U64,
    OP_REQUIRE_U8, OP_REQUIRE_U16, OP_REQUIRE_U32, OP_REQUIRE_U64, OP_REQUIRE_ZERO_RANGE,
    REQUEST_OPERATION_IMMEDIATE_OFFSET, REQUEST_OPERATION_OPCODE_OFFSET,
    REQUEST_OPERATION_REGISTER_OFFSET, REQUEST_OPERATION_REGISTER_SPACE_OFFSET,
    REQUEST_OPERATION_REQUEST_OFFSET_OFFSET, REQUEST_OPERATION_REQUEST_SPACE_OFFSET,
    REQUEST_OPERATION_RESERVED_BYTE_OFFSET, REQUEST_OPERATION_RESERVED_OFFSET,
    REQUEST_OPERATION_RESERVED_SHORT_OFFSET, REQUEST_PROFILE_ARTIFACT_OFFSET,
    REQUEST_PROFILE_COMMON_IDENTITIES_OFFSET, REQUEST_PROFILE_COMMON_SCALARS_OFFSET,
    REQUEST_PROFILE_FIXED_OPERATIONS_OFFSET, REQUEST_PROFILE_FIXED_REQUEST_BYTES_OFFSET,
    REQUEST_PROFILE_ITEM_IDENTITY_STRIDE_OFFSET, REQUEST_PROFILE_ITEM_OPERATIONS_OFFSET,
    REQUEST_PROFILE_ITEM_REQUEST_BYTES_OFFSET, REQUEST_PROFILE_ITEM_SCALAR_STRIDE_OFFSET,
    REQUEST_PROFILE_MAGIC_OFFSET, REQUEST_PROFILE_VERSION_OFFSET,
};

#[cfg(test)]
use generated::{
    AGREEMENT_PROFILE_V1, AGREEMENT_REQUEST_TAIL2_V1, OVERSIZED_PROFILE_V1,
    PROFILE_REFUSAL_CORPUS_V1, REQUEST_REFUSAL_CORPUS_TAIL2_V1,
};

/// Stable hostile-decode or request-projection refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Selected/authenticated RequestProfile identity was zero.
    ZeroProgramIdentity,
    /// Selected and independently authenticated finalized content differed.
    ProgramIdentityMismatch,
    /// Program, request, or caller-owned banks had another exact width.
    InvalidLength,
    /// Program magic selected another interpreter.
    InvalidMagic,
    /// Schema version or artifact profile was unsupported.
    UnsupportedProfile,
    /// Header or operation reserved fields were nonzero.
    NonCanonicalReserved,
    /// The program exposed no request or register bank.
    EmptyProgram,
    /// An opcode or fixed/item space tag was unsupported.
    UnknownOperation,
    /// Active and inactive fields were noncanonical.
    NonCanonicalOperation,
    /// Request or register coordinate was outside its declared address space.
    InvalidCoordinate,
    /// Two operations projected one register in the same fixed/item body.
    DuplicateProjection,
    /// Required magic/version/action/reserved bytes differed.
    CheckFailed,
    /// Checked affine width arithmetic overflowed.
    ArithmeticOverflow,
}

/// Result alias for RequestProfile operations.
pub type Result<T> = core::result::Result<T, Error>;

/// Caller-owned register banks for failure-atomic request projection.
pub struct ProjectionRegistersV1<'a> {
    /// Immutable scalar input.
    pub input_scalars: &'a [u64],
    /// Immutable identity input.
    pub input_identities: &'a [[u8; 32]],
    /// Scratch scalar candidate; may change on refusal.
    pub scratch_scalars: &'a mut [u64],
    /// Scratch identity candidate; may change on refusal.
    pub scratch_identities: &'a mut [[u8; 32]],
    /// Scalar output; changed only on success.
    pub output_scalars: &'a mut [u64],
    /// Identity output; changed only on success.
    pub output_identities: &'a mut [[u8; 32]],
}

/// Borrowed hostile-decoded RequestProfile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestProfileV1<'a> {
    fixed_request_bytes: u32,
    item_request_bytes: u32,
    fixed_operations: u16,
    item_operations: u16,
    common_scalars: u16,
    item_scalar_stride: u16,
    common_identities: u16,
    item_identity_stride: u16,
    bytes: &'a [u8],
}

impl<'a> RequestProfileV1<'a> {
    /// Decode only after descriptor selection joins authenticated raw bytes.
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

    /// Hostile-decode and prevalidate one complete RequestProfile.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < HEADER_BYTES || bytes.len() > MAX_BYTES {
            return Err(Error::InvalidLength);
        }
        if bytes.get(REQUEST_PROFILE_MAGIC_OFFSET..REQUEST_PROFILE_MAGIC_OFFSET + MAGIC.len())
            != Some(MAGIC.as_slice())
        {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, REQUEST_PROFILE_VERSION_OFFSET)? != VERSION
            || read_u16(bytes, REQUEST_PROFILE_ARTIFACT_OFFSET)? != ARTIFACT_PROFILE
        {
            return Err(Error::UnsupportedProfile);
        }
        let value = Self {
            fixed_request_bytes: read_u32(bytes, REQUEST_PROFILE_FIXED_REQUEST_BYTES_OFFSET)?,
            item_request_bytes: read_u32(bytes, REQUEST_PROFILE_ITEM_REQUEST_BYTES_OFFSET)?,
            fixed_operations: read_u16(bytes, REQUEST_PROFILE_FIXED_OPERATIONS_OFFSET)?,
            item_operations: read_u16(bytes, REQUEST_PROFILE_ITEM_OPERATIONS_OFFSET)?,
            common_scalars: read_u16(bytes, REQUEST_PROFILE_COMMON_SCALARS_OFFSET)?,
            item_scalar_stride: read_u16(bytes, REQUEST_PROFILE_ITEM_SCALAR_STRIDE_OFFSET)?,
            common_identities: read_u16(bytes, REQUEST_PROFILE_COMMON_IDENTITIES_OFFSET)?,
            item_identity_stride: read_u16(bytes, REQUEST_PROFILE_ITEM_IDENTITY_STRIDE_OFFSET)?,
            bytes,
        };
        if value.fixed_request_bytes == 0
            || value.fixed_operations == 0
            || (value.common_scalars == 0
                && value.item_scalar_stride == 0
                && value.common_identities == 0
                && value.item_identity_stride == 0)
            || ((value.item_request_bytes == 0) != (value.item_operations == 0))
            || (value.item_operations != 0
                && value.item_scalar_stride == 0
                && value.item_identity_stride == 0)
        {
            return Err(Error::EmptyProgram);
        }
        let operations = usize::from(value.fixed_operations)
            .checked_add(usize::from(value.item_operations))
            .ok_or(Error::InvalidLength)?;
        let expected = HEADER_BYTES
            .checked_add(
                operations
                    .checked_mul(OPERATION_BYTES)
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)?;
        if bytes.len() != expected {
            return Err(Error::InvalidLength);
        }
        let mut index = 0_u16;
        while index < value.fixed_operations {
            let operation = value.operation(false, index)?;
            operation.validate(value, false)?;
            value.require_unique_projection(false, index, operation)?;
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        index = 0;
        while index < value.item_operations {
            let operation = value.operation(true, index)?;
            operation.validate(value, true)?;
            value.require_unique_projection(true, index, operation)?;
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(value)
    }

    /// Exact fixed request-prefix width.
    pub const fn fixed_request_bytes(self) -> u32 {
        self.fixed_request_bytes
    }

    /// Exact bytes repeated per Product-authenticated item.
    pub const fn item_request_bytes(self) -> u32 {
        self.item_request_bytes
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

    /// Exact Product-count-derived request width.
    pub fn request_bytes(self, tail_count: u32) -> Result<usize> {
        affine_width(
            self.fixed_request_bytes,
            self.item_request_bytes,
            tail_count,
        )
    }

    /// Exact Product-count-derived scalar width.
    pub fn scalar_count(self, tail_count: u32) -> Result<usize> {
        affine_width_u16(self.common_scalars, self.item_scalar_stride, tail_count)
    }

    /// Exact Product-count-derived identity width.
    pub fn identity_count(self, tail_count: u32) -> Result<usize> {
        affine_width_u16(
            self.common_identities,
            self.item_identity_stride,
            tail_count,
        )
    }

    /// Borrow exact canonical program bytes.
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
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
        let offset = HEADER_BYTES
            .checked_add(
                ordinal
                    .checked_mul(OPERATION_BYTES)
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)?;
        Operation::decode(self.bytes, offset)
    }

    fn require_unique_projection(
        self,
        item_body: bool,
        index: u16,
        operation: Operation,
    ) -> Result<()> {
        let Some(target) = operation.projection_target() else {
            return Ok(());
        };
        let mut prior = 0_u16;
        while prior < index {
            if self.operation(item_body, prior)?.projection_target() == Some(target) {
                return Err(Error::DuplicateProjection);
            }
            prior = prior.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Operation {
    opcode: u8,
    request_item: bool,
    register_item: bool,
    request_offset: u32,
    register: u16,
    immediate: u64,
}

impl Operation {
    fn decode(bytes: &[u8], offset: usize) -> Result<Self> {
        let request_space = byte(bytes, add(offset, REQUEST_OPERATION_REQUEST_SPACE_OFFSET)?)?;
        let register_space = byte(bytes, add(offset, REQUEST_OPERATION_REGISTER_SPACE_OFFSET)?)?;
        if request_space > 1
            || register_space > 1
            || byte(bytes, add(offset, REQUEST_OPERATION_RESERVED_BYTE_OFFSET)?)? != 0
            || read_u16(bytes, add(offset, REQUEST_OPERATION_RESERVED_SHORT_OFFSET)?)? != 0
            || read_u32(bytes, add(offset, REQUEST_OPERATION_RESERVED_OFFSET)?)? != 0
        {
            return Err(Error::NonCanonicalReserved);
        }
        Ok(Self {
            opcode: byte(bytes, add(offset, REQUEST_OPERATION_OPCODE_OFFSET)?)?,
            request_item: request_space == 1,
            register_item: register_space == 1,
            request_offset: read_u32(bytes, add(offset, REQUEST_OPERATION_REQUEST_OFFSET_OFFSET)?)?,
            register: read_u16(bytes, add(offset, REQUEST_OPERATION_REGISTER_OFFSET)?)?,
            immediate: read_u64(bytes, add(offset, REQUEST_OPERATION_IMMEDIATE_OFFSET)?)?,
        })
    }

    fn validate(self, profile: RequestProfileV1<'_>, item_body: bool) -> Result<()> {
        if self.request_item != item_body
            || (!item_body && self.register_item)
            || (item_body && self.is_projection() && !self.register_item)
        {
            return Err(Error::NonCanonicalOperation);
        }
        let request_bound = if item_body {
            profile.item_request_bytes
        } else {
            profile.fixed_request_bytes
        };
        let width = self.read_width()?;
        if self
            .request_offset
            .checked_add(width)
            .filter(|end| *end <= request_bound)
            .is_none()
        {
            return Err(Error::InvalidCoordinate);
        }
        if self.is_projection() {
            let identity = self.opcode == OP_PROJECT_IDENTITY;
            let bound = match (identity, self.register_item) {
                (true, true) => profile.item_identity_stride,
                (true, false) => profile.common_identities,
                (false, true) => profile.item_scalar_stride,
                (false, false) => profile.common_scalars,
            };
            if self.register >= bound || self.immediate != 0 {
                return Err(Error::InvalidCoordinate);
            }
        } else if self.register != 0 || self.register_item {
            return Err(Error::NonCanonicalOperation);
        }
        if self.opcode == OP_REQUIRE_ZERO_RANGE && self.immediate == 0 {
            return Err(Error::NonCanonicalOperation);
        }
        Ok(())
    }

    fn is_projection(self) -> bool {
        matches!(
            self.opcode,
            OP_PROJECT_U8 | OP_PROJECT_U16 | OP_PROJECT_U32 | OP_PROJECT_U64 | OP_PROJECT_IDENTITY
        )
    }

    fn projection_target(self) -> Option<(bool, bool, u16)> {
        self.is_projection().then_some((
            self.opcode == OP_PROJECT_IDENTITY,
            self.register_item,
            self.register,
        ))
    }

    fn read_width(self) -> Result<u32> {
        match self.opcode {
            OP_REQUIRE_U8 | OP_PROJECT_U8 => Ok(1),
            OP_REQUIRE_U16 | OP_PROJECT_U16 => Ok(2),
            OP_REQUIRE_U32 | OP_PROJECT_U32 => Ok(4),
            OP_REQUIRE_U64 | OP_PROJECT_U64 => Ok(8),
            OP_PROJECT_IDENTITY => Ok(32),
            OP_REQUIRE_ZERO_RANGE => {
                u32::try_from(self.immediate).map_err(|_| Error::InvalidCoordinate)
            }
            _ => Err(Error::UnknownOperation),
        }
    }

    fn apply(
        self,
        profile: RequestProfileV1<'_>,
        item: Option<u32>,
        request: &[u8],
        scalars: &mut [u64],
        identities: &mut [[u8; 32]],
    ) -> Result<()> {
        let start = request_index(profile, self.request_item, item, self.request_offset)?;
        let width = usize::try_from(self.read_width()?).map_err(|_| Error::InvalidCoordinate)?;
        let field = request
            .get(start..start.checked_add(width).ok_or(Error::ArithmeticOverflow)?)
            .ok_or(Error::InvalidCoordinate)?;
        match self.opcode {
            OP_REQUIRE_U8 => require(
                u64::from(*field.first().ok_or(Error::InvalidCoordinate)?) == self.immediate,
            ),
            OP_REQUIRE_U16 => {
                require(u64::from(u16::from_le_bytes(array(field)?)) == self.immediate)
            }
            OP_REQUIRE_U32 => {
                require(u64::from(u32::from_le_bytes(array(field)?)) == self.immediate)
            }
            OP_REQUIRE_U64 => require(u64::from_le_bytes(array(field)?) == self.immediate),
            OP_REQUIRE_ZERO_RANGE => require(field.iter().all(|byte| *byte == 0)),
            OP_PROJECT_U8 => self.write_scalar(
                profile,
                item,
                scalars,
                u64::from(*field.first().ok_or(Error::InvalidCoordinate)?),
            ),
            OP_PROJECT_U16 => self.write_scalar(
                profile,
                item,
                scalars,
                u64::from(u16::from_le_bytes(array(field)?)),
            ),
            OP_PROJECT_U32 => self.write_scalar(
                profile,
                item,
                scalars,
                u64::from(u32::from_le_bytes(array(field)?)),
            ),
            OP_PROJECT_U64 => {
                self.write_scalar(profile, item, scalars, u64::from_le_bytes(array(field)?))
            }
            OP_PROJECT_IDENTITY => self.write_identity(
                profile,
                item,
                identities,
                field.try_into().map_err(|_| Error::InvalidCoordinate)?,
            ),
            _ => Err(Error::UnknownOperation),
        }
    }

    fn write_scalar(
        self,
        profile: RequestProfileV1<'_>,
        item: Option<u32>,
        output: &mut [u64],
        value: u64,
    ) -> Result<()> {
        let index = register_index(
            profile.common_scalars,
            profile.item_scalar_stride,
            self.register_item,
            item,
            self.register,
        )?;
        *output.get_mut(index).ok_or(Error::InvalidCoordinate)? = value;
        Ok(())
    }

    fn write_identity(
        self,
        profile: RequestProfileV1<'_>,
        item: Option<u32>,
        output: &mut [[u8; 32]],
        value: [u8; 32],
    ) -> Result<()> {
        let index = register_index(
            profile.common_identities,
            profile.item_identity_stride,
            self.register_item,
            item,
            self.register,
        )?;
        *output.get_mut(index).ok_or(Error::InvalidCoordinate)? = value;
        Ok(())
    }
}

/// Validate the complete request and project all typed fields atomically.
pub fn project_atomic(
    profile: RequestProfileV1<'_>,
    tail_count: u32,
    request: &[u8],
    registers: ProjectionRegistersV1<'_>,
) -> Result<()> {
    let scalar_count = profile.scalar_count(tail_count)?;
    let identity_count = profile.identity_count(tail_count)?;
    if request.len() != profile.request_bytes(tail_count)?
        || registers.input_scalars.len() != scalar_count
        || registers.scratch_scalars.len() != scalar_count
        || registers.output_scalars.len() != scalar_count
        || registers.input_identities.len() != identity_count
        || registers.scratch_identities.len() != identity_count
        || registers.output_identities.len() != identity_count
    {
        return Err(Error::InvalidLength);
    }
    registers
        .scratch_scalars
        .copy_from_slice(registers.input_scalars);
    registers
        .scratch_identities
        .copy_from_slice(registers.input_identities);
    let mut operation = 0_u16;
    while operation < profile.fixed_operations {
        profile.operation(false, operation)?.apply(
            profile,
            None,
            request,
            registers.scratch_scalars,
            registers.scratch_identities,
        )?;
        operation = operation.checked_add(1).ok_or(Error::InvalidLength)?;
    }
    let mut item = 0_u32;
    while item < tail_count {
        operation = 0;
        while operation < profile.item_operations {
            profile.operation(true, operation)?.apply(
                profile,
                Some(item),
                request,
                registers.scratch_scalars,
                registers.scratch_identities,
            )?;
            operation = operation.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        item = item.checked_add(1).ok_or(Error::InvalidLength)?;
    }
    registers
        .output_scalars
        .copy_from_slice(registers.scratch_scalars);
    registers
        .output_identities
        .copy_from_slice(registers.scratch_identities);
    Ok(())
}

fn request_index(
    profile: RequestProfileV1<'_>,
    item_space: bool,
    item: Option<u32>,
    local: u32,
) -> Result<usize> {
    if !item_space {
        return usize::try_from(local).map_err(|_| Error::InvalidCoordinate);
    }
    let item = item.ok_or(Error::InvalidCoordinate)?;
    let value = u64::from(profile.item_request_bytes)
        .checked_mul(u64::from(item))
        .and_then(|value| value.checked_add(u64::from(profile.fixed_request_bytes)))
        .and_then(|value| value.checked_add(u64::from(local)))
        .ok_or(Error::ArithmeticOverflow)?;
    usize::try_from(value).map_err(|_| Error::ArithmeticOverflow)
}

fn register_index(
    common: u16,
    stride: u16,
    item_space: bool,
    item: Option<u32>,
    local: u16,
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
    let value = u64::from(stride)
        .checked_mul(u64::from(item))
        .and_then(|value| value.checked_add(u64::from(common)))
        .and_then(|value| value.checked_add(u64::from(local)))
        .ok_or(Error::ArithmeticOverflow)?;
    usize::try_from(value).map_err(|_| Error::ArithmeticOverflow)
}

fn affine_width(common: u32, stride: u32, count: u32) -> Result<usize> {
    let value = u64::from(stride)
        .checked_mul(u64::from(count))
        .and_then(|value| value.checked_add(u64::from(common)))
        .ok_or(Error::ArithmeticOverflow)?;
    usize::try_from(value).map_err(|_| Error::ArithmeticOverflow)
}

fn affine_width_u16(common: u16, stride: u16, count: u32) -> Result<usize> {
    affine_width(u32::from(common), u32::from(stride), count)
}

fn require(condition: bool) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(Error::CheckFailed)
    }
}

fn array<const N: usize>(bytes: &[u8]) -> Result<[u8; N]> {
    bytes.try_into().map_err(|_| Error::InvalidCoordinate)
}

fn add(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right).ok_or(Error::InvalidLength)
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    let end = offset.checked_add(2).ok_or(Error::InvalidLength)?;
    Ok(u16::from_le_bytes(array(
        bytes.get(offset..end).ok_or(Error::InvalidLength)?,
    )?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let end = offset.checked_add(4).ok_or(Error::InvalidLength)?;
    Ok(u32::from_le_bytes(array(
        bytes.get(offset..end).ok_or(Error::InvalidLength)?,
    )?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    let end = offset.checked_add(8).ok_or(Error::InvalidLength)?;
    Ok(u64::from_le_bytes(array(
        bytes.get(offset..end).ok_or(Error::InvalidLength)?,
    )?))
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec::Vec;

    use super::*;

    fn canonical() -> Vec<u8> {
        AGREEMENT_PROFILE_V1.to_vec()
    }

    fn request() -> [u8; 96] {
        AGREEMENT_REQUEST_TAIL2_V1
    }

    #[test]
    fn fixed_and_product_tail_fields_project_atomically() {
        let bytes = canonical();
        let profile = RequestProfileV1::decode(&bytes).expect("profile");
        let request = request();
        let input_scalars = [9_u64; 3];
        let input_identities = [[9_u8; 32]; 2];
        let mut scratch_scalars = [0_u64; 3];
        let mut scratch_identities = [[0_u8; 32]; 2];
        let mut output_scalars = [8_u64; 3];
        let mut output_identities = [[8_u8; 32]; 2];
        project_atomic(
            profile,
            2,
            &request,
            ProjectionRegistersV1 {
                input_scalars: &input_scalars,
                input_identities: &input_identities,
                scratch_scalars: &mut scratch_scalars,
                scratch_identities: &mut scratch_identities,
                output_scalars: &mut output_scalars,
                output_identities: &mut output_identities,
            },
        )
        .expect("projection");
        assert_eq!(output_scalars, [9, 3, 4]);
        assert_eq!(output_identities, [[0x31; 32], [0x41; 32]]);
    }

    #[test]
    fn hostile_action_length_and_late_item_preserve_outputs() {
        let bytes = canonical();
        let profile = RequestProfileV1::decode(&bytes).expect("profile");
        for refusal in 0..3 {
            let mut request = request();
            if refusal == 0 {
                *request.get_mut(8).expect("action") = 3;
            }
            if refusal == 2 {
                *request.get_mut(60).expect("late item") ^= 1;
            }
            let input_scalars = [9_u64; 3];
            let input_identities = [[9_u8; 32]; 2];
            let mut scratch_scalars = [0_u64; 3];
            let mut scratch_identities = [[0_u8; 32]; 2];
            let mut output_scalars = [8_u64; 3];
            let mut output_identities = [[8_u8; 32]; 2];
            let before_scalars = output_scalars;
            let before_identities = output_identities;
            let used = if refusal == 1 {
                request.get(..95).expect("short")
            } else {
                request.as_slice()
            };
            let result = project_atomic(
                profile,
                2,
                used,
                ProjectionRegistersV1 {
                    input_scalars: &input_scalars,
                    input_identities: &input_identities,
                    scratch_scalars: &mut scratch_scalars,
                    scratch_identities: &mut scratch_identities,
                    output_scalars: &mut output_scalars,
                    output_identities: &mut output_identities,
                },
            );
            if refusal == 2 {
                assert!(result.is_ok());
            } else {
                assert!(result.is_err());
                assert_eq!(output_scalars, before_scalars);
                assert_eq!(output_identities, before_identities);
            }
        }
    }

    #[test]
    fn hostile_program_and_content_selection_refuse() {
        for hostile in &PROFILE_REFUSAL_CORPUS_V1 {
            assert!(RequestProfileV1::decode(hostile).is_err());
        }
        let canonical = canonical();
        assert_eq!(
            RequestProfileV1::decode_selected([1; 32], [2; 32], &canonical),
            Err(Error::ProgramIdentityMismatch)
        );
    }

    #[test]
    fn generated_request_refusals_preserve_outputs() {
        let bytes = canonical();
        let profile = RequestProfileV1::decode(&bytes).expect("profile");
        for request in &REQUEST_REFUSAL_CORPUS_TAIL2_V1 {
            let input_scalars = [9_u64; 3];
            let input_identities = [[9_u8; 32]; 2];
            let mut scratch_scalars = [0_u64; 3];
            let mut scratch_identities = [[0_u8; 32]; 2];
            let mut output_scalars = [8_u64; 3];
            let mut output_identities = [[8_u8; 32]; 2];
            let before_scalars = output_scalars;
            let before_identities = output_identities;
            assert!(
                project_atomic(
                    profile,
                    2,
                    request,
                    ProjectionRegistersV1 {
                        input_scalars: &input_scalars,
                        input_identities: &input_identities,
                        scratch_scalars: &mut scratch_scalars,
                        scratch_identities: &mut scratch_identities,
                        output_scalars: &mut output_scalars,
                        output_identities: &mut output_identities,
                    },
                )
                .is_err()
            );
            assert_eq!(output_scalars, before_scalars);
            assert_eq!(output_identities, before_identities);
        }
    }

    #[test]
    fn finalized_record_cap_refuses_internally_consistent_oversized_profile() {
        assert_eq!(OVERSIZED_PROFILE_V1.len(), 1328);
        assert_eq!(
            RequestProfileV1::decode(&OVERSIZED_PROFILE_V1),
            Err(Error::InvalidLength)
        );
    }

    #[test]
    fn item_projection_cannot_overwrite_a_common_register() {
        let mut bytes = canonical();
        let first_item_operation = HEADER_BYTES + 3 * OPERATION_BYTES;
        *bytes
            .get_mut(first_item_operation + REQUEST_OPERATION_REGISTER_SPACE_OFFSET)
            .expect("register-space byte") = 0;
        assert_eq!(
            RequestProfileV1::decode(&bytes),
            Err(Error::NonCanonicalOperation)
        );
    }
}
