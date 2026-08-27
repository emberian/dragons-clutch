#![no_std]
#![deny(unsafe_code)]
#![deny(missing_docs)]

//! Canonical data-driven SBF adapter for the fixed Trading execution role.
//!
//! The executable activation path authenticates the common Core envelope,
//! current Registry releases, finalized descriptor/config/profile/effect
//! records, and interpreted transition/effects before committing one composite
//! root, its FundingState accounts, and the exact Core acknowledgment. It has
//! no family discriminator.

// The kernel crates this adapter calls are `no_std`; the adapter layer itself
// allocates, so the executable links `std` for `Vec`/`Box` and for the
// `GlobalAlloc` implementation `entrypoint_adapter` installs.
extern crate std;

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
/// The named machine boundary: SBF entrypoint, input deserialization, heap.
///
/// The ONE `unsafe` exemption in this executable. Everything reachable from
/// `process_instruction` is safe Rust; this module converts the loader's raw
/// input region into the safe values that boundary consumes and owns the bump
/// allocator they are measured against. Its module documentation carries the
/// full trust surface. No other module in this crate may carry this attribute.
#[allow(unsafe_code)]
pub mod entrypoint_adapter;
/// Registry-authenticated family-neutral Execution Strategy V2 admission.
pub mod execution_strategy_v2;
/// General family projection behind the common data-defined Trading boundary.
#[cfg(feature = "families")]
pub mod general;
/// Generic atomic Custody→Core→Claims Market founding and commit-last Open.
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
pub mod generic_market_founding_v1;
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
/// Family-neutral creation of the projected-Custody prestate founding needs.
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
pub mod projected_custody_bootstrap_v1;
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

/// Maximum bounded account vector accepted by the canonical Trading entrypoint.
///
/// This covers the Registry-continuation Hot frame at the common heap-profile
/// maxima: 38 fixed accounts, one continuation admission, eight admitted-AOT
/// evidence accounts, ten 880-byte bank pages, and 251 non-injected physical
/// runtime representatives. `entrypoint_adapter` deserializes this vector
/// without the obsolete 64-account fixed-array limit, on the stack up to
/// `entrypoint_adapter::ADAPTER_STACK_SLOTS_V1` and in an exactly-sized heap
/// buffer above it; larger frames refuse before any family or mutation path
/// executes.
pub const TRADING_MAX_INSTRUCTION_ACCOUNTS_V3: usize = 308;

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
    require_instruction_account_bound_v3(accounts.len())?;
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    if generic_market_founding_v1::is_generic_market_founding_v1(instruction_data) {
        return generic_market_founding_v1::process_generic_market_founding_v1(
            program_id,
            accounts,
            instruction_data,
        );
    }
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    if projected_custody_bootstrap_v1::is_projected_custody_bootstrap_v1(instruction_data) {
        return projected_custody_bootstrap_v1::process_projected_custody_bootstrap_v1(
            program_id,
            accounts,
            instruction_data,
        );
    }
    // The way back out of a staged prestate whose founding never happened. A
    // projection that reached `SourceFunded` and did not found before its
    // expiry slot holds collateral that the forward direction can no longer
    // move, because Core's Found and Open stages both refuse an expired
    // artifact. Without this route that collateral is stranded permanently.
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    if projected_custody_bootstrap_v1::is_projected_custody_abort_v1(instruction_data) {
        return projected_custody_bootstrap_v1::process_projected_custody_abort_v1(
            program_id,
            accounts,
            instruction_data,
        );
    }
    // Decision 0005. The validated-artifact seal is written by its own outer,
    // once per (descriptor, action, Trading interpreter release, Registry).
    // It creates one PDA under this Program, signs nothing else, and can only
    // ever persist this executable's own verdict about immutable public bytes.
    if dclutch_capability_seal_contract::is_capability_seal_request_v1(instruction_data) {
        return hot_v3::process_capability_seal_v1(program_id, accounts, instruction_data);
    }
    if hot_v3::is_hot_execution_v3(instruction_data) {
        hot_v3::process_hot_execution_v3(program_id, accounts, instruction_data)
    } else {
        outer::process_activation(program_id, accounts, instruction_data)
    }
}

fn require_instruction_account_bound_v3(account_count: usize) -> ProgramResult {
    if account_count <= TRADING_MAX_INSTRUCTION_ACCOUNTS_V3 {
        Ok(())
    } else {
        Err(TradingSbfError::UnsupportedContent.into())
    }
}

#[cfg(test)]
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
