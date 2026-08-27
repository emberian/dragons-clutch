//! Exact logical AccountProfile for terminal Bearer redemption.

use dclutch_account_profile_contract::v2::{
    AccountPrestateV2, DYNAMIC_FIXED_SPAN_HEADER_BYTES, OPERATION_BYTES as ACCOUNT_OPERATION_BYTES,
    RULE_BYTES as ACCOUNT_RULE_BYTES, TrustedBuiltinIdentityV2, TrustedEnvironmentV2,
    TrustedIdentityEnvironmentV2,
    encode::{
        AccountAliasInputV2, AccountCoordinateV2, AccountEffectPermissionsV2,
        AccountOperationInputV2, AccountPrivilegesV2, AccountRuleInputV2,
        AccountRuleWithPrestateInputV2, RegisterGeometryV2, ScalarCoordinateV2,
        encode_account_profile_with_dynamic_fixed_span_v2_atomic,
    },
};
use dclutch_product_payoff_v2_codec::runtime_v3::{
    BASIS_HEADER_BYTES_V3, BASIS_WIDTH_OFFSET_V3, ProductBasisV3,
};
use dclutch_rational_representation_v2_contract::{
    RATIONAL_TERMINAL_HOT_COMMON_IDENTITIES_V3, RATIONAL_TERMINAL_HOT_COMMON_SCALARS_V3,
    RATIONAL_TERMINAL_SCALAR_PRODUCT_OUTCOME_COUNT_V3,
};
use dclutch_token_svm::TOKEN_BEHAVIOR_SELECTION_BYTES_V2;

use crate::{Error, RATIONAL_TERMINAL_LOGICAL_ACCOUNT_COUNT_V3, Result};

const PROFILE_OPERATION_COUNT: usize = 1;
const LOGICAL_ACCOUNT_COUNT: usize = RATIONAL_TERMINAL_LOGICAL_ACCOUNT_COUNT_V3 as usize;

/// Exact encoded AccountProfile width for terminal Bearer redemption.
pub const RATIONAL_TERMINAL_ACCOUNT_PROFILE_BYTES_V3: usize = DYNAMIC_FIXED_SPAN_HEADER_BYTES
    + LOGICAL_ACCOUNT_COUNT * ACCOUNT_RULE_BYTES
    + PROFILE_OPERATION_COUNT * ACCOUNT_OPERATION_BYTES;

/// Exact observed data lengths and authenticated Product basis for one profile.
///
/// ProgramData and mutable state widths are deployment facts, so they are
/// supplied from one finalized chain snapshot rather than compiled into the
/// semantic operator. The profile itself pins every supplied width exactly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalTerminalAccountProfileInputV3<'a> {
    /// Exact logical-54 account data lengths in AccountProfile order.
    pub logical_data_lengths: &'a [u32],
    /// Exact finalized ProductBasisV3 bytes at logical coordinate four.
    pub product_basis: &'a [u8],
}

/// Encode the exact logical-54 AccountProfile for one finalized deployment.
pub fn encode_rational_terminal_account_profile_v3(
    input: RationalTerminalAccountProfileInputV3<'_>,
) -> Result<[u8; RATIONAL_TERMINAL_ACCOUNT_PROFILE_BYTES_V3]> {
    if input.logical_data_lengths.len() != LOGICAL_ACCOUNT_COUNT {
        return Err(Error::AccountProfileInput);
    }
    let basis =
        ProductBasisV3::decode(input.product_basis).map_err(|_| Error::AccountProfileInput)?;
    if basis.basis_width() == 0
        || input
            .logical_data_lengths
            .get(4)
            .copied()
            .and_then(|width| usize::try_from(width).ok())
            != Some(input.product_basis.len())
    {
        return Err(Error::AccountProfileInput);
    }
    let mut rules = Vec::with_capacity(LOGICAL_ACCOUNT_COUNT);
    for index in 0..LOGICAL_ACCOUNT_COUNT {
        rules.push(rule(index, input.logical_data_lengths)?);
    }
    let operations = [AccountOperationInputV2::ProjectTailCountU32 {
        account: AccountCoordinateV2::fixed(4),
        destination: ScalarCoordinateV2::common(
            u16::try_from(RATIONAL_TERMINAL_SCALAR_PRODUCT_OUTCOME_COUNT_V3)
                .map_err(|_| Error::AccountProfileInput)?,
        ),
        data_offset: u32::try_from(BASIS_WIDTH_OFFSET_V3)
            .map_err(|_| Error::AccountProfileInput)?,
    }];
    let geometry = RegisterGeometryV2 {
        common_scalars: u16::try_from(RATIONAL_TERMINAL_HOT_COMMON_SCALARS_V3)
            .map_err(|_| Error::AccountProfileInput)?,
        item_scalar_stride: 0,
        common_identities: u16::try_from(RATIONAL_TERMINAL_HOT_COMMON_IDENTITIES_V3)
            .map_err(|_| Error::AccountProfileInput)?,
        item_identity_stride: 0,
    };
    let mut scratch = [0_u8; RATIONAL_TERMINAL_ACCOUNT_PROFILE_BYTES_V3];
    let mut output = [0_u8; RATIONAL_TERMINAL_ACCOUNT_PROFILE_BYTES_V3];
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
    .map_err(Error::AccountProfileArtifact)?;
    Ok(output)
}

fn rule(index: usize, lengths: &[u32]) -> Result<AccountRuleWithPrestateInputV2> {
    let writable = matches!(index, 0 | 16 | 17 | 37 | 38 | 39 | 48 | 50 | 51);
    let signer = index == 8;
    let executable = matches!(index, 6 | 15 | 19 | 21 | 23 | 26 | 27 | 28 | 42 | 53);
    let alias = match index {
        // Claims program placeholders for absent receipt/Position accounts.
        26 | 28 => AccountAliasInputV2::Fixed(19),
        // Child frame reuses the Hot-injected Product basis/Product/portfolio.
        29 => AccountAliasInputV2::Fixed(4),
        31 => AccountAliasInputV2::Fixed(2),
        35 => AccountAliasInputV2::Fixed(3),
        // Terminal suffix reuses the already selected Token program.
        53 => AccountAliasInputV2::Fixed(27),
        _ => AccountAliasInputV2::SelfCoordinate,
    };
    let opaque = matches!(index, 6 | 7 | 19 | 20 | 21 | 23 | 24 | 25 | 27 | 38..=40 | 42..=52);
    let prestate = if index == 4 {
        AccountPrestateV2::AdapterAuthenticatedVariableData
    } else if alias != AccountAliasInputV2::SelfCoordinate {
        AccountPrestateV2::AuthenticatedRouteAlias
    } else if opaque {
        AccountPrestateV2::AuthenticatedOpaqueReadonlyData
    } else {
        AccountPrestateV2::Exact
    };
    let data_length = match index {
        1 => u32::try_from(TOKEN_BEHAVIOR_SELECTION_BYTES_V2)
            .map_err(|_| Error::AccountProfileInput)?,
        4 => u32::try_from(BASIS_HEADER_BYTES_V3).map_err(|_| Error::AccountProfileInput)?,
        26 | 28 | 29 | 31 | 35 | 53 => 0,
        _ if opaque => 0,
        _ => *lengths.get(index).ok_or(Error::AccountProfileInput)?,
    };
    Ok(AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges: AccountPrivilegesV2::new(signer, writable, executable),
            effect_permissions: AccountEffectPermissionsV2::new(false, false, false),
            alias,
            data_length,
            data_item_stride: 0,
        },
        prestate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_account_profile_contract::{
        AccountObservationV1,
        v2::{AccountProfileV2, ProjectionRegistersV2, project_tail_count_atomic},
    };
    use dclutch_product_payoff_v2_codec::runtime_v3::{
        BASIS_HEADER_BYTES_V3, BasisInputV3, BasisKindV3, compile_basis_v3,
    };

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn basis(width: u32) -> [u8; BASIS_HEADER_BYTES_V3] {
        let mut output = [0_u8; BASIS_HEADER_BYTES_V3];
        compile_basis_v3(
            BasisInputV3 {
                kind: BasisKindV3::CategoricalQ1,
                product_id: id(1),
                result_domain_id: id(2),
                coordinate_domain_id: id(3),
                result_unit_id: id(4),
                evaluator_release_id: id(5),
                basis_width: width,
                payout_scale: 1,
                knot_denominator: 1,
                knots: &[],
                terms: &[],
                failure_payouts: &[],
            },
            &mut output,
        )
        .expect("basis");
        output
    }

    #[test]
    fn profile_projects_product_basis_width_and_exact_aliases() {
        let basis = basis(258);
        let mut lengths = [0_u32; LOGICAL_ACCOUNT_COUNT];
        *lengths.get_mut(1).expect("Token selection coordinate") =
            u32::try_from(TOKEN_BEHAVIOR_SELECTION_BYTES_V2).expect("Token selection width");
        let basis_length = u32::try_from(basis.len()).expect("basis width");
        *lengths.get_mut(4).expect("basis logical coordinate") = basis_length;
        *lengths.get_mut(29).expect("basis child coordinate") = basis_length;
        let bytes =
            encode_rational_terminal_account_profile_v3(RationalTerminalAccountProfileInputV3 {
                logical_data_lengths: &lengths,
                product_basis: &basis,
            })
            .expect("profile");
        let profile = AccountProfileV2::decode(&bytes).expect("decode profile");
        assert_eq!(profile.fixed_account_count(), 54);
        assert_eq!(profile.item_account_stride(), 0);

        let mut data = (0..LOGICAL_ACCOUNT_COUNT)
            .map(|index| {
                vec![
                    0_u8;
                    usize::try_from(*lengths.get(index).expect("logical length")).expect("length")
                ]
            })
            .collect::<Vec<_>>();
        data.get_mut(4).expect("basis data").copy_from_slice(&basis);
        data.get_mut(29)
            .expect("basis child data")
            .copy_from_slice(&basis);
        let mut keys = (0..LOGICAL_ACCOUNT_COUNT)
            .map(|index| id(u8::try_from(index + 40).expect("key")))
            .collect::<Vec<_>>();
        for (target, source) in [(26, 19), (28, 19), (29, 4), (31, 2), (35, 3), (53, 27)] {
            let source_key = *keys.get(source).expect("alias source");
            *keys.get_mut(target).expect("alias target") = source_key;
        }
        let common_owner = id(200);
        let observations = (0..LOGICAL_ACCOUNT_COUNT)
            .map(|index| {
                let arguments = (
                    keys.get(index).expect("logical key"),
                    &common_owner,
                    0,
                    data.get(index).expect("logical data").as_slice(),
                    index == 8,
                    matches!(index, 0 | 16 | 17 | 37 | 38 | 39 | 48 | 50 | 51),
                    matches!(index, 6 | 15 | 19 | 21 | 23 | 26 | 27 | 28 | 42 | 53),
                );
                if index == 4 {
                    AccountObservationV1::new_adapter_authenticated_variable_data(
                        arguments.0,
                        arguments.1,
                        arguments.2,
                        arguments.3,
                        arguments.4,
                        arguments.5,
                        arguments.6,
                    )
                } else {
                    AccountObservationV1::new(
                        arguments.0,
                        arguments.1,
                        arguments.2,
                        arguments.3,
                        arguments.4,
                        arguments.5,
                        arguments.6,
                    )
                }
            })
            .collect::<Vec<_>>();
        let input_scalars = [0_u64; RATIONAL_TERMINAL_HOT_COMMON_SCALARS_V3];
        let input_identities = [[0_u8; 32]; RATIONAL_TERMINAL_HOT_COMMON_IDENTITIES_V3];
        let mut scratch_scalars = input_scalars;
        let mut scratch_identities = input_identities;
        let mut output_scalars = input_scalars;
        let mut output_identities = input_identities;
        let projected = project_tail_count_atomic(
            profile,
            &observations,
            ProjectionRegistersV2 {
                input_scalars: &input_scalars,
                input_identities: &input_identities,
                scratch_scalars: &mut scratch_scalars,
                scratch_identities: &mut scratch_identities,
                output_scalars: &mut output_scalars,
                output_identities: &mut output_identities,
            },
        )
        .expect("tail count");
        assert_eq!(projected, 258);
    }

    #[test]
    fn profile_refuses_nonbasis_bytes_and_unmatched_basis_length() {
        let basis = basis(258);
        let mut lengths = [0_u32; LOGICAL_ACCOUNT_COUNT];
        *lengths.get_mut(1).expect("Token selection coordinate") =
            u32::try_from(TOKEN_BEHAVIOR_SELECTION_BYTES_V2).expect("Token selection width");
        *lengths.get_mut(4).expect("basis logical coordinate") =
            u32::try_from(basis.len()).expect("basis length") - 1;
        assert_eq!(
            encode_rational_terminal_account_profile_v3(RationalTerminalAccountProfileInputV3 {
                logical_data_lengths: &lengths,
                product_basis: &basis,
            }),
            Err(Error::AccountProfileInput)
        );
        *lengths.get_mut(4).expect("basis logical coordinate") += 1;
        let mut hostile = basis;
        *hostile.get_mut(0).expect("basis magic") ^= 1;
        assert_eq!(
            encode_rational_terminal_account_profile_v3(RationalTerminalAccountProfileInputV3 {
                logical_data_lengths: &lengths,
                product_basis: &hostile,
            }),
            Err(Error::AccountProfileInput)
        );
    }
}
