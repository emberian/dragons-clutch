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

use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};

/// Family-neutral EffectProgram V3 composition for canonical Claims CPIs.
#[cfg(feature = "families")]
pub mod claims_composition_v3;

/// Dealer family projection behind the common data-defined Trading boundary.
#[cfg(feature = "families")]
pub mod dealer;
/// Manifest-, root-, release-, and descriptor-authenticated generic dispatch.
pub mod dispatch;
/// Registry-authenticated family-neutral Execution Strategy V2 admission.
pub mod execution_strategy_v2;
/// General family projection behind the common data-defined Trading boundary.
#[cfg(feature = "families")]
pub mod general;
/// Family-neutral native-signature evidence authentication and register seeding.
pub mod native_signature;
/// Family-neutral executable Core-to-Trading boundary.
pub mod outer;
/// Series family projection behind the common data-defined Trading boundary.
#[cfg(feature = "families")]
pub mod series;

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

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint_no_alloc!(program_entrypoint);

#[cfg(not(feature = "no-entrypoint"))]
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
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    outer::process_activation(program_id, accounts, instruction_data)
}
