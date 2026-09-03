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
// Re-exported rather than merely imported: this width is not incidental to the
// profile, it is a CONDITION of it -- the encoder refuses unless the config
// account's declared length equals it. A host test that wants to build a valid
// width vector needs to say which number that is, and the alternative is
// retyping 128 in a test, which is how a second author starts.
#[cfg(not(target_os = "solana"))]
pub use dclutch_dealer_codec::config_v4::DEALER_CONFIG_BYTES_V4;
#[cfg(not(target_os = "solana"))]
use dclutch_product_runtime_v2_svm_reader::BASIS_WIDTH_OFFSET_V3;

use dclutch_dealer_codec::generated_scenario_trade_v4::DEALER_SCENARIO_TRADE_ROUTE_SPAN_COUNT_V4;

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
        DEALER_SCENARIO_ITEM_SCALAR_STRIDE_V4, DEALER_SCENARIO_OBSERVED_OBLIGATION_IDENTITY_V4,
        DEALER_SCENARIO_POSITION_COUNT_SCALAR_V4, DEALER_SCENARIO_ROUTE_SPAN_SCALAR_BASE_V4,
        DEALER_SCENARIO_SCRATCH_PAGE_COUNT_SCALAR_V4,
    },
};

/// Number of fixed base rules before protected spans are inserted.
pub const DEALER_SCENARIO_PROFILE_FIXED_RULES_V4: usize = 27;
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
/// The release-selected Custody program the six Custody routes are invoked
/// through.
///
/// A CPI's callee is not a member of its own account list and
/// `CustodyFrameRoleV1` has no `CustodyProgram` variant, so none of the six
/// optional Custody frames can carry it. `SignedDeltaFrameSpecV3` declares
/// `ClaimsProgram` at its own relative 16, which is why the Claims route always
/// resolved and the Custody routes never could. It is appended after the
/// obligation, past every route span, so no earlier coordinate moved: the two
/// trailing spans that used to insert at 26 now insert at 27.
const CUSTODY_PROGRAM_V4: u16 = OBLIGATION_V4 + 1;
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
    /// Release-selected Custody program the Custody routes are invoked through.
    pub custody_program: u32,
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

/// Derive selector 9's nine AccountProfile span widths from the request that
/// declares them.
///
/// # The route this makes live
///
/// `authenticate_accelerator_witness_v4` refuses any witness whose span bank is
/// nonempty, so `AuthenticatedAcceleratorInvocationV4::span_widths` is EMPTY on
/// every admitted invocation. The selector-9 evaluator's first substantive act
/// was `span_widths().try_into::<[u32; 9]>()`, which therefore failed on every
/// input: **the Dealer scenario family was unconditionally refused by the
/// admitted accelerator, with `Invocation`, and nothing was red because nothing
/// exercised that route through it** (`7ef3c82c0`, sixth addendum).
///
/// # Why this derives rather than reads
///
/// A span width shapes the account frame the transition evaluates over, so it
/// is an evaluation INPUT, and `742d7b7be` forbids by name a request field that
/// carries one: *"a request field that carried an evaluation INPUT rather than
/// an authentication RESULT would make the accelerator a mirror of its
/// caller."* A caller-signed span bank is the caller's word, and the fix for
/// this route is therefore a derivation on the accelerator's side rather than a
/// binding on the caller's bank.
///
/// This IS that derivation, and it is the same one common Trading performs.
/// `authenticate_dynamic_span_widths_v3` projects the family request through
/// the RequestProfile and reads the nine span selectors out of the projected
/// scalars; `f5d4912e` put the six optional-Custody route widths in the request
/// header at 384..389 precisely so that phase could reach them, and the
/// RequestProfile writes them into common scalars 7..12 unchanged. The other
/// three selectors are the Claims position count, the trailing evidence count
/// and the fixed scratch-page count, and the first two are header fields of the
/// same request. So the nine widths are a total function of the header, and
/// this reproduces it without a projection.
///
/// **Nothing is taken on the caller's word by doing so.** Every width the
/// caller can state is then checked twice on this side: by
/// [`dealer_scenario_logical_frame_v4`], which admits only `{0, 14}` per
/// Custody route, one or two Claims positions, zero through three evidence
/// coordinates and exactly six scratch pages; and by the evaluator's own
/// `frame.logical_account_count == runtime.len()` conjunct, which pins the
/// total against the runtime slice the accelerator was actually handed and
/// whose bytes it hashes for itself. A header that lied about a width would
/// name a frame of a different total and refuse there, and a header that lied
/// consistently would still have to make every per-coordinate identity join
/// below succeed at the moved coordinates.
pub fn dealer_scenario_span_widths_v4(
    route_span_counts: [u8; DEALER_SCENARIO_TRADE_ROUTE_SPAN_COUNT_V4],
    claims_position_count: u8,
    evidence_span_count: u8,
) -> [u32; DEALER_SCENARIO_PROFILE_SPANS_V4] {
    // The six route widths occupy profile span indices 0, 1, 2, 3, 5 and 6 --
    // the Claims position span sits at 4, between the fourth and fifth route,
    // because its insertion coordinate (56) falls between theirs. The order is
    // the scalar order: span i reads `DEALER_SCENARIO_ROUTE_SPAN_SCALAR_BASE_V4
    // + i` for i in 0..6.
    [
        u32::from(route_span_counts[0]),
        u32::from(route_span_counts[1]),
        u32::from(route_span_counts[2]),
        u32::from(route_span_counts[3]),
        u32::from(claims_position_count),
        u32::from(route_span_counts[4]),
        u32::from(route_span_counts[5]),
        u32::from(evidence_span_count),
        DEALER_SCENARIO_SCRATCH_PAGE_COUNT_V4,
    ]
}

/// The one width the profile fixes rather than projects: `span(27, .., 6, 6, 1)`.
pub const DEALER_SCENARIO_SCRATCH_PAGE_COUNT_V4: u32 = 6;

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
    let custody_program = obligation
        .checked_add(1)
        .ok_or(DealerScenarioAccountProfileErrorV4::Geometry)?;
    cursor = custody_program
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
        custody_program,
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
        || CUSTODY_PROGRAM_V4 != 26
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
        // NOT a `RequireKey` against DEALER_SCENARIO_OBLIGATION_IDENTITY_V4.
        // `OP_REQUIRE_*` reads the INPUT identity bank; `OP_PROJECT_*` writes a
        // separate output bank; and that register is written by the REQUEST
        // profile, which runs after this pass (`project_accounts_atomic` ->
        // swap -> `request_profile.project_atomic`). The guard that stood here
        // compared the obligation key against 32 unwritten zero bytes, so
        // selector 9 was unsatisfiable by any account list at all. The law it
        // stated is real and moves to the bank that can hold both values: the
        // emitted transition carries
        // `identity_eq(OBLIGATION, OBSERVED_OBLIGATION)`.
        AccountOperationInputV2::ProjectKey {
            account: AccountCoordinateV2::fixed(OBLIGATION_V4),
            destination: IdentityCoordinateV2::common(
                DEALER_SCENARIO_OBSERVED_OBLIGATION_IDENTITY_V4,
            ),
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
    // Readonly executable, no effect permission, no asserted width: the loader
    // that deployed it owns the record and the Registry activation cache, not
    // this profile, is the sole authority on which program the Custody role
    // selects.
    *rule_mut(&mut rules, usize::from(CUSTODY_PROGRAM_V4))? = opaque(executable());
    Ok(rules)
}

/// The five roles a Custody transfer frame and the Claims fixed frame both name.
///
/// `(Custody frame offset, Claims frame offset)` for the Core Market, the
/// release activation cache, the Registry program, the caller program and its
/// ProgramData. `CustodyFrameSpecV1`'s common prefix carries them at 1..=5 and
/// `SignedDeltaFrameSpecV3` at 11..=15, and in one selector-9 execution each
/// pair is ONE account: the same Core Market, the same activation cache, the
/// same Registry, and Trading itself as the caller of both children.
///
/// Undeclared, an active route span presents five second representatives of
/// accounts the Claims frame already names, and the account projection refuses
/// `CrossItemAlias` -- measured at `c4b1c5b3` as coordinates 16/28, 17/29,
/// 18/30, 19/31 and 20/32 of the P2 Dealer-pays-counterparty frame.
#[cfg(not(target_os = "solana"))]
const DEALER_SCENARIO_SHARED_CUSTODY_CLAIMS_ROLES_V4: [(u16, u16); 5] =
    [(1, 11), (2, 12), (3, 13), (4, 14), (5, 15)];

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
    // The cross-frame alias partition, declared here and authenticated by the
    // projection. A route span borrows the Claims coordinate rather than
    // observing the account a second time on its own authority, so the alias
    // check the projection then runs -- key, owner, lamports, data and
    // privileges equal to the representative's -- is strictly more than the
    // opaque self-coordinate it replaces, which authenticated no key at all.
    //
    // The condition is the tree's backward-alias rule, read off the span table
    // instead of hard-coded: a span inserted at `insertion_coordinate` sits
    // after every base coordinate below that point and before every one at or
    // after it, so only a route span inserted past the Claims frame can name a
    // Claims coordinate as its representative. Today that is routes 4 and 5
    // (insertion 25); routes 0 through 3 insert at 5, ahead of the frame they
    // would borrow from, and a trade that enables one of them still refuses
    // `CrossItemAlias` at these five pairs. `route_spans_declare_the_alias_they_can_reach`
    // pins which spans carry the declaration and says what closing the rest costs.
    let spans = dynamic_spans();
    for start in starts {
        let insertion = spans
            .iter()
            .find(|span| usize::from(span.rule_start) == start)
            .ok_or(DealerScenarioAccountProfileErrorV4::Geometry)?
            .insertion_coordinate;
        for (custody_offset, claims_offset) in DEALER_SCENARIO_SHARED_CUSTODY_CLAIMS_ROLES_V4 {
            let representative = CLAIMS_START_V4
                .checked_add(claims_offset)
                .ok_or(DealerScenarioAccountProfileErrorV4::Geometry)?;
            if representative >= insertion {
                continue;
            }
            *rule_mut(&mut rules, start + usize::from(custody_offset))? =
                route_alias(readonly(), representative);
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
            27,
            DEALER_SCENARIO_EVIDENCE_SPAN_COUNT_SCALAR_V4,
            85,
            1,
            0,
            3,
            1,
        ),
        span(
            27,
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
const fn executable() -> AccountPrivilegesV2 {
    AccountPrivilegesV2::new(false, false, true)
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
    // The register the REQUEST profile authors. The account pass no longer
    // names it -- that is the repair -- so only these witnesses read it.
    use super::super::v3_trade_artifacts::DEALER_SCENARIO_OBLIGATION_IDENTITY_V4;

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

    /// The accelerator's derivation is the PROFILE's derivation, checked
    /// against it rather than restated beside it.
    ///
    /// `dealer_scenario_span_widths_v4` exists because the admitted accelerator
    /// has no projected scalar bank to run `dynamic_span_widths_from_scalars`
    /// over, so it reproduces the same nine numbers from the request header the
    /// RequestProfile projects INTO that bank. A test that merely re-listed the
    /// mapping would pass for any mapping; this one builds the scalar bank the
    /// RequestProfile would have written, asks the profile itself for the nine
    /// widths, and requires the two to agree -- so a span reordered in
    /// `dynamic_spans()` fails here rather than in a campaign.
    #[test]
    fn the_accelerators_span_derivation_agrees_with_the_profiles_own() {
        let bytes = profile();
        let profile = AccountProfileV2::decode(&bytes).expect("decode");
        let cases: [([u8; DEALER_SCENARIO_TRADE_ROUTE_SPAN_COUNT_V4], u8, u8); 5] = [
            ([0, 0, 0, 0, 0, 0], 1, 0),
            ([14, 0, 0, 0, 0, 0], 1, 3),
            ([0, 0, 14, 14, 0, 0], 2, 1),
            ([0, 14, 0, 0, 14, 0], 2, 2),
            ([14, 14, 14, 14, 14, 14], 2, 3),
        ];
        for (route_span_counts, claims_position_count, evidence_span_count) in cases {
            let derived = dealer_scenario_span_widths_v4(
                route_span_counts,
                claims_position_count,
                evidence_span_count,
            );
            // The bank the RequestProfile writes: the six route widths at
            // scalars 7..12 in slot order, the two counted tails at their own
            // selectors, and the fixed scratch width at its.
            let mut scalars = vec![0_u64; usize::from(DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4)];
            for (slot, width) in route_span_counts.iter().copied().enumerate() {
                scalars[usize::from(DEALER_SCENARIO_ROUTE_SPAN_SCALAR_BASE_V4) + slot] =
                    u64::from(width);
            }
            scalars[usize::from(DEALER_SCENARIO_POSITION_COUNT_SCALAR_V4)] =
                u64::from(claims_position_count);
            scalars[usize::from(DEALER_SCENARIO_EVIDENCE_SPAN_COUNT_SCALAR_V4)] =
                u64::from(evidence_span_count);
            scalars[usize::from(DEALER_SCENARIO_SCRATCH_PAGE_COUNT_SCALAR_V4)] =
                u64::from(DEALER_SCENARIO_SCRATCH_PAGE_COUNT_V4);
            let mut widths = [0_u32; DEALER_SCENARIO_PROFILE_SPANS_V4];
            profile
                .dynamic_span_widths_from_scalars(&scalars, &mut widths)
                .expect("the profile admits the bank the RequestProfile writes");
            assert_eq!(
                widths, derived,
                "profile widths and the accelerator's derivation must agree"
            );
            // And the geometry the evaluator then carves accepts exactly these.
            dealer_scenario_logical_frame_v4(derived).expect("admissible geometry");
        }
    }

    /// A header the geometry does not admit refuses, so the derivation cannot
    /// carve a frame the profile would never have produced.
    #[test]
    fn a_route_width_the_profile_forbids_is_refused_by_the_geometry() {
        // Thirteen is not `{0, 14}`; three Claims positions is not `1 | 2`;
        // four evidence coordinates is not `0..=3`.
        for (route_span_counts, positions, evidence) in [
            ([13_u8, 0, 0, 0, 0, 0], 1_u8, 0_u8),
            ([0, 0, 0, 0, 0, 0], 3, 0),
            ([0, 0, 0, 0, 0, 0], 1, 4),
        ] {
            let derived = dealer_scenario_span_widths_v4(route_span_counts, positions, evidence);
            assert!(
                dealer_scenario_logical_frame_v4(derived).is_err(),
                "the geometry must refuse {derived:?}"
            );
        }
    }

    #[test]
    fn exact_widths_shift_claims_and_obligation_without_placeholders() {
        let bytes = profile();
        let profile = AccountProfileV2::decode(&bytes).expect("decode");
        // Every count is one higher than before the Custody callee coordinate
        // existed, and no coordinate before it moved.
        // The logical widths are unchanged by the cross-frame alias partition:
        // the borrowed coordinates stay in the frame and stay observed. What
        // moves is the PHYSICAL count, by exactly five per route span that
        // declares the five shared Claims roles -- routes 4 and 5 -- so one
        // active late route drops 62 to 57 and all six drop 116 to 106.
        let sparse = [14_u32, 0, 0, 0, 1, 0, 14, 3, 6];
        assert_eq!(
            profile.logical_account_count_with_dynamic_spans(16, &sparse),
            Ok(65)
        );
        assert_eq!(
            profile.physical_account_count_with_dynamic_spans(16, &sparse),
            Ok(57)
        );
        let full = [14_u32, 14, 14, 14, 2, 14, 14, 0, 6];
        assert_eq!(
            profile.logical_account_count_with_dynamic_spans(16, &full),
            Ok(119)
        );
        assert_eq!(
            profile.physical_account_count_with_dynamic_spans(16, &full),
            Ok(106)
        );
        let obligation = profile.rule(false, OBLIGATION_V4).expect("obligation");
        assert_eq!(
            obligation.data_length(),
            u32::try_from(DEALER_OBLIGATION_HEADER_BYTES_V3).expect("header")
        );
        assert_eq!(obligation.data_item_stride(), 8);
        assert_eq!(obligation.effect_permissions(), 4);
    }

    /// Which route spans carry the cross-frame alias, and what the rest cost.
    ///
    /// The campaign's P2 Dealer-pays-counterparty frame is `[0, 0, 0, 0, 2, 0,
    /// 14, 2, 6]`: route 5 alone, inserted at base 25, so its five shared roles
    /// resolve to the Claims frame's own coordinates 16..=20 instead of
    /// standing as second representatives. Routes 0 through 3 insert at base 5,
    /// AHEAD of the frame they would borrow, and the tree's alias partitions
    /// are backward, so they cannot declare it and a trade that enables one
    /// still refuses `CrossItemAlias` at exactly these five pairs.
    ///
    /// Two walls remain and they close together, not separately:
    ///
    /// * routes 0..=3 are inserted before their representative;
    /// * two simultaneously active route spans share seven MORE roles -- caller
    ///   authority, Realm record, Realm staging, replay, mint, Custody
    ///   authority, token program -- whose representative would itself be
    ///   runtime-inserted, and `AliasKindV2::Fixed` names base coordinates only.
    ///
    /// One shape answers both: the Custody frame's twelve non-endpoint roles
    /// become fixed coordinates ahead of every span, and each route span
    /// carries only its source/destination pair. That is a frame move, and it
    /// is not this declaration's to make.
    ///
    /// WHAT THAT SHAPE IS AND IS NOT WORTH, derived from the counts pinned in
    /// `exact_widths_shift_claims_and_obligation_without_placeholders` above,
    /// because the price was about to be paid for the wrong reason. The twelve
    /// are Custody Transfer offsets 0..=9, 12 and 13 -- the endpoints are 10
    /// and 11 -- and only SEVEN of them are new fixed coordinates: offsets
    /// 1..=5 are the Claims frame's own 11..=15 and are already here. So the
    /// base grows by seven physical coordinates and every active route span
    /// loses seven more, which for ONE active late route is exactly zero:
    /// the campaign's `[0, 0, 0, 0, 2, 0, 14, 2, 6]` is 43 physical before and
    /// 43 after, and its authentications are 33 either way, so page 1's CU does
    /// not move either. The shape pays where two or more routes are live, and
    /// it pays enormously: all six go 106 physical to 51, and routes 0..=3
    /// become able to declare at all, which is the whole P1 direction. It is
    /// capability work and multi-route work, not a lock reduction for the trade
    /// this campaign runs, and a lane that reads it as the latter will measure
    /// nothing and conclude the shape is broken.
    ///
    /// It also has a cost the sketch does not name: the twelve are
    /// UNCONDITIONAL, so a selector-9 trade with no Custody route at all
    /// carries seven coordinates it does not use. A span cannot be a
    /// representative -- `AliasKindV2::Fixed` names base coordinates only --
    /// so there is no conditional form of it.
    #[test]
    fn route_spans_declare_the_alias_they_can_reach() {
        let bytes = profile();
        let decoded = AccountProfileV2::decode(&bytes).expect("decode");
        let campaign = [0_u32, 0, 0, 0, 2, 0, 14, 2, 6];
        let frame = dealer_scenario_logical_frame_v4(campaign).expect("campaign frame");
        let route_five = usize::try_from(frame.custody_starts[5]).expect("route 5 start");
        assert_eq!(route_five, 27);
        for (custody_offset, claims_offset) in DEALER_SCENARIO_SHARED_CUSTODY_CLAIMS_ROLES_V4 {
            let borrower = route_five + usize::from(custody_offset);
            let representative = usize::from(CLAIMS_START_V4 + claims_offset);
            assert_eq!(
                decoded.representative_with_dynamic_spans(0, &campaign, borrower),
                Ok(representative),
                "route 5 coordinate {borrower} borrows the Claims frame at {representative}",
            );
            assert_eq!(
                decoded.representative_with_dynamic_spans(0, &campaign, representative),
                Ok(representative),
                "the Claims coordinate is its own representative",
            );
        }
        // Route 0, the same five roles, one span earlier than the frame it
        // would borrow: still five distinct representatives.
        let early = [14_u32, 0, 0, 0, 2, 0, 0, 2, 6];
        let early_frame = dealer_scenario_logical_frame_v4(early).expect("route 0 frame");
        let route_zero = usize::try_from(early_frame.custody_starts[0]).expect("route 0 start");
        assert_eq!(route_zero, 5);
        for (custody_offset, _) in DEALER_SCENARIO_SHARED_CUSTODY_CLAIMS_ROLES_V4 {
            let coordinate = route_zero + usize::from(custody_offset);
            assert_eq!(
                decoded.representative_with_dynamic_spans(0, &early, coordinate),
                Ok(coordinate),
                "route 0 cannot backward-alias a frame inserted after it",
            );
        }
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
        assert_eq!(sparse.custody_program, 27);
        assert_eq!(sparse.evidence_start, 28);
        assert_eq!(sparse.evidence_count, 3);
        assert_eq!(sparse.scratch_start, 31);
        assert_eq!(sparse.scratch_count, 6);
        assert_eq!(sparse.logical_account_count, 37);

        let dense = dealer_scenario_logical_frame_v4([14, 14, 14, 14, 2, 14, 14, 0, 6])
            .expect("dense frame");
        assert_eq!(dense.custody_starts, [5, 19, 33, 47, 83, 97]);
        assert_eq!(dense.claims_fixed_start, 61);
        assert_eq!(dense.claims_positions_start, 81);
        assert_eq!(dense.obligation, 111);
        assert_eq!(dense.custody_program, 112);
        assert_eq!(dense.evidence_start, 113);
        assert_eq!(dense.evidence_count, 0);
        assert_eq!(dense.scratch_start, 113);
        assert_eq!(dense.scratch_count, 6);
        assert_eq!(dense.logical_account_count, 119);

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

    /// The account pass writes the obligation key into a register it OWNS.
    ///
    /// This reads the ENCODED artifact, not the builder's input array. A test
    /// against the builder's own array would be the builder as its own witness,
    /// which is the hazard this class is made of; `writes_register` is a static
    /// inspection of the bytes that ship.
    ///
    /// What stood here was `RequireKey` against
    /// `DEALER_SCENARIO_OBLIGATION_IDENTITY_V4`. `OP_REQUIRE_*` reads the INPUT
    /// identity bank; `OP_PROJECT_*` writes a separate output bank; and that
    /// register is written by the REQUEST profile, which runs after this pass
    /// (`project_accounts_atomic` -> swap -> `request_profile.project_atomic`).
    /// The guard therefore compared the obligation key against 32 unwritten
    /// zero bytes, and selector 9 was unsatisfiable by any account list at all.
    ///
    /// Convicted class, second instance: `4923625a` removed General's three.
    #[test]
    fn the_account_pass_projects_the_obligation_key_into_a_register_it_owns() {
        use dclutch_account_profile_contract::v2::{
            ProjectionRegisterKindV2, ProjectionRegisterSpaceV2, ProjectionTargetV2,
        };
        let bytes = profile();
        let decoded = AccountProfileV2::decode(&bytes).expect("decode");
        let identity = |index| ProjectionTargetV2 {
            kind: ProjectionRegisterKindV2::Identity,
            space: ProjectionRegisterSpaceV2::Common,
            index,
        };
        assert!(
            decoded
                .writes_register(identity(DEALER_SCENARIO_OBSERVED_OBLIGATION_IDENTITY_V4))
                .expect("writes_register"),
            "the account pass must project the obligation key it observes",
        );
        // And it must NOT claim to author the register the request profile owns:
        // one fact, two observers, two registers, joined in the transition.
        assert!(
            !decoded
                .writes_register(identity(DEALER_SCENARIO_OBLIGATION_IDENTITY_V4))
                .expect("writes_register"),
            "the request profile is the sole author of the requested obligation",
        );
    }

    /// The comparison the guard used to state now runs where both values exist.
    ///
    /// A projection with no comparison is worse than a guard that cannot hold:
    /// it is a fact nobody checks. `ProgramV3` exposes no instruction accessor,
    /// so this compares the ENCODED instruction record -- taken from a
    /// one-instruction reference program at the same geometry -- against the
    /// shipped transition's instruction region. Artifact against artifact.
    #[test]
    fn the_transition_compares_the_observed_obligation_to_the_requested_one() {
        use dclutch_transition_vm::v3::{
            HEADER_BYTES, INSTRUCTION_BYTES, IdentityRegisterV3, InstructionV3, ProgramGeometryV3,
            encode_program_atomic,
        };

        use crate::dealer::v3_trade_artifacts::{
            DEALER_SCENARIO_TRANSITION_BYTES_V4, encode_dealer_scenario_transition_v4,
        };
        let geometry = ProgramGeometryV3 {
            common_scalars: DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4,
            item_scalar_stride: DEALER_SCENARIO_ITEM_SCALAR_STRIDE_V4,
            common_identities: DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4,
            item_identity_stride: DEALER_SCENARIO_ITEM_IDENTITY_STRIDE_V4,
        };
        let reference_bytes = HEADER_BYTES + INSTRUCTION_BYTES;
        let mut reference_scratch = vec![0_u8; reference_bytes];
        let mut reference = vec![0_u8; reference_bytes];
        encode_program_atomic(
            geometry,
            &[InstructionV3::identity_eq(
                IdentityRegisterV3::common(DEALER_SCENARIO_OBLIGATION_IDENTITY_V4),
                IdentityRegisterV3::common(DEALER_SCENARIO_OBSERVED_OBLIGATION_IDENTITY_V4),
            )],
            &[],
            &[],
            &mut reference_scratch,
            &mut reference,
        )
        .expect("reference program");
        let wanted = reference
            .get(HEADER_BYTES..reference_bytes)
            .expect("reference instruction")
            .to_vec();
        assert_eq!(wanted.len(), INSTRUCTION_BYTES);

        let mut scratch = vec![0_u8; DEALER_SCENARIO_TRANSITION_BYTES_V4];
        let mut output = vec![0_u8; DEALER_SCENARIO_TRANSITION_BYTES_V4];
        encode_dealer_scenario_transition_v4(&mut scratch, &mut output).expect("transition");
        let found = output
            .get(HEADER_BYTES..)
            .expect("instruction region")
            .chunks_exact(INSTRUCTION_BYTES)
            .any(|instruction| instruction == wanted.as_slice());
        assert!(
            found,
            "the transition must compare the observed obligation to the one the request names",
        );
    }
}
