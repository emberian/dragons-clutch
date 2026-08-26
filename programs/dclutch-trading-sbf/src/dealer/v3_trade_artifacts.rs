//! Canonical selector-9 request, transition, and Effect V4 artifact joins.
//!
//! Scenario exact-fill has one selector for both P1 and P2 SignedDelta frames.
//! The exact request projects Position count and packet width into protected
//! registers; Effect V4 selects the Claims extension from `{1,2}`. Six typed
//! possible Custody routes independently select `{0,14}` accounts, so disabled
//! physical legs cost no placeholder account. The SignedDelta packet is the
//! the sole borrowed child witness. A Product-affine semantic range owns the
//! signed `u64[N]` obligation vector between the fixed header and that witness.

extern crate alloc;

use alloc::{vec, vec::Vec};

use dclutch_claims_svm::signed_delta_v3::{
    SIGNED_DELTA_PLAN_MAGIC_V3, SIGNED_DELTA_RECEIPT_BYTES_V3, SIGNED_DELTA_RECEIPT_MAGIC_V3,
    plan_bytes as signed_delta_plan_bytes,
};
use dclutch_custody_contract::{
    CUSTODY_REQUEST_BYTES_V1, CustodyRequestLayoutV1, DELEGATED_CUSTODY_REQUEST_BYTES_V2,
    DelegatedCustodyRequestLayoutV2,
};
use dclutch_custody_contract::{CallerRoleV1, CompartmentV1, OperationV1};
use dclutch_dealer_codec::MAX_OUTCOMES;
#[cfg(not(target_os = "solana"))]
use dclutch_effect_kernel::v3::{
    HEADER_BYTES as EFFECT_HEADER_BYTES, OPERATION_BYTES as EFFECT_OPERATION_BYTES,
    RECEIPT_DEPENDENCY_BYTES, ROUTE_BYTES as EFFECT_ROUTE_BYTES, RouteReceiptDependencyV3,
    encode::{
        AccountCoordinateV3, EffectGeometryV3, EffectInstructionV3, IdentityCoordinateV3,
        RequestSpaceV3, RouteInputV3, ScalarCoordinateV3, encode_effect_program_v3_atomic,
    },
};
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{ProgramV3 as EffectProgramV3, RouteKindV3},
    v4::{
        BORROWED_RANGE_BYTES_V4, BorrowedRangePolicyV4, BorrowedRangeV4, DYNAMIC_SPAN_BYTES_V4,
        DynamicFixedSpanV4, HEADER_BYTES_V4, ProgramV4 as EffectProgramV4, RequestCoordinateV4,
        SEMANTIC_RANGE_ROUTE_V4, encode_program_v4_atomic,
    },
};
use dclutch_request_profile_contract::{
    encode::{
        IdentityRegisterV1, RequestCoordinateV1, RequestGeometryV1, RequestInstructionV1,
        ScalarRegisterV1, encode_request_profile_v1_atomic,
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
use solana_program::hash::hash;

use super::{
    v3_composer::{
        MAX_DEALER_SCENARIO_CUSTODY_EFFECTS_V3, ScenarioAtomicPlanV3, ScenarioCustodyEffectV3,
    },
    v3_multi_lp::MultiLpCustodyRequestV3,
    v3_obligation::{DEALER_OBLIGATION_HEADER_BYTES_V3, DealerObligationProjectionV3},
    v3_trade::{DealerScenarioTradeRequestV3, ScenarioTradeDirectionV3},
};
use super::{
    v3_hot_artifact::{
        DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3, DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3,
        DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3, DealerCustodyIdentityFieldV3,
        DealerCustodyScalarFieldV3,
    },
    v3_trade::{
        DEALER_SCENARIO_TRADE_ACTION_V3, DEALER_SCENARIO_TRADE_CLAIMS_PACKET_BYTES_OFFSET_V3,
        DEALER_SCENARIO_TRADE_DEALER_EVIDENCE_COUNT_OFFSET_V3,
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
pub const DEALER_SCENARIO_CUSTODY_SCALAR_STRIDE_V4: u16 = 14;
/// Request-projected trailing Dealer Position evidence width, exactly `2 - P`.
pub const DEALER_SCENARIO_DEALER_EVIDENCE_COUNT_SCALAR_V4: u16 = 97;
/// Transition scratch proving `P + dealer_evidence_count == 2`.
pub const DEALER_SCENARIO_POSITION_GEOMETRY_SUM_SCALAR_V4: u16 = 98;
/// Exact common scalar-bank width.
pub const DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4: u16 = 99;
/// Parent digest precedes six exact 19-identity Custody blocks, the trusted
/// current-Trading owner, and the request-bound obligation key.
pub const DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4: u16 = 117;
/// AccountProfile-owned current Trading identity used to authenticate the
/// sole writable obligation account independently of optional child routes.
pub const DEALER_SCENARIO_CURRENT_TRADING_IDENTITY_V4: u16 = 115;
/// Request-projected exact obligation key used to bind the sole local writer.
pub const DEALER_SCENARIO_OBLIGATION_IDENTITY_V4: u16 = 116;

const DEALER_SCENARIO_CUSTODY_IDENTITY_BASE_V4: u16 = 1;
const DEALER_SCENARIO_CUSTODY_IDENTITY_STRIDE_V4: u16 = 19;
const DEALER_SCENARIO_OBLIGATION_ACCOUNT_V4: u16 = DEALER_SCENARIO_BASE_FIXED_ACCOUNTS_V4 - 1;
const DEALER_SCENARIO_OBLIGATION_REVISION_OFFSET_V4: u32 = 16;
const DEALER_SCENARIO_CUSTODY_BASE_SCALAR_FIELDS_V4: usize = 9;
const DEALER_SCENARIO_CUSTODY_BASE_IDENTITY_FIELDS_V4: usize = 17;
const DEALER_SCENARIO_CUSTODY_TEMPLATE_BYTES_V4: usize =
    2 * DELEGATED_CUSTODY_REQUEST_BYTES_V2 + 4 * CUSTODY_REQUEST_BYTES_V1;
#[cfg(not(target_os = "solana"))]
const DEALER_SCENARIO_EFFECT_OPERATION_COUNT_V4: usize = 6
    * (DEALER_SCENARIO_CUSTODY_BASE_SCALAR_FIELDS_V4
        + DEALER_SCENARIO_CUSTODY_BASE_IDENTITY_FIELDS_V4
        + 1)
    + 2 * 7
    + 2;

const REQUEST_PROFILE_OPERATIONS_V4: usize = 10;
const REQUEST_PROFILE_V1_BYTES_V4: usize = dclutch_request_profile_contract::HEADER_BYTES
    + REQUEST_PROFILE_OPERATIONS_V4 * dclutch_request_profile_contract::OPERATION_BYTES;
/// Exact selector-9 RequestProfile V3 bytes.
pub const DEALER_SCENARIO_REQUEST_PROFILE_BYTES_V4: usize =
    REQUEST_PROFILE_V3_HEADER_BYTES + REQUEST_PROFILE_V1_BYTES_V4;
const TRANSITION_OPERATIONS_V4: usize = 8;
/// Exact selector-9 TransitionVM bytes.
pub const DEALER_SCENARIO_TRANSITION_BYTES_V4: usize = dclutch_transition_vm::v3::HEADER_BYTES
    + TRANSITION_OPERATIONS_V4 * dclutch_transition_vm::v3::INSTRUCTION_BYTES;
const DYNAMIC_SPAN_COUNT_V4: usize = DEALER_SCENARIO_CUSTODY_ROUTE_COUNT_V4 + 1;
const BORROWED_RANGE_COUNT_V4: usize = 2;

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
    /// Signed request, scenario plan, Custody bank, or candidate state diverged.
    Projection,
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
        RequestInstructionV1::project_u8(
            RequestCoordinateV1::fixed(
                u32::try_from(DEALER_SCENARIO_TRADE_DEALER_EVIDENCE_COUNT_OFFSET_V3)
                    .map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?,
            ),
            ScalarRegisterV1::common(DEALER_SCENARIO_DEALER_EVIDENCE_COUNT_SCALAR_V4),
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
        RequestInstructionV1::project_identity(
            RequestCoordinateV1::fixed(112),
            IdentityRegisterV1::common(DEALER_SCENARIO_OBLIGATION_IDENTITY_V4),
        ),
        RequestInstructionV1::require_zero(RequestCoordinateV1::fixed(379), 1),
    ];
    let item_instructions = [RequestInstructionV1::project_u64(
        RequestCoordinateV1::item(0),
        ScalarRegisterV1::item(0),
    )];
    let mut embedded_scratch = [0_u8; REQUEST_PROFILE_V1_BYTES_V4];
    let mut embedded = [0_u8; REQUEST_PROFILE_V1_BYTES_V4];
    encode_request_profile_v1_atomic(
        RequestGeometryV1::new(
            u32::try_from(DEALER_SCENARIO_TRADE_HEADER_BYTES_V3)
                .map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?,
            8,
            DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4,
            DEALER_SCENARIO_ITEM_SCALAR_STRIDE_V4,
            DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4,
            DEALER_SCENARIO_ITEM_IDENTITY_STRIDE_V4,
        ),
        &instructions,
        &item_instructions,
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
        InstructionV3::checked_add_into(
            ScalarRegisterV3::common(DEALER_SCENARIO_POSITION_COUNT_SCALAR_V4),
            ScalarRegisterV3::common(DEALER_SCENARIO_DEALER_EVIDENCE_COUNT_SCALAR_V4),
            ScalarRegisterV3::common(DEALER_SCENARIO_POSITION_GEOMETRY_SUM_SCALAR_V4),
        ),
        InstructionV3::scalar_eq(
            ScalarRegisterV3::common(DEALER_SCENARIO_POSITION_GEOMETRY_SUM_SCALAR_V4),
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

/// Exact canonical V3 program width nested beneath selector-9 Effect V4.
#[cfg(not(target_os = "solana"))]
pub fn dealer_scenario_base_effect_program_bytes_v4()
-> Result<usize, DealerScenarioArtifactsErrorV4> {
    EFFECT_HEADER_BYTES
        .checked_add(
            usize::from(DEALER_SCENARIO_ROUTE_COUNT_V4)
                .checked_mul(EFFECT_ROUTE_BYTES)
                .ok_or(DealerScenarioArtifactsErrorV4::Arithmetic)?,
        )
        .and_then(|value| value.checked_add(2 * RECEIPT_DEPENDENCY_BYTES))
        .and_then(|value| {
            value.checked_add(
                DEALER_SCENARIO_EFFECT_OPERATION_COUNT_V4.checked_mul(EFFECT_OPERATION_BYTES)?,
            )
        })
        .and_then(|value| value.checked_add(DEALER_SCENARIO_CUSTODY_TEMPLATE_BYTES_V4))
        .ok_or(DealerScenarioArtifactsErrorV4::Arithmetic)
}

/// Emit the sole selector-9 V3 Effect body from six typed Custody shapes.
///
/// The templates are used only to prove their static operation, caller role,
/// and compartment pair. Every request-owned scalar and identity byte is
/// cleared before finalization and reconstructed from the admitted register
/// bank at execution time.
#[cfg(not(target_os = "solana"))]
pub fn encode_dealer_scenario_base_effect_program_v4(
    custody_templates: &[MultiLpCustodyRequestV3],
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DealerScenarioArtifactsErrorV4> {
    if custody_templates.len() != DEALER_SCENARIO_CUSTODY_ROUTE_COUNT_V4
        || scratch.len() != dealer_scenario_base_effect_program_bytes_v4()?
        || output.len() != dealer_scenario_base_effect_program_bytes_v4()?
    {
        return Err(DealerScenarioArtifactsErrorV4::Effect);
    }
    let mut templates = Vec::with_capacity(DEALER_SCENARIO_CUSTODY_ROUTE_COUNT_V4);
    for (slot, request) in custody_templates.iter().copied().enumerate() {
        validate_scenario_custody_shape(slot, request)?;
        let mut bytes = vec![0; request.encoded_len()];
        request
            .encode_into(&mut bytes)
            .map_err(|_| DealerScenarioArtifactsErrorV4::Effect)?;
        canonicalize_custody_template(request, &mut bytes)?;
        templates.push(bytes);
    }

    let claims_receipt = RouteReceiptDependencyV3::new(
        FixedRole::Claims,
        DEALER_SCENARIO_CLAIMS_ROUTE_V4,
        u16::try_from(SIGNED_DELTA_RECEIPT_BYTES_V3)
            .map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?,
    );
    let mut routes = Vec::with_capacity(usize::from(DEALER_SCENARIO_ROUTE_COUNT_V4));
    for route in 0..DEALER_SCENARIO_ROUTE_COUNT_V4 {
        if route == DEALER_SCENARIO_CLAIMS_ROUTE_V4 {
            routes.push(RouteInputV3 {
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
            });
            continue;
        }
        let slot = scenario_custody_slot(route)?;
        routes.push(RouteInputV3 {
            role: FixedRole::Custody,
            kind: RouteKindV3::Once,
            enable_common_scalar: Some(scenario_route_span_scalar(slot)?),
            witness_range_common_scalar: None,
            receipt_dependency: (route > DEALER_SCENARIO_CLAIMS_ROUTE_V4).then_some(claims_receipt),
            fixed_account_start: if route < DEALER_SCENARIO_CLAIMS_ROUTE_V4 {
                DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3
            } else {
                DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3
                    .checked_add(DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3)
                    .ok_or(DealerScenarioArtifactsErrorV4::Arithmetic)?
            },
            fixed_account_count: 0,
            item_account_start: 0,
            item_account_count: 0,
            fixed_request: templates
                .get(slot)
                .map(Vec::as_slice)
                .ok_or(DealerScenarioArtifactsErrorV4::Effect)?,
            item_request: &[],
        });
    }

    let mut instructions = Vec::with_capacity(DEALER_SCENARIO_EFFECT_OPERATION_COUNT_V4 - 1);
    instructions.push(EffectInstructionV3::write_u64(
        AccountCoordinateV3::fixed(DEALER_SCENARIO_OBLIGATION_ACCOUNT_V4),
        DEALER_SCENARIO_OBLIGATION_REVISION_OFFSET_V4,
        ScalarCoordinateV3::common(DEALER_SCENARIO_OBLIGATION_REVISION_SCALAR_V4),
    ));
    let item_instructions = [EffectInstructionV3::write_u64_affine(
        AccountCoordinateV3::fixed(DEALER_SCENARIO_OBLIGATION_ACCOUNT_V4),
        u32::try_from(DEALER_OBLIGATION_HEADER_BYTES_V3)
            .map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?,
        8,
        ScalarCoordinateV3::item(0),
    )];
    for slot in 0..DEALER_SCENARIO_CUSTODY_ROUTE_COUNT_V4 {
        push_scenario_custody_projection(slot, &mut instructions)?;
    }
    if instructions.len() + item_instructions.len() != DEALER_SCENARIO_EFFECT_OPERATION_COUNT_V4 {
        return Err(DealerScenarioArtifactsErrorV4::Effect);
    }
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
        &instructions,
        &item_instructions,
        scratch,
        output,
    )
    .map_err(|_| DealerScenarioArtifactsErrorV4::Effect)
}

/// Exact common scalar coordinate for one nested Custody V1 field.
pub fn dealer_scenario_custody_scalar_register_v4(
    slot: u16,
    field: DealerCustodyScalarFieldV3,
) -> Option<u16> {
    let field = match field {
        DealerCustodyScalarFieldV3::TransferIndex => 0,
        DealerCustodyScalarFieldV3::ExpectedRevision => 1,
        DealerCustodyScalarFieldV3::ResultingRevision => 2,
        DealerCustodyScalarFieldV3::OrderNonce => 3,
        DealerCustodyScalarFieldV3::Generation => 4,
        DealerCustodyScalarFieldV3::Amount => 5,
        DealerCustodyScalarFieldV3::RentLamports => 6,
        DealerCustodyScalarFieldV3::PageIndex => 7,
        DealerCustodyScalarFieldV3::ExecutionIndex => 8,
    };
    slot.checked_mul(DEALER_SCENARIO_CUSTODY_SCALAR_STRIDE_V4)
        .and_then(|offset| DEALER_SCENARIO_CUSTODY_SCALAR_BASE_V4.checked_add(offset))
        .and_then(|base| base.checked_add(field))
}

/// Exact common identity coordinate for one nested Custody V1 field.
pub fn dealer_scenario_custody_identity_register_v4(
    slot: u16,
    field: DealerCustodyIdentityFieldV3,
) -> Option<u16> {
    let field = match field {
        DealerCustodyIdentityFieldV3::ReleaseSet => 0,
        DealerCustodyIdentityFieldV3::Market => 1,
        DealerCustodyIdentityFieldV3::Realm => 2,
        DealerCustodyIdentityFieldV3::Context => 3,
        DealerCustodyIdentityFieldV3::CallerProgram => 4,
        DealerCustodyIdentityFieldV3::Candidate => 5,
        DealerCustodyIdentityFieldV3::SourceOwner => 6,
        DealerCustodyIdentityFieldV3::DestinationOwner => 7,
        DealerCustodyIdentityFieldV3::Order => 8,
        DealerCustodyIdentityFieldV3::Source => 9,
        DealerCustodyIdentityFieldV3::Destination => 10,
        DealerCustodyIdentityFieldV3::SourceVaultContext => 11,
        DealerCustodyIdentityFieldV3::DestinationVaultContext => 12,
        DealerCustodyIdentityFieldV3::Mint => 13,
        DealerCustodyIdentityFieldV3::TokenProgram => 14,
        DealerCustodyIdentityFieldV3::Payer => 15,
        DealerCustodyIdentityFieldV3::RentRefund => 16,
    };
    slot.checked_mul(DEALER_SCENARIO_CUSTODY_IDENTITY_STRIDE_V4)
        .and_then(|offset| DEALER_SCENARIO_CUSTODY_IDENTITY_BASE_V4.checked_add(offset))
        .and_then(|base| base.checked_add(field))
}

#[cfg(not(target_os = "solana"))]
fn push_scenario_custody_projection(
    slot: usize,
    output: &mut Vec<EffectInstructionV3>,
) -> Result<(), DealerScenarioArtifactsErrorV4> {
    let slot_u16 = u16::try_from(slot).map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?;
    let route = scenario_custody_route(slot)?;
    let delegated = slot < 2;
    let base = if delegated {
        DelegatedCustodyRequestLayoutV2::BASE
    } else {
        0
    };
    output.push(EffectInstructionV3::write_request_u16(
        route,
        RequestSpaceV3::Fixed,
        request_offset(base, CustodyRequestLayoutV1::TRANSFER_INDEX)?,
        ScalarCoordinateV3::common(scenario_custody_scalar(
            slot_u16,
            DealerCustodyScalarFieldV3::TransferIndex,
        )?),
    ));
    output.push(EffectInstructionV3::write_request_identity(
        route,
        RequestSpaceV3::Fixed,
        request_offset(base, CustodyRequestLayoutV1::PARENT_REQUEST_DIGEST)?,
        IdentityCoordinateV3::common(0),
    ));
    for (field, offset) in scenario_identity_fields()
        .into_iter()
        .zip(scenario_identity_offsets())
    {
        output.push(EffectInstructionV3::write_request_identity(
            route,
            RequestSpaceV3::Fixed,
            request_offset(base, offset)?,
            IdentityCoordinateV3::common(scenario_custody_identity(slot_u16, field)?),
        ));
    }
    for (field, offset) in scenario_u64_fields() {
        output.push(EffectInstructionV3::write_request_u64(
            route,
            RequestSpaceV3::Fixed,
            request_offset(base, offset)?,
            ScalarCoordinateV3::common(scenario_custody_scalar(slot_u16, field)?),
        ));
    }
    for (field, offset) in scenario_u32_fields() {
        output.push(EffectInstructionV3::write_request_u32(
            route,
            RequestSpaceV3::Fixed,
            request_offset(base, offset)?,
            ScalarCoordinateV3::common(scenario_custody_scalar(slot_u16, field)?),
        ));
    }
    if delegated {
        for (offset, field) in [
            (DelegatedCustodyRequestLayoutV2::STARTS_ATOMIC_DEBIT, 9),
            (DelegatedCustodyRequestLayoutV2::TERMINAL, 10),
        ] {
            output.push(EffectInstructionV3::write_request_u8(
                route,
                RequestSpaceV3::Fixed,
                request_offset(0, offset)?,
                ScalarCoordinateV3::common(scenario_extra_scalar(slot_u16, field)?),
            ));
        }
        for (offset, field) in [
            (DelegatedCustodyRequestLayoutV2::DELEGATE_BEFORE, 17),
            (DelegatedCustodyRequestLayoutV2::DELEGATE_AFTER, 18),
        ] {
            output.push(EffectInstructionV3::write_request_identity(
                route,
                RequestSpaceV3::Fixed,
                request_offset(0, offset)?,
                IdentityCoordinateV3::common(scenario_extra_identity(slot_u16, field)?),
            ));
        }
        for (offset, field) in [
            (DelegatedCustodyRequestLayoutV2::TOTAL_DEBIT, 11),
            (DelegatedCustodyRequestLayoutV2::ALLOWANCE_BEFORE, 12),
            (DelegatedCustodyRequestLayoutV2::ALLOWANCE_AFTER, 13),
        ] {
            output.push(EffectInstructionV3::write_request_u64(
                route,
                RequestSpaceV3::Fixed,
                request_offset(0, offset)?,
                ScalarCoordinateV3::common(scenario_extra_scalar(slot_u16, field)?),
            ));
        }
    }
    Ok(())
}

fn scenario_custody_slot(route: u16) -> Result<usize, DealerScenarioArtifactsErrorV4> {
    match route {
        0..=3 => Ok(usize::from(route)),
        5 | 6 => Ok(usize::from(route - 1)),
        _ => Err(DealerScenarioArtifactsErrorV4::Effect),
    }
}

fn scenario_custody_route(slot: usize) -> Result<u16, DealerScenarioArtifactsErrorV4> {
    match slot {
        0..=3 => u16::try_from(slot).map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic),
        4 | 5 => u16::try_from(slot + 1).map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic),
        _ => Err(DealerScenarioArtifactsErrorV4::Effect),
    }
}

fn scenario_route_span_scalar(slot: usize) -> Result<u16, DealerScenarioArtifactsErrorV4> {
    DEALER_SCENARIO_ROUTE_SPAN_SCALAR_BASE_V4
        .checked_add(u16::try_from(slot).map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?)
        .ok_or(DealerScenarioArtifactsErrorV4::Arithmetic)
}

fn scenario_custody_scalar(
    slot: u16,
    field: DealerCustodyScalarFieldV3,
) -> Result<u16, DealerScenarioArtifactsErrorV4> {
    dealer_scenario_custody_scalar_register_v4(slot, field)
        .ok_or(DealerScenarioArtifactsErrorV4::Arithmetic)
}

fn scenario_custody_identity(
    slot: u16,
    field: DealerCustodyIdentityFieldV3,
) -> Result<u16, DealerScenarioArtifactsErrorV4> {
    dealer_scenario_custody_identity_register_v4(slot, field)
        .ok_or(DealerScenarioArtifactsErrorV4::Arithmetic)
}

fn scenario_extra_scalar(slot: u16, field: u16) -> Result<u16, DealerScenarioArtifactsErrorV4> {
    slot.checked_mul(DEALER_SCENARIO_CUSTODY_SCALAR_STRIDE_V4)
        .and_then(|offset| DEALER_SCENARIO_CUSTODY_SCALAR_BASE_V4.checked_add(offset))
        .and_then(|base| base.checked_add(field))
        .ok_or(DealerScenarioArtifactsErrorV4::Arithmetic)
}

fn scenario_extra_identity(slot: u16, field: u16) -> Result<u16, DealerScenarioArtifactsErrorV4> {
    slot.checked_mul(DEALER_SCENARIO_CUSTODY_IDENTITY_STRIDE_V4)
        .and_then(|offset| DEALER_SCENARIO_CUSTODY_IDENTITY_BASE_V4.checked_add(offset))
        .and_then(|base| base.checked_add(field))
        .ok_or(DealerScenarioArtifactsErrorV4::Arithmetic)
}

fn request_offset(base: usize, field: usize) -> Result<u32, DealerScenarioArtifactsErrorV4> {
    base.checked_add(field)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(DealerScenarioArtifactsErrorV4::Arithmetic)
}

#[cfg(not(target_os = "solana"))]
fn validate_scenario_custody_shape(
    slot: usize,
    request: MultiLpCustodyRequestV3,
) -> Result<(), DealerScenarioArtifactsErrorV4> {
    let expected = match slot {
        0 => (CompartmentV1::External, CompartmentV1::TradingPrincipal),
        1 => (CompartmentV1::External, CompartmentV1::FeeVault),
        2 => (CompartmentV1::TradingPrincipal, CompartmentV1::FeeVault),
        3 => (
            CompartmentV1::TradingPrincipal,
            CompartmentV1::HoardPrincipal,
        ),
        4 => (
            CompartmentV1::HoardPrincipal,
            CompartmentV1::TradingPrincipal,
        ),
        5 => (CompartmentV1::TradingPrincipal, CompartmentV1::External),
        _ => return Err(DealerScenarioArtifactsErrorV4::Effect),
    };
    let custody = request.custody();
    let kind = matches!(
        (slot, request),
        (0 | 1, MultiLpCustodyRequestV3::Delegated(_))
            | (2..=5, MultiLpCustodyRequestV3::Canonical(_))
    );
    if !kind
        || custody.operation != OperationV1::Transfer
        || custody.caller_role != CallerRoleV1::Trading
        || (custody.source_compartment, custody.destination_compartment) != expected
    {
        return Err(DealerScenarioArtifactsErrorV4::Effect);
    }
    Ok(())
}

#[cfg(not(target_os = "solana"))]
fn canonicalize_custody_template(
    request: MultiLpCustodyRequestV3,
    bytes: &mut [u8],
) -> Result<(), DealerScenarioArtifactsErrorV4> {
    let base = if matches!(request, MultiLpCustodyRequestV3::Delegated(_)) {
        DelegatedCustodyRequestLayoutV2::BASE
    } else {
        0
    };
    for (offset, width) in core::iter::once((CustodyRequestLayoutV1::TRANSFER_INDEX, 2))
        .chain(core::iter::once((
            CustodyRequestLayoutV1::PARENT_REQUEST_DIGEST,
            32,
        )))
        .chain(
            scenario_identity_offsets()
                .into_iter()
                .map(|offset| (offset, 32)),
        )
        .chain(
            scenario_u64_fields()
                .into_iter()
                .map(|(_, offset)| (offset, 8)),
        )
        .chain(
            scenario_u32_fields()
                .into_iter()
                .map(|(_, offset)| (offset, 4)),
        )
    {
        clear(bytes, base, offset, width)?;
    }
    if matches!(request, MultiLpCustodyRequestV3::Delegated(_)) {
        for (offset, width) in [
            (DelegatedCustodyRequestLayoutV2::STARTS_ATOMIC_DEBIT, 1),
            (DelegatedCustodyRequestLayoutV2::TERMINAL, 1),
            (DelegatedCustodyRequestLayoutV2::DELEGATE_BEFORE, 32),
            (DelegatedCustodyRequestLayoutV2::DELEGATE_AFTER, 32),
            (DelegatedCustodyRequestLayoutV2::TOTAL_DEBIT, 8),
            (DelegatedCustodyRequestLayoutV2::ALLOWANCE_BEFORE, 8),
            (DelegatedCustodyRequestLayoutV2::ALLOWANCE_AFTER, 8),
        ] {
            clear(bytes, 0, offset, width)?;
        }
    }
    Ok(())
}

#[cfg(not(target_os = "solana"))]
fn clear(
    bytes: &mut [u8],
    base: usize,
    offset: usize,
    width: usize,
) -> Result<(), DealerScenarioArtifactsErrorV4> {
    let start = base
        .checked_add(offset)
        .ok_or(DealerScenarioArtifactsErrorV4::Arithmetic)?;
    let end = start
        .checked_add(width)
        .ok_or(DealerScenarioArtifactsErrorV4::Arithmetic)?;
    bytes
        .get_mut(start..end)
        .ok_or(DealerScenarioArtifactsErrorV4::Effect)?
        .fill(0);
    Ok(())
}

const fn scenario_identity_offsets() -> [usize; DEALER_SCENARIO_CUSTODY_BASE_IDENTITY_FIELDS_V4] {
    [
        CustodyRequestLayoutV1::RELEASE_SET,
        CustodyRequestLayoutV1::MARKET,
        CustodyRequestLayoutV1::REALM,
        CustodyRequestLayoutV1::CONTEXT,
        CustodyRequestLayoutV1::CALLER_PROGRAM,
        CustodyRequestLayoutV1::CANDIDATE,
        CustodyRequestLayoutV1::SOURCE_OWNER,
        CustodyRequestLayoutV1::DESTINATION_OWNER,
        CustodyRequestLayoutV1::ORDER,
        CustodyRequestLayoutV1::SOURCE,
        CustodyRequestLayoutV1::DESTINATION,
        CustodyRequestLayoutV1::SOURCE_VAULT_CONTEXT,
        CustodyRequestLayoutV1::DESTINATION_VAULT_CONTEXT,
        CustodyRequestLayoutV1::MINT,
        CustodyRequestLayoutV1::TOKEN_PROGRAM,
        CustodyRequestLayoutV1::PAYER,
        CustodyRequestLayoutV1::RENT_REFUND,
    ]
}

const fn scenario_identity_fields()
-> [DealerCustodyIdentityFieldV3; DEALER_SCENARIO_CUSTODY_BASE_IDENTITY_FIELDS_V4] {
    [
        DealerCustodyIdentityFieldV3::ReleaseSet,
        DealerCustodyIdentityFieldV3::Market,
        DealerCustodyIdentityFieldV3::Realm,
        DealerCustodyIdentityFieldV3::Context,
        DealerCustodyIdentityFieldV3::CallerProgram,
        DealerCustodyIdentityFieldV3::Candidate,
        DealerCustodyIdentityFieldV3::SourceOwner,
        DealerCustodyIdentityFieldV3::DestinationOwner,
        DealerCustodyIdentityFieldV3::Order,
        DealerCustodyIdentityFieldV3::Source,
        DealerCustodyIdentityFieldV3::Destination,
        DealerCustodyIdentityFieldV3::SourceVaultContext,
        DealerCustodyIdentityFieldV3::DestinationVaultContext,
        DealerCustodyIdentityFieldV3::Mint,
        DealerCustodyIdentityFieldV3::TokenProgram,
        DealerCustodyIdentityFieldV3::Payer,
        DealerCustodyIdentityFieldV3::RentRefund,
    ]
}

const fn scenario_u64_fields() -> [(DealerCustodyScalarFieldV3, usize); 6] {
    [
        (
            DealerCustodyScalarFieldV3::ExpectedRevision,
            CustodyRequestLayoutV1::EXPECTED_REVISION,
        ),
        (
            DealerCustodyScalarFieldV3::ResultingRevision,
            CustodyRequestLayoutV1::RESULTING_REVISION,
        ),
        (
            DealerCustodyScalarFieldV3::OrderNonce,
            CustodyRequestLayoutV1::ORDER_NONCE,
        ),
        (
            DealerCustodyScalarFieldV3::Generation,
            CustodyRequestLayoutV1::GENERATION,
        ),
        (
            DealerCustodyScalarFieldV3::Amount,
            CustodyRequestLayoutV1::AMOUNT,
        ),
        (
            DealerCustodyScalarFieldV3::RentLamports,
            CustodyRequestLayoutV1::RENT_LAMPORTS,
        ),
    ]
}

const fn scenario_u32_fields() -> [(DealerCustodyScalarFieldV3, usize); 2] {
    [
        (
            DealerCustodyScalarFieldV3::PageIndex,
            CustodyRequestLayoutV1::PAGE_INDEX,
        ),
        (
            DealerCustodyScalarFieldV3::ExecutionIndex,
            CustodyRequestLayoutV1::EXECUTION_INDEX,
        ),
    ]
}

/// Project the admitted selector-9 register bank from the exact semantic plan.
///
/// No route shape, amount, or Claims quantity is supplied separately: active
/// Custody routes are classified from the exact composer-owned requests, the
/// SignedDelta suffix owns P and all Claims rows, and the authenticated
/// candidate obligation account owns the repeated outcome vector.
#[allow(clippy::too_many_arguments)]
pub fn project_dealer_scenario_hot_registers_v4(
    request: DealerScenarioTradeRequestV3<'_>,
    plan: &ScenarioAtomicPlanV3,
    candidate_obligation: DealerObligationProjectionV3<'_>,
    custody_effects: &[Option<ScenarioCustodyEffectV3>; MAX_DEALER_SCENARIO_CUSTODY_EFFECTS_V3],
    trading_program: [u8; 32],
    trusted_current_slot: u64,
    scalars: &mut [u64],
    identities: &mut [[u8; 32]],
) -> Result<(), DealerScenarioArtifactsErrorV4> {
    let width =
        usize::try_from(request.width).map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?;
    let expected_scalars = usize::from(DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4)
        .checked_add(width)
        .ok_or(DealerScenarioArtifactsErrorV4::Arithmetic)?;
    validate_scenario_projection_header_v4(
        request,
        plan,
        candidate_obligation,
        trading_program,
        trusted_current_slot,
        expected_scalars,
        scalars,
        identities,
    )?;
    let parent_digest = hash(request.bytes()).to_bytes();
    let active_count =
        validate_scenario_custody_bank_v4(custody_effects, plan.custody_count, parent_digest)?;
    validate_scenario_custody_amounts_v4(
        request,
        plan,
        custody_effects,
        active_count,
        parent_digest,
    )?;

    let mut staged_scalars = vec![0_u64; expected_scalars];
    let mut staged_identities =
        vec![[0_u8; 32]; usize::from(DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4)];
    seed_scenario_projection_header_v4(
        request,
        plan,
        trading_program,
        trusted_current_slot,
        parent_digest,
        &mut staged_scalars,
        &mut staged_identities,
    )?;
    project_scenario_custody_bank_v4(
        custody_effects,
        active_count,
        parent_digest,
        trading_program,
        &mut staged_scalars,
        &mut staged_identities,
    )?;
    project_scenario_obligations_v4(candidate_obligation, &mut staged_scalars)?;
    scalars.copy_from_slice(&staged_scalars);
    identities.copy_from_slice(&staged_identities);
    Ok(())
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn validate_scenario_projection_header_v4(
    request: DealerScenarioTradeRequestV3<'_>,
    plan: &ScenarioAtomicPlanV3,
    candidate_obligation: DealerObligationProjectionV3<'_>,
    trading_program: [u8; 32],
    trusted_current_slot: u64,
    expected_scalars: usize,
    scalars: &[u64],
    identities: &[[u8; 32]],
) -> Result<(), DealerScenarioArtifactsErrorV4> {
    if scalars.len() != expected_scalars
        || identities.len() != usize::from(DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4)
        || trading_program == [0; 32]
        || trusted_current_slot > request.expires_at
        || candidate_obligation.width() != request.width
        || candidate_obligation.revision() != request.candidate_obligation_revision
        || candidate_obligation.state_digest() != request.candidate_obligation_digest
        || plan.obligation_revision_after != request.candidate_obligation_revision
        || plan.obligation_digest_after != request.candidate_obligation_digest
        || plan.claims.width != request.width
        || plan.claims.dealer_owner != request.dealer_owner
        || plan.claims.counterparty_owner != request.counterparty_owner
        || plan.claims.dealer_revision_before != request.dealer_position_revision
        || plan.claims.counterparty_revision_before != request.counterparty_position_revision
        || plan.claims.claims_revision_before != request.claims_revision
    {
        return Err(DealerScenarioArtifactsErrorV4::Projection);
    }
    let claims = request
        .claims_plan()
        .map_err(|_| DealerScenarioArtifactsErrorV4::Projection)?;
    if claims.position_count() != u32::from(request.claims_position_count)
        || claims.claim_count() != request.width
        || request.claims_packet().len()
            != usize::try_from(request.claims_packet_bytes)
                .map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?
    {
        return Err(DealerScenarioArtifactsErrorV4::Projection);
    }
    Ok(())
}

#[inline(never)]
fn validate_scenario_custody_bank_v4(
    custody_effects: &[Option<ScenarioCustodyEffectV3>; MAX_DEALER_SCENARIO_CUSTODY_EFFECTS_V3],
    custody_count: u8,
    parent_digest: [u8; 32],
) -> Result<usize, DealerScenarioArtifactsErrorV4> {
    let active_count = usize::from(custody_count);
    if active_count > custody_effects.len()
        || custody_effects
            .iter()
            .skip(active_count)
            .any(Option::is_some)
    {
        return Err(DealerScenarioArtifactsErrorV4::Projection);
    }
    for (index, effect) in custody_effects
        .iter()
        .take(active_count)
        .copied()
        .enumerate()
    {
        let effect = effect.ok_or(DealerScenarioArtifactsErrorV4::Projection)?;
        let slot = classify_scenario_custody(effect.request)
            .ok_or(DealerScenarioArtifactsErrorV4::Projection)?;
        if effect.request.custody().semantic.parent_request_digest != parent_digest
            || custody_effects
                .get(..index)
                .ok_or(DealerScenarioArtifactsErrorV4::Projection)?
                .iter()
                .copied()
                .flatten()
                .any(|prior| classify_scenario_custody(prior.request) == Some(slot))
        {
            return Err(DealerScenarioArtifactsErrorV4::Projection);
        }
    }
    Ok(active_count)
}

#[inline(never)]
fn validate_scenario_custody_amounts_v4(
    request: DealerScenarioTradeRequestV3<'_>,
    plan: &ScenarioAtomicPlanV3,
    custody_effects: &[Option<ScenarioCustodyEffectV3>; MAX_DEALER_SCENARIO_CUSTODY_EFFECTS_V3],
    active_count: usize,
    parent_digest: [u8; 32],
) -> Result<(), DealerScenarioArtifactsErrorV4> {
    let mut observed_count = 0_usize;
    for slot in 0..DEALER_SCENARIO_CUSTODY_ROUTE_COUNT_V4 {
        let expected_amount = scenario_expected_custody_amount_v4(request, plan, slot)?;
        match find_scenario_custody_effect_v4(custody_effects, active_count, parent_digest, slot)? {
            Some(effect)
                if expected_amount != 0 && effect.request.custody().amount == expected_amount =>
            {
                observed_count = observed_count
                    .checked_add(1)
                    .ok_or(DealerScenarioArtifactsErrorV4::Arithmetic)?;
            }
            None if expected_amount == 0 => {}
            _ => return Err(DealerScenarioArtifactsErrorV4::Projection),
        }
    }
    if observed_count != active_count {
        return Err(DealerScenarioArtifactsErrorV4::Projection);
    }
    Ok(())
}

#[inline(never)]
fn seed_scenario_projection_header_v4(
    request: DealerScenarioTradeRequestV3<'_>,
    plan: &ScenarioAtomicPlanV3,
    trading_program: [u8; 32],
    trusted_current_slot: u64,
    parent_digest: [u8; 32],
    staged_scalars: &mut [u64],
    staged_identities: &mut [[u8; 32]],
) -> Result<(), DealerScenarioArtifactsErrorV4> {
    set_scenario_scalar(
        staged_scalars,
        DEALER_SCENARIO_POSITION_COUNT_SCALAR_V4,
        u64::from(request.claims_position_count),
    )?;
    set_scenario_scalar(
        staged_scalars,
        DEALER_SCENARIO_DEALER_EVIDENCE_COUNT_SCALAR_V4,
        u64::from(request.dealer_evidence_count),
    )?;
    set_scenario_scalar(
        staged_scalars,
        DEALER_SCENARIO_POSITION_GEOMETRY_SUM_SCALAR_V4,
        u64::from(request.claims_position_count)
            .checked_add(u64::from(request.dealer_evidence_count))
            .ok_or(DealerScenarioArtifactsErrorV4::Arithmetic)?,
    )?;
    set_scenario_scalar(
        staged_scalars,
        DEALER_SCENARIO_WITNESS_BYTES_SCALAR_V4,
        u64::from(request.claims_packet_bytes),
    )?;
    set_scenario_scalar(
        staged_scalars,
        DEALER_SCENARIO_WITNESS_OFFSET_SCALAR_V4,
        u64::try_from(DEALER_SCENARIO_TRADE_HEADER_BYTES_V3)
            .map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?,
    )?;
    set_scenario_scalar(
        staged_scalars,
        DEALER_SCENARIO_CURRENT_SLOT_SCALAR_V4,
        trusted_current_slot,
    )?;
    set_scenario_scalar(
        staged_scalars,
        DEALER_SCENARIO_EXPIRY_SCALAR_V4,
        request.expires_at,
    )?;
    set_scenario_scalar(
        staged_scalars,
        DEALER_SCENARIO_MAX_POSITION_COUNT_SCALAR_V4,
        2,
    )?;
    set_scenario_scalar(
        staged_scalars,
        DEALER_SCENARIO_OBLIGATION_REVISION_SCALAR_V4,
        plan.obligation_revision_after,
    )?;
    *staged_identities
        .get_mut(0)
        .ok_or(DealerScenarioArtifactsErrorV4::Projection)? = parent_digest;
    set_scenario_identity(
        staged_identities,
        DEALER_SCENARIO_CURRENT_TRADING_IDENTITY_V4,
        trading_program,
    )?;
    set_scenario_identity(
        staged_identities,
        DEALER_SCENARIO_OBLIGATION_IDENTITY_V4,
        request.obligation,
    )?;
    Ok(())
}

#[inline(never)]
fn project_scenario_custody_bank_v4(
    custody_effects: &[Option<ScenarioCustodyEffectV3>; MAX_DEALER_SCENARIO_CUSTODY_EFFECTS_V3],
    active_count: usize,
    parent_digest: [u8; 32],
    trading_program: [u8; 32],
    staged_scalars: &mut [u64],
    staged_identities: &mut [[u8; 32]],
) -> Result<(), DealerScenarioArtifactsErrorV4> {
    for slot in 0..DEALER_SCENARIO_CUSTODY_ROUTE_COUNT_V4 {
        let Some(effect) =
            find_scenario_custody_effect_v4(custody_effects, active_count, parent_digest, slot)?
        else {
            continue;
        };
        if effect.request.custody().caller_program != trading_program {
            return Err(DealerScenarioArtifactsErrorV4::Projection);
        }
        set_scenario_scalar(
            staged_scalars,
            scenario_route_span_scalar(slot)?,
            u64::from(DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3),
        )?;
        project_scenario_custody_request(
            u16::try_from(slot).map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?,
            effect.request,
            staged_scalars,
            staged_identities,
        )?;
    }
    Ok(())
}

#[inline(never)]
fn project_scenario_obligations_v4(
    candidate_obligation: DealerObligationProjectionV3<'_>,
    staged_scalars: &mut [u64],
) -> Result<(), DealerScenarioArtifactsErrorV4> {
    for (index, obligation) in candidate_obligation.obligations().enumerate() {
        let destination = usize::from(DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4)
            .checked_add(index)
            .ok_or(DealerScenarioArtifactsErrorV4::Arithmetic)?;
        *staged_scalars
            .get_mut(destination)
            .ok_or(DealerScenarioArtifactsErrorV4::Projection)? = obligation;
    }
    Ok(())
}

fn find_scenario_custody_effect_v4(
    custody_effects: &[Option<ScenarioCustodyEffectV3>; MAX_DEALER_SCENARIO_CUSTODY_EFFECTS_V3],
    active_count: usize,
    parent_digest: [u8; 32],
    slot: usize,
) -> Result<Option<ScenarioCustodyEffectV3>, DealerScenarioArtifactsErrorV4> {
    let mut found = None;
    for effect in custody_effects.iter().take(active_count).copied() {
        let effect = effect.ok_or(DealerScenarioArtifactsErrorV4::Projection)?;
        if classify_scenario_custody(effect.request) == Some(slot)
            && (effect.request.custody().semantic.parent_request_digest != parent_digest
                || found.replace(effect).is_some())
        {
            return Err(DealerScenarioArtifactsErrorV4::Projection);
        }
    }
    Ok(found)
}

fn scenario_expected_custody_amount_v4(
    request: DealerScenarioTradeRequestV3<'_>,
    plan: &ScenarioAtomicPlanV3,
    slot: usize,
) -> Result<u64, DealerScenarioArtifactsErrorV4> {
    let incoming = request.direction == ScenarioTradeDirectionV3::CounterpartyPaysDealer;
    let outgoing = request.direction == ScenarioTradeDirectionV3::DealerPaysCounterparty;
    match slot {
        0 => Ok(if incoming { request.principal } else { 0 }),
        1 => Ok(if incoming { request.realized_fee } else { 0 }),
        2 => Ok(if outgoing { request.realized_fee } else { 0 }),
        3 => Ok(plan.scenario.minimum_complete_sets_to_split),
        4 => Ok(plan.scenario.maximum_complete_sets_to_merge),
        5 => {
            if outgoing {
                request
                    .principal
                    .checked_sub(request.realized_fee)
                    .ok_or(DealerScenarioArtifactsErrorV4::Projection)
            } else {
                Ok(0)
            }
        }
        _ => Err(DealerScenarioArtifactsErrorV4::Projection),
    }
}

fn project_scenario_custody_request(
    slot: u16,
    request: MultiLpCustodyRequestV3,
    scalars: &mut [u64],
    identities: &mut [[u8; 32]],
) -> Result<(), DealerScenarioArtifactsErrorV4> {
    let custody = request.custody();
    for (field, value) in [
        (
            DealerCustodyScalarFieldV3::TransferIndex,
            u64::from(custody.semantic.transfer_index),
        ),
        (
            DealerCustodyScalarFieldV3::ExpectedRevision,
            custody.expected_revision,
        ),
        (
            DealerCustodyScalarFieldV3::ResultingRevision,
            custody.resulting_revision,
        ),
        (
            DealerCustodyScalarFieldV3::OrderNonce,
            custody.semantic.order_nonce,
        ),
        (
            DealerCustodyScalarFieldV3::Generation,
            custody.semantic.generation,
        ),
        (DealerCustodyScalarFieldV3::Amount, custody.amount),
        (
            DealerCustodyScalarFieldV3::RentLamports,
            custody.rent_lamports,
        ),
        (
            DealerCustodyScalarFieldV3::PageIndex,
            u64::from(custody.semantic.page_index),
        ),
        (
            DealerCustodyScalarFieldV3::ExecutionIndex,
            u64::from(custody.semantic.execution_index),
        ),
    ] {
        set_scenario_scalar(scalars, scenario_custody_scalar(slot, field)?, value)?;
    }
    for (field, value) in scenario_identity_fields().into_iter().zip([
        custody.release_set,
        custody.market,
        custody.realm,
        custody.context,
        custody.caller_program,
        custody.semantic.candidate,
        custody.semantic.source_owner,
        custody.semantic.destination_owner,
        custody.semantic.order,
        custody.source,
        custody.destination,
        custody.source_vault_context,
        custody.destination_vault_context,
        custody.mint,
        custody.token_program,
        custody.payer,
        custody.rent_refund,
    ]) {
        set_scenario_identity(identities, scenario_custody_identity(slot, field)?, value)?;
    }
    if let MultiLpCustodyRequestV3::Delegated(value) = request {
        for (field, scalar) in [
            (9, u64::from(value.starts_atomic_debit)),
            (10, u64::from(value.terminal)),
            (11, value.total_debit),
            (12, value.allowance_before),
            (13, value.allowance_after),
        ] {
            set_scenario_scalar(scalars, scenario_extra_scalar(slot, field)?, scalar)?;
        }
        set_scenario_identity(
            identities,
            scenario_extra_identity(slot, 17)?,
            value.delegate_before,
        )?;
        set_scenario_identity(
            identities,
            scenario_extra_identity(slot, 18)?,
            value.delegate_after,
        )?;
    }
    Ok(())
}

fn classify_scenario_custody(request: MultiLpCustodyRequestV3) -> Option<usize> {
    let custody = request.custody();
    match (
        request,
        custody.source_compartment,
        custody.destination_compartment,
    ) {
        (
            MultiLpCustodyRequestV3::Delegated(_),
            CompartmentV1::External,
            CompartmentV1::TradingPrincipal,
        ) => Some(0),
        (
            MultiLpCustodyRequestV3::Delegated(_),
            CompartmentV1::External,
            CompartmentV1::FeeVault,
        ) => Some(1),
        (
            MultiLpCustodyRequestV3::Canonical(_),
            CompartmentV1::TradingPrincipal,
            CompartmentV1::FeeVault,
        ) => Some(2),
        (
            MultiLpCustodyRequestV3::Canonical(_),
            CompartmentV1::TradingPrincipal,
            CompartmentV1::HoardPrincipal,
        ) => Some(3),
        (
            MultiLpCustodyRequestV3::Canonical(_),
            CompartmentV1::HoardPrincipal,
            CompartmentV1::TradingPrincipal,
        ) => Some(4),
        (
            MultiLpCustodyRequestV3::Canonical(_),
            CompartmentV1::TradingPrincipal,
            CompartmentV1::External,
        ) => Some(5),
        _ => None,
    }
}

fn set_scenario_scalar(
    scalars: &mut [u64],
    index: u16,
    value: u64,
) -> Result<(), DealerScenarioArtifactsErrorV4> {
    *scalars
        .get_mut(usize::from(index))
        .ok_or(DealerScenarioArtifactsErrorV4::Projection)? = value;
    Ok(())
}

fn set_scenario_identity(
    identities: &mut [[u8; 32]],
    index: u16,
    value: [u8; 32],
) -> Result<(), DealerScenarioArtifactsErrorV4> {
    *identities
        .get_mut(usize::from(index))
        .ok_or(DealerScenarioArtifactsErrorV4::Projection)? = value;
    Ok(())
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
    let ranges = [
        BorrowedRangeV4::new(
            SEMANTIC_RANGE_ROUTE_V4,
            RequestCoordinateV4::Fixed(
                u32::try_from(DEALER_SCENARIO_TRADE_HEADER_BYTES_V3)
                    .map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?,
            ),
            RequestCoordinateV4::ProductTailAffine { base: 0, stride: 8 },
        ),
        BorrowedRangeV4::new(
            DEALER_SCENARIO_CLAIMS_ROUTE_V4,
            RequestCoordinateV4::ProductTailAffine {
                base: u16::try_from(DEALER_SCENARIO_TRADE_HEADER_BYTES_V3)
                    .map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?,
                stride: 8,
            },
            RequestCoordinateV4::CommonScalar(DEALER_SCENARIO_WITNESS_BYTES_SCALAR_V4),
        ),
    ];
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
    if request.request_profile().fixed_request_bytes()
        != u32::try_from(DEALER_SCENARIO_TRADE_HEADER_BYTES_V3)
            .map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?
        || request.request_profile().item_request_bytes() != 8
        || request.request_profile().common_scalar_count() != DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4
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
        .get_mut(usize::from(DEALER_SCENARIO_DEALER_EVIDENCE_COUNT_SCALAR_V4))
        .ok_or(DealerScenarioArtifactsErrorV4::Effect)? = 1;
    *scalars
        .get_mut(usize::from(DEALER_SCENARIO_POSITION_GEOMETRY_SUM_SCALAR_V4))
        .ok_or(DealerScenarioArtifactsErrorV4::Effect)? = 2;
    *scalars
        .get_mut(usize::from(DEALER_SCENARIO_WITNESS_BYTES_SCALAR_V4))
        .ok_or(DealerScenarioArtifactsErrorV4::Effect)? =
        u64::from(dealer_scenario_witness_bounds_v4()?.0);
    *scalars
        .get_mut(usize::from(DEALER_SCENARIO_WITNESS_OFFSET_SCALAR_V4))
        .ok_or(DealerScenarioArtifactsErrorV4::Effect)? =
        u64::try_from(DEALER_SCENARIO_TRADE_HEADER_BYTES_V3)
            .map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?;
    let mut coverage_scalars = vec![0_u64; scalars.len() + 1];
    coverage_scalars
        .get_mut(..scalars.len())
        .ok_or(DealerScenarioArtifactsErrorV4::Effect)?
        .copy_from_slice(scalars);
    effect
        .validate_request_coverage(
            DEALER_SCENARIO_TRADE_HEADER_BYTES_V3
                .checked_add(8)
                .ok_or(DealerScenarioArtifactsErrorV4::Arithmetic)?
                .checked_add(
                    usize::try_from(
                        *scalars
                            .get(usize::from(DEALER_SCENARIO_WITNESS_BYTES_SCALAR_V4))
                            .ok_or(DealerScenarioArtifactsErrorV4::Effect)?,
                    )
                    .map_err(|_| DealerScenarioArtifactsErrorV4::Arithmetic)?,
                )
                .ok_or(DealerScenarioArtifactsErrorV4::Arithmetic)?,
            1,
            &coverage_scalars,
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
    use dclutch_custody_contract::{ContextV1, CustodyRequestV1, DelegatedCustodyRequestV2};
    use dclutch_effect_kernel::v3::{
        HEADER_BYTES as EFFECT_HEADER_BYTES, RECEIPT_DEPENDENCY_BYTES, ROUTE_BYTES,
        RouteReceiptDependencyV3,
        encode::{EffectGeometryV3, RouteInputV3, encode_effect_program_v3_atomic},
    };
    use dclutch_request_profile_contract::ProjectionRegistersV1;
    use dclutch_transition_vm::v3::{RegisterInput, RegisterOutput, execute_fold_atomic};
    use std::vec;
    use std::vec::Vec;

    fn custody_template(slot: usize) -> MultiLpCustodyRequestV3 {
        let (source_compartment, destination_compartment) = match slot {
            0 => (CompartmentV1::External, CompartmentV1::TradingPrincipal),
            1 => (CompartmentV1::External, CompartmentV1::FeeVault),
            2 => (CompartmentV1::TradingPrincipal, CompartmentV1::FeeVault),
            3 => (
                CompartmentV1::TradingPrincipal,
                CompartmentV1::HoardPrincipal,
            ),
            4 => (
                CompartmentV1::HoardPrincipal,
                CompartmentV1::TradingPrincipal,
            ),
            5 => (CompartmentV1::TradingPrincipal, CompartmentV1::External),
            _ => (CompartmentV1::External, CompartmentV1::External),
        };
        let external_source = source_compartment == CompartmentV1::External;
        let external_destination = destination_compartment == CompartmentV1::External;
        let tag = u8::try_from(slot + 1).expect("tag");
        let request = CustodyRequestV1 {
            operation: OperationV1::Transfer,
            caller_role: CallerRoleV1::Trading,
            source_compartment,
            destination_compartment,
            release_set: [1; 32],
            market: [2; 32],
            realm: [3; 32],
            context: [4; 32],
            caller_program: [5; 32],
            semantic: ContextV1 {
                candidate: [6; 32],
                source_owner: if external_source { [7; 32] } else { [0; 32] },
                destination_owner: if external_destination {
                    [8; 32]
                } else {
                    [0; 32]
                },
                order: [9; 32],
                parent_request_digest: [10; 32],
                order_nonce: 11,
                generation: 12,
                page_index: 0,
                execution_index: 0,
                transfer_index: u16::try_from(slot).expect("slot"),
            },
            source: [tag; 32],
            destination: [tag + 20; 32],
            source_vault_context: if external_source { [0; 32] } else { [13; 32] },
            destination_vault_context: if external_destination {
                [0; 32]
            } else {
                [14; 32]
            },
            mint: [15; 32],
            token_program: [16; 32],
            payer: [0; 32],
            rent_refund: [0; 32],
            expected_revision: 17 + u64::try_from(slot).expect("slot"),
            resulting_revision: 18 + u64::try_from(slot).expect("slot"),
            amount: 1,
            rent_lamports: 0,
        };
        if slot < 2 {
            MultiLpCustodyRequestV3::Delegated(DelegatedCustodyRequestV2 {
                custody: request,
                starts_atomic_debit: true,
                terminal: true,
                delegate_before: [19; 32],
                delegate_after: [0; 32],
                total_debit: 1,
                allowance_before: 1,
                allowance_after: 0,
            })
        } else {
            MultiLpCustodyRequestV3::Canonical(request)
        }
    }

    fn custody_templates() -> [MultiLpCustodyRequestV3; 6] {
        core::array::from_fn(custody_template)
    }

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
        for (positions, tail_count) in [(1_u64, 1_u32), (2, 16)] {
            let witness = u64::from(dealer_scenario_witness_bounds_v4().expect("bounds").0);
            let mut scalars = vec![
                0;
                usize::from(DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4)
                    + usize::try_from(tail_count).expect("bounded")
            ];
            let identities = vec![[1; 32]; usize::from(DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4)];
            scalars[usize::from(DEALER_SCENARIO_POSITION_COUNT_SCALAR_V4)] = positions;
            scalars[usize::from(DEALER_SCENARIO_WITNESS_BYTES_SCALAR_V4)] = witness;
            scalars[usize::from(DEALER_SCENARIO_ROUTE_SPAN_SCALAR_BASE_V4)] = 14;
            scalars[usize::from(DEALER_SCENARIO_ROUTE_SPAN_SCALAR_BASE_V4 + 4)] = 14;
            let expected = usize::from(DEALER_SCENARIO_BASE_FIXED_ACCOUNTS_V4)
                + usize::try_from(positions).expect("small")
                + 28;
            assert_eq!(program.account_count(tail_count, &scalars), Ok(expected));
            let claims = program
                .resolved_invocation(
                    DEALER_SCENARIO_CLAIMS_ROUTE_V4,
                    0,
                    tail_count,
                    &scalars,
                    &identities,
                )
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
                        + usize::try_from(tail_count).expect("bounded") * 8
                        + usize::try_from(witness).expect("bounded"),
                    tail_count,
                    &scalars,
                    &identities,
                ),
                Ok(())
            );
        }
    }

    #[test]
    fn typed_base_effect_is_canonical_and_shifts_local_obligation() {
        let templates = custody_templates();
        let base_bytes = dealer_scenario_base_effect_program_bytes_v4().expect("base width");
        let mut base_scratch = vec![0; base_bytes];
        let mut base = vec![0; base_bytes];
        encode_dealer_scenario_base_effect_program_v4(&templates, &mut base_scratch, &mut base)
            .expect("base effect");
        let decoded = EffectProgramV3::decode(&base).expect("base decode");
        assert_eq!(decoded.route_count(), DEALER_SCENARIO_ROUTE_COUNT_V4);
        assert_eq!(decoded.fixed_operation_count(), 177);
        assert_eq!(decoded.item_operation_count(), 1);
        assert_eq!(
            decoded.route(0).expect("delegated").fixed_request_bytes(),
            776
        );
        assert_eq!(
            decoded.route(2).expect("canonical").fixed_request_bytes(),
            672
        );
        assert_eq!(
            decoded.route(5).expect("post Claims").receipt_dependency(),
            Some(RouteReceiptDependencyV3::new(
                FixedRole::Claims,
                DEALER_SCENARIO_CLAIMS_ROUTE_V4,
                376,
            ))
        );

        let effect_bytes = dealer_scenario_effect_program_bytes_v4(base.len()).expect("width");
        let mut effect_scratch = vec![0; effect_bytes];
        let mut effect = vec![0; effect_bytes];
        encode_dealer_scenario_effect_program_v4(&base, &mut effect_scratch, &mut effect)
            .expect("effect v4");
        let program = EffectProgramV4::decode(&effect).expect("decode v4");
        let mut scalars = vec![0; usize::from(DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4) + 2];
        let identities = vec![[1; 32]; usize::from(DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4)];
        scalars[usize::from(DEALER_SCENARIO_POSITION_COUNT_SCALAR_V4)] = 2;
        scalars[usize::from(DEALER_SCENARIO_ROUTE_SPAN_SCALAR_BASE_V4)] = 14;
        scalars[usize::from(DEALER_SCENARIO_ROUTE_SPAN_SCALAR_BASE_V4 + 4)] = 14;
        assert_eq!(program.account_count(2, &scalars), Ok(56));
        assert!(matches!(
            program.resolved_fixed_effect(0, 2, &scalars, &identities),
            Ok(dclutch_effect_kernel::v3::ResolvedEffectV3::WriteScalar {
                account: 55,
                offset: 16,
                ..
            })
        ));
        assert!(matches!(
            program.resolved_item_effect(1, 0, 2, &scalars, &identities),
            Ok(dclutch_effect_kernel::v3::ResolvedEffectV3::WriteScalar {
                account: 55,
                offset: 200,
                ..
            })
        ));

        let mut altered = custody_templates();
        altered[5] = custody_template(4);
        assert_eq!(
            encode_dealer_scenario_base_effect_program_v4(&altered, &mut base_scratch, &mut base,),
            Err(DealerScenarioArtifactsErrorV4::Effect)
        );
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
        assert_eq!(decoded.request_profile().item_request_bytes(), 8);

        let witness = usize::try_from(dealer_scenario_witness_bounds_v4().expect("bounds").0)
            .expect("bounded");
        let mut family_request = vec![0_u8; 384 + 2 * 8 + witness];
        family_request[..8].copy_from_slice(&DEALER_SCENARIO_TRADE_MAGIC_V3);
        family_request[8..10].copy_from_slice(&DEALER_SCENARIO_TRADE_VERSION_V3.to_le_bytes());
        family_request[10..12].copy_from_slice(&DEALER_SCENARIO_TRADE_ACTION_V3.to_le_bytes());
        family_request[377] = 1;
        family_request[378] = 1;
        family_request[380..384]
            .copy_from_slice(&u32::try_from(witness).expect("bounded").to_le_bytes());
        family_request[352..360].copy_from_slice(&99_u64.to_le_bytes());
        family_request[112..144].copy_from_slice(&[7; 32]);
        family_request[384..392].copy_from_slice(&11_u64.to_le_bytes());
        family_request[392..400].copy_from_slice(&29_u64.to_le_bytes());
        family_request[400..408].copy_from_slice(&SIGNED_DELTA_PLAN_MAGIC_V3);
        let scalar_count = usize::from(DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4) + 2;
        let identity_count = usize::from(DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4);
        let input_scalars = vec![0_u64; scalar_count];
        let mut scratch_scalars = vec![0_u64; scalar_count];
        let mut output_scalars = vec![0_u64; scalar_count];
        let input_identities = vec![[0_u8; 32]; identity_count];
        let mut scratch_identities = vec![[0_u8; 32]; identity_count];
        let mut output_identities = vec![[0_u8; 32]; identity_count];
        decoded
            .project_prefix_atomic(
                2,
                &family_request,
                ProjectionRegistersV1 {
                    input_scalars: &input_scalars,
                    input_identities: &input_identities,
                    scratch_scalars: &mut scratch_scalars,
                    scratch_identities: &mut scratch_identities,
                    output_scalars: &mut output_scalars,
                    output_identities: &mut output_identities,
                },
            )
            .expect("project candidate obligations");
        assert_eq!(
            &output_scalars[usize::from(DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4)..],
            &[11, 29]
        );
        assert_eq!(
            output_scalars
                .get(usize::from(DEALER_SCENARIO_DEALER_EVIDENCE_COUNT_SCALAR_V4))
                .copied(),
            Some(1)
        );

        let mut transition_scratch = [0; DEALER_SCENARIO_TRANSITION_BYTES_V4];
        let mut transition = [0; DEALER_SCENARIO_TRANSITION_BYTES_V4];
        encode_dealer_scenario_transition_v4(&mut transition_scratch, &mut transition)
            .expect("transition");
        let transition_program = TransitionProgramV3::decode(&transition).expect("decode");
        let mut executed_scalars = vec![0_u64; scalar_count];
        let mut executed_identities = vec![[0_u8; 32]; identity_count];
        let mut execution_scratch_scalars = vec![0_u64; scalar_count];
        let mut execution_scratch_identities = vec![[0_u8; 32]; identity_count];
        output_scalars[usize::from(DEALER_SCENARIO_CURRENT_SLOT_SCALAR_V4)] = 98;
        execute_fold_atomic(
            transition_program,
            2,
            RegisterInput {
                scalars: &output_scalars,
                identities: &output_identities,
            },
            RegisterOutput {
                scalars: &mut execution_scratch_scalars,
                identities: &mut execution_scratch_identities,
            },
            RegisterOutput {
                scalars: &mut executed_scalars,
                identities: &mut executed_identities,
            },
        )
        .expect("P1 plus one evidence account");
        assert_eq!(
            executed_scalars
                .get(usize::from(DEALER_SCENARIO_POSITION_GEOMETRY_SUM_SCALAR_V4))
                .copied(),
            Some(2)
        );
        let mut hostile_scalars = output_scalars.clone();
        hostile_scalars[usize::from(DEALER_SCENARIO_DEALER_EVIDENCE_COUNT_SCALAR_V4)] = 0;
        let untouched = executed_scalars.clone();
        assert!(
            execute_fold_atomic(
                transition_program,
                2,
                RegisterInput {
                    scalars: &hostile_scalars,
                    identities: &output_identities,
                },
                RegisterOutput {
                    scalars: &mut execution_scratch_scalars,
                    identities: &mut execution_scratch_identities,
                },
                RegisterOutput {
                    scalars: &mut executed_scalars,
                    identities: &mut executed_identities,
                },
            )
            .is_err()
        );
        assert_eq!(executed_scalars, untouched);

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
