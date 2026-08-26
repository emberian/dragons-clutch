//! Canonical selector-9 request, transition, and Effect V4 artifact joins.
//!
//! Scenario exact-fill has one selector for both P1 and P2 SignedDelta frames.
//! The exact request projects Position count and packet width into protected
//! registers; Effect V4 selects the Claims extension from `{1,2}`. Six typed
//! possible Custody routes independently select `{0,14}` accounts, so disabled
//! physical legs cost no placeholder account. The SignedDelta packet is the
//! sole borrowed request range and covers every byte after the 384-byte
//! semantic header.

use dclutch_claims_svm::signed_delta_v3::{
    SIGNED_DELTA_PLAN_MAGIC_V3, SIGNED_DELTA_RECEIPT_BYTES_V3, SIGNED_DELTA_RECEIPT_MAGIC_V3,
    plan_bytes as signed_delta_plan_bytes,
};
use dclutch_dealer_codec::MAX_OUTCOMES;
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{ProgramV3 as EffectProgramV3, RouteKindV3},
    v4::{
        BORROWED_RANGE_BYTES_V4, BorrowedRangePolicyV4, BorrowedRangeV4, DYNAMIC_SPAN_BYTES_V4,
        DynamicFixedSpanV4, HEADER_BYTES_V4, ProgramV4 as EffectProgramV4, RequestCoordinateV4,
        encode_program_v4_atomic,
    },
};
use dclutch_request_profile_contract::{
    encode::{
        RequestCoordinateV1, RequestGeometryV1, RequestInstructionV1, ScalarRegisterV1,
        encode_request_profile_v1_atomic,
    },
    v3::{
        BorrowedWitnessPolicyV3, BorrowedWitnessRoleV3, REQUEST_PROFILE_V3_HEADER_BYTES,
        RequestProfileV3, encode_request_profile_v3_atomic,
    },
};
use dclutch_transition_vm::v3::{
    InstructionV3, ProgramGeometryV3, ProgramV3 as TransitionProgramV3, ScalarRegisterV3,
    encode_program_atomic,
};

use super::{
    v3_hot_artifact::{
        DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3, DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3,
        DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3,
    },
    v3_trade::{
        DEALER_SCENARIO_TRADE_ACTION_V3, DEALER_SCENARIO_TRADE_CLAIMS_PACKET_BYTES_OFFSET_V3,
        DEALER_SCENARIO_TRADE_HEADER_BYTES_V3, DEALER_SCENARIO_TRADE_MAGIC_V3,
        DEALER_SCENARIO_TRADE_POSITION_COUNT_OFFSET_V3, DEALER_SCENARIO_TRADE_SELECTOR_OFFSET_V3,
        DEALER_SCENARIO_TRADE_VERSION_V3,
    },
};

/// Global route count: four pre-Claims Custody shapes, Claims, then two
/// post-Claims Custody shapes.
pub const DEALER_SCENARIO_ROUTE_COUNT_V4: u16 = 7;
/// Claims route in the one global selector-9 program.
pub const DEALER_SCENARIO_CLAIMS_ROUTE_V4: u16 = 4;
/// Number of optional typed Custody routes.
pub const DEALER_SCENARIO_CUSTODY_ROUTE_COUNT_V4: usize = 6;
/// Base V3 fixed-account geometry before protected extensions.
pub const DEALER_SCENARIO_BASE_FIXED_ACCOUNTS_V4: u16 =
    DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3 + DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3 + 1;
/// Runtime obligation outcome values occupy one scalar per Product item.
pub const DEALER_SCENARIO_ITEM_SCALAR_STRIDE_V4: u16 = 1;
/// No per-item identities are needed.
pub const DEALER_SCENARIO_ITEM_IDENTITY_STRIDE_V4: u16 = 0;

/// Request-projected SignedDelta Position count P1/P2.
pub const DEALER_SCENARIO_POSITION_COUNT_SCALAR_V4: u16 = 0;
/// Request-projected SignedDelta packet byte width.
pub const DEALER_SCENARIO_WITNESS_BYTES_SCALAR_V4: u16 = 1;
/// Transition-owned exact witness offset, always 384.
pub const DEALER_SCENARIO_WITNESS_OFFSET_SCALAR_V4: u16 = 2;
/// Trusted current slot injected by AccountProfile.
pub const DEALER_SCENARIO_CURRENT_SLOT_SCALAR_V4: u16 = 3;
/// Request-projected expiry.
pub const DEALER_SCENARIO_EXPIRY_SCALAR_V4: u16 = 4;
/// Transition scratch scalar holding the constant maximum P=2.
pub const DEALER_SCENARIO_MAX_POSITION_COUNT_SCALAR_V4: u16 = 5;
/// Candidate obligation revision written last.
pub const DEALER_SCENARIO_OBLIGATION_REVISION_SCALAR_V4: u16 = 6;
/// First protected `{0,14}` optional-route span scalar.
pub const DEALER_SCENARIO_ROUTE_SPAN_SCALAR_BASE_V4: u16 = 7;
/// First per-Custody-route request scalar block.
pub const DEALER_SCENARIO_CUSTODY_SCALAR_BASE_V4: u16 = 13;
/// Exact canonical Custody request scalar stride.
pub const DEALER_SCENARIO_CUSTODY_SCALAR_STRIDE_V4: u16 = 9;
/// Exact common scalar-bank width.
pub const DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4: u16 = 67;
/// Parent digest precedes six exact 17-identity Custody blocks.
pub const DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4: u16 = 103;

const REQUEST_PROFILE_OPERATIONS_V4: usize = 7;
const REQUEST_PROFILE_V1_BYTES_V4: usize = dclutch_request_profile_contract::HEADER_BYTES
    + REQUEST_PROFILE_OPERATIONS_V4 * dclutch_request_profile_contract::OPERATION_BYTES;
/// Exact selector-9 RequestProfile V3 bytes.
pub const DEALER_SCENARIO_REQUEST_PROFILE_BYTES_V4: usize =
    REQUEST_PROFILE_V3_HEADER_BYTES + REQUEST_PROFILE_V1_BYTES_V4;
const TRANSITION_OPERATIONS_V4: usize = 6;
/// Exact selector-9 TransitionVM bytes.
pub const DEALER_SCENARIO_TRANSITION_BYTES_V4: usize = dclutch_transition_vm::v3::HEADER_BYTES
    + TRANSITION_OPERATIONS_V4 * dclutch_transition_vm::v3::INSTRUCTION_BYTES;
const DYNAMIC_SPAN_COUNT_V4: usize = DEALER_SCENARIO_CUSTODY_ROUTE_COUNT_V4 + 1;
const BORROWED_RANGE_COUNT_V4: usize = 1;

/// Stable artifact construction or join refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerScenarioArtifactsErrorV4 {
    /// Exact request profile refused or differed.
    RequestProfile,
    /// Exact transition refused or differed.
    Transition,
    /// Base V3 or successor V4 Effect geometry differed.
    Effect,
    /// Checked width or coordinate arithmetic overflowed.
    Arithmetic,
}

/// Exact combined SignedDelta packet bounds for selector 9.
pub fn dealer_scenario_witness_bounds_v4() -> Result<(u32, u32), DealerScenarioArtifactsErrorV4> {
    let outcomes =
        u32::try_from(MAX_OUTCOMES).map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?;
    let minimum =
        signed_delta_plan_bytes(1, 1, 1).map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?;
    let maximum = signed_delta_plan_bytes(
        outcomes,
        2,
        outcomes
            .checked_mul(2)
            .ok_or(DealerScenarioArtifactsErrorV4::Arithmetic)?,
    )
    .map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?;
    Ok((
        u32::try_from(minimum).map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?,
        u32::try_from(maximum).map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?,
    ))
}

/// Encode the exact selector-9 prefix projector and borrowed-witness policy.
pub fn encode_dealer_scenario_request_profile_v4(
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DealerScenarioArtifactsErrorV4> {
    if scratch.len() != DEALER_SCENARIO_REQUEST_PROFILE_BYTES_V4
        || output.len() != DEALER_SCENARIO_REQUEST_PROFILE_BYTES_V4
    {
        return Err(DealerScenarioArtifactsErrorV4::RequestProfile);
    }
    let instructions = [
        RequestInstructionV1::require_u64(
            RequestCoordinateV1::fixed(0),
            u64::from_le_bytes(DEALER_SCENARIO_TRADE_MAGIC_V3),
        ),
        RequestInstructionV1::require_u16(
            RequestCoordinateV1::fixed(8),
            DEALER_SCENARIO_TRADE_VERSION_V3,
        ),
        RequestInstructionV1::require_u16(
            RequestCoordinateV1::fixed(DEALER_SCENARIO_TRADE_SELECTOR_OFFSET_V3),
            DEALER_SCENARIO_TRADE_ACTION_V3,
        ),
        RequestInstructionV1::project_u8(
            RequestCoordinateV1::fixed(
                u32::try_from(DEALER_SCENARIO_TRADE_POSITION_COUNT_OFFSET_V3)
                    .map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?,
            ),
            ScalarRegisterV1::common(DEALER_SCENARIO_POSITION_COUNT_SCALAR_V4),
        ),
        RequestInstructionV1::project_u32(
            RequestCoordinateV1::fixed(
                u32::try_from(DEALER_SCENARIO_TRADE_CLAIMS_PACKET_BYTES_OFFSET_V3)
                    .map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?,
            ),
            ScalarRegisterV1::common(DEALER_SCENARIO_WITNESS_BYTES_SCALAR_V4),
        ),
        RequestInstructionV1::project_u64(
            RequestCoordinateV1::fixed(352),
            ScalarRegisterV1::common(DEALER_SCENARIO_EXPIRY_SCALAR_V4),
        ),
        RequestInstructionV1::require_zero(RequestCoordinateV1::fixed(378), 2),
    ];
    let mut embedded_scratch = [0_u8; REQUEST_PROFILE_V1_BYTES_V4];
    let mut embedded = [0_u8; REQUEST_PROFILE_V1_BYTES_V4];
    encode_request_profile_v1_atomic(
        RequestGeometryV1::new(
            u32::try_from(DEALER_SCENARIO_TRADE_HEADER_BYTES_V3)
                .map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?,
            0,
            DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4,
            DEALER_SCENARIO_ITEM_SCALAR_STRIDE_V4,
            DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4,
            DEALER_SCENARIO_ITEM_IDENTITY_STRIDE_V4,
        ),
        &instructions,
        &[],
        &mut embedded_scratch,
        &mut embedded,
    )
    .map_err(|_| DealerScenarioArtifactsErrorV4::RequestProfile)?;
    let (minimum_bytes, maximum_bytes) = dealer_scenario_witness_bounds_v4()?;
    encode_request_profile_v3_atomic(
        &embedded,
        BorrowedWitnessPolicyV3 {
            minimum_bytes,
            maximum_bytes,
            consumer_role: BorrowedWitnessRoleV3::Claims,
            child_request_magic: SIGNED_DELTA_PLAN_MAGIC_V3,
            child_receipt_magic: SIGNED_DELTA_RECEIPT_MAGIC_V3,
            child_receipt_bytes: u32::try_from(SIGNED_DELTA_RECEIPT_BYTES_V3)
                .map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?,
        },
        scratch,
        output,
    )
    .map_err(|_| DealerScenarioArtifactsErrorV4::RequestProfile)
}

/// Encode the exact geometry/expiry transition. The admitted semantic
/// evaluator owns route activity and candidate economic values; Effect V4
/// independently restricts every emitted span to its finite set.
pub fn encode_dealer_scenario_transition_v4(
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DealerScenarioArtifactsErrorV4> {
    if scratch.len() != DEALER_SCENARIO_TRANSITION_BYTES_V4
        || output.len() != DEALER_SCENARIO_TRANSITION_BYTES_V4
    {
        return Err(DealerScenarioArtifactsErrorV4::Transition);
    }
    let instructions = [
        InstructionV3::load_const(
            ScalarRegisterV3::common(DEALER_SCENARIO_WITNESS_OFFSET_SCALAR_V4),
            u64::try_from(DEALER_SCENARIO_TRADE_HEADER_BYTES_V3)
                .map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?,
        ),
        InstructionV3::nonzero(ScalarRegisterV3::common(
            DEALER_SCENARIO_POSITION_COUNT_SCALAR_V4,
        )),
        InstructionV3::load_const(
            ScalarRegisterV3::common(DEALER_SCENARIO_MAX_POSITION_COUNT_SCALAR_V4),
            2,
        ),
        InstructionV3::scalar_le(
            ScalarRegisterV3::common(DEALER_SCENARIO_POSITION_COUNT_SCALAR_V4),
            ScalarRegisterV3::common(DEALER_SCENARIO_MAX_POSITION_COUNT_SCALAR_V4),
        ),
        InstructionV3::nonzero(ScalarRegisterV3::common(
            DEALER_SCENARIO_WITNESS_BYTES_SCALAR_V4,
        )),
        InstructionV3::scalar_le(
            ScalarRegisterV3::common(DEALER_SCENARIO_CURRENT_SLOT_SCALAR_V4),
            ScalarRegisterV3::common(DEALER_SCENARIO_EXPIRY_SCALAR_V4),
        ),
    ];
    encode_program_atomic(
        ProgramGeometryV3 {
            common_scalars: DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4,
            item_scalar_stride: DEALER_SCENARIO_ITEM_SCALAR_STRIDE_V4,
            common_identities: DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4,
            item_identity_stride: DEALER_SCENARIO_ITEM_IDENTITY_STRIDE_V4,
        },
        &instructions,
        &[],
        &[],
        scratch,
        output,
    )
    .map_err(|_| DealerScenarioArtifactsErrorV4::Transition)
}

/// Exact Effect V4 width around one canonical base V3 program.
pub fn dealer_scenario_effect_program_bytes_v4(
    base_program_bytes: usize,
) -> Result<usize, DealerScenarioArtifactsErrorV4> {
    HEADER_BYTES_V4
        .checked_add(
            DYNAMIC_SPAN_COUNT_V4
                .checked_mul(DYNAMIC_SPAN_BYTES_V4)
                .ok_or(DealerScenarioArtifactsErrorV4::Arithmetic)?,
        )
        .and_then(|value| {
            value.checked_add(BORROWED_RANGE_COUNT_V4.checked_mul(BORROWED_RANGE_BYTES_V4)?)
        })
        .and_then(|value| value.checked_add(base_program_bytes))
        .ok_or(DealerScenarioArtifactsErrorV4::Arithmetic)
}

/// Wrap one canonical selector-9 base program in the exact protected V4
/// geometry. The base owns typed request templates/effects; this function
/// refuses any alternate global route/account/register shape.
pub fn encode_dealer_scenario_effect_program_v4(
    base_program: &[u8],
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DealerScenarioArtifactsErrorV4> {
    let expected = dealer_scenario_effect_program_bytes_v4(base_program.len())?;
    if scratch.len() != expected || output.len() != expected {
        return Err(DealerScenarioArtifactsErrorV4::Effect);
    }
    validate_base_effect(base_program)?;
    let optional_routes = [0_u16, 1, 2, 3, 5, 6];
    let spans = [
        optional_span(optional_routes[0], 0)?,
        optional_span(optional_routes[1], 1)?,
        optional_span(optional_routes[2], 2)?,
        optional_span(optional_routes[3], 3)?,
        DynamicFixedSpanV4::new(
            DEALER_SCENARIO_CLAIMS_ROUTE_V4,
            DEALER_SCENARIO_POSITION_COUNT_SCALAR_V4,
            DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3,
            (1_u64 << 1) | (1_u64 << 2),
        ),
        optional_span(optional_routes[4], 4)?,
        optional_span(optional_routes[5], 5)?,
    ];
    let ranges = [BorrowedRangeV4::new(
        DEALER_SCENARIO_CLAIMS_ROUTE_V4,
        RequestCoordinateV4::Fixed(
            u32::try_from(DEALER_SCENARIO_TRADE_HEADER_BYTES_V3)
                .map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?,
        ),
        RequestCoordinateV4::CommonScalar(DEALER_SCENARIO_WITNESS_BYTES_SCALAR_V4),
    )];
    encode_program_v4_atomic(
        base_program,
        BorrowedRangePolicyV4::DisjointExactCoverage,
        u32::try_from(DEALER_SCENARIO_TRADE_HEADER_BYTES_V3)
            .map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?,
        &spans,
        &ranges,
        scratch,
        output,
    )
    .map_err(|_| DealerScenarioArtifactsErrorV4::Effect)
}

/// Hostile-decode and join the exact selector-9 Request/Transition/Effect
/// geometry. Caller-provided scalar/identity scratch receives only a minimal
/// valid P1 witness geometry for resolution checks.
pub fn authenticate_dealer_scenario_artifacts_v4<'a>(
    request_profile_bytes: &'a [u8],
    transition_bytes: &'a [u8],
    effect_bytes: &'a [u8],
    scalars: &mut [u64],
    identities: &mut [[u8; 32]],
) -> Result<
    (
        RequestProfileV3<'a>,
        TransitionProgramV3<'a>,
        EffectProgramV4<'a>,
    ),
    DealerScenarioArtifactsErrorV4,
> {
    if request_profile_bytes.len() != DEALER_SCENARIO_REQUEST_PROFILE_BYTES_V4
        || transition_bytes.len() != DEALER_SCENARIO_TRANSITION_BYTES_V4
        || scalars.len() != usize::from(DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4)
        || identities.len() != usize::from(DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4)
    {
        return Err(DealerScenarioArtifactsErrorV4::Effect);
    }
    let mut request_scratch = [0_u8; DEALER_SCENARIO_REQUEST_PROFILE_BYTES_V4];
    let mut request_canonical = [0_u8; DEALER_SCENARIO_REQUEST_PROFILE_BYTES_V4];
    encode_dealer_scenario_request_profile_v4(&mut request_scratch, &mut request_canonical)?;
    let mut transition_scratch = [0_u8; DEALER_SCENARIO_TRANSITION_BYTES_V4];
    let mut transition_canonical = [0_u8; DEALER_SCENARIO_TRANSITION_BYTES_V4];
    encode_dealer_scenario_transition_v4(&mut transition_scratch, &mut transition_canonical)?;
    if request_profile_bytes != request_canonical || transition_bytes != transition_canonical {
        return Err(DealerScenarioArtifactsErrorV4::Effect);
    }
    let request = RequestProfileV3::decode(request_profile_bytes)
        .map_err(|_| DealerScenarioArtifactsErrorV4::RequestProfile)?;
    let transition = TransitionProgramV3::decode(transition_bytes)
        .map_err(|_| DealerScenarioArtifactsErrorV4::Transition)?;
    let effect = EffectProgramV4::decode(effect_bytes)
        .map_err(|_| DealerScenarioArtifactsErrorV4::Effect)?;
    validate_base_effect(effect.base().bytes())?;
    if request.request_profile().common_scalar_count() != DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4
        || request.request_profile().item_scalar_stride() != DEALER_SCENARIO_ITEM_SCALAR_STRIDE_V4
        || request.request_profile().common_identity_count()
            != DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4
        || request.request_profile().item_identity_stride()
            != DEALER_SCENARIO_ITEM_IDENTITY_STRIDE_V4
        || transition.common_scalar_count() != DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4
        || transition.item_scalar_stride() != DEALER_SCENARIO_ITEM_SCALAR_STRIDE_V4
        || transition.common_identity_count() != DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4
        || transition.item_identity_stride() != DEALER_SCENARIO_ITEM_IDENTITY_STRIDE_V4
    {
        return Err(DealerScenarioArtifactsErrorV4::Effect);
    }
    scalars.fill(0);
    identities.fill([0; 32]);
    *scalars
        .get_mut(usize::from(DEALER_SCENARIO_POSITION_COUNT_SCALAR_V4))
        .ok_or(DealerScenarioArtifactsErrorV4::Effect)? = 1;
    *scalars
        .get_mut(usize::from(DEALER_SCENARIO_WITNESS_BYTES_SCALAR_V4))
        .ok_or(DealerScenarioArtifactsErrorV4::Effect)? =
        u64::from(dealer_scenario_witness_bounds_v4()?.0);
    *scalars
        .get_mut(usize::from(DEALER_SCENARIO_WITNESS_OFFSET_SCALAR_V4))
        .ok_or(DealerScenarioArtifactsErrorV4::Effect)? =
        u64::try_from(DEALER_SCENARIO_TRADE_HEADER_BYTES_V3)
            .map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?;
    effect
        .validate_request_coverage(
            DEALER_SCENARIO_TRADE_HEADER_BYTES_V3
                .checked_add(
                    usize::try_from(
                        *scalars
                            .get(usize::from(DEALER_SCENARIO_WITNESS_BYTES_SCALAR_V4))
                            .ok_or(DealerScenarioArtifactsErrorV4::Effect)?,
                    )
                    .map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?,
                )
                .ok_or(DealerScenarioArtifactsErrorV4::Arithmetic)?,
            0,
            scalars,
            identities,
        )
        .map_err(|_| DealerScenarioArtifactsErrorV4::Effect)?;
    Ok((request, transition, effect))
}

fn optional_span(
    route: u16,
    ordinal: u16,
) -> Result<DynamicFixedSpanV4, DealerScenarioArtifactsErrorV4> {
    Ok(DynamicFixedSpanV4::new(
        route,
        DEALER_SCENARIO_ROUTE_SPAN_SCALAR_BASE_V4
            .checked_add(ordinal)
            .ok_or(DealerScenarioArtifactsErrorV4::Arithmetic)?,
        0,
        (1_u64 << 0) | (1_u64 << DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3),
    ))
}

fn validate_base_effect(base_program: &[u8]) -> Result<(), DealerScenarioArtifactsErrorV4> {
    let base = EffectProgramV3::decode(base_program)
        .map_err(|_| DealerScenarioArtifactsErrorV4::Effect)?;
    if base.route_count() != DEALER_SCENARIO_ROUTE_COUNT_V4
        || base.fixed_account_count() != DEALER_SCENARIO_BASE_FIXED_ACCOUNTS_V4
        || base.item_account_stride() != 0
        || base.common_scalar_count() != DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4
        || base.item_scalar_stride() != DEALER_SCENARIO_ITEM_SCALAR_STRIDE_V4
        || base.common_identity_count() != DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4
        || base.item_identity_stride() != DEALER_SCENARIO_ITEM_IDENTITY_STRIDE_V4
    {
        return Err(DealerScenarioArtifactsErrorV4::Effect);
    }
    for route_index in 0..DEALER_SCENARIO_ROUTE_COUNT_V4 {
        let route = base
            .route(route_index)
            .map_err(|_| DealerScenarioArtifactsErrorV4::Effect)?;
        if route.kind() != RouteKindV3::Once
            || route.item_account_count() != 0
            || route.item_request_bytes() != 0
            || route.borrows_witness()
        {
            return Err(DealerScenarioArtifactsErrorV4::Effect);
        }
        if route_index == DEALER_SCENARIO_CLAIMS_ROUTE_V4 {
            if route.role() != FixedRole::Claims
                || route.fixed_account_start() != DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3
                || route.fixed_account_count() != DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3
                || route.fixed_request_bytes() != 0
            {
                return Err(DealerScenarioArtifactsErrorV4::Effect);
            }
        } else if route.role() != FixedRole::Custody || route.fixed_account_count() != 0 {
            return Err(DealerScenarioArtifactsErrorV4::Effect);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_effect_kernel::v3::{
        HEADER_BYTES as EFFECT_HEADER_BYTES, RECEIPT_DEPENDENCY_BYTES, ROUTE_BYTES,
        RouteReceiptDependencyV3,
        encode::{EffectGeometryV3, RouteInputV3, encode_effect_program_v3_atomic},
    };
    use std::vec;
    use std::vec::Vec;

    fn base_effect() -> Vec<u8> {
        let template = [0_u8; 1];
        let routes = [
            optional_route(0, &template, None),
            optional_route(1, &template, None),
            optional_route(2, &template, None),
            optional_route(3, &template, None),
            RouteInputV3 {
                role: FixedRole::Claims,
                kind: RouteKindV3::Once,
                enable_common_scalar: None,
                witness_range_common_scalar: None,
                receipt_dependency: None,
                fixed_account_start: DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3,
                fixed_account_count: DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3,
                item_account_start: 0,
                item_account_count: 0,
                fixed_request: &[],
                item_request: &[],
            },
            optional_route(
                5,
                &template,
                Some(RouteReceiptDependencyV3::new(
                    FixedRole::Claims,
                    DEALER_SCENARIO_CLAIMS_ROUTE_V4,
                    u16::try_from(SIGNED_DELTA_RECEIPT_BYTES_V3).expect("receipt"),
                )),
            ),
            optional_route(
                6,
                &template,
                Some(RouteReceiptDependencyV3::new(
                    FixedRole::Claims,
                    DEALER_SCENARIO_CLAIMS_ROUTE_V4,
                    u16::try_from(SIGNED_DELTA_RECEIPT_BYTES_V3).expect("receipt"),
                )),
            ),
        ];
        let bytes = EFFECT_HEADER_BYTES
            + routes.len() * ROUTE_BYTES
            + 2 * RECEIPT_DEPENDENCY_BYTES
            + 6 * template.len();
        let mut scratch = vec![0; bytes];
        let mut output = vec![0; bytes];
        encode_effect_program_v3_atomic(
            EffectGeometryV3 {
                fixed_accounts: DEALER_SCENARIO_BASE_FIXED_ACCOUNTS_V4,
                item_account_stride: 0,
                common_scalars: DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4,
                item_scalar_stride: DEALER_SCENARIO_ITEM_SCALAR_STRIDE_V4,
                common_identities: DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4,
                item_identity_stride: DEALER_SCENARIO_ITEM_IDENTITY_STRIDE_V4,
            },
            &routes,
            &[],
            &[],
            &mut scratch,
            &mut output,
        )
        .expect("base effect");
        output
    }

    fn optional_route<'a>(
        route: u16,
        template: &'a [u8],
        receipt_dependency: Option<RouteReceiptDependencyV3>,
    ) -> RouteInputV3<'a> {
        let ordinal = if route < DEALER_SCENARIO_CLAIMS_ROUTE_V4 {
            route
        } else {
            route - 1
        };
        RouteInputV3 {
            role: FixedRole::Custody,
            kind: RouteKindV3::Once,
            enable_common_scalar: Some(DEALER_SCENARIO_ROUTE_SPAN_SCALAR_BASE_V4 + ordinal),
            witness_range_common_scalar: None,
            receipt_dependency,
            fixed_account_start: if route < DEALER_SCENARIO_CLAIMS_ROUTE_V4 {
                DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3
            } else {
                DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3 + DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3
            },
            fixed_account_count: 0,
            item_account_start: 0,
            item_account_count: 0,
            fixed_request: template,
            item_request: &[],
        }
    }

    #[test]
    fn one_selector_resolves_p1_p2_and_optional_custody_frames() {
        let base = base_effect();
        let bytes = dealer_scenario_effect_program_bytes_v4(base.len()).expect("width");
        let mut scratch = vec![0; bytes];
        let mut output = vec![0; bytes];
        encode_dealer_scenario_effect_program_v4(&base, &mut scratch, &mut output)
            .expect("effect v4");
        let program = EffectProgramV4::decode(&output).expect("decode");
        for positions in [1_u64, 2] {
            let witness = u64::from(dealer_scenario_witness_bounds_v4().expect("bounds").0);
            let mut scalars = vec![0; usize::from(DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4)];
            let identities = vec![[1; 32]; usize::from(DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4)];
            scalars[usize::from(DEALER_SCENARIO_POSITION_COUNT_SCALAR_V4)] = positions;
            scalars[usize::from(DEALER_SCENARIO_WITNESS_BYTES_SCALAR_V4)] = witness;
            scalars[usize::from(DEALER_SCENARIO_ROUTE_SPAN_SCALAR_BASE_V4)] = 14;
            scalars[usize::from(DEALER_SCENARIO_ROUTE_SPAN_SCALAR_BASE_V4 + 4)] = 14;
            let expected = usize::from(DEALER_SCENARIO_BASE_FIXED_ACCOUNTS_V4)
                + usize::try_from(positions).expect("small")
                + 28;
            assert_eq!(program.account_count(0, &scalars), Ok(expected));
            let claims = program
                .resolved_invocation(DEALER_SCENARIO_CLAIMS_ROUTE_V4, 0, 0, &scalars, &identities)
                .expect("Claims invocation");
            assert_eq!(claims.invocation.fixed_account_start, 19);
            assert_eq!(
                claims.invocation.fixed_account_count,
                DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3
                    + u16::try_from(positions).expect("small")
            );
            assert_eq!(
                program.validate_request_coverage(
                    DEALER_SCENARIO_TRADE_HEADER_BYTES_V3
                        + usize::try_from(witness).expect("bounded"),
                    0,
                    &scalars,
                    &identities,
                ),
                Ok(())
            );
        }
    }

    #[test]
    fn request_transition_and_effect_join_exactly() {
        let mut request_scratch = [0; DEALER_SCENARIO_REQUEST_PROFILE_BYTES_V4];
        let mut request = [0; DEALER_SCENARIO_REQUEST_PROFILE_BYTES_V4];
        encode_dealer_scenario_request_profile_v4(&mut request_scratch, &mut request)
            .expect("request profile");
        let decoded = RequestProfileV3::decode(&request).expect("decode request");
        assert_eq!(
            decoded.witness_policy().consumer_role,
            BorrowedWitnessRoleV3::Claims
        );
        assert_eq!(decoded.request_profile().fixed_request_bytes(), 384);

        let mut transition_scratch = [0; DEALER_SCENARIO_TRANSITION_BYTES_V4];
        let mut transition = [0; DEALER_SCENARIO_TRANSITION_BYTES_V4];
        encode_dealer_scenario_transition_v4(&mut transition_scratch, &mut transition)
            .expect("transition");

        let base = base_effect();
        let effect_bytes = dealer_scenario_effect_program_bytes_v4(base.len()).expect("width");
        let mut effect_scratch = vec![0; effect_bytes];
        let mut effect = vec![0; effect_bytes];
        encode_dealer_scenario_effect_program_v4(&base, &mut effect_scratch, &mut effect)
            .expect("effect");
        let mut scalars = vec![0; usize::from(DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4)];
        let mut identities = vec![[0; 32]; usize::from(DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4)];
        authenticate_dealer_scenario_artifacts_v4(
            &request,
            &transition,
            &effect,
            &mut scalars,
            &mut identities,
        )
        .expect("joined artifacts");

        *effect.get_mut(20).expect("reserved Effect byte") ^= 1;
        assert!(
            authenticate_dealer_scenario_artifacts_v4(
                &request,
                &transition,
                &effect,
                &mut scalars,
                &mut identities,
            )
            .is_err()
        );
    }
}
