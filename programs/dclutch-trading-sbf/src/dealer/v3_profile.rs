//! Exact logical AccountProfile for admitted Dealer junior-equity execution.
//!
//! Data widths are finalized deployment facts and therefore enter explicitly;
//! account order, privileges, aliases, trusted-slot projection, and Trading
//! ownership of both local write targets remain family-owned constants. Equity
//! never admits a vacant obligation or LP Position: both are exact live state.

#[cfg(not(target_os = "solana"))]
extern crate alloc;

#[cfg(not(target_os = "solana"))]
use alloc::{vec, vec::Vec};

#[cfg(not(target_os = "solana"))]
use dclutch_account_profile_contract::v2::{
    AccountProfileV2, TrustedEnvironmentV2,
    encode::{
        AccountAliasInputV2, AccountCoordinateV2, AccountEffectPermissionsV2,
        AccountOperationInputV2, AccountPrivilegesV2, AccountProfileArtifactV2, AccountRuleInputV2,
        IdentityCoordinateV2, RegisterGeometryV2,
        encode_account_profile_with_environment_v2_atomic,
    },
};

use super::{
    v3_hot_artifact::{
        DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3, DEALER_EQUITY_CUSTODY_CALLEE_ACCOUNT_COUNT_V3,
        DEALER_EQUITY_LOCAL_ACCOUNT_COUNT_V3, DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3,
        DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3, DealerCustodyIdentityFieldV3,
        dealer_current_slot_scalar_register_v3, dealer_custody_identity_register_v3,
        dealer_equity_identity_count_v3, dealer_equity_scalar_count_v3,
    },
    v3_multi_lp::MultiLpActionV3,
};

const CUSTODY_REPLAY_ACCOUNT_V3: u16 = 8;
const CUSTODY_SOURCE_ACCOUNT_V3: u16 = 10;
const CUSTODY_DESTINATION_ACCOUNT_V3: u16 = 11;
const CUSTODY_REGISTRY_ACCOUNT_V3: u16 = 3;
const CUSTODY_CALLER_PROGRAM_ACCOUNT_V3: u16 = 4;
const CUSTODY_CALLER_PROGRAMDATA_ACCOUNT_V3: u16 = 5;
const CUSTODY_TOKEN_PROGRAM_ACCOUNT_V3: u16 = 13;

const CLAIMS_MARKET_ACCOUNT_V3: u16 = 1;
const CLAIMS_BASIS_RECORD_ACCOUNT_V3: u16 = 2;
const CLAIMS_PRODUCT_RECORD_ACCOUNT_V3: u16 = 4;
const CLAIMS_PORTFOLIO_RECORD_ACCOUNT_V3: u16 = 8;
const CLAIMS_CORE_MARKET_ACCOUNT_V3: u16 = 11;
const CLAIMS_REGISTRY_ACCOUNT_V3: u16 = 13;
const CLAIMS_CALLER_PROGRAM_ACCOUNT_V3: u16 = 14;
const CLAIMS_CALLER_PROGRAMDATA_ACCOUNT_V3: u16 = 15;
const CLAIMS_PROGRAM_ACCOUNT_V3: u16 = 16;
const CLAIMS_CORE_PROGRAM_ACCOUNT_V3: u16 = 18;

/// Stable construction refusal for one exact logical profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerEquityProfileErrorV3 {
    /// Action, SignedDelta Position count, or count-derived geometry refused.
    Geometry,
    /// Exact finalized account data widths were absent or overflowed.
    DataLengths,
    /// The canonical AccountProfile encoder or hostile decoder refused.
    Artifact,
}

/// Finalized deployment observations required to pin one AccountProfile.
#[cfg(not(target_os = "solana"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerEquityAccountProfileInputV3<'a> {
    /// Contribution or redemption physical route shape.
    pub action: MultiLpActionV3,
    /// Canonical SignedDelta Position tail width P0/P1/P2.
    pub signed_position_count: u32,
    /// Exact account data length at every logical Effect coordinate.
    pub logical_data_lengths: &'a [u32],
}

/// Exact logical account count for one action/P physical shape.
pub fn dealer_equity_logical_account_count_v3(
    action: MultiLpActionV3,
    signed_position_count: u32,
) -> Result<u16, DealerEquityProfileErrorV3> {
    if signed_position_count > 2 {
        return Err(DealerEquityProfileErrorV3::Geometry);
    }
    let custody_routes = match action {
        MultiLpActionV3::Add => 2_u16,
        MultiLpActionV3::Remove => 3_u16,
    };
    DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3
        .checked_add(
            custody_routes
                .checked_mul(DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3)
                .ok_or(DealerEquityProfileErrorV3::Geometry)?,
        )
        .and_then(|value| value.checked_add(DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3))
        .and_then(|value| value.checked_add(u16::try_from(signed_position_count).ok()?))
        .and_then(|value| value.checked_add(DEALER_EQUITY_LOCAL_ACCOUNT_COUNT_V3))
        .and_then(|value| value.checked_add(DEALER_EQUITY_CUSTODY_CALLEE_ACCOUNT_COUNT_V3))
        .ok_or(DealerEquityProfileErrorV3::Geometry)
}

/// Encode the exact live-state AccountProfile5 for one equity action/P shape.
///
/// Both output and all logical widths are caller-owned. The returned bytes are
/// hostile-decoded before being exposed; no partial profile is returned.
#[cfg(not(target_os = "solana"))]
pub fn encode_dealer_equity_account_profile_v3(
    input: DealerEquityAccountProfileInputV3<'_>,
) -> Result<Vec<u8>, DealerEquityProfileErrorV3> {
    let account_count =
        dealer_equity_logical_account_count_v3(input.action, input.signed_position_count)?;
    if input.logical_data_lengths.len() != usize::from(account_count) {
        return Err(DealerEquityProfileErrorV3::DataLengths);
    }
    let claims_start = DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3
        .checked_add(DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3)
        .ok_or(DealerEquityProfileErrorV3::Geometry)?;
    let claims_count = DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3
        .checked_add(
            u16::try_from(input.signed_position_count)
                .map_err(|_| DealerEquityProfileErrorV3::Geometry)?,
        )
        .ok_or(DealerEquityProfileErrorV3::Geometry)?;
    let custody_routes = match input.action {
        MultiLpActionV3::Add => 2_u16,
        MultiLpActionV3::Remove => 3_u16,
    };
    let local_start = DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3
        .checked_add(
            custody_routes
                .checked_mul(DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3)
                .ok_or(DealerEquityProfileErrorV3::Geometry)?,
        )
        .and_then(|value| value.checked_add(claims_count))
        .ok_or(DealerEquityProfileErrorV3::Geometry)?;
    let obligation = local_start;
    let lp = obligation
        .checked_add(1)
        .ok_or(DealerEquityProfileErrorV3::Geometry)?;
    // Appended past every route range and past both local write targets, so
    // adding it renumbered nothing.
    let custody_program = lp
        .checked_add(1)
        .ok_or(DealerEquityProfileErrorV3::Geometry)?;
    if custody_program.checked_add(1) != Some(account_count) {
        return Err(DealerEquityProfileErrorV3::Geometry);
    }

    let mut rules = Vec::with_capacity(usize::from(account_count));
    for coordinate in 0..account_count {
        rules.push(account_rule(
            input.action,
            input.signed_position_count,
            coordinate,
            claims_start,
            obligation,
            lp,
            custody_program,
            input.logical_data_lengths,
        )?);
    }
    let trading_identity =
        dealer_custody_identity_register_v3(0, DealerCustodyIdentityFieldV3::CallerProgram)
            .ok_or(DealerEquityProfileErrorV3::Geometry)?;
    let first_custody = DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3;
    let operations = [
        AccountOperationInputV2::ProjectKey {
            account: AccountCoordinateV2::fixed(
                first_custody
                    .checked_add(CUSTODY_CALLER_PROGRAM_ACCOUNT_V3)
                    .ok_or(DealerEquityProfileErrorV3::Geometry)?,
            ),
            destination: IdentityCoordinateV2::common(trading_identity),
        },
        AccountOperationInputV2::RequireOwner {
            account: AccountCoordinateV2::fixed(obligation),
            expected: IdentityCoordinateV2::common(trading_identity),
        },
        AccountOperationInputV2::RequireOwner {
            account: AccountCoordinateV2::fixed(lp),
            expected: IdentityCoordinateV2::common(trading_identity),
        },
    ];
    let current_slot = dealer_current_slot_scalar_register_v3(input.action)
        .ok_or(DealerEquityProfileErrorV3::Geometry)?;
    let geometry = RegisterGeometryV2 {
        common_scalars: u16::try_from(
            dealer_equity_scalar_count_v3(input.action)
                .map_err(|_| DealerEquityProfileErrorV3::Geometry)?,
        )
        .map_err(|_| DealerEquityProfileErrorV3::Geometry)?,
        item_scalar_stride: 0,
        common_identities: u16::try_from(
            dealer_equity_identity_count_v3(input.action)
                .map_err(|_| DealerEquityProfileErrorV3::Geometry)?,
        )
        .map_err(|_| DealerEquityProfileErrorV3::Geometry)?,
        item_identity_stride: 0,
    };
    let bytes = dclutch_account_profile_contract::v2::HEADER_BYTES
        .checked_add(
            usize::from(account_count)
                .checked_mul(dclutch_account_profile_contract::v2::RULE_BYTES)
                .ok_or(DealerEquityProfileErrorV3::Geometry)?,
        )
        .and_then(|value| {
            value.checked_add(
                operations
                    .len()
                    .checked_mul(dclutch_account_profile_contract::v2::OPERATION_BYTES)?,
            )
        })
        .ok_or(DealerEquityProfileErrorV3::Geometry)?;
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_account_profile_with_environment_v2_atomic(
        AccountProfileArtifactV2::TrustedEnvironment,
        TrustedEnvironmentV2::CurrentSlot {
            destination: current_slot,
        },
        &rules,
        &[],
        &operations,
        &[],
        geometry,
        &mut scratch,
        &mut output,
    )
    .map_err(|_| DealerEquityProfileErrorV3::Artifact)?;
    let profile =
        AccountProfileV2::decode(&output).map_err(|_| DealerEquityProfileErrorV3::Artifact)?;
    if profile.fixed_account_count() != account_count
        || profile.item_account_stride() != 0
        || profile.trusted_current_slot_scalar() != Some(current_slot)
    {
        return Err(DealerEquityProfileErrorV3::Artifact);
    }
    Ok(output)
}

#[cfg(not(target_os = "solana"))]
#[allow(clippy::too_many_arguments)]
fn account_rule(
    action: MultiLpActionV3,
    signed_position_count: u32,
    coordinate: u16,
    claims_start: u16,
    obligation: u16,
    lp: u16,
    custody_program: u16,
    lengths: &[u32],
) -> Result<AccountRuleInputV2, DealerEquityProfileErrorV3> {
    let writable = coordinate == 0
        || coordinate == obligation
        || coordinate == lp
        || child_writable(action, signed_position_count, coordinate, claims_start)?;
    // Readonly, executable, no effect permission, self-representative: the
    // loader that deployed it owns the record and the Registry activation cache
    // is the sole authority on which program the Custody role selects. This
    // topology states its five other program coordinates the same way, at the
    // caller-supplied width, because the AccountProfile5 encoder it uses has no
    // prestate channel to say `opaque` with.
    let executable = coordinate == custody_program
        || child_executable(action, signed_position_count, coordinate, claims_start)?;
    let alias = account_alias(action, signed_position_count, coordinate, claims_start)?;
    let write_data = coordinate == obligation || coordinate == lp;
    Ok(AccountRuleInputV2 {
        privileges: AccountPrivilegesV2::new(false, writable, executable),
        effect_permissions: AccountEffectPermissionsV2::new(false, false, write_data),
        alias,
        data_length: *lengths
            .get(usize::from(coordinate))
            .ok_or(DealerEquityProfileErrorV3::DataLengths)?,
        data_item_stride: 0,
    })
}

#[cfg(not(target_os = "solana"))]
fn child_writable(
    action: MultiLpActionV3,
    signed_position_count: u32,
    coordinate: u16,
    claims_start: u16,
) -> Result<bool, DealerEquityProfileErrorV3> {
    if let Some(offset) = custody_offset(action, signed_position_count, coordinate, claims_start)? {
        return Ok(matches!(
            offset,
            CUSTODY_REPLAY_ACCOUNT_V3 | CUSTODY_SOURCE_ACCOUNT_V3 | CUSTODY_DESTINATION_ACCOUNT_V3
        ));
    }
    let claims_end = claims_start
        .checked_add(DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3)
        .and_then(|value| value.checked_add(u16::try_from(signed_position_count).ok()?))
        .ok_or(DealerEquityProfileErrorV3::Geometry)?;
    Ok(coordinate == claims_start + CLAIMS_MARKET_ACCOUNT_V3
        || (coordinate >= claims_start + DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3
            && coordinate < claims_end))
}

#[cfg(not(target_os = "solana"))]
fn child_executable(
    action: MultiLpActionV3,
    signed_position_count: u32,
    coordinate: u16,
    claims_start: u16,
) -> Result<bool, DealerEquityProfileErrorV3> {
    if let Some(offset) = custody_offset(action, signed_position_count, coordinate, claims_start)? {
        return Ok(matches!(
            offset,
            CUSTODY_REGISTRY_ACCOUNT_V3
                | CUSTODY_CALLER_PROGRAM_ACCOUNT_V3
                | CUSTODY_TOKEN_PROGRAM_ACCOUNT_V3
        ));
    }
    let claims_end = claims_start
        .checked_add(DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3)
        .and_then(|value| value.checked_add(u16::try_from(signed_position_count).ok()?))
        .ok_or(DealerEquityProfileErrorV3::Geometry)?;
    if coordinate < claims_start || coordinate >= claims_end {
        return Ok(false);
    }
    let offset = coordinate - claims_start;
    Ok(matches!(
        offset,
        CLAIMS_REGISTRY_ACCOUNT_V3
            | CLAIMS_CALLER_PROGRAM_ACCOUNT_V3
            | CLAIMS_PROGRAM_ACCOUNT_V3
            | CLAIMS_CORE_PROGRAM_ACCOUNT_V3
    ))
}

#[cfg(not(target_os = "solana"))]
fn account_alias(
    action: MultiLpActionV3,
    signed_position_count: u32,
    coordinate: u16,
    claims_start: u16,
) -> Result<AccountAliasInputV2, DealerEquityProfileErrorV3> {
    let first_custody = DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3;
    if let Some(offset) = custody_offset(action, signed_position_count, coordinate, claims_start)? {
        if coordinate >= first_custody + DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3
            && matches!(offset, 1 | 2 | 3 | 4 | 5 | 6 | 7 | 9 | 12 | 13)
        {
            return Ok(AccountAliasInputV2::Fixed(first_custody + offset));
        }
        return Ok(AccountAliasInputV2::SelfCoordinate);
    }
    let claims_end = claims_start
        .checked_add(DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3)
        .and_then(|value| value.checked_add(u16::try_from(signed_position_count).ok()?))
        .ok_or(DealerEquityProfileErrorV3::Geometry)?;
    if coordinate < claims_start || coordinate >= claims_end {
        return Ok(AccountAliasInputV2::SelfCoordinate);
    }
    let offset = coordinate - claims_start;
    let alias = match offset {
        CLAIMS_BASIS_RECORD_ACCOUNT_V3 => Some(4),
        CLAIMS_PRODUCT_RECORD_ACCOUNT_V3 => Some(2),
        CLAIMS_PORTFOLIO_RECORD_ACCOUNT_V3 => Some(3),
        CLAIMS_CORE_MARKET_ACCOUNT_V3 => Some(first_custody + 1),
        CLAIMS_REGISTRY_ACCOUNT_V3 => Some(first_custody + CUSTODY_REGISTRY_ACCOUNT_V3),
        CLAIMS_CALLER_PROGRAM_ACCOUNT_V3 => Some(first_custody + CUSTODY_CALLER_PROGRAM_ACCOUNT_V3),
        CLAIMS_CALLER_PROGRAMDATA_ACCOUNT_V3 => {
            Some(first_custody + CUSTODY_CALLER_PROGRAMDATA_ACCOUNT_V3)
        }
        _ => None,
    };
    Ok(alias.map_or(
        AccountAliasInputV2::SelfCoordinate,
        AccountAliasInputV2::Fixed,
    ))
}

#[cfg(not(target_os = "solana"))]
fn custody_offset(
    action: MultiLpActionV3,
    signed_position_count: u32,
    coordinate: u16,
    claims_start: u16,
) -> Result<Option<u16>, DealerEquityProfileErrorV3> {
    let claims_count = DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3
        .checked_add(
            u16::try_from(signed_position_count)
                .map_err(|_| DealerEquityProfileErrorV3::Geometry)?,
        )
        .ok_or(DealerEquityProfileErrorV3::Geometry)?;
    let first_start = DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3;
    let first_end = first_start
        .checked_add(DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3)
        .ok_or(DealerEquityProfileErrorV3::Geometry)?;
    if coordinate >= first_start && coordinate < first_end {
        return Ok(Some(coordinate - first_start));
    }
    let later_start = claims_start
        .checked_add(claims_count)
        .ok_or(DealerEquityProfileErrorV3::Geometry)?;
    let later_count = match action {
        MultiLpActionV3::Add => 1_u16,
        MultiLpActionV3::Remove => 2_u16,
    };
    let later_end = later_start
        .checked_add(
            later_count
                .checked_mul(DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3)
                .ok_or(DealerEquityProfileErrorV3::Geometry)?,
        )
        .ok_or(DealerEquityProfileErrorV3::Geometry)?;
    if coordinate >= later_start && coordinate < later_end {
        return Ok(Some(
            (coordinate - later_start) % DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3,
        ));
    }
    Ok(None)
}

#[cfg(all(test, not(target_os = "solana")))]
mod tests {
    use super::*;
    use dclutch_account_profile_contract::v2::{
        AccountProfileV2, LIFECYCLE_PRESTATE_ARTIFACT_PROFILE, TRUSTED_ENVIRONMENT_ARTIFACT_PROFILE,
    };

    #[test]
    fn every_equity_shape_emits_exact_live_profile() {
        for action in [MultiLpActionV3::Add, MultiLpActionV3::Remove] {
            for positions in 0..=2 {
                let count = dealer_equity_logical_account_count_v3(action, positions)
                    .expect("logical geometry");
                let lengths = vec![0_u32; usize::from(count)];
                let bytes =
                    encode_dealer_equity_account_profile_v3(DealerEquityAccountProfileInputV3 {
                        action,
                        signed_position_count: positions,
                        logical_data_lengths: &lengths,
                    })
                    .expect("profile");
                let profile = AccountProfileV2::decode(&bytes).expect("decode");
                assert_eq!(profile.fixed_account_count(), count);
                assert_eq!(
                    profile.artifact_profile(),
                    TRUSTED_ENVIRONMENT_ARTIFACT_PROFILE
                );
                assert_ne!(
                    profile.artifact_profile(),
                    LIFECYCLE_PRESTATE_ARTIFACT_PROFILE
                );
                assert_eq!(
                    profile.trusted_current_slot_scalar(),
                    dealer_current_slot_scalar_register_v3(action)
                );
            }
        }
    }

    #[test]
    fn account_width_and_shape_substitution_refuse() {
        let count = dealer_equity_logical_account_count_v3(MultiLpActionV3::Add, 2)
            .expect("logical geometry");
        let short = vec![0_u32; usize::from(count) - 1];
        assert_eq!(
            encode_dealer_equity_account_profile_v3(DealerEquityAccountProfileInputV3 {
                action: MultiLpActionV3::Add,
                signed_position_count: 2,
                logical_data_lengths: &short,
            }),
            Err(DealerEquityProfileErrorV3::DataLengths)
        );
        assert_eq!(
            dealer_equity_logical_account_count_v3(MultiLpActionV3::Remove, 3),
            Err(DealerEquityProfileErrorV3::Geometry)
        );
    }
}
