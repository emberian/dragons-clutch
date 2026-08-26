//! Typed RequestProfile and TransitionVM artifacts.

#[cfg(test)]
use dclutch_rational_representation_v2_lifecycle_contract::hot_v3::{
    RATIONAL_LIFECYCLE_HOT_COMMON_IDENTITIES_V3, RATIONAL_LIFECYCLE_HOT_COMMON_SCALARS_V3,
    RATIONAL_LIFECYCLE_HOT_ITEM_IDENTITIES_V3, RATIONAL_LIFECYCLE_HOT_ITEM_SCALARS_V3,
    RATIONAL_LIFECYCLE_SCALAR_COORDINATE_COUNT_V3,
};
use dclutch_rational_representation_v2_lifecycle_contract::{
    LifecycleActionV2,
    hot_v3::{
        RATIONAL_LIFECYCLE_HOT_MAGIC_V3, RATIONAL_LIFECYCLE_HOT_VERSION_V3,
        RATIONAL_LIFECYCLE_IDENTITY_DESCRIPTOR_V3, RATIONAL_LIFECYCLE_IDENTITY_GRAPH_V3,
        RATIONAL_LIFECYCLE_IDENTITY_MARKET_V3, RATIONAL_LIFECYCLE_IDENTITY_RECEIPT_MINT_V3,
        RATIONAL_LIFECYCLE_IDENTITY_RELEASE_SET_V3, RATIONAL_LIFECYCLE_IDENTITY_RENT_CREDIT_V3,
        RATIONAL_LIFECYCLE_IDENTITY_RENT_PROGRAM_V3,
        RATIONAL_LIFECYCLE_IDENTITY_REPRESENTATION_AUTHORITY_V3,
        RATIONAL_LIFECYCLE_IDENTITY_TOKEN_PROGRAM_V3,
        RATIONAL_LIFECYCLE_ITEM_IDENTITY_CUSTODY_OWNER_V3,
        RATIONAL_LIFECYCLE_ITEM_IDENTITY_CUSTODY_POSITION_V3,
        RATIONAL_LIFECYCLE_ITEM_IDENTITY_POSITION_ADMISSION_V3,
        RATIONAL_LIFECYCLE_ITEM_IDENTITY_SHARD_MINT_V3,
        RATIONAL_LIFECYCLE_ITEM_IDENTITY_STRUCTURED_CUSTODY_V3,
        RATIONAL_LIFECYCLE_ITEM_SCALAR_ADMISSION_LAMPORTS_V3,
        RATIONAL_LIFECYCLE_ITEM_SCALAR_ADMISSION_RENT_V3,
        RATIONAL_LIFECYCLE_ITEM_SCALAR_COEFFICIENT_V3, RATIONAL_LIFECYCLE_ITEM_SCALAR_OUTCOME_V3,
        RATIONAL_LIFECYCLE_ITEM_SCALAR_POSITION_LAMPORTS_V3,
        RATIONAL_LIFECYCLE_ITEM_SCALAR_POSITION_RENT_V3,
        RATIONAL_LIFECYCLE_ITEM_SCALAR_POSITION_REVISION_V3,
        RATIONAL_LIFECYCLE_ITEM_SCALAR_SHARD_LAMPORTS_V3,
        RATIONAL_LIFECYCLE_ITEM_SCALAR_SHARD_RENT_V3,
        RATIONAL_LIFECYCLE_ITEM_SCALAR_SHARD_SUPPLY_V3,
        RATIONAL_LIFECYCLE_ITEM_SCALAR_STRUCTURED_AMOUNT_V3,
        RATIONAL_LIFECYCLE_ITEM_SCALAR_STRUCTURED_LAMPORTS_V3,
        RATIONAL_LIFECYCLE_ITEM_SCALAR_STRUCTURED_RENT_V3, RATIONAL_LIFECYCLE_SCALAR_ACTION_V3,
        RATIONAL_LIFECYCLE_SCALAR_GENERATION_V3, RATIONAL_LIFECYCLE_SCALAR_MARKET_REVISION_V3,
        RATIONAL_LIFECYCLE_SCALAR_OUTCOME_COUNT_V3, RATIONAL_LIFECYCLE_SCALAR_RECEIPT_LAMPORTS_V3,
        RATIONAL_LIFECYCLE_SCALAR_RECEIPT_RENT_V3, RATIONAL_LIFECYCLE_SCALAR_RECEIPT_SUPPLY_V3,
        RATIONAL_LIFECYCLE_SCALAR_RENT_AFTER_V3, RATIONAL_LIFECYCLE_SCALAR_RENT_BEFORE_V3,
        RationalLifecycleHotLayoutV3, RationalLifecycleHotRegisterLayoutV3,
    },
};
#[cfg(test)]
use dclutch_request_profile_contract::v4::{
    REQUEST_PROFILE_V4_HEADER_BYTES, REQUEST_PROFILE_V4_ROW_OPERATION_BYTES, RowInstructionV4,
    RowProgramGeometryV4, encode_request_profile_v4_atomic,
};
use dclutch_request_profile_contract::{
    HEADER_BYTES as REQUEST_HEADER_BYTES, OPERATION_BYTES as REQUEST_OPERATION_BYTES,
    encode::{
        IdentityRegisterV1, RequestCoordinateV1, RequestGeometryV1, RequestInstructionV1,
        ScalarRegisterV1, encode_request_profile_v1_atomic,
    },
};
use dclutch_transition_vm::v3::{
    HEADER_BYTES as TRANSITION_HEADER_BYTES, INSTRUCTION_BYTES as TRANSITION_INSTRUCTION_BYTES,
    InstructionV3, ProgramGeometryV3, ScalarRegisterV3, encode_program_atomic,
};

use crate::{Error, Result, validate_action_geometry};

const BASE_REQUEST_OPERATIONS: usize = 24;
const ROW_REQUEST_OPERATIONS: usize = 20;

/// Encode the compact repeated-row RequestProfile V4 for a nonempty lifecycle support.
///
/// The embedded V1 program validates/projects only the exact 400-byte prefix.
/// One canonical 272-byte row program is repeated for the descriptor-derived
/// nonzero support count, which is supplied independently in protected scalar 7.
/// Product outcome width remains protected scalar 10 and never supplies `K`.
#[cfg(test)]
#[allow(dead_code)]
pub fn encode_rational_lifecycle_request_profile_v4(
    action: LifecycleActionV2,
    coordinate_count: u32,
) -> Result<Vec<u8>> {
    let coordinates = validate_action_geometry(action, coordinate_count)?;
    if action != LifecycleActionV2::RetireReceipt || coordinates == 0 {
        return Err(Error::ActionGeometry);
    }
    let layout = RationalLifecycleHotRegisterLayoutV3::new(coordinates);
    let fixed = base_request_instructions(action, coordinate_count, None)?;
    let embedded_geometry = RequestGeometryV1::new(
        narrow_u32(RationalLifecycleHotLayoutV3::FIXED_BYTES)?,
        0,
        narrow_u16(layout.scalar_count().ok_or(Error::InvalidLength)?)?,
        0,
        narrow_u16(layout.identity_count().ok_or(Error::InvalidLength)?)?,
        0,
    );
    let embedded_bytes = REQUEST_HEADER_BYTES
        .checked_add(
            fixed
                .len()
                .checked_mul(REQUEST_OPERATION_BYTES)
                .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)?;
    let mut embedded_scratch = vec![0_u8; embedded_bytes];
    let mut embedded_output = vec![0_u8; embedded_bytes];
    encode_request_profile_v1_atomic(
        embedded_geometry,
        &fixed,
        &[],
        &mut embedded_scratch,
        &mut embedded_output,
    )
    .map_err(Error::RequestProfile)?;

    let row_instructions = row_request_instructions_v4()?;
    let geometry = RowProgramGeometryV4 {
        expected_row_count: coordinate_count,
        row_bytes: narrow_u32(RationalLifecycleHotLayoutV3::ITEM_BYTES)?,
        request_row_count_offset: narrow_u32(RationalLifecycleHotLayoutV3::COORDINATE_COUNT)?,
        ordered_key_offset: narrow_u32(RationalLifecycleHotLayoutV3::ITEM_OUTCOME)?,
        protected_scalars: narrow_u16(RATIONAL_LIFECYCLE_HOT_COMMON_SCALARS_V3)?,
        row_scalar_stride: narrow_u16(RATIONAL_LIFECYCLE_HOT_ITEM_SCALARS_V3)?,
        protected_identities: narrow_u16(RATIONAL_LIFECYCLE_HOT_COMMON_IDENTITIES_V3)?,
        row_identity_stride: narrow_u16(RATIONAL_LIFECYCLE_HOT_ITEM_IDENTITIES_V3)?,
        row_count_common_scalar: narrow_u16(RATIONAL_LIFECYCLE_SCALAR_COORDINATE_COUNT_V3)?,
        ordered_key_row_scalar: narrow_u16(RATIONAL_LIFECYCLE_ITEM_SCALAR_OUTCOME_V3)?,
    };
    let bytes = REQUEST_PROFILE_V4_HEADER_BYTES
        .checked_add(embedded_output.len())
        .and_then(|prefix| {
            row_instructions
                .len()
                .checked_mul(REQUEST_PROFILE_V4_ROW_OPERATION_BYTES)
                .and_then(|rows| prefix.checked_add(rows))
        })
        .ok_or(Error::InvalidLength)?;
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_request_profile_v4_atomic(
        &embedded_output,
        geometry,
        &row_instructions,
        &mut scratch,
        &mut output,
    )
    .map_err(Error::RequestProfileV4)?;
    Ok(output)
}

/// Encode the exact descriptor/action-specialized RequestProfile.
///
/// This checkpoint uses RequestProfile V1's fixed operation table. Geometry
/// that cannot fit that interpreter's exact artifact bound refuses here; that
/// physical bound is not a semantic limit on Rational descriptor support.
#[cfg(test)]
pub fn encode_rational_lifecycle_request_profile_v3(
    action: LifecycleActionV2,
    coordinate_count: u32,
) -> Result<Vec<u8>> {
    let coordinates = validate_action_geometry(action, coordinate_count)?;
    if action == LifecycleActionV2::RetireReceipt {
        return encode_rational_lifecycle_request_profile_v4(action, coordinate_count);
    }
    let layout = RationalLifecycleHotRegisterLayoutV3::new(coordinates);
    let mut instructions = Vec::with_capacity(
        BASE_REQUEST_OPERATIONS
            .checked_add(
                coordinates
                    .checked_mul(ROW_REQUEST_OPERATIONS)
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)?,
    );
    instructions.extend(base_request_instructions(action, coordinate_count, None)?);
    for row in 0..coordinates {
        append_row_request_instructions(&mut instructions, layout, row)?;
    }
    let request_bytes =
        RationalLifecycleHotLayoutV3::request_bytes(coordinates).ok_or(Error::InvalidLength)?;
    let geometry = RequestGeometryV1::new(
        narrow_u32(request_bytes)?,
        0,
        narrow_u16(layout.scalar_count().ok_or(Error::InvalidLength)?)?,
        0,
        narrow_u16(layout.identity_count().ok_or(Error::InvalidLength)?)?,
        0,
    );
    let bytes = REQUEST_HEADER_BYTES
        .checked_add(
            instructions
                .len()
                .checked_mul(REQUEST_OPERATION_BYTES)
                .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)?;
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_request_profile_v1_atomic(geometry, &instructions, &[], &mut scratch, &mut output)
        .map_err(Error::RequestProfile)?;
    Ok(output)
}

/// Encode one descriptor/release/Token-bound fixed-cardinality request profile.
///
/// The successor retains the exact family wire and register geometry while
/// requiring the immutable descriptor identity and Token behavior authorities
/// selected by CapabilityV4 before projecting them into child registers.
pub fn encode_rational_lifecycle_selected_request_profile_v5(
    action: LifecycleActionV2,
    descriptor_id: [u8; 32],
    release_set: [u8; 32],
    token_program: [u8; 32],
) -> Result<Vec<u8>> {
    let coordinate_count = match action {
        LifecycleActionV2::ActivateReceipt => 0,
        LifecycleActionV2::ActivateCoordinate | LifecycleActionV2::RetireCoordinate => 1,
        LifecycleActionV2::RetireReceipt => return Err(Error::ActionGeometry),
    };
    if descriptor_id == [0; 32] || release_set == [0; 32] || token_program == [0; 32] {
        return Err(Error::ContentIdentity);
    }
    let coordinates = validate_action_geometry(action, coordinate_count)?;
    let layout = RationalLifecycleHotRegisterLayoutV3::new(coordinates);
    let mut instructions = Vec::with_capacity(
        BASE_REQUEST_OPERATIONS
            .checked_add(12)
            .and_then(|count| {
                coordinates
                    .checked_mul(ROW_REQUEST_OPERATIONS)
                    .and_then(|rows| count.checked_add(rows))
            })
            .ok_or(Error::InvalidLength)?,
    );
    instructions.extend(base_request_instructions(
        action,
        coordinate_count,
        Some(descriptor_id),
    )?);
    for row in 0..coordinates {
        append_row_request_instructions(&mut instructions, layout, row)?;
    }
    let request_bytes =
        RationalLifecycleHotLayoutV3::request_bytes(coordinates).ok_or(Error::InvalidLength)?;
    let geometry = RequestGeometryV1::new(
        narrow_u32(request_bytes)?,
        0,
        narrow_u16(layout.scalar_count().ok_or(Error::InvalidLength)?)?,
        0,
        narrow_u16(layout.identity_count().ok_or(Error::InvalidLength)?)?,
        0,
    );
    let bytes = REQUEST_HEADER_BYTES
        .checked_add(
            instructions
                .len()
                .checked_mul(REQUEST_OPERATION_BYTES)
                .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)?;
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_request_profile_v1_atomic(geometry, &instructions, &[], &mut scratch, &mut output)
        .map_err(Error::RequestProfile)?;
    Ok(output)
}

/// Encode the exact representation-width coordinate-bounds transition.
///
/// The request-owned outcome count is the Claims/native-basis width `K`.
/// Product terminal-result width `N` is independently authenticated and is
/// deliberately not equated with `K` here.
pub fn encode_rational_lifecycle_transition_v3(
    action: LifecycleActionV2,
    coordinate_count: u32,
) -> Result<Vec<u8>> {
    let coordinates = validate_action_geometry(action, coordinate_count)?;
    let layout = RationalLifecycleHotRegisterLayoutV3::new(coordinates);
    let mut instructions = Vec::with_capacity(
        coordinates
            .checked_mul(2)
            .and_then(|count| count.checked_add(1))
            .ok_or(Error::InvalidLength)?,
    );
    instructions.push(InstructionV3::nonzero(transition_scalar(
        RATIONAL_LIFECYCLE_SCALAR_OUTCOME_COUNT_V3,
    )?));
    for row in 0..coordinates {
        instructions.push(InstructionV3::scalar_lt(
            transition_scalar(
                layout
                    .coordinate_scalar(row, RATIONAL_LIFECYCLE_ITEM_SCALAR_OUTCOME_V3)
                    .ok_or(Error::InvalidLength)?,
            )?,
            transition_scalar(RATIONAL_LIFECYCLE_SCALAR_OUTCOME_COUNT_V3)?,
        ));
        instructions.push(InstructionV3::nonzero(transition_scalar(
            layout
                .coordinate_scalar(row, RATIONAL_LIFECYCLE_ITEM_SCALAR_COEFFICIENT_V3)
                .ok_or(Error::InvalidLength)?,
        )?));
    }
    let geometry = ProgramGeometryV3 {
        common_scalars: narrow_u16(layout.scalar_count().ok_or(Error::InvalidLength)?)?,
        item_scalar_stride: 0,
        common_identities: narrow_u16(layout.identity_count().ok_or(Error::InvalidLength)?)?,
        item_identity_stride: 0,
    };
    let bytes = TRANSITION_HEADER_BYTES
        .checked_add(
            instructions
                .len()
                .checked_mul(TRANSITION_INSTRUCTION_BYTES)
                .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)?;
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_program_atomic(geometry, &instructions, &[], &[], &mut scratch, &mut output)
        .map_err(Error::Transition)?;
    Ok(output)
}

fn base_request_instructions(
    action: LifecycleActionV2,
    coordinate_count: u32,
    required_descriptor: Option<[u8; 32]>,
) -> Result<Vec<RequestInstructionV1>> {
    let mut output = vec![
        require_u64(
            RationalLifecycleHotLayoutV3::MAGIC,
            u64::from_le_bytes(RATIONAL_LIFECYCLE_HOT_MAGIC_V3),
        )?,
        require_u16(
            RationalLifecycleHotLayoutV3::VERSION,
            RATIONAL_LIFECYCLE_HOT_VERSION_V3,
        )?,
        require_u8(RationalLifecycleHotLayoutV3::ACTION, action.tag())?,
        require_zero(RationalLifecycleHotLayoutV3::RESERVED_HEADER, 5)?,
        require_zero(RationalLifecycleHotLayoutV3::PARENT_CONTEXT, 32)?,
        project_identity(
            RationalLifecycleHotLayoutV3::RELEASE_SET,
            RATIONAL_LIFECYCLE_IDENTITY_RELEASE_SET_V3,
        )?,
        project_identity(
            RationalLifecycleHotLayoutV3::MARKET,
            RATIONAL_LIFECYCLE_IDENTITY_MARKET_V3,
        )?,
        project_identity(
            RationalLifecycleHotLayoutV3::GRAPH_ID,
            RATIONAL_LIFECYCLE_IDENTITY_GRAPH_V3,
        )?,
    ];
    if let Some(descriptor) = required_descriptor {
        for (chunk, bytes) in descriptor.chunks_exact(8).enumerate() {
            output.push(require_u64(
                RationalLifecycleHotLayoutV3::DESCRIPTOR_ID
                    .checked_add(chunk.checked_mul(8).ok_or(Error::InvalidLength)?)
                    .ok_or(Error::InvalidLength)?,
                u64::from_le_bytes(bytes.try_into().map_err(|_| Error::InvalidLength)?),
            )?);
        }
    } else {
        output.push(project_identity(
            RationalLifecycleHotLayoutV3::DESCRIPTOR_ID,
            RATIONAL_LIFECYCLE_IDENTITY_DESCRIPTOR_V3,
        )?);
    }
    output.extend([
        project_identity(
            RationalLifecycleHotLayoutV3::REPRESENTATION_AUTHORITY,
            RATIONAL_LIFECYCLE_IDENTITY_REPRESENTATION_AUTHORITY_V3,
        )?,
        project_identity(
            RationalLifecycleHotLayoutV3::RECEIPT_MINT,
            RATIONAL_LIFECYCLE_IDENTITY_RECEIPT_MINT_V3,
        )?,
        project_identity(
            RationalLifecycleHotLayoutV3::TOKEN_PROGRAM,
            RATIONAL_LIFECYCLE_IDENTITY_TOKEN_PROGRAM_V3,
        )?,
        project_identity(
            RationalLifecycleHotLayoutV3::RENT_CREDIT,
            RATIONAL_LIFECYCLE_IDENTITY_RENT_CREDIT_V3,
        )?,
        project_identity(
            RationalLifecycleHotLayoutV3::RENT_PROGRAM,
            RATIONAL_LIFECYCLE_IDENTITY_RENT_PROGRAM_V3,
        )?,
        project_u8(
            RationalLifecycleHotLayoutV3::ACTION,
            RATIONAL_LIFECYCLE_SCALAR_ACTION_V3,
        )?,
        project_u64(
            RationalLifecycleHotLayoutV3::GENERATION,
            RATIONAL_LIFECYCLE_SCALAR_GENERATION_V3,
        )?,
        project_u64(
            RationalLifecycleHotLayoutV3::EXPECTED_MARKET_REVISION,
            RATIONAL_LIFECYCLE_SCALAR_MARKET_REVISION_V3,
        )?,
        project_u64(
            RationalLifecycleHotLayoutV3::OBSERVED_RECEIPT_LAMPORTS,
            RATIONAL_LIFECYCLE_SCALAR_RECEIPT_LAMPORTS_V3,
        )?,
        project_u64(
            RationalLifecycleHotLayoutV3::RECEIPT_RENT_PRINCIPAL,
            RATIONAL_LIFECYCLE_SCALAR_RECEIPT_RENT_V3,
        )?,
        project_u64(
            RationalLifecycleHotLayoutV3::EXPECTED_RECEIPT_SUPPLY,
            RATIONAL_LIFECYCLE_SCALAR_RECEIPT_SUPPLY_V3,
        )?,
        project_u32(
            RationalLifecycleHotLayoutV3::OUTCOME_COUNT,
            RATIONAL_LIFECYCLE_SCALAR_OUTCOME_COUNT_V3,
        )?,
        require_u32(
            RationalLifecycleHotLayoutV3::COORDINATE_COUNT,
            coordinate_count,
        )?,
        project_u64(
            RationalLifecycleHotLayoutV3::RENT_CREDIT_BEFORE,
            RATIONAL_LIFECYCLE_SCALAR_RENT_BEFORE_V3,
        )?,
        project_u64(
            RationalLifecycleHotLayoutV3::RENT_CREDIT_AFTER,
            RATIONAL_LIFECYCLE_SCALAR_RENT_AFTER_V3,
        )?,
    ]);
    Ok(output)
}

fn append_row_request_instructions(
    output: &mut Vec<RequestInstructionV1>,
    layout: RationalLifecycleHotRegisterLayoutV3,
    row: usize,
) -> Result<()> {
    let base = RationalLifecycleHotLayoutV3::FIXED_BYTES
        .checked_add(
            row.checked_mul(RationalLifecycleHotLayoutV3::ITEM_BYTES)
                .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)?;
    let at = |field: usize| base.checked_add(field).ok_or(Error::InvalidLength);
    let identity = |field: usize| {
        layout
            .coordinate_identity(row, field)
            .ok_or(Error::InvalidLength)
    };
    let scalar = |field: usize| {
        layout
            .coordinate_scalar(row, field)
            .ok_or(Error::InvalidLength)
    };
    output.extend([
        require_zero(at(RationalLifecycleHotLayoutV3::ITEM_RESERVED_HEAD)?, 4)?,
        require_zero(at(RationalLifecycleHotLayoutV3::ITEM_RESERVED_TAIL)?, 8)?,
        project_u32(
            at(RationalLifecycleHotLayoutV3::ITEM_OUTCOME)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_OUTCOME_V3)?,
        )?,
        project_u64(
            at(RationalLifecycleHotLayoutV3::ITEM_COEFFICIENT)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_COEFFICIENT_V3)?,
        )?,
        project_identity(
            at(RationalLifecycleHotLayoutV3::ITEM_SHARD_MINT)?,
            identity(RATIONAL_LIFECYCLE_ITEM_IDENTITY_SHARD_MINT_V3)?,
        )?,
        project_identity(
            at(RationalLifecycleHotLayoutV3::ITEM_STRUCTURED_CUSTODY)?,
            identity(RATIONAL_LIFECYCLE_ITEM_IDENTITY_STRUCTURED_CUSTODY_V3)?,
        )?,
        project_identity(
            at(RationalLifecycleHotLayoutV3::ITEM_CUSTODY_OWNER)?,
            identity(RATIONAL_LIFECYCLE_ITEM_IDENTITY_CUSTODY_OWNER_V3)?,
        )?,
        project_identity(
            at(RationalLifecycleHotLayoutV3::ITEM_CUSTODY_POSITION)?,
            identity(RATIONAL_LIFECYCLE_ITEM_IDENTITY_CUSTODY_POSITION_V3)?,
        )?,
        project_identity(
            at(RationalLifecycleHotLayoutV3::ITEM_POSITION_ADMISSION)?,
            identity(RATIONAL_LIFECYCLE_ITEM_IDENTITY_POSITION_ADMISSION_V3)?,
        )?,
        project_u64(
            at(RationalLifecycleHotLayoutV3::ITEM_SHARD_LAMPORTS)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_SHARD_LAMPORTS_V3)?,
        )?,
        project_u64(
            at(RationalLifecycleHotLayoutV3::ITEM_STRUCTURED_LAMPORTS)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_STRUCTURED_LAMPORTS_V3)?,
        )?,
        project_u64(
            at(RationalLifecycleHotLayoutV3::ITEM_POSITION_LAMPORTS)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_POSITION_LAMPORTS_V3)?,
        )?,
        project_u64(
            at(RationalLifecycleHotLayoutV3::ITEM_ADMISSION_LAMPORTS)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_ADMISSION_LAMPORTS_V3)?,
        )?,
        project_u64(
            at(RationalLifecycleHotLayoutV3::ITEM_SHARD_RENT)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_SHARD_RENT_V3)?,
        )?,
        project_u64(
            at(RationalLifecycleHotLayoutV3::ITEM_STRUCTURED_RENT)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_STRUCTURED_RENT_V3)?,
        )?,
        project_u64(
            at(RationalLifecycleHotLayoutV3::ITEM_POSITION_RENT)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_POSITION_RENT_V3)?,
        )?,
        project_u64(
            at(RationalLifecycleHotLayoutV3::ITEM_ADMISSION_RENT)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_ADMISSION_RENT_V3)?,
        )?,
        project_u64(
            at(RationalLifecycleHotLayoutV3::ITEM_SHARD_SUPPLY)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_SHARD_SUPPLY_V3)?,
        )?,
        project_u64(
            at(RationalLifecycleHotLayoutV3::ITEM_STRUCTURED_AMOUNT)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_STRUCTURED_AMOUNT_V3)?,
        )?,
        project_u64(
            at(RationalLifecycleHotLayoutV3::ITEM_POSITION_REVISION)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_POSITION_REVISION_V3)?,
        )?,
    ]);
    Ok(())
}

#[cfg(test)]
#[allow(dead_code)]
fn row_request_instructions_v4() -> Result<[RowInstructionV4; ROW_REQUEST_OPERATIONS]> {
    let offset = |value: usize| narrow_u32(value);
    let scalar = |value: usize| narrow_u16(value);
    let identity = |value: usize| narrow_u16(value);
    Ok([
        RowInstructionV4::project_u32(
            offset(RationalLifecycleHotLayoutV3::ITEM_OUTCOME)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_OUTCOME_V3)?,
        ),
        RowInstructionV4::require_zero(
            offset(RationalLifecycleHotLayoutV3::ITEM_RESERVED_HEAD)?,
            4,
        ),
        RowInstructionV4::project_u64(
            offset(RationalLifecycleHotLayoutV3::ITEM_COEFFICIENT)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_COEFFICIENT_V3)?,
        ),
        RowInstructionV4::project_identity(
            offset(RationalLifecycleHotLayoutV3::ITEM_SHARD_MINT)?,
            identity(RATIONAL_LIFECYCLE_ITEM_IDENTITY_SHARD_MINT_V3)?,
        ),
        RowInstructionV4::project_identity(
            offset(RationalLifecycleHotLayoutV3::ITEM_STRUCTURED_CUSTODY)?,
            identity(RATIONAL_LIFECYCLE_ITEM_IDENTITY_STRUCTURED_CUSTODY_V3)?,
        ),
        RowInstructionV4::project_identity(
            offset(RationalLifecycleHotLayoutV3::ITEM_CUSTODY_OWNER)?,
            identity(RATIONAL_LIFECYCLE_ITEM_IDENTITY_CUSTODY_OWNER_V3)?,
        ),
        RowInstructionV4::project_identity(
            offset(RationalLifecycleHotLayoutV3::ITEM_CUSTODY_POSITION)?,
            identity(RATIONAL_LIFECYCLE_ITEM_IDENTITY_CUSTODY_POSITION_V3)?,
        ),
        RowInstructionV4::project_identity(
            offset(RationalLifecycleHotLayoutV3::ITEM_POSITION_ADMISSION)?,
            identity(RATIONAL_LIFECYCLE_ITEM_IDENTITY_POSITION_ADMISSION_V3)?,
        ),
        RowInstructionV4::project_u64(
            offset(RationalLifecycleHotLayoutV3::ITEM_SHARD_LAMPORTS)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_SHARD_LAMPORTS_V3)?,
        ),
        RowInstructionV4::project_u64(
            offset(RationalLifecycleHotLayoutV3::ITEM_STRUCTURED_LAMPORTS)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_STRUCTURED_LAMPORTS_V3)?,
        ),
        RowInstructionV4::project_u64(
            offset(RationalLifecycleHotLayoutV3::ITEM_POSITION_LAMPORTS)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_POSITION_LAMPORTS_V3)?,
        ),
        RowInstructionV4::project_u64(
            offset(RationalLifecycleHotLayoutV3::ITEM_ADMISSION_LAMPORTS)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_ADMISSION_LAMPORTS_V3)?,
        ),
        RowInstructionV4::project_u64(
            offset(RationalLifecycleHotLayoutV3::ITEM_SHARD_RENT)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_SHARD_RENT_V3)?,
        ),
        RowInstructionV4::project_u64(
            offset(RationalLifecycleHotLayoutV3::ITEM_STRUCTURED_RENT)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_STRUCTURED_RENT_V3)?,
        ),
        RowInstructionV4::project_u64(
            offset(RationalLifecycleHotLayoutV3::ITEM_POSITION_RENT)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_POSITION_RENT_V3)?,
        ),
        RowInstructionV4::project_u64(
            offset(RationalLifecycleHotLayoutV3::ITEM_ADMISSION_RENT)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_ADMISSION_RENT_V3)?,
        ),
        RowInstructionV4::project_u64(
            offset(RationalLifecycleHotLayoutV3::ITEM_SHARD_SUPPLY)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_SHARD_SUPPLY_V3)?,
        ),
        RowInstructionV4::project_u64(
            offset(RationalLifecycleHotLayoutV3::ITEM_STRUCTURED_AMOUNT)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_STRUCTURED_AMOUNT_V3)?,
        ),
        RowInstructionV4::project_u64(
            offset(RationalLifecycleHotLayoutV3::ITEM_POSITION_REVISION)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_POSITION_REVISION_V3)?,
        ),
        RowInstructionV4::require_zero(
            offset(RationalLifecycleHotLayoutV3::ITEM_RESERVED_TAIL)?,
            8,
        ),
    ])
}

fn request(offset: usize) -> Result<RequestCoordinateV1> {
    Ok(RequestCoordinateV1::fixed(narrow_u32(offset)?))
}

fn identity(index: usize) -> Result<IdentityRegisterV1> {
    Ok(IdentityRegisterV1::common(narrow_u16(index)?))
}

fn scalar(index: usize) -> Result<ScalarRegisterV1> {
    Ok(ScalarRegisterV1::common(narrow_u16(index)?))
}

fn transition_scalar(index: usize) -> Result<ScalarRegisterV3> {
    Ok(ScalarRegisterV3::common(narrow_u16(index)?))
}

fn require_u8(offset: usize, value: u8) -> Result<RequestInstructionV1> {
    Ok(RequestInstructionV1::require_u8(request(offset)?, value))
}

fn require_u16(offset: usize, value: u16) -> Result<RequestInstructionV1> {
    Ok(RequestInstructionV1::require_u16(request(offset)?, value))
}

fn require_u32(offset: usize, value: u32) -> Result<RequestInstructionV1> {
    Ok(RequestInstructionV1::require_u32(request(offset)?, value))
}

fn require_u64(offset: usize, value: u64) -> Result<RequestInstructionV1> {
    Ok(RequestInstructionV1::require_u64(request(offset)?, value))
}

fn require_zero(offset: usize, bytes: u32) -> Result<RequestInstructionV1> {
    Ok(RequestInstructionV1::require_zero(request(offset)?, bytes))
}

fn project_identity(offset: usize, register: usize) -> Result<RequestInstructionV1> {
    Ok(RequestInstructionV1::project_identity(
        request(offset)?,
        identity(register)?,
    ))
}

fn project_u8(offset: usize, register: usize) -> Result<RequestInstructionV1> {
    Ok(RequestInstructionV1::project_u8(
        request(offset)?,
        scalar(register)?,
    ))
}

fn project_u32(offset: usize, register: usize) -> Result<RequestInstructionV1> {
    Ok(RequestInstructionV1::project_u32(
        request(offset)?,
        scalar(register)?,
    ))
}

fn project_u64(offset: usize, register: usize) -> Result<RequestInstructionV1> {
    Ok(RequestInstructionV1::project_u64(
        request(offset)?,
        scalar(register)?,
    ))
}

fn narrow_u16(value: usize) -> Result<u16> {
    u16::try_from(value).map_err(|_| Error::InvalidLength)
}

fn narrow_u32(value: usize) -> Result<u32> {
    u32::try_from(value).map_err(|_| Error::InvalidLength)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_request_profile_contract::RequestProfileV1;
    use dclutch_transition_vm::v3::{
        ProgramV3, RegisterInput, RegisterOutput, execute_fold_atomic,
    };

    fn run_selected_transition(
        product_width: u64,
        representation_width: u64,
        outcome: u64,
    ) -> dclutch_transition_vm::v3::Result<()> {
        let bytes =
            encode_rational_lifecycle_transition_v3(LifecycleActionV2::ActivateCoordinate, 1)
                .expect("transition");
        let program = ProgramV3::decode(&bytes).expect("decode transition");
        let layout = RationalLifecycleHotRegisterLayoutV3::new(1);
        let mut input = vec![0_u64; layout.scalar_count().expect("scalar width")];
        *input
            .get_mut(RATIONAL_LIFECYCLE_SCALAR_OUTCOME_COUNT_V3)
            .expect("representation width register") = representation_width;
        *input
            .get_mut(dclutch_rational_representation_v2_lifecycle_contract::hot_v3::RATIONAL_LIFECYCLE_SCALAR_PRODUCT_OUTCOME_COUNT_V3)
            .expect("Product width register") = product_width;
        *input
            .get_mut(
                layout
                    .coordinate_scalar(0, RATIONAL_LIFECYCLE_ITEM_SCALAR_OUTCOME_V3)
                    .expect("outcome register"),
            )
            .expect("outcome scalar") = outcome;
        *input
            .get_mut(
                layout
                    .coordinate_scalar(0, RATIONAL_LIFECYCLE_ITEM_SCALAR_COEFFICIENT_V3)
                    .expect("coefficient register"),
            )
            .expect("coefficient scalar") = 1;
        let identities = vec![[0_u8; 32]; layout.identity_count().expect("identity width")];
        let mut scratch_scalars = vec![0_u64; input.len()];
        let mut output_scalars = vec![0_u64; input.len()];
        let mut scratch_identities = vec![[0_u8; 32]; identities.len()];
        let mut output_identities = vec![[0_u8; 32]; identities.len()];
        execute_fold_atomic(
            program,
            0,
            RegisterInput {
                scalars: &input,
                identities: &identities,
            },
            RegisterOutput {
                scalars: &mut scratch_scalars,
                identities: &mut scratch_identities,
            },
            RegisterOutput {
                scalars: &mut output_scalars,
                identities: &mut output_identities,
            },
        )
    }

    #[test]
    fn selected_action_artifact_geometries_decode() {
        for (action, coordinates) in [
            (LifecycleActionV2::ActivateReceipt, 0),
            (LifecycleActionV2::ActivateCoordinate, 1),
            (LifecycleActionV2::RetireCoordinate, 1),
        ] {
            let request = encode_rational_lifecycle_request_profile_v3(action, coordinates)
                .expect("request profile");
            let transition =
                encode_rational_lifecycle_transition_v3(action, coordinates).expect("transition");
            RequestProfileV1::decode(&request).expect("decode V1 request profile");
            ProgramV3::decode(&transition).expect("decode transition");
        }
        assert_eq!(
            encode_rational_lifecycle_request_profile_v3(LifecycleActionV2::ActivateReceipt, 1,),
            Err(Error::ActionGeometry)
        );
        assert_eq!(
            encode_rational_lifecycle_request_profile_v3(LifecycleActionV2::RetireReceipt, 1),
            Err(Error::ActionGeometry)
        );
        assert_eq!(
            encode_rational_lifecycle_request_profile_v3(LifecycleActionV2::RetireReceipt, 3),
            Err(Error::ActionGeometry)
        );
    }

    #[test]
    fn selected_transition_keeps_representation_k_distinct_from_product_n() {
        run_selected_transition(9, 3, 2).expect("K=3/N=9");
        run_selected_transition(258, 3, 2).expect("K=3/N=258");
    }

    #[test]
    fn selected_transition_bounds_claim_coordinates_by_k() {
        assert!(run_selected_transition(258, 3, 3).is_err());
        assert!(run_selected_transition(258, 2, 2).is_err());
        run_selected_transition(2, 3, 2).expect("terminal N cannot substitute for Claims K");
    }
}
