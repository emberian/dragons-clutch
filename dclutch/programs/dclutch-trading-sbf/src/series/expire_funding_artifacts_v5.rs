//! Canonical current-source Series Expire AccountProfileV3/EffectV5 artifacts.
//!
//! Expire persists the exact `Expired` Ticket replay poststate. Retire is the
//! sole later delete author, so the V3/V5 funding refinements here are exact
//! canonical empties. A dedicated Ticket representative still precedes every
//! child window: Trading writes the replay candidate at that representative,
//! while Core receives a readonly route alias.

extern crate alloc;

use alloc::{vec, vec::Vec};

use dclutch_custody::ProjectedCustodyRequestLayoutV1;
use dclutch_market::capability_program::{
    CAPABILITY_ROOT_GENERATION_OFFSET, CAPABILITY_ROOT_HEADER_BYTES_V1,
    CAPABILITY_ROOT_MARKET_OFFSET, CAPABILITY_ROOT_SELECTION_OFFSET,
    hot_v3::{HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3, HOT_RUNTIME_PORTFOLIO_COORDINATE_V3},
};
use dclutch_market::{
    SERIES_UNALLOCATED_PERMIT_EXPIRY_EXPECTED_SERIES_REVISION_OFFSET_V1,
    SERIES_UNALLOCATED_PERMIT_EXPIRY_EXPECTED_TICKET_REVISION_OFFSET_V1,
    SERIES_UNALLOCATED_PERMIT_EXPIRY_REQUEST_BYTES_V1, SeriesCoreRequestV1,
    SeriesPermitExpiryRequestV1, SeriesUnallocatedPermitExpiryRequestV1,
};
use dclutch_product::{
    PORTFOLIO_COEFFICIENT_BYTES, PORTFOLIO_COEFFICIENT_COUNT_OFFSET, PORTFOLIO_HEADER_BYTES,
};
use dclutch_registry::release_set::{
    CAPABILITY_EXECUTION_SELECTION_CONFIG_OFFSET,
    CAPABILITY_EXECUTION_SELECTION_ENTRY_INDEX_OFFSET, CAPABILITY_EXECUTION_SELECTION_KIND_OFFSET,
    CAPABILITY_EXECUTION_SELECTION_MANIFEST_OFFSET, CAPABILITY_EXECUTION_SELECTION_RELEASE_OFFSET,
};
use dclutch_trading::series::{series_action_request_bytes_v3, series_proof_count_v3};
use dclutch_vm::account_profile::{
    v2::{
        AccountPrestateV2, AccountProfileV2, DYNAMIC_FIXED_SPAN_HEADER_BYTES, OPERATION_BYTES,
        RULE_BYTES, TrustedBuiltinIdentityV2, TrustedEnvironmentV2, TrustedIdentityEnvironmentV2,
        encode::{
            AccountAliasInputV2, AccountCoordinateV2, AccountEffectPermissionsV2,
            AccountOperationInputV2, AccountPrivilegesV2, AccountRuleInputV2,
            AccountRuleWithPrestateInputV2, IdentityCoordinateV2, RegisterGeometryV2,
            ScalarCoordinateV2, encode_account_profile_with_dynamic_fixed_span_v2_atomic,
        },
    },
    v3::{AccountProfileV3, HEADER_BYTES_V3, encode_account_profile_v3_atomic},
};
use dclutch_vm::effect::{
    v2::FixedRole,
    v3::{
        HEADER_BYTES as EFFECT_HEADER_BYTES_V3, OPERATION_BYTES as EFFECT_OPERATION_BYTES_V3,
        ROUTE_BYTES as EFFECT_ROUTE_BYTES_V3, RouteKindV3,
        encode::{
            AccountCoordinateV3, EffectGeometryV3, EffectInstructionV3, IdentityCoordinateV3,
            RequestSpaceV3, RouteInputV3, ScalarCoordinateV3, encode_effect_program_v4_atomic,
        },
    },
    v4::{
        BORROWED_RANGE_BYTES_V4, BorrowedRangePolicyV4, BorrowedRangeV4,
        HEADER_BYTES_V4 as EFFECT_HEADER_BYTES_V4, ProgramV4, RequestCoordinateV4,
        encode_program_v4_atomic,
    },
    v5::{HEADER_BYTES_V5 as EFFECT_HEADER_BYTES_V5, ProgramV5, encode_program_v5_atomic},
};

use dclutch_vm::request_profile::{
    HEADER_BYTES as REQUEST_HEADER_BYTES, OPERATION_BYTES as REQUEST_OPERATION_BYTES,
    RequestProfileV1,
    encode::{
        IdentityRegisterV1, RequestCoordinateV1, RequestGeometryV1, RequestInstructionV1,
        ScalarRegisterV1, encode_request_profile_v1_atomic,
    },
};
use dclutch_vm::v3::{
    HEADER_BYTES as TRANSITION_HEADER_BYTES, INSTRUCTION_BYTES as TRANSITION_INSTRUCTION_BYTES,
    IdentityRegisterV3, InstructionV3, ProgramGeometryV3, ProgramV3 as TransitionProgramV3,
    ScalarRegisterV3, encode_program_atomic,
};

use super::{
    account_profile_v4::{
        SERIES_CONSUME_ROOT_CAPABILITY_RELEASE_IDENTITY_V4, SERIES_CONSUME_ROOT_CONFIG_IDENTITY_V4,
        SERIES_CONSUME_ROOT_ENTRY_INDEX_SCALAR_V4, SERIES_CONSUME_ROOT_GENERATION_SCALAR_V4,
        SERIES_CONSUME_ROOT_KIND_IDENTITY_V4, SERIES_CONSUME_ROOT_MANIFEST_IDENTITY_V4,
        SERIES_CONSUME_ROOT_MARKET_IDENTITY_V4,
    },
    artifacts_v3::{
        SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3, SERIES_NO_RECEIPT_DEPENDENCIES_V3,
        SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3, SERIES_WITNESS_ITEM_BYTES_V3,
    },
    instruction::{SERIES_ACTION_HEADER_BYTES_V3, SeriesActionV3},
    lifecycle_policy_v5::SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5,
    state::SERIES_TICKET_STATE_BYTES_V3,
};

/// Outer writable Ticket representative retained for replay after Expire.
pub const SERIES_EXPIRE_TICKET_COORDINATE_V5: u16 = 5;
/// Fixed-account start of each of the five authenticated child routes.
pub const SERIES_EXPIRE_ROUTE_STARTS_V5: [u16; 5] = [6, 20, 34, 44, 55];
/// Fixed-account width of each of the five authenticated child routes.
pub const SERIES_EXPIRE_ROUTE_COUNTS_V5: [u16; 5] = [14, 14, 10, 11, 26];
/// Canonical RentCredit representative supplied by the second custody route.
pub const SERIES_EXPIRE_RENT_CREDIT_COORDINATE_V5: u16 = 33;
/// Outer readonly caller PDA synthesized as a signer only for the Core CPI.
pub const SERIES_EXPIRE_PRECOMMIT_CALLER_COORDINATE_V5: u16 = 80;
/// The release-selected Custody program the four Custody routes are invoked
/// through.
///
/// A CPI's callee is not a member of its own account list, and
/// `CustodyFrameRoleV1` has no `CustodyProgram` variant at all -- a Custody
/// frame names `CallerProgram`, which is Trading's -- so no Custody route
/// window can carry it. `hot_v3::resolve_role_carrier_v3` resolves a child
/// route's callee by scanning the downgraded LOGICAL vector for the key the
/// Registry activation cache names for that role, so the program has to BE one
/// of this profile's coordinates or every Custody route refuses `Release`
/// before its first CPI. Series Consume needed no coordinate of its own --
/// `account_profile_v4`'s Core Found suffix, Claims founding frame and Core
/// Open suffix each name the Custody program inside their own frames, which is
/// where its three carriers come from -- and Expire's five frames name it
/// nowhere: three canonical `CustodyFrameSpecV1` windows, one Trading-owned
/// projected-custody window, and the twenty-five-account Core
/// unallocated-permit frame plus its caller.
///
/// It is Expire's own outer coordinate, appended PAST every route range, so
/// carrying it renumbers no frame: `SERIES_EXPIRE_ROUTE_STARTS_V5`,
/// `SERIES_EXPIRE_ROUTE_COUNTS_V5` and every `ROUTE_ALIASES` pair are
/// unchanged. Its rule is executable, readonly and opaque, exactly as Direct's
/// three, General's and Dealer's are: the loader that deployed it owns its
/// record width, and the activation cache -- not this profile -- is the sole
/// authority on which program the Custody role selects.
pub const SERIES_EXPIRE_CUSTODY_PROGRAM_COORDINATE_V5: u16 = 81;
/// Complete fixed-account width of the Expire outer invocation.
pub const SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5: u16 = 82;
/// Common scalar register width authenticated by Expire artifacts.
pub const SERIES_EXPIRE_COMMON_SCALAR_COUNT_V5: u16 = 26;
/// Common identity register width authenticated by Expire artifacts.
pub const SERIES_EXPIRE_COMMON_IDENTITY_COUNT_V5: u16 = 12;

/// Scalar containing exact borrowed proof bytes.
pub const SERIES_EXPIRE_PROOF_BYTES_SCALAR_V5: u16 = 1;
/// Scalar containing the canonical proof item count.
pub const SERIES_EXPIRE_PROOF_COUNT_SCALAR_V5: u16 = 2;
/// Scalar containing the fixed witness-item width.
pub const SERIES_EXPIRE_WITNESS_ITEM_BYTES_SCALAR_V5: u16 = 3;
/// Scalar containing the authenticated Portfolio tail count.
pub const SERIES_EXPIRE_PRODUCT_TAIL_COUNT_SCALAR_V5: u16 = 4;
/// Request-projected expected root revision.
pub const SERIES_EXPIRE_EXPECTED_ROOT_REVISION_SCALAR_V5: u16 = 8;
/// Request-projected expected Ticket revision.
pub const SERIES_EXPIRE_EXPECTED_TICKET_REVISION_SCALAR_V5: u16 = 9;
/// Account-projected current root revision.
pub const SERIES_EXPIRE_OBSERVED_ROOT_REVISION_SCALAR_V5: u16 = 10;
/// Account-projected current Ticket revision.
pub const SERIES_EXPIRE_OBSERVED_TICKET_REVISION_SCALAR_V5: u16 = 11;
/// Transition-derived successor root revision.
pub const SERIES_EXPIRE_CANDIDATE_ROOT_REVISION_SCALAR_V5: u16 = 12;
/// Account-projected next occurrence index.
pub const SERIES_EXPIRE_OBSERVED_NEXT_OCCURRENCE_SCALAR_V5: u16 = 13;
/// Transition-derived successor next occurrence index.
pub const SERIES_EXPIRE_CANDIDATE_NEXT_OCCURRENCE_SCALAR_V5: u16 = 14;
/// Immutable Template occurrence count.
pub const SERIES_EXPIRE_OCCURRENCE_COUNT_SCALAR_V5: u16 = 15;
/// Expiry-request occurrence index.
pub const SERIES_EXPIRE_OCCURRENCE_INDEX_SCALAR_V5: u16 = 16;
/// Canonical zero constant.
pub const SERIES_EXPIRE_ZERO_SCALAR_V5: u16 = 17;
/// Canonical one constant.
pub const SERIES_EXPIRE_ONE_SCALAR_V5: u16 = 18;
/// Account-projected current root phase.
pub const SERIES_EXPIRE_OBSERVED_ROOT_PHASE_SCALAR_V5: u16 = 19;
/// Transition-derived successor root phase.
pub const SERIES_EXPIRE_CANDIDATE_ROOT_PHASE_SCALAR_V5: u16 = 20;
/// Account-projected prepared-occurrence count.
pub const SERIES_EXPIRE_OBSERVED_PREPARED_SCALAR_V5: u16 = 21;
/// Canonical Expired Ticket phase constant.
pub const SERIES_EXPIRE_EXPIRED_PHASE_SCALAR_V5: u16 = 22;
/// Transition-derived successor Ticket revision.
pub const SERIES_EXPIRE_CANDIDATE_TICKET_REVISION_SCALAR_V5: u16 = 23;
/// Exact outer Ticket lamports observed before replay persistence.
pub const SERIES_EXPIRE_OBSERVED_TICKET_LAMPORTS_SCALAR_V5: u16 = 24;
/// Account-projected current Ticket phase.
pub const SERIES_EXPIRE_OBSERVED_TICKET_PHASE_SCALAR_V5: u16 = 25;

/// Authenticated lifecycle RentCredit beneficiary identity.
pub const SERIES_EXPIRE_RENT_CREDIT_BENEFICIARY_IDENTITY_V5: u16 = 6;
/// Current executing Trading program identity.
pub const SERIES_EXPIRE_TRADING_PROGRAM_IDENTITY_V5: u16 = 7;
/// Immutable Template refund-owner identity.
pub const SERIES_EXPIRE_TEMPLATE_REFUND_OWNER_IDENTITY_V5: u16 = 8;
/// Request-projected expected Ticket content identity.
pub const SERIES_EXPIRE_EXPECTED_TICKET_IDENTITY_V5: u16 = 9;
/// Account-projected observed Ticket content identity.
pub const SERIES_EXPIRE_OBSERVED_TICKET_IDENTITY_V5: u16 = 10;
/// Runtime persistent Series-root key patched into projected Abort.
pub const SERIES_EXPIRE_ROOT_KEY_IDENTITY_V5: u16 = 11;

const PRODUCT_RECORD_BYTES: u32 = 112;
const ACTION_OFFSET: u32 = 12;
const PROOF_COUNT_OFFSET: u32 = 13;
const REQUEST_TICKET_OFFSET: u32 = 80;
const EXPECTED_ROOT_REVISION_OFFSET: u32 = 112;
const EXPECTED_TICKET_REVISION_OFFSET: u32 = 120;
const ROOT_PHASE_OFFSET: u32 = CAPABILITY_ROOT_HEADER_BYTES_V1 as u32 + 12;
const ROOT_PREPARED_OFFSET: u32 = CAPABILITY_ROOT_HEADER_BYTES_V1 as u32 + 13;
const ROOT_NEXT_OFFSET: u32 = CAPABILITY_ROOT_HEADER_BYTES_V1 as u32 + 16;
const ROOT_REVISION_OFFSET: u32 = CAPABILITY_ROOT_HEADER_BYTES_V1 as u32 + 24;
const TICKET_PHASE_OFFSET: u32 = 12;
const TICKET_REVISION_OFFSET: u32 = 16;
const TICKET_RECORD_OFFSET: u32 = 24;
const RENT_CREDIT_BENEFICIARY_OFFSET: u32 = 16;

const PROFILE_OPERATIONS: usize = 23;
const EFFECT_OPERATIONS: usize = 9;
const REQUEST_OPERATIONS: usize = 5;
const TRANSITION_OPERATIONS: usize = 19;

/// Exact encoded byte width of the embedded AccountProfileV2.
pub const SERIES_EXPIRE_BASE_ACCOUNT_PROFILE_BYTES_V5: usize = DYNAMIC_FIXED_SPAN_HEADER_BYTES
    + SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 as usize * RULE_BYTES
    + PROFILE_OPERATIONS * OPERATION_BYTES;
/// Exact encoded byte width of the AccountProfileV3 successor.
pub const SERIES_EXPIRE_ACCOUNT_PROFILE_BYTES_V5: usize =
    HEADER_BYTES_V3 + SERIES_EXPIRE_BASE_ACCOUNT_PROFILE_BYTES_V5;
/// Exact encoded byte width of the RequestProfileV1.
pub const SERIES_EXPIRE_REQUEST_PROFILE_BYTES_V5: usize =
    REQUEST_HEADER_BYTES + REQUEST_OPERATIONS * REQUEST_OPERATION_BYTES;
/// Exact encoded byte width of the nineteen-instruction TransitionV3.
pub const SERIES_EXPIRE_TRANSITION_BYTES_V5: usize =
    TRANSITION_HEADER_BYTES + TRANSITION_OPERATIONS * TRANSITION_INSTRUCTION_BYTES;
/// Exact concatenated request-bank byte width for all five child routes.
pub const SERIES_EXPIRE_REQUEST_BANK_BYTES_V5: usize = 3 * SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3
    + SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3
    + SERIES_UNALLOCATED_PERMIT_EXPIRY_REQUEST_BYTES_V1;
/// Exact encoded byte width of the embedded EffectV3.
pub const SERIES_EXPIRE_BASE_EFFECT_BYTES_V5: usize = EFFECT_HEADER_BYTES_V3
    + 5 * EFFECT_ROUTE_BYTES_V3
    + EFFECT_OPERATIONS * EFFECT_OPERATION_BYTES_V3
    + SERIES_EXPIRE_REQUEST_BANK_BYTES_V5;
/// Exact number of borrowed proof ranges route 4 declares for one Template.
///
/// A borrowed range is canonically NONEMPTY on both sides of the seam --
/// `BorrowedRangeV4::resolve` refuses a zero length and
/// `BorrowedWitnessPolicyV3::validate` refuses a zero minimum -- so a Template
/// whose canonical proof is empty must declare NO range rather than a range
/// that happens to resolve to zero. `series_proof_count_v3` is the same
/// authority the Series kernel admits against, so the declaration and the
/// admission cannot drift.
pub const fn series_expire_borrowed_range_count_v5(occurrence_count: u32) -> usize {
    if series_proof_count_v3(occurrence_count) == 0 {
        0
    } else {
        1
    }
}

/// Exact encoded byte width of the borrowed-proof EffectV4 for one Template.
pub const fn series_expire_effect_v4_bytes_v5(occurrence_count: u32) -> usize {
    SERIES_EXPIRE_BASE_EFFECT_BYTES_V5
        + EFFECT_HEADER_BYTES_V4
        + series_expire_borrowed_range_count_v5(occurrence_count) * BORROWED_RANGE_BYTES_V4
}

/// Exact encoded byte width of the empty-funding EffectV5 successor.
pub const fn series_expire_effect_bytes_v5(occurrence_count: u32) -> usize {
    EFFECT_HEADER_BYTES_V5 + series_expire_effect_v4_bytes_v5(occurrence_count)
}

const _: () = assert!(HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3 == 5);
// The callee is appended past the last route range, and the profile ends there.
// Either assertion alone admits a renumbering; together they are the property
// the doc comment on `SERIES_EXPIRE_CUSTODY_PROGRAM_COORDINATE_V5` claims.
const _: () = assert!(
    SERIES_EXPIRE_ROUTE_STARTS_V5[4] + SERIES_EXPIRE_ROUTE_COUNTS_V5[4]
        == SERIES_EXPIRE_CUSTODY_PROGRAM_COORDINATE_V5
);
const _: () = assert!(
    SERIES_EXPIRE_CUSTODY_PROGRAM_COORDINATE_V5 + 1 == SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5
);

#[derive(Clone, Copy, Debug)]
/// Fixed prestate widths required to emit the exact Expire profile.
pub struct SeriesExpireAccountProfileInputV5<'a> {
    /// Data length observed for each outer fixed account coordinate.
    pub fixed_data_lengths: &'a [u32; SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 as usize],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Canonical typed requests concatenated into the five-route Effect bank.
pub struct SeriesExpireChildRequestsV5<'a> {
    /// First custody refund request.
    pub refund: &'a [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    /// Second custody vault-close request.
    pub close_vault: &'a [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    /// Third custody replay-close request.
    pub close_replay: &'a [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    /// Projected custody abort request.
    pub projected_abort: &'a [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
    /// Exact authenticated permit-expiry request.
    pub permit_expiry: SeriesPermitExpiryRequestV1,
    /// Exact authenticated Core Expire request.
    pub core_expire: SeriesCoreRequestV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Complete canonical artifact bundle selected by Series Expire.
pub struct SeriesExpireFundingArtifactsV5 {
    /// AccountProfileV3 bytes with an empty funding refinement.
    pub account_profile: Vec<u8>,
    /// Exact Expire RequestProfileV1 bytes.
    pub request_profile: Vec<u8>,
    /// Exact Expire TransitionV3 bytes.
    pub transition: Vec<u8>,
    /// EffectV5 bytes with borrowed proof topology and empty funding actions.
    pub effect: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Refusal reason while deriving the canonical Expire artifact bundle.
pub enum SeriesExpireFundingArtifactErrorV5 {
    /// A fixed geometry or exact-width invariant failed.
    Geometry,
    /// AccountProfile encoding or self-authentication failed.
    AccountProfile,
    /// RequestProfile encoding or self-authentication failed.
    RequestProfile,
    /// Transition encoding or self-authentication failed.
    Transition,
    /// Effect encoding or self-authentication failed.
    Effect,
}

/// Result returned by the Series Expire artifact emitter.
pub type Result<T> = core::result::Result<T, SeriesExpireFundingArtifactErrorV5>;

/// Emits and hostile-decodes the canonical current-source Expire artifact bundle.
pub fn emit_series_expire_funding_artifacts_v5(
    profile: SeriesExpireAccountProfileInputV5<'_>,
    requests: SeriesExpireChildRequestsV5<'_>,
    occurrence_count: u32,
) -> Result<SeriesExpireFundingArtifactsV5> {
    Ok(SeriesExpireFundingArtifactsV5 {
        account_profile: emit_account_profile(profile)?,
        request_profile: emit_request_profile(occurrence_count)?,
        transition: emit_transition()?,
        effect: emit_effect(requests, occurrence_count)?,
    })
}

fn emit_account_profile(input: SeriesExpireAccountProfileInputV5<'_>) -> Result<Vec<u8>> {
    let mut rules = Vec::with_capacity(usize::from(SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5));
    for coordinate in 0..SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 {
        rules.push(account_rule(coordinate, input.fixed_data_lengths)?);
    }
    let operations = profile_operations()?;
    let mut base_scratch = vec![0; SERIES_EXPIRE_BASE_ACCOUNT_PROFILE_BYTES_V5];
    let mut base = vec![0; SERIES_EXPIRE_BASE_ACCOUNT_PROFILE_BYTES_V5];
    encode_account_profile_with_dynamic_fixed_span_v2_atomic(
        TrustedEnvironmentV2::None,
        TrustedIdentityEnvironmentV2::CurrentExecutingProgram {
            destination: SERIES_EXPIRE_TRADING_PROGRAM_IDENTITY_V5,
        },
        TrustedBuiltinIdentityV2::None,
        &[],
        &rules,
        &[],
        &operations,
        RegisterGeometryV2 {
            common_scalars: SERIES_EXPIRE_COMMON_SCALAR_COUNT_V5,
            item_scalar_stride: 0,
            common_identities: SERIES_EXPIRE_COMMON_IDENTITY_COUNT_V5,
            item_identity_stride: 0,
        },
        &mut base_scratch,
        &mut base,
    )
    .map_err(|_| SeriesExpireFundingArtifactErrorV5::AccountProfile)?;
    AccountProfileV2::decode(&base)
        .map_err(|_| SeriesExpireFundingArtifactErrorV5::AccountProfile)?;
    let mut scratch = vec![0; SERIES_EXPIRE_ACCOUNT_PROFILE_BYTES_V5];
    let mut output = vec![0; SERIES_EXPIRE_ACCOUNT_PROFILE_BYTES_V5];
    encode_account_profile_v3_atomic(&base, &[], &mut scratch, &mut output)
        .map_err(|_| SeriesExpireFundingArtifactErrorV5::AccountProfile)?;
    let decoded = AccountProfileV3::decode(&output)
        .map_err(|_| SeriesExpireFundingArtifactErrorV5::AccountProfile)?;
    if decoded.funding_bound_count() != 0 {
        return Err(SeriesExpireFundingArtifactErrorV5::AccountProfile);
    }
    Ok(output)
}

fn account_rule(
    coordinate: u16,
    lengths: &[u32; SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 as usize],
) -> Result<AccountRuleWithPrestateInputV2> {
    if let Some(representative) = alias_representative(coordinate) {
        return Ok(AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: readonly(),
                effect_permissions: no_effects(),
                alias: AccountAliasInputV2::Fixed(representative),
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::AuthenticatedRouteAlias,
        });
    }
    let (data_length, stride, prestate) = match coordinate {
        0 => (
            u32::try_from(SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5)
                .map_err(|_| SeriesExpireFundingArtifactErrorV5::Geometry)?,
            0,
            AccountPrestateV2::Exact,
        ),
        1 => (
            u32::try_from(dclutch_trading::series::SERIES_TEMPLATE_BYTES_V3)
                .map_err(|_| SeriesExpireFundingArtifactErrorV5::Geometry)?,
            0,
            AccountPrestateV2::Exact,
        ),
        2 => (PRODUCT_RECORD_BYTES, 0, AccountPrestateV2::Exact),
        3 => (
            u32::try_from(PORTFOLIO_HEADER_BYTES)
                .map_err(|_| SeriesExpireFundingArtifactErrorV5::Geometry)?,
            u32::try_from(PORTFOLIO_COEFFICIENT_BYTES)
                .map_err(|_| SeriesExpireFundingArtifactErrorV5::Geometry)?,
            AccountPrestateV2::Exact,
        ),
        4 => (0, 0, AccountPrestateV2::AuthenticatedOpaqueReadonlyData),
        // Opaque on purpose: whichever loader deployed the Custody program owns
        // its record width, so this profile declares none. See the constant.
        SERIES_EXPIRE_CUSTODY_PROGRAM_COORDINATE_V5 => {
            (0, 0, AccountPrestateV2::AuthenticatedOpaqueReadonlyData)
        }
        SERIES_EXPIRE_TICKET_COORDINATE_V5 => (
            u32::try_from(SERIES_TICKET_STATE_BYTES_V3)
                .map_err(|_| SeriesExpireFundingArtifactErrorV5::Geometry)?,
            0,
            AccountPrestateV2::Exact,
        ),
        SERIES_EXPIRE_RENT_CREDIT_COORDINATE_V5 => (
            u32::try_from(dclutch_market::rent::lifecycle_v2::LIFECYCLE_RENT_CREDIT_BYTES_V2)
                .map_err(|_| SeriesExpireFundingArtifactErrorV5::Geometry)?,
            0,
            AccountPrestateV2::Exact,
        ),
        _ => (
            *lengths
                .get(usize::from(coordinate))
                .ok_or(SeriesExpireFundingArtifactErrorV5::Geometry)?,
            0,
            AccountPrestateV2::Exact,
        ),
    };
    let privileges = if WRITABLE_REPRESENTATIVES.contains(&coordinate) {
        writable()
    } else if EXECUTABLE_REPRESENTATIVES.contains(&coordinate) {
        executable()
    } else {
        readonly()
    };
    Ok(AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges,
            effect_permissions: if matches!(coordinate, 0 | SERIES_EXPIRE_TICKET_COORDINATE_V5) {
                write_data()
            } else {
                no_effects()
            },
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length,
            data_item_stride: stride,
        },
        prestate,
    })
}

fn profile_operations() -> Result<[AccountOperationInputV2; PROFILE_OPERATIONS]> {
    Ok([
        AccountOperationInputV2::ProjectTailCountU32 {
            account: AccountCoordinateV2::fixed(
                u16::try_from(HOT_RUNTIME_PORTFOLIO_COORDINATE_V3)
                    .map_err(|_| SeriesExpireFundingArtifactErrorV5::Geometry)?,
            ),
            destination: ScalarCoordinateV2::common(SERIES_EXPIRE_PRODUCT_TAIL_COUNT_SCALAR_V5),
            data_offset: PORTFOLIO_COEFFICIENT_COUNT_OFFSET as u32,
        },
        AccountOperationInputV2::RequireOwner {
            account: AccountCoordinateV2::fixed(0),
            expected: IdentityCoordinateV2::common(SERIES_EXPIRE_TRADING_PROGRAM_IDENTITY_V5),
        },
        AccountOperationInputV2::ProjectKey {
            account: AccountCoordinateV2::fixed(0),
            destination: IdentityCoordinateV2::common(SERIES_EXPIRE_ROOT_KEY_IDENTITY_V5),
        },
        AccountOperationInputV2::RequireOwner {
            account: AccountCoordinateV2::fixed(SERIES_EXPIRE_TICKET_COORDINATE_V5),
            expected: IdentityCoordinateV2::common(SERIES_EXPIRE_TRADING_PROGRAM_IDENTITY_V5),
        },
        root_identity(
            SERIES_CONSUME_ROOT_MARKET_IDENTITY_V4,
            CAPABILITY_ROOT_MARKET_OFFSET,
        )?,
        project_u64(
            0,
            SERIES_CONSUME_ROOT_GENERATION_SCALAR_V4,
            CAPABILITY_ROOT_GENERATION_OFFSET as u32,
        ),
        root_identity(
            SERIES_CONSUME_ROOT_MANIFEST_IDENTITY_V4,
            CAPABILITY_ROOT_SELECTION_OFFSET + CAPABILITY_EXECUTION_SELECTION_MANIFEST_OFFSET,
        )?,
        AccountOperationInputV2::ProjectDataU16 {
            account: AccountCoordinateV2::fixed(0),
            destination: ScalarCoordinateV2::common(SERIES_CONSUME_ROOT_ENTRY_INDEX_SCALAR_V4),
            data_offset: (CAPABILITY_ROOT_SELECTION_OFFSET
                + CAPABILITY_EXECUTION_SELECTION_ENTRY_INDEX_OFFSET)
                as u32,
        },
        root_identity(
            SERIES_CONSUME_ROOT_KIND_IDENTITY_V4,
            CAPABILITY_ROOT_SELECTION_OFFSET + CAPABILITY_EXECUTION_SELECTION_KIND_OFFSET,
        )?,
        root_identity(
            SERIES_CONSUME_ROOT_CAPABILITY_RELEASE_IDENTITY_V4,
            CAPABILITY_ROOT_SELECTION_OFFSET + CAPABILITY_EXECUTION_SELECTION_RELEASE_OFFSET,
        )?,
        root_identity(
            SERIES_CONSUME_ROOT_CONFIG_IDENTITY_V4,
            CAPABILITY_ROOT_SELECTION_OFFSET + CAPABILITY_EXECUTION_SELECTION_CONFIG_OFFSET,
        )?,
        project_u64(
            0,
            SERIES_EXPIRE_OBSERVED_ROOT_REVISION_SCALAR_V5,
            ROOT_REVISION_OFFSET,
        ),
        project_u32(
            0,
            SERIES_EXPIRE_OBSERVED_NEXT_OCCURRENCE_SCALAR_V5,
            ROOT_NEXT_OFFSET,
        ),
        project_u8(
            0,
            SERIES_EXPIRE_OBSERVED_ROOT_PHASE_SCALAR_V5,
            ROOT_PHASE_OFFSET,
        ),
        project_u8(
            0,
            SERIES_EXPIRE_OBSERVED_PREPARED_SCALAR_V5,
            ROOT_PREPARED_OFFSET,
        ),
        AccountOperationInputV2::ProjectLamports {
            account: AccountCoordinateV2::fixed(SERIES_EXPIRE_TICKET_COORDINATE_V5),
            destination: ScalarCoordinateV2::common(
                SERIES_EXPIRE_OBSERVED_TICKET_LAMPORTS_SCALAR_V5,
            ),
        },
        project_u64(
            SERIES_EXPIRE_TICKET_COORDINATE_V5,
            SERIES_EXPIRE_OBSERVED_TICKET_REVISION_SCALAR_V5,
            TICKET_REVISION_OFFSET,
        ),
        AccountOperationInputV2::ProjectDataIdentity {
            account: AccountCoordinateV2::fixed(SERIES_EXPIRE_TICKET_COORDINATE_V5),
            destination: IdentityCoordinateV2::common(SERIES_EXPIRE_OBSERVED_TICKET_IDENTITY_V5),
            data_offset: TICKET_RECORD_OFFSET,
        },
        project_u32(
            1,
            SERIES_EXPIRE_OCCURRENCE_COUNT_SCALAR_V5,
            dclutch_trading::series::generated::SERIES_TEMPLATE_OCCURRENCE_COUNT_OFFSET_V3 as u32,
        ),
        project_u32(
            73,
            SERIES_EXPIRE_OCCURRENCE_INDEX_SCALAR_V5,
            dclutch_trading::series::generated::SERIES_OCCURRENCE_INDEX_OFFSET_V3 as u32,
        ),
        AccountOperationInputV2::ProjectDataIdentity {
            account: AccountCoordinateV2::fixed(1),
            destination: IdentityCoordinateV2::common(
                SERIES_EXPIRE_TEMPLATE_REFUND_OWNER_IDENTITY_V5,
            ),
            data_offset: dclutch_trading::series::generated::SERIES_TEMPLATE_REFUND_OWNER_OFFSET_V3
                as u32,
        },
        AccountOperationInputV2::ProjectDataIdentity {
            account: AccountCoordinateV2::fixed(SERIES_EXPIRE_RENT_CREDIT_COORDINATE_V5),
            destination: IdentityCoordinateV2::common(
                SERIES_EXPIRE_RENT_CREDIT_BENEFICIARY_IDENTITY_V5,
            ),
            data_offset: RENT_CREDIT_BENEFICIARY_OFFSET,
        },
        project_u8(
            SERIES_EXPIRE_TICKET_COORDINATE_V5,
            SERIES_EXPIRE_OBSERVED_TICKET_PHASE_SCALAR_V5,
            TICKET_PHASE_OFFSET,
        ),
    ])
}

fn emit_request_profile(occurrence_count: u32) -> Result<Vec<u8>> {
    let request_bytes = u32::try_from(series_action_request_bytes_v3(occurrence_count))
        .map_err(|_| SeriesExpireFundingArtifactErrorV5::Geometry)?;
    let instructions = [
        RequestInstructionV1::require_u8(
            RequestCoordinateV1::fixed(ACTION_OFFSET),
            SeriesActionV3::Expire as u8,
        ),
        RequestInstructionV1::project_u8(
            RequestCoordinateV1::fixed(PROOF_COUNT_OFFSET),
            ScalarRegisterV1::common(SERIES_EXPIRE_PROOF_COUNT_SCALAR_V5),
        ),
        RequestInstructionV1::project_u64(
            RequestCoordinateV1::fixed(EXPECTED_ROOT_REVISION_OFFSET),
            ScalarRegisterV1::common(SERIES_EXPIRE_EXPECTED_ROOT_REVISION_SCALAR_V5),
        ),
        RequestInstructionV1::project_u64(
            RequestCoordinateV1::fixed(EXPECTED_TICKET_REVISION_OFFSET),
            ScalarRegisterV1::common(SERIES_EXPIRE_EXPECTED_TICKET_REVISION_SCALAR_V5),
        ),
        RequestInstructionV1::project_identity(
            RequestCoordinateV1::fixed(REQUEST_TICKET_OFFSET),
            IdentityRegisterV1::common(SERIES_EXPIRE_EXPECTED_TICKET_IDENTITY_V5),
        ),
    ];
    let mut scratch = vec![0; SERIES_EXPIRE_REQUEST_PROFILE_BYTES_V5];
    let mut output = vec![0; SERIES_EXPIRE_REQUEST_PROFILE_BYTES_V5];
    encode_request_profile_v1_atomic(
        RequestGeometryV1::new(
            request_bytes,
            0,
            SERIES_EXPIRE_COMMON_SCALAR_COUNT_V5,
            0,
            SERIES_EXPIRE_COMMON_IDENTITY_COUNT_V5,
            0,
        ),
        &instructions,
        &[],
        &mut scratch,
        &mut output,
    )
    .map_err(|_| SeriesExpireFundingArtifactErrorV5::RequestProfile)?;
    RequestProfileV1::decode(&output)
        .map_err(|_| SeriesExpireFundingArtifactErrorV5::RequestProfile)?;
    Ok(output)
}

fn emit_transition() -> Result<Vec<u8>> {
    let s = ScalarRegisterV3::common;
    let i = IdentityRegisterV3::common;
    let instructions = [
        InstructionV3::load_const(s(0), SERIES_ACTION_HEADER_BYTES_V3 as u64),
        InstructionV3::load_const(
            s(SERIES_EXPIRE_WITNESS_ITEM_BYTES_SCALAR_V5),
            SERIES_WITNESS_ITEM_BYTES_V3 as u64,
        ),
        InstructionV3::checked_mul_into(
            s(SERIES_EXPIRE_PROOF_COUNT_SCALAR_V5),
            s(SERIES_EXPIRE_WITNESS_ITEM_BYTES_SCALAR_V5),
            s(SERIES_EXPIRE_PROOF_BYTES_SCALAR_V5),
        ),
        InstructionV3::scalar_eq(
            s(SERIES_EXPIRE_EXPECTED_ROOT_REVISION_SCALAR_V5),
            s(SERIES_EXPIRE_OBSERVED_ROOT_REVISION_SCALAR_V5),
        ),
        InstructionV3::scalar_eq(
            s(SERIES_EXPIRE_EXPECTED_TICKET_REVISION_SCALAR_V5),
            s(SERIES_EXPIRE_OBSERVED_TICKET_REVISION_SCALAR_V5),
        ),
        InstructionV3::identity_eq(
            i(SERIES_EXPIRE_EXPECTED_TICKET_IDENTITY_V5),
            i(SERIES_EXPIRE_OBSERVED_TICKET_IDENTITY_V5),
        ),
        InstructionV3::identity_eq(
            i(SERIES_EXPIRE_TEMPLATE_REFUND_OWNER_IDENTITY_V5),
            i(SERIES_EXPIRE_RENT_CREDIT_BENEFICIARY_IDENTITY_V5),
        ),
        InstructionV3::load_const(s(SERIES_EXPIRE_ZERO_SCALAR_V5), 0),
        InstructionV3::load_const(s(SERIES_EXPIRE_ONE_SCALAR_V5), 1),
        InstructionV3::load_const(s(SERIES_EXPIRE_EXPIRED_PHASE_SCALAR_V5), 2),
        InstructionV3::scalar_eq(
            s(SERIES_EXPIRE_OBSERVED_ROOT_PHASE_SCALAR_V5),
            s(SERIES_EXPIRE_ZERO_SCALAR_V5),
        ),
        InstructionV3::scalar_eq(
            s(SERIES_EXPIRE_OBSERVED_PREPARED_SCALAR_V5),
            s(SERIES_EXPIRE_ONE_SCALAR_V5),
        ),
        InstructionV3::scalar_eq(
            s(SERIES_EXPIRE_OBSERVED_TICKET_PHASE_SCALAR_V5),
            s(SERIES_EXPIRE_ZERO_SCALAR_V5),
        ),
        InstructionV3::scalar_eq(
            s(SERIES_EXPIRE_OBSERVED_NEXT_OCCURRENCE_SCALAR_V5),
            s(SERIES_EXPIRE_OCCURRENCE_INDEX_SCALAR_V5),
        ),
        InstructionV3::increment_into(
            s(SERIES_EXPIRE_OBSERVED_ROOT_REVISION_SCALAR_V5),
            s(SERIES_EXPIRE_CANDIDATE_ROOT_REVISION_SCALAR_V5),
        ),
        InstructionV3::increment_into(
            s(SERIES_EXPIRE_OBSERVED_NEXT_OCCURRENCE_SCALAR_V5),
            s(SERIES_EXPIRE_CANDIDATE_NEXT_OCCURRENCE_SCALAR_V5),
        ),
        InstructionV3::increment_into(
            s(SERIES_EXPIRE_OBSERVED_TICKET_REVISION_SCALAR_V5),
            s(SERIES_EXPIRE_CANDIDATE_TICKET_REVISION_SCALAR_V5),
        ),
        InstructionV3::copy_scalar(
            s(SERIES_EXPIRE_ZERO_SCALAR_V5),
            s(SERIES_EXPIRE_CANDIDATE_ROOT_PHASE_SCALAR_V5),
        ),
        InstructionV3::select_eq(
            s(SERIES_EXPIRE_CANDIDATE_NEXT_OCCURRENCE_SCALAR_V5),
            s(SERIES_EXPIRE_OCCURRENCE_COUNT_SCALAR_V5),
            s(SERIES_EXPIRE_ONE_SCALAR_V5),
            s(SERIES_EXPIRE_CANDIDATE_ROOT_PHASE_SCALAR_V5),
        ),
    ];
    let mut scratch = vec![0; SERIES_EXPIRE_TRANSITION_BYTES_V5];
    let mut output = vec![0; SERIES_EXPIRE_TRANSITION_BYTES_V5];
    encode_program_atomic(
        ProgramGeometryV3 {
            common_scalars: SERIES_EXPIRE_COMMON_SCALAR_COUNT_V5,
            item_scalar_stride: 0,
            common_identities: SERIES_EXPIRE_COMMON_IDENTITY_COUNT_V5,
            item_identity_stride: 0,
        },
        &instructions,
        &[],
        &[],
        &mut scratch,
        &mut output,
    )
    .map_err(|_| SeriesExpireFundingArtifactErrorV5::Transition)?;
    TransitionProgramV3::decode(&output)
        .map_err(|_| SeriesExpireFundingArtifactErrorV5::Transition)?;
    Ok(output)
}

fn emit_effect(
    requests: SeriesExpireChildRequestsV5<'_>,
    occurrence_count: u32,
) -> Result<Vec<u8>> {
    let bank = encode_request_bank(requests)?;
    let a = SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3;
    let p = SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3;
    let offsets = [
        0,
        a,
        2 * a,
        3 * a,
        3 * a + p,
        SERIES_EXPIRE_REQUEST_BANK_BYTES_V5,
    ];
    let routes = [
        route(FixedRole::Custody, 0, &bank[offsets[0]..offsets[1]]),
        route(FixedRole::Custody, 1, &bank[offsets[1]..offsets[2]]),
        route(FixedRole::Custody, 2, &bank[offsets[2]..offsets[3]]),
        route(FixedRole::Custody, 3, &bank[offsets[3]..offsets[4]]),
        route(FixedRole::Core, 4, &bank[offsets[4]..offsets[5]]),
    ];
    let account = AccountCoordinateV3::fixed;
    let scalar = ScalarCoordinateV3::common;
    let operations = [
        EffectInstructionV3::write_u8(
            account(0),
            ROOT_PHASE_OFFSET,
            scalar(SERIES_EXPIRE_CANDIDATE_ROOT_PHASE_SCALAR_V5),
        ),
        EffectInstructionV3::write_u8(
            account(0),
            ROOT_PREPARED_OFFSET,
            scalar(SERIES_EXPIRE_ZERO_SCALAR_V5),
        ),
        EffectInstructionV3::write_u32(
            account(0),
            ROOT_NEXT_OFFSET,
            scalar(SERIES_EXPIRE_CANDIDATE_NEXT_OCCURRENCE_SCALAR_V5),
        ),
        EffectInstructionV3::write_u64(
            account(0),
            ROOT_REVISION_OFFSET,
            scalar(SERIES_EXPIRE_CANDIDATE_ROOT_REVISION_SCALAR_V5),
        ),
        EffectInstructionV3::write_u8(
            account(SERIES_EXPIRE_TICKET_COORDINATE_V5),
            TICKET_PHASE_OFFSET,
            scalar(SERIES_EXPIRE_EXPIRED_PHASE_SCALAR_V5),
        ),
        EffectInstructionV3::write_u64(
            account(SERIES_EXPIRE_TICKET_COORDINATE_V5),
            TICKET_REVISION_OFFSET,
            scalar(SERIES_EXPIRE_CANDIDATE_TICKET_REVISION_SCALAR_V5),
        ),
        EffectInstructionV3::write_request_identity(
            3,
            RequestSpaceV3::Fixed,
            u32::try_from(ProjectedCustodyRequestLayoutV1::PARENT_CAPABILITY_ROOT_OFFSET)
                .map_err(|_| SeriesExpireFundingArtifactErrorV5::Geometry)?,
            IdentityCoordinateV3::common(SERIES_EXPIRE_ROOT_KEY_IDENTITY_V5),
        ),
        EffectInstructionV3::write_request_u64(
            4,
            RequestSpaceV3::Fixed,
            u32::try_from(SERIES_UNALLOCATED_PERMIT_EXPIRY_EXPECTED_SERIES_REVISION_OFFSET_V1)
                .map_err(|_| SeriesExpireFundingArtifactErrorV5::Geometry)?,
            scalar(SERIES_EXPIRE_EXPECTED_ROOT_REVISION_SCALAR_V5),
        ),
        EffectInstructionV3::write_request_u64(
            4,
            RequestSpaceV3::Fixed,
            u32::try_from(SERIES_UNALLOCATED_PERMIT_EXPIRY_EXPECTED_TICKET_REVISION_OFFSET_V1)
                .map_err(|_| SeriesExpireFundingArtifactErrorV5::Geometry)?,
            scalar(SERIES_EXPIRE_EXPECTED_TICKET_REVISION_SCALAR_V5),
        ),
    ];
    let dependencies = [&SERIES_NO_RECEIPT_DEPENDENCIES_V3[..]; 5];
    let mut base_scratch = vec![0; SERIES_EXPIRE_BASE_EFFECT_BYTES_V5];
    let mut base = vec![0; SERIES_EXPIRE_BASE_EFFECT_BYTES_V5];
    encode_effect_program_v4_atomic(
        EffectGeometryV3 {
            fixed_accounts: SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5,
            item_account_stride: 0,
            common_scalars: SERIES_EXPIRE_COMMON_SCALAR_COUNT_V5,
            item_scalar_stride: 0,
            common_identities: SERIES_EXPIRE_COMMON_IDENTITY_COUNT_V5,
            item_identity_stride: 0,
        },
        &routes,
        &dependencies,
        &operations,
        &[],
        &mut base_scratch,
        &mut base,
    )
    .map_err(|_| SeriesExpireFundingArtifactErrorV5::Effect)?;
    // Route 4 declares the proof it will borrow, and declares NOTHING when the
    // Template's canonical proof is empty. A `BorrowedRangeV4` is canonically
    // nonempty -- `resolve` refuses a zero length -- so "borrow zero bytes" has
    // no spelling, and the honest declaration for `proof_height(count) == 0` is
    // an empty range table. Coverage still closes: `validate_request_coverage`
    // starts its cursor at the 128-byte semantic prefix and requires it to
    // reach the family request's exact end, which for the empty proof it
    // already has.
    let range = BorrowedRangeV4::new(
        4,
        RequestCoordinateV4::Fixed(SERIES_ACTION_HEADER_BYTES_V3 as u32),
        RequestCoordinateV4::CommonScalar(SERIES_EXPIRE_PROOF_BYTES_SCALAR_V5),
    );
    let ranges: &[BorrowedRangeV4] = if series_expire_borrowed_range_count_v5(occurrence_count) == 0
    {
        &[]
    } else {
        core::slice::from_ref(&range)
    };
    let v4_bytes = series_expire_effect_v4_bytes_v5(occurrence_count);
    let mut v4_scratch = vec![0; v4_bytes];
    let mut v4 = vec![0; v4_bytes];
    encode_program_v4_atomic(
        &base,
        BorrowedRangePolicyV4::DisjointExactCoverage,
        SERIES_ACTION_HEADER_BYTES_V3 as u32,
        &[],
        ranges,
        &mut v4_scratch,
        &mut v4,
    )
    .map_err(|_| SeriesExpireFundingArtifactErrorV5::Effect)?;
    ProgramV4::decode(&v4).map_err(|_| SeriesExpireFundingArtifactErrorV5::Effect)?;
    let v5_bytes = series_expire_effect_bytes_v5(occurrence_count);
    let mut v5_scratch = vec![0; v5_bytes];
    let mut v5 = vec![0; v5_bytes];
    encode_program_v5_atomic(&v4, &[], &[], &mut v5_scratch, &mut v5)
        .map_err(|_| SeriesExpireFundingArtifactErrorV5::Effect)?;
    let effect = ProgramV5::decode(&v5).map_err(|_| SeriesExpireFundingArtifactErrorV5::Effect)?;
    if effect.funding_action_count() != 0 || effect.funding_seed_count() != 0 {
        return Err(SeriesExpireFundingArtifactErrorV5::Effect);
    }
    Ok(v5)
}

fn encode_request_bank(requests: SeriesExpireChildRequestsV5<'_>) -> Result<Vec<u8>> {
    // The permit body and Core occurrence request remain host-side authority
    // facts only. The hashed release artifact carries the distinct transient
    // transport with zero placeholders; authenticated RequestProfile scalars
    // patch both revisions immediately before the Core CPI.
    let _authority = (requests.permit_expiry, requests.core_expire);
    let core = SeriesUnallocatedPermitExpiryRequestV1::new(0, 0).encode();
    let mut projected_abort = *requests.projected_abort;
    projected_abort
        .get_mut(
            ProjectedCustodyRequestLayoutV1::PARENT_CAPABILITY_ROOT_OFFSET
                ..ProjectedCustodyRequestLayoutV1::PARENT_CAPABILITY_ROOT_OFFSET + 32,
        )
        .ok_or(SeriesExpireFundingArtifactErrorV5::Geometry)?
        .fill(0);
    let mut output = Vec::with_capacity(SERIES_EXPIRE_REQUEST_BANK_BYTES_V5);
    output.extend_from_slice(requests.refund);
    output.extend_from_slice(requests.close_vault);
    output.extend_from_slice(requests.close_replay);
    output.extend_from_slice(&projected_abort);
    output.extend_from_slice(&core);
    if output.len() != SERIES_EXPIRE_REQUEST_BANK_BYTES_V5 {
        return Err(SeriesExpireFundingArtifactErrorV5::Geometry);
    }
    Ok(output)
}

fn route<'a>(role: FixedRole, index: usize, request: &'a [u8]) -> RouteInputV3<'a> {
    RouteInputV3 {
        role,
        kind: RouteKindV3::Once,
        enable_common_scalar: None,
        witness_range_common_scalar: None,
        receipt_dependency: None,
        fixed_account_start: SERIES_EXPIRE_ROUTE_STARTS_V5[index],
        fixed_account_count: SERIES_EXPIRE_ROUTE_COUNTS_V5[index],
        item_account_start: 0,
        item_account_count: 0,
        fixed_request: request,
        item_request: &[],
    }
}

fn root_identity(destination: u16, offset: usize) -> Result<AccountOperationInputV2> {
    Ok(AccountOperationInputV2::ProjectDataIdentity {
        account: AccountCoordinateV2::fixed(0),
        destination: IdentityCoordinateV2::common(destination),
        data_offset: u32::try_from(offset)
            .map_err(|_| SeriesExpireFundingArtifactErrorV5::Geometry)?,
    })
}

const fn project_u64(account: u16, destination: u16, data_offset: u32) -> AccountOperationInputV2 {
    AccountOperationInputV2::ProjectDataU64 {
        account: AccountCoordinateV2::fixed(account),
        destination: ScalarCoordinateV2::common(destination),
        data_offset,
    }
}

const fn project_u32(account: u16, destination: u16, data_offset: u32) -> AccountOperationInputV2 {
    AccountOperationInputV2::ProjectDataU32 {
        account: AccountCoordinateV2::fixed(account),
        destination: ScalarCoordinateV2::common(destination),
        data_offset,
    }
}

const fn project_u8(account: u16, destination: u16, data_offset: u32) -> AccountOperationInputV2 {
    AccountOperationInputV2::ProjectDataU8 {
        account: AccountCoordinateV2::fixed(account),
        destination: ScalarCoordinateV2::common(destination),
        data_offset,
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

const fn no_effects() -> AccountEffectPermissionsV2 {
    AccountEffectPermissionsV2::new(false, false, false)
}

const fn write_data() -> AccountEffectPermissionsV2 {
    AccountEffectPermissionsV2::new(false, false, true)
}

const WRITABLE_REPRESENTATIVES: &[u16] = &[0, 5, 14, 16, 17, 33, 45, 51, 55];
const EXECUTABLE_REPRESENTATIVES: &[u16] = &[
    9,
    10,
    19,
    57,
    79,
    SERIES_EXPIRE_CUSTODY_PROGRAM_COORDINATE_V5,
];

const ROUTE_ALIASES: &[(u16, u16)] = &[
    (21, 7),
    (22, 8),
    (23, 9),
    (24, 10),
    (25, 11),
    (26, 12),
    (27, 13),
    (28, 14),
    (29, 15),
    (30, 16),
    (31, 18),
    (32, 19),
    (35, 7),
    (36, 8),
    (37, 9),
    (38, 10),
    (39, 11),
    (40, 12),
    (41, 13),
    (42, 14),
    (43, 33),
    (46, 8),
    (47, 9),
    (48, 10),
    (49, 11),
    (50, 33),
    (52, 18),
    (53, 19),
    (54, 7),
    (56, 33),
    (61, 9),
    (66, 8),
    (67, 10),
    (68, 11),
    (69, 0),
    (70, 5),
    (71, 1),
];

fn alias_representative(coordinate: u16) -> Option<u16> {
    ROUTE_ALIASES
        .iter()
        .find_map(|(alias, representative)| (*alias == coordinate).then_some(*representative))
}

#[cfg(test)]
mod tests {
    use dclutch_custody::{CustodyFrameRoleV1, CustodyFrameSpecV1, OperationV1};
    use dclutch_market::{FoundingIntentV5, Identity, SeriesCoreActionV1, SeriesFoundingPermitV1};
    use dclutch_vm::account_profile::v2::AccountPrestateV2;
    use dclutch_vm::effect::v2::AccountPermission;

    use super::*;

    fn profile() -> Vec<u8> {
        let mut lengths = [0; SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 as usize];
        lengths[73] = dclutch_trading::series::SERIES_OCCURRENCE_BYTES_V3 as u32;
        emit_account_profile(SeriesExpireAccountProfileInputV5 {
            fixed_data_lengths: &lengths,
        })
        .expect("Expire ProfileV3")
    }

    fn id(value: u8) -> Identity {
        Identity::new([value; 32]).expect("nonzero identity")
    }

    fn custody_representative(role: CustodyFrameRoleV1, route_start: u16) -> u16 {
        match role {
            CustodyFrameRoleV1::CallerAuthority => route_start,
            CustodyFrameRoleV1::CoreMarket => 7,
            CustodyFrameRoleV1::ActivationCache => 8,
            CustodyFrameRoleV1::RegistryProgram => 9,
            CustodyFrameRoleV1::CallerProgram => 10,
            CustodyFrameRoleV1::CallerProgramData => 11,
            CustodyFrameRoleV1::RealmRecord => 12,
            CustodyFrameRoleV1::RealmStaging => 13,
            CustodyFrameRoleV1::Replay => 14,
            CustodyFrameRoleV1::Mint => 15,
            CustodyFrameRoleV1::Vault | CustodyFrameRoleV1::TransferSource => 16,
            CustodyFrameRoleV1::TransferDestination => 17,
            CustodyFrameRoleV1::CustodyAuthority => 18,
            CustodyFrameRoleV1::TokenProgram => 19,
            CustodyFrameRoleV1::RentRefund => 33,
            CustodyFrameRoleV1::Payer
            | CustodyFrameRoleV1::SystemProgram
            | CustodyFrameRoleV1::RentSysvar => {
                panic!("role is absent from an Expire custody frame")
            }
        }
    }

    fn privilege_bits(writable: bool, executable: bool) -> u8 {
        u8::from(writable) << 1 | u8::from(executable) << 2
    }

    fn coordinate_matches(
        profile: AccountProfileV2<'_>,
        logical: u16,
        representative: u16,
        required_privileges: u8,
    ) -> bool {
        if profile.representative(0, usize::from(logical)) != Ok(usize::from(representative)) {
            return false;
        }
        let Ok(logical_rule) = profile.rule(false, logical) else {
            return false;
        };
        if logical != representative
            && (logical_rule.prestate() != AccountPrestateV2::AuthenticatedRouteAlias
                || logical_rule.privileges() != 0
                || logical_rule.effect_permissions() != 0)
        {
            return false;
        }
        let Ok(representative_rule) = profile.rule(false, representative) else {
            return false;
        };
        representative_rule.privileges() & required_privileges == required_privileges
    }

    fn profile_matches_canonical_child_frames(profile: AccountProfileV2<'_>) -> bool {
        for (operation, start) in [
            (OperationV1::Transfer, 6_u16),
            (OperationV1::CloseVault, 20),
            (OperationV1::CloseReplay, 34),
        ] {
            let frame = CustodyFrameSpecV1::new(operation);
            for local in 0..frame.account_count() {
                let Ok(account) = frame.account(local) else {
                    return false;
                };
                let privileges = account.privileges();
                let representative = custody_representative(account.role(), start);
                if !coordinate_matches(
                    profile,
                    start + local,
                    representative,
                    privilege_bits(privileges.writable(), privileges.executable()),
                ) {
                    return false;
                }
            }
        }

        // Canonical ProjectedCustody AbortOpenAndClose frame: caller, state,
        // activation, Registry, Trading, ProgramData, RentCredit, empty vault,
        // Custody authority, token program, and the vacant future Market.
        let projected_representatives = [44, 45, 8, 9, 10, 11, 33, 51, 18, 19, 7];
        let projected_privileges = [0, 2, 0, 4, 4, 0, 2, 2, 0, 4, 0];
        for local in 0..projected_representatives.len() {
            if !coordinate_matches(
                profile,
                44 + u16::try_from(local).expect("projected coordinate"),
                projected_representatives[local],
                projected_privileges[local],
            ) {
                return false;
            }
        }

        // Canonical Core unallocated-permit Expire precommit frame. Coordinate
        // 80 is the readonly outer Trading caller; CPI alone makes it a signer.
        let core_representatives = [
            55, 33, 57, 58, 59, 60, 9, 62, 63, 64, 65, 8, 10, 11, 0, 5, 1, 72, 73, 74, 75, 76, 77,
            78, 79, 80,
        ];
        let core_privileges = [
            2, 2, 4, 0, 0, 0, 4, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0,
        ];
        for local in 0..core_representatives.len() {
            if !coordinate_matches(
                profile,
                55 + u16::try_from(local).expect("Core coordinate"),
                core_representatives[local],
                core_privileges[local],
            ) {
                return false;
            }
        }

        for coordinate in 0..SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 {
            let Ok(rule) = profile.rule(false, coordinate) else {
                return false;
            };
            let expected = if alias_representative(coordinate).is_some() {
                0
            } else if WRITABLE_REPRESENTATIVES.contains(&coordinate) {
                2
            } else if EXECUTABLE_REPRESENTATIVES.contains(&coordinate) {
                4
            } else {
                0
            };
            if rule.privileges() != expected {
                return false;
            }
        }
        true
    }

    fn child_requests<'a>(
        refund: &'a [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
        close_vault: &'a [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
        close_replay: &'a [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
        projected_abort: &'a [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
    ) -> SeriesExpireChildRequestsV5<'a> {
        let intent = FoundingIntentV5::new(
            255,
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            id(6),
            id(7),
            id(8),
            id(9),
            id(10),
            id(11),
            id(12),
            id(13),
            id(14),
            id(15),
            8,
            1,
            1,
            100,
            4,
            1,
        )
        .expect("founding intent");
        let permit_expiry = SeriesPermitExpiryRequestV1::new(
            SeriesFoundingPermitV1::new(intent, id(16), id(17)).expect("permit"),
        );
        let core_expire = SeriesCoreRequestV1::occurrence(
            SeriesCoreActionV1::Expire,
            id(1),
            id(18),
            id(19),
            id(2),
            id(20),
            id(3),
            id(21),
            id(5),
            7,
            3,
            1,
            22,
            23,
            24,
            25,
        )
        .expect("Expire Core request");
        SeriesExpireChildRequestsV5 {
            refund,
            close_vault,
            close_replay,
            projected_abort,
            permit_expiry,
            core_expire,
        }
    }

    #[test]
    fn outer_ticket_write_is_downgraded_for_the_core_child() {
        let bytes = profile();
        let profile = AccountProfileV3::decode(&bytes).expect("ProfileV3");
        assert_eq!(profile.funding_bound_count(), 0);
        assert_eq!(
            profile.base().fixed_account_count(),
            SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5
        );
        assert_eq!(profile.base().representative(0, 70), Ok(5));
        let outer = profile.base().rule(false, 5).expect("Ticket rep");
        assert_eq!(outer.prestate(), AccountPrestateV2::Exact);
        assert_ne!(outer.privileges() & 2, 0);
        let child = profile.base().rule(false, 70).expect("Ticket alias");
        assert_eq!(child.privileges(), 0);
        assert_eq!(child.effect_permissions(), 0);
        assert_eq!(profile.base().representative(0, 56), Ok(33));
        assert_eq!(profile.base().representative(0, 69), Ok(0));
    }

    #[test]
    fn hot_prefix_and_exact_route_windows_are_pinned() {
        let bytes = profile();
        let profile = AccountProfileV3::decode(&bytes).expect("ProfileV3").base();
        let mut permissions =
            [AccountPermission::read_only(); SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 as usize];
        dclutch_vm::account_profile::v2::derive_effect_permissions(profile, 2, &mut permissions)
            .expect("permission geometry");
        for permission in permissions.iter().take(5).skip(1) {
            assert_eq!(*permission, AccountPermission::read_only());
        }
        assert_eq!(SERIES_EXPIRE_ROUTE_STARTS_V5, [6, 20, 34, 44, 55]);
        assert_eq!(SERIES_EXPIRE_ROUTE_COUNTS_V5, [14, 14, 10, 11, 26]);
        assert_eq!(SERIES_EXPIRE_PRECOMMIT_CALLER_COORDINATE_V5, 80);
    }

    /// Every child role this Effect routes to has a coordinate the Hot executor
    /// can resolve its callee through, and the Custody one renumbers no frame.
    ///
    /// `hot_v3::resolve_role_carrier_v3` scans the downgraded logical vector for
    /// the key the activation cache names for the role and refuses `Release`
    /// when nothing carries it. Custody is the only role Expire routes to --
    /// `FixedRole::Core` is resolved from the runtime prefix, not from the frame
    /// -- so exactly one coordinate must carry it, it must be a readonly
    /// executable, and it must sit past the last route range or the pinned
    /// starts and aliases above are wrong.
    #[test]
    fn the_custody_callee_is_one_readonly_executable_past_every_route_range() {
        let bytes = profile();
        let profile = AccountProfileV3::decode(&bytes).expect("ProfileV3");
        let base = profile.base();
        let callee = SERIES_EXPIRE_CUSTODY_PROGRAM_COORDINATE_V5;

        // The roles come from the REAL emitted Effect bytes, not from this
        // file's own list of them.
        let refund = [0_u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3];
        let projected_abort = [0_u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3];
        let effect_bytes = emit_effect(
            child_requests(&refund, &refund, &refund, &projected_abort),
            1,
        )
        .expect("Expire EffectV5");
        let effect = ProgramV5::decode(&effect_bytes).expect("EffectV5");
        let successor = effect.base().base();
        let mut custody_routes = 0_u16;
        let mut route = 0_u16;
        while route < successor.route_count() {
            if successor.route(route).expect("route").role() == FixedRole::Custody {
                custody_routes += 1;
            }
            route += 1;
        }
        assert_eq!(custody_routes, 4, "Expire routes to Custody four times");

        assert!(
            SERIES_EXPIRE_ROUTE_STARTS_V5
                .iter()
                .zip(SERIES_EXPIRE_ROUTE_COUNTS_V5)
                .all(|(start, count)| start + count <= callee),
            "the callee must not be inside any route window",
        );
        assert_eq!(
            base.representative(0, usize::from(callee)),
            Ok(usize::from(callee))
        );
        let rule = base.rule(false, callee).expect("Custody callee rule");
        assert_eq!(rule.privileges(), 4, "readonly executable");
        assert_eq!(rule.effect_permissions(), 0);
        assert_eq!(
            rule.prestate(),
            AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
            "the loader that deployed Custody owns its record width",
        );
        // One carrier, not two: the executor refuses as hard on a second
        // distinct physical account as it does on none.
        let carriers = (0..SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5)
            .filter(|coordinate| {
                base.representative(0, usize::from(*coordinate)) == Ok(usize::from(callee))
            })
            .count();
        assert_eq!(carriers, 1, "exactly one coordinate carries the callee");
    }

    #[test]
    fn every_child_coordinate_has_one_canonical_identity_and_physical_privilege() {
        let bytes = profile();
        let profile = AccountProfileV3::decode(&bytes).expect("ProfileV3");
        assert!(profile_matches_canonical_child_frames(profile.base()));
    }

    #[test]
    fn off_by_one_identity_and_privilege_substitutions_are_detected() {
        let bytes = profile();

        let mut wrong_identity = bytes.clone();
        let close_vault_core_market =
            HEADER_BYTES_V3 + DYNAMIC_FIXED_SPAN_HEADER_BYTES + 21 * RULE_BYTES + 4;
        wrong_identity[close_vault_core_market..close_vault_core_market + 2]
            .copy_from_slice(&8_u16.to_le_bytes());
        let wrong_identity = AccountProfileV3::decode(&wrong_identity).expect("structural profile");
        assert!(!profile_matches_canonical_child_frames(
            wrong_identity.base()
        ));

        let mut missing_registry_executable = bytes;
        let registry_rule = HEADER_BYTES_V3 + DYNAMIC_FIXED_SPAN_HEADER_BYTES + 9 * RULE_BYTES;
        missing_registry_executable[registry_rule] = 0;
        let missing_registry_executable =
            AccountProfileV3::decode(&missing_registry_executable).expect("structural profile");
        assert!(!profile_matches_canonical_child_frames(
            missing_registry_executable.base()
        ));
    }

    #[test]
    fn empty_profile_successor_refuses_hybrid_bytes() {
        let mut bytes = profile();
        assert_eq!(
            AccountProfileV3::decode(&bytes).map(|p| p.funding_bound_count()),
            Ok(0)
        );
        bytes[10] = 1;
        assert!(AccountProfileV3::decode(&bytes).is_err());
    }

    #[test]
    fn profile_refuses_a_forward_route_alias() {
        let mut bytes = profile();
        let core_ticket_alias =
            HEADER_BYTES_V3 + DYNAMIC_FIXED_SPAN_HEADER_BYTES + 70 * RULE_BYTES + 4;
        bytes[core_ticket_alias..core_ticket_alias + 2].copy_from_slice(&80_u16.to_le_bytes());
        assert!(AccountProfileV3::decode(&bytes).is_err());
    }

    /// The profile's pinned request width and the effect's coverage agree.
    ///
    /// This is the conjunction `6f258cf5e` convicted, and neither half alone
    /// shows it: the RequestProfile pins ONE exact family-request width and
    /// `validate_request_coverage` requires the semantic prefix plus every
    /// declared borrowed range to reach exactly that width. When the profile
    /// was fixed at 128 for every Template and route 4 declared a range
    /// unconditionally, `proof_count = 0` refused in the effect (a borrowed
    /// range is canonically nonempty) and `proof_count >= 1` refused in the
    /// profile. Both spellings had to move at once, so the test asks both at
    /// once, across the whole occurrence-count family and not only the two
    /// endpoints.
    #[test]
    fn profile_width_and_effect_coverage_agree_for_every_occurrence_count() {
        let refund = [0x21; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3];
        let close_vault = [0x22; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3];
        let close_replay = [0x23; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3];
        let projected_abort = [0x24; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3];
        for occurrence_count in [0, 1, 2, 3, 4, 5, 8, 9, 1_000, u32::MAX] {
            let proof_count = series_proof_count_v3(occurrence_count);
            let request_bytes = series_action_request_bytes_v3(occurrence_count);
            assert_eq!(
                request_bytes,
                SERIES_ACTION_HEADER_BYTES_V3 + 32 * proof_count as usize
            );

            // The two authors of "does this Template have a proof" compute the
            // same function, and `core_composition_v3`'s expiry shape gate is
            // the third: it reads the family request's own width, which the
            // RequestProfile below has already pinned to `request_bytes`.
            assert_eq!(
                series_expire_borrowed_range_count_v5(occurrence_count),
                usize::from(request_bytes > SERIES_ACTION_HEADER_BYTES_V3)
            );

            let profile = emit_request_profile(occurrence_count).expect("request profile");
            let profile = RequestProfileV1::decode(&profile).expect("request decode");
            // Item stride is zero, so every Product tail count must agree.
            for tail_count in [0, 1, 7] {
                assert_eq!(profile.request_bytes(tail_count), Ok(request_bytes));
            }

            let effect = emit_effect(
                child_requests(&refund, &close_vault, &close_replay, &projected_abort),
                occurrence_count,
            )
            .expect("Expire EffectV5");
            let effect = ProgramV5::decode(&effect).expect("EffectV5");
            let v4 = effect.base();
            assert_eq!(
                usize::from(v4.range_count()),
                series_expire_borrowed_range_count_v5(occurrence_count)
            );
            assert_eq!(
                v4.borrowed_range_count_for_route(4),
                Ok(
                    u16::try_from(series_expire_borrowed_range_count_v5(occurrence_count))
                        .expect("range count")
                )
            );

            let mut scalars = [0_u64; SERIES_EXPIRE_COMMON_SCALAR_COUNT_V5 as usize];
            scalars[usize::from(SERIES_EXPIRE_PROOF_BYTES_SCALAR_V5)] = 32 * u64::from(proof_count);
            let identities = [[0_u8; 32]; SERIES_EXPIRE_COMMON_IDENTITY_COUNT_V5 as usize];
            assert_eq!(
                v4.validate_request_coverage(request_bytes, 0, &scalars, &identities),
                Ok(()),
                "occurrence_count {occurrence_count}"
            );
            // The width the profile pins is the ONLY width coverage accepts.
            assert!(
                v4.validate_request_coverage(request_bytes + 32, 0, &scalars, &identities)
                    .is_err()
            );
            if request_bytes > SERIES_ACTION_HEADER_BYTES_V3 {
                assert!(
                    v4.validate_request_coverage(
                        SERIES_ACTION_HEADER_BYTES_V3,
                        0,
                        &scalars,
                        &identities
                    )
                    .is_err()
                );
            }
        }
    }

    #[test]
    fn request_and_transition_are_exact_action_selected_programs() {
        let request = emit_request_profile(1).expect("request profile");
        let request = RequestProfileV1::decode(&request).expect("request decode");
        assert_eq!(request.fixed_request_bytes(), 128);
        assert_eq!(request.common_scalar_count(), 26);
        let transition = emit_transition().expect("transition");
        let transition = TransitionProgramV3::decode(&transition).expect("transition decode");
        assert_eq!(transition.common_scalar_count(), 26);
        assert_eq!(transition.common_identity_count(), 12);
        assert_eq!(transition.bytes().len(), SERIES_EXPIRE_TRANSITION_BYTES_V5);
        assert_eq!(
            transition.bytes().len() - TRANSITION_HEADER_BYTES,
            19 * TRANSITION_INSTRUCTION_BYTES
        );
    }

    #[test]
    fn effect_pins_routes_borrowed_proof_and_empty_funding() {
        let refund = [0x21; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3];
        let close_vault = [0x22; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3];
        let close_replay = [0x23; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3];
        let projected_abort = [0x24; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3];
        let bytes = emit_effect(
            child_requests(&refund, &close_vault, &close_replay, &projected_abort),
            4,
        )
        .expect("Expire EffectV5");
        let effect = ProgramV5::decode(&bytes).expect("EffectV5");
        assert_eq!(effect.funding_action_count(), 0);
        assert_eq!(effect.funding_seed_count(), 0);
        let v4 = effect.base();
        assert_eq!(v4.range_count(), 1);
        assert_eq!(v4.borrowed_range_count_for_route(4), Ok(1));
        for route_index in 0..4 {
            assert_eq!(v4.borrowed_range_count_for_route(route_index), Ok(0));
        }
        let base = v4.base();
        assert_eq!(
            base.fixed_account_count(),
            SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5
        );
        assert_eq!(base.route_count(), 5);
        assert_eq!(base.fixed_operation_count(), 9);
        for route_index in 0..5 {
            let route = base.route(route_index).expect("route");
            assert_eq!(
                route.fixed_account_start(),
                SERIES_EXPIRE_ROUTE_STARTS_V5[usize::from(route_index)]
            );
            assert_eq!(
                route.fixed_account_count(),
                SERIES_EXPIRE_ROUTE_COUNTS_V5[usize::from(route_index)]
            );
        }
        assert_eq!(
            base.route(4).expect("Core route").fixed_request_bytes(),
            SERIES_UNALLOCATED_PERMIT_EXPIRY_REQUEST_BYTES_V1 as u32
        );

        let mut phantom_funding = bytes;
        phantom_funding[6..8].copy_from_slice(&1_u16.to_le_bytes());
        assert!(ProgramV5::decode(&phantom_funding).is_err());
    }
}
