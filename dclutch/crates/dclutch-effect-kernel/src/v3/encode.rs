//! Safe, allocation-free EffectProgram V3 artifact encoder.
//!
//! Typed constructors retain opcode and mode authority in the effect kernel.
//! The encoder builds into caller scratch, hostile-decodes the complete
//! candidate, and copies to output only after every route, operation, template,
//! and overlap check succeeds.

use super::{
    Error, FixedRole, HEADER_BYTES, MAGIC, MODE_ACCOUNT_A_ITEM, MODE_ACCOUNT_B_ITEM,
    MODE_REGISTER_ITEM, MODE_REQUEST_ITEM, OP_REQUIRE_LAMPORTS_EQ, OP_TRANSFER_LAMPORTS,
    OP_WRITE_DATA_U8, OP_WRITE_DATA_U8_AFFINE, OP_WRITE_DATA_U16, OP_WRITE_DATA_U16_AFFINE,
    OP_WRITE_DATA_U32, OP_WRITE_DATA_U32_AFFINE, OP_WRITE_IDENTITY, OP_WRITE_IDENTITY_AFFINE,
    OP_WRITE_REQUEST_IDENTITY, OP_WRITE_REQUEST_U8, OP_WRITE_REQUEST_U16, OP_WRITE_REQUEST_U32,
    OP_WRITE_REQUEST_U64, OP_WRITE_SCALAR, OP_WRITE_SCALAR_AFFINE, OPERATION_BYTES, ProgramV3,
    RECEIPT_DEPENDENCY_BYTES, ROUTE_BYTES, RouteKindV3, RouteReceiptDependencyV3, VERSION,
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

    /// Write one scalar narrowed to `u8` account data.
    pub const fn write_u8(
        account: AccountCoordinateV3,
        offset: u32,
        value: ScalarCoordinateV3,
    ) -> Self {
        Self::scalar(OP_WRITE_DATA_U8, account, None, value, offset, 0)
    }

    /// Write one scalar narrowed to little-endian `u16` account data.
    pub const fn write_u16(
        account: AccountCoordinateV3,
        offset: u32,
        value: ScalarCoordinateV3,
    ) -> Self {
        Self::scalar(OP_WRITE_DATA_U16, account, None, value, offset, 0)
    }

    /// Write one scalar narrowed to little-endian `u32` account data.
    pub const fn write_u32(
        account: AccountCoordinateV3,
        offset: u32,
        value: ScalarCoordinateV3,
    ) -> Self {
        Self::scalar(OP_WRITE_DATA_U32, account, None, value, offset, 0)
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
            scalar: None,
            identity: Some(identity),
            data_offset,
            extra,
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
    if instruction
        .account_b
        .is_some_and(|account| account.space == CoordinateSpaceV3::Item)
    {
        mode |= MODE_ACCOUNT_B_ITEM;
    }
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
        (4, instruction.account_b.map_or(0, |account| account.index)),
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
    extern crate std;

    use super::*;

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
