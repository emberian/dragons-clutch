//! Narrow Series execution staging behind the common Hot V3 outer.
//!
//! This module performs no CPI, account-frame selection, signing, receipt
//! validation, or state write. It validates the five already-resolved generic
//! EffectProgram routes, joins them to the Series semantic composition, and
//! returns the only permitted projected Lock→Core Found→realize→Claims→Core Open
//! order plus the still-uncommitted replay candidate. The common outer remains
//! sole authority for physical accounts, current role programs, receipts, and
//! commit order.

use dclutch_capability_program_contract::hot_v3::HOT_FAMILY_REQUEST_OFFSET_V3;
use dclutch_claims_svm::founding_v5::ClaimsFoundingRequestV5;
use dclutch_custody_contract::{ProjectedCustodyOperationV1, ProjectedCustodyRequestV1};
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{ResolvedInvocationV3, RouteKindV3, RouteReceiptDependencyV3},
};
use dclutch_series_v3_kernel::{
    AccountKeyV3, AuthenticatedProductProjectionV2, composition::SeriesConsumeCompositionV3,
    plan::SeriesReplayWitnessV3,
};
use solana_program::{
    hash::{hash, hashv},
    pubkey::Pubkey,
};

use super::{
    artifacts_v3::{
        SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3, SERIES_CLAIMS_RECEIPT_DEPENDENCIES_V3,
        SERIES_CONSUME_CLAIMS_OFFSET_V3, SERIES_CONSUME_CORE_FOUND_OFFSET_V3,
        SERIES_CONSUME_CORE_OPEN_OFFSET_V3, SERIES_CONSUME_CORE_REQUEST_BYTES_V3,
        SERIES_CONSUME_IR_REQUEST_BYTES_V3, SERIES_CONSUME_LOCK_OFFSET_V3,
        SERIES_CONSUME_REALIZE_OFFSET_V3, SERIES_CONSUME_ROUTE_COUNT_V3,
        SERIES_CORE_FOUND_RECEIPT_DEPENDENCIES_V3, SERIES_CORE_OPEN_RECEIPT_DEPENDENCIES_V3,
        SERIES_NO_RECEIPT_DEPENDENCIES_V3, SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
        SeriesArtifactBundleV3, resolved_dependencies_match,
    },
    instruction::SERIES_ACTION_HEADER_BYTES_V3,
    projected_custody_v3::{
        SeriesProjectedCustodyErrorV3, SeriesProjectedCustodyPhysicalV3,
        project_lock_and_close_source_v3, project_realize_and_close_v3,
    },
    projector::{AuthenticatedSeriesActionV3, SeriesProjectorErrorV3},
};

/// Stable refusal from Series Hot V3 call staging.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesExecuteErrorV3 {
    /// Hot request digest or the fixed family-request boundary differed.
    HotContext,
    /// A resolved route had another role/order/request geometry.
    Invocation,
    /// Projected child request bytes differed from the Series semantic plan.
    RequestBank,
    /// Content, replay, schedule, or Core composition refused.
    Composition(SeriesProjectorErrorV3),
    /// Exact projected-Custody request construction refused.
    ProjectedCustody(SeriesProjectedCustodyErrorV3),
}

impl From<SeriesProjectorErrorV3> for SeriesExecuteErrorV3 {
    fn from(value: SeriesProjectorErrorV3) -> Self {
        Self::Composition(value)
    }
}

impl From<SeriesProjectedCustodyErrorV3> for SeriesExecuteErrorV3 {
    fn from(value: SeriesProjectedCustodyErrorV3) -> Self {
        Self::ProjectedCustody(value)
    }
}

/// Result alias for exact Series execution staging.
pub type Result<T> = core::result::Result<T, SeriesExecuteErrorV3>;

#[derive(Clone, Copy)]
struct SeriesInvocationExpectationV3<'a> {
    role: FixedRole,
    request_offset: usize,
    request_len: usize,
    borrows_witness: bool,
    receipt_dependencies: &'a [RouteReceiptDependencyV3],
}

/// Common-outer observations required by the Series stage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesHotContextV3<'a> {
    family_request: &'a [u8],
    request_digest: [u8; 32],
    ir_request_bank: &'a [u8],
}

impl<'a> SeriesHotContextV3<'a> {
    /// Bind the exact family request and projected IR request bank.
    pub fn new(
        family_request: &'a [u8],
        request_digest: [u8; 32],
        ir_request_bank: &'a [u8],
    ) -> Result<Self> {
        if HOT_FAMILY_REQUEST_OFFSET_V3 != 128
            || SERIES_ACTION_HEADER_BYTES_V3 != 128
            || request_digest == [0; 32]
            || hash(family_request).to_bytes() != request_digest
            || ir_request_bank.len() != SERIES_CONSUME_IR_REQUEST_BYTES_V3
        {
            return Err(SeriesExecuteErrorV3::HotContext);
        }
        Ok(Self {
            family_request,
            request_digest,
            ir_request_bank,
        })
    }

    /// Exact complete Series family request authenticated by Hot V3.
    pub const fn family_request(self) -> &'a [u8] {
        self.family_request
    }

    /// SHA-256 of the exact family request.
    pub const fn request_digest(self) -> [u8; 32] {
        self.request_digest
    }

    /// Exact flat request bank projected by the selected EffectProgram.
    pub const fn ir_request_bank(self) -> &'a [u8] {
        self.ir_request_bank
    }
}

/// One ordered child call with route-resolved geometry but no physical account authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesStagedChildCallV3<'a> {
    invocation: ResolvedInvocationV3,
    request: &'a [u8],
    witness: Option<&'a [u8]>,
    base_request_digest: [u8; 32],
}

impl<'a> SeriesStagedChildCallV3<'a> {
    /// Generic role/account/request geometry resolved by EffectProgram V3.
    pub const fn invocation(self) -> ResolvedInvocationV3 {
        self.invocation
    }

    /// Exact IR-owned child request bytes.
    pub const fn request(self) -> &'a [u8] {
        self.request
    }

    /// Exact borrowed proof suffix; present only for the two Core calls.
    pub const fn witness(self) -> Option<&'a [u8]> {
        self.witness
    }

    /// SHA-256 of the IR request plus borrowed witness, before a typed prior
    /// receipt is appended by the common role executor.
    pub const fn base_request_digest(self) -> [u8; 32] {
        self.base_request_digest
    }
}

/// Five calls plus a replay candidate that remains uncommitted until receipts accept.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesConsumeExecutionPlanV3<'a> {
    composition: SeriesConsumeCompositionV3,
    calls: [SeriesStagedChildCallV3<'a>; SERIES_CONSUME_ROUTE_COUNT_V3],
}

impl<'a> SeriesConsumeExecutionPlanV3<'a> {
    /// Ordered projected Lock, Core Found, Realize, Claims, and Core Open calls.
    pub const fn calls(self) -> [SeriesStagedChildCallV3<'a>; SERIES_CONSUME_ROUTE_COUNT_V3] {
        self.calls
    }

    /// Semantic replay candidate; this function grants no authority to write it.
    pub const fn replay_candidate(self) -> SeriesReplayWitnessV3 {
        self.composition.replay()
    }

    /// Full joined semantic plan used for later receipt validation and commit.
    pub const fn composition(self) -> SeriesConsumeCompositionV3 {
        self.composition
    }
}

/// Authenticated semantic and physical observations for one Consume stage.
pub struct SeriesConsumeExecutionInputsV3<'a, 'content> {
    /// Finalized Template/Occurrence/Ticket projection.
    pub action: AuthenticatedSeriesActionV3<'content>,
    /// Complete action-selected generic artifact join.
    pub artifacts: SeriesArtifactBundleV3<'content>,
    /// Exact five post-strategy route resolutions in program order.
    pub invocations: [ResolvedInvocationV3; SERIES_CONSUME_ROUTE_COUNT_V3],
    /// Common Hot V3 request and request-bank observations.
    pub hot: SeriesHotContextV3<'a>,
    /// Independently authenticated Product Runtime V2 projection.
    pub product: AuthenticatedProductProjectionV2,
    /// Current Registry program identity.
    pub registry_program: AccountKeyV3,
    /// Trading-owned Ticket replay account.
    pub ticket_state_key: Pubkey,
    /// Exact prestate Series root tail.
    pub series_bytes: &'a [u8],
    /// Exact prestate Ticket replay bytes.
    pub ticket_state_bytes: &'a [u8],
    /// Current Clock slot.
    pub now_slot: u64,
    /// Adapter-authenticated projected-Custody wire observations.
    pub projected_custody: SeriesProjectedCustodyPhysicalV3,
}

/// Stage one complete Consume without invoking a program or writing state.
pub fn stage_series_consume_execution_v3<'a>(
    inputs: &SeriesConsumeExecutionInputsV3<'a, 'a>,
) -> Result<SeriesConsumeExecutionPlanV3<'a>> {
    let composition = compose_execution_v3(inputs)?;
    let expiry_slot = occurrence_expiry_slot_v3(inputs)?;
    let calls = stage_consume_calls_v3(inputs, &composition, expiry_slot)?;
    Ok(SeriesConsumeExecutionPlanV3 { composition, calls })
}

#[inline(never)]
fn stage_consume_calls_v3<'a>(
    inputs: &SeriesConsumeExecutionInputsV3<'a, 'a>,
    composition: &SeriesConsumeCompositionV3,
    expiry_slot: u64,
) -> Result<[SeriesStagedChildCallV3<'a>; SERIES_CONSUME_ROUTE_COUNT_V3]> {
    let lock = stage_child_call_v3(
        inputs,
        0,
        SeriesInvocationExpectationV3 {
            role: FixedRole::Custody,
            request_offset: SERIES_CONSUME_LOCK_OFFSET_V3,
            request_len: SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
            borrows_witness: false,
            receipt_dependencies: &SERIES_NO_RECEIPT_DEPENDENCIES_V3,
        },
    )?;
    validate_projected_lock_v3(
        composition,
        expiry_slot,
        inputs.projected_custody,
        lock.request(),
    )?;

    let found = stage_child_call_v3(
        inputs,
        1,
        SeriesInvocationExpectationV3 {
            role: FixedRole::Core,
            request_offset: SERIES_CONSUME_CORE_FOUND_OFFSET_V3,
            request_len: SERIES_CONSUME_CORE_REQUEST_BYTES_V3,
            borrows_witness: true,
            receipt_dependencies: &SERIES_CORE_FOUND_RECEIPT_DEPENDENCIES_V3,
        },
    )?;
    validate_core_request_v3(composition, found.request())?;

    let realize = stage_child_call_v3(
        inputs,
        2,
        SeriesInvocationExpectationV3 {
            role: FixedRole::Custody,
            request_offset: SERIES_CONSUME_REALIZE_OFFSET_V3,
            request_len: SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
            borrows_witness: false,
            receipt_dependencies: &SERIES_NO_RECEIPT_DEPENDENCIES_V3,
        },
    )?;
    validate_projected_realize_v3(
        composition,
        expiry_slot,
        inputs.projected_custody,
        realize.request(),
    )?;

    let claims = stage_child_call_v3(
        inputs,
        3,
        SeriesInvocationExpectationV3 {
            role: FixedRole::Claims,
            request_offset: SERIES_CONSUME_CLAIMS_OFFSET_V3,
            request_len: SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3,
            borrows_witness: false,
            receipt_dependencies: &SERIES_CLAIMS_RECEIPT_DEPENDENCIES_V3,
        },
    )?;
    validate_claims_request_v3(claims.request())?;

    let open = stage_child_call_v3(
        inputs,
        4,
        SeriesInvocationExpectationV3 {
            role: FixedRole::Core,
            request_offset: SERIES_CONSUME_CORE_OPEN_OFFSET_V3,
            request_len: SERIES_CONSUME_CORE_REQUEST_BYTES_V3,
            borrows_witness: true,
            receipt_dependencies: &SERIES_CORE_OPEN_RECEIPT_DEPENDENCIES_V3,
        },
    )?;
    validate_core_request_v3(composition, open.request())?;
    Ok([lock, found, realize, claims, open])
}

#[inline(never)]
fn compose_execution_v3(
    inputs: &SeriesConsumeExecutionInputsV3<'_, '_>,
) -> Result<SeriesConsumeCompositionV3> {
    inputs
        .action
        .compose_consume(
            inputs.product,
            inputs.registry_program,
            inputs.ticket_state_key,
            inputs.series_bytes,
            inputs.ticket_state_bytes,
            inputs.now_slot,
        )
        .map_err(SeriesExecuteErrorV3::from)
}

#[inline(never)]
fn occurrence_expiry_slot_v3(inputs: &SeriesConsumeExecutionInputsV3<'_, '_>) -> Result<u64> {
    let occurrence = inputs
        .action
        .occurrence()
        .ok_or(SeriesExecuteErrorV3::Invocation)?
        .occurrence()
        .occurrence();
    inputs
        .action
        .template()
        .retry_through(occurrence)
        .map_err(|_| SeriesExecuteErrorV3::Invocation)
}

#[inline(never)]
fn validate_core_request_v3(
    composition: &SeriesConsumeCompositionV3,
    request: &[u8],
) -> Result<()> {
    let expected = composition
        .core_request()
        .encode()
        .map_err(|_| SeriesExecuteErrorV3::RequestBank)?;
    if request == expected {
        Ok(())
    } else {
        Err(SeriesExecuteErrorV3::RequestBank)
    }
}

#[inline(never)]
fn validate_projected_lock_v3(
    composition: &SeriesConsumeCompositionV3,
    expiry_slot: u64,
    physical: SeriesProjectedCustodyPhysicalV3,
    request: &[u8],
) -> Result<()> {
    let decoded = ProjectedCustodyRequestV1::decode(request)
        .map_err(|_| SeriesExecuteErrorV3::RequestBank)?;
    if decoded.operation != ProjectedCustodyOperationV1::LockHoardAndCloseSource {
        return Err(SeriesExecuteErrorV3::RequestBank);
    }
    let expected = project_lock_and_close_source_v3(composition.escrow(), expiry_slot, physical)?
        .encode()
        .map_err(|_| SeriesExecuteErrorV3::RequestBank)?;
    if request == expected {
        Ok(())
    } else {
        Err(SeriesExecuteErrorV3::RequestBank)
    }
}

#[inline(never)]
fn validate_projected_realize_v3(
    composition: &SeriesConsumeCompositionV3,
    expiry_slot: u64,
    physical: SeriesProjectedCustodyPhysicalV3,
    request: &[u8],
) -> Result<()> {
    let decoded = ProjectedCustodyRequestV1::decode(request)
        .map_err(|_| SeriesExecuteErrorV3::RequestBank)?;
    if decoded.operation != ProjectedCustodyOperationV1::RealizeAndClose {
        return Err(SeriesExecuteErrorV3::RequestBank);
    }
    let expected = project_realize_and_close_v3(composition.escrow(), expiry_slot, physical)?
        .encode()
        .map_err(|_| SeriesExecuteErrorV3::RequestBank)?;
    if request == expected {
        Ok(())
    } else {
        Err(SeriesExecuteErrorV3::RequestBank)
    }
}

#[inline(never)]
fn validate_claims_request_v3(request: &[u8]) -> Result<()> {
    ClaimsFoundingRequestV5::decode(request)
        .map(|_| ())
        .map_err(|_| SeriesExecuteErrorV3::RequestBank)
}

#[inline(never)]
fn stage_child_call_v3<'a>(
    inputs: &SeriesConsumeExecutionInputsV3<'a, 'a>,
    index: usize,
    expected: SeriesInvocationExpectationV3<'_>,
) -> Result<SeriesStagedChildCallV3<'a>> {
    let invocation = inputs
        .invocations
        .get(index)
        .copied()
        .ok_or(SeriesExecuteErrorV3::Invocation)?;
    let request = validate_invocation(
        invocation,
        inputs.artifacts.effect,
        expected,
        inputs.hot.ir_request_bank(),
    )?;
    let witness = expected
        .borrows_witness
        .then_some(inputs.artifacts.slices.witness);
    let base_request_digest = match witness {
        Some(witness) => hashv(&[request, witness]).to_bytes(),
        None => hash(request).to_bytes(),
    };
    Ok(SeriesStagedChildCallV3 {
        invocation,
        request,
        witness,
        base_request_digest,
    })
}

fn validate_invocation<'a>(
    invocation: ResolvedInvocationV3,
    effect: dclutch_effect_kernel::v3::ProgramV3<'_>,
    expected: SeriesInvocationExpectationV3<'_>,
    request_bank: &'a [u8],
) -> Result<&'a [u8]> {
    if invocation.role != expected.role
        || invocation.kind != RouteKindV3::Once
        || invocation.item.is_some()
        || invocation.repeated_item_count != 0
        || invocation.request_offset != expected.request_offset
        || invocation.request_len != expected.request_len
        || invocation.fixed_account_count == 0
        || invocation.borrowed_witness.is_some() != expected.borrows_witness
        || !resolved_dependencies_match(effect, invocation, expected.receipt_dependencies)
    {
        return Err(SeriesExecuteErrorV3::Invocation);
    }
    let end = expected
        .request_offset
        .checked_add(expected.request_len)
        .ok_or(SeriesExecuteErrorV3::Invocation)?;
    request_bank
        .get(expected.request_offset..end)
        .ok_or(SeriesExecuteErrorV3::Invocation)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::{boxed::Box, vec};

    use super::*;

    fn empty_effect_program() -> dclutch_effect_kernel::v3::ProgramV3<'static> {
        let mut bytes = vec![0_u8; dclutch_effect_kernel::v3::HEADER_BYTES];
        bytes
            .get_mut(..4)
            .expect("magic")
            .copy_from_slice(&dclutch_effect_kernel::v3::MAGIC);
        *bytes.get_mut(4).expect("version") = dclutch_effect_kernel::v3::VERSION;
        bytes
            .get_mut(12..14)
            .expect("fixed accounts")
            .copy_from_slice(&1_u16.to_le_bytes());
        bytes
            .get_mut(16..18)
            .expect("common scalars")
            .copy_from_slice(&1_u16.to_le_bytes());
        dclutch_effect_kernel::v3::ProgramV3::decode(Box::leak(bytes.into_boxed_slice()))
            .expect("empty route program")
    }

    fn invocation(role: FixedRole, offset: usize, len: usize) -> ResolvedInvocationV3 {
        ResolvedInvocationV3 {
            role,
            kind: RouteKindV3::Once,
            item: None,
            fixed_account_start: 0,
            fixed_account_count: 1,
            item_account_start: 0,
            item_account_count: 0,
            item_account_stride: 0,
            repeated_item_count: 0,
            request_offset: offset,
            request_len: len,
            borrowed_witness: None,
            receipt_dependencies: dclutch_effect_kernel::v3::ResolvedReceiptDependenciesV3::empty(),
            receipt_dependency: None,
        }
    }

    #[test]
    fn hot_context_refuses_digest_substitution_and_wrong_bank_width() {
        let request = [7_u8; 128];
        let bank = [0_u8; SERIES_CONSUME_IR_REQUEST_BYTES_V3];
        let digest = hash(&request).to_bytes();
        assert!(SeriesHotContextV3::new(&request, digest, &bank).is_ok());
        let mut wrong = digest;
        *wrong.get_mut(0).expect("digest byte") ^= 1;
        assert_eq!(
            SeriesHotContextV3::new(&request, wrong, &bank),
            Err(SeriesExecuteErrorV3::HotContext)
        );
        assert_eq!(
            SeriesHotContextV3::new(
                &request,
                digest,
                bank.get(..bank.len() - 1).expect("short bank"),
            ),
            Err(SeriesExecuteErrorV3::HotContext)
        );
    }

    #[test]
    fn consume_route_offsets_are_global_and_gap_free() {
        let bank = [0_u8; SERIES_CONSUME_IR_REQUEST_BYTES_V3];
        for (role, offset, width) in [
            (
                FixedRole::Custody,
                SERIES_CONSUME_LOCK_OFFSET_V3,
                SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
            ),
            (
                FixedRole::Custody,
                SERIES_CONSUME_REALIZE_OFFSET_V3,
                SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
            ),
            (
                FixedRole::Claims,
                SERIES_CONSUME_CLAIMS_OFFSET_V3,
                SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3,
            ),
        ] {
            let route = invocation(role, offset, width);
            let expected = SeriesInvocationExpectationV3 {
                role,
                request_offset: offset,
                request_len: width,
                borrows_witness: false,
                receipt_dependencies: &SERIES_NO_RECEIPT_DEPENDENCIES_V3,
            };
            assert_eq!(
                validate_invocation(route, empty_effect_program(), expected, &bank)
                    .expect("fixed child request")
                    .len(),
                width
            );
        }
        let wrong = invocation(
            FixedRole::Custody,
            SERIES_CONSUME_LOCK_OFFSET_V3 + 1,
            SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
        );
        assert_eq!(
            validate_invocation(
                wrong,
                empty_effect_program(),
                SeriesInvocationExpectationV3 {
                    role: FixedRole::Custody,
                    request_offset: SERIES_CONSUME_LOCK_OFFSET_V3,
                    request_len: SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
                    borrows_witness: false,
                    receipt_dependencies: &SERIES_NO_RECEIPT_DEPENDENCIES_V3,
                },
                &bank,
            ),
            Err(SeriesExecuteErrorV3::Invocation)
        );
    }
}
