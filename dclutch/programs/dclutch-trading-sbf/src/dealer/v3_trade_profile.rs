//! Exact Profile13 physical account program for Dealer scenario execution.
//!
//! One fixed base contains the five common Hot coordinates, the canonical
//! twenty-account Claims frame, and the sole Trading-owned obligation. Nine
//! protected spans insert six optional Custody transfer frames, the exact
//! one-or-two Position tail, and a trailing zero-to-three-account readonly
//! evidence row for otherwise absent Fee/Hoard balances and the P1 Dealer
//! Position. The final exact-six-account readonly span carries the authenticated
//! admitted input bank. Child data remains opaque to Trading; only the
//! obligation carries local write authority.

#[cfg(not(target_os = "solana"))]
extern crate alloc;

#[cfg(all(test, not(target_os = "solana")))]
use alloc::{vec, vec::Vec};

#[cfg(not(target_os = "solana"))]
use dclutch_account_profile_contract::v2::{
    AccountPrestateV2, AccountProfileV2, DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE,
    TrustedBuiltinIdentityV2, TrustedEnvironmentV2, TrustedIdentityEnvironmentV2,
    encode::{
        AccountAliasInputV2, AccountCoordinateV2, AccountEffectPermissionsV2,
        AccountOperationInputV2, AccountPrivilegesV2, AccountRuleInputV2,
        AccountRuleWithPrestateInputV2, DynamicFixedSpanInputV2, IdentityCoordinateV2,
        RegisterGeometryV2, encode_account_profile_with_dynamic_fixed_span_v2_atomic,
    },
};
use dclutch_account_profile_contract::v2::{
    DYNAMIC_FIXED_SPAN_ENTRY_BYTES, DYNAMIC_FIXED_SPAN_HEADER_BYTES, OPERATION_BYTES, RULE_BYTES,
};
use dclutch_claims_svm::frame_spec_v1::SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3;
#[cfg(not(target_os = "solana"))]
use dclutch_claims_svm::frame_spec_v1::SignedDeltaFrameSpecV3;
#[cfg(not(target_os = "solana"))]
use dclutch_custody_contract::{CustodyFrameSpecV1, OperationV1};
#[cfg(not(target_os = "solana"))]
use dclutch_dealer_codec::config_v4::DEALER_CONFIG_BYTES_V4;
#[cfg(not(target_os = "solana"))]
use dclutch_product_runtime_v2_svm_reader::BASIS_WIDTH_OFFSET_V3;

use super::v3_hot_artifact::{
    DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3, DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3,
};
#[cfg(not(target_os = "solana"))]
use super::{
    v3_obligation::DEALER_OBLIGATION_HEADER_BYTES_V3,
    v3_trade_artifacts::{
        DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4, DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4,
        DEALER_SCENARIO_CURRENT_SLOT_SCALAR_V4, DEALER_SCENARIO_CURRENT_TRADING_IDENTITY_V4,
        DEALER_SCENARIO_EVIDENCE_SPAN_COUNT_SCALAR_V4, DEALER_SCENARIO_ITEM_IDENTITY_STRIDE_V4,
        DEALER_SCENARIO_ITEM_SCALAR_STRIDE_V4, DEALER_SCENARIO_OBLIGATION_IDENTITY_V4,
        DEALER_SCENARIO_POSITION_COUNT_SCALAR_V4, DEALER_SCENARIO_ROUTE_SPAN_SCALAR_BASE_V4,
        DEALER_SCENARIO_SCRATCH_PAGE_COUNT_SCALAR_V4,
    },
};

/// Number of fixed base rules before protected spans are inserted.
pub const DEALER_SCENARIO_PROFILE_FIXED_RULES_V4: usize = 26;
/// Number of canonical protected span entries.
pub const DEALER_SCENARIO_PROFILE_SPANS_V4: usize = 9;
/// Fourteen rules for each of six Custody frames, one Claims Position rule,
/// plus homogeneous rules cycled across trailing readonly evidence and scratch
/// transport pages.
pub const DEALER_SCENARIO_PROFILE_SPAN_RULES_V4: usize = 87;
/// Exact selector-9 Profile13 artifact width.
pub const DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4: usize = DYNAMIC_FIXED_SPAN_HEADER_BYTES
    + DEALER_SCENARIO_PROFILE_SPANS_V4 * DYNAMIC_FIXED_SPAN_ENTRY_BYTES
    + (DEALER_SCENARIO_PROFILE_FIXED_RULES_V4 + DEALER_SCENARIO_PROFILE_SPAN_RULES_V4) * RULE_BYTES
    + 3 * OPERATION_BYTES;

const CLAIMS_START_V4: u16 = DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3;
const OBLIGATION_V4: u16 = CLAIMS_START_V4 + SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3;
const CLAIMS_LINKED_BASIS_OFFSET_V4: u16 = 2;
const CLAIMS_PRODUCT_OFFSET_V4: u16 = 4;
const CLAIMS_PORTFOLIO_OFFSET_V4: u16 = 8;

/// Exact finalized widths of the five common logical Hot accounts.
#[cfg(not(target_os = "solana"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerScenarioAccountProfileInputV4 {
    /// Root, config, Product root, portfolio, and linked-basis data widths.
    pub common_data_lengths: [u32; 5],
}

/// Stable refusal from selector-9 Profile13 construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerScenarioAccountProfileErrorV4 {
    /// The fixed or span geometry differed from selector 9.
    Geometry,
    /// The generic profile encoder or hostile decoder refused.
    Profile,
}

/// Expanded logical coordinates selected by one exact nine-span row.
///
/// The external admitted evaluator consumes this map after the common Hot
/// helper authenticates Profile13 and its protected span counts. No caller
/// provides a parallel Claims or Custody account index.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerScenarioLogicalFrameV4 {
    /// First logical coordinate for each of the six optional Custody frames.
    pub custody_starts: [u32; 6],
    /// First logical coordinate of the canonical fixed20 Claims frame.
    pub claims_fixed_start: u32,
    /// First logical coordinate of the one-or-two Claims Position tail.
    pub claims_positions_start: u32,
    /// Sole writable Trading obligation logical coordinate.
    pub obligation: u32,
    /// First trailing readonly evidence coordinate after the obligation.
    pub evidence_start: u32,
    /// Exact request-projected trailing evidence width, zero through three.
    pub evidence_count: u32,
    /// First Hot-owned authenticated input scratch-page coordinate.
    pub scratch_start: u32,
    /// Exact authenticated input scratch-page width.
    pub scratch_count: u32,
    /// Complete expanded logical-account count.
    pub logical_account_count: u32,
}

/// Derive selector 9's expanded logical frame from authenticated span counts.
pub fn dealer_scenario_logical_frame_v4(
    span_counts: [u32; DEALER_SCENARIO_PROFILE_SPANS_V4],
) -> Result<DealerScenarioLogicalFrameV4, DealerScenarioAccountProfileErrorV4> {
    let custody_width = u32::from(DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3);
    if span_counts
        .get(..4)
        .ok_or(DealerScenarioAccountProfileErrorV4::Geometry)?
        .iter()
        .any(|width| !matches!(*width, 0) && *width != custody_width)
        || !matches!(span_counts.get(4), Some(1 | 2))
        || span_counts
            .get(5..7)
            .ok_or(DealerScenarioAccountProfileErrorV4::Geometry)?
            .iter()
            .any(|width| !matches!(*width, 0) && *width != custody_width)
        || !matches!(span_counts.get(7), Some(0..=3))
        || span_counts.get(8) != Some(&6)
    {
        return Err(DealerScenarioAccountProfileErrorV4::Geometry);
    }
    let mut custody_starts = [0_u32; 6];
    let mut cursor = u32::from(DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3);
    for (destination, width) in custody_starts
        .get_mut(..4)
        .ok_or(DealerScenarioAccountProfileErrorV4::Geometry)?
        .iter_mut()
        .zip(
            span_counts
                .get(..4)
                .ok_or(DealerScenarioAccountProfileErrorV4::Geometry)?,
        )
    {
        *destination = cursor;
        cursor = cursor
            .checked_add(*width)
            .ok_or(DealerScenarioAccountProfileErrorV4::Geometry)?;
    }
    let claims_fixed_start = cursor;
    cursor = cursor
        .checked_add(u32::from(SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3))
        .ok_or(DealerScenarioAccountProfileErrorV4::Geometry)?;
    let claims_positions_start = cursor;
    cursor = cursor
        .checked_add(
            *span_counts
                .get(4)
                .ok_or(DealerScenarioAccountProfileErrorV4::Geometry)?,
        )
        .ok_or(DealerScenarioAccountProfileErrorV4::Geometry)?;
    for (destination, width) in custody_starts
        .get_mut(4..)
        .ok_or(DealerScenarioAccountProfileErrorV4::Geometry)?
        .iter_mut()
        .zip(
            span_counts
                .get(5..7)
                .ok_or(DealerScenarioAccountProfileErrorV4::Geometry)?,
        )
    {
        *destination = cursor;
        cursor = cursor
            .checked_add(*width)
            .ok_or(DealerScenarioAccountProfileErrorV4::Geometry)?;
    }
    let obligation = cursor;
    cursor = obligation
        .checked_add(1)
        .ok_or(DealerScenarioAccountProfileErrorV4::Geometry)?;
    let evidence_start = cursor;
    let evidence_count = span_counts
        .get(7)
        .copied()
        .ok_or(DealerScenarioAccountProfileErrorV4::Geometry)?;
    cursor = cursor
        .checked_add(evidence_count)
        .ok_or(DealerScenarioAccountProfileErrorV4::Geometry)?;
    let scratch_start = cursor;
    let scratch_count = span_counts
        .get(8)
        .copied()
        .ok_or(DealerScenarioAccountProfileErrorV4::Geometry)?;
    cursor = cursor
        .checked_add(scratch_count)
        .ok_or(DealerScenarioAccountProfileErrorV4::Geometry)?;
    let logical_account_count = cursor;
    Ok(DealerScenarioLogicalFrameV4 {
        custody_starts,
        claims_fixed_start,
        claims_positions_start,
        obligation,
        evidence_start,
        evidence_count,
        scratch_start,
        scratch_count,
        logical_account_count,
    })
}

/// Encode the sole selector-9 AccountProfile13 atomically.
#[cfg(not(target_os = "solana"))]
pub fn encode_dealer_scenario_account_profile_v4_atomic(
    input: DealerScenarioAccountProfileInputV4,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DealerScenarioAccountProfileErrorV4> {
    if scratch.len() != DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4
        || output.len() != DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4
        || usize::try_from(input.common_data_lengths[1]).ok() != Some(DEALER_CONFIG_BYTES_V4)
        || OBLIGATION_V4 != 25
        || DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3 != 14
    {
        return Err(DealerScenarioAccountProfileErrorV4::Geometry);
    }
    let fixed_rules = fixed_rules(input)?;
    let span_rules = span_rules()?;
    let spans = dynamic_spans();
    let operations = [
        AccountOperationInputV2::ProjectTailCountU32 {
            account: AccountCoordinateV2::fixed(2),
            destination: dclutch_account_profile_contract::v2::encode::ScalarCoordinateV2::common(
                super::v3_trade_artifacts::DEALER_SCENARIO_MAX_POSITION_COUNT_SCALAR_V4,
            ),
            data_offset: u32::try_from(BASIS_WIDTH_OFFSET_V3)
                .map_err(|_| DealerScenarioAccountProfileErrorV4::Geometry)?,
        },
        AccountOperationInputV2::RequireOwner {
            account: AccountCoordinateV2::fixed(OBLIGATION_V4),
            expected: IdentityCoordinateV2::common(DEALER_SCENARIO_CURRENT_TRADING_IDENTITY_V4),
        },
        AccountOperationInputV2::RequireKey {
            account: AccountCoordinateV2::fixed(OBLIGATION_V4),
            expected: IdentityCoordinateV2::common(DEALER_SCENARIO_OBLIGATION_IDENTITY_V4),
        },
    ];
    encode_account_profile_with_dynamic_fixed_span_v2_atomic(
        TrustedEnvironmentV2::CurrentSlot {
            destination: DEALER_SCENARIO_CURRENT_SLOT_SCALAR_V4,
        },
        TrustedIdentityEnvironmentV2::CurrentExecutingProgram {
            destination: DEALER_SCENARIO_CURRENT_TRADING_IDENTITY_V4,
        },
        TrustedBuiltinIdentityV2::None,
        &spans,
        &fixed_rules,
        &span_rules,
        &operations,
        RegisterGeometryV2 {
            common_scalars: DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4,
            item_scalar_stride: DEALER_SCENARIO_ITEM_SCALAR_STRIDE_V4,
            common_identities: DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4,
            item_identity_stride: DEALER_SCENARIO_ITEM_IDENTITY_STRIDE_V4,
        },
        scratch,
        output,
    )
    .map_err(|_| DealerScenarioAccountProfileErrorV4::Profile)?;
    let profile = AccountProfileV2::decode(output)
        .map_err(|_| DealerScenarioAccountProfileErrorV4::Profile)?;
    if profile.artifact_profile() != DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
        || profile.fixed_account_count()
            != u16::try_from(DEALER_SCENARIO_PROFILE_FIXED_RULES_V4)
                .map_err(|_| DealerScenarioAccountProfileErrorV4::Geometry)?
        || profile.item_account_stride()
            != u16::try_from(DEALER_SCENARIO_PROFILE_SPAN_RULES_V4)
                .map_err(|_| DealerScenarioAccountProfileErrorV4::Geometry)?
        || profile.dynamic_fixed_span_count()
            != u16::try_from(DEALER_SCENARIO_PROFILE_SPANS_V4)
                .map_err(|_| DealerScenarioAccountProfileErrorV4::Geometry)?
    {
        return Err(DealerScenarioAccountProfileErrorV4::Profile);
    }
    Ok(())
}

#[cfg(not(target_os = "solana"))]
fn fixed_rules(
    input: DealerScenarioAccountProfileInputV4,
) -> Result<
    [AccountRuleWithPrestateInputV2; DEALER_SCENARIO_PROFILE_FIXED_RULES_V4],
    DealerScenarioAccountProfileErrorV4,
> {
    let mut rules = [exact(readonly(), none(), 0, 0); DEALER_SCENARIO_PROFILE_FIXED_RULES_V4];
    for (rule, length) in rules
        .get_mut(..5)
        .ok_or(DealerScenarioAccountProfileErrorV4::Geometry)?
        .iter_mut()
        .zip(input.common_data_lengths)
    {
        rule.rule.data_length = length;
    }
    rule_mut(&mut rules, 0)?.rule.privileges = writable();
    for offset in 0..SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3 {
        *rule_mut(&mut rules, usize::from(CLAIMS_START_V4 + offset))? =
            opaque(claims_privileges(offset)?);
    }
    for (offset, representative) in [
        (CLAIMS_LINKED_BASIS_OFFSET_V4, 4_u16),
        (CLAIMS_PRODUCT_OFFSET_V4, 2_u16),
        (CLAIMS_PORTFOLIO_OFFSET_V4, 3_u16),
    ] {
        *rule_mut(&mut rules, usize::from(CLAIMS_START_V4 + offset))? =
            route_alias(readonly(), representative);
    }
    *rule_mut(&mut rules, usize::from(OBLIGATION_V4))? = exact(
        writable(),
        write_data(),
        u32::try_from(DEALER_OBLIGATION_HEADER_BYTES_V3)
            .map_err(|_| DealerScenarioAccountProfileErrorV4::Geometry)?,
        8,
    );
    Ok(rules)
}

#[cfg(not(target_os = "solana"))]
fn span_rules() -> Result<
    [AccountRuleWithPrestateInputV2; DEALER_SCENARIO_PROFILE_SPAN_RULES_V4],
    DealerScenarioAccountProfileErrorV4,
> {
    let mut rules = [opaque(readonly()); DEALER_SCENARIO_PROFILE_SPAN_RULES_V4];
    let starts = [0_usize, 14, 28, 42, 57, 71];
    let spec = CustodyFrameSpecV1::new(OperationV1::Transfer);
    for start in starts {
        for offset in 0..DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3 {
            let account = spec
                .account(offset)
                .map_err(|_| DealerScenarioAccountProfileErrorV4::Geometry)?;
            let privileges = account.privileges();
            *rule_mut(&mut rules, start + usize::from(offset))? = opaque(AccountPrivilegesV2::new(
                false,
                privileges.writable(),
                privileges.executable(),
            ));
        }
    }
    *rule_mut(&mut rules, 56)? = opaque(claims_position_privileges()?);
    *rule_mut(&mut rules, 85)? = opaque(readonly());
    *rule_mut(&mut rules, 86)? = opaque(readonly());
    Ok(rules)
}

#[cfg(not(target_os = "solana"))]
const fn dynamic_spans() -> [DynamicFixedSpanInputV2; DEALER_SCENARIO_PROFILE_SPANS_V4] {
    [
        span(
            5,
            DEALER_SCENARIO_ROUTE_SPAN_SCALAR_BASE_V4,
            0,
            14,
            0,
            14,
            14,
        ),
        span(
            5,
            DEALER_SCENARIO_ROUTE_SPAN_SCALAR_BASE_V4 + 1,
            14,
            14,
            0,
            14,
            14,
        ),
        span(
            5,
            DEALER_SCENARIO_ROUTE_SPAN_SCALAR_BASE_V4 + 2,
            28,
            14,
            0,
            14,
            14,
        ),
        span(
            5,
            DEALER_SCENARIO_ROUTE_SPAN_SCALAR_BASE_V4 + 3,
            42,
            14,
            0,
            14,
            14,
        ),
        span(25, DEALER_SCENARIO_POSITION_COUNT_SCALAR_V4, 56, 1, 1, 2, 1),
        span(
            25,
            DEALER_SCENARIO_ROUTE_SPAN_SCALAR_BASE_V4 + 4,
            57,
            14,
            0,
            14,
            14,
        ),
        span(
            25,
            DEALER_SCENARIO_ROUTE_SPAN_SCALAR_BASE_V4 + 5,
            71,
            14,
            0,
            14,
            14,
        ),
        span(
            26,
            DEALER_SCENARIO_EVIDENCE_SPAN_COUNT_SCALAR_V4,
            85,
            1,
            0,
            3,
            1,
        ),
        span(
            26,
            DEALER_SCENARIO_SCRATCH_PAGE_COUNT_SCALAR_V4,
            86,
            1,
            6,
            6,
            1,
        ),
    ]
}

#[cfg(not(target_os = "solana"))]
const fn span(
    insertion_coordinate: u16,
    count_scalar: u16,
    rule_start: u16,
    rule_stride: u16,
    minimum: u32,
    maximum: u32,
    step: u32,
) -> DynamicFixedSpanInputV2 {
    DynamicFixedSpanInputV2 {
        insertion_coordinate,
        count_scalar,
        rule_start,
        rule_stride,
        minimum,
        maximum,
        step,
    }
}

#[cfg(not(target_os = "solana"))]
fn claims_privileges(
    offset: u16,
) -> Result<AccountPrivilegesV2, DealerScenarioAccountProfileErrorV4> {
    let account = SignedDeltaFrameSpecV3::new(1)
        .and_then(|spec| spec.account(offset))
        .map_err(|_| DealerScenarioAccountProfileErrorV4::Geometry)?;
    let privileges = account.privileges();
    Ok(AccountPrivilegesV2::new(
        false,
        privileges.writable(),
        privileges.executable(),
    ))
}

#[cfg(not(target_os = "solana"))]
fn claims_position_privileges() -> Result<AccountPrivilegesV2, DealerScenarioAccountProfileErrorV4>
{
    claims_privileges(SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3)
}

#[cfg(not(target_os = "solana"))]
const fn exact(
    privileges: AccountPrivilegesV2,
    effect_permissions: AccountEffectPermissionsV2,
    data_length: u32,
    data_item_stride: u32,
) -> AccountRuleWithPrestateInputV2 {
    AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges,
            effect_permissions,
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length,
            data_item_stride,
        },
        prestate: AccountPrestateV2::Exact,
    }
}

#[cfg(not(target_os = "solana"))]
const fn opaque(privileges: AccountPrivilegesV2) -> AccountRuleWithPrestateInputV2 {
    AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges,
            effect_permissions: none(),
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length: 0,
            data_item_stride: 0,
        },
        prestate: AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
    }
}

#[cfg(not(target_os = "solana"))]
const fn route_alias(
    privileges: AccountPrivilegesV2,
    representative: u16,
) -> AccountRuleWithPrestateInputV2 {
    AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges,
            effect_permissions: none(),
            alias: AccountAliasInputV2::Fixed(representative),
            data_length: 0,
            data_item_stride: 0,
        },
        prestate: AccountPrestateV2::AuthenticatedRouteAlias,
    }
}

#[cfg(not(target_os = "solana"))]
const fn readonly() -> AccountPrivilegesV2 {
    AccountPrivilegesV2::new(false, false, false)
}

#[cfg(not(target_os = "solana"))]
const fn writable() -> AccountPrivilegesV2 {
    AccountPrivilegesV2::new(false, true, false)
}

#[cfg(not(target_os = "solana"))]
const fn none() -> AccountEffectPermissionsV2 {
    AccountEffectPermissionsV2::new(false, false, false)
}

#[cfg(not(target_os = "solana"))]
const fn write_data() -> AccountEffectPermissionsV2 {
    AccountEffectPermissionsV2::new(false, false, true)
}

#[cfg(not(target_os = "solana"))]
fn rule_mut<const N: usize>(
    rules: &mut [AccountRuleWithPrestateInputV2; N],
    coordinate: usize,
) -> Result<&mut AccountRuleWithPrestateInputV2, DealerScenarioAccountProfileErrorV4> {
    rules
        .get_mut(coordinate)
        .ok_or(DealerScenarioAccountProfileErrorV4::Geometry)
}

#[cfg(all(test, not(target_os = "solana")))]
mod tests {
    use super::*;

    fn profile() -> Vec<u8> {
        let mut scratch = vec![0; DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4];
        let mut output = vec![0; DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4];
        encode_dealer_scenario_account_profile_v4_atomic(
            DealerScenarioAccountProfileInputV4 {
                common_data_lengths: [64, 128, 96, 112, 128],
            },
            &mut scratch,
            &mut output,
        )
        .expect("profile");
        output
    }

    #[test]
    fn selector_nine_profile_owns_all_nine_spans() {
        let bytes = profile();
        let profile = AccountProfileV2::decode(&bytes).expect("decode");
        assert_eq!(
            usize::from(profile.dynamic_fixed_span_count()),
            DEALER_SCENARIO_PROFILE_SPANS_V4
        );
        for (index, expected) in dynamic_spans().iter().copied().enumerate() {
            let observed = profile
                .dynamic_fixed_span(u16::try_from(index).expect("index"))
                .expect("span");
            assert_eq!(
                observed.insertion_coordinate(),
                expected.insertion_coordinate
            );
            assert_eq!(observed.count_scalar(), expected.count_scalar);
            assert_eq!(observed.rule_start(), expected.rule_start);
            assert_eq!(observed.rule_stride(), expected.rule_stride);
            assert_eq!(observed.minimum(), expected.minimum);
            assert_eq!(observed.maximum(), expected.maximum);
            assert_eq!(observed.step(), expected.step);
        }
        assert_eq!(profile.trusted_current_slot_scalar(), Some(3));
        assert_eq!(
            profile.trusted_current_executing_program_identity(),
            Some(DEALER_SCENARIO_CURRENT_TRADING_IDENTITY_V4)
        );
    }

    #[test]
    fn exact_widths_shift_claims_and_obligation_without_placeholders() {
        let bytes = profile();
        let profile = AccountProfileV2::decode(&bytes).expect("decode");
        let sparse = [14_u32, 0, 0, 0, 1, 0, 14, 3, 6];
        assert_eq!(
            profile.logical_account_count_with_dynamic_spans(16, &sparse),
            Ok(64)
        );
        assert_eq!(
            profile.physical_account_count_with_dynamic_spans(16, &sparse),
            Ok(61)
        );
        let full = [14_u32, 14, 14, 14, 2, 14, 14, 0, 6];
        assert_eq!(
            profile.logical_account_count_with_dynamic_spans(16, &full),
            Ok(118)
        );
        assert_eq!(
            profile.physical_account_count_with_dynamic_spans(16, &full),
            Ok(115)
        );
        let obligation = profile.rule(false, OBLIGATION_V4).expect("obligation");
        assert_eq!(
            obligation.data_length(),
            u32::try_from(DEALER_OBLIGATION_HEADER_BYTES_V3).expect("header")
        );
        assert_eq!(obligation.data_item_stride(), 8);
        assert_eq!(obligation.effect_permissions(), 4);
    }

    #[test]
    fn intermediate_custody_width_and_zero_claim_positions_refuse() {
        let bytes = profile();
        let profile = AccountProfileV2::decode(&bytes).expect("decode");
        assert!(
            profile
                .logical_account_count_with_dynamic_spans(8, &[1, 0, 0, 0, 1, 0, 0, 3, 6])
                .is_err()
        );
        assert!(
            profile
                .logical_account_count_with_dynamic_spans(8, &[0, 0, 0, 0, 0, 0, 0, 4, 6])
                .is_err()
        );
    }

    #[test]
    fn synthetic_or_legacy_config_width_refuses() {
        let mut scratch = vec![0; DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4];
        let mut output = vec![0; DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4];
        assert_eq!(
            encode_dealer_scenario_account_profile_v4_atomic(
                DealerScenarioAccountProfileInputV4 {
                    common_data_lengths: [64, 160, 96, 112, 128],
                },
                &mut scratch,
                &mut output,
            ),
            Err(DealerScenarioAccountProfileErrorV4::Geometry)
        );
        assert!(output.iter().all(|byte| *byte == 0));
    }

    #[test]
    fn child_caller_authorities_are_outer_nonsigners() {
        let bytes = profile();
        let profile = AccountProfileV2::decode(&bytes).expect("decode");
        assert_eq!(
            profile.rule(true, 0).expect("custody caller").privileges() & 1,
            0
        );
        assert_eq!(
            profile
                .rule(false, CLAIMS_START_V4)
                .expect("Claims caller")
                .privileges()
                & 1,
            0
        );
    }

    #[test]
    fn logical_frame_map_tracks_every_optional_span_at_n_boundaries() {
        let sparse =
            dealer_scenario_logical_frame_v4([0, 0, 0, 0, 1, 0, 0, 3, 6]).expect("sparse frame");
        assert_eq!(sparse.custody_starts, [5, 5, 5, 5, 26, 26]);
        assert_eq!(sparse.claims_fixed_start, 5);
        assert_eq!(sparse.claims_positions_start, 25);
        assert_eq!(sparse.obligation, 26);
        assert_eq!(sparse.evidence_start, 27);
        assert_eq!(sparse.evidence_count, 3);
        assert_eq!(sparse.scratch_start, 30);
        assert_eq!(sparse.scratch_count, 6);
        assert_eq!(sparse.logical_account_count, 36);

        let dense = dealer_scenario_logical_frame_v4([14, 14, 14, 14, 2, 14, 14, 0, 6])
            .expect("dense frame");
        assert_eq!(dense.custody_starts, [5, 19, 33, 47, 83, 97]);
        assert_eq!(dense.claims_fixed_start, 61);
        assert_eq!(dense.claims_positions_start, 81);
        assert_eq!(dense.obligation, 111);
        assert_eq!(dense.evidence_start, 112);
        assert_eq!(dense.evidence_count, 0);
        assert_eq!(dense.scratch_start, 112);
        assert_eq!(dense.scratch_count, 6);
        assert_eq!(dense.logical_account_count, 118);

        let bytes = profile();
        let profile = AccountProfileV2::decode(&bytes).expect("decode");
        for width in [1, 16] {
            assert_eq!(
                profile.logical_account_count_with_dynamic_spans(
                    width,
                    &[14, 14, 14, 14, 2, 14, 14, 0, 6],
                ),
                Ok(usize::try_from(dense.logical_account_count).expect("logical width"))
            );
        }
    }

    #[test]
    fn logical_frame_map_refuses_caller_invented_span_shapes() {
        for hostile in [
            [1, 0, 0, 0, 1, 0, 0, 3, 6],
            [0, 0, 0, 0, 0, 0, 0, 4, 6],
            [0, 0, 0, 0, 3, 0, 0, 0, 6],
            [0, 0, 0, 0, 1, 7, 0, 1, 6],
            [0, 0, 0, 0, 1, 0, 0, 0, 5],
        ] {
            assert_eq!(
                dealer_scenario_logical_frame_v4(hostile),
                Err(DealerScenarioAccountProfileErrorV4::Geometry)
            );
        }
    }
}
