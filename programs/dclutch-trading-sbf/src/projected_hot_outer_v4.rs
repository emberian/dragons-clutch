//! Projected-Market entry into one authenticated Hot V4 plan.
//!
//! This outer deliberately owns no semantic or receipt state.  The opaque
//! common plan authenticates the global artifact tuple once, retains the two
//! prefix-route receipts, promotes the affine span only from the typed current
//! Core Found acknowledgement, and exposes the live-Market continuation only
//! after reauthentication.  Consuming stage types make route reordering,
//! prefix replay, and caller-authored resume state unrepresentable here.

use crate::{
    projected_hot_plan_v4::authenticate_projected_hot_plan_v4,
    projected_market_v2::PROJECTED_MARKET_EXECUTION_MAGIC_V2,
};
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey};

/// Return whether bytes select the compact projected-Market V4 execution.
///
/// This is only a cheap dispatcher discriminator.  The selected common plan
/// hostile-decodes the complete instruction and authenticates its exact width,
/// artifacts, request, accounts, and runtime observations.
pub(crate) fn is_projected_hot_execution_v4(instruction_data: &[u8]) -> bool {
    instruction_data.get(..8) == Some(PROJECTED_MARKET_EXECUTION_MAGIC_V2.as_slice())
}

/// Execute the sole projected prefix and live-Market continuation chain.
///
/// No scalar, request-bank slice, route frame, child receipt, or commit
/// candidate crosses this outer API.  Each call consumes its predecessor; the
/// common authenticated-plan module is the sole owner of all intermediate
/// evidence and commits the root and Ticket only after the final Core Open
/// acknowledgement.
#[inline(never)]
pub(crate) fn process_projected_hot_execution_v4(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    authenticate_projected_hot_plan_v4(program_id, accounts, instruction_data)?
        .execute_lock()?
        .execute_found()?
        .resume_after_found_ack()?
        .execute_realize()?
        .execute_claims()?
        .execute_open_and_commit()
}
