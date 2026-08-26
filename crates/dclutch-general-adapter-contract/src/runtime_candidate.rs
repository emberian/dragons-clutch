//! Exact General settlement projection into the generic Strategy V2 bank.
//!
//! The stateless General evaluator returns a canonical settlement effect plan.
//! This module gives that result one sparse, affine register ABI consumed by
//! generic Trading's common EffectProgram path. It does not select accounts,
//! write state, invoke a child, or commit scratch. The caller supplies the
//! independently Product-authenticated tail count and exact candidate banks;
//! any mismatch refuses before the output bank changes.

use dclutch_execution_strategy_contract::v2::{ExecutionCandidateV2, register_bank_bytes_v2};

use crate::runtime_settlement::{RuntimeSettlementActionV2, RuntimeSettlementEffectPlanV2};
use crate::runtime_verify::RuntimeCompleteSetMoveV2;

/// Common scalar registers before one scalar quantity per Product outcome.
pub const GENERAL_SETTLEMENT_COMMON_SCALARS_V2: u32 = 11;
/// One exact quantity register per Product outcome.
pub const GENERAL_SETTLEMENT_ITEM_SCALAR_STRIDE_V2: u32 = 1;
/// Candidate, owner, order, and beneficiary identities.
pub const GENERAL_SETTLEMENT_COMMON_IDENTITIES_V2: u32 = 4;
/// Settlement has no per-outcome identity tail.
pub const GENERAL_SETTLEMENT_ITEM_IDENTITY_STRIDE_V2: u32 = 0;

/// Common scalar coordinate of the action tag.
pub const GENERAL_SETTLEMENT_ACTION_SCALAR_V2: u32 = 0;
/// Common scalar coordinate of the complete-set direction.
pub const GENERAL_SETTLEMENT_MOVE_SCALAR_V2: u32 = 1;
/// Common scalar coordinate enabling the Claims route.
pub const GENERAL_SETTLEMENT_CLAIMS_ACTIVE_SCALAR_V2: u32 = 2;
/// Common scalar coordinate enabling the Custody route.
pub const GENERAL_SETTLEMENT_CUSTODY_ACTIVE_SCALAR_V2: u32 = 3;
/// Common scalar coordinate enabling the terminal state effect.
pub const GENERAL_SETTLEMENT_TERMINAL_SCALAR_V2: u32 = 4;
/// Common scalar coordinate of the one-based order coordinate.
pub const GENERAL_SETTLEMENT_ORDER_COORDINATE_SCALAR_V2: u32 = 5;
/// Common scalar coordinate of the consumed settlement revision.
pub const GENERAL_SETTLEMENT_REVISION_SCALAR_V2: u32 = 6;
/// Common scalar coordinate of the signed-order nonce.
pub const GENERAL_SETTLEMENT_NONCE_SCALAR_V2: u32 = 7;
/// Common scalar coordinate of the exact quote movement.
pub const GENERAL_SETTLEMENT_QUOTE_QUANTITY_SCALAR_V2: u32 = 8;
/// Common scalar coordinate of the uniform complete-set movement.
pub const GENERAL_SETTLEMENT_COMPLETE_SET_QUANTITY_SCALAR_V2: u32 = 9;
/// Common scalar coordinate of the nonzero terminal coordinate.
pub const GENERAL_SETTLEMENT_TERMINAL_COORDINATE_SCALAR_V2: u32 = 10;

/// Common identity coordinate of the selected Candidate.
pub const GENERAL_SETTLEMENT_CANDIDATE_IDENTITY_V2: u32 = 0;
/// Common identity coordinate of the selected row owner.
pub const GENERAL_SETTLEMENT_OWNER_IDENTITY_V2: u32 = 1;
/// Common identity coordinate of the selected signed order.
pub const GENERAL_SETTLEMENT_ORDER_IDENTITY_V2: u32 = 2;
/// Common identity coordinate of the immutable surplus beneficiary.
pub const GENERAL_SETTLEMENT_BENEFICIARY_IDENTITY_V2: u32 = 3;

/// Stable refusal from General-to-Strategy candidate projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeCandidateErrorV2 {
    /// The semantic effect plan refused hostile decoding.
    InvalidPlan,
    /// Product tail count and General plan width differed.
    TailCountMismatch,
    /// A caller-owned complete candidate bank had another exact capacity.
    InvalidCapacity,
    /// Checked affine register or byte geometry overflowed.
    ArithmeticOverflow,
}

/// Result alias for General Strategy-bank projection.
pub type RuntimeCandidateResultV2<T> = core::result::Result<T, RuntimeCandidateErrorV2>;

/// Return the exact runtime scalar count for one authenticated Product width.
pub fn general_settlement_scalar_count_v2(tail_count: u32) -> RuntimeCandidateResultV2<u32> {
    GENERAL_SETTLEMENT_COMMON_SCALARS_V2
        .checked_add(
            tail_count
                .checked_mul(GENERAL_SETTLEMENT_ITEM_SCALAR_STRIDE_V2)
                .ok_or(RuntimeCandidateErrorV2::ArithmeticOverflow)?,
        )
        .ok_or(RuntimeCandidateErrorV2::ArithmeticOverflow)
}

/// Return the exact scalar-then-identity Strategy candidate-bank width.
pub fn general_settlement_candidate_bank_len_v2(
    tail_count: u32,
) -> RuntimeCandidateResultV2<usize> {
    if tail_count == 0 {
        return Err(RuntimeCandidateErrorV2::TailCountMismatch);
    }
    let bytes = register_bank_bytes_v2(
        general_settlement_scalar_count_v2(tail_count)?,
        GENERAL_SETTLEMENT_COMMON_IDENTITIES_V2,
    )
    .map_err(|_| RuntimeCandidateErrorV2::ArithmeticOverflow)?;
    usize::try_from(bytes).map_err(|_| RuntimeCandidateErrorV2::ArithmeticOverflow)
}

/// Project one complete General plan into a Strategy V2 candidate atomically.
///
/// `tail_count` is supplied from the independently authenticated Product
/// result domain. `scratch` is non-authoritative; `output` remains byte-for-byte
/// unchanged on every refusal. Success returns the complete output bank as the
/// sole candidate consumed by common Trading.
pub fn project_general_settlement_candidate_v2<'a>(
    effect_plan: &[u8],
    tail_count: u32,
    scratch: &mut [u8],
    output: &'a mut [u8],
) -> RuntimeCandidateResultV2<ExecutionCandidateV2<'a>> {
    let plan = RuntimeSettlementEffectPlanV2::decode(effect_plan)
        .map_err(|_| RuntimeCandidateErrorV2::InvalidPlan)?;
    if plan.header().outcome_count != tail_count {
        return Err(RuntimeCandidateErrorV2::TailCountMismatch);
    }
    let required = general_settlement_candidate_bank_len_v2(tail_count)?;
    if scratch.len() != required || output.len() != required {
        return Err(RuntimeCandidateErrorV2::InvalidCapacity);
    }
    scratch.fill(0);
    let header = plan.header();
    for (coordinate, value) in [
        (
            GENERAL_SETTLEMENT_ACTION_SCALAR_V2,
            action_tag(header.action),
        ),
        (
            GENERAL_SETTLEMENT_MOVE_SCALAR_V2,
            move_tag(header.complete_set_move),
        ),
        (
            GENERAL_SETTLEMENT_CLAIMS_ACTIVE_SCALAR_V2,
            u64::from(header.claims_active),
        ),
        (
            GENERAL_SETTLEMENT_CUSTODY_ACTIVE_SCALAR_V2,
            u64::from(header.custody_active),
        ),
        (
            GENERAL_SETTLEMENT_TERMINAL_SCALAR_V2,
            u64::from(header.terminal),
        ),
        (
            GENERAL_SETTLEMENT_ORDER_COORDINATE_SCALAR_V2,
            u64::from(header.order_coordinate),
        ),
        (GENERAL_SETTLEMENT_REVISION_SCALAR_V2, header.revision),
        (GENERAL_SETTLEMENT_NONCE_SCALAR_V2, header.nonce),
        (
            GENERAL_SETTLEMENT_QUOTE_QUANTITY_SCALAR_V2,
            header.quote_quantity,
        ),
        (
            GENERAL_SETTLEMENT_COMPLETE_SET_QUANTITY_SCALAR_V2,
            header.complete_set_quantity,
        ),
        (
            GENERAL_SETTLEMENT_TERMINAL_COORDINATE_SCALAR_V2,
            header.terminal_coordinate,
        ),
    ] {
        write_scalar(scratch, coordinate, value)?;
    }
    for outcome in 0..tail_count {
        let coordinate = GENERAL_SETTLEMENT_COMMON_SCALARS_V2
            .checked_add(outcome)
            .ok_or(RuntimeCandidateErrorV2::ArithmeticOverflow)?;
        write_scalar(
            scratch,
            coordinate,
            plan.quantity(outcome)
                .map_err(|_| RuntimeCandidateErrorV2::InvalidPlan)?,
        )?;
    }
    let scalar_count = general_settlement_scalar_count_v2(tail_count)?;
    for (coordinate, value) in [
        (
            GENERAL_SETTLEMENT_CANDIDATE_IDENTITY_V2,
            header.candidate_id,
        ),
        (GENERAL_SETTLEMENT_OWNER_IDENTITY_V2, header.owner_id),
        (GENERAL_SETTLEMENT_ORDER_IDENTITY_V2, header.order_id),
        (
            GENERAL_SETTLEMENT_BENEFICIARY_IDENTITY_V2,
            header.beneficiary,
        ),
    ] {
        write_identity(scratch, scalar_count, coordinate, value)?;
    }
    output.copy_from_slice(scratch);
    Ok(ExecutionCandidateV2::Accepted(output))
}

fn action_tag(value: RuntimeSettlementActionV2) -> u64 {
    match value {
        RuntimeSettlementActionV2::Collect => 1,
        RuntimeSettlementActionV2::Materialize => 2,
        RuntimeSettlementActionV2::Distribute => 3,
        RuntimeSettlementActionV2::Close => 4,
    }
}

fn move_tag(value: RuntimeCompleteSetMoveV2) -> u64 {
    match value {
        RuntimeCompleteSetMoveV2::None => 0,
        RuntimeCompleteSetMoveV2::Mint => 1,
        RuntimeCompleteSetMoveV2::Merge => 2,
    }
}

fn write_scalar(output: &mut [u8], coordinate: u32, value: u64) -> RuntimeCandidateResultV2<()> {
    let offset = usize::try_from(coordinate)
        .map_err(|_| RuntimeCandidateErrorV2::ArithmeticOverflow)?
        .checked_mul(8)
        .ok_or(RuntimeCandidateErrorV2::ArithmeticOverflow)?;
    put(output, offset, &value.to_le_bytes())
}

fn write_identity(
    output: &mut [u8],
    scalar_count: u32,
    coordinate: u32,
    value: [u8; 32],
) -> RuntimeCandidateResultV2<()> {
    let scalar_bytes = usize::try_from(scalar_count)
        .map_err(|_| RuntimeCandidateErrorV2::ArithmeticOverflow)?
        .checked_mul(8)
        .ok_or(RuntimeCandidateErrorV2::ArithmeticOverflow)?;
    let identity_bytes = usize::try_from(coordinate)
        .map_err(|_| RuntimeCandidateErrorV2::ArithmeticOverflow)?
        .checked_mul(32)
        .ok_or(RuntimeCandidateErrorV2::ArithmeticOverflow)?;
    let offset = scalar_bytes
        .checked_add(identity_bytes)
        .ok_or(RuntimeCandidateErrorV2::ArithmeticOverflow)?;
    put(output, offset, &value)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> RuntimeCandidateResultV2<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(RuntimeCandidateErrorV2::ArithmeticOverflow)?;
    output
        .get_mut(offset..end)
        .ok_or(RuntimeCandidateErrorV2::InvalidCapacity)?
        .copy_from_slice(value);
    Ok(())
}
