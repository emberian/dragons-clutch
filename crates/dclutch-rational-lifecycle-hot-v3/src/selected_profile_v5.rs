//! Profile13 account observations for fixed-cardinality lifecycle actions.

use dclutch_account_profile_contract::v2::{
    AccountPrestateV2, DYNAMIC_FIXED_SPAN_HEADER_BYTES, OPERATION_BYTES as ACCOUNT_OPERATION_BYTES,
    RULE_BYTES as ACCOUNT_RULE_BYTES, TrustedBuiltinIdentityV2, TrustedEnvironmentV2,
    TrustedIdentityEnvironmentV2,
    encode::{
        AccountAliasInputV2, AccountCoordinateV2, AccountOperationInputV2,
        AccountRuleWithPrestateInputV2, IdentityCoordinateV2, RegisterGeometryV2,
        ScalarCoordinateV2, encode_account_profile_with_dynamic_fixed_span_v2_atomic,
    },
};
use dclutch_product_payoff_v2_codec::runtime_v3::{BASIS_WIDTH_OFFSET_V3, ProductBasisV3};
use dclutch_rational_representation_v2_kernel::DESCRIPTOR_HEADER_BYTES;
use dclutch_rational_representation_v2_lifecycle_contract::{
    LifecycleActionV2,
    hot_v3::{
        RATIONAL_LIFECYCLE_IDENTITY_DESCRIPTOR_V3,
        RATIONAL_LIFECYCLE_SCALAR_PRODUCT_OUTCOME_COUNT_V3, RationalLifecycleHotRegisterLayoutV3,
    },
};
use dclutch_token_svm::TOKEN_BEHAVIOR_SELECTION_BYTES_V2;

use crate::{
    Error, Result, account_profile::rule, lifecycle_logical_account_count_v3,
    validate_action_geometry,
};

/// Exact account observations used to emit one selected Profile13 artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalLifecycleSelectedAccountProfileInputV5<'a> {
    /// Exact logical data lengths in injected-prefix plus Claims order.
    pub logical_data_lengths: &'a [u32],
    /// Exact finalized ProductBasisV3 bytes authenticated at logical coordinate four.
    pub product_basis: &'a [u8],
}

/// Encode one Profile13 account interpreter for a fixed-cardinality action.
pub fn encode_rational_lifecycle_selected_account_profile_v5(
    action: LifecycleActionV2,
    input: RationalLifecycleSelectedAccountProfileInputV5<'_>,
) -> Result<Vec<u8>> {
    let coordinate_count = match action {
        LifecycleActionV2::ActivateReceipt => 0,
        LifecycleActionV2::ActivateCoordinate | LifecycleActionV2::RetireCoordinate => 1,
        LifecycleActionV2::RetireReceipt => return Err(Error::ActionGeometry),
    };
    let coordinates = validate_action_geometry(action, coordinate_count)?;
    let logical_count = usize::from(lifecycle_logical_account_count_v3(
        action,
        coordinate_count,
    )?);
    let basis =
        ProductBasisV3::decode(input.product_basis).map_err(|_| Error::AccountObservation)?;
    if basis.basis_width() < 2
        || input.logical_data_lengths.len() != logical_count
        || input.logical_data_lengths.get(1).copied()
            != u32::try_from(TOKEN_BEHAVIOR_SELECTION_BYTES_V2).ok()
        || input.logical_data_lengths.get(4).copied()
            != u32::try_from(input.product_basis.len()).ok()
        || input
            .logical_data_lengths
            .get(14)
            .copied()
            .unwrap_or_default()
            < u32::try_from(DESCRIPTOR_HEADER_BYTES).map_err(|_| Error::InvalidLength)?
    {
        return Err(Error::AccountObservation);
    }

    let mut rules = Vec::with_capacity(logical_count);
    for index in 0..logical_count {
        let mut value = rule(action, index, input.logical_data_lengths)?;
        let alias = match index {
            31 => Some(4),
            33 => Some(2),
            37 => Some(3),
            _ => None,
        };
        value.alias = alias.map_or(
            AccountAliasInputV2::SelfCoordinate,
            AccountAliasInputV2::Fixed,
        );
        let opaque = matches!(
            index,
            6 | 7 | 8 | 9 | 10 | 13 | 17 | 18 | 20 | 23 | 24 | 26 | 27
        );
        let prestate = match (index, alias, opaque) {
            (4, _, _) => {
                value.data_length = u32::try_from(
                    dclutch_product_payoff_v2_codec::runtime_v3::BASIS_HEADER_BYTES_V3,
                )
                .map_err(|_| Error::InvalidLength)?;
                AccountPrestateV2::AdapterAuthenticatedVariableData
            }
            (14, _, _) => {
                value.data_length =
                    u32::try_from(DESCRIPTOR_HEADER_BYTES).map_err(|_| Error::InvalidLength)?;
                AccountPrestateV2::AdapterAuthenticatedVariableData
            }
            (_, Some(_), _) => {
                value.data_length = 0;
                AccountPrestateV2::AuthenticatedRouteAlias
            }
            (_, _, true) => {
                value.data_length = 0;
                AccountPrestateV2::AuthenticatedOpaqueReadonlyData
            }
            _ => AccountPrestateV2::Exact,
        };
        rules.push(AccountRuleWithPrestateInputV2 {
            rule: value,
            prestate,
        });
    }
    let operations = [
        AccountOperationInputV2::ProjectTailCountU32 {
            account: AccountCoordinateV2::fixed(4),
            destination: ScalarCoordinateV2::common(narrow_u16(
                RATIONAL_LIFECYCLE_SCALAR_PRODUCT_OUTCOME_COUNT_V3,
            )?),
            data_offset: u32::try_from(BASIS_WIDTH_OFFSET_V3).map_err(|_| Error::InvalidLength)?,
        },
        AccountOperationInputV2::ProjectKey {
            account: AccountCoordinateV2::fixed(14),
            destination: IdentityCoordinateV2::common(narrow_u16(
                RATIONAL_LIFECYCLE_IDENTITY_DESCRIPTOR_V3,
            )?),
        },
    ];
    let registers = RationalLifecycleHotRegisterLayoutV3::new(coordinates);
    let geometry = RegisterGeometryV2 {
        common_scalars: narrow_u16(registers.scalar_count().ok_or(Error::InvalidLength)?)?,
        item_scalar_stride: 0,
        common_identities: narrow_u16(registers.identity_count().ok_or(Error::InvalidLength)?)?,
        item_identity_stride: 0,
    };
    let bytes = DYNAMIC_FIXED_SPAN_HEADER_BYTES
        .checked_add(
            rules
                .len()
                .checked_mul(ACCOUNT_RULE_BYTES)
                .ok_or(Error::InvalidLength)?,
        )
        .and_then(|prefix| {
            operations
                .len()
                .checked_mul(ACCOUNT_OPERATION_BYTES)
                .and_then(|operations| prefix.checked_add(operations))
        })
        .ok_or(Error::InvalidLength)?;
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_account_profile_with_dynamic_fixed_span_v2_atomic(
        TrustedEnvironmentV2::None,
        TrustedIdentityEnvironmentV2::None,
        TrustedBuiltinIdentityV2::None,
        &[],
        &rules,
        &[],
        &operations,
        geometry,
        &mut scratch,
        &mut output,
    )
    .map_err(Error::AccountProfile)?;
    Ok(output)
}

fn narrow_u16(value: usize) -> Result<u16> {
    u16::try_from(value).map_err(|_| Error::InvalidLength)
}
