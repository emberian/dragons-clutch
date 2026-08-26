//! Canonical occurrence-specific artifact emitters for Series Consume.
//!
//! The five child requests are semantic inputs owned by their respective
//! Core, Custody, and Claims codecs. This module does not restate those DTOs:
//! it orders their exact encoded bytes into the generic Effect request bank
//! and emits the matching RequestProfile, TransitionVM, and DCE5 programs.
//! All encoders are allocation-free and failure-atomic over caller buffers.

use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{
        HEADER_BYTES as EFFECT_HEADER_BYTES_V4, RECEIPT_DEPENDENCY_BYTES,
        ROUTE_BYTES as EFFECT_ROUTE_BYTES_V4, RouteKindV3,
        encode::{EffectGeometryV3, RouteInputV3, encode_effect_program_v4_atomic},
    },
};
use dclutch_request_profile_contract::{
    HEADER_BYTES as REQUEST_PROFILE_HEADER_BYTES_V4,
    OPERATION_BYTES as REQUEST_PROFILE_OPERATION_BYTES_V4,
    encode::{
        RequestCoordinateV1, RequestGeometryV1, RequestInstructionV1, ScalarRegisterV1,
        encode_request_profile_v1_atomic,
    },
};
use dclutch_transition_vm::v3::{
    HEADER_BYTES as TRANSITION_HEADER_BYTES_V4,
    INSTRUCTION_BYTES as TRANSITION_INSTRUCTION_BYTES_V4, InstructionV3, ProgramGeometryV3,
    ScalarRegisterV3, encode_program_atomic,
};

use super::{
    artifacts_v3::{
        SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3, SERIES_CLAIMS_RECEIPT_DEPENDENCIES_V3,
        SERIES_CONSUME_CLAIMS_ACCOUNT_COUNT_V3, SERIES_CONSUME_CORE_FOUND_ACCOUNT_BASE_V3,
        SERIES_CONSUME_CORE_OPEN_ACCOUNT_COUNT_V3, SERIES_CONSUME_CORE_REQUEST_BYTES_V3,
        SERIES_CONSUME_IR_REQUEST_BYTES_V3, SERIES_CONSUME_LOCK_ACCOUNT_COUNT_V3,
        SERIES_CONSUME_REALIZE_ACCOUNT_COUNT_V3, SERIES_CORE_FOUND_RECEIPT_DEPENDENCIES_V3,
        SERIES_CORE_OPEN_RECEIPT_DEPENDENCIES_V3, SERIES_NO_RECEIPT_DEPENDENCIES_V3,
        SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3, SERIES_WITNESS_ITEM_BYTES_V3,
    },
    effect_v4::{
        SERIES_CONSUME_EFFECT_V4_OVERHEAD_BYTES, SERIES_CONSUME_INJECTED_ACCOUNT_COUNT_V4,
        SERIES_CONSUME_LOGICAL_ACCOUNT_BASE_V4, encode_series_consume_effect_v4_atomic,
    },
    instruction::SERIES_ACTION_HEADER_BYTES_V3,
};

/// Exact common scalar bank shared by RequestProfile, Transition, and Effect.
pub const SERIES_CONSUME_COMMON_SCALAR_COUNT_V4: u16 = 5;
/// Exact common identity bank containing the authenticated Trading program.
pub const SERIES_CONSUME_COMMON_IDENTITY_COUNT_V4: u16 = 1;
/// Exact fixed RequestProfile operation count.
pub const SERIES_CONSUME_REQUEST_PROFILE_OPERATION_COUNT_V4: usize = 2;
/// Exact Series Consume RequestProfile width.
pub const SERIES_CONSUME_REQUEST_PROFILE_BYTES_V4: usize = REQUEST_PROFILE_HEADER_BYTES_V4
    + SERIES_CONSUME_REQUEST_PROFILE_OPERATION_COUNT_V4 * REQUEST_PROFILE_OPERATION_BYTES_V4;
/// Exact fixed TransitionVM operation count.
pub const SERIES_CONSUME_TRANSITION_OPERATION_COUNT_V4: usize = 3;
/// Exact Series Consume TransitionVM width.
pub const SERIES_CONSUME_TRANSITION_BYTES_V4: usize = TRANSITION_HEADER_BYTES_V4
    + SERIES_CONSUME_TRANSITION_OPERATION_COUNT_V4 * TRANSITION_INSTRUCTION_BYTES_V4;
/// Exact number of ordered receipt-dependency entries in the five-route plan.
pub const SERIES_CONSUME_RECEIPT_DEPENDENCY_COUNT_V4: usize = 4;
/// Exact underlying ordered-dependency EffectProgram width.
pub const SERIES_CONSUME_BASE_EFFECT_BYTES_V4: usize = EFFECT_HEADER_BYTES_V4
    + 5 * EFFECT_ROUTE_BYTES_V4
    + SERIES_CONSUME_RECEIPT_DEPENDENCY_COUNT_V4 * RECEIPT_DEPENDENCY_BYTES
    + SERIES_CONSUME_IR_REQUEST_BYTES_V3;
/// Exact DCE5 Series Consume Effect width.
pub const SERIES_CONSUME_EFFECT_BYTES_V4: usize =
    SERIES_CONSUME_BASE_EFFECT_BYTES_V4 + SERIES_CONSUME_EFFECT_V4_OVERHEAD_BYTES;

const ACTION_SELECTOR_OFFSET: u32 = 12;
const PROOF_COUNT_OFFSET: u32 = 13;
const PROOF_COUNT_SCALAR: u16 = 2;
const HEADER_BYTES_SCALAR: u16 = 0;
const PROOF_BYTES_SCALAR: u16 = 1;
const PROOF_ITEM_BYTES_SCALAR: u16 = 3;

const LOCK_ACCOUNT_START: u16 = SERIES_CONSUME_INJECTED_ACCOUNT_COUNT_V4;
const FOUND_ACCOUNT_START: u16 = LOCK_ACCOUNT_START + SERIES_CONSUME_LOCK_ACCOUNT_COUNT_V3;
const REALIZE_ACCOUNT_START: u16 = FOUND_ACCOUNT_START + SERIES_CONSUME_CORE_FOUND_ACCOUNT_BASE_V3;
const CLAIMS_ACCOUNT_START: u16 = REALIZE_ACCOUNT_START + SERIES_CONSUME_REALIZE_ACCOUNT_COUNT_V3;
const OPEN_ACCOUNT_START: u16 = CLAIMS_ACCOUNT_START + SERIES_CONSUME_CLAIMS_ACCOUNT_COUNT_V3;

const _: () = assert!(SERIES_CONSUME_IR_REQUEST_BYTES_V3 == 3_040);
const _: () = assert!(SERIES_CONSUME_BASE_EFFECT_BYTES_V4 == 3_264);
const _: () = assert!(SERIES_CONSUME_EFFECT_BYTES_V4 == 3_336);
const _: () = assert!(OPEN_ACCOUNT_START + SERIES_CONSUME_CORE_OPEN_ACCOUNT_COUNT_V3 == 157);

/// Exact child requests owned by the canonical child codecs.
///
/// Both Core routes intentionally reuse one exact 336-byte request. The
/// generic receipt-dependency table, rather than a second caller-supplied Core
/// DTO, distinguishes Found from final Open execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesConsumeChildRequestsV4<'a> {
    /// Projected Custody `LockHoardAndCloseSource` request.
    pub lock: &'a [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
    /// Sole exact Series Core base request used by Found and Open.
    pub core: &'a [u8; SERIES_CONSUME_CORE_REQUEST_BYTES_V3],
    /// Projected Custody `RealizeAndClose` request.
    pub realize: &'a [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
    /// Claims Founding V5 semantic request before its two typed receipts.
    pub claims: &'a [u8; SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3],
}

/// Stable refusal from canonical Series Consume artifact emission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesConsumeArtifactEmitErrorV4 {
    /// One caller-owned buffer did not have its exact frozen width.
    Buffer,
    /// RequestProfile construction or hostile decode refused.
    RequestProfile,
    /// TransitionVM construction or hostile decode refused.
    Transition,
    /// Underlying ordered-dependency Effect construction refused.
    BaseEffect,
    /// DCE5 dynamic-span/borrowed-range construction refused.
    Effect,
}

/// Emit the exact fixed-header Series Consume RequestProfile.
pub fn encode_series_consume_request_profile_v4_atomic(
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), SeriesConsumeArtifactEmitErrorV4> {
    let instructions = [
        RequestInstructionV1::require_u8(
            RequestCoordinateV1::fixed(ACTION_SELECTOR_OFFSET),
            dclutch_series_v3_kernel::request::SeriesActionV3::Consume as u8,
        ),
        RequestInstructionV1::project_u8(
            RequestCoordinateV1::fixed(PROOF_COUNT_OFFSET),
            ScalarRegisterV1::common(PROOF_COUNT_SCALAR),
        ),
    ];
    encode_request_profile_v1_atomic(
        RequestGeometryV1::new(
            u32::try_from(SERIES_ACTION_HEADER_BYTES_V3)
                .map_err(|_| SeriesConsumeArtifactEmitErrorV4::RequestProfile)?,
            0,
            SERIES_CONSUME_COMMON_SCALAR_COUNT_V4,
            0,
            SERIES_CONSUME_COMMON_IDENTITY_COUNT_V4,
            0,
        ),
        &instructions,
        &[],
        scratch,
        output,
    )
    .map_err(|_| SeriesConsumeArtifactEmitErrorV4::RequestProfile)
}

/// Emit the exact Series Consume TransitionVM program.
///
/// The RequestProfile projects the proof-word count into scalar 2. This
/// transition fixes the 128-byte header and 32-byte sibling width and derives
/// the exact proof byte count in scalar 1 without trusting caller arithmetic.
pub fn encode_series_consume_transition_v4_atomic(
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), SeriesConsumeArtifactEmitErrorV4> {
    let prelude = [
        InstructionV3::load_const(
            ScalarRegisterV3::common(HEADER_BYTES_SCALAR),
            u64::try_from(SERIES_ACTION_HEADER_BYTES_V3)
                .map_err(|_| SeriesConsumeArtifactEmitErrorV4::Transition)?,
        ),
        InstructionV3::load_const(
            ScalarRegisterV3::common(PROOF_ITEM_BYTES_SCALAR),
            u64::try_from(SERIES_WITNESS_ITEM_BYTES_V3)
                .map_err(|_| SeriesConsumeArtifactEmitErrorV4::Transition)?,
        ),
        InstructionV3::checked_mul_into(
            ScalarRegisterV3::common(PROOF_COUNT_SCALAR),
            ScalarRegisterV3::common(PROOF_ITEM_BYTES_SCALAR),
            ScalarRegisterV3::common(PROOF_BYTES_SCALAR),
        ),
    ];
    encode_program_atomic(
        ProgramGeometryV3 {
            common_scalars: SERIES_CONSUME_COMMON_SCALAR_COUNT_V4,
            item_scalar_stride: 0,
            common_identities: SERIES_CONSUME_COMMON_IDENTITY_COUNT_V4,
            item_identity_stride: 0,
        },
        &prelude,
        &[],
        &[],
        scratch,
        output,
    )
    .map_err(|_| SeriesConsumeArtifactEmitErrorV4::Transition)
}

/// Copy the exact five-route child request bank atomically.
pub fn encode_series_consume_request_bank_v4_atomic(
    requests: SeriesConsumeChildRequestsV4<'_>,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), SeriesConsumeArtifactEmitErrorV4> {
    if scratch.len() != SERIES_CONSUME_IR_REQUEST_BYTES_V3
        || output.len() != SERIES_CONSUME_IR_REQUEST_BYTES_V3
    {
        return Err(SeriesConsumeArtifactEmitErrorV4::Buffer);
    }
    let mut cursor = 0_usize;
    for request in [
        requests.lock.as_slice(),
        requests.core.as_slice(),
        requests.realize.as_slice(),
        requests.claims.as_slice(),
        requests.core.as_slice(),
    ] {
        let end = cursor
            .checked_add(request.len())
            .ok_or(SeriesConsumeArtifactEmitErrorV4::Buffer)?;
        scratch
            .get_mut(cursor..end)
            .ok_or(SeriesConsumeArtifactEmitErrorV4::Buffer)?
            .copy_from_slice(request);
        cursor = end;
    }
    if cursor != scratch.len() {
        return Err(SeriesConsumeArtifactEmitErrorV4::Buffer);
    }
    output.copy_from_slice(scratch);
    Ok(())
}

/// Emit the exact global five-route DCE5 program from canonical child bytes.
///
/// `base_output` is retained as a separately hostile-decodable intermediate;
/// the final `output` is committed only after DCE5 validates the complete
/// dynamic funding span and duplicate proof ranges.
pub fn encode_series_consume_effect_v4_from_requests_atomic(
    requests: SeriesConsumeChildRequestsV4<'_>,
    base_scratch: &mut [u8],
    base_output: &mut [u8],
    successor_scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), SeriesConsumeArtifactEmitErrorV4> {
    if base_scratch.len() != SERIES_CONSUME_BASE_EFFECT_BYTES_V4
        || base_output.len() != SERIES_CONSUME_BASE_EFFECT_BYTES_V4
        || successor_scratch.len() != SERIES_CONSUME_EFFECT_BYTES_V4
        || output.len() != SERIES_CONSUME_EFFECT_BYTES_V4
    {
        return Err(SeriesConsumeArtifactEmitErrorV4::Buffer);
    }
    let routes = [
        route(
            FixedRole::Custody,
            LOCK_ACCOUNT_START,
            SERIES_CONSUME_LOCK_ACCOUNT_COUNT_V3,
            requests.lock,
        ),
        route(
            FixedRole::Core,
            FOUND_ACCOUNT_START,
            SERIES_CONSUME_CORE_FOUND_ACCOUNT_BASE_V3,
            requests.core,
        ),
        route(
            FixedRole::Custody,
            REALIZE_ACCOUNT_START,
            SERIES_CONSUME_REALIZE_ACCOUNT_COUNT_V3,
            requests.realize,
        ),
        route(
            FixedRole::Claims,
            CLAIMS_ACCOUNT_START,
            SERIES_CONSUME_CLAIMS_ACCOUNT_COUNT_V3,
            requests.claims,
        ),
        route(
            FixedRole::Core,
            OPEN_ACCOUNT_START,
            SERIES_CONSUME_CORE_OPEN_ACCOUNT_COUNT_V3,
            requests.core,
        ),
    ];
    let dependencies = [
        &SERIES_NO_RECEIPT_DEPENDENCIES_V3[..],
        &SERIES_CORE_FOUND_RECEIPT_DEPENDENCIES_V3[..],
        &SERIES_NO_RECEIPT_DEPENDENCIES_V3[..],
        &SERIES_CLAIMS_RECEIPT_DEPENDENCIES_V3[..],
        &SERIES_CORE_OPEN_RECEIPT_DEPENDENCIES_V3[..],
    ];
    encode_effect_program_v4_atomic(
        EffectGeometryV3 {
            fixed_accounts: SERIES_CONSUME_LOGICAL_ACCOUNT_BASE_V4,
            item_account_stride: 0,
            common_scalars: SERIES_CONSUME_COMMON_SCALAR_COUNT_V4,
            item_scalar_stride: 0,
            common_identities: SERIES_CONSUME_COMMON_IDENTITY_COUNT_V4,
            item_identity_stride: 0,
        },
        &routes,
        &dependencies,
        &[],
        &[],
        base_scratch,
        base_output,
    )
    .map_err(|_| SeriesConsumeArtifactEmitErrorV4::BaseEffect)?;
    encode_series_consume_effect_v4_atomic(base_output, successor_scratch, output)
        .map_err(|_| SeriesConsumeArtifactEmitErrorV4::Effect)
}

fn route<'a>(
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
    use dclutch_effect_kernel::v4::ProgramV4;
    use dclutch_request_profile_contract::RequestProfileV1;
    use dclutch_transition_vm::v3::ProgramV3 as TransitionProgramV3;

    use super::*;

    fn requests() -> (
        [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
        [u8; SERIES_CONSUME_CORE_REQUEST_BYTES_V3],
        [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
        [u8; SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3],
    ) {
        (
            [0x11; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
            [0x22; SERIES_CONSUME_CORE_REQUEST_BYTES_V3],
            [0x33; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
            [0x44; SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3],
        )
    }

    #[test]
    fn request_profile_and_transition_are_exact_typed_programs() {
        let mut request_scratch = [0_u8; SERIES_CONSUME_REQUEST_PROFILE_BYTES_V4];
        let mut request = [0_u8; SERIES_CONSUME_REQUEST_PROFILE_BYTES_V4];
        encode_series_consume_request_profile_v4_atomic(&mut request_scratch, &mut request)
            .expect("request profile");
        let decoded = RequestProfileV1::decode(&request).expect("hostile decode request profile");
        assert_eq!(decoded.fixed_request_bytes(), 128);
        assert_eq!(decoded.common_scalar_count(), 5);
        assert_eq!(decoded.common_identity_count(), 1);

        let mut transition_scratch = [0_u8; SERIES_CONSUME_TRANSITION_BYTES_V4];
        let mut transition = [0_u8; SERIES_CONSUME_TRANSITION_BYTES_V4];
        encode_series_consume_transition_v4_atomic(&mut transition_scratch, &mut transition)
            .expect("transition");
        let decoded = TransitionProgramV3::decode(&transition).expect("hostile decode transition");
        assert_eq!(decoded.common_scalar_count(), 5);
        assert_eq!(decoded.common_identity_count(), 1);
    }

    #[test]
    fn request_bank_reuses_the_one_exact_core_request() {
        let (lock, core, realize, claims) = requests();
        let inputs = SeriesConsumeChildRequestsV4 {
            lock: &lock,
            core: &core,
            realize: &realize,
            claims: &claims,
        };
        let mut scratch = [0_u8; SERIES_CONSUME_IR_REQUEST_BYTES_V3];
        let mut output = [0_u8; SERIES_CONSUME_IR_REQUEST_BYTES_V3];
        encode_series_consume_request_bank_v4_atomic(inputs, &mut scratch, &mut output)
            .expect("request bank");
        assert_eq!(&output[..768], lock.as_slice());
        assert_eq!(&output[768..1_104], core.as_slice());
        assert_eq!(&output[1_104..1_872], realize.as_slice());
        assert_eq!(&output[1_872..2_704], claims.as_slice());
        assert_eq!(&output[2_704..], core.as_slice());
    }

    #[test]
    fn effect_emitter_preserves_exact_route_templates_and_dependencies() {
        let (lock, core, realize, claims) = requests();
        let inputs = SeriesConsumeChildRequestsV4 {
            lock: &lock,
            core: &core,
            realize: &realize,
            claims: &claims,
        };
        let mut base_scratch = vec![0_u8; SERIES_CONSUME_BASE_EFFECT_BYTES_V4];
        let mut base = vec![0_u8; SERIES_CONSUME_BASE_EFFECT_BYTES_V4];
        let mut effect_scratch = vec![0_u8; SERIES_CONSUME_EFFECT_BYTES_V4];
        let mut effect = vec![0_u8; SERIES_CONSUME_EFFECT_BYTES_V4];
        encode_series_consume_effect_v4_from_requests_atomic(
            inputs,
            &mut base_scratch,
            &mut base,
            &mut effect_scratch,
            &mut effect,
        )
        .expect("effect");
        let decoded = ProgramV4::decode(&effect).expect("hostile decode DCE5");
        assert_eq!(decoded.base().route_template(0).expect("lock").0, lock);
        assert_eq!(decoded.base().route_template(1).expect("found").0, core);
        assert_eq!(
            decoded.base().route_template(2).expect("realize").0,
            realize
        );
        assert_eq!(decoded.base().route_template(3).expect("claims").0, claims);
        assert_eq!(decoded.base().route_template(4).expect("open").0, core);
        assert_eq!(
            decoded
                .base()
                .route(1)
                .expect("found route")
                .receipt_dependency_count(),
            1
        );
        assert_eq!(
            decoded
                .base()
                .route(3)
                .expect("claims route")
                .receipt_dependency_count(),
            2
        );
        assert_eq!(
            decoded
                .base()
                .route(4)
                .expect("open route")
                .receipt_dependency_count(),
            1
        );
    }

    #[test]
    fn every_emitter_is_failure_atomic_on_width_refusal() {
        let mut short_request_scratch = [0_u8; SERIES_CONSUME_REQUEST_PROFILE_BYTES_V4 - 1];
        let mut request = [0x7a; SERIES_CONSUME_REQUEST_PROFILE_BYTES_V4];
        assert_eq!(
            encode_series_consume_request_profile_v4_atomic(
                &mut short_request_scratch,
                &mut request,
            ),
            Err(SeriesConsumeArtifactEmitErrorV4::RequestProfile)
        );
        assert!(request.iter().all(|byte| *byte == 0x7a));

        let (lock, core, realize, claims) = requests();
        let inputs = SeriesConsumeChildRequestsV4 {
            lock: &lock,
            core: &core,
            realize: &realize,
            claims: &claims,
        };
        let mut short_bank = [0_u8; SERIES_CONSUME_IR_REQUEST_BYTES_V3 - 1];
        let mut bank = [0x6b; SERIES_CONSUME_IR_REQUEST_BYTES_V3];
        assert_eq!(
            encode_series_consume_request_bank_v4_atomic(inputs, &mut short_bank, &mut bank),
            Err(SeriesConsumeArtifactEmitErrorV4::Buffer)
        );
        assert!(bank.iter().all(|byte| *byte == 0x6b));
    }
}
