//! Safe, allocation-free RequestProfile V1 artifact encoder.
//!
//! Typed constructors keep opcode authority in this crate. The public encoder
//! writes a caller-owned scratch candidate, hostile-decodes the complete
//! candidate with [`RequestProfileV1`], and copies to output only on success.

use super::{
    ARTIFACT_PROFILE, Error, HEADER_BYTES, MAGIC, MAX_BYTES, OPERATION_BYTES, RequestProfileV1,
    VERSION,
    generated::{
        OP_PROJECT_IDENTITY, OP_PROJECT_U8, OP_PROJECT_U16, OP_PROJECT_U32, OP_PROJECT_U64,
        OP_REQUIRE_U8, OP_REQUIRE_U16, OP_REQUIRE_U32, OP_REQUIRE_U64, OP_REQUIRE_ZERO_RANGE,
        REQUEST_OPERATION_IMMEDIATE_OFFSET, REQUEST_OPERATION_OPCODE_OFFSET,
        REQUEST_OPERATION_REGISTER_OFFSET, REQUEST_OPERATION_REGISTER_SPACE_OFFSET,
        REQUEST_OPERATION_REQUEST_OFFSET_OFFSET, REQUEST_OPERATION_REQUEST_SPACE_OFFSET,
        REQUEST_PROFILE_ARTIFACT_OFFSET, REQUEST_PROFILE_COMMON_IDENTITIES_OFFSET,
        REQUEST_PROFILE_COMMON_SCALARS_OFFSET, REQUEST_PROFILE_FIXED_OPERATIONS_OFFSET,
        REQUEST_PROFILE_FIXED_REQUEST_BYTES_OFFSET, REQUEST_PROFILE_ITEM_IDENTITY_STRIDE_OFFSET,
        REQUEST_PROFILE_ITEM_OPERATIONS_OFFSET, REQUEST_PROFILE_ITEM_REQUEST_BYTES_OFFSET,
        REQUEST_PROFILE_ITEM_SCALAR_STRIDE_OFFSET, REQUEST_PROFILE_MAGIC_OFFSET,
        REQUEST_PROFILE_VERSION_OFFSET,
    },
};

/// Fixed request prefix or one Product-item request body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestSpaceV1 {
    /// Fixed request prefix.
    Fixed,
    /// Per-Product-item request body.
    Item,
}

/// One checked request-byte coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestCoordinateV1 {
    space: RequestSpaceV1,
    offset: u32,
}

impl RequestCoordinateV1 {
    /// Address one field in the fixed request prefix.
    pub const fn fixed(offset: u32) -> Self {
        Self {
            space: RequestSpaceV1::Fixed,
            offset,
        }
    }

    /// Address one field in each repeated Product-item request body.
    pub const fn item(offset: u32) -> Self {
        Self {
            space: RequestSpaceV1::Item,
            offset,
        }
    }
}

/// Common or per-Product-item output register.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegisterSpaceV1 {
    /// Common register bank.
    Common,
    /// Repeated Product-item register bank.
    Item,
}

/// One checked scalar-register coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarRegisterV1 {
    space: RegisterSpaceV1,
    index: u16,
}

impl ScalarRegisterV1 {
    /// Address one common scalar.
    pub const fn common(index: u16) -> Self {
        Self {
            space: RegisterSpaceV1::Common,
            index,
        }
    }

    /// Address one scalar in each repeated Product-item bank.
    pub const fn item(index: u16) -> Self {
        Self {
            space: RegisterSpaceV1::Item,
            index,
        }
    }
}

/// One checked identity-register coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdentityRegisterV1 {
    space: RegisterSpaceV1,
    index: u16,
}

impl IdentityRegisterV1 {
    /// Address one common identity.
    pub const fn common(index: u16) -> Self {
        Self {
            space: RegisterSpaceV1::Common,
            index,
        }
    }

    /// Address one identity in each repeated Product-item bank.
    pub const fn item(index: u16) -> Self {
        Self {
            space: RegisterSpaceV1::Item,
            index,
        }
    }
}

/// One typed RequestProfile instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestInstructionV1 {
    opcode: u8,
    request: RequestCoordinateV1,
    register: Option<(RegisterSpaceV1, u16)>,
    immediate: u64,
}

impl RequestInstructionV1 {
    /// Require one exact byte.
    pub const fn require_u8(request: RequestCoordinateV1, value: u8) -> Self {
        Self::require(OP_REQUIRE_U8, request, value as u64)
    }

    /// Require one exact little-endian `u16`.
    pub const fn require_u16(request: RequestCoordinateV1, value: u16) -> Self {
        Self::require(OP_REQUIRE_U16, request, value as u64)
    }

    /// Require one exact little-endian `u32`.
    pub const fn require_u32(request: RequestCoordinateV1, value: u32) -> Self {
        Self::require(OP_REQUIRE_U32, request, value as u64)
    }

    /// Require one exact little-endian `u64`.
    pub const fn require_u64(request: RequestCoordinateV1, value: u64) -> Self {
        Self::require(OP_REQUIRE_U64, request, value)
    }

    /// Require one nonempty exact zero range.
    pub const fn require_zero(request: RequestCoordinateV1, bytes: u32) -> Self {
        Self::require(OP_REQUIRE_ZERO_RANGE, request, bytes as u64)
    }

    /// Project one byte into a scalar.
    pub const fn project_u8(request: RequestCoordinateV1, register: ScalarRegisterV1) -> Self {
        Self::project(OP_PROJECT_U8, request, register.space, register.index)
    }

    /// Project one little-endian `u16` into a scalar.
    pub const fn project_u16(request: RequestCoordinateV1, register: ScalarRegisterV1) -> Self {
        Self::project(OP_PROJECT_U16, request, register.space, register.index)
    }

    /// Project one little-endian `u32` into a scalar.
    pub const fn project_u32(request: RequestCoordinateV1, register: ScalarRegisterV1) -> Self {
        Self::project(OP_PROJECT_U32, request, register.space, register.index)
    }

    /// Project one little-endian `u64` into a scalar.
    pub const fn project_u64(request: RequestCoordinateV1, register: ScalarRegisterV1) -> Self {
        Self::project(OP_PROJECT_U64, request, register.space, register.index)
    }

    /// Project one exact 32-byte identity.
    pub const fn project_identity(
        request: RequestCoordinateV1,
        register: IdentityRegisterV1,
    ) -> Self {
        Self::project(OP_PROJECT_IDENTITY, request, register.space, register.index)
    }

    const fn require(opcode: u8, request: RequestCoordinateV1, immediate: u64) -> Self {
        Self {
            opcode,
            request,
            register: None,
            immediate,
        }
    }

    const fn project(
        opcode: u8,
        request: RequestCoordinateV1,
        space: RegisterSpaceV1,
        index: u16,
    ) -> Self {
        Self {
            opcode,
            request,
            register: Some((space, index)),
            immediate: 0,
        }
    }
}

/// Exact fixed and affine RequestProfile geometry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestGeometryV1 {
    fixed_request_bytes: u32,
    item_request_bytes: u32,
    common_scalars: u16,
    item_scalar_stride: u16,
    common_identities: u16,
    item_identity_stride: u16,
}

impl RequestGeometryV1 {
    /// Construct exact request and register-bank geometry.
    pub const fn new(
        fixed_request_bytes: u32,
        item_request_bytes: u32,
        common_scalars: u16,
        item_scalar_stride: u16,
        common_identities: u16,
        item_identity_stride: u16,
    ) -> Self {
        Self {
            fixed_request_bytes,
            item_request_bytes,
            common_scalars,
            item_scalar_stride,
            common_identities,
            item_identity_stride,
        }
    }
}

/// Encode one complete RequestProfile V1 into caller-owned buffers atomically.
pub fn encode_request_profile_v1_atomic(
    geometry: RequestGeometryV1,
    fixed_instructions: &[RequestInstructionV1],
    item_instructions: &[RequestInstructionV1],
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), Error> {
    let fixed_count = u16::try_from(fixed_instructions.len()).map_err(|_| Error::InvalidLength)?;
    let item_count = u16::try_from(item_instructions.len()).map_err(|_| Error::InvalidLength)?;
    let expected = fixed_instructions
        .len()
        .checked_add(item_instructions.len())
        .and_then(|count| count.checked_mul(OPERATION_BYTES))
        .and_then(|body| HEADER_BYTES.checked_add(body))
        .ok_or(Error::InvalidLength)?;
    if expected > MAX_BYTES || scratch.len() != expected || output.len() != expected {
        return Err(Error::InvalidLength);
    }
    scratch.fill(0);
    write(scratch, REQUEST_PROFILE_MAGIC_OFFSET, &MAGIC)?;
    write(
        scratch,
        REQUEST_PROFILE_VERSION_OFFSET,
        &VERSION.to_le_bytes(),
    )?;
    write(
        scratch,
        REQUEST_PROFILE_ARTIFACT_OFFSET,
        &ARTIFACT_PROFILE.to_le_bytes(),
    )?;
    for (offset, value) in [
        (
            REQUEST_PROFILE_FIXED_REQUEST_BYTES_OFFSET,
            geometry.fixed_request_bytes,
        ),
        (
            REQUEST_PROFILE_ITEM_REQUEST_BYTES_OFFSET,
            geometry.item_request_bytes,
        ),
    ] {
        write(scratch, offset, &value.to_le_bytes())?;
    }
    for (offset, value) in [
        (REQUEST_PROFILE_FIXED_OPERATIONS_OFFSET, fixed_count),
        (REQUEST_PROFILE_ITEM_OPERATIONS_OFFSET, item_count),
        (
            REQUEST_PROFILE_COMMON_SCALARS_OFFSET,
            geometry.common_scalars,
        ),
        (
            REQUEST_PROFILE_ITEM_SCALAR_STRIDE_OFFSET,
            geometry.item_scalar_stride,
        ),
        (
            REQUEST_PROFILE_COMMON_IDENTITIES_OFFSET,
            geometry.common_identities,
        ),
        (
            REQUEST_PROFILE_ITEM_IDENTITY_STRIDE_OFFSET,
            geometry.item_identity_stride,
        ),
    ] {
        write(scratch, offset, &value.to_le_bytes())?;
    }
    let mut cursor = HEADER_BYTES;
    for instruction in fixed_instructions {
        encode_instruction(*instruction, RequestSpaceV1::Fixed, scratch, cursor)?;
        cursor = cursor
            .checked_add(OPERATION_BYTES)
            .ok_or(Error::InvalidLength)?;
    }
    for instruction in item_instructions {
        encode_instruction(*instruction, RequestSpaceV1::Item, scratch, cursor)?;
        cursor = cursor
            .checked_add(OPERATION_BYTES)
            .ok_or(Error::InvalidLength)?;
    }
    if cursor != expected {
        return Err(Error::InvalidLength);
    }
    RequestProfileV1::decode(scratch)?;
    output.copy_from_slice(scratch);
    Ok(())
}

fn encode_instruction(
    instruction: RequestInstructionV1,
    body: RequestSpaceV1,
    output: &mut [u8],
    offset: usize,
) -> Result<(), Error> {
    if instruction.request.space != body {
        return Err(Error::NonCanonicalOperation);
    }
    write_byte(
        output,
        add(offset, REQUEST_OPERATION_OPCODE_OFFSET)?,
        instruction.opcode,
    )?;
    write_byte(
        output,
        add(offset, REQUEST_OPERATION_REQUEST_SPACE_OFFSET)?,
        u8::from(body == RequestSpaceV1::Item),
    )?;
    write(
        output,
        add(offset, REQUEST_OPERATION_REQUEST_OFFSET_OFFSET)?,
        &instruction.request.offset.to_le_bytes(),
    )?;
    if let Some((space, register)) = instruction.register {
        write_byte(
            output,
            add(offset, REQUEST_OPERATION_REGISTER_SPACE_OFFSET)?,
            u8::from(space == RegisterSpaceV1::Item),
        )?;
        write(
            output,
            add(offset, REQUEST_OPERATION_REGISTER_OFFSET)?,
            &register.to_le_bytes(),
        )?;
    }
    write(
        output,
        add(offset, REQUEST_OPERATION_IMMEDIATE_OFFSET)?,
        &instruction.immediate.to_le_bytes(),
    )
}

fn add(left: usize, right: usize) -> Result<usize, Error> {
    left.checked_add(right).ok_or(Error::InvalidLength)
}

fn write(output: &mut [u8], offset: usize, bytes: &[u8]) -> Result<(), Error> {
    let end = offset
        .checked_add(bytes.len())
        .ok_or(Error::InvalidLength)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(bytes);
    Ok(())
}

fn write_byte(output: &mut [u8], offset: usize, value: u8) -> Result<(), Error> {
    *output.get_mut(offset).ok_or(Error::InvalidLength)? = value;
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::{ProjectionRegistersV1, project_atomic};

    #[test]
    fn typed_encoder_round_trips_and_refuses_without_output_commit() {
        let fixed = [
            RequestInstructionV1::require_u16(RequestCoordinateV1::fixed(0), 7),
            RequestInstructionV1::project_u32(
                RequestCoordinateV1::fixed(2),
                ScalarRegisterV1::common(0),
            ),
            RequestInstructionV1::project_identity(
                RequestCoordinateV1::fixed(6),
                IdentityRegisterV1::common(0),
            ),
        ];
        let geometry = RequestGeometryV1::new(38, 0, 1, 0, 1, 0);
        let width = HEADER_BYTES + fixed.len() * OPERATION_BYTES;
        let mut scratch = [0_u8; HEADER_BYTES + 3 * OPERATION_BYTES];
        let mut output = [9_u8; HEADER_BYTES + 3 * OPERATION_BYTES];
        assert_eq!(output.len(), width);
        encode_request_profile_v1_atomic(geometry, &fixed, &[], &mut scratch, &mut output)
            .expect("encode");
        let profile = RequestProfileV1::decode(&output).expect("decode encoded profile");
        let mut request = [0_u8; 38];
        request
            .get_mut(..2)
            .expect("version")
            .copy_from_slice(&7_u16.to_le_bytes());
        request
            .get_mut(2..6)
            .expect("scalar")
            .copy_from_slice(&11_u32.to_le_bytes());
        request.get_mut(6..).expect("identity").fill(0x31);
        let mut scalar_scratch = [0_u64];
        let mut identity_scratch = [[0_u8; 32]];
        let mut scalar_output = [0_u64];
        let mut identity_output = [[0_u8; 32]];
        project_atomic(
            profile,
            0,
            &request,
            ProjectionRegistersV1 {
                input_scalars: &[0],
                input_identities: &[[0; 32]],
                scratch_scalars: &mut scalar_scratch,
                scratch_identities: &mut identity_scratch,
                output_scalars: &mut scalar_output,
                output_identities: &mut identity_output,
            },
        )
        .expect("project");
        assert_eq!(scalar_output, [11]);
        assert_eq!(identity_output, [[0x31; 32]]);

        let wrong_space = [RequestInstructionV1::project_u8(
            RequestCoordinateV1::item(0),
            ScalarRegisterV1::item(0),
        )];
        let mut hostile_scratch = [0_u8; HEADER_BYTES + OPERATION_BYTES];
        let mut hostile_output = [7_u8; HEADER_BYTES + OPERATION_BYTES];
        let before = hostile_output;
        assert_eq!(
            encode_request_profile_v1_atomic(
                RequestGeometryV1::new(1, 0, 1, 0, 0, 0),
                &wrong_space,
                &[],
                &mut hostile_scratch,
                &mut hostile_output,
            ),
            Err(Error::NonCanonicalOperation)
        );
        assert_eq!(hostile_output, before);
    }
}
