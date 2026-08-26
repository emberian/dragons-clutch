//! Narrow Series execution staging behind the common Hot V3 outer.
//!
//! This module performs no CPI, account-frame selection, signing, receipt
//! validation, or state write. It validates the four already-resolved generic
//! EffectProgram routes, joins them to the Series semantic composition, and
//! returns the only permitted Core→Custody→Custody→Custody call order plus the
//! still-uncommitted replay candidate. The common outer remains sole authority
//! for physical accounts, current role programs, receipts, and commit order.

use dclutch_capability_program_contract::hot_v3::HOT_FAMILY_REQUEST_OFFSET_V3;
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{ResolvedInvocationV3, RouteKindV3},
};
use dclutch_series_v3_kernel::{
    AccountKeyV3, AuthenticatedProductProjectionV2, plan::SeriesReplayWitnessV3,
};
use solana_program::{hash::hash, pubkey::Pubkey};

use super::{
    artifacts_v3::{
        SERIES_CONSUME_CORE_REQUEST_BYTES_V3, SERIES_CONSUME_IR_REQUEST_BYTES_V3,
        SERIES_CONSUME_ROUTE_COUNT_V3, SERIES_CUSTODY_REQUEST_BYTES_V3, SeriesArtifactBundleV3,
    },
    composer_v3::{
        SeriesConsumePhysicalPlanV3, SeriesPhysicalComposerErrorV3, compose_consume_physical_v3,
    },
    custody_v3::SeriesCustodyPhysicalV3,
    instruction::SERIES_ACTION_HEADER_BYTES_V3,
    projector::AuthenticatedSeriesActionV3,
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
    /// Content, replay, schedule, Core, or Custody composition refused.
    Composition(SeriesPhysicalComposerErrorV3),
}

impl From<SeriesPhysicalComposerErrorV3> for SeriesExecuteErrorV3 {
    fn from(value: SeriesPhysicalComposerErrorV3) -> Self {
        Self::Composition(value)
    }
}

/// Result alias for exact Series execution staging.
pub type Result<T> = core::result::Result<T, SeriesExecuteErrorV3>;

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
    request_digest: [u8; 32],
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

    /// Exact borrowed proof suffix; present only for the first Core call.
    pub const fn witness(self) -> Option<&'a [u8]> {
        self.witness
    }

    /// SHA-256 of `request || witness` for this exact execution.
    pub const fn request_digest(self) -> [u8; 32] {
        self.request_digest
    }
}

/// Four calls plus a replay candidate that remains uncommitted until receipts accept.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesConsumeExecutionPlanV3<'a> {
    physical: SeriesConsumePhysicalPlanV3<'a>,
    calls: [SeriesStagedChildCallV3<'a>; SERIES_CONSUME_ROUTE_COUNT_V3],
}

impl<'a> SeriesConsumeExecutionPlanV3<'a> {
    /// Ordered Core, transfer-to-Hoard, close-Vault, close-replay calls.
    pub const fn calls(self) -> [SeriesStagedChildCallV3<'a>; SERIES_CONSUME_ROUTE_COUNT_V3] {
        self.calls
    }

    /// Semantic replay candidate; this function grants no authority to write it.
    pub const fn replay_candidate(self) -> SeriesReplayWitnessV3 {
        self.physical.composition().replay()
    }

    /// Full joined semantic plan used for later receipt validation and commit.
    pub const fn physical(self) -> SeriesConsumePhysicalPlanV3<'a> {
        self.physical
    }
}

/// Authenticated semantic and physical observations for one Consume stage.
pub struct SeriesConsumeExecutionInputsV3<'a, 'content> {
    /// Finalized Template/Occurrence/Ticket projection.
    pub action: AuthenticatedSeriesActionV3<'content>,
    /// Complete action-selected generic artifact join.
    pub artifacts: SeriesArtifactBundleV3<'content>,
    /// Exact four post-strategy route resolutions in program order.
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
    /// Adapter-authenticated Custody wire observations.
    pub custody_physical: SeriesCustodyPhysicalV3,
}

/// Stage one complete Consume without invoking a program or writing state.
pub fn stage_series_consume_execution_v3<'a>(
    inputs: SeriesConsumeExecutionInputsV3<'a, '_>,
) -> Result<SeriesConsumeExecutionPlanV3<'a>> {
    if inputs.custody_physical.parent_request_digest != inputs.hot.request_digest() {
        return Err(SeriesExecuteErrorV3::HotContext);
    }
    let physical = compose_consume_physical_v3(
        inputs.action,
        inputs.artifacts,
        *inputs
            .invocations
            .first()
            .ok_or(SeriesExecuteErrorV3::Invocation)?,
        inputs.hot.ir_request_bank(),
        inputs.hot.family_request(),
        inputs.product,
        inputs.registry_program,
        inputs.ticket_state_key,
        inputs.series_bytes,
        inputs.ticket_state_bytes,
        inputs.now_slot,
        inputs.custody_physical,
    )?;

    let custody_requests = physical.custody();
    let mut calls: [Option<SeriesStagedChildCallV3<'a>>; SERIES_CONSUME_ROUTE_COUNT_V3] =
        [None; SERIES_CONSUME_ROUTE_COUNT_V3];
    for (index, invocation) in inputs.invocations.into_iter().enumerate() {
        let (role, offset, width, witness) = if index == 0 {
            (
                FixedRole::Core,
                0,
                SERIES_CONSUME_CORE_REQUEST_BYTES_V3,
                Some(physical.core().witness),
            )
        } else {
            let custody_index = index
                .checked_sub(1)
                .ok_or(SeriesExecuteErrorV3::Invocation)?;
            let offset = SERIES_CONSUME_CORE_REQUEST_BYTES_V3
                .checked_add(
                    custody_index
                        .checked_mul(SERIES_CUSTODY_REQUEST_BYTES_V3)
                        .ok_or(SeriesExecuteErrorV3::Invocation)?,
                )
                .ok_or(SeriesExecuteErrorV3::Invocation)?;
            (
                FixedRole::Custody,
                offset,
                SERIES_CUSTODY_REQUEST_BYTES_V3,
                None,
            )
        };
        let request = validate_invocation(
            invocation,
            role,
            offset,
            width,
            witness.is_some(),
            inputs.hot.ir_request_bank(),
        )?;
        if index == 0 {
            if request != physical.core().core_request {
                return Err(SeriesExecuteErrorV3::RequestBank);
            }
        } else {
            let custody = *custody_requests
                .get(index - 1)
                .ok_or(SeriesExecuteErrorV3::RequestBank)?;
            if custody
                .to_bytes()
                .map_err(|_| SeriesExecuteErrorV3::RequestBank)?
                .as_slice()
                != request
            {
                return Err(SeriesExecuteErrorV3::RequestBank);
            }
        }
        let request_digest = match witness {
            Some(_) => physical.core().child_request_digest,
            None => hash(request).to_bytes(),
        };
        *calls
            .get_mut(index)
            .ok_or(SeriesExecuteErrorV3::Invocation)? = Some(SeriesStagedChildCallV3 {
            invocation,
            request,
            witness,
            request_digest,
        });
    }
    let [
        Some(core),
        Some(transfer),
        Some(close_vault),
        Some(close_replay),
    ] = calls
    else {
        return Err(SeriesExecuteErrorV3::Invocation);
    };
    let calls = [core, transfer, close_vault, close_replay];
    Ok(SeriesConsumeExecutionPlanV3 { physical, calls })
}

fn validate_invocation(
    invocation: ResolvedInvocationV3,
    role: FixedRole,
    request_offset: usize,
    request_len: usize,
    borrows_witness: bool,
    request_bank: &[u8],
) -> Result<&[u8]> {
    if invocation.role != role
        || invocation.kind != RouteKindV3::Once
        || invocation.item.is_some()
        || invocation.repeated_item_count != 0
        || invocation.request_offset != request_offset
        || invocation.request_len != request_len
        || invocation.fixed_account_count == 0
        || invocation.borrowed_witness.is_some() != borrows_witness
    {
        return Err(SeriesExecuteErrorV3::Invocation);
    }
    let end = request_offset
        .checked_add(request_len)
        .ok_or(SeriesExecuteErrorV3::Invocation)?;
    request_bank
        .get(request_offset..end)
        .ok_or(SeriesExecuteErrorV3::Invocation)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

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
    fn custody_route_offsets_are_global_and_gap_free() {
        let bank = [0_u8; SERIES_CONSUME_IR_REQUEST_BYTES_V3];
        for index in 0..3 {
            let offset =
                SERIES_CONSUME_CORE_REQUEST_BYTES_V3 + index * SERIES_CUSTODY_REQUEST_BYTES_V3;
            let route = invocation(FixedRole::Custody, offset, SERIES_CUSTODY_REQUEST_BYTES_V3);
            assert_eq!(
                validate_invocation(
                    route,
                    FixedRole::Custody,
                    offset,
                    SERIES_CUSTODY_REQUEST_BYTES_V3,
                    false,
                    &bank,
                )
                .expect("Custody request")
                .len(),
                SERIES_CUSTODY_REQUEST_BYTES_V3
            );
        }
        let wrong = invocation(
            FixedRole::Custody,
            SERIES_CONSUME_CORE_REQUEST_BYTES_V3 + 1,
            SERIES_CUSTODY_REQUEST_BYTES_V3,
        );
        assert_eq!(
            validate_invocation(
                wrong,
                FixedRole::Custody,
                SERIES_CONSUME_CORE_REQUEST_BYTES_V3,
                SERIES_CUSTODY_REQUEST_BYTES_V3,
                false,
                &bank,
            ),
            Err(SeriesExecuteErrorV3::Invocation)
        );
    }
}
