// SPDX-License-Identifier: AGPL-3.0-or-later
//! Sole current Recovery78/action11 paid-advance owner.
//!
//! The checked handler hostile-reopens RootV2, the pinned read-only LinkV2,
//! RegistryV4, the exact current compiled schedule provenance, persisted Source
//! success, the reusable Failure interval pair, and the sole Recovery custody.
//! It then invokes one atomic Recovery-payment -> cell -> shared-runtime writer.
//! No RootV1, LinkV1, BundleV5, QuoteV4, or caller schedule identity is accepted.

use std::boxed::Box;

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::failure_market_admission::authenticate_failure_market_root_v2;
use crate::instructions::failure_market_dispatch_v2::{
    account_for_role_v2, FailureMarketAccountRoleV2 as Role, FailureMarketActionPayloadV2,
};
use crate::instructions::failure_market_interval_advance_v2::advance_failure_market_interval_paid_v2;
use crate::instructions::failure_market_interval_v2::{
    authenticate_failure_market_recovery_quote_for_resolution_v2,
    reopen_failure_market_interval_accounts_v2, FailureMarketIntervalFundingPreimageV2,
};
use crate::instructions::failure_market_runtime::authenticate_failure_market_runtime_root_v1;
use crate::instructions::product_artifact::authenticate_product_artifact_v1;
use crate::instructions::product_failure_begin_current::authenticate_product_failure_active_schedule_v2;
use crate::instructions::product_series_current::{
    authenticate_market_lifecycle_root_v2, authenticate_registry_capability_v4,
    authenticate_series_market_link_v2, authenticate_series_registry_account_v3,
};
use crate::source_plane_v3::{
    authenticate_release, authenticate_route,
    authenticate_successful_source_handoff_from_accounts_v1,
};
use crate::source_plane_v3_actions::authenticate_source_work_schedule_artifact;
use clutch_product_series::{
    MarketGenesisProfileV2, MarketInstancePreimageV2, NativeClaimBasisV1,
    PriceMeasurePolicyV1, ProductTemplateV4, QuantizedIntervalConsensusContextV1,
    QuantizedIntervalConsensusProfileV1,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV2, SeriesMarketLinkAccountV2,
};
use clutch_solana_layout::registry::RecoveryAction;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

fn require_advance_aliases(
    accounts: &[AccountInfo<'_>],
    keeper: &AccountInfo<'_>,
    refund: &AccountInfo<'_>,
) -> Outcome<()> {
    let mut left = 0usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            if accounts[left].key == accounts[right].key {
                let allowed = (core::ptr::eq(&accounts[left], keeper)
                    && core::ptr::eq(&accounts[right], refund))
                    || (core::ptr::eq(&accounts[left], refund)
                        && core::ptr::eq(&accounts[right], keeper));
                require(allowed, ClutchError::AccountAlias)?;
            }
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

/// Apply one exact current priced progress step through Recovery custody.
#[allow(clippy::too_many_lines)]
pub(crate) fn process_advance_failure_market_session_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: FailureMarketActionPayloadV2<'_>,
) -> Outcome<()> {
    let FailureMarketActionPayloadV2::Advance {
        requested_coordinates,
        recovery_quote_schedule,
        interval_funding_preimage,
    } = payload
    else {
        return crate::instructions::failure_market_dispatch_v2::process_reserved_disabled(
            RecoveryAction::AdvanceIntervalConsensus,
        );
    };
    let action = RecoveryAction::AdvanceIntervalConsensus;
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
    let series_plan = account_for_role_v2(action, accounts, Role::SeriesPlanArtifact)?;
    let template = account_for_role_v2(action, accounts, Role::ProductTemplateArtifact)?;
    let basis = account_for_role_v2(action, accounts, Role::NativeClaimBasisArtifact)?;
    let recovery_policy = account_for_role_v2(action, accounts, Role::RecoveryPolicyArtifact)?;
    let price = account_for_role_v2(action, accounts, Role::PriceMeasurePolicyArtifact)?;
    let genesis = account_for_role_v2(action, accounts, Role::MarketGenesisArtifact)?;
    let attachment = account_for_role_v2(action, accounts, Role::AttachmentPlanArtifact)?;
    let market = account_for_role_v2(action, accounts, Role::MarketInstanceArtifact)?;
    let source_release = account_for_role_v2(action, accounts, Role::SourceRelease)?;
    let source_adapter = account_for_role_v2(action, accounts, Role::SourceAdapterProgram)?;
    let source_adapter_data =
        account_for_role_v2(action, accounts, Role::SourceAdapterProgramData)?;
    let source_parser = account_for_role_v2(action, accounts, Role::SourceParserProgram)?;
    let source_parser_data =
        account_for_role_v2(action, accounts, Role::SourceParserProgramData)?;
    let source_parser_config = account_for_role_v2(action, accounts, Role::SourceParserConfig)?;
    let source_spec = account_for_role_v2(action, accounts, Role::SourceSpec)?;
    let source_work_schedule = account_for_role_v2(action, accounts, Role::SourceWorkSchedule)?;
    let source_occurrence = account_for_role_v2(action, accounts, Role::SourceOccurrence)?;
    let source_window = account_for_role_v2(action, accounts, Role::SourceWindowArtifact)?;
    let source_key = account_for_role_v2(action, accounts, Role::SourceStatisticKeyArtifact)?;
    let source_summary = account_for_role_v2(action, accounts, Role::SourceSummaryArtifact)?;
    let source_seal = account_for_role_v2(action, accounts, Role::SourceWindowSeal)?;
    let source_result = account_for_role_v2(action, accounts, Role::SourceStatisticResult)?;
    let source_lineage = account_for_role_v2(action, accounts, Role::SourceResultLineage)?;
    let source_handoff = account_for_role_v2(action, accounts, Role::SourceHandoffReceipt)?;
    let source_work_receipt = account_for_role_v2(action, accounts, Role::SourceWorkReceipt)?;
    let liveness_policy = account_for_role_v2(action, accounts, Role::FailureLivenessPolicy)?;
    let recovery = account_for_role_v2(action, accounts, Role::FailureRecoveryCompartment)?;
    let keeper = account_for_role_v2(action, accounts, Role::Keeper)?;
    let refund = account_for_role_v2(action, accounts, Role::RecoveryRefundOwner)?;
    require_advance_aliases(accounts, keeper, refund)?;

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
        false,
        &mut link_decode,
    )?;
    let link_binding = link.state().binding();
    let root_binding_id = root_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        link_binding.market_binding_id == root_binding_id
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
    let quote = authenticate_failure_market_recovery_quote_for_resolution_v2(
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
        quote.receipt(),
        funding,
        true,
        false,
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
    require(sequence == expected_sequence, ClutchError::Replay)?;

    let route = authenticate_route(
        program_id,
        source_release,
        source_adapter,
        source_adapter_data,
        source_parser,
        source_parser_data,
        source_parser_config,
        source_spec,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let _release = authenticate_release(program_id, source_release)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let source_schedule =
        authenticate_source_work_schedule_artifact(program_id, route, source_work_schedule)?;
    let successful = authenticate_successful_source_handoff_from_accounts_v1(
        program_id,
        route,
        source_schedule,
        source_occurrence,
        source_window,
        source_key,
        source_summary,
        source_seal,
        source_result,
        source_lineage,
        source_handoff,
        source_work_receipt,
    )?;
    let attempt_index = interval.cell().attempt_index();
    let mut schedule_root = Box::new(MarketLifecycleRootAccountV2::decode_buffer());
    let mut schedule_link = Box::new(SeriesMarketLinkAccountV2::decode_buffer());
    let product_schedule = authenticate_product_failure_active_schedule_v2(
        program_id,
        root_account,
        link_account,
        root,
        link,
        &registry,
        compiler_bundle,
        funding_quote,
        series_plan,
        template,
        basis,
        recovery_policy,
        price,
        genesis,
        attachment,
        market,
        &quote,
        attempt_index,
        &mut schedule_root,
        &mut schedule_link,
    )?;

    let template_value = authenticate_product_artifact_v1::<ProductTemplateV4>(
        program_id,
        template,
        root_binding.product_template_id,
    )?;
    let basis_value = authenticate_product_artifact_v1::<NativeClaimBasisV1>(
        program_id,
        basis,
        root_binding.native_claim_basis_id,
    )?;
    let price_value = authenticate_product_artifact_v1::<PriceMeasurePolicyV1>(
        program_id,
        price,
        root_binding.price_measure_policy_id,
    )?;
    let genesis_value = authenticate_product_artifact_v1::<MarketGenesisProfileV2>(
        program_id,
        genesis,
        root_binding.market_genesis_profile_id,
    )?;
    let market_value = authenticate_product_artifact_v1::<MarketInstancePreimageV2>(
        program_id,
        market,
        root_binding.market_instance_id.content_id(),
    )?;
    let projection = registry.projection();
    let work_profile = QuantizedIntervalConsensusProfileV1 {
        capability_profile_id: projection.capability_profile_id,
        maximum_interval_width: projection.maximum_interval_width,
        maximum_coordinates_per_advance: projection.maximum_coordinates_per_advance,
    };
    let work_profile_id = work_profile
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        template_value.semantic_id() == root_binding.product_template_id
            && basis_value.semantic_id() == root_binding.native_claim_basis_id
            && price_value.semantic_id() == root_binding.price_measure_policy_id
            && genesis_value.semantic_id() == root_binding.market_genesis_profile_id
            && market_value.semantic_id() == root_binding.market_instance_id.content_id()
            && work_profile_id.bytes() == root_binding.interval_consensus_profile_id.bytes()
            && work_profile_id.bytes() == policy.interval_consensus_profile_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    let source_interval = successful.interval();
    let occurrence = source_interval.occurrence();
    let statistic_result = source_interval.statistic_result();
    let statistic_key = source_interval.statistic_key();
    let summary = source_interval.summary_program();
    let seal = source_interval.window_seal();
    let window = source_interval.window();
    let context = QuantizedIntervalConsensusContextV1 {
        market: market_value.value(),
        product_template: template_value.value(),
        native_claim_basis: basis_value.value(),
        price_measure_policy: price_value.value(),
        market_genesis: genesis_value.value(),
        resolved_edge_policy: projection.resolved_edge_policy,
        source_occurrence: &occurrence,
        source_interval: &statistic_result,
        statistic_key: &statistic_key,
        summary_program: &summary,
        window_seal: &seal,
        window: &window,
        work_profile: &work_profile,
    };
    let mut live_root = Box::new(MarketLifecycleRootAccountV2::decode_buffer());
    let mut live_link = Box::new(SeriesMarketLinkAccountV2::decode_buffer());
    let _ = advance_failure_market_interval_paid_v2(
        program_id,
        root_account,
        link_account,
        cell_account,
        history_account,
        admission_account,
        runtime_account,
        liveness_policy,
        recovery,
        keeper,
        refund,
        &registry,
        &product_schedule,
        root,
        link,
        admission,
        runtime,
        interval,
        successful.join(),
        successful.handoff(),
        context,
        requested_coordinates,
        &mut live_root,
        &mut live_link,
    )?;
    Ok(())
}

#[cfg(test)]
mod adversarial_tests {
    #[test]
    fn current_action11_has_one_rootv2_paid_writer_and_no_lowering() {
        let source = include_str!("failure_market_action11_current.rs");
        let production = source.split("#[cfg(test)]").next().expect("production");
        assert_eq!(production.matches("advance_failure_market_interval_paid_v2(").count(), 1);
        for required in [
            "authenticate_market_lifecycle_root_v2",
            "authenticate_series_market_link_v2",
            "authenticate_registry_capability_v4",
            "authenticate_product_failure_active_schedule_v2",
            "authenticate_successful_source_handoff_from_accounts_v1",
        ] {
            assert!(production.contains(required), "missing current authority {required}");
        }
        for forbidden in [
            concat!("MarketLifecycleRootAccount", "V1"),
            concat!("SeriesMarketLinkAccount", "V1"),
            concat!("AuthenticatedRegistryCapability", "V3"),
            concat!("authenticate_product_failure_begin_schedule_", "v1"),
        ] {
            assert!(!production.contains(forbidden), "historical authority {forbidden}");
        }
    }

    #[test]
    fn current_action11_refuses_aliases_except_keeper_refund_union() {
        let source = include_str!("failure_market_action11_current.rs");
        assert!(source.contains("require_advance_aliases(accounts, keeper, refund)"));
        assert!(source.contains("core::ptr::eq(&accounts[left], keeper)"));
        assert!(source.contains("core::ptr::eq(&accounts[right], refund)"));
    }
}
