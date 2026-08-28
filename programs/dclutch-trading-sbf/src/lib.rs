#![no_std]
#![deny(unsafe_code)]
#![deny(missing_docs)]

//! Canonical data-driven SBF adapter for the fixed Trading execution role.
//!
//! The executable capability lifecycle path authenticates the common Core
//! envelope, current Registry releases, finalized
//! descriptor/config/profile/effect records, and interpreted
//! transition/effects. Activation commits one composite root and its selected
//! FundingLedgerV2 row. Native close refunds the exact remaining principal,
//! root/ledger Rent, and separately classified surplus to the Market's one
//! RentCredit before making the root and selected ledger vacant. Realm/token
//! custody remains explicitly unsupported. The boundary has no family
//! discriminator.

// The kernel crates this adapter calls are `no_std`; the adapter layer itself
// allocates, so the executable links `std` for `Vec`/`Box` and for the
// `GlobalAlloc` implementation `entrypoint_adapter` installs.
extern crate std;

use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};

/// One author for the child-CPI caller authority, derived once per execution.
pub mod child_authority_v4;

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
// Composed entirely of `crate::series` routes and constants, so it is gated
// exactly as `series` is: one feature wider and `--features dealer-family`
// alone builds this module against a module that is not there.
#[cfg(any(feature = "families", feature = "series-family"))]
#[allow(dead_code)]
mod projected_claims_composition_v4;
/// Exact current-Core Found route and acknowledgment join for projected Markets.
// Composed entirely of `crate::series` routes and constants, so it is gated
// exactly as `series` is: one feature wider and `--features dealer-family`
// alone builds this module against a module that is not there.
#[cfg(any(feature = "families", feature = "series-family"))]
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
// Composed entirely of `crate::series` routes and constants, so it is gated
// exactly as `series` is: one feature wider and `--features dealer-family`
// alone builds this module against a module that is not there.
#[cfg(any(feature = "families", feature = "series-family"))]
#[allow(dead_code)]
mod projected_open_composition_v4;
/// Exact projected-Hoard realization route and receipt join.
// Composed entirely of `crate::series` routes and constants, so it is gated
// exactly as `series` is: one feature wider and `--features dealer-family`
// alone builds this module against a module that is not there.
#[cfg(any(feature = "families", feature = "series-family"))]
#[allow(dead_code)]
mod projected_realize_composition_v4;
/// Family-neutral EffectProgram V3 composition for canonical Resolution CPIs.
pub mod resolution_composition_v3;
/// Series family projection behind the common data-defined Trading boundary.
#[cfg(any(feature = "families", feature = "series-family"))]
pub mod series;
/// Family-neutral read-only Shadow-AOT comparison CPI.
pub mod shadow_composition_v3;
/// Wallet-authorized caller for one canonical Claims User Position.
pub mod user_position_admission_v1;

/// Stable refusal from the canonical Trading SBF boundary.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TradingSbfError {
    /// The instruction is not supported by an admitted content profile.
    UnsupportedContent = 0x4000,
    /// The Registry receipt did not authenticate this Program as current Trading.
    Release = 0x4001,
    /// The immutable Trading child root or its PDA refused.
    Root = 0x4002,
    /// Manifest, selected entry, descriptor, or config content refused.
    Content = 0x4003,
    /// The checked data-defined transition refused.
    Transition = 0x4004,
    /// A projected physical mutation or account write could not commit.
    Commit = 0x4005,
    /// Instructions-sysvar or native-signature evidence was not exact.
    NativeSignature = 0x4006,
    /// The release's pinned deployment slot moved: the substrate was upgraded.
    ///
    /// Decision 0012. Not a corrupted account and not an attack: the exact
    /// upgrade authority the release names shipped new bytes, so the cached
    /// authentication no longer describes what is deployed. Every open market
    /// on the superseded release generation refuses until a re-release
    /// re-authenticates the new deployment and re-pins its slot.
    ReleaseSuperseded = 0x4007,
}

// Registered refusal band (`docs/decisions/0007-namespaced-refusal-codes.md`).
// The discriminants stay literal so a code seen in a validator log is greppable;
// these assertions are what stops them drifting out of the allocated band.
const _: () = assert!(
    TradingSbfError::UnsupportedContent as u32 == dclutch_refusal_registry::TRADING_REFUSAL_BASE,
    "TradingSbfError must start at its registered refusal band base"
);
const _: () = assert!(
    (TradingSbfError::ReleaseSuperseded as u32)
        < dclutch_refusal_registry::TRADING_REFUSAL_BASE + dclutch_refusal_registry::BAND_SPAN,
    "TradingSbfError must not run past its registered refusal band"
);

impl From<TradingSbfError> for ProgramError {
    fn from(value: TradingSbfError) -> Self {
        Self::Custom(value as u32)
    }
}

use crate::admitted_composition_v3::ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4;
use dclutch_capability_program_contract::hot_v3::HOT_FIXED_ACCOUNT_COUNT_V3;

/// Bank pages the widest common heap profile carries.
///
/// **Measured-profile headroom, not a protocol fact** -- the same kind of
/// number as `entrypoint_adapter::ADAPTER_STACK_SLOTS_V1`, and named for the
/// same reason: so the bound it feeds is a sum of things that can be checked
/// one at a time. Each page carries
/// `dclutch_execution_strategy_contract::generated_v2::ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2`
/// (880) bytes of accelerator input bank.
pub const TRADING_MAX_HOT_BANK_PAGES_V3: usize = 10;

/// Non-injected physical runtime representatives the widest common heap
/// profile carries.
///
/// **Measured-profile headroom, not a protocol fact.** The canonical
/// Registry-continuation Hot bundle uses 78 account slots; this is the ceiling
/// the entrypoint will deserialize, not a width anything reaches.
pub const TRADING_MAX_HOT_PHYSICAL_REPRESENTATIVES_V3: usize = 251;

/// Maximum bounded account vector accepted by the canonical Trading entrypoint.
///
/// This covers the Registry-continuation Hot frame at the common heap-profile
/// maxima, and is now the SUM OF ITS TERMS rather than a literal that has to be
/// re-added by hand:
///
/// | term | value | authority |
/// |---|---:|---|
/// | common fixed frame | [`HOT_FIXED_ACCOUNT_COUNT_V3`] | capability-program contract |
/// | Registry continuation admission | 1 | `hot_v3::authenticate_hot_invocation_v3` |
/// | admitted-AOT strategy evidence | [`ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4`] | `admitted_composition_v3` |
/// | bank pages | [`TRADING_MAX_HOT_BANK_PAGES_V3`] | measured profile |
/// | physical runtime representatives | [`TRADING_MAX_HOT_PHYSICAL_REPRESENTATIVES_V3`] | measured profile |
///
/// It was written as `308` on 2026-08-26 with `38 fixed accounts` in its prose,
/// and the fixed frame had ALREADY become 39 -- `ca5e5f14` appended
/// `HOT_CAPABILITY_SEAL_ACCOUNT_V3` at index 38. So the declared bound was one
/// account short of the very shape its own comment enumerated, and would have
/// refused a maximal frame of that shape. This is the third copy of that stale
/// `38` to be found (see `ADMITTED_ACCELERATOR_HOT_FIXED_COUNT_V4` for the first
/// two), which is why it is a sum now and not a number.
///
/// The Registry continuation's six-account OUTER prefix is deliberately absent
/// from this arithmetic, and that resolves an open question rather than
/// forgetting one: the prefix lives in the top-level instruction's sysvar
/// record, never in the account vector this bound measures.
/// `hot_v3::authenticate_hot_invocation_v3` proves it, comparing
/// `observed.metas_from(REGISTRY_CONTINUATION_OUTER_PREFIX_ACCOUNTS_V1).len()`
/// against `accounts.len()` -- the nested vector begins after the prefix.
///
/// `entrypoint_adapter` deserializes this vector without the obsolete
/// 64-account fixed-array limit, on the stack up to
/// `entrypoint_adapter::ADAPTER_STACK_SLOTS_V1` and in an exactly-sized heap
/// buffer above it; larger frames refuse before any family or mutation path
/// executes.
pub const TRADING_MAX_INSTRUCTION_ACCOUNTS_V3: usize = HOT_FIXED_ACCOUNT_COUNT_V3
    + 1
    + ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4
    + TRADING_MAX_HOT_BANK_PAGES_V3
    + TRADING_MAX_HOT_PHYSICAL_REPRESENTATIVES_V3;

/// Execute the family-neutral authenticated capability lifecycle route.
///
/// Activation and native closure share the common descriptor/profile boundary.
/// Realm/token closure remains fail-closed until its ordered-vault adapter
/// lands in this same authority boundary.
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    require_instruction_account_bound_v3(accounts.len())?;
    if dclutch_user_position_admission_contract::is_user_position_admission_v1(instruction_data) {
        return user_position_admission_v1::process_user_position_admission_v1(
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
    if generic_market_founding_v1::is_generic_market_founding_v2(instruction_data) {
        return generic_market_founding_v1::process_generic_market_founding_v2(
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
    if projected_custody_bootstrap_v1::is_projected_custody_bootstrap_v2(instruction_data) {
        return projected_custody_bootstrap_v1::process_projected_custody_bootstrap_v2(
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
        outer::process_capability_lifecycle(program_id, accounts, instruction_data)
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
        // Deliberately a compile-time fact: the canonical bound must exceed
        // the legacy 64-account limit for the runtime checks below to mean
        // anything, so let the compiler enforce it.
        const { assert!(TRADING_MAX_INSTRUCTION_ACCOUNTS_V3 > 64) };
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

    #[test]
    fn canonical_bound_admits_a_maximal_frame_and_moves_when_its_terms_move() {
        // NOT a restatement of the definition. The bound is a sum now, so the
        // only thing left to assert is that its VALUE is pinned: any term
        // changing lands here as a diff on a number a reviewer has to look at,
        // the same discipline this repo's identity tables use. The reason it
        // needs one is on the record -- the bound read `308` with `38 fixed
        // accounts` in its prose while the contract already said 39, so it
        // would have refused a maximal frame of the exact shape it enumerated.
        assert_eq!(HOT_FIXED_ACCOUNT_COUNT_V3, 39);
        assert_eq!(TRADING_MAX_INSTRUCTION_ACCOUNTS_V3, 309);

        // The property the pin exists to protect: a frame of the widest
        // enumerated shape is ADMITTED, and one account wider is refused.
        let widest = HOT_FIXED_ACCOUNT_COUNT_V3
            + 1
            + ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4
            + TRADING_MAX_HOT_BANK_PAGES_V3
            + TRADING_MAX_HOT_PHYSICAL_REPRESENTATIVES_V3;
        assert_eq!(require_instruction_account_bound_v3(widest), Ok(()));
        assert_eq!(
            require_instruction_account_bound_v3(widest + 1),
            Err(TradingSbfError::UnsupportedContent.into())
        );
    }
}
