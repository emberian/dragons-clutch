//! Static admission of the recurring-Series Consume Effect V4 artifact.
//!
//! The one global program has five `Once` routes.  A projected-Market outer
//! may execute only routes `[0, 2)`; after the exact Core Found receipt has
//! promoted the bounded funding-count hint into an attested scalar, the
//! ordinary live-Market outer may execute only `[2, 5)`.  This module grants
//! no receipt, scalar, CPI, or state-write authority.  It only proves that one
//! finalized generic Effect V4 artifact encodes the frozen Series topology.

use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{ProgramV3, RouteKindV3, RouteReceiptDependencyV3},
    v4::{
        BorrowedRangePolicyV4, BorrowedRangeV4, DynamicFixedSpanV4, ProgramV4, RequestCoordinateV4,
    },
};
use dclutch_series_v3_kernel::request::{SeriesActionRequestV3, SeriesActionV3};

use super::artifacts_v3::{
    SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3, SERIES_CLAIMS_RECEIPT_DEPENDENCIES_V3,
    SERIES_CONSUME_CLAIMS_ACCOUNT_COUNT_V3, SERIES_CONSUME_CLAIMS_OFFSET_V3,
    SERIES_CONSUME_CORE_FOUND_ACCOUNT_BASE_V3, SERIES_CONSUME_CORE_FOUND_OFFSET_V3,
    SERIES_CONSUME_CORE_OPEN_ACCOUNT_COUNT_V3, SERIES_CONSUME_CORE_OPEN_OFFSET_V3,
    SERIES_CONSUME_CORE_REQUEST_BYTES_V3, SERIES_CONSUME_LOCK_ACCOUNT_COUNT_V3,
    SERIES_CONSUME_LOCK_OFFSET_V3, SERIES_CONSUME_MAXIMUM_FUNDING_STATES_V3,
    SERIES_CONSUME_REALIZE_ACCOUNT_COUNT_V3, SERIES_CONSUME_REALIZE_OFFSET_V3,
    SERIES_CONSUME_ROUTE_COUNT_V3, SERIES_CORE_FOUND_RECEIPT_DEPENDENCIES_V3,
    SERIES_CORE_OPEN_RECEIPT_DEPENDENCIES_V3, SERIES_NO_RECEIPT_DEPENDENCIES_V3,
    SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
};
use super::instruction::SERIES_ACTION_HEADER_BYTES_V3;

/// Common scalar containing the exact proof byte width (`proof_count * 32`).
pub const SERIES_CONSUME_PROOF_BYTES_SCALAR_V4: u16 = 1;
/// Common scalar reserved for the exact ordered FundingState count.
///
/// Before Core Found this is only a bounded routing hint.  The live-Market
/// continuation may use it only after the generic resume boundary verifies a
/// current-Core acknowledgment that binds the same count and list commitment.
pub const SERIES_CONSUME_FUNDING_COUNT_SCALAR_V4: u16 = 4;
/// Five family-neutral accounts injected into the logical runtime vector.
pub const SERIES_CONSUME_INJECTED_ACCOUNT_COUNT_V4: u16 = 5;
/// Projected-Market execution window: Lock then Core Found.
pub const SERIES_CONSUME_PREFIX_ROUTE_START_V4: u16 = 0;
/// Exclusive end of the projected-Market execution window.
pub const SERIES_CONSUME_PREFIX_ROUTE_END_V4: u16 = 2;
/// Live-Market continuation starts at Projected Custody realization.
pub const SERIES_CONSUME_CONTINUATION_ROUTE_START_V4: u16 = 2;
/// Exclusive end of the live-Market continuation.
pub const SERIES_CONSUME_CONTINUATION_ROUTE_END_V4: u16 = 5;
/// Projected-Custody Lock-and-close-source global route.
pub const SERIES_CONSUME_LOCK_ROUTE_V4: u16 = 0;
/// Core Found global route.
pub const SERIES_CONSUME_FOUND_ROUTE_V4: u16 = 1;
/// Projected-Custody Realize-and-close global route.
pub const SERIES_CONSUME_REALIZE_ROUTE_V4: u16 = 2;
/// Claims Founding V5 global route.
pub const SERIES_CONSUME_CLAIMS_ROUTE_V4: u16 = 3;
/// Final Core Open global route.
pub const SERIES_CONSUME_OPEN_ROUTE_V4: u16 = 4;

const ROUTE_LOCK: u16 = SERIES_CONSUME_LOCK_ROUTE_V4;
const ROUTE_FOUND: u16 = SERIES_CONSUME_FOUND_ROUTE_V4;
const ROUTE_REALIZE: u16 = SERIES_CONSUME_REALIZE_ROUTE_V4;
const ROUTE_CLAIMS: u16 = SERIES_CONSUME_CLAIMS_ROUTE_V4;
const ROUTE_OPEN: u16 = SERIES_CONSUME_OPEN_ROUTE_V4;

const LOCK_ACCOUNT_START: u16 = SERIES_CONSUME_INJECTED_ACCOUNT_COUNT_V4;
const FOUND_ACCOUNT_START: u16 = LOCK_ACCOUNT_START + SERIES_CONSUME_LOCK_ACCOUNT_COUNT_V3;
const REALIZE_ACCOUNT_START_BEFORE_FUNDING: u16 =
    FOUND_ACCOUNT_START + SERIES_CONSUME_CORE_FOUND_ACCOUNT_BASE_V3;
const CLAIMS_ACCOUNT_START_BEFORE_FUNDING: u16 =
    REALIZE_ACCOUNT_START_BEFORE_FUNDING + SERIES_CONSUME_REALIZE_ACCOUNT_COUNT_V3;
const OPEN_ACCOUNT_START_BEFORE_FUNDING: u16 =
    CLAIMS_ACCOUNT_START_BEFORE_FUNDING + SERIES_CONSUME_CLAIMS_ACCOUNT_COUNT_V3;
/// Logical AccountProfile width before the ordered FundingState span.
///
/// This is not a physical transaction-meta count: authenticated route aliases
/// may compact repeated identities only through the generic AccountProfile
/// successor that owns their privilege subsets.
pub const SERIES_CONSUME_LOGICAL_ACCOUNT_BASE_V4: u16 =
    OPEN_ACCOUNT_START_BEFORE_FUNDING + SERIES_CONSUME_CORE_OPEN_ACCOUNT_COUNT_V3;
const ALLOWED_FUNDING_COUNTS: u64 =
    ((1_u64 << (SERIES_CONSUME_MAXIMUM_FUNDING_STATES_V3 + 1)) - 1) & !1;
const SERIES_ACTION_HEADER_OFFSET_V4: u32 = 128;
const SERIES_CONSUME_COMMON_SCALAR_COUNT_V4: u16 = 5;
const _: () = assert!(SERIES_ACTION_HEADER_BYTES_V3 == 128);

/// Stable refusal from the Series-owned Effect V4 topology join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesConsumeEffectErrorV4 {
    /// Family request was not one exact Consume header and proof.
    Request,
    /// Successor wire, policy, span, or borrowed-range table differed.
    Successor,
    /// Embedded five-route program differed from the canonical topology.
    BaseProgram,
    /// Funding hint or proof-width scalar differed from authenticated bytes.
    Registers,
    /// Caller requested another prefix/continuation route window.
    Window,
}

/// The only two execution phases admitted for the global five-route program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesConsumeRouteWindowV4 {
    /// Projected-Market Lock and Core Found.
    ProjectedPrefix,
    /// Live-Market Realize, Claims founding, and Core Open.
    LiveMarketContinuation,
}

/// Hostile-decoded global Series Consume program plus its bounded hint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesConsumeEffectV4<'a> {
    program: ProgramV4<'a>,
    funding_count_hint: u16,
}

impl<'a> SeriesConsumeEffectV4<'a> {
    /// Admit one exact global five-route Effect V4 artifact.
    ///
    /// `funding_count_hint` is deliberately named as a hint: this function
    /// proves only its bounded agreement with the resolved generic artifact.
    /// It does not promote the value into a protected continuation register.
    pub fn decode(
        bytes: &'a [u8],
        family_request: &[u8],
        tail_count: u32,
        scalars: &[u64],
        identities: &[[u8; 32]],
        funding_count_hint: u16,
    ) -> Result<Self, SeriesConsumeEffectErrorV4> {
        let request = SeriesActionRequestV3::decode(family_request)
            .map_err(|_| SeriesConsumeEffectErrorV4::Request)?;
        if request.action() != SeriesActionV3::Consume {
            return Err(SeriesConsumeEffectErrorV4::Request);
        }
        let proof_bytes = usize::from(request.proof_count())
            .checked_mul(32)
            .ok_or(SeriesConsumeEffectErrorV4::Request)?;
        if proof_bytes != request.proof_bytes().len()
            || scalars
                .get(usize::from(SERIES_CONSUME_PROOF_BYTES_SCALAR_V4))
                .copied()
                != u64::try_from(proof_bytes).ok()
            || scalars
                .get(usize::from(SERIES_CONSUME_FUNDING_COUNT_SCALAR_V4))
                .copied()
                != Some(u64::from(funding_count_hint))
            || funding_count_hint == 0
            || funding_count_hint > SERIES_CONSUME_MAXIMUM_FUNDING_STATES_V3
        {
            return Err(SeriesConsumeEffectErrorV4::Registers);
        }

        let program =
            ProgramV4::decode(bytes).map_err(|_| SeriesConsumeEffectErrorV4::Successor)?;
        validate_successor(program)?;
        validate_base(program.base())?;
        program
            .validate_request_coverage(family_request.len(), tail_count, scalars, identities)
            .map_err(|_| SeriesConsumeEffectErrorV4::Successor)?;
        validate_resolved(program, tail_count, scalars, identities, funding_count_hint)?;
        Ok(Self {
            program,
            funding_count_hint,
        })
    }

    /// Borrow the authenticated generic successor program.
    pub const fn program(self) -> ProgramV4<'a> {
        self.program
    }

    /// Bounded pre-Core routing hint.  This is not an attested scalar.
    pub const fn funding_count_hint(self) -> u16 {
        self.funding_count_hint
    }

    /// Admit only one of the two exact nonoverlapping global route windows.
    pub fn require_window(
        self,
        window: SeriesConsumeRouteWindowV4,
    ) -> Result<(), SeriesConsumeEffectErrorV4> {
        let (start, end) = match window {
            SeriesConsumeRouteWindowV4::ProjectedPrefix => (
                SERIES_CONSUME_PREFIX_ROUTE_START_V4,
                SERIES_CONSUME_PREFIX_ROUTE_END_V4,
            ),
            SeriesConsumeRouteWindowV4::LiveMarketContinuation => (
                SERIES_CONSUME_CONTINUATION_ROUTE_START_V4,
                SERIES_CONSUME_CONTINUATION_ROUTE_END_V4,
            ),
        };
        self.program
            .validate_route_window(start, end)
            .map_err(|_| SeriesConsumeEffectErrorV4::Window)
    }
}

/// Exact logical AccountProfile width for one bounded funding span.
pub const fn series_consume_logical_account_count_v4(funding_count: u16) -> Option<u16> {
    if funding_count == 0 || funding_count > SERIES_CONSUME_MAXIMUM_FUNDING_STATES_V3 {
        None
    } else {
        SERIES_CONSUME_LOGICAL_ACCOUNT_BASE_V4.checked_add(funding_count)
    }
}

/// Exact logical start of one global route after applying the funding span.
pub const fn series_consume_route_account_start_v4(route: u16, funding_count: u16) -> Option<u16> {
    if funding_count == 0 || funding_count > SERIES_CONSUME_MAXIMUM_FUNDING_STATES_V3 {
        return None;
    }
    match route {
        ROUTE_LOCK => Some(LOCK_ACCOUNT_START),
        ROUTE_FOUND => Some(FOUND_ACCOUNT_START),
        ROUTE_REALIZE => REALIZE_ACCOUNT_START_BEFORE_FUNDING.checked_add(funding_count),
        ROUTE_CLAIMS => CLAIMS_ACCOUNT_START_BEFORE_FUNDING.checked_add(funding_count),
        ROUTE_OPEN => OPEN_ACCOUNT_START_BEFORE_FUNDING.checked_add(funding_count),
        _ => None,
    }
}

fn validate_successor(program: ProgramV4<'_>) -> Result<(), SeriesConsumeEffectErrorV4> {
    if program.borrowed_range_policy() != BorrowedRangePolicyV4::IdenticalReuseExactCoverage
        || program.semantic_prefix_bytes()
            != u32::try_from(SERIES_ACTION_HEADER_BYTES_V3)
                .map_err(|_| SeriesConsumeEffectErrorV4::Successor)?
        || program.span_count() != 1
        || program.range_count() != 2
        || program
            .span(0)
            .map_err(|_| SeriesConsumeEffectErrorV4::Successor)?
            != DynamicFixedSpanV4::new(
                ROUTE_FOUND,
                SERIES_CONSUME_FUNDING_COUNT_SCALAR_V4,
                SERIES_CONSUME_CORE_FOUND_ACCOUNT_BASE_V3,
                ALLOWED_FUNDING_COUNTS,
            )
        || program
            .borrowed_range(0)
            .map_err(|_| SeriesConsumeEffectErrorV4::Successor)?
            != proof_range(ROUTE_FOUND)
        || program
            .borrowed_range(1)
            .map_err(|_| SeriesConsumeEffectErrorV4::Successor)?
            != proof_range(ROUTE_OPEN)
    {
        return Err(SeriesConsumeEffectErrorV4::Successor);
    }
    Ok(())
}

const fn proof_range(route: u16) -> BorrowedRangeV4 {
    BorrowedRangeV4::new(
        route,
        RequestCoordinateV4::Fixed(SERIES_ACTION_HEADER_OFFSET_V4),
        RequestCoordinateV4::CommonScalar(SERIES_CONSUME_PROOF_BYTES_SCALAR_V4),
    )
}

fn validate_base(program: ProgramV3<'_>) -> Result<(), SeriesConsumeEffectErrorV4> {
    if program.route_count()
        != u16::try_from(SERIES_CONSUME_ROUTE_COUNT_V3)
            .map_err(|_| SeriesConsumeEffectErrorV4::BaseProgram)?
        || program.fixed_account_count() != SERIES_CONSUME_LOGICAL_ACCOUNT_BASE_V4
        || program.item_account_stride() != 0
        || program.common_scalar_count() != SERIES_CONSUME_COMMON_SCALAR_COUNT_V4
        || program.item_scalar_stride() != 0
        || program.common_identity_count() != 0
        || program.item_identity_stride() != 0
        || program.item_operation_count() != 0
        || SERIES_CONSUME_LOCK_OFFSET_V3 != 0
        || SERIES_CONSUME_CORE_FOUND_OFFSET_V3 != SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3
        || SERIES_CONSUME_REALIZE_OFFSET_V3
            != SERIES_CONSUME_CORE_FOUND_OFFSET_V3 + SERIES_CONSUME_CORE_REQUEST_BYTES_V3
        || SERIES_CONSUME_CLAIMS_OFFSET_V3
            != SERIES_CONSUME_REALIZE_OFFSET_V3 + SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3
        || SERIES_CONSUME_CORE_OPEN_OFFSET_V3
            != SERIES_CONSUME_CLAIMS_OFFSET_V3 + SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3
    {
        return Err(SeriesConsumeEffectErrorV4::BaseProgram);
    }
    for expected in [
        RouteExpectationV4::new(
            ROUTE_LOCK,
            FixedRole::Custody,
            LOCK_ACCOUNT_START,
            SERIES_CONSUME_LOCK_ACCOUNT_COUNT_V3,
            SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
            &SERIES_NO_RECEIPT_DEPENDENCIES_V3,
        ),
        RouteExpectationV4::new(
            ROUTE_FOUND,
            FixedRole::Core,
            FOUND_ACCOUNT_START,
            SERIES_CONSUME_CORE_FOUND_ACCOUNT_BASE_V3,
            SERIES_CONSUME_CORE_REQUEST_BYTES_V3,
            &SERIES_CORE_FOUND_RECEIPT_DEPENDENCIES_V3,
        ),
        RouteExpectationV4::new(
            ROUTE_REALIZE,
            FixedRole::Custody,
            REALIZE_ACCOUNT_START_BEFORE_FUNDING,
            SERIES_CONSUME_REALIZE_ACCOUNT_COUNT_V3,
            SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
            &SERIES_NO_RECEIPT_DEPENDENCIES_V3,
        ),
        RouteExpectationV4::new(
            ROUTE_CLAIMS,
            FixedRole::Claims,
            CLAIMS_ACCOUNT_START_BEFORE_FUNDING,
            SERIES_CONSUME_CLAIMS_ACCOUNT_COUNT_V3,
            SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3,
            &SERIES_CLAIMS_RECEIPT_DEPENDENCIES_V3,
        ),
        RouteExpectationV4::new(
            ROUTE_OPEN,
            FixedRole::Core,
            OPEN_ACCOUNT_START_BEFORE_FUNDING,
            SERIES_CONSUME_CORE_OPEN_ACCOUNT_COUNT_V3,
            SERIES_CONSUME_CORE_REQUEST_BYTES_V3,
            &SERIES_CORE_OPEN_RECEIPT_DEPENDENCIES_V3,
        ),
    ] {
        validate_base_route(program, expected)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct RouteExpectationV4<'a> {
    route: u16,
    role: FixedRole,
    account_start: u16,
    account_count: u16,
    request_len: usize,
    dependencies: &'a [RouteReceiptDependencyV3],
}

impl<'a> RouteExpectationV4<'a> {
    const fn new(
        route: u16,
        role: FixedRole,
        account_start: u16,
        account_count: u16,
        request_len: usize,
        dependencies: &'a [RouteReceiptDependencyV3],
    ) -> Self {
        Self {
            route,
            role,
            account_start,
            account_count,
            request_len,
            dependencies,
        }
    }
}

fn validate_base_route(
    program: ProgramV3<'_>,
    expected: RouteExpectationV4<'_>,
) -> Result<(), SeriesConsumeEffectErrorV4> {
    let route = program
        .route(expected.route)
        .map_err(|_| SeriesConsumeEffectErrorV4::BaseProgram)?;
    if route.role() != expected.role
        || route.kind() != RouteKindV3::Once
        || route.fixed_account_start() != expected.account_start
        || route.fixed_account_count() != expected.account_count
        || usize::try_from(route.fixed_request_bytes()).ok() != Some(expected.request_len)
        || route.item_account_count() != 0
        || route.item_request_bytes() != 0
        || route.borrows_witness()
        || usize::from(route.receipt_dependency_count()) != expected.dependencies.len()
    {
        return Err(SeriesConsumeEffectErrorV4::BaseProgram);
    }
    let zero_scalars = [0_u64; 5];
    let zero_identities: [[u8; 32]; 0] = [];
    let resolved = program
        .resolved_invocation(expected.route, 0, 0, &zero_scalars, &zero_identities)
        .map_err(|_| SeriesConsumeEffectErrorV4::BaseProgram)?;
    if resolved.request_len != expected.request_len {
        return Err(SeriesConsumeEffectErrorV4::BaseProgram);
    }
    let mut index = 0_u16;
    while usize::from(index) < expected.dependencies.len() {
        if program
            .route_receipt_dependency(expected.route, index)
            .map_err(|_| SeriesConsumeEffectErrorV4::BaseProgram)?
            != *expected
                .dependencies
                .get(usize::from(index))
                .ok_or(SeriesConsumeEffectErrorV4::BaseProgram)?
        {
            return Err(SeriesConsumeEffectErrorV4::BaseProgram);
        }
        index = index
            .checked_add(1)
            .ok_or(SeriesConsumeEffectErrorV4::BaseProgram)?;
    }
    Ok(())
}

fn validate_resolved(
    program: ProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    funding_count: u16,
) -> Result<(), SeriesConsumeEffectErrorV4> {
    let expected_total = SERIES_CONSUME_LOGICAL_ACCOUNT_BASE_V4
        .checked_add(funding_count)
        .ok_or(SeriesConsumeEffectErrorV4::Registers)?;
    if program
        .account_count(tail_count, scalars)
        .map_err(|_| SeriesConsumeEffectErrorV4::Registers)?
        != usize::from(expected_total)
    {
        return Err(SeriesConsumeEffectErrorV4::Registers);
    }
    let found = program
        .resolved_invocation(ROUTE_FOUND, 0, tail_count, scalars, identities)
        .map_err(|_| SeriesConsumeEffectErrorV4::Registers)?;
    let realize = program
        .resolved_invocation(ROUTE_REALIZE, 0, tail_count, scalars, identities)
        .map_err(|_| SeriesConsumeEffectErrorV4::Registers)?;
    let claims = program
        .resolved_invocation(ROUTE_CLAIMS, 0, tail_count, scalars, identities)
        .map_err(|_| SeriesConsumeEffectErrorV4::Registers)?;
    let open = program
        .resolved_invocation(ROUTE_OPEN, 0, tail_count, scalars, identities)
        .map_err(|_| SeriesConsumeEffectErrorV4::Registers)?;
    if found.invocation.fixed_account_start != FOUND_ACCOUNT_START
        || found.invocation.fixed_account_count
            != SERIES_CONSUME_CORE_FOUND_ACCOUNT_BASE_V3 + funding_count
        || found.borrowed_range_count() != 1
        || realize.invocation.fixed_account_start
            != REALIZE_ACCOUNT_START_BEFORE_FUNDING + funding_count
        || claims.invocation.fixed_account_start
            != CLAIMS_ACCOUNT_START_BEFORE_FUNDING + funding_count
        || open.invocation.fixed_account_start != OPEN_ACCOUNT_START_BEFORE_FUNDING + funding_count
        || open.borrowed_range_count() != 1
    {
        return Err(SeriesConsumeEffectErrorV4::Registers);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use alloc::{vec, vec::Vec};
    use dclutch_core_contract::ContentId;
    use dclutch_effect_kernel::{
        v3::{
            HEADER_BYTES, RECEIPT_DEPENDENCY_BYTES, ROUTE_BYTES,
            encode::{EffectGeometryV3, RouteInputV3, encode_effect_program_v4_atomic},
        },
        v4::{
            BORROWED_RANGE_BYTES_V4, DYNAMIC_SPAN_BYTES_V4, HEADER_BYTES_V4,
            encode_program_v4_atomic,
        },
    };

    use super::*;
    use crate::series::instruction::encode_series_action_header_v3;

    const SCALARS: usize = 5;

    fn id(byte: u8) -> ContentId {
        ContentId::new([byte; 32]).expect("fixture identity")
    }

    fn request() -> Vec<u8> {
        let header = encode_series_action_header_v3(
            SeriesActionV3::Consume,
            id(1),
            Some(id(2)),
            Some(id(3)),
            4,
            5,
            2,
        )
        .expect("Consume header");
        let mut output = header.to_vec();
        output.extend_from_slice(&[6_u8; 64]);
        output
    }

    fn base_program() -> Vec<u8> {
        let request_bytes = 2 * SERIES_CONSUME_CORE_REQUEST_BYTES_V3
            + 2 * SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3
            + SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3;
        let request_bank = vec![0_u8; request_bytes];
        let specs = [
            (
                FixedRole::Custody,
                LOCK_ACCOUNT_START,
                SERIES_CONSUME_LOCK_ACCOUNT_COUNT_V3,
                SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
                &SERIES_NO_RECEIPT_DEPENDENCIES_V3[..],
            ),
            (
                FixedRole::Core,
                FOUND_ACCOUNT_START,
                SERIES_CONSUME_CORE_FOUND_ACCOUNT_BASE_V3,
                SERIES_CONSUME_CORE_REQUEST_BYTES_V3,
                &SERIES_CORE_FOUND_RECEIPT_DEPENDENCIES_V3[..],
            ),
            (
                FixedRole::Custody,
                REALIZE_ACCOUNT_START_BEFORE_FUNDING,
                SERIES_CONSUME_REALIZE_ACCOUNT_COUNT_V3,
                SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
                &SERIES_NO_RECEIPT_DEPENDENCIES_V3[..],
            ),
            (
                FixedRole::Claims,
                CLAIMS_ACCOUNT_START_BEFORE_FUNDING,
                SERIES_CONSUME_CLAIMS_ACCOUNT_COUNT_V3,
                SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3,
                &SERIES_CLAIMS_RECEIPT_DEPENDENCIES_V3[..],
            ),
            (
                FixedRole::Core,
                OPEN_ACCOUNT_START_BEFORE_FUNDING,
                SERIES_CONSUME_CORE_OPEN_ACCOUNT_COUNT_V3,
                SERIES_CONSUME_CORE_REQUEST_BYTES_V3,
                &SERIES_CORE_OPEN_RECEIPT_DEPENDENCIES_V3[..],
            ),
        ];
        let mut routes = Vec::new();
        let mut dependencies = Vec::new();
        let mut cursor = 0_usize;
        for (role, account_start, account_count, width, deps) in specs {
            let end = cursor + width;
            routes.push(RouteInputV3 {
                role,
                kind: RouteKindV3::Once,
                enable_common_scalar: None,
                witness_range_common_scalar: None,
                receipt_dependency: None,
                fixed_account_start: account_start,
                fixed_account_count: account_count,
                item_account_start: 0,
                item_account_count: 0,
                fixed_request: request_bank.get(cursor..end).expect("request region"),
                item_request: &[],
            });
            dependencies.push(deps);
            cursor = end;
        }
        let dependency_count = dependencies.iter().map(|items| items.len()).sum::<usize>();
        let width = HEADER_BYTES
            + routes.len() * ROUTE_BYTES
            + dependency_count * RECEIPT_DEPENDENCY_BYTES
            + request_bytes;
        let mut scratch = vec![0_u8; width];
        let mut output = vec![0_u8; width];
        encode_effect_program_v4_atomic(
            EffectGeometryV3 {
                fixed_accounts: SERIES_CONSUME_LOGICAL_ACCOUNT_BASE_V4,
                item_account_stride: 0,
                common_scalars: SCALARS as u16,
                item_scalar_stride: 0,
                common_identities: 0,
                item_identity_stride: 0,
            },
            &routes,
            &dependencies,
            &[],
            &[],
            &mut scratch,
            &mut output,
        )
        .expect("base program");
        output
    }

    fn successor() -> Vec<u8> {
        let base = base_program();
        let spans = [DynamicFixedSpanV4::new(
            ROUTE_FOUND,
            SERIES_CONSUME_FUNDING_COUNT_SCALAR_V4,
            SERIES_CONSUME_CORE_FOUND_ACCOUNT_BASE_V3,
            ALLOWED_FUNDING_COUNTS,
        )];
        let ranges = [proof_range(ROUTE_FOUND), proof_range(ROUTE_OPEN)];
        let width = HEADER_BYTES_V4
            + DYNAMIC_SPAN_BYTES_V4 * spans.len()
            + BORROWED_RANGE_BYTES_V4 * ranges.len()
            + base.len();
        let mut scratch = vec![0_u8; width];
        let mut output = vec![0_u8; width];
        encode_program_v4_atomic(
            &base,
            BorrowedRangePolicyV4::IdenticalReuseExactCoverage,
            SERIES_ACTION_HEADER_OFFSET_V4,
            &spans,
            &ranges,
            &mut scratch,
            &mut output,
        )
        .expect("successor program");
        output
    }

    fn registers(funding_count: u16) -> [u64; SCALARS] {
        [128, 64, 2, 32, u64::from(funding_count)]
    }

    #[test]
    fn global_program_has_only_the_two_exact_nonoverlapping_windows() {
        let bytes = successor();
        let request = request();
        let scalars = registers(7);
        let admitted = SeriesConsumeEffectV4::decode(&bytes, &request, 0, &scalars, &[], 7)
            .expect("global Series program");
        assert_eq!(admitted.program().account_count(0, &scalars), Ok(164));
        assert_eq!(
            admitted.require_window(SeriesConsumeRouteWindowV4::ProjectedPrefix),
            Ok(())
        );
        assert_eq!(
            admitted.require_window(SeriesConsumeRouteWindowV4::LiveMarketContinuation),
            Ok(())
        );
        assert_eq!(series_consume_logical_account_count_v4(7), Some(164));
        assert_eq!(
            series_consume_route_account_start_v4(ROUTE_LOCK, 7),
            Some(5)
        );
        assert_eq!(
            series_consume_route_account_start_v4(ROUTE_FOUND, 7),
            Some(19)
        );
        assert_eq!(
            series_consume_route_account_start_v4(ROUTE_REALIZE, 7),
            Some(83)
        );
        assert_eq!(
            series_consume_route_account_start_v4(ROUTE_CLAIMS, 7),
            Some(95)
        );
        assert_eq!(
            series_consume_route_account_start_v4(ROUTE_OPEN, 7),
            Some(127)
        );
        assert_eq!(
            SERIES_CONSUME_PREFIX_ROUTE_END_V4,
            SERIES_CONSUME_CONTINUATION_ROUTE_START_V4
        );
    }

    #[test]
    fn funding_hint_and_duplicate_proof_ranges_are_exact() {
        let bytes = successor();
        let request = request();
        for count in [1_u16, 16] {
            let scalars = registers(count);
            let admitted = SeriesConsumeEffectV4::decode(&bytes, &request, 0, &scalars, &[], count)
                .expect("bounded funding count");
            for route in [ROUTE_FOUND, ROUTE_OPEN] {
                let range = admitted
                    .program()
                    .resolved_borrowed_range(route, 0, &scalars)
                    .expect("proof range");
                assert_eq!(range.slice(&request), Ok(request[128..].as_ref()));
            }
        }
        for count in [0_u16, 17] {
            let scalars = registers(count);
            assert_eq!(
                SeriesConsumeEffectV4::decode(&bytes, &request, 0, &scalars, &[], count),
                Err(SeriesConsumeEffectErrorV4::Registers)
            );
        }
        let scalars = registers(3);
        assert_eq!(
            SeriesConsumeEffectV4::decode(&bytes, &request, 0, &scalars, &[], 4),
            Err(SeriesConsumeEffectErrorV4::Registers)
        );
    }

    #[test]
    fn proof_padding_and_range_substitution_refuse() {
        let bytes = successor();
        let mut padded_request = request();
        let scalars = registers(2);
        padded_request.push(0);
        assert!(
            SeriesConsumeEffectV4::decode(&bytes, &padded_request, 0, &scalars, &[], 2).is_err()
        );

        let mut hostile = bytes;
        // First borrowed-range route is encoded after header + one span.
        let route = HEADER_BYTES_V4 + DYNAMIC_SPAN_BYTES_V4;
        hostile
            .get_mut(route..route + 2)
            .expect("range route")
            .copy_from_slice(&ROUTE_REALIZE.to_le_bytes());
        assert!(SeriesConsumeEffectV4::decode(&hostile, &request(), 0, &scalars, &[], 2,).is_err());
    }
}
