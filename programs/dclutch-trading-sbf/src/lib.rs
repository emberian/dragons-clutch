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

// The canonical physical profiles exceed the 64-account hard limit of the
// pinned `entrypoint_no_alloc!` deserializer. The standard entrypoint owns its
// bounded per-instruction account vector on the SBF heap instead.
extern crate std;

#[cfg(feature = "shadow-accelerator-auth-only")]
use solana_program::program_error::ProgramError;
#[cfg(not(feature = "shadow-accelerator-auth-only"))]
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};

/// Ephemeral exact prior-child receipt retention for the common Hot executor.
#[cfg(not(feature = "shadow-accelerator-auth-only"))]
mod child_receipt_v3;

/// Family-neutral authoritative admitted-AOT candidate CPI.
#[cfg(not(feature = "shadow-accelerator-auth-only"))]
pub mod admitted_composition_v3;

/// Family-neutral EffectProgram V3 composition for canonical Claims CPIs.
#[cfg(all(
    not(feature = "shadow-accelerator-auth-only"),
    any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    )
))]
pub mod claims_composition_v3;
/// Family-neutral EffectProgram V3 composition for canonical Core CPIs.
#[cfg(not(feature = "shadow-accelerator-auth-only"))]
pub mod core_composition_v3;
/// Family-neutral EffectProgram V3 composition for canonical Custody CPIs.
#[cfg(all(
    not(feature = "shadow-accelerator-auth-only"),
    any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    )
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
#[cfg(not(feature = "shadow-accelerator-auth-only"))]
pub mod dispatch_v3;
/// Profile13 physical representative expansion shared by prefix and continuation.
#[cfg(not(feature = "shadow-accelerator-auth-only"))]
mod dynamic_accounts_v4;
/// Registry-authenticated family-neutral Execution Strategy V2 admission.
pub mod execution_strategy_v2;
/// General family projection behind the common data-defined Trading boundary.
#[cfg(feature = "families")]
pub mod general;
/// Family-neutral authenticated V3 hot execution outer.
#[cfg(not(feature = "shadow-accelerator-auth-only"))]
pub mod hot_v3;
/// Family-neutral native-signature evidence authentication and register seeding.
#[cfg(not(feature = "shadow-accelerator-auth-only"))]
pub mod native_signature;
/// Family-neutral executable Core-to-Trading boundary.
#[cfg(not(feature = "shadow-accelerator-auth-only"))]
pub mod outer;
/// Exact Claims Founding route and ordered projected-receipt join.
#[cfg(all(
    not(feature = "shadow-accelerator-auth-only"),
    any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    )
))]
#[allow(dead_code)]
mod projected_claims_composition_v4;
/// Exact current-Core Found route and acknowledgment join for projected Markets.
#[cfg(all(
    not(feature = "shadow-accelerator-auth-only"),
    any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    )
))]
#[allow(dead_code)]
mod projected_core_composition_v4;
/// Family-neutral projected-Custody route-zero execution and receipt join.
#[cfg(all(
    not(feature = "shadow-accelerator-auth-only"),
    any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    )
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
#[cfg(all(
    not(feature = "shadow-accelerator-auth-only"),
    any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    )
))]
#[allow(dead_code)]
mod projected_open_composition_v4;
/// Exact projected-Hoard realization route and receipt join.
#[cfg(all(
    not(feature = "shadow-accelerator-auth-only"),
    any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    )
))]
#[allow(dead_code)]
mod projected_realize_composition_v4;
/// Family-neutral EffectProgram V3 composition for canonical Resolution CPIs.
#[cfg(not(feature = "shadow-accelerator-auth-only"))]
pub mod resolution_composition_v3;
/// Series family projection behind the common data-defined Trading boundary.
#[cfg(any(feature = "families", feature = "series-family"))]
pub mod series;
/// Small authenticated boundary for external Shadow accelerator callbacks.
pub mod shadow_accelerator_auth_v4;
/// Family-neutral read-only Shadow-AOT comparison CPI.
#[cfg(not(feature = "shadow-accelerator-auth-only"))]
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

/// Maximum bounded account vector accepted by the canonical Trading entrypoint.
///
/// This covers the Registry-continuation Hot frame at the common heap-profile
/// maxima: 38 fixed accounts, one continuation admission, eight admitted-AOT
/// evidence accounts, ten 880-byte bank pages, and 251 non-injected physical
/// runtime representatives. The standard entrypoint deserializes this vector
/// without the obsolete 64-account fixed-array limit; larger frames refuse
/// before any family or mutation path executes.
pub const TRADING_MAX_INSTRUCTION_ACCOUNTS_V3: usize = 308;

#[cfg(all(
    not(feature = "no-entrypoint"),
    not(feature = "shadow-accelerator-auth-only")
))]
solana_program::entrypoint!(program_entrypoint);

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
    require_instruction_account_bound_v3(accounts.len())?;
    if hot_v3::is_hot_execution_v3(instruction_data) {
        hot_v3::process_hot_execution_v3(program_id, accounts, instruction_data)
    } else {
        outer::process_activation(program_id, accounts, instruction_data)
    }
}

#[cfg(not(feature = "shadow-accelerator-auth-only"))]
fn require_instruction_account_bound_v3(account_count: usize) -> ProgramResult {
    if account_count <= TRADING_MAX_INSTRUCTION_ACCOUNTS_V3 {
        Ok(())
    } else {
        Err(TradingSbfError::UnsupportedContent.into())
    }
}

#[cfg(all(test, not(feature = "shadow-accelerator-auth-only")))]
mod entrypoint_tests {
    use super::*;

    #[test]
    fn canonical_bound_exceeds_legacy_limit_and_refuses_overflow() {
        assert!(TRADING_MAX_INSTRUCTION_ACCOUNTS_V3 > 64);
        assert_eq!(require_instruction_account_bound_v3(65), Ok(()));
        assert_eq!(
            require_instruction_account_bound_v3(TRADING_MAX_INSTRUCTION_ACCOUNTS_V3),
            Ok(())
        );
        assert_eq!(
            require_instruction_account_bound_v3(TRADING_MAX_INSTRUCTION_ACCOUNTS_V3 + 1),
            Err(TradingSbfError::UnsupportedContent.into())
        );
    }
}
