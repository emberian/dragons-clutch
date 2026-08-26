//! One-route Claims EffectProgram for a descriptor/action-specific lifecycle.

use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{
        HEADER_BYTES as EFFECT_HEADER_BYTES, OPERATION_BYTES as EFFECT_OPERATION_BYTES,
        ROUTE_BYTES as EFFECT_ROUTE_BYTES, RouteKindV3,
        encode::{
            EffectGeometryV3, EffectInstructionV3, IdentityCoordinateV3, RequestSpaceV3,
            RouteInputV3, ScalarCoordinateV3, encode_effect_program_v3_atomic,
        },
    },
};
use dclutch_rational_representation_v2_lifecycle_contract::{
    LIFECYCLE_COMMON_ACCOUNT_COUNT_V2, LIFECYCLE_COORDINATE_ACCOUNT_COUNT_V2,
    LIFECYCLE_REQUEST_MAGIC_V2, LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2, LIFECYCLE_VERSION_V2,
    LifecycleActionV2,
    hot_v3::{
        RATIONAL_LIFECYCLE_IDENTITY_DESCRIPTOR_V3, RATIONAL_LIFECYCLE_IDENTITY_GRAPH_V3,
        RATIONAL_LIFECYCLE_IDENTITY_MARKET_V3, RATIONAL_LIFECYCLE_IDENTITY_PARENT_DIGEST_V3,
        RATIONAL_LIFECYCLE_IDENTITY_RECEIPT_MINT_V3, RATIONAL_LIFECYCLE_IDENTITY_RELEASE_SET_V3,
        RATIONAL_LIFECYCLE_IDENTITY_RENT_CREDIT_V3, RATIONAL_LIFECYCLE_IDENTITY_RENT_PROGRAM_V3,
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
        RATIONAL_LIFECYCLE_ITEM_SCALAR_STRUCTURED_RENT_V3, RATIONAL_LIFECYCLE_SCALAR_GENERATION_V3,
        RATIONAL_LIFECYCLE_SCALAR_MARKET_REVISION_V3, RATIONAL_LIFECYCLE_SCALAR_OUTCOME_COUNT_V3,
        RATIONAL_LIFECYCLE_SCALAR_RECEIPT_LAMPORTS_V3, RATIONAL_LIFECYCLE_SCALAR_RECEIPT_RENT_V3,
        RATIONAL_LIFECYCLE_SCALAR_RECEIPT_SUPPLY_V3, RATIONAL_LIFECYCLE_SCALAR_RENT_AFTER_V3,
        RATIONAL_LIFECYCLE_SCALAR_RENT_BEFORE_V3, RationalLifecycleHotLayoutV3,
        RationalLifecycleHotRegisterLayoutV3,
    },
};

use crate::{Error, Result, validate_action_geometry};

/// Logical accounts injected by common Hot before the Claims child frame.
pub const RATIONAL_LIFECYCLE_HOT_INJECTED_ACCOUNT_COUNT_V3: u16 = 5;

/// Exact Claims child account count for one specialized action.
pub fn lifecycle_claims_account_count_v3(
    action: LifecycleActionV2,
    coordinate_count: u32,
) -> Result<u16> {
    let coordinates = validate_action_geometry(action, coordinate_count)?;
    let count = match action {
        LifecycleActionV2::ActivateReceipt => LIFECYCLE_COMMON_ACCOUNT_COUNT_V2,
        LifecycleActionV2::ActivateCoordinate | LifecycleActionV2::RetireCoordinate => {
            LIFECYCLE_COORDINATE_ACCOUNT_COUNT_V2
        }
        LifecycleActionV2::RetireReceipt => coordinates
            .checked_mul(LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2)
            .and_then(|tail| tail.checked_add(LIFECYCLE_COMMON_ACCOUNT_COUNT_V2))
            .ok_or(Error::InvalidLength)?,
    };
    narrow_u16(count)
}

/// Exact logical AccountProfile/EffectProgram account width.
pub fn lifecycle_logical_account_count_v3(
    action: LifecycleActionV2,
    coordinate_count: u32,
) -> Result<u16> {
    RATIONAL_LIFECYCLE_HOT_INJECTED_ACCOUNT_COUNT_V3
        .checked_add(lifecycle_claims_account_count_v3(action, coordinate_count)?)
        .ok_or(Error::InvalidLength)
}

/// Encode one exact Claims `Once` EffectProgram.
pub fn encode_rational_lifecycle_effect_v3(
    action: LifecycleActionV2,
    coordinate_count: u32,
) -> Result<Vec<u8>> {
    let coordinates = validate_action_geometry(action, coordinate_count)?;
    let registers = RationalLifecycleHotRegisterLayoutV3::new(coordinates);
    let template = child_template(action, coordinate_count, coordinates)?;
    let routes = [RouteInputV3 {
        role: FixedRole::Claims,
        kind: RouteKindV3::Once,
        enable_common_scalar: None,
        witness_range_common_scalar: None,
        receipt_dependency: None,
        fixed_account_start: RATIONAL_LIFECYCLE_HOT_INJECTED_ACCOUNT_COUNT_V3,
        fixed_account_count: lifecycle_claims_account_count_v3(action, coordinate_count)?,
        item_account_start: 0,
        item_account_count: 0,
        fixed_request: &template,
        item_request: &[],
    }];
    let mut instructions = Vec::with_capacity(
        coordinates
            .checked_mul(18)
            .and_then(|count| count.checked_add(18))
            .ok_or(Error::InvalidLength)?,
    );
    append_header_instructions(&mut instructions)?;
    for row in 0..coordinates {
        append_row_instructions(&mut instructions, registers, row)?;
    }
    let geometry = EffectGeometryV3 {
        fixed_accounts: lifecycle_logical_account_count_v3(action, coordinate_count)?,
        item_account_stride: 0,
        common_scalars: narrow_u16(registers.scalar_count().ok_or(Error::InvalidLength)?)?,
        item_scalar_stride: 0,
        common_identities: narrow_u16(registers.identity_count().ok_or(Error::InvalidLength)?)?,
        item_identity_stride: 0,
    };
    let bytes = EFFECT_HEADER_BYTES
        .checked_add(EFFECT_ROUTE_BYTES)
        .and_then(|value| {
            instructions
                .len()
                .checked_mul(EFFECT_OPERATION_BYTES)
                .and_then(|ops| value.checked_add(ops))
        })
        .and_then(|value| value.checked_add(template.len()))
        .ok_or(Error::InvalidLength)?;
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_effect_program_v3_atomic(
        geometry,
        &routes,
        &instructions,
        &[],
        &mut scratch,
        &mut output,
    )
    .map_err(Error::Effect)?;
    Ok(output)
}

fn child_template(
    action: LifecycleActionV2,
    coordinate_count: u32,
    coordinates: usize,
) -> Result<Vec<u8>> {
    let bytes =
        RationalLifecycleHotLayoutV3::request_bytes(coordinates).ok_or(Error::InvalidLength)?;
    let mut output = vec![0_u8; bytes];
    put(
        &mut output,
        RationalLifecycleHotLayoutV3::MAGIC,
        &LIFECYCLE_REQUEST_MAGIC_V2,
    )?;
    put(
        &mut output,
        RationalLifecycleHotLayoutV3::VERSION,
        &LIFECYCLE_VERSION_V2.to_le_bytes(),
    )?;
    put(
        &mut output,
        RationalLifecycleHotLayoutV3::ACTION,
        &[action.tag()],
    )?;
    put(
        &mut output,
        RationalLifecycleHotLayoutV3::COORDINATE_COUNT,
        &coordinate_count.to_le_bytes(),
    )?;
    Ok(output)
}

fn append_header_instructions(output: &mut Vec<EffectInstructionV3>) -> Result<()> {
    output.extend([
        write_identity(
            RationalLifecycleHotLayoutV3::RELEASE_SET,
            RATIONAL_LIFECYCLE_IDENTITY_RELEASE_SET_V3,
        )?,
        write_identity(
            RationalLifecycleHotLayoutV3::MARKET,
            RATIONAL_LIFECYCLE_IDENTITY_MARKET_V3,
        )?,
        write_identity(
            RationalLifecycleHotLayoutV3::GRAPH_ID,
            RATIONAL_LIFECYCLE_IDENTITY_GRAPH_V3,
        )?,
        write_identity(
            RationalLifecycleHotLayoutV3::DESCRIPTOR_ID,
            RATIONAL_LIFECYCLE_IDENTITY_DESCRIPTOR_V3,
        )?,
        write_identity(
            RationalLifecycleHotLayoutV3::PARENT_CONTEXT,
            RATIONAL_LIFECYCLE_IDENTITY_PARENT_DIGEST_V3,
        )?,
        write_identity(
            RationalLifecycleHotLayoutV3::REPRESENTATION_AUTHORITY,
            RATIONAL_LIFECYCLE_IDENTITY_REPRESENTATION_AUTHORITY_V3,
        )?,
        write_identity(
            RationalLifecycleHotLayoutV3::RECEIPT_MINT,
            RATIONAL_LIFECYCLE_IDENTITY_RECEIPT_MINT_V3,
        )?,
        write_identity(
            RationalLifecycleHotLayoutV3::TOKEN_PROGRAM,
            RATIONAL_LIFECYCLE_IDENTITY_TOKEN_PROGRAM_V3,
        )?,
        write_identity(
            RationalLifecycleHotLayoutV3::RENT_CREDIT,
            RATIONAL_LIFECYCLE_IDENTITY_RENT_CREDIT_V3,
        )?,
        write_identity(
            RationalLifecycleHotLayoutV3::RENT_PROGRAM,
            RATIONAL_LIFECYCLE_IDENTITY_RENT_PROGRAM_V3,
        )?,
        write_u64(
            RationalLifecycleHotLayoutV3::GENERATION,
            RATIONAL_LIFECYCLE_SCALAR_GENERATION_V3,
        )?,
        write_u64(
            RationalLifecycleHotLayoutV3::EXPECTED_MARKET_REVISION,
            RATIONAL_LIFECYCLE_SCALAR_MARKET_REVISION_V3,
        )?,
        write_u64(
            RationalLifecycleHotLayoutV3::OBSERVED_RECEIPT_LAMPORTS,
            RATIONAL_LIFECYCLE_SCALAR_RECEIPT_LAMPORTS_V3,
        )?,
        write_u64(
            RationalLifecycleHotLayoutV3::RECEIPT_RENT_PRINCIPAL,
            RATIONAL_LIFECYCLE_SCALAR_RECEIPT_RENT_V3,
        )?,
        write_u64(
            RationalLifecycleHotLayoutV3::EXPECTED_RECEIPT_SUPPLY,
            RATIONAL_LIFECYCLE_SCALAR_RECEIPT_SUPPLY_V3,
        )?,
        write_u32(
            RationalLifecycleHotLayoutV3::OUTCOME_COUNT,
            RATIONAL_LIFECYCLE_SCALAR_OUTCOME_COUNT_V3,
        )?,
        write_u64(
            RationalLifecycleHotLayoutV3::RENT_CREDIT_BEFORE,
            RATIONAL_LIFECYCLE_SCALAR_RENT_BEFORE_V3,
        )?,
        write_u64(
            RationalLifecycleHotLayoutV3::RENT_CREDIT_AFTER,
            RATIONAL_LIFECYCLE_SCALAR_RENT_AFTER_V3,
        )?,
    ]);
    Ok(())
}

fn append_row_instructions(
    output: &mut Vec<EffectInstructionV3>,
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
        write_u32(
            at(RationalLifecycleHotLayoutV3::ITEM_OUTCOME)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_OUTCOME_V3)?,
        )?,
        write_u64(
            at(RationalLifecycleHotLayoutV3::ITEM_COEFFICIENT)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_COEFFICIENT_V3)?,
        )?,
        write_identity(
            at(RationalLifecycleHotLayoutV3::ITEM_SHARD_MINT)?,
            identity(RATIONAL_LIFECYCLE_ITEM_IDENTITY_SHARD_MINT_V3)?,
        )?,
        write_identity(
            at(RationalLifecycleHotLayoutV3::ITEM_STRUCTURED_CUSTODY)?,
            identity(RATIONAL_LIFECYCLE_ITEM_IDENTITY_STRUCTURED_CUSTODY_V3)?,
        )?,
        write_identity(
            at(RationalLifecycleHotLayoutV3::ITEM_CUSTODY_OWNER)?,
            identity(RATIONAL_LIFECYCLE_ITEM_IDENTITY_CUSTODY_OWNER_V3)?,
        )?,
        write_identity(
            at(RationalLifecycleHotLayoutV3::ITEM_CUSTODY_POSITION)?,
            identity(RATIONAL_LIFECYCLE_ITEM_IDENTITY_CUSTODY_POSITION_V3)?,
        )?,
        write_identity(
            at(RationalLifecycleHotLayoutV3::ITEM_POSITION_ADMISSION)?,
            identity(RATIONAL_LIFECYCLE_ITEM_IDENTITY_POSITION_ADMISSION_V3)?,
        )?,
        write_u64(
            at(RationalLifecycleHotLayoutV3::ITEM_SHARD_LAMPORTS)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_SHARD_LAMPORTS_V3)?,
        )?,
        write_u64(
            at(RationalLifecycleHotLayoutV3::ITEM_STRUCTURED_LAMPORTS)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_STRUCTURED_LAMPORTS_V3)?,
        )?,
        write_u64(
            at(RationalLifecycleHotLayoutV3::ITEM_POSITION_LAMPORTS)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_POSITION_LAMPORTS_V3)?,
        )?,
        write_u64(
            at(RationalLifecycleHotLayoutV3::ITEM_ADMISSION_LAMPORTS)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_ADMISSION_LAMPORTS_V3)?,
        )?,
        write_u64(
            at(RationalLifecycleHotLayoutV3::ITEM_SHARD_RENT)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_SHARD_RENT_V3)?,
        )?,
        write_u64(
            at(RationalLifecycleHotLayoutV3::ITEM_STRUCTURED_RENT)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_STRUCTURED_RENT_V3)?,
        )?,
        write_u64(
            at(RationalLifecycleHotLayoutV3::ITEM_POSITION_RENT)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_POSITION_RENT_V3)?,
        )?,
        write_u64(
            at(RationalLifecycleHotLayoutV3::ITEM_ADMISSION_RENT)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_ADMISSION_RENT_V3)?,
        )?,
        write_u64(
            at(RationalLifecycleHotLayoutV3::ITEM_SHARD_SUPPLY)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_SHARD_SUPPLY_V3)?,
        )?,
        write_u64(
            at(RationalLifecycleHotLayoutV3::ITEM_STRUCTURED_AMOUNT)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_STRUCTURED_AMOUNT_V3)?,
        )?,
        write_u64(
            at(RationalLifecycleHotLayoutV3::ITEM_POSITION_REVISION)?,
            scalar(RATIONAL_LIFECYCLE_ITEM_SCALAR_POSITION_REVISION_V3)?,
        )?,
    ]);
    Ok(())
}

fn write_identity(offset: usize, register: usize) -> Result<EffectInstructionV3> {
    Ok(EffectInstructionV3::write_request_identity(
        0,
        RequestSpaceV3::Fixed,
        narrow_u32(offset)?,
        IdentityCoordinateV3::common(narrow_u16(register)?),
    ))
}

fn write_u64(offset: usize, register: usize) -> Result<EffectInstructionV3> {
    Ok(EffectInstructionV3::write_request_u64(
        0,
        RequestSpaceV3::Fixed,
        narrow_u32(offset)?,
        ScalarCoordinateV3::common(narrow_u16(register)?),
    ))
}

fn write_u32(offset: usize, register: usize) -> Result<EffectInstructionV3> {
    Ok(EffectInstructionV3::write_request_u32(
        0,
        RequestSpaceV3::Fixed,
        narrow_u32(offset)?,
        ScalarCoordinateV3::common(narrow_u16(register)?),
    ))
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(Error::InvalidLength)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

fn narrow_u16(value: usize) -> Result<u16> {
    u16::try_from(value).map_err(|_| Error::InvalidLength)
}

fn narrow_u32(value: usize) -> Result<u32> {
    u32::try_from(value).map_err(|_| Error::InvalidLength)
}
