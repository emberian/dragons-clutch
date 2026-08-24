// SPDX-License-Identifier: AGPL-3.0-or-later
//! Sole current Recovery78/action12 RootV2/Product/Source/Failure owner.
//!
//! This handler accepts no historical RootV1, LinkV1, RegistryV3 receipt,
//! BundleV5, QuoteV4, AttachmentV4, GraphV2, or Source ResolutionInputV3. It
//! hostile-reconstructs every current semantic owner and calls the one atomic
//! ResolutionV5 -> Source terminal -> LinkV2 release -> Recovery close ->
//! durable Failure-family composer.

use std::boxed::Box;

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::collateral_position_v3::authenticate_general_market_liabilities_v2;
use crate::instructions::failure_market_admission::authenticate_failure_market_root_v3;
use crate::instructions::failure_market_dispatch_v2::{
    account_for_role_v2, FailureMarketAccountRoleV2 as Role, FailureMarketActionPayloadV2,
};
use crate::instructions::failure_market_interval_v2::{
    authenticate_failure_market_recovery_quote_for_resolution_v2,
    reopen_failure_market_interval_accounts_v2, FailureMarketIntervalFundingPreimageV2,
};
use crate::instructions::failure_market_replay_v2::{
    reopen_failure_market_replay_v2, FailureMarketReplayFundingPreimageV2,
};
use crate::instructions::failure_market_resolution_v5::resolve_failure_market_interval_and_source_v5;
use crate::instructions::failure_market_runtime::authenticate_failure_market_runtime_root_v1;
use crate::instructions::product_artifact::authenticate_product_artifact_v1;
use crate::instructions::product_series_current::{
    authenticate_market_foundation_preallocation_from_bytes_v3,
    authenticate_market_lifecycle_root_v2, authenticate_registry_capability_v4,
    authenticate_series_market_link_v2, authenticate_series_registry_account_v3,
};
use crate::instructions::product_source_current::{
    authenticate_compiled_product_series_bundle_v6, authenticate_series_source_artifacts_v5,
    authenticate_source_product_route_v4, authenticate_source_resolution_input_v4,
};
use crate::source_plane_v3::{
    authenticate_receiver_route, authenticate_release, authenticate_route,
    authenticate_successful_source_handoff_for_resolution_v1,
};
use crate::source_plane_v3_actions::authenticate_source_work_schedule_artifact;
use clutch_product_series::{
    MarketFoundationSlotV3, MarketGenesisProfileV2, MarketInstancePreimageV2,
    MarketLifecyclePhaseV2, NativeClaimBasisV1, PriceMeasurePolicyV1, ProductTemplateV4,
    QuantizedIntervalConsensusContextV1, QuantizedIntervalConsensusProfileV1,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV2, SeriesMarketLinkAccountV2,
};
use clutch_solana_layout::registry::RecoveryAction;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// Refuse every alias except the one exact lifecycle-custody/refund union.
fn require_current_resolution_aliases(
    accounts: &[AccountInfo<'_>],
    source_custody: &AccountInfo<'_>,
    recovery_refund: &AccountInfo<'_>,
) -> Outcome<()> {
    let mut left = 0usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            if accounts[left].key == accounts[right].key {
                let allowed = (core::ptr::eq(&accounts[left], source_custody)
                    && core::ptr::eq(&accounts[right], recovery_refund))
                    || (core::ptr::eq(&accounts[left], recovery_refund)
                        && core::ptr::eq(&accounts[right], source_custody));
                require(allowed, ClutchError::AccountAlias)?;
            }
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

/// Resolve one exact current successful Source handoff and persist every
/// dependent Product, Source, collateral, Failure, Recovery, and replay
/// poststate in one Solana instruction.
#[allow(clippy::too_many_lines)]
pub(crate) fn process_resolve_failure_market_session_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: FailureMarketActionPayloadV2<'_>,
) -> Outcome<()> {
    let FailureMarketActionPayloadV2::Resolve {
        replay_funding_preimage,
        foundation_account_graph,
    } = payload
    else {
        return crate::instructions::failure_market_dispatch_v2::process_reserved_disabled(
            RecoveryAction::ResolveIntervalConsensus,
        );
    };
    let action = RecoveryAction::ResolveIntervalConsensus;
    let root_account = account_for_role_v2(action, accounts, Role::MarketLifecycleRoot)?;
    let link_account = account_for_role_v2(action, accounts, Role::SeriesMarketLink)?;
    let admission_account = account_for_role_v2(action, accounts, Role::FailureAdmissionRoot)?;
    let runtime_account = account_for_role_v2(action, accounts, Role::FailureRuntimeRoot)?;
    let cell_account = account_for_role_v2(action, accounts, Role::FailureIntervalCell)?;
    let history_account = account_for_role_v2(action, accounts, Role::FailureIntervalHistory)?;
    let replay_account = account_for_role_v2(action, accounts, Role::FailureMarketReplay)?;
    let registry_account = account_for_role_v2(action, accounts, Role::SeriesRegistry)?;
    let registry_program = account_for_role_v2(action, accounts, Role::RegistryProgram)?;
    let registry_programdata = account_for_role_v2(action, accounts, Role::RegistryProgramData)?;
    let registry_release = account_for_role_v2(action, accounts, Role::RegistryReleaseArtifact)?;
    let capability_profile =
        account_for_role_v2(action, accounts, Role::CapabilityProfileArtifact)?;
    let compiler_bundle = account_for_role_v2(action, accounts, Role::CompilerBundleArtifact)?;
    let funding_quote = account_for_role_v2(action, accounts, Role::FundingQuoteArtifact)?;
    let series_plan = account_for_role_v2(action, accounts, Role::SeriesPlanArtifact)?;
    let funding_terms = account_for_role_v2(action, accounts, Role::SeriesFundingTermsArtifact)?;
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
    let receiver_program = account_for_role_v2(action, accounts, Role::SourceReceiverProgram)?;
    let receiver_programdata =
        account_for_role_v2(action, accounts, Role::SourceReceiverProgramData)?;
    let receiver_config = account_for_role_v2(action, accounts, Role::SourceReceiverConfig)?;
    let source_occurrence = account_for_role_v2(action, accounts, Role::SourceOccurrence)?;
    let source_window = account_for_role_v2(action, accounts, Role::SourceWindowArtifact)?;
    let source_key = account_for_role_v2(action, accounts, Role::SourceStatisticKeyArtifact)?;
    let source_summary = account_for_role_v2(action, accounts, Role::SourceSummaryArtifact)?;
    let source_seal = account_for_role_v2(action, accounts, Role::SourceWindowSeal)?;
    let source_result = account_for_role_v2(action, accounts, Role::SourceStatisticResult)?;
    let source_lineage = account_for_role_v2(action, accounts, Role::SourceResultLineage)?;
    let source_handoff = account_for_role_v2(action, accounts, Role::SourceHandoffReceipt)?;
    let source_work_receipt = account_for_role_v2(action, accounts, Role::SourceWorkReceipt)?;
    let realm = account_for_role_v2(action, accounts, Role::Realm)?;
    let collateral_profile = account_for_role_v2(action, accounts, Role::CollateralProfile)?;
    let collateral_release =
        account_for_role_v2(action, accounts, Role::CollateralPolicyRelease)?;
    let collateral_token_program =
        account_for_role_v2(action, accounts, Role::CollateralTokenProgram)?;
    let general_market_binding =
        account_for_role_v2(action, accounts, Role::GeneralMarketBinding)?;
    let general_market_runtime =
        account_for_role_v2(action, accounts, Role::GeneralMarketRuntime)?;
    let resolution = account_for_role_v2(action, accounts, Role::ResolutionV5)?;
    let hoard = account_for_role_v2(action, accounts, Role::HoardV2)?;
    let claim_ledger = account_for_role_v2(action, accounts, Role::ClaimLedgerV3)?;
    let source_terminal_policy =
        account_for_role_v2(action, accounts, Role::SourceTerminalPolicy)?;
    let source_terminal_receipt =
        account_for_role_v2(action, accounts, Role::SourceTerminalReceipt)?;
    let source_liveness_policy =
        account_for_role_v2(action, accounts, Role::SourceLivenessPolicy)?;
    let source_liveness =
        account_for_role_v2(action, accounts, Role::SourceLivenessCompartment)?;
    let source_custody = account_for_role_v2(action, accounts, Role::SourceFundingCustody)?;
    let neutral_sink = account_for_role_v2(action, accounts, Role::SourceNeutralSink)?;
    let failure_liveness_policy =
        account_for_role_v2(action, accounts, Role::FailureLivenessPolicy)?;
    let recovery = account_for_role_v2(action, accounts, Role::FailureRecoveryCompartment)?;
    let recovery_refund = account_for_role_v2(action, accounts, Role::RecoveryRefundOwner)?;
    let rent = account_for_role_v2(action, accounts, Role::RentSysvar)?;
    let system_program = account_for_role_v2(action, accounts, Role::SystemProgram)?;
    require_current_resolution_aliases(accounts, source_custody, recovery_refund)?;

    let admission = authenticate_failure_market_root_v3(program_id, admission_account, false)?;
    let failure_policy = admission.state().binding().facts();
    let mut root_decode = Box::new(MarketLifecycleRootAccountV2::decode_buffer());
    let root = authenticate_market_lifecycle_root_v2(
        program_id,
        root_account,
        failure_policy.market_instance_id,
        failure_policy.generation,
        true,
        &mut root_decode,
    )?;
    let root_binding = root.state().binding();
    let root_binding_id = root_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        root.state().phase() == MarketLifecyclePhaseV2::Active
            && root.state().resolution_semantic_id() == clutch_product_series::ContentId::ZERO
            && root.state().resolution_data_id() == clutch_product_series::ContentId::ZERO
            && root.state().resolution_activation_receipt_id()
                == clutch_product_series::ContentId::ZERO
            && root_binding.market_failure_policy_binding_id.bytes()
                == admission.state().binding().id().bytes()
            && root_binding.market_instance_id == failure_policy.market_instance_id
            && root_binding.generation == failure_policy.generation,
        ClutchError::MismatchedState,
    )?;
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
        failure_policy.market_instance_id,
        failure_policy.generation,
        *root_account.key,
        true,
        &mut link_decode,
    )?;
    let link_binding = link.state().binding();
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
        failure_liveness_policy,
    )?;
    let interval_funding_preimage = admission.interval_funding_preimage();
    let interval_funding =
        FailureMarketIntervalFundingPreimageV2::decode(&interval_funding_preimage)?;
    let interval = reopen_failure_market_interval_accounts_v2(
        program_id,
        cell_account,
        history_account,
        admission,
        quote.receipt(),
        interval_funding,
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
    require(sequence == expected_sequence, ClutchError::Replay)?;
    let replay_preimage = FailureMarketReplayFundingPreimageV2::decode(replay_funding_preimage)?;
    let replay = reopen_failure_market_replay_v2(
        program_id,
        replay_account,
        admission,
        replay_preimage,
        true,
    )?;

    let route_release = authenticate_release(program_id, source_release)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
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
    let source_schedule = authenticate_source_work_schedule_artifact(
        program_id,
        route,
        source_work_schedule,
    )?;
    let receiver = authenticate_receiver_route(
        route,
        receiver_program,
        receiver_programdata,
        receiver_config,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let successful = authenticate_successful_source_handoff_for_resolution_v1(
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
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let source_artifact_accounts = [
        series_plan.clone(),
        funding_terms.clone(),
        template.clone(),
        basis.clone(),
        recovery_policy.clone(),
        price.clone(),
        genesis.clone(),
        funding_quote.clone(),
        attachment.clone(),
    ];
    let artifacts = authenticate_series_source_artifacts_v5(
        program_id,
        &source_artifact_accounts,
        link_binding.series_plan_id,
        registry.funding_terms_id(),
    )?;
    let bundle = authenticate_compiled_product_series_bundle_v6(
        program_id,
        compiler_bundle,
        &registry,
        route_release,
        &artifacts,
    )?;
    let source_product =
        authenticate_source_product_route_v4(route, receiver, &registry, bundle, &artifacts)?;
    let source_input = authenticate_source_resolution_input_v4(
        source_product,
        successful.handoff(),
        successful.join(),
        successful.persisted(),
    )?;
    let slot10 = authenticate_market_foundation_preallocation_from_bytes_v3(
        root,
        resolution,
        &artifacts.quote().foundation,
        foundation_account_graph,
        MarketFoundationSlotV3::ResolutionV5,
    )?;

    let liabilities = authenticate_general_market_liabilities_v2(
        program_id,
        realm,
        collateral_profile,
        collateral_release,
        collateral_token_program,
        general_market_binding,
        general_market_runtime,
        market,
        hoard,
        claim_ledger,
        true,
        true,
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
            && work_profile_id.bytes() == failure_policy.interval_consensus_profile_id.bytes()
            && quote.receipt().id() == interval.quote().id(),
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

    let mut resolution_root_before = Box::new(MarketLifecycleRootAccountV2::decode_buffer());
    let mut resolution_link_before = Box::new(SeriesMarketLinkAccountV2::decode_buffer());
    let mut resolution_root_after = Box::new(MarketLifecycleRootAccountV2::decode_buffer());
    let mut link_release = Box::new(SeriesMarketLinkAccountV2::decode_buffer());
    let mut link_rebound = Box::new(SeriesMarketLinkAccountV2::decode_buffer());
    let mut resolved_root = Box::new(MarketLifecycleRootAccountV2::decode_buffer());
    let mut persisted_root = Box::new(MarketLifecycleRootAccountV2::decode_buffer());
    let _ = resolve_failure_market_interval_and_source_v5(
        program_id,
        admission_account,
        runtime_account,
        root_account,
        link_account,
        cell_account,
        history_account,
        resolution,
        hoard,
        claim_ledger,
        replay_account,
        source_result,
        source_lineage,
        source_terminal_policy,
        source_terminal_receipt,
        source_liveness_policy,
        source_liveness,
        source_custody,
        neutral_sink,
        source_custody,
        failure_liveness_policy,
        recovery,
        recovery_refund,
        rent,
        system_program,
        root,
        link,
        admission,
        runtime,
        interval,
        replay,
        &registry,
        bundle,
        slot10,
        liabilities,
        route,
        source_schedule,
        successful.handoff(),
        source_input,
        successful.lineage(),
        context,
        &mut resolution_root_before,
        &mut resolution_link_before,
        &mut resolution_root_after,
        &mut link_release,
        &mut link_rebound,
        &mut resolved_root,
        &mut persisted_root,
    )?;
    Ok(())
}

#[cfg(test)]
mod adversarial_tests {
    #[test]
    fn current_action12_has_one_v4_rootv2_composer_callsite_and_no_lowering() {
        let source = include_str!("failure_market_action12_current.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("production source");
        assert_eq!(
            production
                .matches("resolve_failure_market_interval_and_source_v5(")
                .count(),
            1
        );
        for forbidden in [
            concat!("MarketLifecycleRootAccount", "V1"),
            concat!("SeriesMarketLinkAccount", "V1"),
            concat!("authenticate_source_resolution_input_", "v3"),
            concat!("CompiledProductSeriesBundle", "V5"),
            concat!("SeriesFundingQuote", "V4"),
            concat!("SeriesAttachmentPlan", "V4"),
            concat!("MarketFoundationSlot", "V2"),
            concat!("authenticate_market_foundation_preallocation_", "v2"),
        ] {
            assert!(
                !production.contains(forbidden),
                "historical authority: {forbidden}"
            );
        }
        for required in [
            "authenticate_source_resolution_input_v4",
            "authenticate_market_lifecycle_root_v2",
            "authenticate_series_market_link_v2",
            "authenticate_registry_capability_v4",
            "authenticate_compiled_product_series_bundle_v6",
            "authenticate_market_foundation_preallocation_from_bytes_v3",
            "MarketFoundationSlotV3::ResolutionV5",
        ] {
            assert!(
                production.contains(required),
                "missing current owner: {required}"
            );
        }
    }

    #[test]
    fn current_action12_rejects_every_alias_except_exact_custody_refund_union() {
        let source = include_str!("failure_market_action12_current.rs");
        assert!(source.contains("require_current_resolution_aliases(accounts, source_custody, recovery_refund)"));
        assert!(source.contains("core::ptr::eq(&accounts[left], source_custody)"));
        assert!(source.contains("core::ptr::eq(&accounts[right], recovery_refund)"));
    }

    #[test]
    fn current_action12_order_keeps_runtime_and_family_writes_inside_atomic_outer() {
        let source = include_str!("failure_market_action12_current.rs");
        let call = source
            .find("resolve_failure_market_interval_and_source_v5(")
            .expect("sole atomic resolution call");
        let tail = &source[call..];
        assert!(tail.contains("&mut resolution_root_before"));
        assert!(tail.contains("&mut link_release"));
        assert!(tail.contains("&mut persisted_root"));
        assert!(tail.contains("Ok(())"));
    }
}
