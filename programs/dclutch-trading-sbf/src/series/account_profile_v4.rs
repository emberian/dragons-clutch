//! Exact physical AccountProfile for the global Series Consume lifecycle.
//!
//! One profile owns the complete Lock→Found→Realize→Claims→Open logical
//! account vector. The ordered FundingState span is inserted inside Core Found
//! after its fixed 48-account prefix. Repeated identities are authenticated as
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
        RegisterGeometryV2, ScalarCoordinateV2,
        encode_account_profile_with_dynamic_fixed_span_v2_borrowed_generated_atomic,
    },
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_GENERATION_OFFSET, CAPABILITY_ROOT_MARKET_OFFSET,
    CAPABILITY_ROOT_SELECTION_OFFSET,
};
use dclutch_release_set_contract::{
    CAPABILITY_EXECUTION_SELECTION_CONFIG_OFFSET,
    CAPABILITY_EXECUTION_SELECTION_ENTRY_INDEX_OFFSET, CAPABILITY_EXECUTION_SELECTION_KIND_OFFSET,
    CAPABILITY_EXECUTION_SELECTION_MANIFEST_OFFSET, CAPABILITY_EXECUTION_SELECTION_RELEASE_OFFSET,
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
const OPERATION_COUNT: usize = 9;
const CURRENT_TRADING_IDENTITY: u16 = 0;
const COMMON_SCALAR_COUNT: u16 = 7;
const COMMON_IDENTITY_COUNT: u16 = 6;
const ROOT: usize = 0;
const TICKET_REPLAY: usize = 59;

/// Sole Series root coordinate in the global Consume logical frame.
pub const SERIES_CONSUME_ROOT_COORDINATE_V4: u16 = 0;
/// Sole Ticket replay coordinate in the global Consume logical frame.
///
/// The Ticket appears in the Consume frame as a referenced coordinate only:
/// its lamport flow is authored by the funding path
/// ([`super::commit_plans::PendingFundingPlanV3::ticket_capability_refund`]),
/// so a lifecycle plan naming this coordinate — directly or through a route
/// alias — would be a second author for one lamport flow.
pub const SERIES_CONSUME_TICKET_REPLAY_COORDINATE_V4: u16 = 59;

/// Common identity register carrying the root header's Market identity.
pub const SERIES_CONSUME_ROOT_MARKET_IDENTITY_V4: u16 = 1;
/// Common identity register carrying the root header's manifest identity.
pub const SERIES_CONSUME_ROOT_MANIFEST_IDENTITY_V4: u16 = 2;
/// Common identity register carrying the root header's capability kind.
pub const SERIES_CONSUME_ROOT_KIND_IDENTITY_V4: u16 = 3;
/// Common identity register carrying the root header's capability release.
pub const SERIES_CONSUME_ROOT_CAPABILITY_RELEASE_IDENTITY_V4: u16 = 4;
/// Common identity register carrying the root header's config identity.
pub const SERIES_CONSUME_ROOT_CONFIG_IDENTITY_V4: u16 = 5;
/// Common scalar register carrying the root header's Market generation.
pub const SERIES_CONSUME_ROOT_GENERATION_SCALAR_V4: u16 = 5;
/// Common scalar register carrying the root header's manifest entry index.
pub const SERIES_CONSUME_ROOT_ENTRY_INDEX_SCALAR_V4: u16 = 6;

/// Exact Profile13 artifact width for the global Consume account vector.
pub const SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4: usize = DYNAMIC_FIXED_SPAN_HEADER_BYTES
    + DYNAMIC_FIXED_SPAN_ENTRY_BYTES
    + (FIXED_RULE_COUNT + SPAN_RULE_COUNT) * RULE_BYTES
    + OPERATION_COUNT * OPERATION_BYTES;

/// Exact finalized account widths used to specialize the physical profile.
///
/// Coordinates are the 161 fixed base coordinates before insertion of the
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
        || SERIES_CONSUME_ACCOUNT_PROFILE_PREFIX_V4 != 67
        || SERIES_CONSUME_ACCOUNT_PROFILE_SUFFIX_V4 != 94
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
    // The root's own immutable header is the sole author of the root PDA
    // derivation (`CapabilityRootSeedsV1`). Projecting its seven seed fields
    // into common registers lets the lifecycle policy's seed table reference
    // the header's values instead of re-authoring them — a recipe built from
    // any other source would be a second author for the derivation.
    let operations = [
        require_current_owner(ROOT)?,
        require_current_owner(TICKET_REPLAY)?,
        project_root_identity(
            SERIES_CONSUME_ROOT_MARKET_IDENTITY_V4,
            CAPABILITY_ROOT_MARKET_OFFSET,
        )?,
        AccountOperationInputV2::ProjectDataU64 {
            account: AccountCoordinateV2::fixed(SERIES_CONSUME_ROOT_COORDINATE_V4),
            destination: ScalarCoordinateV2::common(SERIES_CONSUME_ROOT_GENERATION_SCALAR_V4),
            data_offset: root_offset(CAPABILITY_ROOT_GENERATION_OFFSET, 0)?,
        },
        project_root_identity(
            SERIES_CONSUME_ROOT_MANIFEST_IDENTITY_V4,
            CAPABILITY_ROOT_SELECTION_OFFSET + CAPABILITY_EXECUTION_SELECTION_MANIFEST_OFFSET,
        )?,
        AccountOperationInputV2::ProjectDataU16 {
            account: AccountCoordinateV2::fixed(SERIES_CONSUME_ROOT_COORDINATE_V4),
            destination: ScalarCoordinateV2::common(SERIES_CONSUME_ROOT_ENTRY_INDEX_SCALAR_V4),
            data_offset: root_offset(
                CAPABILITY_ROOT_SELECTION_OFFSET,
                CAPABILITY_EXECUTION_SELECTION_ENTRY_INDEX_OFFSET,
            )?,
        },
        project_root_identity(
            SERIES_CONSUME_ROOT_KIND_IDENTITY_V4,
            CAPABILITY_ROOT_SELECTION_OFFSET + CAPABILITY_EXECUTION_SELECTION_KIND_OFFSET,
        )?,
        project_root_identity(
            SERIES_CONSUME_ROOT_CAPABILITY_RELEASE_IDENTITY_V4,
            CAPABILITY_ROOT_SELECTION_OFFSET + CAPABILITY_EXECUTION_SELECTION_RELEASE_OFFSET,
        )?,
        project_root_identity(
            SERIES_CONSUME_ROOT_CONFIG_IDENTITY_V4,
            CAPABILITY_ROOT_SELECTION_OFFSET + CAPABILITY_EXECUTION_SELECTION_CONFIG_OFFSET,
        )?,
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

/// Stamp the two Trading-owned state widths a Series release derives.
///
/// The composite root (coordinate 0) and the Ticket replay account
/// (coordinate 59) have widths fixed by release constants rather than by
/// deployment observation, so the release compiler derives them here — at the
/// representatives and at every route alias, which
/// [`encode_series_consume_account_profile_v4_atomic`] requires to agree.
/// This module owns the alias table, so the caller cannot hold a second copy
/// of it to keep in sync.
pub fn stamp_series_release_owned_widths_v4(
    lengths: &mut [u32; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4],
    root_bytes: u32,
    ticket_bytes: u32,
) {
    for (coordinate, width) in [(ROOT, root_bytes), (TICKET_REPLAY, ticket_bytes)] {
        lengths[coordinate] = width;
        for (alias, representative) in ROUTE_ALIASES {
            if *representative == coordinate {
                lengths[*alias] = width;
            }
        }
    }
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

fn project_root_identity(
    destination: u16,
    data_offset: usize,
) -> Result<AccountOperationInputV2, SeriesConsumeAccountProfileErrorV4> {
    Ok(AccountOperationInputV2::ProjectDataIdentity {
        account: AccountCoordinateV2::fixed(SERIES_CONSUME_ROOT_COORDINATE_V4),
        destination: IdentityCoordinateV2::common(destination),
        data_offset: root_offset(data_offset, 0)?,
    })
}

fn root_offset(base: usize, field: usize) -> Result<u32, SeriesConsumeAccountProfileErrorV4> {
    base.checked_add(field)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(SeriesConsumeAccountProfileErrorV4::Geometry)
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
    18, 19, 67, // Shared Market representative, Core payer, permit.
    76, 77, 78, // Claims aggregate, Position, admission representatives.
];

const EXECUTABLE_COORDINATES: &[usize] = &[
    8, 9, 16, // Lock Registry, Trading, Token.
    22, 44, 48, 72, 74, // Found Rent/Core/System and shared role programs.
];

// Base coordinates exclude the inserted FundingState span. Every target is an
// earlier fixed coordinate and the dynamic profile shifts both source and
// target consistently after insertion coordinate 67.
const ROUTE_ALIASES: &[(usize, usize)] = &[
    (20, 18),
    (21, 11),
    (25, 2),
    (29, 3),
    (43, 7),
    (46, 8),
    (56, 9),
    (57, 10),
    (58, 0),
    (68, 6),
    (69, 12),
    (70, 13),
    (71, 17),
    (31, 4),
    (81, 6),
    (82, 7),
    (83, 8),
    (84, 9),
    (85, 10),
    (86, 11),
    (87, 12),
    (88, 18),
    (89, 14),
    (90, 15),
    (91, 16),
    (93, 67),
    (94, 76),
    (95, 77),
    (96, 78),
    (97, 13),
    (98, 12),
    (99, 6),
    (100, 4),
    (101, 32),
    (102, 2),
    (103, 26),
    (104, 27),
    (105, 28),
    (106, 3),
    (107, 30),
    (108, 47),
    (109, 48),
    (110, 18),
    (111, 7),
    (112, 8),
    (113, 72),
    (114, 73),
    (115, 44),
    (116, 45),
    (117, 9),
    (118, 10),
    (119, 74),
    (120, 75),
    (121, 79),
    (122, 11),
    (123, 22),
    (124, 19),
    (125, 18),
    (126, 67),
    (127, 11),
    (128, 22),
    (129, 7),
    (130, 8),
    (131, 9),
    (132, 10),
    (133, 72),
    (134, 73),
    (135, 74),
    (136, 75),
    (137, 44),
    (138, 45),
    (139, 0),
    (140, 59),
    (141, 60),
    (142, 61),
    (143, 62),
    (144, 63),
    (145, 64),
    (146, 65),
    (147, 2),
    (148, 26),
    (149, 27),
    (150, 28),
    (151, 3),
    (152, 30),
    (153, 6),
    (154, 12),
    (155, 13),
    (156, 76),
    (157, 77),
    (158, 78),
    (159, 66),
    (160, 47),
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
        assert_eq!(span.insertion_coordinate(), 67);
        assert_eq!(span.count_scalar(), SERIES_CONSUME_FUNDING_COUNT_SCALAR_V4);
        assert_eq!(span.minimum(), 1);
        assert_eq!(span.maximum(), 16);
        assert_eq!(span.step(), 1);
        assert_eq!(
            profile.logical_account_count_with_dynamic_spans(0, &[1]),
            Ok(162)
        );
        assert_eq!(
            profile.logical_account_count_with_dynamic_spans(0, &[16]),
            Ok(177)
        );
        assert_eq!(
            profile.physical_account_count_with_dynamic_spans(0, &[1]),
            Ok(69)
        );
        assert_eq!(
            profile.physical_account_count_with_dynamic_spans(0, &[16]),
            Ok(84)
        );
        assert_eq!(
            profile.representative_with_dynamic_spans(0, &[7], 67),
            Ok(67)
        );
        assert_eq!(
            profile.representative_with_dynamic_spans(0, &[7], 73),
            Ok(73)
        );
        // Base projected replay coordinate68 shifts to expanded75 and aliases6.
        assert_eq!(
            profile.representative_with_dynamic_spans(0, &[7], 75),
            Ok(6)
        );
        // Base route2 begins80, therefore expanded route2 begins87.
        assert_eq!(
            profile.representative_with_dynamic_spans(0, &[7], 88),
            Ok(6)
        );
    }

    #[test]
    fn ticket_outer_write_does_not_leak_into_core_child_views() {
        let bytes = encoded_profile();
        let profile = AccountProfileV2::decode(&bytes).expect("profile");
        let ticket = profile
            .rule(
                false,
                u16::try_from(TICKET_REPLAY).expect("Ticket replay coordinate fits in u16"),
            )
            .expect("Ticket rule");
        assert_eq!(
            ticket.effect_permissions() & EFFECT_PERMISSION_WRITE_DATA,
            EFFECT_PERMISSION_WRITE_DATA
        );
        let found_ticket = profile
            .route_privileges_with_dynamic_spans(0, &[7], 59)
            .expect("Found Ticket");
        let open_ticket = profile
            .route_privileges_with_dynamic_spans(0, &[7], 147)
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

    /// Each role program the child routes are invoked through is ONE PHYSICAL
    /// ACCOUNT carried at THREE LOGICAL COORDINATES, and that is correct.
    ///
    /// `f680c9e` states, for the two Direct topologies, that a callee coordinate
    /// may be neither an alias nor the representative of one, because
    /// `hot_v3::downgraded_effect_accounts_v3` pushes one entry per LOGICAL
    /// coordinate -- aliases included -- and `selected_role_program_v3` refuses
    /// as hard on the second match as on none. This topology cannot satisfy
    /// that invariant and should not try to: the Custody program is a member of
    /// the Core Found suffix, the Claims founding frame AND the Core Open
    /// suffix, because each of those three child programs genuinely needs it in
    /// its own account list. Three frames naming one account is exactly what an
    /// authenticated route alias is FOR. The same is true of the Claims program.
    ///
    /// So the layout is right and the executor's uniqueness test is at the wrong
    /// granularity: it counts logical coordinates where it means physical
    /// accounts. What this witness pins is the precondition a deduped lookup
    /// needs and the layout owes -- for each role program, the set of logical
    /// coordinates carrying it has exactly ONE representative, and that
    /// representative is a readonly executable. A layout change that split a
    /// role's program across two physical accounts would break the fix as
    /// surely as the fix repairs the layout.
    #[test]
    fn each_role_program_is_one_physical_account_at_three_logical_coordinates() {
        let bytes = encoded_profile();
        let profile = AccountProfileV2::decode(&bytes).expect("profile");
        let funding = [7_u32];
        let shift = usize::try_from(funding[0]).expect("funding width");
        let count = profile
            .logical_account_count_with_dynamic_spans(0, &funding)
            .expect("expanded logical count");
        // Both role programs sit past the FundingState span's insertion
        // coordinate, so each base coordinate shifts by the funding width.
        for (role, base, aliases) in [
            ("Claims", 72_usize, [113_usize, 133]),
            ("Custody", 74_usize, [119_usize, 135]),
        ] {
            let representative = base + shift;
            let mut carriers = vec![];
            for coordinate in 0..count {
                if profile.representative_with_dynamic_spans(0, &funding, coordinate)
                    == Ok(representative)
                {
                    carriers.push(coordinate);
                }
            }
            let expected = vec![representative, aliases[0] + shift, aliases[1] + shift];
            assert_eq!(carriers, expected, "{role} program carriers");
            let privileges = profile
                .route_privileges_with_dynamic_spans(0, &funding, representative)
                .expect("representative privileges");
            assert!(
                privileges.executable() && !privileges.writable() && !privileges.signer(),
                "{role} program representative is not a readonly executable"
            );
            // The aliases are privilege-free, so every entry the executor scans
            // is the SAME readonly executable view of the SAME account.
            for alias in aliases {
                let rule = profile
                    .rule(false, u16::try_from(alias).expect("alias coordinate"))
                    .expect("alias rule");
                assert_eq!(rule.privileges(), 0, "{role} alias {alias}");
                assert_eq!(rule.effect_permissions(), 0, "{role} alias {alias}");
            }
        }
        // The two roles are distinct physical accounts. A shared one would make
        // both lookups resolve to the same program.
        assert_ne!(72 + shift, 74 + shift);
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
