//! Finalized artifact construction and cross-artifact joins for Dealer equity.
//!
//! The exact 480-byte Dealer header remains the family request prefix. A P1 or
//! P2 physical shape admits one nonempty canonical SignedDeltaV3 packet as the
//! sole borrowed suffix; P0 admits no suffix. The generic Transition program
//! enforces only the chain-derived current-slot/request-expiry relation and
//! witness geometry. Scenario-basket equity remains owned by the admitted
//! semantic executor and is never re-encoded in these artifacts.

use dclutch_claims_svm::signed_delta_v3::{
    SIGNED_DELTA_PLAN_MAGIC_V3, SIGNED_DELTA_RECEIPT_BYTES_V3, SIGNED_DELTA_RECEIPT_MAGIC_V3,
    plan_bytes as signed_delta_plan_bytes,
};
use dclutch_custody_contract::{CUSTODY_REQUEST_BYTES_V1, DELEGATED_CUSTODY_REQUEST_BYTES_V2};
use dclutch_dealer_codec::MAX_OUTCOMES;
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{ProgramV3 as EffectProgramV3, RouteKindV3},
};
use dclutch_request_profile_contract::{
    RequestProfileV1,
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
    v3_equity_operator::{
        DEALER_EQUITY_CLAIMS_PACKET_BYTES_OFFSET_V3, DEALER_EQUITY_HEADER_BYTES_V3,
        DEALER_EQUITY_REQUEST_MAGIC_V3, DEALER_EQUITY_REQUEST_VERSION_V3,
        DEALER_EQUITY_SELECTOR_OFFSET_V3, EquityRequestActionV3, dealer_equity_selector_v3,
    },
    v3_hot_artifact::{
        DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3, DEALER_EQUITY_LOCAL_ACCOUNT_COUNT_V3,
        DEALER_EQUITY_WITNESS_BYTES_SCALAR_V3, DEALER_EQUITY_WITNESS_OFFSET_SCALAR_V3,
        DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3, DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3,
        dealer_current_slot_scalar_register_v3, dealer_equity_identity_count_v3,
        dealer_equity_scalar_count_v3, dealer_expiry_scalar_register_v3,
    },
    v3_multi_lp::MultiLpActionV3,
};

const FIXED_PROFILE_OPERATIONS_V3: usize = 6;
const P0_PROFILE_OPERATIONS_V3: usize = 7;
const REQUEST_PROFILE_OPERATION_BYTES_V3: usize = dclutch_request_profile_contract::OPERATION_BYTES;
const REQUEST_PROFILE_HEADER_BYTES_V3: usize = dclutch_request_profile_contract::HEADER_BYTES;
const DEALER_EQUITY_V1_PROFILE_BYTES_V3: usize = REQUEST_PROFILE_HEADER_BYTES_V3
    + FIXED_PROFILE_OPERATIONS_V3 * REQUEST_PROFILE_OPERATION_BYTES_V3;
const DEALER_EQUITY_P0_PROFILE_BYTES_V3: usize =
    REQUEST_PROFILE_HEADER_BYTES_V3 + P0_PROFILE_OPERATIONS_V3 * REQUEST_PROFILE_OPERATION_BYTES_V3;
const DEALER_EQUITY_V3_PROFILE_BYTES_V3: usize =
    REQUEST_PROFILE_V3_HEADER_BYTES + DEALER_EQUITY_V1_PROFILE_BYTES_V3;
const DEALER_EQUITY_TRANSITION_BASE_OPERATIONS_V3: usize = 2;
const DEALER_EQUITY_TRANSITION_WITNESS_OPERATIONS_V3: usize = 3;

/// Largest exact RequestProfile artifact emitted by this family.
pub const DEALER_EQUITY_REQUEST_PROFILE_MAX_BYTES_V3: usize = DEALER_EQUITY_V3_PROFILE_BYTES_V3;
/// Largest exact TransitionVM artifact emitted by this family.
pub const DEALER_EQUITY_TRANSITION_MAX_BYTES_V3: usize = dclutch_transition_vm::v3::HEADER_BYTES
    + DEALER_EQUITY_TRANSITION_WITNESS_OPERATIONS_V3 * dclutch_transition_vm::v3::INSTRUCTION_BYTES;

/// Stable construction or cross-artifact refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerEquityArtifactsErrorV3 {
    /// Action or SignedDelta Position-table width was outside P0/P1/P2.
    Shape,
    /// RequestProfile bytes or exact prefix/witness policy differed.
    RequestProfile,
    /// Transition bytes or register geometry differed.
    Transition,
    /// EffectProgram bytes or route geometry differed.
    Effect,
    /// Register/account geometry did not join across selected artifacts.
    Geometry,
    /// The Claims route was not the sole exact borrowed-suffix consumer.
    BorrowedWitness,
    /// Checked width arithmetic overflowed.
    Arithmetic,
}

/// Exact request interpreter selected by one P0/P1/P2 physical shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerEquityRequestProfileArtifactV3<'a> {
    /// P0 uses an exact V1 profile and admits no suffix.
    Exact(RequestProfileV1<'a>),
    /// P1/P2 use V3 and lend the complete suffix once to Claims.
    Borrowed(RequestProfileV3<'a>),
}

impl<'a> DealerEquityRequestProfileArtifactV3<'a> {
    /// Embedded V1 prefix projector used for cross-artifact geometry.
    pub const fn request_profile(self) -> RequestProfileV1<'a> {
        match self {
            Self::Exact(profile) => profile,
            Self::Borrowed(profile) => profile.request_profile(),
        }
    }
}

/// Fully hostile-decoded Dealer equity artifact geometry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerEquityArtifactBundleV3<'a> {
    /// Exact P0/P1/P2 RequestProfile.
    pub request_profile: DealerEquityRequestProfileArtifactV3<'a>,
    /// Exact trusted-slot/expiry TransitionVM program.
    pub transition: TransitionProgramV3<'a>,
    /// Exact Custody/Claims/local-state EffectProgram.
    pub effect: EffectProgramV3<'a>,
}

/// Exact RequestProfile artifact width for one physical shape.
pub const fn dealer_equity_request_profile_bytes_v3(
    signed_position_count: u32,
) -> Result<usize, DealerEquityArtifactsErrorV3> {
    match signed_position_count {
        0 => Ok(DEALER_EQUITY_P0_PROFILE_BYTES_V3),
        1 | 2 => Ok(DEALER_EQUITY_V3_PROFILE_BYTES_V3),
        _ => Err(DealerEquityArtifactsErrorV3::Shape),
    }
}

/// Exact TransitionVM artifact width for one physical shape.
pub const fn dealer_equity_transition_bytes_v3(
    signed_position_count: u32,
) -> Result<usize, DealerEquityArtifactsErrorV3> {
    let operations = match signed_position_count {
        0 => DEALER_EQUITY_TRANSITION_BASE_OPERATIONS_V3,
        1 | 2 => DEALER_EQUITY_TRANSITION_WITNESS_OPERATIONS_V3,
        _ => return Err(DealerEquityArtifactsErrorV3::Shape),
    };
    Ok(dclutch_transition_vm::v3::HEADER_BYTES
        + operations * dclutch_transition_vm::v3::INSTRUCTION_BYTES)
}

/// Inclusive canonical SignedDelta packet bounds for a P1/P2 shape.
pub fn dealer_equity_witness_bounds_v3(
    signed_position_count: u32,
) -> Result<(u32, u32), DealerEquityArtifactsErrorV3> {
    if !(1..=2).contains(&signed_position_count) {
        return Err(DealerEquityArtifactsErrorV3::Shape);
    }
    let maximum_outcomes =
        u32::try_from(MAX_OUTCOMES).map_err(|_| DealerEquityArtifactsErrorV3::Arithmetic)?;
    let maximum_deltas = signed_position_count
        .checked_mul(maximum_outcomes)
        .ok_or(DealerEquityArtifactsErrorV3::Arithmetic)?;
    let minimum = signed_delta_plan_bytes(1, signed_position_count, signed_position_count)
        .map_err(|_| DealerEquityArtifactsErrorV3::Arithmetic)?;
    let maximum = signed_delta_plan_bytes(maximum_outcomes, signed_position_count, maximum_deltas)
        .map_err(|_| DealerEquityArtifactsErrorV3::Arithmetic)?;
    Ok((
        u32::try_from(minimum).map_err(|_| DealerEquityArtifactsErrorV3::Arithmetic)?,
        u32::try_from(maximum).map_err(|_| DealerEquityArtifactsErrorV3::Arithmetic)?,
    ))
}

/// Emit the exact action/P-specific RequestProfile atomically.
pub fn encode_dealer_equity_request_profile_v3(
    action: MultiLpActionV3,
    signed_position_count: u32,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DealerEquityArtifactsErrorV3> {
    let expected = dealer_equity_request_profile_bytes_v3(signed_position_count)?;
    if scratch.len() != expected || output.len() != expected {
        return Err(DealerEquityArtifactsErrorV3::Geometry);
    }
    let request_action = equity_request_action(action);
    let selector = dealer_equity_selector_v3(request_action, signed_position_count)
        .map_err(|_| DealerEquityArtifactsErrorV3::Shape)?;
    let scalar_count = u16::try_from(
        dealer_equity_scalar_count_v3(action)
            .map_err(|_| DealerEquityArtifactsErrorV3::Geometry)?,
    )
    .map_err(|_| DealerEquityArtifactsErrorV3::Arithmetic)?;
    let identity_count = u16::try_from(
        dealer_equity_identity_count_v3(action)
            .map_err(|_| DealerEquityArtifactsErrorV3::Geometry)?,
    )
    .map_err(|_| DealerEquityArtifactsErrorV3::Arithmetic)?;
    let expiry =
        dealer_expiry_scalar_register_v3(action).ok_or(DealerEquityArtifactsErrorV3::Arithmetic)?;
    let shared = [
        RequestInstructionV1::require_u64(
            RequestCoordinateV1::fixed(0),
            u64::from_le_bytes(DEALER_EQUITY_REQUEST_MAGIC_V3),
        ),
        RequestInstructionV1::require_u16(
            RequestCoordinateV1::fixed(8),
            DEALER_EQUITY_REQUEST_VERSION_V3,
        ),
        RequestInstructionV1::require_u16(
            RequestCoordinateV1::fixed(DEALER_EQUITY_SELECTOR_OFFSET_V3),
            selector,
        ),
        RequestInstructionV1::project_u64(
            RequestCoordinateV1::fixed(440),
            ScalarRegisterV1::common(expiry),
        ),
        RequestInstructionV1::project_u32(
            RequestCoordinateV1::fixed(
                u32::try_from(DEALER_EQUITY_CLAIMS_PACKET_BYTES_OFFSET_V3)
                    .map_err(|_| DealerEquityArtifactsErrorV3::Arithmetic)?,
            ),
            ScalarRegisterV1::common(DEALER_EQUITY_WITNESS_BYTES_SCALAR_V3),
        ),
        RequestInstructionV1::require_zero(RequestCoordinateV1::fixed(476), 4),
    ];
    let mut embedded_scratch = [0_u8; DEALER_EQUITY_P0_PROFILE_BYTES_V3];
    let mut embedded_output = [0_u8; DEALER_EQUITY_P0_PROFILE_BYTES_V3];
    let geometry = RequestGeometryV1::new(
        u32::try_from(DEALER_EQUITY_HEADER_BYTES_V3)
            .map_err(|_| DealerEquityArtifactsErrorV3::Arithmetic)?,
        0,
        scalar_count,
        0,
        identity_count,
        0,
    );
    let embedded_bytes = if signed_position_count == 0 {
        let instructions = [
            shared[0],
            shared[1],
            shared[2],
            RequestInstructionV1::require_u32(
                RequestCoordinateV1::fixed(
                    u32::try_from(DEALER_EQUITY_CLAIMS_PACKET_BYTES_OFFSET_V3)
                        .map_err(|_| DealerEquityArtifactsErrorV3::Arithmetic)?,
                ),
                0,
            ),
            shared[3],
            shared[4],
            shared[5],
        ];
        encode_request_profile_v1_atomic(
            geometry,
            &instructions,
            &[],
            &mut embedded_scratch[..DEALER_EQUITY_P0_PROFILE_BYTES_V3],
            &mut embedded_output[..DEALER_EQUITY_P0_PROFILE_BYTES_V3],
        )
        .map_err(|_| DealerEquityArtifactsErrorV3::RequestProfile)?;
        DEALER_EQUITY_P0_PROFILE_BYTES_V3
    } else {
        encode_request_profile_v1_atomic(
            geometry,
            &shared,
            &[],
            &mut embedded_scratch[..DEALER_EQUITY_V1_PROFILE_BYTES_V3],
            &mut embedded_output[..DEALER_EQUITY_V1_PROFILE_BYTES_V3],
        )
        .map_err(|_| DealerEquityArtifactsErrorV3::RequestProfile)?;
        DEALER_EQUITY_V1_PROFILE_BYTES_V3
    };
    let embedded = embedded_output
        .get(..embedded_bytes)
        .ok_or(DealerEquityArtifactsErrorV3::Geometry)?;
    if signed_position_count == 0 {
        scratch.copy_from_slice(embedded);
        RequestProfileV1::decode(scratch)
            .map_err(|_| DealerEquityArtifactsErrorV3::RequestProfile)?;
        output.copy_from_slice(scratch);
        return Ok(());
    }
    let (minimum_bytes, maximum_bytes) = dealer_equity_witness_bounds_v3(signed_position_count)?;
    encode_request_profile_v3_atomic(
        embedded,
        BorrowedWitnessPolicyV3 {
            minimum_bytes,
            maximum_bytes,
            consumer_role: BorrowedWitnessRoleV3::Claims,
            child_request_magic: SIGNED_DELTA_PLAN_MAGIC_V3,
            child_receipt_magic: SIGNED_DELTA_RECEIPT_MAGIC_V3,
            child_receipt_bytes: u32::try_from(SIGNED_DELTA_RECEIPT_BYTES_V3)
                .map_err(|_| DealerEquityArtifactsErrorV3::Arithmetic)?,
        },
        scratch,
        output,
    )
    .map_err(|_| DealerEquityArtifactsErrorV3::RequestProfile)
}

/// Emit the exact trusted-current-slot/expiry TransitionVM atomically.
pub fn encode_dealer_equity_transition_v3(
    action: MultiLpActionV3,
    signed_position_count: u32,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DealerEquityArtifactsErrorV3> {
    let expected = dealer_equity_transition_bytes_v3(signed_position_count)?;
    if scratch.len() != expected || output.len() != expected {
        return Err(DealerEquityArtifactsErrorV3::Geometry);
    }
    let current = dealer_current_slot_scalar_register_v3(action)
        .ok_or(DealerEquityArtifactsErrorV3::Arithmetic)?;
    let expiry =
        dealer_expiry_scalar_register_v3(action).ok_or(DealerEquityArtifactsErrorV3::Arithmetic)?;
    let base = [
        InstructionV3::load_const(
            ScalarRegisterV3::common(DEALER_EQUITY_WITNESS_OFFSET_SCALAR_V3),
            u64::try_from(DEALER_EQUITY_HEADER_BYTES_V3)
                .map_err(|_| DealerEquityArtifactsErrorV3::Arithmetic)?,
        ),
        InstructionV3::scalar_le(
            ScalarRegisterV3::common(current),
            ScalarRegisterV3::common(expiry),
        ),
    ];
    let witness = [InstructionV3::nonzero(ScalarRegisterV3::common(
        DEALER_EQUITY_WITNESS_BYTES_SCALAR_V3,
    ))];
    let epilogue = if signed_position_count == 0 {
        &[][..]
    } else {
        &witness[..]
    };
    encode_program_atomic(
        ProgramGeometryV3 {
            common_scalars: u16::try_from(
                dealer_equity_scalar_count_v3(action)
                    .map_err(|_| DealerEquityArtifactsErrorV3::Geometry)?,
            )
            .map_err(|_| DealerEquityArtifactsErrorV3::Arithmetic)?,
            item_scalar_stride: 0,
            common_identities: u16::try_from(
                dealer_equity_identity_count_v3(action)
                    .map_err(|_| DealerEquityArtifactsErrorV3::Geometry)?,
            )
            .map_err(|_| DealerEquityArtifactsErrorV3::Arithmetic)?,
            item_identity_stride: 0,
        },
        &base,
        &[],
        epilogue,
        scratch,
        output,
    )
    .map_err(|_| DealerEquityArtifactsErrorV3::Transition)
}

/// Hostile-decode and require the exact generated request artifact.
pub fn decode_dealer_equity_request_profile_v3<'a>(
    action: MultiLpActionV3,
    signed_position_count: u32,
    bytes: &'a [u8],
) -> Result<DealerEquityRequestProfileArtifactV3<'a>, DealerEquityArtifactsErrorV3> {
    let expected = dealer_equity_request_profile_bytes_v3(signed_position_count)?;
    if bytes.len() != expected {
        return Err(DealerEquityArtifactsErrorV3::RequestProfile);
    }
    let mut scratch = [0_u8; DEALER_EQUITY_REQUEST_PROFILE_MAX_BYTES_V3];
    let mut canonical = [0_u8; DEALER_EQUITY_REQUEST_PROFILE_MAX_BYTES_V3];
    let scratch = scratch
        .get_mut(..expected)
        .ok_or(DealerEquityArtifactsErrorV3::RequestProfile)?;
    let canonical = canonical
        .get_mut(..expected)
        .ok_or(DealerEquityArtifactsErrorV3::RequestProfile)?;
    encode_dealer_equity_request_profile_v3(action, signed_position_count, scratch, canonical)?;
    if bytes != canonical {
        return Err(DealerEquityArtifactsErrorV3::RequestProfile);
    }
    if signed_position_count == 0 {
        RequestProfileV1::decode(bytes)
            .map(DealerEquityRequestProfileArtifactV3::Exact)
            .map_err(|_| DealerEquityArtifactsErrorV3::RequestProfile)
    } else {
        RequestProfileV3::decode(bytes)
            .map(DealerEquityRequestProfileArtifactV3::Borrowed)
            .map_err(|_| DealerEquityArtifactsErrorV3::RequestProfile)
    }
}

/// Hostile-decode and require the exact generated transition artifact.
pub fn decode_dealer_equity_transition_v3<'a>(
    action: MultiLpActionV3,
    signed_position_count: u32,
    bytes: &'a [u8],
) -> Result<TransitionProgramV3<'a>, DealerEquityArtifactsErrorV3> {
    let expected = dealer_equity_transition_bytes_v3(signed_position_count)?;
    if bytes.len() != expected {
        return Err(DealerEquityArtifactsErrorV3::Transition);
    }
    let mut scratch = [0_u8; DEALER_EQUITY_TRANSITION_MAX_BYTES_V3];
    let mut canonical = [0_u8; DEALER_EQUITY_TRANSITION_MAX_BYTES_V3];
    let scratch = scratch
        .get_mut(..expected)
        .ok_or(DealerEquityArtifactsErrorV3::Transition)?;
    let canonical = canonical
        .get_mut(..expected)
        .ok_or(DealerEquityArtifactsErrorV3::Transition)?;
    encode_dealer_equity_transition_v3(action, signed_position_count, scratch, canonical)?;
    if bytes != canonical {
        return Err(DealerEquityArtifactsErrorV3::Transition);
    }
    TransitionProgramV3::decode(bytes).map_err(|_| DealerEquityArtifactsErrorV3::Transition)
}

/// Join exact Dealer RequestProfile/Transition geometry to the selected Effect.
///
/// Caller-owned scratch is used only to resolve the authenticated Effect route
/// coordinates; no state or artifact bytes are mutated.
pub fn authenticate_dealer_equity_artifacts_v3<'a>(
    action: MultiLpActionV3,
    signed_position_count: u32,
    request_profile_bytes: &'a [u8],
    transition_bytes: &'a [u8],
    effect_bytes: &'a [u8],
    scratch_scalars: &mut [u64],
    scratch_identities: &mut [[u8; 32]],
) -> Result<DealerEquityArtifactBundleV3<'a>, DealerEquityArtifactsErrorV3> {
    let request_profile = decode_dealer_equity_request_profile_v3(
        action,
        signed_position_count,
        request_profile_bytes,
    )?;
    let transition =
        decode_dealer_equity_transition_v3(action, signed_position_count, transition_bytes)?;
    let effect =
        EffectProgramV3::decode(effect_bytes).map_err(|_| DealerEquityArtifactsErrorV3::Effect)?;
    let scalars = dealer_equity_scalar_count_v3(action)
        .map_err(|_| DealerEquityArtifactsErrorV3::Geometry)?;
    let identities = dealer_equity_identity_count_v3(action)
        .map_err(|_| DealerEquityArtifactsErrorV3::Geometry)?;
    let profile = request_profile.request_profile();
    if scratch_scalars.len() != scalars
        || scratch_identities.len() != identities
        || usize::from(profile.common_scalar_count()) != scalars
        || profile.item_scalar_stride() != 0
        || usize::from(profile.common_identity_count()) != identities
        || profile.item_identity_stride() != 0
        || usize::from(transition.common_scalar_count()) != scalars
        || transition.item_scalar_stride() != 0
        || usize::from(transition.common_identity_count()) != identities
        || transition.item_identity_stride() != 0
        || usize::from(effect.common_scalar_count()) != scalars
        || effect.item_scalar_stride() != 0
        || usize::from(effect.common_identity_count()) != identities
        || effect.item_identity_stride() != 0
    {
        return Err(DealerEquityArtifactsErrorV3::Geometry);
    }
    let custody_routes = match action {
        MultiLpActionV3::Add => 2_u16,
        MultiLpActionV3::Remove => 3_u16,
    };
    if effect.route_count() != custody_routes + 1 {
        return Err(DealerEquityArtifactsErrorV3::Effect);
    }
    let claims_accounts = DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3
        .checked_add(
            u16::try_from(signed_position_count)
                .map_err(|_| DealerEquityArtifactsErrorV3::Arithmetic)?,
        )
        .ok_or(DealerEquityArtifactsErrorV3::Arithmetic)?;
    let expected_fixed_accounts = DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3
        .checked_add(
            custody_routes
                .checked_mul(DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3)
                .ok_or(DealerEquityArtifactsErrorV3::Arithmetic)?,
        )
        .and_then(|value| value.checked_add(claims_accounts))
        .and_then(|value| value.checked_add(DEALER_EQUITY_LOCAL_ACCOUNT_COUNT_V3))
        .ok_or(DealerEquityArtifactsErrorV3::Arithmetic)?;
    if effect.fixed_account_count() != expected_fixed_accounts || effect.item_account_stride() != 0
    {
        return Err(DealerEquityArtifactsErrorV3::Geometry);
    }
    let mut account_start = DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3;
    let mut route_index = 0_u16;
    while route_index < effect.route_count() {
        let route = effect
            .route(route_index)
            .map_err(|_| DealerEquityArtifactsErrorV3::Effect)?;
        if route.kind() != RouteKindV3::Once
            || route.fixed_account_start() != account_start
            || route.item_account_start() != 0
            || route.item_account_count() != 0
            || route.item_request_bytes() != 0
        {
            return Err(DealerEquityArtifactsErrorV3::Geometry);
        }
        if route_index == 1 {
            if route.role() != FixedRole::Claims
                || !route.borrows_witness()
                || route.fixed_account_count() != claims_accounts
                || route.fixed_request_bytes() != 0
                || route.receipt_dependency_count() != 0
            {
                return Err(DealerEquityArtifactsErrorV3::BorrowedWitness);
            }
            account_start = account_start
                .checked_add(claims_accounts)
                .ok_or(DealerEquityArtifactsErrorV3::Arithmetic)?;
        } else {
            let expected_request = if action == MultiLpActionV3::Add && route_index == 0 {
                u32::try_from(DELEGATED_CUSTODY_REQUEST_BYTES_V2)
                    .map_err(|_| DealerEquityArtifactsErrorV3::Arithmetic)?
            } else {
                u32::try_from(CUSTODY_REQUEST_BYTES_V1)
                    .map_err(|_| DealerEquityArtifactsErrorV3::Arithmetic)?
            };
            let expected_dependencies = u16::from(signed_position_count != 0 && route_index > 1);
            if route.role() != FixedRole::Custody
                || route.borrows_witness()
                || route.fixed_account_count() != DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3
                || route.fixed_request_bytes() != expected_request
                || route.receipt_dependency_count() != expected_dependencies
            {
                return Err(DealerEquityArtifactsErrorV3::Effect);
            }
            if expected_dependencies != 0 {
                let dependency = effect
                    .route_receipt_dependency(route_index, 0)
                    .map_err(|_| DealerEquityArtifactsErrorV3::Effect)?;
                if dependency.producer_role() != FixedRole::Claims
                    || dependency.producer_route() != 1
                    || usize::from(dependency.expected_receipt_bytes())
                        != SIGNED_DELTA_RECEIPT_BYTES_V3
                {
                    return Err(DealerEquityArtifactsErrorV3::Effect);
                }
            }
            account_start = account_start
                .checked_add(DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3)
                .ok_or(DealerEquityArtifactsErrorV3::Arithmetic)?;
        }
        route_index = route_index
            .checked_add(1)
            .ok_or(DealerEquityArtifactsErrorV3::Arithmetic)?;
    }
    scratch_scalars.fill(0);
    scratch_identities.fill([0; 32]);
    let witness_bytes = if signed_position_count == 0 {
        1_u64
    } else {
        u64::from(dealer_equity_witness_bounds_v3(signed_position_count)?.0)
    };
    *scratch_scalars
        .get_mut(usize::from(DEALER_EQUITY_WITNESS_OFFSET_SCALAR_V3))
        .ok_or(DealerEquityArtifactsErrorV3::Geometry)? =
        u64::try_from(DEALER_EQUITY_HEADER_BYTES_V3)
            .map_err(|_| DealerEquityArtifactsErrorV3::Arithmetic)?;
    *scratch_scalars
        .get_mut(usize::from(DEALER_EQUITY_WITNESS_BYTES_SCALAR_V3))
        .ok_or(DealerEquityArtifactsErrorV3::Geometry)? = witness_bytes;
    let invocation = effect
        .resolved_invocation(1, 0, 0, scratch_scalars, scratch_identities)
        .map_err(|_| DealerEquityArtifactsErrorV3::BorrowedWitness)?;
    let witness = invocation
        .borrowed_witness
        .ok_or(DealerEquityArtifactsErrorV3::BorrowedWitness)?;
    if witness.source_offset() != DEALER_EQUITY_HEADER_BYTES_V3
        || u64::try_from(witness.len()).ok() != Some(witness_bytes)
    {
        return Err(DealerEquityArtifactsErrorV3::BorrowedWitness);
    }
    Ok(DealerEquityArtifactBundleV3 {
        request_profile,
        transition,
        effect,
    })
}

const fn equity_request_action(action: MultiLpActionV3) -> EquityRequestActionV3 {
    match action {
        MultiLpActionV3::Add => EquityRequestActionV3::Contribute,
        MultiLpActionV3::Remove => EquityRequestActionV3::Redeem,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_request_profile_contract::{ProjectionRegistersV1, project_atomic};
    use dclutch_transition_vm::v3::{RegisterInput, RegisterOutput, execute_fold_atomic};
    use std::vec;

    #[test]
    fn profiles_are_action_specific_and_admit_only_the_exact_suffix_policy() {
        for (action, positions) in [
            (MultiLpActionV3::Add, 0_u32),
            (MultiLpActionV3::Add, 2),
            (MultiLpActionV3::Remove, 1),
        ] {
            let bytes = dealer_equity_request_profile_bytes_v3(positions).expect("width");
            let mut scratch = [0_u8; DEALER_EQUITY_REQUEST_PROFILE_MAX_BYTES_V3];
            let mut output = [0_u8; DEALER_EQUITY_REQUEST_PROFILE_MAX_BYTES_V3];
            encode_dealer_equity_request_profile_v3(
                action,
                positions,
                &mut scratch[..bytes],
                &mut output[..bytes],
            )
            .expect("profile");
            let decoded =
                decode_dealer_equity_request_profile_v3(action, positions, &output[..bytes])
                    .expect("decode");
            let profile = decoded.request_profile();
            assert_eq!(profile.fixed_request_bytes(), 480);
            assert_eq!(profile.item_request_bytes(), 0);
            assert_eq!(
                usize::from(profile.common_scalar_count()),
                dealer_equity_scalar_count_v3(action).expect("scalars")
            );
            if let DealerEquityRequestProfileArtifactV3::Borrowed(v3) = decoded {
                let expected = dealer_equity_witness_bounds_v3(positions).expect("bounds");
                let policy = v3.witness_policy();
                assert_eq!((policy.minimum_bytes, policy.maximum_bytes), expected);
                assert_eq!(policy.consumer_role, BorrowedWitnessRoleV3::Claims);
                assert_eq!(policy.child_request_magic, SIGNED_DELTA_PLAN_MAGIC_V3);
                assert_eq!(policy.child_receipt_magic, SIGNED_DELTA_RECEIPT_MAGIC_V3);
                assert_eq!(
                    policy.child_receipt_bytes,
                    u32::try_from(SIGNED_DELTA_RECEIPT_BYTES_V3).expect("receipt")
                );
            } else {
                assert_eq!(positions, 0);
            }
            let mut hostile = output[..bytes].to_vec();
            *hostile.last_mut().expect("hostile") ^= 1;
            assert_eq!(
                decode_dealer_equity_request_profile_v3(action, positions, &hostile),
                Err(DealerEquityArtifactsErrorV3::RequestProfile)
            );
        }
    }

    #[test]
    fn transition_enforces_trusted_slot_and_exact_witness_offset() {
        let action = MultiLpActionV3::Add;
        let positions = 2;
        let bytes = dealer_equity_transition_bytes_v3(positions).expect("width");
        let mut scratch_bytes = [0_u8; DEALER_EQUITY_TRANSITION_MAX_BYTES_V3];
        let mut output_bytes = [0_u8; DEALER_EQUITY_TRANSITION_MAX_BYTES_V3];
        encode_dealer_equity_transition_v3(
            action,
            positions,
            &mut scratch_bytes[..bytes],
            &mut output_bytes[..bytes],
        )
        .expect("transition");
        let transition = TransitionProgramV3::decode(&output_bytes[..bytes]).expect("decode");
        let scalar_count = dealer_equity_scalar_count_v3(action).expect("scalars");
        let identity_count = dealer_equity_identity_count_v3(action).expect("identities");
        let mut input_scalars = vec![0_u64; scalar_count];
        let input_identities = vec![[0_u8; 32]; identity_count];
        input_scalars
            [usize::from(dealer_current_slot_scalar_register_v3(action).expect("current"))] = 42;
        input_scalars[usize::from(dealer_expiry_scalar_register_v3(action).expect("expiry"))] = 42;
        input_scalars[usize::from(DEALER_EQUITY_WITNESS_BYTES_SCALAR_V3)] = 384;
        let mut scalar_scratch = input_scalars.clone();
        let mut scalar_output = input_scalars.clone();
        let mut identity_scratch = input_identities.clone();
        let mut identity_output = input_identities.clone();
        execute_fold_atomic(
            transition,
            0,
            RegisterInput {
                scalars: &input_scalars,
                identities: &input_identities,
            },
            RegisterOutput {
                scalars: &mut scalar_scratch,
                identities: &mut identity_scratch,
            },
            RegisterOutput {
                scalars: &mut scalar_output,
                identities: &mut identity_output,
            },
        )
        .expect("admitted");
        assert_eq!(
            scalar_output[usize::from(DEALER_EQUITY_WITNESS_OFFSET_SCALAR_V3)],
            480
        );
        input_scalars
            [usize::from(dealer_current_slot_scalar_register_v3(action).expect("current"))] = 43;
        scalar_output.fill(99);
        let before = scalar_output.clone();
        assert!(
            execute_fold_atomic(
                transition,
                0,
                RegisterInput {
                    scalars: &input_scalars,
                    identities: &input_identities,
                },
                RegisterOutput {
                    scalars: &mut scalar_scratch,
                    identities: &mut identity_scratch,
                },
                RegisterOutput {
                    scalars: &mut scalar_output,
                    identities: &mut identity_output,
                },
            )
            .is_err()
        );
        assert_eq!(scalar_output, before);
    }

    #[test]
    fn prefix_projection_is_atomic_and_p0_requires_an_empty_suffix() {
        let action = MultiLpActionV3::Remove;
        let positions = 0;
        let bytes = dealer_equity_request_profile_bytes_v3(positions).expect("width");
        let mut profile_scratch = [0_u8; DEALER_EQUITY_REQUEST_PROFILE_MAX_BYTES_V3];
        let mut profile_output = [0_u8; DEALER_EQUITY_REQUEST_PROFILE_MAX_BYTES_V3];
        encode_dealer_equity_request_profile_v3(
            action,
            positions,
            &mut profile_scratch[..bytes],
            &mut profile_output[..bytes],
        )
        .expect("profile");
        let profile = RequestProfileV1::decode(&profile_output[..bytes]).expect("decode");
        let mut request = [0_u8; DEALER_EQUITY_HEADER_BYTES_V3];
        request[..8].copy_from_slice(&DEALER_EQUITY_REQUEST_MAGIC_V3);
        request[8..10].copy_from_slice(&DEALER_EQUITY_REQUEST_VERSION_V3.to_le_bytes());
        let selector =
            dealer_equity_selector_v3(EquityRequestActionV3::Redeem, 0).expect("selector");
        request[10..12].copy_from_slice(&selector.to_le_bytes());
        request[440..448].copy_from_slice(&77_u64.to_le_bytes());
        let scalar_count = dealer_equity_scalar_count_v3(action).expect("scalars");
        let identity_count = dealer_equity_identity_count_v3(action).expect("identities");
        let input_scalars = vec![0_u64; scalar_count];
        let input_identities = vec![[0_u8; 32]; identity_count];
        let mut scalar_scratch = input_scalars.clone();
        let mut scalar_output = input_scalars.clone();
        let mut identity_scratch = input_identities.clone();
        let mut identity_output = input_identities.clone();
        project_atomic(
            profile,
            0,
            &request,
            ProjectionRegistersV1 {
                input_scalars: &input_scalars,
                input_identities: &input_identities,
                scratch_scalars: &mut scalar_scratch,
                scratch_identities: &mut identity_scratch,
                output_scalars: &mut scalar_output,
                output_identities: &mut identity_output,
            },
        )
        .expect("project");
        assert_eq!(
            scalar_output[usize::from(dealer_expiry_scalar_register_v3(action).expect("expiry"))],
            77
        );
        request[DEALER_EQUITY_CLAIMS_PACKET_BYTES_OFFSET_V3
            ..DEALER_EQUITY_CLAIMS_PACKET_BYTES_OFFSET_V3 + 4]
            .copy_from_slice(&1_u32.to_le_bytes());
        scalar_output.fill(55);
        let before = scalar_output.clone();
        assert!(
            project_atomic(
                profile,
                0,
                &request,
                ProjectionRegistersV1 {
                    input_scalars: &input_scalars,
                    input_identities: &input_identities,
                    scratch_scalars: &mut scalar_scratch,
                    scratch_identities: &mut identity_scratch,
                    output_scalars: &mut scalar_output,
                    output_identities: &mut identity_output,
                },
            )
            .is_err()
        );
        assert_eq!(scalar_output, before);
    }
}
