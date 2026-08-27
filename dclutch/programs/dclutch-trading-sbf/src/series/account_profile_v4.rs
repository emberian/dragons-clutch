//! Exact physical AccountProfile for the global Series Consume lifecycle.
//!
//! One profile owns the complete Lock→Found→Realize→Claims→Open logical
//! account vector. The ordered FundingState span is inserted inside Core Found
//! after its fixed 42-account prefix. Repeated identities are authenticated as
//! route-local aliases of one physical representative; child adapters receive
//! downgraded privileges while the physical representative supplies their
//! union. Root and Ticket writes are outer-only commit-last authority.

use dclutch_account_profile_contract::v2::{
    AccountPrestateV2, DYNAMIC_FIXED_SPAN_ENTRY_BYTES, DYNAMIC_FIXED_SPAN_HEADER_BYTES,
    OPERATION_BYTES, RULE_BYTES, TrustedBuiltinIdentityV2, TrustedEnvironmentV2,
    TrustedIdentityEnvironmentV2,
    encode::{
        AccountAliasInputV2, AccountCoordinateV2, AccountEffectPermissionsV2,
        AccountOperationInputV2, AccountPrivilegesV2, AccountRuleInputV2,
        AccountRuleWithPrestateInputV2, DynamicFixedSpanInputV2, IdentityCoordinateV2,
        RegisterGeometryV2,
        encode_account_profile_with_dynamic_fixed_span_v2_borrowed_generated_atomic,
    },
};

use super::{
    artifacts_v3::SERIES_CONSUME_MAXIMUM_FUNDING_STATES_V3,
    effect_v4::{
        SERIES_CONSUME_ACCOUNT_PROFILE_PREFIX_V4, SERIES_CONSUME_ACCOUNT_PROFILE_SUFFIX_V4,
        SERIES_CONSUME_FUNDING_COUNT_SCALAR_V4, SERIES_CONSUME_LOGICAL_ACCOUNT_BASE_V4,
    },
};

/// Exact fixed logical account count before the dynamic FundingState span.
pub const SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4: usize =
    SERIES_CONSUME_LOGICAL_ACCOUNT_BASE_V4 as usize;
const FIXED_RULE_COUNT: usize = SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4;
const SPAN_RULE_COUNT: usize = 1;
const OPERATION_COUNT: usize = 2;
const CURRENT_TRADING_IDENTITY: u16 = 0;
const COMMON_SCALAR_COUNT: u16 = 5;
const COMMON_IDENTITY_COUNT: u16 = 1;
const ROOT: usize = 0;
const TICKET_REPLAY: usize = 53;

/// Exact Profile13 artifact width for the global Consume account vector.
pub const SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4: usize = DYNAMIC_FIXED_SPAN_HEADER_BYTES
    + DYNAMIC_FIXED_SPAN_ENTRY_BYTES
    + (FIXED_RULE_COUNT + SPAN_RULE_COUNT) * RULE_BYTES
    + OPERATION_COUNT * OPERATION_BYTES;

/// Exact finalized account widths used to specialize the physical profile.
///
/// Coordinates are the 157 fixed base coordinates before insertion of the
/// opaque FundingState span. Aliases must repeat their representative's exact
/// pre-execution width; no parallel account-layout authority is accepted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesConsumeAccountProfileInputV4<'a> {
    /// Exact initial data length at every fixed base coordinate.
    pub fixed_data_lengths: &'a [u32; FIXED_RULE_COUNT],
}

/// Stable refusal from the Series-owned physical profile compiler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesConsumeAccountProfileErrorV4 {
    /// Fixed geometry or an alias observation was inconsistent.
    Geometry,
    /// The neutral AccountProfile encoder or hostile decoder refused.
    Profile(dclutch_account_profile_contract::v2::Error),
}

/// Encode one complete global Series Consume Profile13 atomically.
pub fn encode_series_consume_account_profile_v4_atomic(
    input: SeriesConsumeAccountProfileInputV4<'_>,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), SeriesConsumeAccountProfileErrorV4> {
    if scratch.len() != SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4
        || output.len() != SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4
        || SERIES_CONSUME_ACCOUNT_PROFILE_PREFIX_V4 != 61
        || SERIES_CONSUME_ACCOUNT_PROFILE_SUFFIX_V4 != 96
    {
        return Err(SeriesConsumeAccountProfileErrorV4::Geometry);
    }
    validate_alias_lengths(input.fixed_data_lengths)?;
    let span_rules = [AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges: readonly(),
            effect_permissions: none(),
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length: 0,
            data_item_stride: 0,
        },
        prestate: AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
    }];
    let operations = [
        require_current_owner(ROOT)?,
        require_current_owner(TICKET_REPLAY)?,
    ];
    let mut project_fixed_rule = |coordinate| {
        fixed_rule(input.fixed_data_lengths, usize::from(coordinate))
            .map_err(|_| dclutch_account_profile_contract::v2::Error::InvalidLength)
    };
    encode_account_profile_with_dynamic_fixed_span_v2_borrowed_generated_atomic(
        TrustedEnvironmentV2::None,
        TrustedIdentityEnvironmentV2::CurrentExecutingProgram {
            destination: CURRENT_TRADING_IDENTITY,
        },
        TrustedBuiltinIdentityV2::None,
        &[DynamicFixedSpanInputV2 {
            insertion_coordinate: SERIES_CONSUME_ACCOUNT_PROFILE_PREFIX_V4,
            count_scalar: SERIES_CONSUME_FUNDING_COUNT_SCALAR_V4,
            rule_start: 0,
            rule_stride: 1,
            minimum: 1,
            maximum: u32::from(SERIES_CONSUME_MAXIMUM_FUNDING_STATES_V3),
            step: 1,
        }],
        u16::try_from(FIXED_RULE_COUNT)
            .map_err(|_| SeriesConsumeAccountProfileErrorV4::Geometry)?,
        &mut project_fixed_rule,
        &span_rules,
        &operations,
        RegisterGeometryV2 {
            common_scalars: COMMON_SCALAR_COUNT,
            item_scalar_stride: 0,
            common_identities: COMMON_IDENTITY_COUNT,
            item_identity_stride: 0,
        },
        scratch,
        output,
    )
    .map_err(SeriesConsumeAccountProfileErrorV4::Profile)?;
    Ok(())
}

fn fixed_rule(
    lengths: &[u32; FIXED_RULE_COUNT],
    coordinate: usize,
) -> Result<AccountRuleWithPrestateInputV2, SeriesConsumeAccountProfileErrorV4> {
    let data_length = lengths
        .get(coordinate)
        .copied()
        .ok_or(SeriesConsumeAccountProfileErrorV4::Geometry)?;
    if let Some((_, representative)) = ROUTE_ALIASES.iter().find(|(alias, _)| *alias == coordinate)
    {
        return Ok(AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                // Route aliases own no privilege truth. The self
                // representative carries the top-level physical union, while
                // each child FrameSpec independently supplies and downgrades
                // its exact CPI privileges.
                privileges: readonly(),
                effect_permissions: none(),
                alias: AccountAliasInputV2::Fixed(
                    u16::try_from(*representative)
                        .map_err(|_| SeriesConsumeAccountProfileErrorV4::Geometry)?,
                ),
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::AuthenticatedRouteAlias,
        });
    }
    let privileges = if WRITABLE_COORDINATES.contains(&coordinate) {
        writable()
    } else if EXECUTABLE_COORDINATES.contains(&coordinate) {
        executable()
    } else {
        readonly()
    };
    let effect_permissions = if coordinate == ROOT || coordinate == TICKET_REPLAY {
        write_data()
    } else {
        none()
    };
    // Ticket is readonly in both Core child frames. Profile13 converts this
    // authenticated outer WRITE_DATA grant into physical writable authority;
    // route-local child views remain readonly.
    Ok(exact(privileges, effect_permissions, data_length))
}

fn validate_alias_lengths(
    lengths: &[u32; FIXED_RULE_COUNT],
) -> Result<(), SeriesConsumeAccountProfileErrorV4> {
    for (coordinate, representative) in ROUTE_ALIASES {
        if lengths.get(*coordinate) != lengths.get(*representative) {
            return Err(SeriesConsumeAccountProfileErrorV4::Geometry);
        }
    }
    Ok(())
}

fn require_current_owner(
    coordinate: usize,
) -> Result<AccountOperationInputV2, SeriesConsumeAccountProfileErrorV4> {
    Ok(AccountOperationInputV2::RequireOwner {
        account: AccountCoordinateV2::fixed(
            u16::try_from(coordinate).map_err(|_| SeriesConsumeAccountProfileErrorV4::Geometry)?,
        ),
        expected: IdentityCoordinateV2::common(CURRENT_TRADING_IDENTITY),
    })
}

const fn exact(
    privileges: AccountPrivilegesV2,
    effect_permissions: AccountEffectPermissionsV2,
    data_length: u32,
) -> AccountRuleWithPrestateInputV2 {
    AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges,
            effect_permissions,
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length,
            data_item_stride: 0,
        },
        prestate: AccountPrestateV2::Exact,
    }
}

const fn readonly() -> AccountPrivilegesV2 {
    AccountPrivilegesV2::new(false, false, false)
}

const fn writable() -> AccountPrivilegesV2 {
    AccountPrivilegesV2::new(false, true, false)
}

const fn executable() -> AccountPrivilegesV2 {
    AccountPrivilegesV2::new(false, false, true)
}

const fn none() -> AccountEffectPermissionsV2 {
    AccountEffectPermissionsV2::new(false, false, false)
}

const fn write_data() -> AccountEffectPermissionsV2 {
    AccountEffectPermissionsV2::new(false, false, true)
}

// Caller PDAs remain nonsigners in outer observations. The generic fixed-role
// adapters authenticate their exact seeds and add only child-meta index zero as
// an invoke_signed signer.
const WRITABLE_COORDINATES: &[usize] = &[
    0, // Series root, outer commit-last.
    6, 11, 12, 13, 17, // Projected Lock.
    18, 19, 61, // Shared Market representative, Core payer, permit.
    72, 73, 74, // Claims aggregate, Position, admission representatives.
];

const EXECUTABLE_COORDINATES: &[usize] = &[
    8, 9, 16, // Lock Registry, Trading, Token.
    22, 38, 42, 68, 70, // Found Rent/Core/System and shared role programs.
];

// Base coordinates exclude the inserted FundingState span. Every target is an
// earlier fixed coordinate and the dynamic profile shifts both source and
// target consistently after insertion coordinate 61.
const ROUTE_ALIASES: &[(usize, usize)] = &[
    (20, 18),
    (21, 11),
    (25, 2),
    (29, 3),
    (37, 7),
    (40, 8),
    (50, 9),
    (51, 10),
    (52, 0),
    (62, 6),
    (63, 12),
    (64, 13),
    (65, 17),
    (66, 4),
    (77, 6),
    (78, 7),
    (79, 8),
    (80, 9),
    (81, 10),
    (82, 11),
    (83, 12),
    (84, 18),
    (85, 14),
    (86, 15),
    (87, 16),
    (89, 61),
    (90, 72),
    (91, 73),
    (92, 74),
    (93, 13),
    (94, 12),
    (95, 6),
    (96, 4),
    (97, 67),
    (98, 2),
    (99, 26),
    (100, 27),
    (101, 28),
    (102, 3),
    (103, 30),
    (104, 41),
    (105, 42),
    (106, 18),
    (107, 7),
    (108, 8),
    (109, 68),
    (110, 69),
    (111, 38),
    (112, 39),
    (113, 9),
    (114, 10),
    (115, 70),
    (116, 71),
    (117, 75),
    (118, 11),
    (119, 22),
    (120, 19),
    (121, 18),
    (122, 61),
    (123, 11),
    (124, 22),
    (125, 7),
    (126, 8),
    (127, 9),
    (128, 10),
    (129, 68),
    (130, 69),
    (131, 70),
    (132, 71),
    (133, 38),
    (134, 39),
    (135, 0),
    (136, 53),
    (137, 54),
    (138, 55),
    (139, 56),
    (140, 57),
    (141, 58),
    (142, 59),
    (143, 2),
    (144, 26),
    (145, 27),
    (146, 28),
    (147, 3),
    (148, 30),
    (149, 6),
    (150, 12),
    (151, 13),
    (152, 72),
    (153, 73),
    (154, 74),
    (155, 60),
    (156, 41),
];

#[cfg(test)]
mod tests {
    extern crate alloc;

    use alloc::vec;
    use dclutch_account_profile_contract::{
        EFFECT_PERMISSION_WRITE_DATA,
        v2::{AccountProfileV2, DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE},
    };

    use super::*;

    fn encoded_profile() -> alloc::vec::Vec<u8> {
        let lengths = [0_u32; FIXED_RULE_COUNT];
        let mut scratch = vec![0_u8; SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4];
        let mut output = vec![0_u8; SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4];
        encode_series_consume_account_profile_v4_atomic(
            SeriesConsumeAccountProfileInputV4 {
                fixed_data_lengths: &lengths,
            },
            &mut scratch,
            &mut output,
        )
        .expect("Series Consume Profile13");
        output
    }

    #[test]
    fn dynamic_span_and_physical_alias_geometry_are_exact() {
        let bytes = encoded_profile();
        let profile = AccountProfileV2::decode(&bytes).expect("profile");
        let span = profile.dynamic_fixed_span(0).expect("dynamic span");
        assert_eq!(
            profile.artifact_profile(),
            DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
        );
        assert_eq!(span.insertion_coordinate(), 61);
        assert_eq!(span.count_scalar(), SERIES_CONSUME_FUNDING_COUNT_SCALAR_V4);
        assert_eq!(span.minimum(), 1);
        assert_eq!(span.maximum(), 16);
        assert_eq!(span.step(), 1);
        assert_eq!(
            profile.logical_account_count_with_dynamic_spans(0, &[1]),
            Ok(158)
        );
        assert_eq!(
            profile.logical_account_count_with_dynamic_spans(0, &[16]),
            Ok(173)
        );
        assert_eq!(
            profile.physical_account_count_with_dynamic_spans(0, &[1]),
            Ok(65)
        );
        assert_eq!(
            profile.physical_account_count_with_dynamic_spans(0, &[16]),
            Ok(80)
        );
        assert_eq!(
            profile.representative_with_dynamic_spans(0, &[7], 61),
            Ok(61)
        );
        assert_eq!(
            profile.representative_with_dynamic_spans(0, &[7], 67),
            Ok(67)
        );
        // Base projected replay coordinate62 shifts to expanded69 and aliases6.
        assert_eq!(
            profile.representative_with_dynamic_spans(0, &[7], 69),
            Ok(6)
        );
        // Base route2 begins76, therefore expanded route2 begins83.
        assert_eq!(
            profile.representative_with_dynamic_spans(0, &[7], 84),
            Ok(6)
        );
    }

    #[test]
    fn ticket_outer_write_does_not_leak_into_core_child_views() {
        let bytes = encoded_profile();
        let profile = AccountProfileV2::decode(&bytes).expect("profile");
        let ticket = profile
            .rule(false, TICKET_REPLAY as u16)
            .expect("Ticket rule");
        assert_eq!(
            ticket.effect_permissions() & EFFECT_PERMISSION_WRITE_DATA,
            EFFECT_PERMISSION_WRITE_DATA
        );
        let found_ticket = profile
            .route_privileges_with_dynamic_spans(0, &[7], 53)
            .expect("Found Ticket");
        let open_ticket = profile
            .route_privileges_with_dynamic_spans(0, &[7], 143)
            .expect("Open Ticket");
        assert!(!found_ticket.writable());
        assert!(!open_ticket.writable());
    }

    #[test]
    fn aliases_are_zero_privilege_and_representatives_own_outer_union() {
        let bytes = encoded_profile();
        let profile = AccountProfileV2::decode(&bytes).expect("profile");
        for (alias, _) in ROUTE_ALIASES {
            let alias = u16::try_from(*alias).expect("bounded alias coordinate");
            let rule = profile.rule(false, alias).expect("alias rule");
            assert_eq!(rule.privileges(), 0);
            assert_eq!(rule.effect_permissions(), 0);
        }

        let market_alias = profile
            .route_privileges_with_dynamic_spans(0, &[7], 20)
            .expect("Core Found Market alias");
        assert!(!market_alias.signer());
        assert!(!market_alias.writable());
        assert!(!market_alias.executable());

        let market_ordinal = profile
            .physical_account_ordinal_with_dynamic_spans(0, &[7], 18)
            .expect("Market representative ordinal");
        let market = profile
            .physical_account_geometry_with_dynamic_spans(0, &[7], market_ordinal)
            .expect("Market representative geometry");
        assert_eq!(market.logical_representative(), 18);
        assert!(market.privileges().writable());
    }

    #[test]
    fn alias_width_substitution_refuses_before_encoding() {
        let mut lengths = [0_u32; FIXED_RULE_COUNT];
        lengths[20] = 1;
        let mut scratch = vec![0_u8; SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4];
        let mut output = vec![0_u8; SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4];
        assert_eq!(
            encode_series_consume_account_profile_v4_atomic(
                SeriesConsumeAccountProfileInputV4 {
                    fixed_data_lengths: &lengths,
                },
                &mut scratch,
                &mut output,
            ),
            Err(SeriesConsumeAccountProfileErrorV4::Geometry)
        );
    }
}
