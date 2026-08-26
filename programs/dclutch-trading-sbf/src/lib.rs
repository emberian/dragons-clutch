#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Canonical data-driven SBF adapter for the fixed Trading execution role.
//!
//! The executable activation path authenticates the common Core envelope,
//! current Registry releases, finalized descriptor/config/profile/effect
//! records, and interpreted transition/effects before committing one composite
//! root, its FundingState accounts, and the exact Core acknowledgment. It has
//! no family discriminator.

// `entrypoint_no_alloc!` expands through `std::mem::MaybeUninit` on both host
// and SBF targets; the Solana toolchain supplies that core-compatible facade.
extern crate std;

#[cfg(feature = "shadow-accelerator-auth-only")]
use solana_program::program_error::ProgramError;
#[cfg(not(feature = "shadow-accelerator-auth-only"))]
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};

/// Ephemeral exact prior-child receipt retention for the common Hot executor.
mod child_receipt_v3;

/// Family-neutral authoritative admitted-AOT candidate CPI.
pub mod admitted_composition_v3;

/// Family-neutral EffectProgram V3 composition for canonical Claims CPIs.
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
pub mod claims_composition_v3;
/// Family-neutral EffectProgram V3 composition for canonical Core CPIs.
pub mod core_composition_v3;
/// Family-neutral EffectProgram V3 composition for canonical Custody CPIs.
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
pub mod custody_composition_v3;
/// Dealer family projection behind the common data-defined Trading boundary.
#[cfg(any(feature = "families", feature = "dealer-family"))]
pub mod dealer;
/// Direct family projection behind the common data-defined Trading boundary.
#[cfg(feature = "families")]
pub mod direct;
/// Manifest-, root-, release-, and descriptor-authenticated generic dispatch.
pub mod dispatch;
/// V3 descriptor joins for independently finalized runtime-tail artifacts.
pub mod dispatch_v3;
/// Profile13 physical representative expansion shared by prefix and continuation.
mod dynamic_accounts_v4;
/// Registry-authenticated family-neutral Execution Strategy V2 admission.
pub mod execution_strategy_v2;
/// General family projection behind the common data-defined Trading boundary.
#[cfg(feature = "families")]
pub mod general;
/// Family-neutral authenticated V3 hot execution outer.
pub mod hot_v3;
/// Family-neutral native-signature evidence authentication and register seeding.
pub mod native_signature;
/// Family-neutral executable Core-to-Trading boundary.
pub mod outer;
/// Exact Claims Founding route and ordered projected-receipt join.
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
#[allow(dead_code)]
mod projected_claims_composition_v4;
/// Exact current-Core Found route and acknowledgment join for projected Markets.
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
#[allow(dead_code)]
mod projected_core_composition_v4;
/// Family-neutral projected-Custody route-zero execution and receipt join.
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
#[allow(dead_code)]
mod projected_custody_composition_v4;
/// Compact projected-Market execution and persisted-state reconstruction.
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
pub mod projected_market_v2;
/// Final Core Open and Trading replay commit-last boundary.
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
#[allow(dead_code)]
mod projected_open_composition_v4;
/// Exact projected-Hoard realization route and receipt join.
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
#[allow(dead_code)]
mod projected_realize_composition_v4;
/// Family-neutral EffectProgram V3 composition for canonical Resolution CPIs.
pub mod resolution_composition_v3;
/// Series family projection behind the common data-defined Trading boundary.
#[cfg(any(feature = "families", feature = "series-family"))]
pub mod series;
/// Small authenticated boundary for external Shadow accelerator callbacks.
pub mod shadow_accelerator_auth_v4;
/// Family-neutral read-only Shadow-AOT comparison CPI.
pub mod shadow_composition_v3;

/// Stable refusal from the canonical Trading SBF boundary.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TradingSbfError {
    /// The instruction is not supported by an admitted content profile.
    UnsupportedContent = 0,
    /// The Registry receipt did not authenticate this Program as current Trading.
    Release = 1,
    /// The immutable Trading child root or its PDA refused.
    Root = 2,
    /// Manifest, selected entry, descriptor, or config content refused.
    Content = 3,
    /// The checked data-defined transition refused.
    Transition = 4,
    /// A projected physical mutation or account write could not commit.
    Commit = 5,
    /// Instructions-sysvar or native-signature evidence was not exact.
    NativeSignature = 6,
}

impl From<TradingSbfError> for ProgramError {
    fn from(value: TradingSbfError) -> Self {
        Self::Custom(value as u32)
    }
}

#[cfg(all(
    not(feature = "no-entrypoint"),
    not(feature = "shadow-accelerator-auth-only")
))]
solana_program::entrypoint_no_alloc!(program_entrypoint);

#[cfg(all(
    not(feature = "no-entrypoint"),
    not(feature = "shadow-accelerator-auth-only")
))]
fn program_entrypoint(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    process_instruction(program_id, accounts, instruction_data)
}

/// Execute the family-neutral authenticated activation route.
///
/// Hot actions and closure remain fail-closed until their common profile and
/// fixed-role receipt composition land in this same authority boundary.
#[cfg(not(feature = "shadow-accelerator-auth-only"))]
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if hot_v3::is_hot_execution_v3(instruction_data) {
        hot_v3::process_hot_execution_v3(program_id, accounts, instruction_data)
    } else {
        outer::process_activation(program_id, accounts, instruction_data)
    }
}
