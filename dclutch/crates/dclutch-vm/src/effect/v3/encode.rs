//! Safe, allocation-free EffectProgram V3 artifact encoder.
//!
//! Typed constructors retain opcode and mode authority in the effect kernel.
//! The encoder builds into caller scratch, hostile-decodes the complete
//! candidate, and copies to output only after every route, operation, template,
//! and overlap check succeeds.

use super::{
    Error, FixedRole, HEADER_BYTES, MAGIC, MODE_ACCOUNT_A_ITEM, MODE_ACCOUNT_B_ITEM,
    MODE_REGISTER_ITEM, MODE_REQUEST_ITEM, OP_REQUIRE_LAMPORTS_EQ, OP_TRANSFER_LAMPORTS,
    OP_WRITE_DATA_U8, OP_WRITE_DATA_U8_AFFINE, OP_WRITE_DATA_U8_IF, OP_WRITE_DATA_U16,
    OP_WRITE_DATA_U16_AFFINE, OP_WRITE_DATA_U16_IF, OP_WRITE_DATA_U32, OP_WRITE_DATA_U32_AFFINE,
    OP_WRITE_DATA_U32_IF, OP_WRITE_IDENTITY, OP_WRITE_IDENTITY_AFFINE, OP_WRITE_IDENTITY_IF,
    OP_WRITE_REQUEST_IDENTITY, OP_WRITE_REQUEST_U8, OP_WRITE_REQUEST_U16, OP_WRITE_REQUEST_U32,
    OP_WRITE_REQUEST_U64, OP_WRITE_SCALAR, OP_WRITE_SCALAR_AFFINE, OP_WRITE_SCALAR_AFFINE_IF,
    OP_WRITE_SCALAR_FIFTH_TAIL_AFFINE, OP_WRITE_SCALAR_FOURTH_TAIL_AFFINE, OP_WRITE_SCALAR_IF,
    OP_WRITE_SCALAR_SECOND_TAIL_AFFINE, OP_WRITE_SCALAR_SECOND_TAIL_AFFINE_IF,
    OP_WRITE_SCALAR_THIRD_TAIL_AFFINE, OPERATION_BYTES, ProgramV3, RECEIPT_DEPENDENCY_BYTES,
    ROUTE_BYTES, RouteKindV3, RouteReceiptDependencyV3, VERSION,
};

/// Fixed-prefix or per-Product-item coordinate space.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoordinateSpaceV3 {
    /// Fixed prefix or common register bank.
    Fixed,
    /// Per-Product-item account or register bank.
    Item,
}

/// One account coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountCoordinateV3 {
    space: CoordinateSpaceV3,
    index: u16,
}

impl AccountCoordinateV3 {
    /// Address one fixed-prefix account.
    pub const fn fixed(index: u16) -> Self {
        Self {
            space: CoordinateSpaceV3::Fixed,
            index,
        }
    }

    /// Address one account within each Product-item subframe.
    pub const fn item(index: u16) -> Self {
        Self {
            space: CoordinateSpaceV3::Item,
            index,
        }
    }
}

/// One scalar-register coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarCoordinateV3 {
    space: CoordinateSpaceV3,
    index: u16,
}

impl ScalarCoordinateV3 {
    /// Address one common scalar.
    pub const fn common(index: u16) -> Self {
        Self {
            space: CoordinateSpaceV3::Fixed,
            index,
        }
    }

    /// Address one scalar within each Product-item bank.
    pub const fn item(index: u16) -> Self {
        Self {
            space: CoordinateSpaceV3::Item,
            index,
        }
    }
}

/// One identity-register coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdentityCoordinateV3 {
    space: CoordinateSpaceV3,
    index: u16,
}

impl IdentityCoordinateV3 {
    /// Address one common identity.
    pub const fn common(index: u16) -> Self {
        Self {
            space: CoordinateSpaceV3::Fixed,
            index,
        }
    }

    /// Address one identity within each Product-item bank.
    pub const fn item(index: u16) -> Self {
        Self {
            space: CoordinateSpaceV3::Item,
            index,
        }
    }
}

/// Fixed or repeated-item child request template space.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestSpaceV3 {
    /// Fixed request template.
    Fixed,
    /// Per-Product-item request template.
    Item,
}

/// One complete fixed-role route and its authenticated request templates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteInputV3<'a> {
    /// Selected fixed state-owning role.
    pub role: FixedRole,
    /// Once, affine-once, or each-item invocation geometry.
    pub kind: RouteKindV3,
    /// Optional common scalar enabling this route when nonzero.
    pub enable_common_scalar: Option<u16>,
    /// Optional first of two common scalars selecting a borrowed witness range.
    pub witness_range_common_scalar: Option<u16>,
    /// Optional exact backward dependency on one prior route receipt.
    pub receipt_dependency: Option<RouteReceiptDependencyV3>,
    /// First fixed-prefix account in the child frame.
    pub fixed_account_start: u16,
    /// Fixed-prefix child-account count.
    pub fixed_account_count: u16,
    /// First account within each Product-item subframe.
    pub item_account_start: u16,
    /// Accounts from each Product-item subframe.
    pub item_account_count: u16,
    /// Exact fixed request template.
    pub fixed_request: &'a [u8],
    /// Exact repeated-item request template.
    pub item_request: &'a [u8],
}

/// Exact EffectProgram account/register geometry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectGeometryV3 {
    /// Fixed-prefix account count.
    pub fixed_accounts: u16,
    /// Per-Product-item account stride.
    pub item_account_stride: u16,
    /// Common scalar count.
    pub common_scalars: u16,
    /// Per-Product-item scalar stride.
    pub item_scalar_stride: u16,
    /// Common identity count.
    pub common_identities: u16,
    /// Per-Product-item identity stride.
    pub item_identity_stride: u16,
}

/// One typed EffectProgram operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectInstructionV3 {
    opcode: u8,
    account_a: AccountCoordinateV3,
    account_b: Option<AccountCoordinateV3>,
    enable_common_scalar: Option<u16>,
    scalar: Option<ScalarCoordinateV3>,
    identity: Option<IdentityCoordinateV3>,
    data_offset: u32,
    extra: u32,
    route: u16,
    request_space: Option<RequestSpaceV3>,
}

impl EffectInstructionV3 {
    /// Transfer exact lamports between two authenticated accounts.
    pub const fn transfer_lamports(
        source: AccountCoordinateV3,
        destination: AccountCoordinateV3,
        amount: ScalarCoordinateV3,
    ) -> Self {
        Self::scalar(
            OP_TRANSFER_LAMPORTS,
            source,
            Some(destination),
            amount,
            0,
            0,
        )
    }

    /// Require one authenticated account's lamports to equal a scalar.
    pub const fn require_lamports_eq(
        account: AccountCoordinateV3,
        value: ScalarCoordinateV3,
    ) -> Self {
        Self::scalar(OP_REQUIRE_LAMPORTS_EQ, account, None, value, 0, 0)
    }

    /// Write one common/item scalar as little-endian `u64` account data.
    pub const fn write_u64(
        account: AccountCoordinateV3,
        offset: u32,
        value: ScalarCoordinateV3,
    ) -> Self {
        Self::scalar(OP_WRITE_SCALAR, account, None, value, offset, 0)
    }

    /// Write one scalar to a fixed account at `base + item * stride`.
    pub const fn write_u64_affine(
        account: AccountCoordinateV3,
        base: u32,
        stride: u32,
        value: ScalarCoordinateV3,
    ) -> Self {
        Self::scalar(OP_WRITE_SCALAR_AFFINE, account, None, value, base, stride)
    }

    /// Conditionally write a common scalar as little-endian `u64` account
    /// data when the selected common enable scalar is nonzero.
    pub const fn write_u64_if_nonzero(
        account: AccountCoordinateV3,
        offset: u32,
        value: ScalarCoordinateV3,
        enable_common_scalar: u16,
    ) -> Self {
        Self::conditional_scalar(
            OP_WRITE_SCALAR_IF,
            account,
            value,
            offset,
            0,
            enable_common_scalar,
        )
    }

    /// Conditionally write an item scalar at `base + item * stride` in a
    /// fixed account when the selected common enable scalar is nonzero.
    pub const fn write_u64_affine_if_nonzero(
        account: AccountCoordinateV3,
        base: u32,
        stride: u32,
        value: ScalarCoordinateV3,
        enable_common_scalar: u16,
    ) -> Self {
        Self::conditional_scalar(
            OP_WRITE_SCALAR_AFFINE_IF,
            account,
            value,
            base,
            stride,
            enable_common_scalar,
        )
    }

    /// Conditionally write an item scalar into the second of two adjacent
    /// runtime tails at `base + (tail_count + item) * stride` in a fixed
    /// account when the selected common enable scalar is nonzero.
    pub const fn write_u64_second_tail_affine_if_nonzero(
        account: AccountCoordinateV3,
        base: u32,
        stride: u32,
        value: ScalarCoordinateV3,
        enable_common_scalar: u16,
    ) -> Self {
        Self::conditional_scalar(
            OP_WRITE_SCALAR_SECOND_TAIL_AFFINE_IF,
            account,
            value,
            base,
            stride,
            enable_common_scalar,
        )
    }

    /// Write an item scalar into the second of planar runtime tails at
    /// `base + (tail_count + item) * stride` in a fixed account.
    pub const fn write_u64_second_tail_affine(
        account: AccountCoordinateV3,
        base: u32,
        stride: u32,
        value: ScalarCoordinateV3,
    ) -> Self {
        Self::scalar(
            OP_WRITE_SCALAR_SECOND_TAIL_AFFINE,
            account,
            None,
            value,
            base,
            stride,
        )
    }

    /// Write an item scalar into the third of five planar runtime tails at
    /// `base + (2 * tail_count + item) * stride` in a fixed account.
    pub const fn write_u64_third_tail_affine(
        account: AccountCoordinateV3,
        base: u32,
        stride: u32,
        value: ScalarCoordinateV3,
    ) -> Self {
        Self::scalar(
            OP_WRITE_SCALAR_THIRD_TAIL_AFFINE,
            account,
            None,
            value,
            base,
            stride,
        )
    }

    /// Write an item scalar into the fourth of five planar runtime tails at
    /// `base + (3 * tail_count + item) * stride` in a fixed account.
    pub const fn write_u64_fourth_tail_affine(
        account: AccountCoordinateV3,
        base: u32,
        stride: u32,
        value: ScalarCoordinateV3,
    ) -> Self {
        Self::scalar(
            OP_WRITE_SCALAR_FOURTH_TAIL_AFFINE,
            account,
            None,
            value,
            base,
            stride,
        )
    }

    /// Write an item scalar into the fifth of five planar runtime tails at
    /// `base + (4 * tail_count + item) * stride` in a fixed account.
    pub const fn write_u64_fifth_tail_affine(
        account: AccountCoordinateV3,
        base: u32,
        stride: u32,
        value: ScalarCoordinateV3,
    ) -> Self {
        Self::scalar(
            OP_WRITE_SCALAR_FIFTH_TAIL_AFFINE,
            account,
            None,
            value,
            base,
            stride,
        )
    }

    /// Write one scalar narrowed to `u8` account data.
    pub const fn write_u8(
        account: AccountCoordinateV3,
        offset: u32,
        value: ScalarCoordinateV3,
    ) -> Self {
        Self::scalar(OP_WRITE_DATA_U8, account, None, value, offset, 0)
    }

    /// Conditionally write one scalar narrowed to `u8` account data when the
    /// selected common enable scalar is nonzero.
    pub const fn write_u8_if_nonzero(
        account: AccountCoordinateV3,
        offset: u32,
        value: ScalarCoordinateV3,
        enable_common_scalar: u16,
    ) -> Self {
        Self::conditional_scalar(
            OP_WRITE_DATA_U8_IF,
            account,
            value,
            offset,
            0,
            enable_common_scalar,
        )
    }

    /// Write one scalar narrowed to little-endian `u16` account data.
    pub const fn write_u16(
        account: AccountCoordinateV3,
        offset: u32,
        value: ScalarCoordinateV3,
    ) -> Self {
        Self::scalar(OP_WRITE_DATA_U16, account, None, value, offset, 0)
    }

    /// Conditionally write one scalar narrowed to little-endian `u16`
    /// account data when the selected common enable scalar is nonzero.
    pub const fn write_u16_if_nonzero(
        account: AccountCoordinateV3,
        offset: u32,
        value: ScalarCoordinateV3,
        enable_common_scalar: u16,
    ) -> Self {
        Self::conditional_scalar(
            OP_WRITE_DATA_U16_IF,
            account,
            value,
            offset,
            0,
            enable_common_scalar,
        )
    }

    /// Write one scalar narrowed to little-endian `u32` account data.
    pub const fn write_u32(
        account: AccountCoordinateV3,
        offset: u32,
        value: ScalarCoordinateV3,
    ) -> Self {
        Self::scalar(OP_WRITE_DATA_U32, account, None, value, offset, 0)
    }

    /// Conditionally write one scalar narrowed to little-endian `u32`
    /// account data when the selected common enable scalar is nonzero.
    pub const fn write_u32_if_nonzero(
        account: AccountCoordinateV3,
        offset: u32,
        value: ScalarCoordinateV3,
        enable_common_scalar: u16,
    ) -> Self {
        Self::conditional_scalar(
            OP_WRITE_DATA_U32_IF,
            account,
            value,
            offset,
            0,
            enable_common_scalar,
        )
    }

    /// Write one narrowed `u8` at `base + item * stride` in a fixed account.
    pub const fn write_u8_affine(
        account: AccountCoordinateV3,
        base: u32,
        stride: u32,
        value: ScalarCoordinateV3,
    ) -> Self {
        Self::scalar(OP_WRITE_DATA_U8_AFFINE, account, None, value, base, stride)
    }

    /// Write one narrowed `u16` at `base + item * stride` in a fixed account.
    pub const fn write_u16_affine(
        account: AccountCoordinateV3,
        base: u32,
        stride: u32,
        value: ScalarCoordinateV3,
    ) -> Self {
        Self::scalar(OP_WRITE_DATA_U16_AFFINE, account, None, value, base, stride)
    }

    /// Write one narrowed `u32` at `base + item * stride` in a fixed account.
    pub const fn write_u32_affine(
        account: AccountCoordinateV3,
        base: u32,
        stride: u32,
        value: ScalarCoordinateV3,
    ) -> Self {
        Self::scalar(OP_WRITE_DATA_U32_AFFINE, account, None, value, base, stride)
    }

    /// Write one exact identity into account data.
    pub const fn write_identity(
        account: AccountCoordinateV3,
        offset: u32,
        value: IdentityCoordinateV3,
    ) -> Self {
        Self::identity(OP_WRITE_IDENTITY, account, value, offset, 0)
    }

    /// Conditionally write one exact identity into account data when the
    /// selected common enable scalar is nonzero.
    pub const fn write_identity_if_nonzero(
        account: AccountCoordinateV3,
        offset: u32,
        value: IdentityCoordinateV3,
        enable_common_scalar: u16,
    ) -> Self {
        Self::conditional_identity(
            OP_WRITE_IDENTITY_IF,
            account,
            value,
            offset,
            enable_common_scalar,
        )
    }

    /// Write one identity at `base + item * stride` in a fixed account.
    pub const fn write_identity_affine(
        account: AccountCoordinateV3,
        base: u32,
        stride: u32,
        value: IdentityCoordinateV3,
    ) -> Self {
        Self::identity(OP_WRITE_IDENTITY_AFFINE, account, value, base, stride)
    }

    /// Patch a child request byte from a scalar.
    pub const fn write_request_u8(
        route: u16,
        space: RequestSpaceV3,
        offset: u32,
        value: ScalarCoordinateV3,
    ) -> Self {
        Self::request_scalar(OP_WRITE_REQUEST_U8, route, space, offset, value)
    }

    /// Patch a child request little-endian `u16` from a scalar.
    pub const fn write_request_u16(
        route: u16,
        space: RequestSpaceV3,
        offset: u32,
        value: ScalarCoordinateV3,
    ) -> Self {
        Self::request_scalar(OP_WRITE_REQUEST_U16, route, space, offset, value)
    }

    /// Patch a child request little-endian `u32` from a scalar.
    pub const fn write_request_u32(
        route: u16,
        space: RequestSpaceV3,
        offset: u32,
        value: ScalarCoordinateV3,
    ) -> Self {
        Self::request_scalar(OP_WRITE_REQUEST_U32, route, space, offset, value)
    }

    /// Patch a child request little-endian `u64` from a scalar.
    pub const fn write_request_u64(
        route: u16,
        space: RequestSpaceV3,
        offset: u32,
        value: ScalarCoordinateV3,
    ) -> Self {
        Self::request_scalar(OP_WRITE_REQUEST_U64, route, space, offset, value)
    }

    /// Patch a child request identity.
    pub const fn write_request_identity(
        route: u16,
        space: RequestSpaceV3,
        offset: u32,
        value: IdentityCoordinateV3,
    ) -> Self {
        Self {
            opcode: OP_WRITE_REQUEST_IDENTITY,
            account_a: AccountCoordinateV3::fixed(0),
            account_b: None,
            enable_common_scalar: None,
            scalar: None,
            identity: Some(value),
            data_offset: offset,
            extra: 0,
            route,
            request_space: Some(space),
        }
    }

    const fn scalar(
        opcode: u8,
        account_a: AccountCoordinateV3,
        account_b: Option<AccountCoordinateV3>,
        scalar: ScalarCoordinateV3,
        data_offset: u32,
        extra: u32,
    ) -> Self {
        Self {
            opcode,
            account_a,
            account_b,
            enable_common_scalar: None,
            scalar: Some(scalar),
            identity: None,
            data_offset,
            extra,
            route: 0,
            request_space: None,
        }
    }

    const fn conditional_scalar(
        opcode: u8,
        account: AccountCoordinateV3,
        scalar: ScalarCoordinateV3,
        data_offset: u32,
        extra: u32,
        enable_common_scalar: u16,
    ) -> Self {
        Self {
            opcode,
            account_a: account,
            account_b: None,
            enable_common_scalar: Some(enable_common_scalar),
            scalar: Some(scalar),
            identity: None,
            data_offset,
            extra,
            route: 0,
            request_space: None,
        }
    }

    const fn identity(
        opcode: u8,
        account: AccountCoordinateV3,
        identity: IdentityCoordinateV3,
        data_offset: u32,
        extra: u32,
    ) -> Self {
        Self {
            opcode,
            account_a: account,
            account_b: None,
            enable_common_scalar: None,
            scalar: None,
            identity: Some(identity),
            data_offset,
            extra,
            route: 0,
            request_space: None,
        }
    }

    const fn conditional_identity(
        opcode: u8,
        account: AccountCoordinateV3,
        identity: IdentityCoordinateV3,
        data_offset: u32,
        enable_common_scalar: u16,
    ) -> Self {
        Self {
            opcode,
            account_a: account,
            account_b: None,
            enable_common_scalar: Some(enable_common_scalar),
            scalar: None,
            identity: Some(identity),
            data_offset,
            extra: 0,
            route: 0,
            request_space: None,
        }
    }

    const fn request_scalar(
        opcode: u8,
        route: u16,
        space: RequestSpaceV3,
        offset: u32,
        value: ScalarCoordinateV3,
    ) -> Self {
        Self {
            opcode,
            account_a: AccountCoordinateV3::fixed(0),
            account_b: None,
            enable_common_scalar: None,
            scalar: Some(value),
            identity: None,
            data_offset: offset,
            extra: 0,
            route,
            request_space: Some(space),
        }
    }
}

/// Encode one complete EffectProgram V3 into caller-owned buffers atomically.
pub fn encode_effect_program_v3_atomic(
    geometry: EffectGeometryV3,
    routes: &[RouteInputV3<'_>],
    fixed_instructions: &[EffectInstructionV3],
    item_instructions: &[EffectInstructionV3],
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), Error> {
    encode_effect_program_with_dependencies(
        geometry,
        routes,
        fixed_instructions,
        item_instructions,
        |index| {
            routes
                .get(index)
                .map_or(&[], |route| route.receipt_dependency.as_slice())
        },
        scratch,
        output,
    )
}

/// Encode the ordered-dependency EffectProgram successor atomically.
///
/// `route_receipt_dependencies` has exactly one borrowed ordered list per
/// route. Empty and single-entry lists naturally encode existing capability
/// shapes; longer lists append every selected immediate receipt in declaration
/// order without a family discriminator.
#[allow(clippy::too_many_arguments)]
pub fn encode_effect_program_v4_atomic<'a>(
    geometry: EffectGeometryV3,
    routes: &[RouteInputV3<'_>],
    route_receipt_dependencies: &'a [&'a [RouteReceiptDependencyV3]],
    fixed_instructions: &[EffectInstructionV3],
    item_instructions: &[EffectInstructionV3],
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), Error> {
    if route_receipt_dependencies.len() != routes.len()
        || routes
            .iter()
            .any(|route| route.receipt_dependency.is_some())
    {
        return Err(Error::InvalidReceiptDependency);
    }
    encode_effect_program_with_dependencies(
        geometry,
        routes,
        fixed_instructions,
        item_instructions,
        |index| {
            route_receipt_dependencies
                .get(index)
                .copied()
                .unwrap_or(&[])
        },
        scratch,
        output,
    )
}

#[allow(clippy::too_many_arguments)]
fn encode_effect_program_with_dependencies<'a, F>(
    geometry: EffectGeometryV3,
    routes: &[RouteInputV3<'_>],
    fixed_instructions: &[EffectInstructionV3],
    item_instructions: &[EffectInstructionV3],
    dependencies: F,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), Error>
where
    F: Fn(usize) -> &'a [RouteReceiptDependencyV3],
{
    let route_count = u16::try_from(routes.len()).map_err(|_| Error::InvalidLength)?;
    let dependency_count = routes
        .iter()
        .enumerate()
        .try_fold(0_usize, |total, (index, _)| {
            total
                .checked_add(dependencies(index).len())
                .ok_or(Error::InvalidLength)
        })?;
    let dependency_count_u16 = u16::try_from(dependency_count).map_err(|_| Error::InvalidLength)?;
    let fixed_count = u16::try_from(fixed_instructions.len()).map_err(|_| Error::InvalidLength)?;
    let item_count = u16::try_from(item_instructions.len()).map_err(|_| Error::InvalidLength)?;
    let template_bytes = routes.iter().try_fold(0_usize, |total, route| {
        total
            .checked_add(route.fixed_request.len())
            .and_then(|value| value.checked_add(route.item_request.len()))
            .ok_or(Error::InvalidLength)
    })?;
    let expected = routes
        .len()
        .checked_mul(ROUTE_BYTES)
        .and_then(|route_bytes| {
            dependency_count
                .checked_mul(RECEIPT_DEPENDENCY_BYTES)
                .and_then(|dependency_bytes| route_bytes.checked_add(dependency_bytes))
        })
        .and_then(|route_bytes| {
            fixed_instructions
                .len()
                .checked_add(item_instructions.len())
                .and_then(|count| count.checked_mul(OPERATION_BYTES))
                .and_then(|operation_bytes| route_bytes.checked_add(operation_bytes))
        })
        .and_then(|body| body.checked_add(template_bytes))
        .and_then(|body| HEADER_BYTES.checked_add(body))
        .ok_or(Error::InvalidLength)?;
    if scratch.len() != expected || output.len() != expected {
        return Err(Error::InvalidLength);
    }
    scratch.fill(0);
    write(scratch, 0, &MAGIC)?;
    write_byte(scratch, 4, VERSION)?;
    for (offset, value) in [
        (6, route_count),
        (8, fixed_count),
        (10, item_count),
        (12, geometry.fixed_accounts),
        (14, geometry.item_account_stride),
        (16, geometry.common_scalars),
        (18, geometry.item_scalar_stride),
        (20, geometry.common_identities),
        (22, geometry.item_identity_stride),
        (24, dependency_count_u16),
    ] {
        write(scratch, offset, &value.to_le_bytes())?;
    }
    let mut cursor = HEADER_BYTES;
    let mut dependency_start = 0_u16;
    for (index, route) in routes.iter().enumerate() {
        let dependency_len = u16::try_from(dependencies(index).len())
            .map_err(|_| Error::InvalidReceiptDependency)?;
        encode_route(*route, dependency_start, dependency_len, scratch, cursor)?;
        dependency_start = dependency_start
            .checked_add(dependency_len)
            .ok_or(Error::InvalidReceiptDependency)?;
        cursor = add(cursor, ROUTE_BYTES)?;
    }
    for (index, _) in routes.iter().enumerate() {
        for dependency in dependencies(index) {
            encode_receipt_dependency(*dependency, scratch, cursor)?;
            cursor = add(cursor, RECEIPT_DEPENDENCY_BYTES)?;
        }
    }
    for instruction in fixed_instructions {
        encode_instruction(*instruction, false, scratch, cursor)?;
        cursor = add(cursor, OPERATION_BYTES)?;
    }
    for instruction in item_instructions {
        encode_instruction(*instruction, true, scratch, cursor)?;
        cursor = add(cursor, OPERATION_BYTES)?;
    }
    for route in routes {
        write(scratch, cursor, route.fixed_request)?;
        cursor = add(cursor, route.fixed_request.len())?;
        write(scratch, cursor, route.item_request)?;
        cursor = add(cursor, route.item_request.len())?;
    }
    if cursor != expected {
        return Err(Error::InvalidLength);
    }
    ProgramV3::decode(scratch)?;
    output.copy_from_slice(scratch);
    Ok(())
}

fn encode_route(
    route: RouteInputV3<'_>,
    dependency_start: u16,
    dependency_count: u16,
    output: &mut [u8],
    offset: usize,
) -> Result<(), Error> {
    let role = match route.role {
        FixedRole::Core => 0,
        FixedRole::Claims => 1,
        FixedRole::Resolution => 3,
        FixedRole::Custody => 4,
    };
    let kind = match route.kind {
        RouteKindV3::Once => 0,
        RouteKindV3::AffineOnce => 1,
        RouteKindV3::Each => 2,
    };
    write_byte(output, offset, role)?;
    write_byte(output, add(offset, 1)?, kind)?;
    write_byte(
        output,
        add(offset, 2)?,
        u8::from(route.enable_common_scalar.is_some()),
    )?;
    write_byte(
        output,
        add(offset, 3)?,
        u8::from(route.witness_range_common_scalar.is_some()),
    )?;
    for (local, value) in [
        (4, route.enable_common_scalar.unwrap_or(0)),
        (6, route.fixed_account_start),
        (8, route.fixed_account_count),
        (10, route.item_account_start),
        (12, route.item_account_count),
        (14, route.witness_range_common_scalar.unwrap_or(0)),
    ] {
        write(output, add(offset, local)?, &value.to_le_bytes())?;
    }
    write(
        output,
        add(offset, 16)?,
        &u32::try_from(route.fixed_request.len())
            .map_err(|_| Error::InvalidLength)?
            .to_le_bytes(),
    )?;
    write(
        output,
        add(offset, 20)?,
        &u32::try_from(route.item_request.len())
            .map_err(|_| Error::InvalidLength)?
            .to_le_bytes(),
    )?;
    write(output, add(offset, 24)?, &dependency_start.to_le_bytes())?;
    write(output, add(offset, 26)?, &dependency_count.to_le_bytes())?;
    Ok(())
}

fn encode_receipt_dependency(
    dependency: RouteReceiptDependencyV3,
    output: &mut [u8],
    offset: usize,
) -> Result<(), Error> {
    let role = match dependency.producer_role() {
        FixedRole::Core => 0,
        FixedRole::Claims => 1,
        FixedRole::Resolution => 3,
        FixedRole::Custody => 4,
    };
    write_byte(output, offset, role)?;
    write(
        output,
        add(offset, 2)?,
        &dependency.producer_route().to_le_bytes(),
    )?;
    write(
        output,
        add(offset, 4)?,
        &dependency.expected_receipt_bytes().to_le_bytes(),
    )?;
    Ok(())
}

fn encode_instruction(
    instruction: EffectInstructionV3,
    item_body: bool,
    output: &mut [u8],
    offset: usize,
) -> Result<(), Error> {
    let mut mode = 0_u8;
    if instruction.account_a.space == CoordinateSpaceV3::Item {
        mode |= MODE_ACCOUNT_A_ITEM;
    }
    if instruction.account_b.is_some() && instruction.enable_common_scalar.is_some() {
        return Err(Error::NonCanonicalOperation);
    }
    if instruction
        .account_b
        .is_some_and(|account| account.space == CoordinateSpaceV3::Item)
    {
        mode |= MODE_ACCOUNT_B_ITEM;
    }
    let account_b = instruction
        .enable_common_scalar
        .or_else(|| instruction.account_b.map(|account| account.index))
        .unwrap_or(0);
    let (register_space, register) = match (instruction.scalar, instruction.identity) {
        (Some(value), None) => (value.space, value.index),
        (None, Some(value)) => (value.space, value.index),
        _ => return Err(Error::NonCanonicalOperation),
    };
    if register_space == CoordinateSpaceV3::Item {
        mode |= MODE_REGISTER_ITEM;
    }
    if instruction.request_space == Some(RequestSpaceV3::Item) {
        mode |= MODE_REQUEST_ITEM;
    }
    if !item_body && mode != 0 {
        return Err(Error::NonCanonicalOperation);
    }
    write_byte(output, offset, instruction.opcode)?;
    write_byte(output, add(offset, 1)?, mode)?;
    for (local, value) in [
        (2, instruction.account_a.index),
        (4, account_b),
        (6, register),
        (16, instruction.route),
    ] {
        write(output, add(offset, local)?, &value.to_le_bytes())?;
    }
    write(
        output,
        add(offset, 8)?,
        &instruction.data_offset.to_le_bytes(),
    )?;
    write(output, add(offset, 12)?, &instruction.extra.to_le_bytes())
}

fn add(left: usize, right: usize) -> Result<usize, Error> {
    left.checked_add(right).ok_or(Error::InvalidLength)
}

fn write(output: &mut [u8], offset: usize, bytes: &[u8]) -> Result<(), Error> {
    let end = add(offset, bytes.len())?;
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
    #![allow(clippy::indexing_slicing)]

    extern crate std;

    use super::*;
    use crate::effect::v2::{AccountInput, AccountPermission};
    use crate::effect::v3::{ProjectionV3, ResolvedEffectV3, project_atomic};

    fn conditional_program() -> std::vec::Vec<u8> {
        let fixed = [
            EffectInstructionV3::write_u8_if_nonzero(
                AccountCoordinateV3::fixed(0),
                0,
                ScalarCoordinateV3::common(0),
                4,
            ),
            EffectInstructionV3::write_u16_if_nonzero(
                AccountCoordinateV3::fixed(0),
                1,
                ScalarCoordinateV3::common(1),
                4,
            ),
            EffectInstructionV3::write_u32_if_nonzero(
                AccountCoordinateV3::fixed(0),
                3,
                ScalarCoordinateV3::common(2),
                4,
            ),
            EffectInstructionV3::write_u64_if_nonzero(
                AccountCoordinateV3::fixed(0),
                7,
                ScalarCoordinateV3::common(3),
                4,
            ),
            EffectInstructionV3::write_identity_if_nonzero(
                AccountCoordinateV3::fixed(0),
                15,
                IdentityCoordinateV3::common(0),
                4,
            ),
        ];
        let item = [EffectInstructionV3::write_u64_affine_if_nonzero(
            AccountCoordinateV3::fixed(0),
            47,
            8,
            ScalarCoordinateV3::item(0),
            4,
        )];
        let width = HEADER_BYTES + (fixed.len() + item.len()) * OPERATION_BYTES;
        let mut scratch = std::vec![0_u8; width];
        let mut output = std::vec![0_u8; width];
        encode_effect_program_v3_atomic(
            EffectGeometryV3 {
                fixed_accounts: 1,
                item_account_stride: 0,
                common_scalars: 5,
                item_scalar_stride: 1,
                common_identities: 1,
                item_identity_stride: 0,
            },
            &[],
            &fixed,
            &item,
            &mut scratch,
            &mut output,
        )
        .expect("conditional program");
        output
    }

    fn two_tail_program(
        first_account: AccountCoordinateV3,
        first_base: u32,
        second_account: AccountCoordinateV3,
        second_base: u32,
        stride: u32,
    ) -> std::vec::Vec<u8> {
        let item = [
            EffectInstructionV3::write_u64_affine(
                first_account,
                first_base,
                stride,
                ScalarCoordinateV3::item(0),
            ),
            EffectInstructionV3::write_u64_second_tail_affine_if_nonzero(
                second_account,
                second_base,
                stride,
                ScalarCoordinateV3::item(0),
                0,
            ),
        ];
        let width = HEADER_BYTES + item.len() * OPERATION_BYTES;
        let mut scratch = std::vec![0_u8; width];
        let mut output = std::vec![0_u8; width];
        encode_effect_program_v3_atomic(
            EffectGeometryV3 {
                fixed_accounts: 2,
                item_account_stride: 0,
                common_scalars: 1,
                item_scalar_stride: 1,
                common_identities: 0,
                item_identity_stride: 0,
            },
            &[],
            &[],
            &item,
            &mut scratch,
            &mut output,
        )
        .expect("two-tail program");
        output
    }

    fn second_tail_only_program() -> std::vec::Vec<u8> {
        let item = [
            EffectInstructionV3::write_u64_second_tail_affine_if_nonzero(
                AccountCoordinateV3::fixed(0),
                0,
                8,
                ScalarCoordinateV3::item(0),
                0,
            ),
        ];
        let width = HEADER_BYTES + OPERATION_BYTES;
        let mut scratch = std::vec![0_u8; width];
        let mut output = std::vec![0_u8; width];
        encode_effect_program_v3_atomic(
            EffectGeometryV3 {
                fixed_accounts: 1,
                item_account_stride: 0,
                common_scalars: 1,
                item_scalar_stride: 1,
                common_identities: 0,
                item_identity_stride: 0,
            },
            &[],
            &[],
            &item,
            &mut scratch,
            &mut output,
        )
        .expect("second-tail-only program");
        output
    }

    fn five_tail_program() -> std::vec::Vec<u8> {
        let account = AccountCoordinateV3::fixed(0);
        let value = ScalarCoordinateV3::item(0);
        let item = [
            EffectInstructionV3::write_u64_affine(account, 0, 8, value),
            EffectInstructionV3::write_u64_second_tail_affine(account, 0, 8, value),
            EffectInstructionV3::write_u64_third_tail_affine(account, 0, 8, value),
            EffectInstructionV3::write_u64_fourth_tail_affine(account, 0, 8, value),
            EffectInstructionV3::write_u64_fifth_tail_affine(account, 0, 8, value),
        ];
        let width = HEADER_BYTES + item.len() * OPERATION_BYTES;
        let mut scratch = std::vec![0_u8; width];
        let mut output = std::vec![0_u8; width];
        encode_effect_program_v3_atomic(
            EffectGeometryV3 {
                fixed_accounts: 1,
                item_account_stride: 0,
                common_scalars: 1,
                item_scalar_stride: 1,
                common_identities: 0,
                item_identity_stride: 0,
            },
            &[],
            &[],
            &item,
            &mut scratch,
            &mut output,
        )
        .expect("five-tail program");
        output
    }

    #[test]
    fn conditional_writes_preserve_old_wire_and_encode_one_common_enable() {
        let old = [EffectInstructionV3::write_u64(
            AccountCoordinateV3::fixed(0),
            7,
            ScalarCoordinateV3::common(0),
        )];
        let width = HEADER_BYTES + OPERATION_BYTES;
        let mut scratch = std::vec![0_u8; width];
        let mut encoded = std::vec![0_u8; width];
        encode_effect_program_v3_atomic(
            EffectGeometryV3 {
                fixed_accounts: 1,
                item_account_stride: 0,
                common_scalars: 1,
                item_scalar_stride: 0,
                common_identities: 0,
                item_identity_stride: 0,
            },
            &[],
            &old,
            &[],
            &mut scratch,
            &mut encoded,
        )
        .expect("old write");
        let mut expected = std::vec![0_u8; width];
        expected[..4].copy_from_slice(&MAGIC);
        expected[4] = VERSION;
        expected[8..10].copy_from_slice(&1_u16.to_le_bytes());
        expected[12..14].copy_from_slice(&1_u16.to_le_bytes());
        expected[16..18].copy_from_slice(&1_u16.to_le_bytes());
        expected[HEADER_BYTES] = OP_WRITE_SCALAR;
        expected[HEADER_BYTES + 8..HEADER_BYTES + 12].copy_from_slice(&7_u32.to_le_bytes());
        assert_eq!(encoded, expected, "predecessor opcode bytes changed");

        let encoded = conditional_program();
        for (ordinal, opcode) in [
            OP_WRITE_DATA_U8_IF,
            OP_WRITE_DATA_U16_IF,
            OP_WRITE_DATA_U32_IF,
            OP_WRITE_SCALAR_IF,
            OP_WRITE_IDENTITY_IF,
            OP_WRITE_SCALAR_AFFINE_IF,
        ]
        .into_iter()
        .enumerate()
        {
            let start = HEADER_BYTES + ordinal * OPERATION_BYTES;
            assert_eq!(encoded[start], opcode);
            assert_eq!(&encoded[start + 4..start + 6], &4_u16.to_le_bytes());
            assert_eq!(&encoded[start + 16..start + 24], &[0_u8; 8]);
        }
        assert_eq!(
            encoded[HEADER_BYTES + 5 * OPERATION_BYTES + 1],
            MODE_REGISTER_ITEM
        );
    }

    #[test]
    fn conditional_writes_omit_vacant_state_and_enforce_enabled_hostiles() {
        let encoded = conditional_program();
        let program = ProgramV3::decode(&encoded).expect("conditional decode");
        assert_eq!(program.data_write_operation_count(2), Ok(7));
        let mut scalars = [255, u16::MAX as u64, u32::MAX as u64, 9, 0, 11, 12];
        let identities = [[0x5a; 32]];
        assert_eq!(
            program.resolved_fixed_effect(0, 2, &scalars, &identities),
            Ok(ResolvedEffectV3::Noop)
        );
        assert_eq!(
            program.resolved_item_effect(1, 0, 2, &scalars, &identities),
            Ok(ResolvedEffectV3::Noop)
        );
        let vacant = [AccountInput {
            lamports: 0,
            data_len: 0,
        }];
        let read_only = [AccountPermission::read_only()];
        let aliases = [0_usize];
        let mut scratch_lamports = [91_u64];
        let mut output_lamports = [92_u64];
        project_atomic(
            program,
            2,
            ProjectionV3 {
                scalars: &scalars,
                identities: &identities,
                aliases: &aliases,
                accounts: &vacant,
                permissions: &read_only,
                scratch_lamports: &mut scratch_lamports,
                output_lamports: &mut output_lamports,
                requests: &mut [],
            },
        )
        .expect("disabled write never touches vacant result");
        assert_eq!(output_lamports, [0]);

        scalars[4] = 1;
        assert_eq!(
            program.resolved_fixed_effect(0, 2, &scalars, &identities),
            Ok(ResolvedEffectV3::WriteU8 {
                account: 0,
                offset: 0,
                value: u8::MAX,
            })
        );
        assert_eq!(
            program.resolved_item_effect(1, 0, 2, &scalars, &identities),
            Ok(ResolvedEffectV3::WriteScalar {
                account: 0,
                offset: 55,
                value: 12,
            })
        );
        output_lamports = [77];
        assert_eq!(
            project_atomic(
                program,
                2,
                ProjectionV3 {
                    scalars: &scalars,
                    identities: &identities,
                    aliases: &aliases,
                    accounts: &vacant,
                    permissions: &read_only,
                    scratch_lamports: &mut scratch_lamports,
                    output_lamports: &mut output_lamports,
                    requests: &mut [],
                },
            ),
            Err(Error::PermissionDenied)
        );
        assert_eq!(output_lamports, [77], "refusal exposed partial output");

        let writable = [AccountPermission::new(false, false, true)];
        assert_eq!(
            project_atomic(
                program,
                2,
                ProjectionV3 {
                    scalars: &scalars,
                    identities: &identities,
                    aliases: &aliases,
                    accounts: &vacant,
                    permissions: &writable,
                    scratch_lamports: &mut scratch_lamports,
                    output_lamports: &mut output_lamports,
                    requests: &mut [],
                },
            ),
            Err(Error::DataOutOfBounds)
        );

        scalars[0] = u64::from(u8::MAX) + 1;
        assert_eq!(
            program.resolved_fixed_effect(0, 2, &scalars, &identities),
            Err(Error::NarrowingOverflow)
        );
        scalars[4] = 0;
        assert_eq!(
            program.resolved_fixed_effect(0, 2, &scalars, &identities),
            Ok(ResolvedEffectV3::Noop),
            "disabled write narrowed a value it must not read"
        );
    }

    #[test]
    fn conditional_writes_refuse_enable_geometry_and_possible_overlaps() {
        let instruction = [EffectInstructionV3::write_u8_if_nonzero(
            AccountCoordinateV3::fixed(0),
            0,
            ScalarCoordinateV3::common(0),
            1,
        )];
        let width = HEADER_BYTES + OPERATION_BYTES;
        let mut scratch = std::vec![0_u8; width];
        let mut output = std::vec![0xa5_u8; width];
        let before = output.clone();
        assert_eq!(
            encode_effect_program_v3_atomic(
                EffectGeometryV3 {
                    fixed_accounts: 1,
                    item_account_stride: 0,
                    common_scalars: 1,
                    item_scalar_stride: 0,
                    common_identities: 0,
                    item_identity_stride: 0,
                },
                &[],
                &instruction,
                &[],
                &mut scratch,
                &mut output,
            ),
            Err(Error::InvalidCoordinate)
        );
        assert_eq!(output, before, "hostile enable changed encoder output");

        let overlap = [
            EffectInstructionV3::write_u8_if_nonzero(
                AccountCoordinateV3::fixed(0),
                0,
                ScalarCoordinateV3::common(0),
                1,
            ),
            EffectInstructionV3::write_u16(
                AccountCoordinateV3::fixed(0),
                0,
                ScalarCoordinateV3::common(0),
            ),
        ];
        let overlap_width = HEADER_BYTES + overlap.len() * OPERATION_BYTES;
        let mut overlap_scratch = std::vec![0_u8; overlap_width];
        let mut overlap_output = std::vec![0_u8; overlap_width];
        assert_eq!(
            encode_effect_program_v3_atomic(
                EffectGeometryV3 {
                    fixed_accounts: 1,
                    item_account_stride: 0,
                    common_scalars: 2,
                    item_scalar_stride: 0,
                    common_identities: 0,
                    item_identity_stride: 0,
                },
                &[],
                &overlap,
                &[],
                &mut overlap_scratch,
                &mut overlap_output,
            ),
            Err(Error::OverlappingWrites),
            "static overlap ignored a possibly enabled write"
        );

        let mut hostile = conditional_program();
        let affine = HEADER_BYTES + 5 * OPERATION_BYTES;
        hostile[affine + 1] |= MODE_ACCOUNT_B_ITEM;
        assert_eq!(
            ProgramV3::decode(&hostile),
            Err(Error::NonCanonicalOperation)
        );

        let mut hostile = conditional_program();
        hostile[HEADER_BYTES + 6..HEADER_BYTES + 8].copy_from_slice(&5_u16.to_le_bytes());
        assert_eq!(ProgramV3::decode(&hostile), Err(Error::InvalidCoordinate));
    }

    #[test]
    fn enabled_conditional_write_refuses_dynamic_alias_overlap_atomically() {
        let fixed = [
            EffectInstructionV3::write_u64_if_nonzero(
                AccountCoordinateV3::fixed(0),
                0,
                ScalarCoordinateV3::common(0),
                1,
            ),
            EffectInstructionV3::write_u64(
                AccountCoordinateV3::fixed(1),
                0,
                ScalarCoordinateV3::common(0),
            ),
        ];
        let width = HEADER_BYTES + fixed.len() * OPERATION_BYTES;
        let mut scratch = std::vec![0_u8; width];
        let mut encoded = std::vec![0_u8; width];
        encode_effect_program_v3_atomic(
            EffectGeometryV3 {
                fixed_accounts: 2,
                item_account_stride: 0,
                common_scalars: 2,
                item_scalar_stride: 0,
                common_identities: 0,
                item_identity_stride: 0,
            },
            &[],
            &fixed,
            &[],
            &mut scratch,
            &mut encoded,
        )
        .expect("structurally disjoint writes");
        let program = ProgramV3::decode(&encoded).expect("alias program");
        let accounts = [
            AccountInput {
                lamports: 1,
                data_len: 8,
            },
            AccountInput {
                lamports: 1,
                data_len: 8,
            },
        ];
        let permissions = [
            AccountPermission::new(false, false, true),
            AccountPermission::new(false, false, true),
        ];
        let aliases = [0_usize, 0];
        let mut scratch_lamports = [0_u64; 2];
        let mut output_lamports = [88_u64; 2];
        let before = output_lamports;
        assert_eq!(
            project_atomic(
                program,
                0,
                ProjectionV3 {
                    scalars: &[7, 1],
                    identities: &[],
                    aliases: &aliases,
                    accounts: &accounts,
                    permissions: &permissions,
                    scratch_lamports: &mut scratch_lamports,
                    output_lamports: &mut output_lamports,
                    requests: &mut [],
                },
            ),
            Err(Error::OverlappingWrites)
        );
        assert_eq!(output_lamports, before);

        project_atomic(
            program,
            0,
            ProjectionV3 {
                scalars: &[7, 0],
                identities: &[],
                aliases: &aliases,
                accounts: &accounts,
                permissions: &permissions,
                scratch_lamports: &mut scratch_lamports,
                output_lamports: &mut output_lamports,
                requests: &mut [],
            },
        )
        .expect("disabled write has no alias range");
    }

    #[test]
    fn second_tail_affine_is_exact_at_zero_one_and_258_items() {
        let encoded = two_tail_program(
            AccountCoordinateV3::fixed(0),
            0,
            AccountCoordinateV3::fixed(0),
            0,
            8,
        );
        let program = ProgramV3::decode(&encoded).expect("two tails");
        assert_eq!(program.data_write_operation_count(0), Ok(0));
        assert_eq!(program.data_write_operation_count(1), Ok(2));
        assert_eq!(program.data_write_operation_count(258), Ok(516));

        let aliases = [0_usize, 1];
        let vacant = [
            AccountInput {
                lamports: 0,
                data_len: 0,
            },
            AccountInput {
                lamports: 0,
                data_len: 0,
            },
        ];
        let read_only = [AccountPermission::read_only(); 2];
        let mut scratch_lamports = [7_u64; 2];
        let mut output_lamports = [8_u64; 2];
        project_atomic(
            program,
            0,
            ProjectionV3 {
                scalars: &[1],
                identities: &[],
                aliases: &aliases,
                accounts: &vacant,
                permissions: &read_only,
                scratch_lamports: &mut scratch_lamports,
                output_lamports: &mut output_lamports,
                requests: &mut [],
            },
        )
        .expect("zero items execute no affine writes");

        let disabled = [0_u64, 44];
        assert_eq!(
            program.resolved_item_effect(0, 1, 1, &disabled, &[]),
            Ok(ResolvedEffectV3::Noop)
        );
        project_atomic(
            program,
            1,
            ProjectionV3 {
                scalars: &disabled,
                identities: &[],
                aliases: &aliases,
                accounts: &vacant,
                permissions: &read_only,
                scratch_lamports: &mut scratch_lamports,
                output_lamports: &mut output_lamports,
                requests: &mut [],
            },
        )
        .expect_err("the unconditional first tail still reaches vacant data");

        let enabled = [1_u64, 44];
        assert_eq!(
            program.resolved_item_effect(0, 0, 1, &enabled, &[]),
            Ok(ResolvedEffectV3::WriteScalar {
                account: 0,
                offset: 0,
                value: 44,
            })
        );
        assert_eq!(
            program.resolved_item_effect(0, 1, 1, &enabled, &[]),
            Ok(ResolvedEffectV3::WriteScalar {
                account: 0,
                offset: 8,
                value: 44,
            })
        );

        let mut large_scalars = std::vec![0_u64; 259];
        large_scalars[0] = 1;
        large_scalars[258] = 0x258;
        assert_eq!(
            program.resolved_item_effect(257, 0, 258, &large_scalars, &[]),
            Ok(ResolvedEffectV3::WriteScalar {
                account: 0,
                offset: 2_056,
                value: 0x258,
            })
        );
        assert_eq!(
            program.resolved_item_effect(257, 1, 258, &large_scalars, &[]),
            Ok(ResolvedEffectV3::WriteScalar {
                account: 0,
                offset: 4_120,
                value: 0x258,
            })
        );
    }

    #[test]
    fn disabled_second_tail_leaves_its_vacant_result_coordinate_untouched() {
        let encoded = second_tail_only_program();
        let program = ProgramV3::decode(&encoded).expect("second tail");
        let aliases = [0_usize];
        let vacant = [AccountInput {
            lamports: 0,
            data_len: 0,
        }];
        let mut scratch_lamports = [71_u64];
        let mut output_lamports = [72_u64];
        project_atomic(
            program,
            1,
            ProjectionV3 {
                scalars: &[0, u64::MAX],
                identities: &[],
                aliases: &aliases,
                accounts: &vacant,
                permissions: &[AccountPermission::read_only()],
                scratch_lamports: &mut scratch_lamports,
                output_lamports: &mut output_lamports,
                requests: &mut [],
            },
        )
        .expect("disabled second tail");
        assert_eq!(output_lamports, [0]);

        output_lamports = [72];
        assert_eq!(
            project_atomic(
                program,
                1,
                ProjectionV3 {
                    scalars: &[1, 9],
                    identities: &[],
                    aliases: &aliases,
                    accounts: &vacant,
                    permissions: &[AccountPermission::read_only()],
                    scratch_lamports: &mut scratch_lamports,
                    output_lamports: &mut output_lamports,
                    requests: &mut [],
                },
            ),
            Err(Error::PermissionDenied)
        );
        assert_eq!(output_lamports, [72]);
    }

    #[test]
    fn second_tail_affine_refuses_wrong_spaces_stride_and_runtime_overlap() {
        let wrong_fixed = [
            EffectInstructionV3::write_u64_second_tail_affine_if_nonzero(
                AccountCoordinateV3::fixed(0),
                0,
                8,
                ScalarCoordinateV3::item(0),
                0,
            ),
        ];
        let wrong_account = [
            EffectInstructionV3::write_u64_second_tail_affine_if_nonzero(
                AccountCoordinateV3::item(0),
                0,
                8,
                ScalarCoordinateV3::item(0),
                0,
            ),
        ];
        let wrong_value = [
            EffectInstructionV3::write_u64_second_tail_affine_if_nonzero(
                AccountCoordinateV3::fixed(0),
                0,
                8,
                ScalarCoordinateV3::common(0),
                0,
            ),
        ];
        for (fixed, item) in [
            (wrong_fixed.as_slice(), &[][..]),
            (&[][..], wrong_account.as_slice()),
            (&[][..], wrong_value.as_slice()),
        ] {
            let width = HEADER_BYTES + (fixed.len() + item.len()) * OPERATION_BYTES;
            let mut scratch = std::vec![0_u8; width];
            let mut output = std::vec![0xa5_u8; width];
            let before = output.clone();
            assert_eq!(
                encode_effect_program_v3_atomic(
                    EffectGeometryV3 {
                        fixed_accounts: 1,
                        item_account_stride: 1,
                        common_scalars: 1,
                        item_scalar_stride: 1,
                        common_identities: 0,
                        item_identity_stride: 0,
                    },
                    &[],
                    fixed,
                    item,
                    &mut scratch,
                    &mut output,
                ),
                Err(Error::NonCanonicalOperation)
            );
            assert_eq!(output, before);
        }
        for stride in [0_u32, 7] {
            let item = [
                EffectInstructionV3::write_u64_second_tail_affine_if_nonzero(
                    AccountCoordinateV3::fixed(0),
                    0,
                    stride,
                    ScalarCoordinateV3::item(0),
                    0,
                ),
            ];
            let width = HEADER_BYTES + OPERATION_BYTES;
            let mut scratch = std::vec![0_u8; width];
            let mut output = std::vec![0_u8; width];
            assert_eq!(
                encode_effect_program_v3_atomic(
                    EffectGeometryV3 {
                        fixed_accounts: 1,
                        item_account_stride: 0,
                        common_scalars: 1,
                        item_scalar_stride: 1,
                        common_identities: 0,
                        item_identity_stride: 0,
                    },
                    &[],
                    &[],
                    &item,
                    &mut scratch,
                    &mut output,
                ),
                Err(Error::NonCanonicalOperation)
            );
        }

        let canonical = second_tail_only_program();
        let mut hostile = canonical.clone();
        hostile[HEADER_BYTES + 1] |= MODE_ACCOUNT_B_ITEM;
        assert_eq!(
            ProgramV3::decode(&hostile),
            Err(Error::NonCanonicalOperation)
        );
        let mut hostile = canonical.clone();
        hostile[HEADER_BYTES + 4..HEADER_BYTES + 6].copy_from_slice(&1_u16.to_le_bytes());
        assert_eq!(ProgramV3::decode(&hostile), Err(Error::InvalidCoordinate));
        let mut hostile = canonical;
        hostile[HEADER_BYTES + 6..HEADER_BYTES + 8].copy_from_slice(&1_u16.to_le_bytes());
        assert_eq!(ProgramV3::decode(&hostile), Err(Error::InvalidCoordinate));

        let encoded = two_tail_program(
            AccountCoordinateV3::fixed(0),
            8,
            AccountCoordinateV3::fixed(0),
            0,
            8,
        );
        let program = ProgramV3::decode(&encoded).expect("runtime overlap program");
        let accounts = [
            AccountInput {
                lamports: 1,
                data_len: 24,
            },
            AccountInput {
                lamports: 1,
                data_len: 24,
            },
        ];
        let permissions = [AccountPermission::new(false, false, true); 2];
        let aliases = [0_usize, 1];
        let mut scratch_lamports = [0_u64; 2];
        let mut output_lamports = [77_u64; 2];
        let before = output_lamports;
        assert_eq!(
            project_atomic(
                program,
                1,
                ProjectionV3 {
                    scalars: &[1, 5],
                    identities: &[],
                    aliases: &aliases,
                    accounts: &accounts,
                    permissions: &permissions,
                    scratch_lamports: &mut scratch_lamports,
                    output_lamports: &mut output_lamports,
                    requests: &mut [],
                },
            ),
            Err(Error::OverlappingWrites)
        );
        assert_eq!(output_lamports, before);

        project_atomic(
            program,
            1,
            ProjectionV3 {
                scalars: &[0, 5],
                identities: &[],
                aliases: &aliases,
                accounts: &accounts,
                permissions: &permissions,
                scratch_lamports: &mut scratch_lamports,
                output_lamports: &mut output_lamports,
                requests: &mut [],
            },
        )
        .expect("disabled second tail has no overlapping range");

        let aliased = two_tail_program(
            AccountCoordinateV3::fixed(0),
            8,
            AccountCoordinateV3::fixed(1),
            0,
            8,
        );
        let program = ProgramV3::decode(&aliased).expect("alias overlap program");
        assert_eq!(
            project_atomic(
                program,
                1,
                ProjectionV3 {
                    scalars: &[1, 5],
                    identities: &[],
                    aliases: &[0, 0],
                    accounts: &accounts,
                    permissions: &permissions,
                    scratch_lamports: &mut scratch_lamports,
                    output_lamports: &mut output_lamports,
                    requests: &mut [],
                },
            ),
            Err(Error::OverlappingWrites)
        );
    }

    #[test]
    fn five_planar_tails_are_exact_at_zero_one_and_258_items() {
        let encoded = five_tail_program();
        let program = ProgramV3::decode(&encoded).expect("five planar tails");
        assert_eq!(program.data_write_operation_count(0), Ok(0));
        assert_eq!(program.data_write_operation_count(1), Ok(5));
        assert_eq!(program.data_write_operation_count(258), Ok(1_290));
        for (ordinal, opcode) in [
            OP_WRITE_SCALAR_AFFINE,
            OP_WRITE_SCALAR_SECOND_TAIL_AFFINE,
            OP_WRITE_SCALAR_THIRD_TAIL_AFFINE,
            OP_WRITE_SCALAR_FOURTH_TAIL_AFFINE,
            OP_WRITE_SCALAR_FIFTH_TAIL_AFFINE,
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(encoded[HEADER_BYTES + ordinal * OPERATION_BYTES], opcode);
        }

        let mut scratch_lamports = [7_u64];
        let mut output_lamports = [8_u64];
        project_atomic(
            program,
            0,
            ProjectionV3 {
                scalars: &[1],
                identities: &[],
                aliases: &[0],
                accounts: &[AccountInput {
                    lamports: 0,
                    data_len: 0,
                }],
                permissions: &[AccountPermission::read_only()],
                scratch_lamports: &mut scratch_lamports,
                output_lamports: &mut output_lamports,
                requests: &mut [],
            },
        )
        .expect("zero items touch no plane");

        for (operation, offset) in [0_u32, 8, 16, 24, 32].into_iter().enumerate() {
            assert_eq!(
                program.resolved_item_effect(
                    0,
                    u16::try_from(operation).expect("small"),
                    1,
                    &[1, 44],
                    &[],
                ),
                Ok(ResolvedEffectV3::WriteScalar {
                    account: 0,
                    offset,
                    value: 44,
                })
            );
        }

        let mut scalars = std::vec![0_u64; 259];
        scalars[0] = 1;
        scalars[258] = 0x258;
        for (operation, offset) in [2_056_u32, 4_120, 6_184, 8_248, 10_312]
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                program.resolved_item_effect(
                    257,
                    u16::try_from(operation).expect("small"),
                    258,
                    &scalars,
                    &[],
                ),
                Ok(ResolvedEffectV3::WriteScalar {
                    account: 0,
                    offset,
                    value: 0x258,
                })
            );
        }
    }

    #[test]
    fn later_planar_tails_refuse_wrong_geometry_permission_and_overlap_atomically() {
        let account = AccountCoordinateV3::fixed(0);
        let item_value = ScalarCoordinateV3::item(0);
        let fixed_hostiles = [
            EffectInstructionV3::write_u64_second_tail_affine(account, 0, 8, item_value),
            EffectInstructionV3::write_u64_third_tail_affine(account, 0, 8, item_value),
            EffectInstructionV3::write_u64_fourth_tail_affine(account, 0, 8, item_value),
            EffectInstructionV3::write_u64_fifth_tail_affine(account, 0, 8, item_value),
        ];
        for instruction in fixed_hostiles {
            let width = HEADER_BYTES + OPERATION_BYTES;
            let mut scratch = std::vec![0_u8; width];
            let mut output = std::vec![0xa5_u8; width];
            let before = output.clone();
            assert_eq!(
                encode_effect_program_v3_atomic(
                    EffectGeometryV3 {
                        fixed_accounts: 1,
                        item_account_stride: 0,
                        common_scalars: 1,
                        item_scalar_stride: 1,
                        common_identities: 0,
                        item_identity_stride: 0,
                    },
                    &[],
                    &[instruction],
                    &[],
                    &mut scratch,
                    &mut output,
                ),
                Err(Error::NonCanonicalOperation)
            );
            assert_eq!(output, before);
        }
        for instruction in [
            EffectInstructionV3::write_u64_third_tail_affine(
                AccountCoordinateV3::item(0),
                0,
                8,
                item_value,
            ),
            EffectInstructionV3::write_u64_fourth_tail_affine(
                account,
                0,
                8,
                ScalarCoordinateV3::common(0),
            ),
            EffectInstructionV3::write_u64_fifth_tail_affine(account, 0, 0, item_value),
            EffectInstructionV3::write_u64_fifth_tail_affine(account, 0, 7, item_value),
            EffectInstructionV3::write_u64_second_tail_affine(account, 0, 0, item_value),
        ] {
            let width = HEADER_BYTES + OPERATION_BYTES;
            let mut scratch = std::vec![0_u8; width];
            let mut output = std::vec![0_u8; width];
            assert_eq!(
                encode_effect_program_v3_atomic(
                    EffectGeometryV3 {
                        fixed_accounts: 1,
                        item_account_stride: 1,
                        common_scalars: 1,
                        item_scalar_stride: 1,
                        common_identities: 0,
                        item_identity_stride: 0,
                    },
                    &[],
                    &[],
                    &[instruction],
                    &mut scratch,
                    &mut output,
                ),
                Err(Error::NonCanonicalOperation)
            );
        }

        let encode_overlap = |third_base: u32, fifth: bool| {
            let later = if fifth {
                EffectInstructionV3::write_u64_fifth_tail_affine(account, 0, 8, item_value)
            } else {
                EffectInstructionV3::write_u64_fourth_tail_affine(account, 0, 8, item_value)
            };
            let item = [
                EffectInstructionV3::write_u64_third_tail_affine(
                    account, third_base, 8, item_value,
                ),
                later,
            ];
            let width = HEADER_BYTES + item.len() * OPERATION_BYTES;
            let mut scratch = std::vec![0_u8; width];
            let mut output = std::vec![0_u8; width];
            encode_effect_program_v3_atomic(
                EffectGeometryV3 {
                    fixed_accounts: 1,
                    item_account_stride: 0,
                    common_scalars: 1,
                    item_scalar_stride: 1,
                    common_identities: 0,
                    item_identity_stride: 0,
                },
                &[],
                &[],
                &item,
                &mut scratch,
                &mut output,
            )
            .expect("runtime overlap program");
            output
        };
        for encoded in [encode_overlap(8, false), encode_overlap(16, true)] {
            let program = ProgramV3::decode(&encoded).expect("overlap decode");
            let mut scratch_lamports = [0_u64];
            let mut output_lamports = [77_u64];
            assert_eq!(
                project_atomic(
                    program,
                    1,
                    ProjectionV3 {
                        scalars: &[1, 5],
                        identities: &[],
                        aliases: &[0],
                        accounts: &[AccountInput {
                            lamports: 1,
                            data_len: 40,
                        }],
                        permissions: &[AccountPermission::new(false, false, true)],
                        scratch_lamports: &mut scratch_lamports,
                        output_lamports: &mut output_lamports,
                        requests: &mut [],
                    },
                ),
                Err(Error::OverlappingWrites)
            );
            assert_eq!(output_lamports, [77]);
        }

        let alias_operations = [
            EffectInstructionV3::write_u64_third_tail_affine(
                AccountCoordinateV3::fixed(0),
                8,
                8,
                item_value,
            ),
            EffectInstructionV3::write_u64_fourth_tail_affine(
                AccountCoordinateV3::fixed(1),
                0,
                8,
                item_value,
            ),
        ];
        let alias_width = HEADER_BYTES + alias_operations.len() * OPERATION_BYTES;
        let mut alias_scratch = std::vec![0_u8; alias_width];
        let mut alias_bytes = std::vec![0_u8; alias_width];
        encode_effect_program_v3_atomic(
            EffectGeometryV3 {
                fixed_accounts: 2,
                item_account_stride: 0,
                common_scalars: 1,
                item_scalar_stride: 1,
                common_identities: 0,
                item_identity_stride: 0,
            },
            &[],
            &[],
            &alias_operations,
            &mut alias_scratch,
            &mut alias_bytes,
        )
        .expect("structurally disjoint later planes");
        let alias_program = ProgramV3::decode(&alias_bytes).expect("alias program");
        let aliased_accounts = [
            AccountInput {
                lamports: 1,
                data_len: 40,
            },
            AccountInput {
                lamports: 1,
                data_len: 40,
            },
        ];
        let aliased_permissions = [AccountPermission::new(false, false, true); 2];
        let mut alias_scratch_lamports = [0_u64; 2];
        let mut alias_output_lamports = [81_u64; 2];
        assert_eq!(
            project_atomic(
                alias_program,
                1,
                ProjectionV3 {
                    scalars: &[1, 5],
                    identities: &[],
                    aliases: &[0, 0],
                    accounts: &aliased_accounts,
                    permissions: &aliased_permissions,
                    scratch_lamports: &mut alias_scratch_lamports,
                    output_lamports: &mut alias_output_lamports,
                    requests: &mut [],
                },
            ),
            Err(Error::OverlappingWrites)
        );
        assert_eq!(alias_output_lamports, [81; 2]);

        let fifth_only = [EffectInstructionV3::write_u64_fifth_tail_affine(
            account, 0, 8, item_value,
        )];
        let width = HEADER_BYTES + OPERATION_BYTES;
        let mut scratch = std::vec![0_u8; width];
        let mut output = std::vec![0_u8; width];
        encode_effect_program_v3_atomic(
            EffectGeometryV3 {
                fixed_accounts: 1,
                item_account_stride: 0,
                common_scalars: 1,
                item_scalar_stride: 1,
                common_identities: 0,
                item_identity_stride: 0,
            },
            &[],
            &[],
            &fifth_only,
            &mut scratch,
            &mut output,
        )
        .expect("fifth plane");
        let program = ProgramV3::decode(&output).expect("fifth decode");
        let mut scratch_lamports = [0_u64];
        let mut output_lamports = [91_u64];
        assert_eq!(
            project_atomic(
                program,
                1,
                ProjectionV3 {
                    scalars: &[1, 5],
                    identities: &[],
                    aliases: &[0],
                    accounts: &[AccountInput {
                        lamports: 0,
                        data_len: 0,
                    }],
                    permissions: &[AccountPermission::read_only()],
                    scratch_lamports: &mut scratch_lamports,
                    output_lamports: &mut output_lamports,
                    requests: &mut [],
                },
            ),
            Err(Error::PermissionDenied)
        );
        assert_eq!(output_lamports, [91]);

        let mut hostile = five_tail_program();
        let third = HEADER_BYTES + 2 * OPERATION_BYTES;
        hostile[third + 6..third + 8].copy_from_slice(&1_u16.to_le_bytes());
        assert_eq!(ProgramV3::decode(&hostile), Err(Error::InvalidCoordinate));
    }

    #[test]
    fn typed_encoder_round_trips_routes_writes_and_preserves_output() {
        let routes = [RouteInputV3 {
            role: FixedRole::Custody,
            kind: RouteKindV3::Once,
            enable_common_scalar: Some(0),
            witness_range_common_scalar: None,
            receipt_dependency: None,
            fixed_account_start: 0,
            fixed_account_count: 1,
            item_account_start: 0,
            item_account_count: 0,
            fixed_request: &[0_u8; 8],
            item_request: &[],
        }];
        let instructions = [
            EffectInstructionV3::write_request_u64(
                0,
                RequestSpaceV3::Fixed,
                0,
                ScalarCoordinateV3::common(0),
            ),
            EffectInstructionV3::write_u16(
                AccountCoordinateV3::fixed(0),
                2,
                ScalarCoordinateV3::common(1),
            ),
        ];
        let geometry = EffectGeometryV3 {
            fixed_accounts: 1,
            item_account_stride: 0,
            common_scalars: 2,
            item_scalar_stride: 0,
            common_identities: 0,
            item_identity_stride: 0,
        };
        let width = HEADER_BYTES
            + ROUTE_BYTES
            + instructions.len() * OPERATION_BYTES
            + routes[0].fixed_request.len();
        let mut scratch = std::vec![0_u8; width];
        let mut output = std::vec![9_u8; width];
        encode_effect_program_v3_atomic(
            geometry,
            &routes,
            &instructions,
            &[],
            &mut scratch,
            &mut output,
        )
        .expect("encode");
        let program = ProgramV3::decode(&output).expect("decode encoded program");
        assert_eq!(program.route_count(), 1);
        assert_eq!(program.fixed_operation_count(), 2);

        let overlapping = [
            EffectInstructionV3::write_u16(
                AccountCoordinateV3::fixed(0),
                0,
                ScalarCoordinateV3::common(0),
            ),
            EffectInstructionV3::write_u32(
                AccountCoordinateV3::fixed(0),
                1,
                ScalarCoordinateV3::common(1),
            ),
        ];
        let hostile_width = HEADER_BYTES
            + ROUTE_BYTES
            + overlapping.len() * OPERATION_BYTES
            + routes[0].fixed_request.len();
        let mut hostile_scratch = std::vec![0_u8; hostile_width];
        let mut hostile_output = std::vec![7_u8; hostile_width];
        let before = hostile_output.clone();
        assert_eq!(
            encode_effect_program_v3_atomic(
                geometry,
                &routes,
                &overlapping,
                &[],
                &mut hostile_scratch,
                &mut hostile_output,
            ),
            Err(Error::OverlappingWrites)
        );
        assert_eq!(hostile_output, before);
    }

    #[test]
    fn ordered_dependency_encoder_preserves_plural_declaration_order() {
        let routes = [
            RouteInputV3 {
                role: FixedRole::Custody,
                kind: RouteKindV3::Once,
                enable_common_scalar: None,
                witness_range_common_scalar: None,
                receipt_dependency: None,
                fixed_account_start: 0,
                fixed_account_count: 1,
                item_account_start: 0,
                item_account_count: 0,
                fixed_request: b"LOCK_REQ",
                item_request: &[],
            },
            RouteInputV3 {
                role: FixedRole::Custody,
                kind: RouteKindV3::Once,
                enable_common_scalar: None,
                witness_range_common_scalar: None,
                receipt_dependency: None,
                fixed_account_start: 0,
                fixed_account_count: 1,
                item_account_start: 0,
                item_account_count: 0,
                fixed_request: b"REAL_REQ",
                item_request: &[],
            },
            RouteInputV3 {
                role: FixedRole::Claims,
                kind: RouteKindV3::Once,
                enable_common_scalar: None,
                witness_range_common_scalar: None,
                receipt_dependency: None,
                fixed_account_start: 0,
                fixed_account_count: 1,
                item_account_start: 0,
                item_account_count: 0,
                fixed_request: b"CLAI_REQ",
                item_request: &[],
            },
        ];
        let dependencies = [
            RouteReceiptDependencyV3::new(FixedRole::Custody, 0, 320),
            RouteReceiptDependencyV3::new(FixedRole::Custody, 1, 320),
        ];
        let lists: [&[RouteReceiptDependencyV3]; 3] = [&[], &[], &dependencies];
        let width = HEADER_BYTES
            + routes.len() * ROUTE_BYTES
            + dependencies.len() * RECEIPT_DEPENDENCY_BYTES
            + routes
                .iter()
                .map(|route| route.fixed_request.len())
                .sum::<usize>();
        let mut scratch = std::vec![0_u8; width];
        let mut output = std::vec![0_u8; width];
        encode_effect_program_v4_atomic(
            EffectGeometryV3 {
                fixed_accounts: 1,
                item_account_stride: 0,
                common_scalars: 1,
                item_scalar_stride: 0,
                common_identities: 0,
                item_identity_stride: 0,
            },
            &routes,
            &lists,
            &[],
            &[],
            &mut scratch,
            &mut output,
        )
        .expect("plural program");
        let program = ProgramV3::decode(&output).expect("decoded plural program");
        assert_eq!(program.receipt_dependency_count(), 2);
        assert_eq!(
            program
                .route(2)
                .expect("consumer")
                .receipt_dependency_count(),
            2
        );
        assert_eq!(
            program.route(2).expect("consumer").receipt_dependency(),
            None
        );
        assert_eq!(program.route_receipt_dependency(2, 0), Ok(dependencies[0]));
        assert_eq!(program.route_receipt_dependency(2, 1), Ok(dependencies[1]));
        let resolved = program
            .resolved_invocation(2, 0, 0, &[1], &[])
            .expect("resolved consumer");
        assert_eq!(resolved.receipt_dependencies.len(), 2);
        assert_eq!(resolved.receipt_dependencies.expected_receipt_bytes(), 640);
    }
}
