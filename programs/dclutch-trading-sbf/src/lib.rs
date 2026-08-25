#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Canonical data-driven SBF adapter for the fixed Trading execution role.
//!
//! The base release exposes the common authenticated dispatch boundary. It
//! deliberately contains no family discriminator and applies no child effect
//! until an exact content profile is implemented by this checked artifact.

// `entrypoint_no_alloc!` expands through `std::mem::MaybeUninit` on both host
// and SBF targets; the Solana toolchain supplies that core-compatible facade.
extern crate std;

use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};

/// Manifest-, root-, release-, and descriptor-authenticated generic dispatch.
pub mod dispatch;

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

/// Refuse until an exact supported content projector invokes [`dispatch`].
///
/// This base entrypoint cannot interpret arbitrary accounts as registers and
/// cannot apply arbitrary effects. Family integration extends the artifact
/// with exact content-ID support and calls the common dispatch boundary; it
/// must not add another Program-authority route.
#[inline(never)]
pub fn process_instruction(
    _program_id: &Pubkey,
    _accounts: &[AccountInfo<'_>],
    _instruction_data: &[u8],
) -> ProgramResult {
    Err(TradingSbfError::UnsupportedContent.into())
}
