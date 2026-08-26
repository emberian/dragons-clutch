//! Exact logical AccountProfile for one lifecycle action/support geometry.

use dclutch_account_profile_contract::v2::{
    HEADER_BYTES as ACCOUNT_HEADER_BYTES, OPERATION_BYTES as ACCOUNT_OPERATION_BYTES,
    RULE_BYTES as ACCOUNT_RULE_BYTES,
    encode::{
        AccountAliasInputV2, AccountCoordinateV2, AccountEffectPermissionsV2,
        AccountOperationInputV2, AccountPrivilegesV2, AccountProfileArtifactV2, AccountRuleInputV2,
        RegisterGeometryV2, ScalarCoordinateV2, encode_account_profile_v2_atomic,
    },
};
use dclutch_product_payoff_v2_codec::runtime_v3::{BASIS_WIDTH_OFFSET_V3, ProductBasisV3};
use dclutch_rational_representation_v2_lifecycle_contract::{
    LifecycleActionV2,
    hot_v3::{
        RATIONAL_LIFECYCLE_SCALAR_PRODUCT_OUTCOME_COUNT_V3, RationalLifecycleHotRegisterLayoutV3,
    },
};

use crate::{Error, Result, lifecycle_logical_account_count_v3, validate_action_geometry};

const PROFILE_OPERATION_COUNT: usize = 1;

/// Exact account observations needed to emit one descriptor-specific profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalLifecycleAccountProfileInputV3<'a> {
    /// Exact logical account data lengths in injected-prefix plus child order.
    pub logical_data_lengths: &'a [u32],
    /// Exact finalized ProductBasisV3 bytes at injected logical coordinate four.
    pub product_basis: &'a [u8],
}

/// Encode one exact lifecycle AccountProfile.
pub fn encode_rational_lifecycle_account_profile_v3(
    action: LifecycleActionV2,
    coordinate_count: u32,
    input: RationalLifecycleAccountProfileInputV3<'_>,
) -> Result<Vec<u8>> {
    let coordinates = validate_action_geometry(action, coordinate_count)?;
    let logical_count = usize::from(lifecycle_logical_account_count_v3(
        action,
        coordinate_count,
    )?);
    if input.logical_data_lengths.len() != logical_count {
        return Err(Error::AccountObservation);
    }
    let basis =
        ProductBasisV3::decode(input.product_basis).map_err(|_| Error::AccountObservation)?;
    if basis.basis_width() < 2
        || input
            .logical_data_lengths
            .get(4)
            .copied()
            .and_then(|width| usize::try_from(width).ok())
            != Some(input.product_basis.len())
    {
        return Err(Error::AccountObservation);
    }
    let mut rules = Vec::with_capacity(logical_count);
    for index in 0..logical_count {
        rules.push(rule(action, index, input.logical_data_lengths)?);
    }
    let operation = [AccountOperationInputV2::ProjectTailCountU32 {
        account: AccountCoordinateV2::fixed(4),
        destination: ScalarCoordinateV2::common(narrow_u16(
            RATIONAL_LIFECYCLE_SCALAR_PRODUCT_OUTCOME_COUNT_V3,
        )?),
        data_offset: narrow_u32(BASIS_WIDTH_OFFSET_V3)?,
    }];
    let registers = RationalLifecycleHotRegisterLayoutV3::new(coordinates);
    let geometry = RegisterGeometryV2 {
        common_scalars: narrow_u16(registers.scalar_count().ok_or(Error::InvalidLength)?)?,
        item_scalar_stride: 0,
        common_identities: narrow_u16(registers.identity_count().ok_or(Error::InvalidLength)?)?,
        item_identity_stride: 0,
    };
    let bytes = ACCOUNT_HEADER_BYTES
        .checked_add(
            logical_count
                .checked_mul(ACCOUNT_RULE_BYTES)
                .ok_or(Error::InvalidLength)?,
        )
        .and_then(|value| {
            PROFILE_OPERATION_COUNT
                .checked_mul(ACCOUNT_OPERATION_BYTES)
                .and_then(|operations| value.checked_add(operations))
        })
        .ok_or(Error::InvalidLength)?;
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_account_profile_v2_atomic(
        AccountProfileArtifactV2::TypedScalar,
        &rules,
        &[],
        &operation,
        &[],
        geometry,
        &mut scratch,
        &mut output,
    )
    .map_err(Error::AccountProfile)?;
    Ok(output)
}

fn rule(action: LifecycleActionV2, index: usize, lengths: &[u32]) -> Result<AccountRuleInputV2> {
    let data_length = *lengths.get(index).ok_or(Error::AccountObservation)?;
    let receipt_writable = matches!(
        action,
        LifecycleActionV2::ActivateReceipt | LifecycleActionV2::RetireReceipt
    ) && index == 17;
    let retirement_credit = action.retires() && index == 19;
    let coordinate_writable = matches!(
        action,
        LifecycleActionV2::ActivateCoordinate | LifecycleActionV2::RetireCoordinate
    ) && matches!(index, 26..=29);
    let writable = index == 0 || receipt_writable || retirement_credit || coordinate_writable;
    let executable = matches!(index, 6 | 8 | 10 | 13 | 18 | 20 | 23);
    let alias = match index {
        // Claims descriptor raw is the selected immutable capability config.
        14 => AccountAliasInputV2::Fixed(1),
        // Coordinate actions reuse the Hot-authenticated Product graph roots.
        31 => AccountAliasInputV2::Fixed(4),
        33 => AccountAliasInputV2::Fixed(2),
        37 => AccountAliasInputV2::Fixed(3),
        _ => AccountAliasInputV2::SelfCoordinate,
    };
    Ok(AccountRuleInputV2 {
        privileges: AccountPrivilegesV2::new(false, writable, executable),
        effect_permissions: AccountEffectPermissionsV2::new(false, false, false),
        alias,
        data_length,
        data_item_stride: 0,
    })
}

fn narrow_u16(value: usize) -> Result<u16> {
    u16::try_from(value).map_err(|_| Error::InvalidLength)
}

fn narrow_u32(value: usize) -> Result<u32> {
    u32::try_from(value).map_err(|_| Error::InvalidLength)
}
