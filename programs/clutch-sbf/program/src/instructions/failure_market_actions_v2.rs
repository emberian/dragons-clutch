// SPDX-License-Identifier: AGPL-3.0-or-later
//! Concrete routed owners for current Market Failure actions 10 through 13.
//!
//! Each handler starts from the exact checked wire/account contract and then
//! hostile-reopens every semantic owner. This module contains no fallback DTO
//! path and never imports the withdrawn occurrence-scoped ExternalV2 runtime.

use std::boxed::Box;

use crate::accounts::Outcome;
use crate::instructions::failure_market_dispatch_v2::{
    account_for_role_v2, FailureMarketAccountRoleV2 as Role, FailureMarketActionPayloadV2,
};
use crate::instructions::failure_market_execution_v2::authenticate_failure_market_execution_v2;
use crate::instructions::failure_market_interval_v2::exhaust_and_archive_failure_market_interval_session_v2;
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV1, SeriesMarketLinkAccountV1,
};
use clutch_solana_layout::registry::RecoveryAction;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// Execute the deterministic finite-exhaustion action13 path.
///
/// The Product root and Recovery compartment are both read-only. Only the
/// shared runtime, reusable cell/history, and initiating Series link mutate;
/// the exact terminal cell is folded before reset and Product release.
pub(crate) fn process_archive_failure_market_session_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: FailureMarketActionPayloadV2<'_>,
) -> Outcome<()> {
    let FailureMarketActionPayloadV2::Archive {
        recovery_quote_schedule,
        interval_funding_preimage,
    } = payload
    else {
        return crate::instructions::failure_market_dispatch_v2::process_reserved_disabled(
            RecoveryAction::CloseIntervalConsensusWork,
        );
    };
    let action = RecoveryAction::CloseIntervalConsensusWork;
    let root_account = account_for_role_v2(action, accounts, Role::MarketLifecycleRoot)?;
    let link_account = account_for_role_v2(action, accounts, Role::SeriesMarketLink)?;
    let admission_account = account_for_role_v2(action, accounts, Role::FailureAdmissionRoot)?;
    let runtime_account = account_for_role_v2(action, accounts, Role::FailureRuntimeRoot)?;
    let cell_account = account_for_role_v2(action, accounts, Role::FailureIntervalCell)?;
    let history_account = account_for_role_v2(action, accounts, Role::FailureIntervalHistory)?;
    let series_registry = account_for_role_v2(action, accounts, Role::SeriesRegistry)?;
    let registry_program = account_for_role_v2(action, accounts, Role::RegistryProgram)?;
    let registry_programdata = account_for_role_v2(action, accounts, Role::RegistryProgramData)?;
    let registry_release =
        account_for_role_v2(action, accounts, Role::RegistryReleaseArtifact)?;
    let capability_profile =
        account_for_role_v2(action, accounts, Role::CapabilityProfileArtifact)?;
    let compiler_bundle = account_for_role_v2(action, accounts, Role::CompilerBundleArtifact)?;
    let funding_quote = account_for_role_v2(action, accounts, Role::FundingQuoteArtifact)?;
    let liveness_policy = account_for_role_v2(action, accounts, Role::FailureLivenessPolicy)?;
    let recovery = account_for_role_v2(action, accounts, Role::FailureRecoveryCompartment)?;

    let mut root_before = Box::new(MarketLifecycleRootAccountV1::decode_buffer());
    let mut link_before = Box::new(SeriesMarketLinkAccountV1::decode_buffer());
    let execution = authenticate_failure_market_execution_v2(
        program_id,
        root_account,
        link_account,
        admission_account,
        runtime_account,
        cell_account,
        history_account,
        series_registry,
        registry_program,
        registry_programdata,
        registry_release,
        capability_profile,
        compiler_bundle,
        funding_quote,
        liveness_policy,
        recovery_quote_schedule,
        interval_funding_preimage,
        false,
        true,
        true,
        true,
        &mut root_before,
        &mut link_before,
    )?;
    execution.require_next_sequence(sequence)?;

    let mut root_reopen = Box::new(MarketLifecycleRootAccountV1::decode_buffer());
    let mut link_preauthorization = Box::new(SeriesMarketLinkAccountV1::decode_buffer());
    let mut link_rebound = Box::new(SeriesMarketLinkAccountV1::decode_buffer());
    let _ = exhaust_and_archive_failure_market_interval_session_v2(
        program_id,
        root_account,
        link_account,
        admission_account,
        runtime_account,
        cell_account,
        history_account,
        liveness_policy,
        recovery,
        execution.root(),
        execution.link(),
        execution.admission(),
        execution.runtime(),
        execution.interval(),
        &mut root_reopen,
        &mut link_preauthorization,
        &mut link_rebound,
    )?;
    Ok(())
}

#[cfg(test)]
mod adversarial_action_tests {
    #[test]
    fn action13_has_one_typed_routed_outer_and_no_recovery_close() {
        let source = include_str!("failure_market_actions_v2.rs");
        let handler = source
            .split("fn process_archive_failure_market_session_v2")
            .nth(1)
            .expect("action13 handler");
        for owner in [
            "authenticate_failure_market_execution_v2",
            "execution.require_next_sequence(sequence)",
            "exhaust_and_archive_failure_market_interval_session_v2",
        ] {
            assert!(handler.contains(owner));
        }
        assert!(handler.contains("false,\n+        true,\n+        true,\n+        true,"));
        assert!(!handler.contains("close_failure_market_recovery_v2"));
        assert!(!handler.contains("ExternalRecoveryStateV1"));
    }
}
