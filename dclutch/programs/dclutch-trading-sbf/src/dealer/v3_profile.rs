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
    AccountPrestateV2, AccountProfileV2, TrustedBuiltinIdentityV2, TrustedEnvironmentV2,
    TrustedIdentityEnvironmentV2,
    encode::{
        AccountAliasInputV2, AccountCoordinateV2, AccountEffectPermissionsV2,
        AccountOperationInputV2, AccountPrivilegesV2, AccountRuleInputV2,
        AccountRuleWithPrestateInputV2, IdentityCoordinateV2, RegisterGeometryV2,
        encode_account_profile_with_authenticated_route_alias_v2_atomic,
    },
};

use super::{
    v3_hot_artifact::{
        DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3, DEALER_EQUITY_CUSTODY_CALLEE_ACCOUNT_COUNT_V3,
        DEALER_EQUITY_LOCAL_ACCOUNT_COUNT_V3, DEALER_EQUITY_POSITION_EVIDENCE_ACCOUNT_COUNT_V3,
        DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3, DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3,
        DealerCustodyIdentityFieldV3, dealer_current_slot_scalar_register_v3,
        dealer_custody_identity_register_v3, dealer_equity_evidence_owner_identity_register_v3,
        dealer_equity_identity_count_v3, dealer_equity_scalar_count_v3,
    },
    v3_multi_lp::MultiLpActionV3,
};

const CUSTODY_REPLAY_ACCOUNT_V3: u16 = 8;
const CUSTODY_SOURCE_ACCOUNT_V3: u16 = 10;
const CUSTODY_DESTINATION_ACCOUNT_V3: u16 = 11;
const CUSTODY_ACTIVATION_CACHE_ACCOUNT_V3: u16 = 2;
const CUSTODY_REGISTRY_ACCOUNT_V3: u16 = 3;
const CUSTODY_CALLER_PROGRAM_ACCOUNT_V3: u16 = 4;
const CUSTODY_CALLER_PROGRAMDATA_ACCOUNT_V3: u16 = 5;
const CUSTODY_TOKEN_PROGRAM_ACCOUNT_V3: u16 = 13;

const CLAIMS_MARKET_ACCOUNT_V3: u16 = 1;
const CLAIMS_ACTIVATION_CACHE_ACCOUNT_V3: u16 = 12;
const CLAIMS_BASIS_RECORD_ACCOUNT_V3: u16 = 2;
const CLAIMS_PRODUCT_RECORD_ACCOUNT_V3: u16 = 4;
const CLAIMS_PORTFOLIO_RECORD_ACCOUNT_V3: u16 = 8;
const CLAIMS_CORE_MARKET_ACCOUNT_V3: u16 = 11;
const CLAIMS_REGISTRY_ACCOUNT_V3: u16 = 13;
const CLAIMS_CALLER_PROGRAM_ACCOUNT_V3: u16 = 14;
const CLAIMS_CALLER_PROGRAMDATA_ACCOUNT_V3: u16 = 15;
const CLAIMS_PROGRAM_ACCOUNT_V3: u16 = 16;
const CLAIMS_CORE_PROGRAM_ACCOUNT_V3: u16 = 18;

// The shared Hot prefix projects this finalized record through its authenticated
// content digest rather than pinning one adapter serialization. Claims borrows
// that same semantic owner at its linked-basis route coordinate.
const LINKED_BASIS_CONTENT_ACCOUNT_V3: u16 = 4;

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
        .and_then(|value| value.checked_add(DEALER_EQUITY_POSITION_EVIDENCE_ACCOUNT_COUNT_V3))
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
    let evidence_start = custody_program
        .checked_add(DEALER_EQUITY_CUSTODY_CALLEE_ACCOUNT_COUNT_V3)
        .ok_or(DealerEquityProfileErrorV3::Geometry)?;
    if evidence_start.checked_add(DEALER_EQUITY_POSITION_EVIDENCE_ACCOUNT_COUNT_V3)
        != Some(account_count)
    {
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
            evidence_start,
            input.logical_data_lengths,
        )?);
    }
    let trading_identity =
        dealer_custody_identity_register_v3(0, DealerCustodyIdentityFieldV3::CallerProgram)
            .ok_or(DealerEquityProfileErrorV3::Geometry)?;
    let first_custody = DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3;
    let claims_program = claims_start
        .checked_add(CLAIMS_PROGRAM_ACCOUNT_V3)
        .ok_or(DealerEquityProfileErrorV3::Geometry)?;
    let evidence_owner = dealer_equity_evidence_owner_identity_register_v3(input.action)
        .ok_or(DealerEquityProfileErrorV3::Geometry)?;
    let operations = [
        AccountOperationInputV2::RequireKey {
            account: AccountCoordinateV2::fixed(
                first_custody
                    .checked_add(CUSTODY_CALLER_PROGRAM_ACCOUNT_V3)
                    .ok_or(DealerEquityProfileErrorV3::Geometry)?,
            ),
            expected: IdentityCoordinateV2::common(trading_identity),
        },
        AccountOperationInputV2::RequireOwner {
            account: AccountCoordinateV2::fixed(obligation),
            expected: IdentityCoordinateV2::common(trading_identity),
        },
        AccountOperationInputV2::RequireOwner {
            account: AccountCoordinateV2::fixed(lp),
            expected: IdentityCoordinateV2::common(trading_identity),
        },
        AccountOperationInputV2::ProjectKey {
            account: AccountCoordinateV2::fixed(claims_program),
            destination: IdentityCoordinateV2::common(evidence_owner),
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
    let bytes = dclutch_account_profile_contract::v2::AUTHENTICATED_ROUTE_ALIAS_HEADER_BYTES
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
    encode_account_profile_with_authenticated_route_alias_v2_atomic(
        TrustedEnvironmentV2::CurrentSlot {
            destination: current_slot,
        },
        TrustedIdentityEnvironmentV2::CurrentExecutingProgram {
            destination: trading_identity,
        },
        TrustedBuiltinIdentityV2::None,
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
    evidence_start: u16,
    lengths: &[u32],
) -> Result<AccountRuleWithPrestateInputV2, DealerEquityProfileErrorV3> {
    let writable = coordinate == 0
        || coordinate == obligation
        || coordinate == lp
        || child_writable(action, signed_position_count, coordinate, claims_start)?;
    // Readonly, executable, no effect permission, self-representative: the
    // loader that deployed it owns the record and the Registry activation cache
    // is the sole authority on which program the Custody role selects. This
    // topology states its five other program coordinates the same way, at the
    // caller-supplied width. Profile11's route-alias prestate is reserved for
    // later non-owning coordinates; each self representative remains exact.
    let executable = coordinate == custody_program
        || child_executable(action, signed_position_count, coordinate, claims_start)?;
    let alias = if coordinate >= evidence_start {
        let evidence = coordinate - evidence_start;
        if u32::from(evidence) < signed_position_count {
            AccountAliasInputV2::Fixed(
                claims_start
                    .checked_add(DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3)
                    .and_then(|value| value.checked_add(evidence))
                    .ok_or(DealerEquityProfileErrorV3::Geometry)?,
            )
        } else {
            AccountAliasInputV2::SelfCoordinate
        }
    } else {
        account_alias(action, signed_position_count, coordinate, claims_start)?
    };
    let aliased = alias != AccountAliasInputV2::SelfCoordinate;
    let variable_data_owner = coordinate == LINKED_BASIS_CONTENT_ACCOUNT_V3;
    let variable_data_alias = alias == AccountAliasInputV2::Fixed(LINKED_BASIS_CONTENT_ACCOUNT_V3);
    let write_data = coordinate == obligation || coordinate == lp;
    Ok(AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges: if aliased {
                AccountPrivilegesV2::new(false, false, false)
            } else {
                AccountPrivilegesV2::new(false, writable, executable)
            },
            effect_permissions: if aliased {
                AccountEffectPermissionsV2::new(false, false, false)
            } else {
                AccountEffectPermissionsV2::new(false, false, write_data)
            },
            alias,
            data_length: if aliased {
                0
            } else {
                *lengths
                    .get(usize::from(coordinate))
                    .ok_or(DealerEquityProfileErrorV3::DataLengths)?
            },
            data_item_stride: 0,
        },
        prestate: if variable_data_owner {
            AccountPrestateV2::AdapterAuthenticatedVariableData
        } else if variable_data_alias {
            AccountPrestateV2::AdapterAuthenticatedVariableDataAlias
        } else if aliased {
            AccountPrestateV2::AuthenticatedRouteAlias
        } else {
            AccountPrestateV2::Exact
        },
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
        if coordinate >= first_custody + DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3 {
            if matches!(offset, 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 12 | 13) {
                return Ok(AccountAliasInputV2::Fixed(first_custody + offset));
            }
            let claims_count = DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3
                .checked_add(
                    u16::try_from(signed_position_count)
                        .map_err(|_| DealerEquityProfileErrorV3::Geometry)?,
                )
                .ok_or(DealerEquityProfileErrorV3::Geometry)?;
            let later_start = claims_start
                .checked_add(claims_count)
                .ok_or(DealerEquityProfileErrorV3::Geometry)?;
            let route = (coordinate - later_start) / DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3 + 1;
            let endpoint = match (action, route, offset) {
                (MultiLpActionV3::Add, 1, CUSTODY_DESTINATION_ACCOUNT_V3) => {
                    Some(CUSTODY_DESTINATION_ACCOUNT_V3)
                }
                (MultiLpActionV3::Remove, 1, CUSTODY_SOURCE_ACCOUNT_V3)
                | (MultiLpActionV3::Remove, 2, CUSTODY_DESTINATION_ACCOUNT_V3) => {
                    Some(CUSTODY_SOURCE_ACCOUNT_V3)
                }
                (MultiLpActionV3::Remove, 2, CUSTODY_SOURCE_ACCOUNT_V3) => {
                    Some(CUSTODY_DESTINATION_ACCOUNT_V3)
                }
                _ => None,
            };
            if let Some(representative_offset) = endpoint {
                return Ok(AccountAliasInputV2::Fixed(
                    first_custody + representative_offset,
                ));
            }
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
        CLAIMS_ACTIVATION_CACHE_ACCOUNT_V3 => {
            Some(first_custody + CUSTODY_ACTIVATION_CACHE_ACCOUNT_V3)
        }
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
        AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE, AccountProfileV2,
        LIFECYCLE_PRESTATE_ARTIFACT_PROFILE,
    };

    #[test]
    fn every_equity_shape_emits_exact_live_profile() {
        for action in [MultiLpActionV3::Add, MultiLpActionV3::Remove] {
            for positions in 0..=2 {
                let count = dealer_equity_logical_account_count_v3(action, positions)
                    .expect("logical geometry");
                let mut lengths = vec![0_u32; usize::from(count)];
                *lengths
                    .get_mut(usize::from(LINKED_BASIS_CONTENT_ACCOUNT_V3))
                    .expect("linked-basis content coordinate") = 1;
                let bytes =
                    encode_dealer_equity_account_profile_v3(DealerEquityAccountProfileInputV3 {
                        action,
                        signed_position_count: positions,
                        logical_data_lengths: &lengths,
                    })
                    .unwrap_or_else(|error| {
                        panic!("{action:?} profile with {positions} positions: {error:?}")
                    });
                let profile = AccountProfileV2::decode(&bytes).expect("decode");
                assert_eq!(profile.fixed_account_count(), count);
                assert_eq!(
                    profile.artifact_profile(),
                    AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE
                );
                assert!(profile.supports_route_alias_packing());
                assert_ne!(
                    profile.artifact_profile(),
                    LIFECYCLE_PRESTATE_ARTIFACT_PROFILE
                );
                assert_eq!(
                    profile.trusted_current_slot_scalar(),
                    dealer_current_slot_scalar_register_v3(action)
                );
                assert_eq!(
                    profile.trusted_current_executing_program_identity(),
                    dealer_custody_identity_register_v3(
                        0,
                        DealerCustodyIdentityFieldV3::CallerProgram,
                    ),
                );
                let evidence_start = count - DEALER_EQUITY_POSITION_EVIDENCE_ACCOUNT_COUNT_V3;
                let claims_start =
                    DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3 + DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3;
                assert_eq!(
                    profile
                        .rule(false, LINKED_BASIS_CONTENT_ACCOUNT_V3)
                        .expect("linked-basis content rule")
                        .prestate(),
                    AccountPrestateV2::AdapterAuthenticatedVariableData,
                );
                assert_eq!(
                    profile
                        .rule(false, claims_start + CLAIMS_BASIS_RECORD_ACCOUNT_V3)
                        .expect("Claims linked-basis alias rule")
                        .prestate(),
                    AccountPrestateV2::AdapterAuthenticatedVariableDataAlias,
                );
                assert_eq!(
                    profile.representative(
                        0,
                        usize::from(claims_start + CLAIMS_BASIS_RECORD_ACCOUNT_V3),
                    ),
                    Ok(usize::from(LINKED_BASIS_CONTENT_ACCOUNT_V3)),
                );
                for (offset, representative) in [
                    (CLAIMS_PRODUCT_RECORD_ACCOUNT_V3, 2_usize),
                    (CLAIMS_PORTFOLIO_RECORD_ACCOUNT_V3, 3_usize),
                ] {
                    let coordinate = claims_start + offset;
                    assert_eq!(
                        profile
                            .rule(false, coordinate)
                            .expect("ordinary record alias rule")
                            .prestate(),
                        AccountPrestateV2::AuthenticatedRouteAlias,
                    );
                    assert_eq!(
                        profile.representative(0, usize::from(coordinate)),
                        Ok(representative),
                    );
                }
                let claims_count = DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3
                    + u16::try_from(positions).expect("small P");
                let later_start = claims_start + claims_count;
                let later_routes = match action {
                    MultiLpActionV3::Add => 1_u16,
                    MultiLpActionV3::Remove => 2_u16,
                };
                for route in 1..=later_routes {
                    let route_start =
                        later_start + (route - 1) * DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3;
                    assert_eq!(
                        profile.representative(
                            0,
                            usize::from(route_start + CUSTODY_REPLAY_ACCOUNT_V3),
                        ),
                        Ok(usize::from(
                            DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3 + CUSTODY_REPLAY_ACCOUNT_V3,
                        )),
                        "{action:?} route {route} reuses the sole replay representative",
                    );
                }
                let first_source =
                    usize::from(DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3 + CUSTODY_SOURCE_ACCOUNT_V3);
                let first_destination = usize::from(
                    DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3 + CUSTODY_DESTINATION_ACCOUNT_V3,
                );
                match action {
                    MultiLpActionV3::Add => {
                        assert_eq!(
                            profile.representative(
                                0,
                                usize::from(later_start + CUSTODY_DESTINATION_ACCOUNT_V3),
                            ),
                            Ok(first_destination),
                        );
                        assert_eq!(
                            profile.representative(
                                0,
                                usize::from(later_start + CUSTODY_SOURCE_ACCOUNT_V3),
                            ),
                            Ok(usize::from(later_start + CUSTODY_SOURCE_ACCOUNT_V3)),
                        );
                    }
                    MultiLpActionV3::Remove => {
                        assert_eq!(
                            profile.representative(
                                0,
                                usize::from(later_start + CUSTODY_SOURCE_ACCOUNT_V3),
                            ),
                            Ok(first_source),
                        );
                        assert_eq!(
                            profile.representative(
                                0,
                                usize::from(later_start + CUSTODY_DESTINATION_ACCOUNT_V3),
                            ),
                            Ok(usize::from(later_start + CUSTODY_DESTINATION_ACCOUNT_V3)),
                        );
                        let third_start = later_start + DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3;
                        assert_eq!(
                            profile.representative(
                                0,
                                usize::from(third_start + CUSTODY_SOURCE_ACCOUNT_V3),
                            ),
                            Ok(first_destination),
                        );
                        assert_eq!(
                            profile.representative(
                                0,
                                usize::from(third_start + CUSTODY_DESTINATION_ACCOUNT_V3),
                            ),
                            Ok(first_source),
                        );
                    }
                }
                for evidence in 0..DEALER_EQUITY_POSITION_EVIDENCE_ACCOUNT_COUNT_V3 {
                    let coordinate = evidence_start + evidence;
                    let rule = profile.rule(false, coordinate).expect("evidence rule");
                    assert!(!rule.route_privileges().signer());
                    assert!(!rule.route_privileges().writable());
                    assert!(!rule.route_privileges().executable());
                    let expected = if u32::from(evidence) < positions {
                        usize::from(
                            claims_start + DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3 + evidence,
                        )
                    } else {
                        usize::from(coordinate)
                    };
                    assert_eq!(
                        profile.representative(0, usize::from(coordinate)),
                        Ok(expected)
                    );
                }
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
