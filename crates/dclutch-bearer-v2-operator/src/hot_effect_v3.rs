//! Generic Hot EffectProgram specialization for terminal Bearer redemption.

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
    v4::{
        BorrowedRangePolicyV4, HEADER_BYTES_V4 as EFFECT_V4_HEADER_BYTES, encode_program_v4_atomic,
    },
};
use dclutch_rational_representation_v2_contract::{
    CallerRoleV2, PHYSICAL_ABI_VERSION_V2, RATIONAL_TERMINAL_HOT_ACTION_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_ACTOR_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_ASSET_ACTOR_SHARD_ACCOUNT_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_ASSET_CLAIMS_CUSTODY_OWNER_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_ASSET_COEFFICIENT_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_ASSET_EXPECTED_ACTOR_SHARDS_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_ASSET_EXPECTED_SHARD_SUPPLY_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_ASSET_EXPECTED_STRUCTURED_SHARDS_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_ASSET_SHARD_MINT_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_ASSET_STRUCTURED_CUSTODY_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_CALLER_ROLE_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_COLLATERAL_RECIPIENT_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_COMMON_IDENTITIES_V3, RATIONAL_TERMINAL_HOT_COMMON_SCALARS_V3,
    RATIONAL_TERMINAL_HOT_DENOMINATOR_OFFSET_V3, RATIONAL_TERMINAL_HOT_DESCRIPTOR_ID_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_EXPECTED_ACTOR_POSITION_REVISION_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_EXPECTED_CLAIMS_MARKET_REVISION_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_EXPECTED_CUSTODY_POSITION_REVISION_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_EXPECTED_CUSTODY_REPLAY_REVISION_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_EXPECTED_RECEIPT_SUPPLY_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_EXPECTED_REPRESENTATION_REVISION_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_GENERATION_OFFSET_V3, RATIONAL_TERMINAL_HOT_GRAPH_ID_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_MARKET_OFFSET_V3, RATIONAL_TERMINAL_HOT_OUTCOME_COUNT_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_PARENT_CONTEXT_OFFSET_V3, RATIONAL_TERMINAL_HOT_QUANTITY_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_REALM_OFFSET_V3, RATIONAL_TERMINAL_HOT_RECEIPT_MINT_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_RELEASE_SET_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_REPRESENTATION_AUTHORITY_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3, RATIONAL_TERMINAL_HOT_SELECTED_OUTCOME_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_TOKEN_PROGRAM_OFFSET_V3,
    RATIONAL_TERMINAL_IDENTITY_ACTOR_SHARD_ACCOUNT_V3, RATIONAL_TERMINAL_IDENTITY_ACTOR_V3,
    RATIONAL_TERMINAL_IDENTITY_CLAIMS_CUSTODY_OWNER_V3,
    RATIONAL_TERMINAL_IDENTITY_COLLATERAL_RECIPIENT_V3, RATIONAL_TERMINAL_IDENTITY_DESCRIPTOR_V3,
    RATIONAL_TERMINAL_IDENTITY_GRAPH_V3, RATIONAL_TERMINAL_IDENTITY_MARKET_V3,
    RATIONAL_TERMINAL_IDENTITY_PARENT_DIGEST_V3, RATIONAL_TERMINAL_IDENTITY_REALM_V3,
    RATIONAL_TERMINAL_IDENTITY_RECEIPT_MINT_V3, RATIONAL_TERMINAL_IDENTITY_RELEASE_SET_V3,
    RATIONAL_TERMINAL_IDENTITY_REPRESENTATION_AUTHORITY_V3,
    RATIONAL_TERMINAL_IDENTITY_SHARD_MINT_V3, RATIONAL_TERMINAL_IDENTITY_STRUCTURED_CUSTODY_V3,
    RATIONAL_TERMINAL_IDENTITY_TOKEN_PROGRAM_V3,
    RATIONAL_TERMINAL_SCALAR_ACTOR_POSITION_REVISION_V3, RATIONAL_TERMINAL_SCALAR_ACTOR_SHARDS_V3,
    RATIONAL_TERMINAL_SCALAR_ASSET_COUNT_V3, RATIONAL_TERMINAL_SCALAR_CLAIMS_MARKET_REVISION_V3,
    RATIONAL_TERMINAL_SCALAR_COEFFICIENT_V3, RATIONAL_TERMINAL_SCALAR_CUSTODY_POSITION_REVISION_V3,
    RATIONAL_TERMINAL_SCALAR_CUSTODY_REPLAY_REVISION_V3, RATIONAL_TERMINAL_SCALAR_DENOMINATOR_V3,
    RATIONAL_TERMINAL_SCALAR_GENERATION_V3, RATIONAL_TERMINAL_SCALAR_OUTCOME_COUNT_V3,
    RATIONAL_TERMINAL_SCALAR_QUANTITY_V3, RATIONAL_TERMINAL_SCALAR_RECEIPT_SUPPLY_V3,
    RATIONAL_TERMINAL_SCALAR_REPRESENTATION_REVISION_V3,
    RATIONAL_TERMINAL_SCALAR_SELECTED_OUTCOME_V3, RATIONAL_TERMINAL_SCALAR_SHARD_SUPPLY_V3,
    RATIONAL_TERMINAL_SCALAR_STRUCTURED_SHARDS_V3, REQUEST_MAGIC_V2, RepresentationActionV2,
};

use crate::{Error, Result};

/// Logical accounts injected by the common Hot outer before the family suffix:
/// root, config, Product record, portfolio record, and linked Product basis.
pub const RATIONAL_TERMINAL_HOT_INJECTED_ACCOUNT_COUNT_V3: u16 = 5;
/// Exact Rational terminal Claims child account frame.
pub const RATIONAL_TERMINAL_CLAIMS_ACCOUNT_COUNT_V3: u16 = 49;
/// Exact logical AccountProfile/EffectProgram account width.
pub const RATIONAL_TERMINAL_LOGICAL_ACCOUNT_COUNT_V3: u16 =
    RATIONAL_TERMINAL_HOT_INJECTED_ACCOUNT_COUNT_V3 + RATIONAL_TERMINAL_CLAIMS_ACCOUNT_COUNT_V3;

const EFFECT_ROUTE_COUNT: usize = 1;
const EFFECT_INSTRUCTION_COUNT: usize = 31;

/// Exact encoded EffectProgram width for terminal Rational redemption.
const RATIONAL_TERMINAL_EFFECT_BASE_BYTES_V3: usize = EFFECT_HEADER_BYTES
    + EFFECT_ROUTE_COUNT * EFFECT_ROUTE_BYTES
    + EFFECT_INSTRUCTION_COUNT * EFFECT_OPERATION_BYTES
    + RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3;
/// Exact encoded EffectProgram V4 width for terminal Rational redemption.
pub const RATIONAL_TERMINAL_EFFECT_BYTES_V3: usize =
    EFFECT_V4_HEADER_BYTES + RATIONAL_TERMINAL_EFFECT_BASE_BYTES_V3;

/// Encode the one-route Claims program selected for terminal redemption.
///
/// The finalized artifact owns only the child wire skeleton and register
/// translation. The selected Claims program remains the sole mutation and
/// receipt authority. There is deliberately no prior-receipt dependency:
/// terminal Custody execution is internal to the authenticated Claims child.
pub fn encode_rational_terminal_effect_v3() -> Result<[u8; RATIONAL_TERMINAL_EFFECT_BYTES_V3]> {
    let template = child_template()?;
    let routes = [RouteInputV3 {
        role: FixedRole::Claims,
        kind: RouteKindV3::Once,
        enable_common_scalar: None,
        witness_range_common_scalar: None,
        receipt_dependency: None,
        fixed_account_start: RATIONAL_TERMINAL_HOT_INJECTED_ACCOUNT_COUNT_V3,
        fixed_account_count: RATIONAL_TERMINAL_CLAIMS_ACCOUNT_COUNT_V3,
        item_account_start: 0,
        item_account_count: 0,
        fixed_request: &template,
        item_request: &[],
    }];
    let instructions = request_instructions()?;
    let geometry = EffectGeometryV3 {
        fixed_accounts: RATIONAL_TERMINAL_LOGICAL_ACCOUNT_COUNT_V3,
        item_account_stride: 0,
        common_scalars: u16::try_from(RATIONAL_TERMINAL_HOT_COMMON_SCALARS_V3)
            .map_err(|_| Error::EffectArtifact(dclutch_effect_kernel::v3::Error::InvalidLength))?,
        item_scalar_stride: 0,
        common_identities: u16::try_from(RATIONAL_TERMINAL_HOT_COMMON_IDENTITIES_V3)
            .map_err(|_| Error::EffectArtifact(dclutch_effect_kernel::v3::Error::InvalidLength))?,
        item_identity_stride: 0,
    };
    let mut scratch = [0_u8; RATIONAL_TERMINAL_EFFECT_BASE_BYTES_V3];
    let mut base = [0_u8; RATIONAL_TERMINAL_EFFECT_BASE_BYTES_V3];
    encode_effect_program_v3_atomic(
        geometry,
        &routes,
        &instructions,
        &[],
        &mut scratch,
        &mut base,
    )
    .map_err(Error::EffectArtifact)?;
    let mut scratch = [0_u8; RATIONAL_TERMINAL_EFFECT_BYTES_V3];
    let mut output = [0_u8; RATIONAL_TERMINAL_EFFECT_BYTES_V3];
    encode_program_v4_atomic(
        &base,
        BorrowedRangePolicyV4::DisjointExactCoverage,
        u32::try_from(RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3)
            .map_err(|_| Error::ArtifactGeometry)?,
        &[],
        &[],
        &mut scratch,
        &mut output,
    )
    .map_err(Error::EffectArtifactV4)?;
    Ok(output)
}

fn child_template() -> Result<[u8; RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3]> {
    let mut output = [0_u8; RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3];
    put(&mut output, 0, &REQUEST_MAGIC_V2)?;
    put(&mut output, 8, &PHYSICAL_ABI_VERSION_V2.to_le_bytes())?;
    put(
        &mut output,
        RATIONAL_TERMINAL_HOT_ACTION_OFFSET_V3,
        &[RepresentationActionV2::RedeemTerminal as u8],
    )?;
    put(
        &mut output,
        RATIONAL_TERMINAL_HOT_CALLER_ROLE_OFFSET_V3,
        &[CallerRoleV2::Trading as u8],
    )?;
    Ok(output)
}

fn request_instructions() -> Result<[EffectInstructionV3; EFFECT_INSTRUCTION_COUNT]> {
    Ok([
        write_identity(RATIONAL_TERMINAL_HOT_RELEASE_SET_OFFSET_V3, RATIONAL_TERMINAL_IDENTITY_RELEASE_SET_V3)?,
        write_identity(RATIONAL_TERMINAL_HOT_MARKET_OFFSET_V3, RATIONAL_TERMINAL_IDENTITY_MARKET_V3)?,
        write_identity(RATIONAL_TERMINAL_HOT_GRAPH_ID_OFFSET_V3, RATIONAL_TERMINAL_IDENTITY_GRAPH_V3)?,
        write_identity(RATIONAL_TERMINAL_HOT_DESCRIPTOR_ID_OFFSET_V3, RATIONAL_TERMINAL_IDENTITY_DESCRIPTOR_V3)?,
        write_identity(RATIONAL_TERMINAL_HOT_PARENT_CONTEXT_OFFSET_V3, RATIONAL_TERMINAL_IDENTITY_PARENT_DIGEST_V3)?,
        write_identity(RATIONAL_TERMINAL_HOT_ACTOR_OFFSET_V3, RATIONAL_TERMINAL_IDENTITY_ACTOR_V3)?,
        write_identity(RATIONAL_TERMINAL_HOT_RECEIPT_MINT_OFFSET_V3, RATIONAL_TERMINAL_IDENTITY_RECEIPT_MINT_V3)?,
        write_identity(RATIONAL_TERMINAL_HOT_REPRESENTATION_AUTHORITY_OFFSET_V3, RATIONAL_TERMINAL_IDENTITY_REPRESENTATION_AUTHORITY_V3)?,
        write_identity(RATIONAL_TERMINAL_HOT_TOKEN_PROGRAM_OFFSET_V3, RATIONAL_TERMINAL_IDENTITY_TOKEN_PROGRAM_V3)?,
        write_identity(RATIONAL_TERMINAL_HOT_REALM_OFFSET_V3, RATIONAL_TERMINAL_IDENTITY_REALM_V3)?,
        write_identity(RATIONAL_TERMINAL_HOT_COLLATERAL_RECIPIENT_OFFSET_V3, RATIONAL_TERMINAL_IDENTITY_COLLATERAL_RECIPIENT_V3)?,
        write_identity(RATIONAL_TERMINAL_HOT_ASSET_SHARD_MINT_OFFSET_V3, RATIONAL_TERMINAL_IDENTITY_SHARD_MINT_V3)?,
        write_identity(RATIONAL_TERMINAL_HOT_ASSET_ACTOR_SHARD_ACCOUNT_OFFSET_V3, RATIONAL_TERMINAL_IDENTITY_ACTOR_SHARD_ACCOUNT_V3)?,
        write_identity(RATIONAL_TERMINAL_HOT_ASSET_STRUCTURED_CUSTODY_OFFSET_V3, RATIONAL_TERMINAL_IDENTITY_STRUCTURED_CUSTODY_V3)?,
        write_identity(RATIONAL_TERMINAL_HOT_ASSET_CLAIMS_CUSTODY_OWNER_OFFSET_V3, RATIONAL_TERMINAL_IDENTITY_CLAIMS_CUSTODY_OWNER_V3)?,
        write_u64(RATIONAL_TERMINAL_HOT_EXPECTED_REPRESENTATION_REVISION_OFFSET_V3, RATIONAL_TERMINAL_SCALAR_REPRESENTATION_REVISION_V3)?,
        write_u64(RATIONAL_TERMINAL_HOT_EXPECTED_CLAIMS_MARKET_REVISION_OFFSET_V3, RATIONAL_TERMINAL_SCALAR_CLAIMS_MARKET_REVISION_V3)?,
        write_u64(RATIONAL_TERMINAL_HOT_EXPECTED_ACTOR_POSITION_REVISION_OFFSET_V3, RATIONAL_TERMINAL_SCALAR_ACTOR_POSITION_REVISION_V3)?,
        write_u64(RATIONAL_TERMINAL_HOT_EXPECTED_CUSTODY_POSITION_REVISION_OFFSET_V3, RATIONAL_TERMINAL_SCALAR_CUSTODY_POSITION_REVISION_V3)?,
        write_u64(RATIONAL_TERMINAL_HOT_EXPECTED_CUSTODY_REPLAY_REVISION_OFFSET_V3, RATIONAL_TERMINAL_SCALAR_CUSTODY_REPLAY_REVISION_V3)?,
        write_u64(RATIONAL_TERMINAL_HOT_GENERATION_OFFSET_V3, RATIONAL_TERMINAL_SCALAR_GENERATION_V3)?,
        write_u64(RATIONAL_TERMINAL_HOT_QUANTITY_OFFSET_V3, RATIONAL_TERMINAL_SCALAR_QUANTITY_V3)?,
        write_u64(RATIONAL_TERMINAL_HOT_DENOMINATOR_OFFSET_V3, RATIONAL_TERMINAL_SCALAR_DENOMINATOR_V3)?,
        write_u64(RATIONAL_TERMINAL_HOT_EXPECTED_RECEIPT_SUPPLY_OFFSET_V3, RATIONAL_TERMINAL_SCALAR_RECEIPT_SUPPLY_V3)?,
        write_u32(RATIONAL_TERMINAL_HOT_OUTCOME_COUNT_OFFSET_V3, RATIONAL_TERMINAL_SCALAR_OUTCOME_COUNT_V3)?,
        write_u32(RATIONAL_TERMINAL_HOT_SELECTED_OUTCOME_OFFSET_V3, RATIONAL_TERMINAL_SCALAR_SELECTED_OUTCOME_V3)?,
        write_u32(dclutch_rational_representation_v2_contract::RATIONAL_TERMINAL_HOT_ASSET_COUNT_OFFSET_V3, RATIONAL_TERMINAL_SCALAR_ASSET_COUNT_V3)?,
        write_u64(RATIONAL_TERMINAL_HOT_ASSET_COEFFICIENT_OFFSET_V3, RATIONAL_TERMINAL_SCALAR_COEFFICIENT_V3)?,
        write_u64(RATIONAL_TERMINAL_HOT_ASSET_EXPECTED_SHARD_SUPPLY_OFFSET_V3, RATIONAL_TERMINAL_SCALAR_SHARD_SUPPLY_V3)?,
        write_u64(RATIONAL_TERMINAL_HOT_ASSET_EXPECTED_ACTOR_SHARDS_OFFSET_V3, RATIONAL_TERMINAL_SCALAR_ACTOR_SHARDS_V3)?,
        write_u64(RATIONAL_TERMINAL_HOT_ASSET_EXPECTED_STRUCTURED_SHARDS_OFFSET_V3, RATIONAL_TERMINAL_SCALAR_STRUCTURED_SHARDS_V3)?,
    ])
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

fn narrow_u16(value: usize) -> Result<u16> {
    u16::try_from(value)
        .map_err(|_| Error::EffectArtifact(dclutch_effect_kernel::v3::Error::InvalidLength))
}

fn narrow_u32(value: usize) -> Result<u32> {
    u32::try_from(value)
        .map_err(|_| Error::EffectArtifact(dclutch_effect_kernel::v3::Error::InvalidLength))
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(Error::EffectArtifact(
            dclutch_effect_kernel::v3::Error::InvalidLength,
        ))?;
    let destination = output.get_mut(offset..end).ok_or(Error::EffectArtifact(
        dclutch_effect_kernel::v3::Error::InvalidLength,
    ))?;
    destination.copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_effect_kernel::{
        v2::{AccountInput, AccountPermission},
        v3::{ProjectionV3, project_atomic},
        v4::ProgramV4,
    };
    use dclutch_rational_representation_v2_contract::{
        ABSENT_REVISION, RATIONAL_TERMINAL_SCALAR_PRODUCT_OUTCOME_COUNT_V3, RepresentationRequestV2,
    };
    use dclutch_token_svm::TOKEN_2022_PROGRAM_ID;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    #[test]
    fn effect_reconstructs_exact_terminal_claims_child() {
        let bytes = encode_rational_terminal_effect_v3().expect("effect");
        let successor = ProgramV4::decode(&bytes).expect("decode");
        assert_eq!(successor.span_count(), 0);
        assert_eq!(successor.range_count(), 0);
        assert_eq!(
            successor.semantic_prefix_bytes(),
            u32::try_from(RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3).expect("request width")
        );
        let program = successor.base();
        assert_eq!(program.route_count(), 1);
        assert_eq!(program.account_count(258).expect("accounts"), 54);
        let route = program.route(0).expect("route");
        assert_eq!(route.role(), FixedRole::Claims);
        assert_eq!(route.kind(), RouteKindV3::Once);
        assert_eq!(route.receipt_dependency(), None);

        let mut scalars = [1_u64; RATIONAL_TERMINAL_HOT_COMMON_SCALARS_V3];
        scalars[RATIONAL_TERMINAL_SCALAR_ACTOR_POSITION_REVISION_V3] = ABSENT_REVISION;
        scalars[RATIONAL_TERMINAL_SCALAR_OUTCOME_COUNT_V3] = 258;
        scalars[RATIONAL_TERMINAL_SCALAR_SELECTED_OUTCOME_V3] = 257;
        scalars[RATIONAL_TERMINAL_SCALAR_ASSET_COUNT_V3] = 1;
        scalars[RATIONAL_TERMINAL_SCALAR_DENOMINATOR_V3] = 10;
        scalars[RATIONAL_TERMINAL_SCALAR_COEFFICIENT_V3] = 10;
        scalars[RATIONAL_TERMINAL_SCALAR_PRODUCT_OUTCOME_COUNT_V3] = 258;
        scalars[RATIONAL_TERMINAL_SCALAR_STRUCTURED_SHARDS_V3] = 0;
        let mut identities = [[0_u8; 32]; RATIONAL_TERMINAL_HOT_COMMON_IDENTITIES_V3];
        for (index, identity) in identities.iter_mut().enumerate() {
            *identity = id(u8::try_from(index + 1).expect("small identity bank"));
        }
        identities[RATIONAL_TERMINAL_IDENTITY_TOKEN_PROGRAM_V3] = TOKEN_2022_PROGRAM_ID;
        let aliases: [usize; RATIONAL_TERMINAL_LOGICAL_ACCOUNT_COUNT_V3 as usize] =
            core::array::from_fn(|index| index);
        let accounts = [AccountInput {
            lamports: 0,
            data_len: 0,
        }; RATIONAL_TERMINAL_LOGICAL_ACCOUNT_COUNT_V3 as usize];
        let permissions =
            [AccountPermission::read_only(); RATIONAL_TERMINAL_LOGICAL_ACCOUNT_COUNT_V3 as usize];
        let mut scratch_lamports = [0_u64; RATIONAL_TERMINAL_LOGICAL_ACCOUNT_COUNT_V3 as usize];
        let mut output_lamports = scratch_lamports;
        let mut request = [0_u8; RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3];
        project_atomic(
            program,
            258,
            ProjectionV3 {
                scalars: &scalars,
                identities: &identities,
                aliases: &aliases,
                accounts: &accounts,
                permissions: &permissions,
                scratch_lamports: &mut scratch_lamports,
                output_lamports: &mut output_lamports,
                requests: &mut request,
            },
        )
        .expect("project");
        let request = RepresentationRequestV2::decode(&request).expect("Claims child");
        assert_eq!(request.header().parent_context, identities[0]);
        assert_eq!(request.header().market, identities[2]);
        assert_eq!(request.header().outcome_count, 258);
        assert_eq!(request.header().selected_outcome, 257);
        assert_eq!(request.asset(0).expect("asset").coefficient, 10);
    }
}
