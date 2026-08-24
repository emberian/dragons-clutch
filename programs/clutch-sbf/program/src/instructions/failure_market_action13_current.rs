// SPDX-License-Identifier: AGPL-3.0-or-later
//! Sole current Recovery78/action13 finite-exhaustion owner.
//!
//! The handler hostile-reopens RootV2, writable pinned LinkV2, RegistryV4,
//! BundleV6, QuoteV5, the reusable Failure interval pair, and read-only
//! Recovery custody. It derives the canonical finite exhaustion, folds it into
//! history, resets the cell, releases the exact Product pin, and writes the
//! shared Failure runtime transcript last. It cannot resolve or terminalize
//! the shared Product Market.

use std::boxed::Box;

use crate::accounts::{require, require_distinct, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::failure_market_admission::authenticate_failure_market_root_v2;
use crate::instructions::failure_market_dispatch_v2::{
    account_for_role_v2, FailureMarketAccountRoleV2 as Role, FailureMarketActionPayloadV2,
};
use crate::instructions::failure_market_interval_v2::{
    authenticate_failure_market_recovery_quote_v2,
    exhaust_and_archive_failure_market_interval_session_v3,
    reopen_failure_market_interval_accounts_v2, FailureMarketIntervalFundingPreimageV2,
};
use crate::instructions::failure_market_runtime::authenticate_failure_market_runtime_root_v1;
use crate::instructions::product_artifact::authenticate_product_artifact_v1;
use crate::instructions::product_series_current::{
    authenticate_market_lifecycle_root_v2, authenticate_registry_capability_v4,
    authenticate_series_market_link_v2, authenticate_series_registry_account_v3,
};
use clutch_product_series::{CompiledProductSeriesBundleV6, MarketLifecyclePhaseV2, SeriesFundingQuoteV5, SeriesMarketLinkPhaseV2};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV2, SeriesMarketLinkAccountV2,
};
use clutch_solana_layout::registry::RecoveryAction;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// Persist one deterministic exhausted session without touching Recovery.
#[allow(clippy::too_many_lines)]
pub(crate) fn process_archive_failure_market_session_v3(
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
    require_distinct(accounts)?;
    let root_account = account_for_role_v2(action, accounts, Role::MarketLifecycleRoot)?;
    let link_account = account_for_role_v2(action, accounts, Role::SeriesMarketLink)?;
    let admission_account = account_for_role_v2(action, accounts, Role::FailureAdmissionRoot)?;
    let runtime_account = account_for_role_v2(action, accounts, Role::FailureRuntimeRoot)?;
    let cell_account = account_for_role_v2(action, accounts, Role::FailureIntervalCell)?;
    let history_account = account_for_role_v2(action, accounts, Role::FailureIntervalHistory)?;
    let registry_account = account_for_role_v2(action, accounts, Role::SeriesRegistry)?;
    let registry_program = account_for_role_v2(action, accounts, Role::RegistryProgram)?;
    let registry_programdata = account_for_role_v2(action, accounts, Role::RegistryProgramData)?;
    let registry_release = account_for_role_v2(action, accounts, Role::RegistryReleaseArtifact)?;
    let capability_profile =
        account_for_role_v2(action, accounts, Role::CapabilityProfileArtifact)?;
    let compiler_bundle = account_for_role_v2(action, accounts, Role::CompilerBundleArtifact)?;
    let funding_quote = account_for_role_v2(action, accounts, Role::FundingQuoteArtifact)?;
    let liveness_policy = account_for_role_v2(action, accounts, Role::FailureLivenessPolicy)?;
    let recovery = account_for_role_v2(action, accounts, Role::FailureRecoveryCompartment)?;

    let admission = authenticate_failure_market_root_v2(program_id, admission_account, false)?;
    let policy = admission.state().binding().facts();
    let mut root_decode = Box::new(MarketLifecycleRootAccountV2::decode_buffer());
    let root = authenticate_market_lifecycle_root_v2(
        program_id,
        root_account,
        policy.market_instance_id,
        policy.generation,
        false,
        &mut root_decode,
    )?;
    let root_binding = root.state().binding();
    let root_binding_id = root_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut link_decode = Box::new(SeriesMarketLinkAccountV2::decode_buffer());
    let link_data = link_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    SeriesMarketLinkAccountV2::decode_into(&link_data, &mut link_decode)?;
    drop(link_data);
    let decoded_link = link_decode.state.binding();
    let link = authenticate_series_market_link_v2(
        program_id,
        link_account,
        decoded_link.series_plan_id,
        decoded_link.ordinal,
        policy.market_instance_id,
        policy.generation,
        *root_account.key,
        true,
        &mut link_decode,
    )?;
    let link_binding = link.state().binding();
    require(
        root.state().phase() == MarketLifecyclePhaseV2::Active
            && link.state().phase() == SeriesMarketLinkPhaseV2::Active
            && link.state().active_failure_sessions() == 1
            && link.state().failure_sessions_started() != 0
            && !link.state().failure_session_transcript_id().is_zero()
            && link_binding.market_binding_id == root_binding_id
            && link_binding.market_instance_id == root_binding.market_instance_id
            && link_binding.generation == root_binding.generation
            && link_binding.market_root_account_id.bytes() == root_account.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let registry_account = authenticate_series_registry_account_v3(
        program_id,
        registry_account,
        link_binding.series_plan_id,
        false,
    )?;
    let registry = authenticate_registry_capability_v4(
        program_id,
        registry_account,
        registry_program,
        registry_programdata,
        registry_release,
        capability_profile,
    )?;
    let bundle = authenticate_product_artifact_v1::<CompiledProductSeriesBundleV6>(
        program_id,
        compiler_bundle,
        registry.compiler_bundle_id().content_id(),
    )?;
    let quote = authenticate_product_artifact_v1::<SeriesFundingQuoteV5>(
        program_id,
        funding_quote,
        bundle.value().funding_quote_id.content_id(),
    )?;
    require(
        registry.registry_release_id() == root_binding.registry_release_id
            && registry.capability_profile_id() == root_binding.capability_profile_id
            && bundle.semantic_id() == registry.compiler_bundle_id().content_id()
            && bundle.value().registry_release_id == registry.registry_release_id()
            && bundle.value().capability_profile_id.bytes()
                == registry.capability_profile_id().bytes()
            && bundle.value().series_plan_id == link_binding.series_plan_id
            && bundle.value().funding_quote_id == link_binding.funding_quote_id
            && bundle.semantic_id().bytes() == link_binding.compiler_bundle_id.bytes()
            && quote.semantic_id().bytes() == link_binding.funding_quote_id.bytes()
            && link_binding.capability_profile_id == registry.capability_profile_id()
            && quote.value().failure_liveness_policy_id
                == root_binding.failure_liveness_policy_id
            && quote.value().failure_liveness_policy_id.bytes()
                == policy.liveness_policy_id.bytes()
            && quote.value().failure_recovery_quote_schedule_id
                == root_binding.failure_liveness_quote_schedule_id
            && quote.value().failure_recovery_quote_schedule_id.bytes()
                == policy.recovery_quote_schedule_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    let admitted_quote = authenticate_failure_market_recovery_quote_v2(
        program_id,
        admission,
        root,
        &registry,
        liveness_policy,
        recovery_quote_schedule,
    )?;
    let funding = FailureMarketIntervalFundingPreimageV2::decode(interval_funding_preimage)?;
    let interval = reopen_failure_market_interval_accounts_v2(
        program_id,
        cell_account,
        history_account,
        admission,
        admitted_quote.receipt(),
        funding,
        true,
        true,
    )?;
    let runtime = authenticate_failure_market_runtime_root_v1(
        program_id,
        admission_account,
        runtime_account,
        admission,
        true,
    )?;
    let expected_sequence = runtime
        .state()
        .transition_sequence()
        .checked_add(1)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        sequence == expected_sequence
            && interval.cell().session_binding_id().bytes()
                == link.state().failure_session_transcript_id().bytes(),
        ClutchError::Replay,
    )?;

    let mut live_root = Box::new(MarketLifecycleRootAccountV2::decode_buffer());
    let mut live_link = Box::new(SeriesMarketLinkAccountV2::decode_buffer());
    let mut rebound_link = Box::new(SeriesMarketLinkAccountV2::decode_buffer());
    let _ = exhaust_and_archive_failure_market_interval_session_v3(
        program_id,
        root_account,
        link_account,
        admission_account,
        runtime_account,
        cell_account,
        history_account,
        liveness_policy,
        recovery,
        root,
        link,
        admission,
        runtime,
        interval,
        &mut live_root,
        &mut live_link,
        &mut rebound_link,
    )?;
    Ok(())
}

#[cfg(test)]
mod adversarial_tests {
    #[test]
    fn action13_is_current_exhaustion_only_and_runtime_last() {
        let source = include_str!("failure_market_action13_current.rs");
        let production = source.split("#[cfg(test)]").next().expect("production");
        for required in [
            "require_distinct(accounts)",
            "authenticate_market_lifecycle_root_v2",
            "authenticate_series_market_link_v2",
            "authenticate_registry_capability_v4",
            "CompiledProductSeriesBundleV6",
            "SeriesFundingQuoteV5",
            "authenticate_failure_market_recovery_quote_v2",
            "exhaust_and_archive_failure_market_interval_session_v3",
        ] {
            assert!(production.contains(required), "missing current authority {required}");
        }
        for forbidden in [
            concat!("MarketLifecycleRootAccount", "V1"),
            concat!("SeriesMarketLinkAccount", "V1"),
            concat!("AuthenticatedRegistryCapability", "V3"),
            "resolve_failure_market_interval_and_source_v5",
            "RecoveryCompartmentPhaseV1::Closed",
        ] {
            assert!(!production.contains(forbidden), "historical/terminal path {forbidden}");
        }
    }

    #[test]
    fn action13_binds_current_product_graph_and_exact_active_pin() {
        let source = include_str!("failure_market_action13_current.rs");
        for predicate in [
            "link.state().active_failure_sessions() == 1",
            "link.state().failure_session_transcript_id().is_zero()",
            "bundle.value().series_plan_id == link_binding.series_plan_id",
            "bundle.value().funding_quote_id == link_binding.funding_quote_id",
            "bundle.semantic_id().bytes() == link_binding.compiler_bundle_id.bytes()",
            "quote.value().failure_recovery_quote_schedule_id",
            "interval.cell().session_binding_id()",
            "link.state().failure_session_transcript_id()",
        ] {
            assert!(source.contains(predicate), "missing substitution guard {predicate}");
        }
    }
}
