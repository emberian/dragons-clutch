//! The Series `StateLifecyclePolicyV5`: root-only, derived, lamport-silent.
//!
//! This is the artifact whose absence made every Series release inadmissible:
//! [`super::artifacts_v4::authenticate_series_consume_artifacts_v4`] decodes
//! the lifecycle policy under `CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5`, joins
//! it to the Series Consume Profile13, and refuses any policy whose
//! `action_plan_count(Consume)` is zero. The checklist this encoder satisfies
//! is [`super::release_v4::SERIES_CONSUME_LIFECYCLE_REQUIREMENTS_V4`], read
//! off that verifier — not the prose of any ruling.
//!
//! # What the ruling fixed (WAVE.md, 1b8228e9)
//!
//! **Coverage is the root, and only the root.** The one plan authenticates the
//! composite Trading root at coordinate zero. The Ticket replay account
//! appears in the Consume frame as a referenced coordinate only: its lamport
//! flow is authored by the funding path
//! ([`super::commit_plans::PendingFundingPlanV3::ticket_capability_refund`]),
//! and a policy claiming it is refused by the artifact join, not merely left
//! unwritten here.
//!
//! **Every pin is derived at emit time.** This encoder takes no widths, no
//! schemas, and no identities. The root data width is the sum of the two
//! constants the release set already binds (`CAPABILITY_ROOT_HEADER_BYTES_V1`
//! plus the `root_state_bytes` the descriptor pins to
//! `SERIES_STATE_BYTES_V3`); the rent-quote generation is the V5 schema the
//! verifier demands, carried with an empty quote table because a Consume plan
//! that creates nothing has no rent to quote. There is no supplied field for
//! a human to keep in sync.
//!
//! **The refund recipient is projected, not supplied.** The legacy Consume
//! Authenticate plan declares no payer, RentCredit, principal, or beneficiary
//! because it moves no lamports. The successor Close plan is the sole terminal
//! exception: it names the post-prefix RentCredit coordinate and consumes the
//! principal and beneficiary registers projected by its selected profile.
//! Generic Lifecycle then requires that beneficiary to match the authenticated
//! RentCredit account before crediting and closing the root.
//!
//! # Why the seed table is honest
//!
//! A recipe's seeds may only reference registers, and the root PDA is the
//! eight-seed `CapabilityRootSeedsV1` derivation over per-instance values. The
//! Series Profile13 projects exactly those values out of the root header's own
//! account data (see [`super::account_profile_v4`]), so every register-backed
//! seed below reads a value whose single author is the root's immutable
//! header. The generic adapter then derives the PDA from these seeds and
//! refuses unless it equals the account key at coordinate zero — the same
//! join `TradingFamilyContextV1::authenticate` performs from the header
//! directly.

use dclutch_account_profile_contract::lifecycle_v3::{
    ACTION_PLAN_BYTES, HEADER_BYTES, PROTECTED_OUTPUT_BYTES, RECIPE_BYTES, SEED_BYTES,
    StateLifecyclePolicyV5,
    encode::{
        LifecycleAccountCoordinateV3, LifecycleGuardInputV3, LifecycleOperationInputV3,
        LifecyclePlanInputV3, LifecycleRecipeInputV3, LifecycleRefundSourceInputV3,
        LifecycleRegisterCoordinateV3, LifecycleSeedInputV3, encode_lifecycle_policy_v5_atomic,
    },
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CAPABILITY_ROOT_PDA_DOMAIN_V1,
};
use dclutch_series_v3_kernel::request::SeriesActionV3;

use super::{
    account_profile_v4::{
        SERIES_CONSUME_ROOT_CAPABILITY_RELEASE_IDENTITY_V4, SERIES_CONSUME_ROOT_CONFIG_IDENTITY_V4,
        SERIES_CONSUME_ROOT_COORDINATE_V4, SERIES_CONSUME_ROOT_ENTRY_INDEX_SCALAR_V4,
        SERIES_CONSUME_ROOT_GENERATION_SCALAR_V4, SERIES_CONSUME_ROOT_KIND_IDENTITY_V4,
        SERIES_CONSUME_ROOT_MANIFEST_IDENTITY_V4, SERIES_CONSUME_ROOT_MARKET_IDENTITY_V4,
    },
    state::SERIES_STATE_BYTES_V3,
};

/// Exact composite Trading root account width the Authenticate plan pins.
///
/// Derived from the two constants the release set binds — never supplied.
pub const SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5: usize =
    CAPABILITY_ROOT_HEADER_BYTES_V1 + SERIES_STATE_BYTES_V3;

/// Exact canonical Series `StateLifecyclePolicyV5` width.
///
/// One recipe, the nine root-derivation seeds, one Consume plan, and that
/// plan's (absent) protected-output slot. No bindings, no rent quotes.
pub const SERIES_CONSUME_STATE_LIFECYCLE_BYTES_V5: usize = HEADER_BYTES
    + RECIPE_BYTES
    + SERIES_CONSUME_ROOT_SEED_COUNT_V5 * SEED_BYTES
    + ACTION_PLAN_BYTES
    + PROTECTED_OUTPUT_BYTES;

/// Exact width of the canonical policy selected by every live-root action.
pub const SERIES_EMPTY_STATE_LIFECYCLE_BYTES_V5: usize = HEADER_BYTES;

/// Exact seed count of the root recipe: the eight `CapabilityRootSeedsV1`
/// slices plus the canonical bump.
pub const SERIES_CONSUME_ROOT_SEED_COUNT_V5: usize = 9;

/// Close-only lifecycle RentCredit coordinate after the five-account Hot prefix.
pub const SERIES_CLOSE_RENT_CREDIT_COORDINATE_V5: u16 = 5;
/// Close-only projection of the root's historical rent principal.
pub const SERIES_CLOSE_ROOT_PRINCIPAL_SCALAR_V5: u16 = 7;
/// Close-only projection of the authenticated RentCredit beneficiary.
pub const SERIES_CLOSE_BENEFICIARY_IDENTITY_V5: u16 = 6;

/// Stable refusal from Series lifecycle-policy emission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesLifecyclePolicyErrorV5 {
    /// A checked width overflowed or a buffer was not exact.
    Geometry,
    /// The generic lifecycle encoder or its own V5 decoder refused.
    Lifecycle(dclutch_account_profile_contract::lifecycle_v3::Error),
}

/// Result alias for Series lifecycle-policy emission.
pub type Result<T> = core::result::Result<T, SeriesLifecyclePolicyErrorV5>;

/// Emit the canonical plan-free policy for Prepare, Consume, Expire, or Retire.
///
/// These actions authenticate the live root through the ordinary Hot root
/// join. FundingV5 separately owns any Ticket create/close, so Lifecycle must
/// not acquire a second account-authority path for the same action.
pub fn encode_series_empty_state_lifecycle_v5_atomic(
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    if scratch.len() != SERIES_EMPTY_STATE_LIFECYCLE_BYTES_V5
        || output.len() != SERIES_EMPTY_STATE_LIFECYCLE_BYTES_V5
    {
        return Err(SeriesLifecyclePolicyErrorV5::Geometry);
    }
    encode_lifecycle_policy_v5_atomic(&[], &[], &[], &[], &[], &[], scratch, output)
        .map_err(SeriesLifecyclePolicyErrorV5::Lifecycle)?;
    StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], output)
        .map_err(SeriesLifecyclePolicyErrorV5::Lifecycle)?;
    Ok(())
}

/// Emit the canonical Series Consume `StateLifecyclePolicyV5` atomically.
///
/// Takes nothing but buffers: every value below is a constant of the Series
/// release or a register the Profile13 derives from the root's own header.
pub fn encode_series_consume_state_lifecycle_v5_atomic(
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    if scratch.len() != SERIES_CONSUME_STATE_LIFECYCLE_BYTES_V5
        || output.len() != SERIES_CONSUME_STATE_LIFECYCLE_BYTES_V5
    {
        return Err(SeriesLifecyclePolicyErrorV5::Geometry);
    }
    let recipes = [LifecycleRecipeInputV3 {
        state: LifecycleAccountCoordinateV3::fixed(SERIES_CONSUME_ROOT_COORDINATE_V4),
        seed_start: 0,
        seed_count: u8::try_from(SERIES_CONSUME_ROOT_SEED_COUNT_V5)
            .map_err(|_| SeriesLifecyclePolicyErrorV5::Geometry)?,
        bump_offset: u8::try_from(SERIES_CONSUME_ROOT_SEED_COUNT_V5 - 1)
            .map_err(|_| SeriesLifecyclePolicyErrorV5::Geometry)?,
        data_base: u32::try_from(SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5)
            .map_err(|_| SeriesLifecyclePolicyErrorV5::Geometry)?,
        data_stride: 0,
    }];
    // Exact `CapabilityRootSeedsV1::as_slices` order. Every register named
    // here is written only by the Profile13 root-header projections.
    let seeds = [
        LifecycleSeedInputV3::Literal(CAPABILITY_ROOT_PDA_DOMAIN_V1),
        LifecycleSeedInputV3::CommonIdentity(SERIES_CONSUME_ROOT_MARKET_IDENTITY_V4),
        LifecycleSeedInputV3::CommonScalar {
            index: SERIES_CONSUME_ROOT_GENERATION_SCALAR_V4,
            width: 8,
        },
        LifecycleSeedInputV3::CommonIdentity(SERIES_CONSUME_ROOT_MANIFEST_IDENTITY_V4),
        LifecycleSeedInputV3::CommonScalar {
            index: SERIES_CONSUME_ROOT_ENTRY_INDEX_SCALAR_V4,
            width: 2,
        },
        LifecycleSeedInputV3::CommonIdentity(SERIES_CONSUME_ROOT_KIND_IDENTITY_V4),
        LifecycleSeedInputV3::CommonIdentity(SERIES_CONSUME_ROOT_CAPABILITY_RELEASE_IDENTITY_V4),
        LifecycleSeedInputV3::CommonIdentity(SERIES_CONSUME_ROOT_CONFIG_IDENTITY_V4),
        LifecycleSeedInputV3::CanonicalBump,
    ];
    let plans = [LifecyclePlanInputV3 {
        action: SeriesActionV3::Consume as u32,
        operation: LifecycleOperationInputV3::Authenticate,
        recipe: 0,
        payer: None,
        rent_credit: None,
        principal: None,
        beneficiary: None,
        refund_source: LifecycleRefundSourceInputV3::Credit,
        guard: LifecycleGuardInputV3::Always,
    }];
    encode_lifecycle_policy_v5_atomic(&recipes, &seeds, &plans, &[None], &[], &[], scratch, output)
        .map_err(SeriesLifecyclePolicyErrorV5::Lifecycle)?;
    StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], output)
        .map_err(SeriesLifecyclePolicyErrorV5::Lifecycle)?;
    Ok(())
}

/// Emit the canonical terminal Series root-close policy atomically.
///
/// This is the only reachable Series lifecycle plan. Live roots remain under
/// the ordinary Hot root/header authentication path; Prepare, Consume,
/// Expire, and Retire therefore select no lifecycle invocation. Close alone
/// credits every root lamport to the authenticated lifecycle RentCredit and
/// leaves Ticket retirement to the EffectV5 funding owner.
pub fn encode_series_close_state_lifecycle_v5_atomic(
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    if scratch.len() != SERIES_CONSUME_STATE_LIFECYCLE_BYTES_V5
        || output.len() != SERIES_CONSUME_STATE_LIFECYCLE_BYTES_V5
    {
        return Err(SeriesLifecyclePolicyErrorV5::Geometry);
    }
    let recipes = [LifecycleRecipeInputV3 {
        state: LifecycleAccountCoordinateV3::fixed(SERIES_CONSUME_ROOT_COORDINATE_V4),
        seed_start: 0,
        seed_count: u8::try_from(SERIES_CONSUME_ROOT_SEED_COUNT_V5)
            .map_err(|_| SeriesLifecyclePolicyErrorV5::Geometry)?,
        bump_offset: u8::try_from(SERIES_CONSUME_ROOT_SEED_COUNT_V5 - 1)
            .map_err(|_| SeriesLifecyclePolicyErrorV5::Geometry)?,
        data_base: u32::try_from(SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5)
            .map_err(|_| SeriesLifecyclePolicyErrorV5::Geometry)?,
        data_stride: 0,
    }];
    let seeds = [
        LifecycleSeedInputV3::Literal(CAPABILITY_ROOT_PDA_DOMAIN_V1),
        LifecycleSeedInputV3::CommonIdentity(SERIES_CONSUME_ROOT_MARKET_IDENTITY_V4),
        LifecycleSeedInputV3::CommonScalar {
            index: SERIES_CONSUME_ROOT_GENERATION_SCALAR_V4,
            width: 8,
        },
        LifecycleSeedInputV3::CommonIdentity(SERIES_CONSUME_ROOT_MANIFEST_IDENTITY_V4),
        LifecycleSeedInputV3::CommonScalar {
            index: SERIES_CONSUME_ROOT_ENTRY_INDEX_SCALAR_V4,
            width: 2,
        },
        LifecycleSeedInputV3::CommonIdentity(SERIES_CONSUME_ROOT_KIND_IDENTITY_V4),
        LifecycleSeedInputV3::CommonIdentity(SERIES_CONSUME_ROOT_CAPABILITY_RELEASE_IDENTITY_V4),
        LifecycleSeedInputV3::CommonIdentity(SERIES_CONSUME_ROOT_CONFIG_IDENTITY_V4),
        LifecycleSeedInputV3::CanonicalBump,
    ];
    let plans = [LifecyclePlanInputV3 {
        action: SeriesActionV3::Close as u32,
        operation: LifecycleOperationInputV3::Close,
        recipe: 0,
        payer: None,
        rent_credit: Some(LifecycleAccountCoordinateV3::fixed(
            SERIES_CLOSE_RENT_CREDIT_COORDINATE_V5,
        )),
        principal: Some(LifecycleRegisterCoordinateV3::common(
            SERIES_CLOSE_ROOT_PRINCIPAL_SCALAR_V5,
        )),
        beneficiary: Some(LifecycleRegisterCoordinateV3::common(
            SERIES_CLOSE_BENEFICIARY_IDENTITY_V5,
        )),
        refund_source: LifecycleRefundSourceInputV3::Credit,
        guard: LifecycleGuardInputV3::Always,
    }];
    encode_lifecycle_policy_v5_atomic(&recipes, &seeds, &plans, &[None], &[], &[], scratch, output)
        .map_err(SeriesLifecyclePolicyErrorV5::Lifecycle)?;
    StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], output)
        .map_err(SeriesLifecyclePolicyErrorV5::Lifecycle)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use alloc::vec;

    use dclutch_account_profile_contract::v2::{
        AccountProfileV2, ProjectionRegisterKindV2, ProjectionRegisterSpaceV2, ProjectionTargetV2,
    };

    use super::super::account_profile_v4::{
        SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4, SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4,
        SeriesConsumeAccountProfileInputV4, encode_series_consume_account_profile_v4_atomic,
    };
    use super::*;
    use dclutch_account_profile_contract::lifecycle_v3::{
        CoordinateScopeV3, LifecycleRegisterKindV3,
    };

    fn policy_bytes() -> vec::Vec<u8> {
        let mut scratch = vec![0_u8; SERIES_CONSUME_STATE_LIFECYCLE_BYTES_V5];
        let mut output = vec![0_u8; SERIES_CONSUME_STATE_LIFECYCLE_BYTES_V5];
        encode_series_consume_state_lifecycle_v5_atomic(&mut scratch, &mut output)
            .expect("Series lifecycle policy");
        output
    }

    fn close_policy_bytes() -> vec::Vec<u8> {
        let mut scratch = vec![0_u8; SERIES_CONSUME_STATE_LIFECYCLE_BYTES_V5];
        let mut output = vec![0_u8; SERIES_CONSUME_STATE_LIFECYCLE_BYTES_V5];
        encode_series_close_state_lifecycle_v5_atomic(&mut scratch, &mut output)
            .expect("Series close lifecycle policy");
        output
    }

    fn empty_policy_bytes() -> vec::Vec<u8> {
        let mut scratch = vec![0_u8; SERIES_EMPTY_STATE_LIFECYCLE_BYTES_V5];
        let mut output = vec![0_u8; SERIES_EMPTY_STATE_LIFECYCLE_BYTES_V5];
        encode_series_empty_state_lifecycle_v5_atomic(&mut scratch, &mut output)
            .expect("Series empty lifecycle policy");
        output
    }

    #[test]
    fn live_actions_select_no_lifecycle_authority() {
        let bytes = empty_policy_bytes();
        let policy = StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &bytes)
            .expect("empty policy");
        for action in [
            SeriesActionV3::Prepare,
            SeriesActionV3::Consume,
            SeriesActionV3::Expire,
            SeriesActionV3::Retire,
            SeriesActionV3::Close,
        ] {
            assert_eq!(policy.action_plan_count(action as u32), Ok(0));
        }
        assert_eq!(policy.current_rent_quote_count(), 0);
    }

    #[test]
    fn empty_policy_refuses_nonexact_buffers() {
        let mut short_scratch = vec![0_u8; SERIES_EMPTY_STATE_LIFECYCLE_BYTES_V5 - 1];
        let mut short_output = vec![0_u8; SERIES_EMPTY_STATE_LIFECYCLE_BYTES_V5 - 1];
        assert_eq!(
            encode_series_empty_state_lifecycle_v5_atomic(&mut short_scratch, &mut short_output),
            Err(SeriesLifecyclePolicyErrorV5::Geometry)
        );
    }

    #[test]
    fn terminal_policy_selects_only_root_close() {
        let bytes = close_policy_bytes();
        let policy = StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &bytes)
            .expect("close policy");
        let selected = policy
            .action_plan(SeriesActionV3::Close as u32, 0)
            .expect("sole Close plan");
        assert_eq!(
            selected.operation(),
            dclutch_account_profile_contract::lifecycle_v3::LifecycleOperationV3::Close
        );
        assert_eq!(
            policy.action_plan_count(SeriesActionV3::Close as u32),
            Ok(1)
        );
        for live in [
            SeriesActionV3::Prepare,
            SeriesActionV3::Consume,
            SeriesActionV3::Expire,
            SeriesActionV3::Retire,
        ] {
            assert_eq!(policy.action_plan_count(live as u32), Ok(0));
        }
        assert_eq!(selected.protected_outputs(), Ok(None));
        assert_eq!(policy.current_rent_quote_count(), 0);
    }

    #[test]
    fn terminal_policy_is_deterministic_and_refuses_wrong_width() {
        assert_eq!(close_policy_bytes(), close_policy_bytes());
        let mut scratch = vec![0_u8; SERIES_CONSUME_STATE_LIFECYCLE_BYTES_V5 - 1];
        let mut output = vec![0_u8; SERIES_CONSUME_STATE_LIFECYCLE_BYTES_V5 - 1];
        assert_eq!(
            encode_series_close_state_lifecycle_v5_atomic(&mut scratch, &mut output),
            Err(SeriesLifecyclePolicyErrorV5::Geometry)
        );
    }

    fn profile_bytes() -> vec::Vec<u8> {
        let lengths = [0_u32; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4];
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

    /// Requirements 2–4 of the verifier-derived checklist, exercised on the
    /// exact artifacts this crate emits rather than on fixtures.
    #[test]
    fn the_policy_satisfies_the_verifier_derived_checklist() {
        let bytes = policy_bytes();
        let policy = StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &bytes)
            .expect("decodes under the V5 schema profile");
        let profile_bytes = profile_bytes();
        let profile = AccountProfileV2::decode(&profile_bytes).expect("Profile13");
        policy
            .validate_account_profile(profile)
            .expect("joins the Series Consume Profile13");
        assert_eq!(
            policy.action_plan_count(SeriesActionV3::Consume as u32),
            Ok(1)
        );
        for other in [
            SeriesActionV3::Prepare,
            SeriesActionV3::Expire,
            SeriesActionV3::Retire,
            SeriesActionV3::Close,
        ] {
            assert_eq!(policy.action_plan_count(other as u32), Ok(0));
        }
    }

    /// The one plan covers the root, authors no lamport flow, and derives the
    /// root's own eight-seed derivation with a canonical bump.
    #[test]
    fn the_consume_plan_authenticates_the_root_and_authors_nothing() {
        let bytes = policy_bytes();
        let policy =
            StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &bytes).expect("policy");
        let profile_bytes = profile_bytes();
        let profile = AccountProfileV2::decode(&profile_bytes).expect("Profile13");
        let selected = policy
            .action_plan(SeriesActionV3::Consume as u32, 0)
            .expect("Consume plan");
        assert_eq!(selected.uses_canonical_bump(), Ok(true));
        assert_eq!(
            selected.seed_count(),
            Ok(u8::try_from(SERIES_CONSUME_ROOT_SEED_COUNT_V5).expect("seed count"))
        );
        assert_eq!(selected.protected_outputs().expect("protected"), None);
        assert_eq!(selected.immutable_identity_binding_count(), Ok(0));
        assert_eq!(policy.current_rent_quote_count(), 0);
        let indices = selected
            .project_account_indices(profile, 0, None)
            .expect("account indices");
        assert_eq!(
            indices.state(),
            usize::from(SERIES_CONSUME_ROOT_COORDINATE_V4)
        );
        assert_eq!(indices.payer(), None);
        assert_eq!(indices.rent_credit(), None);
        assert_eq!(
            selected.target_data_bytes(0),
            Ok(u32::try_from(SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5).expect("root width"))
        );
        // The width pin is the sum the release set binds, not a free number.
        assert_eq!(SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5, 232 + 64);
    }

    /// Every register-backed seed reads a register the Profile13 writes from
    /// the root header's own data — the root stays the single author of its
    /// derivation, and no seed value can arrive from a request or transition.
    #[test]
    fn every_register_backed_seed_is_a_root_header_projection() {
        let bytes = policy_bytes();
        let policy =
            StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &bytes).expect("policy");
        let profile_bytes = profile_bytes();
        let profile = AccountProfileV2::decode(&profile_bytes).expect("Profile13");
        let selected = policy
            .action_plan(SeriesActionV3::Consume as u32, 0)
            .expect("Consume plan");
        let mut register_backed = 0_usize;
        for ordinal in 0..u8::try_from(SERIES_CONSUME_ROOT_SEED_COUNT_V5).expect("seed count") {
            let Some(target) = selected
                .seed_register_target(ordinal)
                .expect("seed inspection")
            else {
                continue;
            };
            register_backed += 1;
            assert_eq!(target.scope(), CoordinateScopeV3::Fixed);
            let projection = ProjectionTargetV2 {
                kind: match target.kind() {
                    LifecycleRegisterKindV3::Scalar => ProjectionRegisterKindV2::Scalar,
                    LifecycleRegisterKindV3::Identity => ProjectionRegisterKindV2::Identity,
                },
                space: ProjectionRegisterSpaceV2::Common,
                index: target.index(),
            };
            assert_eq!(
                profile.writes_register(projection),
                Ok(true),
                "seed {ordinal} reads a register nothing in the profile writes"
            );
        }
        // Seven register-backed seeds: five identities and two scalars. The
        // domain literal and the canonical bump are the other two.
        assert_eq!(register_backed, 7);
    }

    /// Emission is deterministic so the release's digest addresses it.
    #[test]
    fn emission_is_deterministic() {
        assert_eq!(policy_bytes(), policy_bytes());
    }

    /// The width constant is exactly what the encoder emits.
    #[test]
    fn the_width_constant_is_exact() {
        assert_eq!(
            policy_bytes().len(),
            SERIES_CONSUME_STATE_LIFECYCLE_BYTES_V5
        );
        let mut scratch = vec![0_u8; SERIES_CONSUME_STATE_LIFECYCLE_BYTES_V5 - 1];
        let mut output = vec![0_u8; SERIES_CONSUME_STATE_LIFECYCLE_BYTES_V5 - 1];
        assert_eq!(
            encode_series_consume_state_lifecycle_v5_atomic(&mut scratch, &mut output),
            Err(SeriesLifecyclePolicyErrorV5::Geometry)
        );
    }
}
