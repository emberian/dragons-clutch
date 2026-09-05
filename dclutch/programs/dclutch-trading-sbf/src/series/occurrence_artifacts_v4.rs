//! Canonical fixed-geometry artifacts for Series Prepare and Expire.
//!
//! Consume has an affine FundingState span and therefore owns its specialized
//! emitters in [`super::consume_artifacts_v4`]. Prepare and Expire have no
//! affine account dimension: each is one fixed five-route walk. This module is
//! their single author for the RequestProfile, Transition, request bank, and
//! successor Effect artifacts selected by a current `CapabilityProgramV4`.
//!
//! The occurrence proof remains part of the authenticated family request. A
//! Prepare walk consumes it only in the Series semantic admission, so its one
//! successor range is explicitly semantic-owned. Expire's final Core permit
//! cleanup consumes the same exact proof bytes, so that range belongs to route
//! four and is appended by the family-neutral Hot range seam.

use dclutch_trading::series::request::SeriesActionV3;
use dclutch_vm::effect::{
    v2::FixedRole,
    v3::{
        HEADER_BYTES as EFFECT_HEADER_BYTES_V3, ROUTE_BYTES as EFFECT_ROUTE_BYTES_V3, RouteKindV3,
        encode::{EffectGeometryV3, RouteInputV3, encode_effect_program_v4_atomic},
    },
    v4::{
        BORROWED_RANGE_BYTES_V4, BorrowedRangePolicyV4, BorrowedRangeV4,
        HEADER_BYTES_V4 as EFFECT_HEADER_BYTES_V4, RequestCoordinateV4, SEMANTIC_RANGE_ROUTE_V4,
        encode_program_v4_atomic,
    },
};
use dclutch_vm::request_profile::{
    HEADER_BYTES as REQUEST_PROFILE_HEADER_BYTES_V4,
    OPERATION_BYTES as REQUEST_PROFILE_OPERATION_BYTES_V4,
    encode::{
        RequestCoordinateV1, RequestGeometryV1, RequestInstructionV1, ScalarRegisterV1,
        encode_request_profile_v1_atomic,
    },
};
use dclutch_vm::v3::{
    HEADER_BYTES as TRANSITION_HEADER_BYTES_V4,
    INSTRUCTION_BYTES as TRANSITION_INSTRUCTION_BYTES_V4, InstructionV3, ProgramGeometryV3,
    ScalarRegisterV3, encode_program_atomic,
};

use super::{
    artifacts_v3::{
        SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3, SERIES_EXPIRE_CLOSE_REPLAY_ACCOUNT_COUNT_V3,
        SERIES_EXPIRE_CLOSE_VAULT_ACCOUNT_COUNT_V3, SERIES_EXPIRE_IR_REQUEST_BYTES_V3,
        SERIES_EXPIRE_PERMIT_ACCOUNT_COUNT_V3, SERIES_EXPIRE_PERMIT_REQUEST_BYTES_V3,
        SERIES_EXPIRE_PROJECTED_ABORT_ACCOUNT_COUNT_V3, SERIES_EXPIRE_REFUND_ACCOUNT_COUNT_V3,
        SERIES_NO_RECEIPT_DEPENDENCIES_V3, SERIES_PREPARE_ESCROW_LOCK_ACCOUNT_COUNT_V3,
        SERIES_PREPARE_ESCROW_OPEN_ACCOUNT_COUNT_V3, SERIES_PREPARE_IR_REQUEST_BYTES_V3,
        SERIES_PREPARE_PROJECTED_INITIALIZE_ACCOUNT_COUNT_V3,
        SERIES_PREPARE_PROJECTED_OPEN_ACCOUNT_COUNT_V3,
        SERIES_PREPARE_REPLAY_INITIALIZE_ACCOUNT_COUNT_V3,
        SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3, SERIES_WITNESS_ITEM_BYTES_V3,
    },
    instruction::SERIES_ACTION_HEADER_BYTES_V3,
    terminal::{SERIES_CLOSE_ACCOUNT_COUNT_V3, SERIES_RETIRE_ACCOUNT_COUNT_V3},
};

/// Common scalar geometry shared with the selected Consume profile.
pub const SERIES_OCCURRENCE_COMMON_SCALAR_COUNT_V4: u16 = 7;
/// Common identity geometry shared with the selected Consume profile.
pub const SERIES_OCCURRENCE_COMMON_IDENTITY_COUNT_V4: u16 = 6;
/// Exact action-selected RequestProfile width.
pub const SERIES_OCCURRENCE_REQUEST_PROFILE_BYTES_V4: usize =
    REQUEST_PROFILE_HEADER_BYTES_V4 + 2 * REQUEST_PROFILE_OPERATION_BYTES_V4;
/// Exact action-selected TransitionVM width.
pub const SERIES_OCCURRENCE_TRANSITION_BYTES_V4: usize =
    TRANSITION_HEADER_BYTES_V4 + 3 * TRANSITION_INSTRUCTION_BYTES_V4;
/// Exact Retire/Close RequestProfile width.
pub const SERIES_TERMINAL_REQUEST_PROFILE_BYTES_V4: usize =
    REQUEST_PROFILE_HEADER_BYTES_V4 + 2 * REQUEST_PROFILE_OPERATION_BYTES_V4;
/// Exact Retire/Close Transition width.
///
/// TransitionVM refuses an empty program, so the one instruction derives the
/// already-fixed 128-byte semantic header width into scalar zero. It grants no
/// account or child authority.
pub const SERIES_TERMINAL_TRANSITION_BYTES_V4: usize =
    TRANSITION_HEADER_BYTES_V4 + TRANSITION_INSTRUCTION_BYTES_V4;
/// Exact zero-route terminal Effect V3 width.
pub const SERIES_TERMINAL_BASE_EFFECT_BYTES_V4: usize = EFFECT_HEADER_BYTES_V3;
/// Exact zero-route terminal successor Effect width.
pub const SERIES_TERMINAL_EFFECT_BYTES_V4: usize =
    SERIES_TERMINAL_BASE_EFFECT_BYTES_V4 + EFFECT_HEADER_BYTES_V4;

/// Exact global Prepare logical-account count before alias compaction.
pub const SERIES_PREPARE_LOGICAL_ACCOUNT_COUNT_V4: u16 =
    SERIES_PREPARE_PROJECTED_INITIALIZE_ACCOUNT_COUNT_V3
        + SERIES_PREPARE_PROJECTED_OPEN_ACCOUNT_COUNT_V3
        + SERIES_PREPARE_REPLAY_INITIALIZE_ACCOUNT_COUNT_V3
        + SERIES_PREPARE_ESCROW_OPEN_ACCOUNT_COUNT_V3
        + SERIES_PREPARE_ESCROW_LOCK_ACCOUNT_COUNT_V3;
/// Exact global Expire logical-account count before alias compaction.
pub const SERIES_EXPIRE_LOGICAL_ACCOUNT_COUNT_V4: u16 = SERIES_EXPIRE_REFUND_ACCOUNT_COUNT_V3
    + SERIES_EXPIRE_CLOSE_VAULT_ACCOUNT_COUNT_V3
    + SERIES_EXPIRE_CLOSE_REPLAY_ACCOUNT_COUNT_V3
    + SERIES_EXPIRE_PROJECTED_ABORT_ACCOUNT_COUNT_V3
    + SERIES_EXPIRE_PERMIT_ACCOUNT_COUNT_V3;

/// Exact embedded Prepare Effect V3 width.
pub const SERIES_PREPARE_BASE_EFFECT_BYTES_V4: usize =
    EFFECT_HEADER_BYTES_V3 + 5 * EFFECT_ROUTE_BYTES_V3 + SERIES_PREPARE_IR_REQUEST_BYTES_V3;
/// Exact successor Prepare Effect width.
pub const SERIES_PREPARE_EFFECT_BYTES_V4: usize =
    SERIES_PREPARE_BASE_EFFECT_BYTES_V4 + EFFECT_HEADER_BYTES_V4 + BORROWED_RANGE_BYTES_V4;
/// Exact embedded Expire Effect V3 width.
pub const SERIES_EXPIRE_BASE_EFFECT_BYTES_V4: usize =
    EFFECT_HEADER_BYTES_V3 + 5 * EFFECT_ROUTE_BYTES_V3 + SERIES_EXPIRE_IR_REQUEST_BYTES_V3;
/// Exact successor Expire Effect width.
pub const SERIES_EXPIRE_EFFECT_BYTES_V4: usize =
    SERIES_EXPIRE_BASE_EFFECT_BYTES_V4 + EFFECT_HEADER_BYTES_V4 + BORROWED_RANGE_BYTES_V4;

const ACTION_SELECTOR_OFFSET: u32 = 12;
const PROOF_COUNT_OFFSET: u32 = 13;
const PROOF_COUNT_SCALAR: u16 = 2;
const HEADER_BYTES_SCALAR: u16 = 0;
const PROOF_BYTES_SCALAR: u16 = 1;
const PROOF_ITEM_BYTES_SCALAR: u16 = 3;
const PROOF_OFFSET: u32 = 128;

/// Exact gap-free start of each Prepare child route in the V4 logical frame.
pub const SERIES_PREPARE_ROUTE_STARTS_V4: [u16; 5] = [
    PREPARE_PROJECTED_INITIALIZE_START,
    PREPARE_PROJECTED_OPEN_START,
    PREPARE_REPLAY_INITIALIZE_START,
    PREPARE_ESCROW_OPEN_START,
    PREPARE_ESCROW_LOCK_START,
];
/// Exact child-owned width of each Prepare route.
pub const SERIES_PREPARE_ROUTE_COUNTS_V4: [u16; 5] = [
    SERIES_PREPARE_PROJECTED_INITIALIZE_ACCOUNT_COUNT_V3,
    SERIES_PREPARE_PROJECTED_OPEN_ACCOUNT_COUNT_V3,
    SERIES_PREPARE_REPLAY_INITIALIZE_ACCOUNT_COUNT_V3,
    SERIES_PREPARE_ESCROW_OPEN_ACCOUNT_COUNT_V3,
    SERIES_PREPARE_ESCROW_LOCK_ACCOUNT_COUNT_V3,
];

const PREPARE_PROJECTED_INITIALIZE_START: u16 = 0;
const PREPARE_PROJECTED_OPEN_START: u16 =
    PREPARE_PROJECTED_INITIALIZE_START + SERIES_PREPARE_PROJECTED_INITIALIZE_ACCOUNT_COUNT_V3;
const PREPARE_REPLAY_INITIALIZE_START: u16 =
    PREPARE_PROJECTED_OPEN_START + SERIES_PREPARE_PROJECTED_OPEN_ACCOUNT_COUNT_V3;
const PREPARE_ESCROW_OPEN_START: u16 =
    PREPARE_REPLAY_INITIALIZE_START + SERIES_PREPARE_REPLAY_INITIALIZE_ACCOUNT_COUNT_V3;
const PREPARE_ESCROW_LOCK_START: u16 =
    PREPARE_ESCROW_OPEN_START + SERIES_PREPARE_ESCROW_OPEN_ACCOUNT_COUNT_V3;

const EXPIRE_REFUND_START: u16 = 0;
const EXPIRE_CLOSE_VAULT_START: u16 = EXPIRE_REFUND_START + SERIES_EXPIRE_REFUND_ACCOUNT_COUNT_V3;
const EXPIRE_CLOSE_REPLAY_START: u16 =
    EXPIRE_CLOSE_VAULT_START + SERIES_EXPIRE_CLOSE_VAULT_ACCOUNT_COUNT_V3;
const EXPIRE_PROJECTED_ABORT_START: u16 =
    EXPIRE_CLOSE_REPLAY_START + SERIES_EXPIRE_CLOSE_REPLAY_ACCOUNT_COUNT_V3;
const EXPIRE_PERMIT_START: u16 =
    EXPIRE_PROJECTED_ABORT_START + SERIES_EXPIRE_PROJECTED_ABORT_ACCOUNT_COUNT_V3;

const _: () = assert!(SERIES_ACTION_HEADER_BYTES_V3 == 128);
const _: () = assert!(SERIES_PREPARE_LOGICAL_ACCOUNT_COUNT_V4 == 105);
const _: () = assert!(SERIES_PREPARE_ROUTE_STARTS_V4[0] == 0);
const _: () = assert!(SERIES_PREPARE_ROUTE_STARTS_V4[1] == 47);
const _: () = assert!(SERIES_PREPARE_ROUTE_STARTS_V4[2] == 62);
const _: () = assert!(SERIES_PREPARE_ROUTE_STARTS_V4[3] == 75);
const _: () = assert!(SERIES_PREPARE_ROUTE_STARTS_V4[4] == 91);
const _: () = assert!(SERIES_PREPARE_ROUTE_COUNTS_V4[0] == 47);
const _: () = assert!(SERIES_PREPARE_ROUTE_COUNTS_V4[1] == 15);
const _: () = assert!(SERIES_PREPARE_ROUTE_COUNTS_V4[2] == 13);
const _: () = assert!(SERIES_PREPARE_ROUTE_COUNTS_V4[3] == 16);
const _: () = assert!(SERIES_PREPARE_ROUTE_COUNTS_V4[4] == 14);
const _: () = assert!(SERIES_EXPIRE_LOGICAL_ACCOUNT_COUNT_V4 == 74);

/// Exact Prepare child requests in canonical route order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesPrepareChildRequestsV4<'a> {
    /// Projected Custody initialize request.
    pub projected_initialize: &'a [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
    /// Projected Custody open-Hoard request.
    pub projected_open: &'a [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
    /// Custody replay initialization request.
    pub replay_initialize: &'a [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    /// Custody SeriesEscrow vault-open request.
    pub escrow_open: &'a [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    /// Custody founder-to-SeriesEscrow transfer request.
    pub escrow_lock: &'a [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
}

/// Exact Expire child requests in canonical route order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesExpireChildRequestsV4<'a> {
    /// Custody SeriesEscrow refund request.
    pub refund: &'a [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    /// Custody SeriesEscrow vault-close request.
    pub close_vault: &'a [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    /// Custody SeriesEscrow replay-close request.
    pub close_replay: &'a [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    /// Projected Custody abort-and-close request.
    pub projected_abort: &'a [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
    /// Core permissionless permit-expiry request, before the proof suffix.
    pub permit_expiry: &'a [u8; SERIES_EXPIRE_PERMIT_REQUEST_BYTES_V3],
}

/// Stable refusal from fixed Series occurrence artifact emission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesOccurrenceArtifactErrorV4 {
    /// Caller buffers or the selected action had the wrong exact geometry.
    Geometry,
    /// RequestProfile emission refused.
    RequestProfile,
    /// Transition emission refused.
    Transition,
    /// Embedded Effect V3 emission refused.
    BaseEffect,
    /// Successor Effect V4 emission or hostile decode refused.
    Effect,
}

/// Emit the canonical RequestProfile for Prepare or Expire.
pub fn encode_series_occurrence_request_profile_v4_atomic(
    action: SeriesActionV3,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), SeriesOccurrenceArtifactErrorV4> {
    if !matches!(action, SeriesActionV3::Prepare | SeriesActionV3::Expire) {
        return Err(SeriesOccurrenceArtifactErrorV4::Geometry);
    }
    let instructions = [
        RequestInstructionV1::require_u8(
            RequestCoordinateV1::fixed(ACTION_SELECTOR_OFFSET),
            action as u8,
        ),
        RequestInstructionV1::project_u8(
            RequestCoordinateV1::fixed(PROOF_COUNT_OFFSET),
            ScalarRegisterV1::common(PROOF_COUNT_SCALAR),
        ),
    ];
    encode_request_profile_v1_atomic(
        RequestGeometryV1::new(
            u32::try_from(SERIES_ACTION_HEADER_BYTES_V3)
                .map_err(|_| SeriesOccurrenceArtifactErrorV4::RequestProfile)?,
            0,
            SERIES_OCCURRENCE_COMMON_SCALAR_COUNT_V4,
            0,
            SERIES_OCCURRENCE_COMMON_IDENTITY_COUNT_V4,
            0,
        ),
        &instructions,
        &[],
        scratch,
        output,
    )
    .map_err(|_| SeriesOccurrenceArtifactErrorV4::RequestProfile)
}

/// Emit the canonical proof-width Transition shared by Prepare and Expire.
pub fn encode_series_occurrence_transition_v4_atomic(
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), SeriesOccurrenceArtifactErrorV4> {
    let prelude = [
        InstructionV3::load_const(
            ScalarRegisterV3::common(HEADER_BYTES_SCALAR),
            u64::try_from(SERIES_ACTION_HEADER_BYTES_V3)
                .map_err(|_| SeriesOccurrenceArtifactErrorV4::Transition)?,
        ),
        InstructionV3::load_const(
            ScalarRegisterV3::common(PROOF_ITEM_BYTES_SCALAR),
            u64::try_from(SERIES_WITNESS_ITEM_BYTES_V3)
                .map_err(|_| SeriesOccurrenceArtifactErrorV4::Transition)?,
        ),
        InstructionV3::checked_mul_into(
            ScalarRegisterV3::common(PROOF_COUNT_SCALAR),
            ScalarRegisterV3::common(PROOF_ITEM_BYTES_SCALAR),
            ScalarRegisterV3::common(PROOF_BYTES_SCALAR),
        ),
    ];
    encode_program_atomic(
        ProgramGeometryV3 {
            common_scalars: SERIES_OCCURRENCE_COMMON_SCALAR_COUNT_V4,
            item_scalar_stride: 0,
            common_identities: SERIES_OCCURRENCE_COMMON_IDENTITY_COUNT_V4,
            item_identity_stride: 0,
        },
        &prelude,
        &[],
        &[],
        scratch,
        output,
    )
    .map_err(|_| SeriesOccurrenceArtifactErrorV4::Transition)
}

/// Emit the canonical fixed-header RequestProfile for Retire or Close.
///
/// Terminal actions carry no occurrence proof. Requiring the proof-count byte
/// to be zero makes that absence an authenticated fact instead of relying on a
/// later exact-length decoder to notice an appended witness.
pub fn encode_series_terminal_request_profile_v4_atomic(
    action: SeriesActionV3,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), SeriesOccurrenceArtifactErrorV4> {
    if !matches!(action, SeriesActionV3::Retire | SeriesActionV3::Close) {
        return Err(SeriesOccurrenceArtifactErrorV4::Geometry);
    }
    let instructions = [
        RequestInstructionV1::require_u8(
            RequestCoordinateV1::fixed(ACTION_SELECTOR_OFFSET),
            action as u8,
        ),
        RequestInstructionV1::require_u8(RequestCoordinateV1::fixed(PROOF_COUNT_OFFSET), 0),
    ];
    encode_request_profile_v1_atomic(
        RequestGeometryV1::new(
            u32::try_from(SERIES_ACTION_HEADER_BYTES_V3)
                .map_err(|_| SeriesOccurrenceArtifactErrorV4::RequestProfile)?,
            0,
            SERIES_OCCURRENCE_COMMON_SCALAR_COUNT_V4,
            0,
            SERIES_OCCURRENCE_COMMON_IDENTITY_COUNT_V4,
            0,
        ),
        &instructions,
        &[],
        scratch,
        output,
    )
    .map_err(|_| SeriesOccurrenceArtifactErrorV4::RequestProfile)
}

/// Emit the canonical header-width Transition for Retire and Close.
pub fn encode_series_terminal_transition_v4_atomic(
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), SeriesOccurrenceArtifactErrorV4> {
    let prelude = [InstructionV3::load_const(
        ScalarRegisterV3::common(HEADER_BYTES_SCALAR),
        u64::try_from(SERIES_ACTION_HEADER_BYTES_V3)
            .map_err(|_| SeriesOccurrenceArtifactErrorV4::Transition)?,
    )];
    encode_program_atomic(
        ProgramGeometryV3 {
            common_scalars: SERIES_OCCURRENCE_COMMON_SCALAR_COUNT_V4,
            item_scalar_stride: 0,
            common_identities: SERIES_OCCURRENCE_COMMON_IDENTITY_COUNT_V4,
            item_identity_stride: 0,
        },
        &prelude,
        &[],
        &[],
        scratch,
        output,
    )
    .map_err(|_| SeriesOccurrenceArtifactErrorV4::Transition)
}

/// Emit the canonical zero-route terminal successor Effect.
///
/// The fixed account width is the exact differential-oracle frame for the
/// selected terminal action. This artifact grants those accounts no child CPI
/// or local mutation authority. It is the selected action shape only; the
/// authenticated family-local commit seam that applies the already-admitted
/// Retire/Close plan remains a separate required production boundary.
pub fn encode_series_terminal_effect_v4_atomic(
    action: SeriesActionV3,
    base_scratch: &mut [u8],
    base_output: &mut [u8],
    successor_scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), SeriesOccurrenceArtifactErrorV4> {
    let fixed_accounts = match action {
        SeriesActionV3::Retire => u16::try_from(SERIES_RETIRE_ACCOUNT_COUNT_V3)
            .map_err(|_| SeriesOccurrenceArtifactErrorV4::Geometry)?,
        SeriesActionV3::Close => u16::try_from(SERIES_CLOSE_ACCOUNT_COUNT_V3)
            .map_err(|_| SeriesOccurrenceArtifactErrorV4::Geometry)?,
        _ => return Err(SeriesOccurrenceArtifactErrorV4::Geometry),
    };
    require_buffers(
        base_scratch,
        base_output,
        SERIES_TERMINAL_BASE_EFFECT_BYTES_V4,
        successor_scratch,
        output,
        SERIES_TERMINAL_EFFECT_BYTES_V4,
    )?;
    encode_effect_program_v4_atomic(
        EffectGeometryV3 {
            fixed_accounts,
            item_account_stride: 0,
            common_scalars: SERIES_OCCURRENCE_COMMON_SCALAR_COUNT_V4,
            item_scalar_stride: 0,
            common_identities: SERIES_OCCURRENCE_COMMON_IDENTITY_COUNT_V4,
            item_identity_stride: 0,
        },
        &[],
        &[],
        &[],
        &[],
        base_scratch,
        base_output,
    )
    .map_err(|_| SeriesOccurrenceArtifactErrorV4::BaseEffect)?;
    encode_program_v4_atomic(
        base_output,
        BorrowedRangePolicyV4::DisjointExactCoverage,
        u32::try_from(SERIES_ACTION_HEADER_BYTES_V3)
            .map_err(|_| SeriesOccurrenceArtifactErrorV4::Effect)?,
        &[],
        &[],
        successor_scratch,
        output,
    )
    .map_err(|_| SeriesOccurrenceArtifactErrorV4::Effect)?;
    dclutch_vm::effect::v4::ProgramV4::decode(output)
        .map_err(|_| SeriesOccurrenceArtifactErrorV4::Effect)?;
    Ok(())
}

/// Emit the exact global five-route Prepare successor Effect atomically.
pub fn encode_series_prepare_effect_v4_atomic(
    requests: SeriesPrepareChildRequestsV4<'_>,
    base_scratch: &mut [u8],
    base_output: &mut [u8],
    successor_scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), SeriesOccurrenceArtifactErrorV4> {
    require_buffers(
        base_scratch,
        base_output,
        SERIES_PREPARE_BASE_EFFECT_BYTES_V4,
        successor_scratch,
        output,
        SERIES_PREPARE_EFFECT_BYTES_V4,
    )?;
    let routes = [
        route(
            FixedRole::Custody,
            PREPARE_PROJECTED_INITIALIZE_START,
            SERIES_PREPARE_PROJECTED_INITIALIZE_ACCOUNT_COUNT_V3,
            requests.projected_initialize,
        ),
        route(
            FixedRole::Custody,
            PREPARE_PROJECTED_OPEN_START,
            SERIES_PREPARE_PROJECTED_OPEN_ACCOUNT_COUNT_V3,
            requests.projected_open,
        ),
        route(
            FixedRole::Custody,
            PREPARE_REPLAY_INITIALIZE_START,
            SERIES_PREPARE_REPLAY_INITIALIZE_ACCOUNT_COUNT_V3,
            requests.replay_initialize,
        ),
        route(
            FixedRole::Custody,
            PREPARE_ESCROW_OPEN_START,
            SERIES_PREPARE_ESCROW_OPEN_ACCOUNT_COUNT_V3,
            requests.escrow_open,
        ),
        route(
            FixedRole::Custody,
            PREPARE_ESCROW_LOCK_START,
            SERIES_PREPARE_ESCROW_LOCK_ACCOUNT_COUNT_V3,
            requests.escrow_lock,
        ),
    ];
    encode_base_effect(
        SERIES_PREPARE_LOGICAL_ACCOUNT_COUNT_V4,
        &routes,
        base_scratch,
        base_output,
    )?;
    let ranges = [proof_range(SEMANTIC_RANGE_ROUTE_V4)];
    encode_successor(base_output, &ranges, successor_scratch, output)
}

/// Emit the exact global five-route Expire successor Effect atomically.
pub fn encode_series_expire_effect_v4_atomic(
    requests: SeriesExpireChildRequestsV4<'_>,
    base_scratch: &mut [u8],
    base_output: &mut [u8],
    successor_scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), SeriesOccurrenceArtifactErrorV4> {
    require_buffers(
        base_scratch,
        base_output,
        SERIES_EXPIRE_BASE_EFFECT_BYTES_V4,
        successor_scratch,
        output,
        SERIES_EXPIRE_EFFECT_BYTES_V4,
    )?;
    let routes = [
        route(
            FixedRole::Custody,
            EXPIRE_REFUND_START,
            SERIES_EXPIRE_REFUND_ACCOUNT_COUNT_V3,
            requests.refund,
        ),
        route(
            FixedRole::Custody,
            EXPIRE_CLOSE_VAULT_START,
            SERIES_EXPIRE_CLOSE_VAULT_ACCOUNT_COUNT_V3,
            requests.close_vault,
        ),
        route(
            FixedRole::Custody,
            EXPIRE_CLOSE_REPLAY_START,
            SERIES_EXPIRE_CLOSE_REPLAY_ACCOUNT_COUNT_V3,
            requests.close_replay,
        ),
        route(
            FixedRole::Custody,
            EXPIRE_PROJECTED_ABORT_START,
            SERIES_EXPIRE_PROJECTED_ABORT_ACCOUNT_COUNT_V3,
            requests.projected_abort,
        ),
        route(
            FixedRole::Core,
            EXPIRE_PERMIT_START,
            SERIES_EXPIRE_PERMIT_ACCOUNT_COUNT_V3,
            requests.permit_expiry,
        ),
    ];
    encode_base_effect(
        SERIES_EXPIRE_LOGICAL_ACCOUNT_COUNT_V4,
        &routes,
        base_scratch,
        base_output,
    )?;
    let ranges = [proof_range(4)];
    encode_successor(base_output, &ranges, successor_scratch, output)
}

fn require_buffers(
    base_scratch: &[u8],
    base_output: &[u8],
    base_width: usize,
    successor_scratch: &[u8],
    output: &[u8],
    successor_width: usize,
) -> Result<(), SeriesOccurrenceArtifactErrorV4> {
    if base_scratch.len() != base_width
        || base_output.len() != base_width
        || successor_scratch.len() != successor_width
        || output.len() != successor_width
    {
        return Err(SeriesOccurrenceArtifactErrorV4::Geometry);
    }
    Ok(())
}

fn encode_base_effect(
    fixed_accounts: u16,
    routes: &[RouteInputV3<'_>],
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), SeriesOccurrenceArtifactErrorV4> {
    let dependencies = [
        &SERIES_NO_RECEIPT_DEPENDENCIES_V3[..],
        &SERIES_NO_RECEIPT_DEPENDENCIES_V3[..],
        &SERIES_NO_RECEIPT_DEPENDENCIES_V3[..],
        &SERIES_NO_RECEIPT_DEPENDENCIES_V3[..],
        &SERIES_NO_RECEIPT_DEPENDENCIES_V3[..],
    ];
    encode_effect_program_v4_atomic(
        EffectGeometryV3 {
            fixed_accounts,
            item_account_stride: 0,
            common_scalars: SERIES_OCCURRENCE_COMMON_SCALAR_COUNT_V4,
            item_scalar_stride: 0,
            common_identities: SERIES_OCCURRENCE_COMMON_IDENTITY_COUNT_V4,
            item_identity_stride: 0,
        },
        routes,
        &dependencies,
        &[],
        &[],
        scratch,
        output,
    )
    .map_err(|_| SeriesOccurrenceArtifactErrorV4::BaseEffect)
}

fn encode_successor(
    base: &[u8],
    ranges: &[BorrowedRangeV4],
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), SeriesOccurrenceArtifactErrorV4> {
    encode_program_v4_atomic(
        base,
        BorrowedRangePolicyV4::DisjointExactCoverage,
        u32::try_from(SERIES_ACTION_HEADER_BYTES_V3)
            .map_err(|_| SeriesOccurrenceArtifactErrorV4::Effect)?,
        &[],
        ranges,
        scratch,
        output,
    )
    .map_err(|_| SeriesOccurrenceArtifactErrorV4::Effect)?;
    dclutch_vm::effect::v4::ProgramV4::decode(output)
        .map_err(|_| SeriesOccurrenceArtifactErrorV4::Effect)?;
    Ok(())
}

const fn proof_range(route: u16) -> BorrowedRangeV4 {
    BorrowedRangeV4::new(
        route,
        RequestCoordinateV4::Fixed(PROOF_OFFSET),
        RequestCoordinateV4::CommonScalar(PROOF_BYTES_SCALAR),
    )
}

const fn route<'a>(
    role: FixedRole,
    account_start: u16,
    account_count: u16,
    request: &'a [u8],
) -> RouteInputV3<'a> {
    RouteInputV3 {
        role,
        kind: RouteKindV3::Once,
        enable_common_scalar: None,
        witness_range_common_scalar: None,
        receipt_dependency: None,
        fixed_account_start: account_start,
        fixed_account_count: account_count,
        item_account_start: 0,
        item_account_count: 0,
        fixed_request: request,
        item_request: &[],
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use alloc::vec;

    use dclutch_vm::effect::v4::ProgramV4;
    use dclutch_vm::request_profile::{ProjectionRegistersV1, RequestProfileV1, project_atomic};
    use dclutch_vm::v3::ProgramV3 as TransitionProgramV3;

    use super::*;

    fn prepare_requests() -> (
        [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
        [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
        [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
        [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
        [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    ) {
        (
            [0x11; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
            [0x22; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
            [0x33; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
            [0x44; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
            [0x55; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
        )
    }

    fn expire_requests() -> (
        [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
        [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
        [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
        [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
        [u8; SERIES_EXPIRE_PERMIT_REQUEST_BYTES_V3],
    ) {
        (
            [0x61; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
            [0x62; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
            [0x63; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
            [0x64; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
            [0x65; SERIES_EXPIRE_PERMIT_REQUEST_BYTES_V3],
        )
    }

    #[test]
    fn action_profiles_require_the_exact_selector_and_derive_proof_width() {
        for action in [SeriesActionV3::Prepare, SeriesActionV3::Expire] {
            let mut request_scratch = [0_u8; SERIES_OCCURRENCE_REQUEST_PROFILE_BYTES_V4];
            let mut request = [0_u8; SERIES_OCCURRENCE_REQUEST_PROFILE_BYTES_V4];
            encode_series_occurrence_request_profile_v4_atomic(
                action,
                &mut request_scratch,
                &mut request,
            )
            .expect("request profile");
            let profile = RequestProfileV1::decode(&request).expect("hostile decode");
            let mut family = [0_u8; SERIES_ACTION_HEADER_BYTES_V3];
            family[usize::try_from(ACTION_SELECTOR_OFFSET).expect("offset")] = action as u8;
            family[usize::try_from(PROOF_COUNT_OFFSET).expect("offset")] = 3;
            let input_scalars = [0_u64; SERIES_OCCURRENCE_COMMON_SCALAR_COUNT_V4 as usize];
            let input_identities =
                [[0_u8; 32]; SERIES_OCCURRENCE_COMMON_IDENTITY_COUNT_V4 as usize];
            let mut scratch_scalars = input_scalars;
            let mut output_scalars = input_scalars;
            let mut scratch_identities = input_identities;
            let mut output_identities = input_identities;
            project_atomic(
                profile,
                0,
                &family,
                ProjectionRegistersV1 {
                    input_scalars: &input_scalars,
                    input_identities: &input_identities,
                    scratch_scalars: &mut scratch_scalars,
                    scratch_identities: &mut scratch_identities,
                    output_scalars: &mut output_scalars,
                    output_identities: &mut output_identities,
                },
            )
            .expect("exact selector");
            assert_eq!(output_scalars[usize::from(PROOF_COUNT_SCALAR)], 3);
            family[usize::try_from(ACTION_SELECTOR_OFFSET).expect("offset")] ^= 1;
            assert!(
                project_atomic(
                    profile,
                    0,
                    &family,
                    ProjectionRegistersV1 {
                        input_scalars: &input_scalars,
                        input_identities: &input_identities,
                        scratch_scalars: &mut scratch_scalars,
                        scratch_identities: &mut scratch_identities,
                        output_scalars: &mut output_scalars,
                        output_identities: &mut output_identities,
                    },
                )
                .is_err()
            );
        }

        let mut scratch = [0_u8; SERIES_OCCURRENCE_TRANSITION_BYTES_V4];
        let mut transition = [0_u8; SERIES_OCCURRENCE_TRANSITION_BYTES_V4];
        encode_series_occurrence_transition_v4_atomic(&mut scratch, &mut transition)
            .expect("transition");
        TransitionProgramV3::decode(&transition).expect("hostile decode");
    }

    #[test]
    fn prepare_proof_is_semantic_owned_and_routes_are_exact() {
        let (projected_initialize, projected_open, replay_initialize, escrow_open, escrow_lock) =
            prepare_requests();
        let mut base_scratch = vec![0_u8; SERIES_PREPARE_BASE_EFFECT_BYTES_V4];
        let mut base = vec![0_u8; SERIES_PREPARE_BASE_EFFECT_BYTES_V4];
        let mut successor_scratch = vec![0_u8; SERIES_PREPARE_EFFECT_BYTES_V4];
        let mut output = vec![0_u8; SERIES_PREPARE_EFFECT_BYTES_V4];
        encode_series_prepare_effect_v4_atomic(
            SeriesPrepareChildRequestsV4 {
                projected_initialize: &projected_initialize,
                projected_open: &projected_open,
                replay_initialize: &replay_initialize,
                escrow_open: &escrow_open,
                escrow_lock: &escrow_lock,
            },
            &mut base_scratch,
            &mut base,
            &mut successor_scratch,
            &mut output,
        )
        .expect("Prepare Effect");
        let effect = ProgramV4::decode(&output).expect("successor");
        assert_eq!(effect.base().route_count(), 5);
        assert_eq!(effect.base().fixed_account_count(), 105);
        for (route_index, (&start, &count)) in SERIES_PREPARE_ROUTE_STARTS_V4
            .iter()
            .zip(SERIES_PREPARE_ROUTE_COUNTS_V4.iter())
            .enumerate()
        {
            let route = effect
                .base()
                .route(u16::try_from(route_index).expect("bounded route"))
                .expect("Prepare route");
            assert_eq!(route.fixed_account_start(), start);
            assert_eq!(route.fixed_account_count(), count);
        }
        assert_eq!(
            SERIES_PREPARE_ROUTE_STARTS_V4[4] + SERIES_PREPARE_ROUTE_COUNTS_V4[4],
            105
        );
        assert_eq!(effect.range_count(), 1);
        assert_eq!(
            effect.borrowed_range(0),
            Ok(proof_range(SEMANTIC_RANGE_ROUTE_V4))
        );
        for route in 0..5 {
            assert_eq!(effect.borrowed_range_count_for_route(route), Ok(0));
        }
    }

    #[test]
    fn expire_proof_belongs_only_to_the_permissionless_core_route() {
        let (refund, close_vault, close_replay, projected_abort, permit_expiry) = expire_requests();
        let mut base_scratch = vec![0_u8; SERIES_EXPIRE_BASE_EFFECT_BYTES_V4];
        let mut base = vec![0_u8; SERIES_EXPIRE_BASE_EFFECT_BYTES_V4];
        let mut successor_scratch = vec![0_u8; SERIES_EXPIRE_EFFECT_BYTES_V4];
        let mut output = vec![0_u8; SERIES_EXPIRE_EFFECT_BYTES_V4];
        encode_series_expire_effect_v4_atomic(
            SeriesExpireChildRequestsV4 {
                refund: &refund,
                close_vault: &close_vault,
                close_replay: &close_replay,
                projected_abort: &projected_abort,
                permit_expiry: &permit_expiry,
            },
            &mut base_scratch,
            &mut base,
            &mut successor_scratch,
            &mut output,
        )
        .expect("Expire Effect");
        let effect = ProgramV4::decode(&output).expect("successor");
        assert_eq!(effect.base().route_count(), 5);
        assert_eq!(effect.base().fixed_account_count(), 74);
        assert_eq!(
            effect.base().route(4).expect("Core route").role(),
            FixedRole::Core
        );
        for route in 0..4 {
            assert_eq!(effect.borrowed_range_count_for_route(route), Ok(0));
        }
        assert_eq!(effect.borrowed_range_count_for_route(4), Ok(1));
        assert_eq!(effect.borrowed_range(0), Ok(proof_range(4)));
    }

    #[test]
    fn substitution_changes_the_selected_effect_and_short_output_rolls_back() {
        let (refund, close_vault, close_replay, projected_abort, permit_expiry) = expire_requests();
        let requests = SeriesExpireChildRequestsV4 {
            refund: &refund,
            close_vault: &close_vault,
            close_replay: &close_replay,
            projected_abort: &projected_abort,
            permit_expiry: &permit_expiry,
        };
        let mut base_scratch = vec![0_u8; SERIES_EXPIRE_BASE_EFFECT_BYTES_V4];
        let mut base = vec![0_u8; SERIES_EXPIRE_BASE_EFFECT_BYTES_V4];
        let mut successor_scratch = vec![0_u8; SERIES_EXPIRE_EFFECT_BYTES_V4];
        let mut output = vec![0_u8; SERIES_EXPIRE_EFFECT_BYTES_V4];
        encode_series_expire_effect_v4_atomic(
            requests,
            &mut base_scratch,
            &mut base,
            &mut successor_scratch,
            &mut output,
        )
        .expect("baseline");
        let baseline = output.clone();

        let mut substituted_permit = permit_expiry;
        substituted_permit[31] ^= 0x80;
        encode_series_expire_effect_v4_atomic(
            SeriesExpireChildRequestsV4 {
                permit_expiry: &substituted_permit,
                ..requests
            },
            &mut base_scratch,
            &mut base,
            &mut successor_scratch,
            &mut output,
        )
        .expect("substituted bytes remain structurally encodable");
        assert_ne!(output, baseline);

        let mut short = vec![0xa5; SERIES_EXPIRE_EFFECT_BYTES_V4 - 1];
        let before = short.clone();
        assert_eq!(
            encode_series_expire_effect_v4_atomic(
                requests,
                &mut base_scratch,
                &mut base,
                &mut successor_scratch,
                &mut short,
            ),
            Err(SeriesOccurrenceArtifactErrorV4::Geometry)
        );
        assert_eq!(short, before);
    }

    #[test]
    fn terminal_artifacts_grant_no_child_or_local_effect_authority() {
        for action in [SeriesActionV3::Retire, SeriesActionV3::Close] {
            let mut request_scratch = [0_u8; SERIES_TERMINAL_REQUEST_PROFILE_BYTES_V4];
            let mut request = [0_u8; SERIES_TERMINAL_REQUEST_PROFILE_BYTES_V4];
            encode_series_terminal_request_profile_v4_atomic(
                action,
                &mut request_scratch,
                &mut request,
            )
            .expect("terminal request profile");
            let profile = RequestProfileV1::decode(&request).expect("request profile");
            let mut family = [0_u8; SERIES_ACTION_HEADER_BYTES_V3];
            family[usize::try_from(ACTION_SELECTOR_OFFSET).expect("offset")] = action as u8;
            dclutch_vm::request_profile::validate_request(profile, 0, &family)
                .expect("exact terminal header");
            family[usize::try_from(PROOF_COUNT_OFFSET).expect("offset")] = 1;
            assert!(dclutch_vm::request_profile::validate_request(profile, 0, &family).is_err());
        }

        let mut transition_scratch = [0_u8; SERIES_TERMINAL_TRANSITION_BYTES_V4];
        let mut transition = [0_u8; SERIES_TERMINAL_TRANSITION_BYTES_V4];
        encode_series_terminal_transition_v4_atomic(&mut transition_scratch, &mut transition)
            .expect("identity transition");
        let transition = TransitionProgramV3::decode(&transition).expect("transition");
        assert_eq!(
            transition.common_scalar_count(),
            SERIES_OCCURRENCE_COMMON_SCALAR_COUNT_V4
        );

        for (action, fixed_accounts) in [
            (SeriesActionV3::Retire, SERIES_RETIRE_ACCOUNT_COUNT_V3),
            (SeriesActionV3::Close, SERIES_CLOSE_ACCOUNT_COUNT_V3),
        ] {
            let mut base_scratch = [0_u8; SERIES_TERMINAL_BASE_EFFECT_BYTES_V4];
            let mut base = [0_u8; SERIES_TERMINAL_BASE_EFFECT_BYTES_V4];
            let mut successor_scratch = [0_u8; SERIES_TERMINAL_EFFECT_BYTES_V4];
            let mut output = [0_u8; SERIES_TERMINAL_EFFECT_BYTES_V4];
            encode_series_terminal_effect_v4_atomic(
                action,
                &mut base_scratch,
                &mut base,
                &mut successor_scratch,
                &mut output,
            )
            .expect("terminal Effect");
            let effect = ProgramV4::decode(&output).expect("successor");
            assert_eq!(
                effect.base().fixed_account_count(),
                u16::try_from(fixed_accounts).expect("small frame")
            );
            assert_eq!(effect.base().route_count(), 0);
            assert_eq!(effect.base().fixed_operation_count(), 0);
            assert_eq!(effect.base().item_operation_count(), 0);
            assert_eq!(effect.range_count(), 0);
            assert_eq!(effect.span_count(), 0);
            assert_eq!(
                effect.validate_request_coverage(
                    SERIES_ACTION_HEADER_BYTES_V3,
                    0,
                    &[0; SERIES_OCCURRENCE_COMMON_SCALAR_COUNT_V4 as usize],
                    &[[0; 32]; SERIES_OCCURRENCE_COMMON_IDENTITY_COUNT_V4 as usize],
                ),
                Ok(())
            );
        }
    }
}
