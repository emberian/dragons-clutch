// SPDX-License-Identifier: AGPL-3.0-or-later
//! Concrete routed owners for current Market Failure actions 10 through 13.
//!
//! Each handler starts from the exact checked wire/account contract and then
//! hostile-reopens every semantic owner. This module contains no fallback DTO
//! path and never imports the withdrawn occurrence-scoped ExternalV2 runtime.

use std::boxed::Box;

use crate::accounts::{require, require_distinct, Outcome};
use crate::error::ClutchError;
use crate::instructions::failure_market_dispatch_v2::{
    account_for_role_v2, FailureMarketAccountRoleV2 as Role, FailureMarketActionPayloadV2,
};
use crate::instructions::failure_market_execution_v2::{
    authenticate_failure_market_execution_v2, authenticate_failure_market_product_context_v2,
    authenticate_failure_market_resolution_foundation_v2,
    authenticate_failure_market_source_product_route_v3,
    authenticate_failure_market_source_route_v2,
};
use crate::instructions::failure_market_interval_advance_v2::advance_failure_market_interval_paid_v2;
use crate::instructions::failure_market_interval_v2::exhaust_and_archive_failure_market_interval_session_v2;
use crate::instructions::failure_market_resolution_v5::resolve_failure_market_interval_and_source_v5;
use crate::instructions::collateral_position_v3::authenticate_general_market_liabilities_v2;
use crate::instructions::product_failure_begin::{
    authenticate_product_failure_begin_schedule_v1, begin_failure_market_interval_session_v2,
};
use crate::instructions::product_series::authenticate_source_resolution_input_v3;
use crate::source_plane_v3::authenticate_successful_source_handoff_from_accounts_v1;
use crate::source_plane_v3::authenticate_successful_source_handoff_for_resolution_v1;
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV1, SeriesMarketLinkAccountV1,
};
use clutch_solana_layout::registry::RecoveryAction;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// Refuse every alias except one deliberately unioned recipient pair.
///
/// Advance permits only Keeper==RecoveryRefundOwner. The inner liveness owner
/// reauthenticates that union's effective writable metadata and exact economic
/// identities; no third role may inherit it.
fn require_distinct_except_pair(
    accounts: &[AccountInfo<'_>],
    first: usize,
    second: usize,
) -> Outcome<()> {
    let mut left = 0usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            if accounts[left].key == accounts[right].key {
                require(
                    (left == first && right == second) || (left == second && right == first),
                    ClutchError::AccountAlias,
                )?;
            }
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

/// Refuse every alias involving protocol state while allowing only explicitly
/// named external recipient roles to share one System account.
fn require_protocol_distinct_with_external_aliases(
    accounts: &[AccountInfo<'_>],
    external_roles: &[&AccountInfo<'_>],
) -> Outcome<()> {
    let mut left = 0usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            if accounts[left].key == accounts[right].key {
                let left_external = external_roles
                    .iter()
                    .any(|account| core::ptr::eq(*account, &accounts[left]));
                let right_external = external_roles
                    .iter()
                    .any(|account| core::ptr::eq(*account, &accounts[right]));
                require(
                    left_external && right_external,
                    ClutchError::AccountAlias,
                )?;
            }
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

/// Begin one recurring shared-Market Failure interval from exact Product and
/// persisted Source authority, then atomically pin the initiating Series link.
#[allow(clippy::too_many_lines)]
pub(crate) fn process_begin_failure_market_session_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: FailureMarketActionPayloadV2<'_>,
) -> Outcome<()> {
    let FailureMarketActionPayloadV2::Begin {
        recovery_quote_schedule,
        interval_funding_preimage,
    } = payload
    else {
        return crate::instructions::failure_market_dispatch_v2::process_reserved_disabled(
            RecoveryAction::BeginIntervalConsensus,
        );
    };
    let action = RecoveryAction::BeginIntervalConsensus;
    require_distinct(accounts)?;
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
    let source_work_schedule =
        account_for_role_v2(action, accounts, Role::SourceWorkSchedule)?;
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
        false,
        &mut root_before,
        &mut link_before,
    )?;
    execution.require_next_sequence(sequence)?;
    let source = authenticate_failure_market_source_route_v2(
        program_id,
        &execution,
        source_release,
        source_adapter,
        source_adapter_data,
        source_parser,
        source_parser_data,
        source_parser_config,
        source_spec,
        source_work_schedule,
    )?;
    let successful = authenticate_successful_source_handoff_from_accounts_v1(
        program_id,
        source.route(),
        source.schedule(),
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
    let product = authenticate_failure_market_product_context_v2(
        program_id,
        &execution,
        template,
        basis,
        price,
        genesis,
        market,
    )?;
    let mut schedule_root = Box::new(MarketLifecycleRootAccountV1::decode_buffer());
    let mut schedule_link = Box::new(SeriesMarketLinkAccountV1::decode_buffer());
    let schedule = authenticate_product_failure_begin_schedule_v1(
        program_id,
        root_account,
        link_account,
        execution.root(),
        execution.link(),
        execution.registry(),
        compiler_bundle,
        series_plan,
        template,
        basis,
        recovery_policy,
        price,
        genesis,
        attachment,
        market,
        &mut schedule_root,
        &mut schedule_link,
    )?;
    let interval = successful.interval();
    let occurrence = interval.occurrence();
    let result = interval.statistic_result();
    let statistic_key = interval.statistic_key();
    let summary = interval.summary_program();
    let seal = interval.window_seal();
    let window = interval.window();
    let context = product.context(
        &occurrence,
        &result,
        &statistic_key,
        &summary,
        &seal,
        &window,
    );
    let mut root_rebound = Box::new(MarketLifecycleRootAccountV1::decode_buffer());
    let mut link_rebound = Box::new(SeriesMarketLinkAccountV1::decode_buffer());
    let _ = begin_failure_market_interval_session_v2(
        program_id,
        root_account,
        link_account,
        cell_account,
        history_account,
        admission_account,
        runtime_account,
        execution.root(),
        execution.link(),
        execution.admission(),
        execution.runtime(),
        execution.interval(),
        schedule,
        successful.join(),
        successful.handoff(),
        context,
        &mut root_rebound,
        &mut link_rebound,
    )?;
    Ok(())
}

/// Apply one exact priced progress step through the sole Recovery custody.
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
    let refund_index = accounts
        .len()
        .checked_sub(1)
        .ok_or(ClutchError::WrongAccountCount)?;
    let keeper_index = refund_index
        .checked_sub(1)
        .ok_or(ClutchError::WrongAccountCount)?;
    require_distinct_except_pair(accounts, keeper_index, refund_index)?;
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
    let template = account_for_role_v2(action, accounts, Role::ProductTemplateArtifact)?;
    let basis = account_for_role_v2(action, accounts, Role::NativeClaimBasisArtifact)?;
    let price = account_for_role_v2(action, accounts, Role::PriceMeasurePolicyArtifact)?;
    let genesis = account_for_role_v2(action, accounts, Role::MarketGenesisArtifact)?;
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
    let source_work_schedule =
        account_for_role_v2(action, accounts, Role::SourceWorkSchedule)?;
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
        false,
        true,
        false,
        &mut root_before,
        &mut link_before,
    )?;
    execution.require_next_sequence(sequence)?;
    let source = authenticate_failure_market_source_route_v2(
        program_id,
        &execution,
        source_release,
        source_adapter,
        source_adapter_data,
        source_parser,
        source_parser_data,
        source_parser_config,
        source_spec,
        source_work_schedule,
    )?;
    let successful = authenticate_successful_source_handoff_from_accounts_v1(
        program_id,
        source.route(),
        source.schedule(),
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
    let product = authenticate_failure_market_product_context_v2(
        program_id,
        &execution,
        template,
        basis,
        price,
        genesis,
        market,
    )?;
    let interval = successful.interval();
    let occurrence = interval.occurrence();
    let result = interval.statistic_result();
    let statistic_key = interval.statistic_key();
    let summary = interval.summary_program();
    let seal = interval.window_seal();
    let window = interval.window();
    let context = product.context(
        &occurrence,
        &result,
        &statistic_key,
        &summary,
        &seal,
        &window,
    );
    let mut root_reopen = Box::new(MarketLifecycleRootAccountV1::decode_buffer());
    let mut link_reopen = Box::new(SeriesMarketLinkAccountV1::decode_buffer());
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
        execution.registry(),
        execution.root(),
        execution.link(),
        execution.admission(),
        execution.runtime(),
        execution.interval(),
        successful.join(),
        successful.handoff(),
        context,
        requested_coordinates,
        &mut root_reopen,
        &mut link_reopen,
    )?;
    Ok(())
}

/// Resolve one interval and atomically finish its Source, Recovery, replay,
/// history, and durable Failure-family poststates.
#[allow(clippy::too_many_lines)]
pub(crate) fn process_resolve_failure_market_session_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: FailureMarketActionPayloadV2<'_>,
) -> Outcome<()> {
    let FailureMarketActionPayloadV2::Resolve {
        recovery_quote_schedule,
        interval_funding_preimage,
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
    let series_registry = account_for_role_v2(action, accounts, Role::SeriesRegistry)?;
    let registry_program = account_for_role_v2(action, accounts, Role::RegistryProgram)?;
    let registry_programdata = account_for_role_v2(action, accounts, Role::RegistryProgramData)?;
    let registry_release =
        account_for_role_v2(action, accounts, Role::RegistryReleaseArtifact)?;
    let capability_profile =
        account_for_role_v2(action, accounts, Role::CapabilityProfileArtifact)?;
    let compiler_bundle = account_for_role_v2(action, accounts, Role::CompilerBundleArtifact)?;
    let funding_quote = account_for_role_v2(action, accounts, Role::FundingQuoteArtifact)?;
    let template = account_for_role_v2(action, accounts, Role::ProductTemplateArtifact)?;
    let basis = account_for_role_v2(action, accounts, Role::NativeClaimBasisArtifact)?;
    let price = account_for_role_v2(action, accounts, Role::PriceMeasurePolicyArtifact)?;
    let genesis = account_for_role_v2(action, accounts, Role::MarketGenesisArtifact)?;
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
    let source_work_schedule =
        account_for_role_v2(action, accounts, Role::SourceWorkSchedule)?;
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
    let source_neutral_sink = account_for_role_v2(action, accounts, Role::SourceNeutralSink)?;
    let recovery_liveness_policy =
        account_for_role_v2(action, accounts, Role::FailureLivenessPolicy)?;
    let recovery = account_for_role_v2(action, accounts, Role::FailureRecoveryCompartment)?;
    let recovery_refund = account_for_role_v2(action, accounts, Role::RecoveryRefundOwner)?;
    let rent = account_for_role_v2(action, accounts, Role::RentSysvar)?;
    let system_program = account_for_role_v2(action, accounts, Role::SystemProgram)?;
    require_protocol_distinct_with_external_aliases(
        accounts,
        &[source_custody, recovery_refund],
    )?;

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
        recovery_liveness_policy,
        recovery_quote_schedule,
        interval_funding_preimage,
        true,
        true,
        true,
        true,
        &mut root_before,
        &mut link_before,
    )?;
    execution.require_next_sequence(sequence)?;
    let source = authenticate_failure_market_source_route_v2(
        program_id,
        &execution,
        source_release,
        source_adapter,
        source_adapter_data,
        source_parser,
        source_parser_data,
        source_parser_config,
        source_spec,
        source_work_schedule,
    )?;
    let successful = authenticate_successful_source_handoff_for_resolution_v1(
        program_id,
        source.route(),
        source.schedule(),
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
    let product = authenticate_failure_market_product_context_v2(
        program_id,
        &execution,
        template,
        basis,
        price,
        genesis,
        market,
    )?;
    let source_product = authenticate_failure_market_source_product_route_v3(
        &execution,
        source,
        &product,
        receiver_program,
        receiver_programdata,
        receiver_config,
    )?;
    let source_input = authenticate_source_resolution_input_v3(
        source_product,
        successful.handoff(),
        successful.join(),
        successful.persisted(),
    )?;
    let foundation = authenticate_failure_market_resolution_foundation_v2(
        program_id,
        &execution,
        replay_account,
        resolution,
        replay_funding_preimage,
        foundation_account_graph,
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
    let interval = successful.interval();
    let occurrence = interval.occurrence();
    let result = interval.statistic_result();
    let statistic_key = interval.statistic_key();
    let summary = interval.summary_program();
    let seal = interval.window_seal();
    let window = interval.window();
    let context = product.context(
        &occurrence,
        &result,
        &statistic_key,
        &summary,
        &seal,
        &window,
    );
    let root = execution.root();
    let admission = execution.admission();
    let runtime = execution.runtime();
    let failure_interval = execution.interval();
    let registry = execution.registry();
    let bundle = execution.into_bundle();
    let mut root_resolution_before = Box::new(MarketLifecycleRootAccountV1::decode_buffer());
    let mut link_resolution_before = Box::new(SeriesMarketLinkAccountV1::decode_buffer());
    let mut root_resolution_after = Box::new(MarketLifecycleRootAccountV1::decode_buffer());
    let mut link_release = Box::new(SeriesMarketLinkAccountV1::decode_buffer());
    let mut link_rebound = Box::new(SeriesMarketLinkAccountV1::decode_buffer());
    let mut root_resolved = Box::new(MarketLifecycleRootAccountV1::decode_buffer());
    let mut root_terminal = Box::new(MarketLifecycleRootAccountV1::decode_buffer());
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
        source_neutral_sink,
        source_custody,
        recovery_liveness_policy,
        recovery,
        recovery_refund,
        rent,
        system_program,
        root,
        admission,
        runtime,
        failure_interval,
        foundation.replay(),
        registry,
        bundle,
        foundation.resolution(),
        liabilities,
        source.route(),
        source.schedule(),
        successful.handoff(),
        source_input,
        successful.lineage(),
        context,
        &mut root_resolution_before,
        &mut link_resolution_before,
        &mut root_resolution_after,
        &mut link_release,
        &mut link_rebound,
        &mut root_resolved,
        &mut root_terminal,
    )?;
    Ok(())
}

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
    require_distinct(accounts)?;
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
    use crate::instructions::failure_market_dispatch_v2::{
        ADVANCE_FAILURE_MARKET_SESSION_METAS_V2, ARCHIVE_FAILURE_MARKET_SESSION_METAS_V2,
        BEGIN_FAILURE_MARKET_SESSION_METAS_V2, FailureMarketAccountMetaV2,
        RESOLVE_FAILURE_MARKET_SESSION_METAS_V2,
    };

    fn require_every_contract_role_is_consumed(
        handler: &str,
        contract: &[FailureMarketAccountMetaV2],
    ) {
        for meta in contract {
            let role = std::format!("Role::{:?}", meta.role);
            assert!(handler.contains(&role), "unconsumed account role {role}");
        }
    }

    #[test]
    fn begin_reconstructs_source_and_compiler_authority_before_atomic_pin() {
        let source = include_str!("failure_market_actions_v2.rs");
        let handler = source
            .split("fn process_begin_failure_market_session_v2")
            .nth(1)
            .and_then(|value| value.split("fn process_advance_failure_market_session_v2").next())
            .expect("action10 handler");
        for owner in [
            "require_distinct(accounts)",
            "authenticate_failure_market_execution_v2",
            "execution.require_next_sequence(sequence)",
            "authenticate_failure_market_source_route_v2",
            "authenticate_successful_source_handoff_from_accounts_v1",
            "authenticate_product_failure_begin_schedule_v1",
            "begin_failure_market_interval_session_v2",
        ] {
            assert!(handler.contains(owner));
        }
        require_every_contract_role_is_consumed(handler, BEGIN_FAILURE_MARKET_SESSION_METAS_V2);
        assert!(!handler.contains("ExternalRecoveryStateV1"));
    }

    #[test]
    fn advance_reconstructs_source_and_allows_only_keeper_refund_union() {
        let source = include_str!("failure_market_actions_v2.rs");
        let handler = source
            .split("fn process_advance_failure_market_session_v2")
            .nth(1)
            .and_then(|value| value.split("fn process_archive_failure_market_session_v2").next())
            .expect("action11 handler");
        for owner in [
            "require_distinct_except_pair(accounts, keeper_index, refund_index)",
            "authenticate_failure_market_execution_v2",
            "execution.require_next_sequence(sequence)",
            "authenticate_successful_source_handoff_from_accounts_v1",
            "authenticate_failure_market_product_context_v2",
            "advance_failure_market_interval_paid_v2",
        ] {
            assert!(handler.contains(owner));
        }
        require_every_contract_role_is_consumed(handler, ADVANCE_FAILURE_MARKET_SESSION_METAS_V2);
        assert!(!handler.contains("ExternalRecoveryStateV1"));
    }

    #[test]
    fn resolve_has_one_full_source_product_failure_terminal_outer() {
        let source = include_str!("failure_market_actions_v2.rs");
        let handler = source
            .split("fn process_resolve_failure_market_session_v2")
            .nth(1)
            .and_then(|value| value.split("fn process_archive_failure_market_session_v2").next())
            .expect("action12 handler");
        for owner in [
            "require_protocol_distinct_with_external_aliases",
            "authenticate_failure_market_execution_v2",
            "execution.require_next_sequence(sequence)",
            "authenticate_successful_source_handoff_for_resolution_v1",
            "authenticate_failure_market_source_product_route_v3",
            "authenticate_source_resolution_input_v3",
            "authenticate_failure_market_resolution_foundation_v2",
            "authenticate_general_market_liabilities_v2",
            "resolve_failure_market_interval_and_source_v5",
        ] {
            assert!(handler.contains(owner));
        }
        require_every_contract_role_is_consumed(handler, RESOLVE_FAILURE_MARKET_SESSION_METAS_V2);
        assert!(handler.matches("source_custody,").count() >= 2);
        assert!(!handler.contains("Role::NeutralSink"));
        assert!(!handler.contains("ExternalRecoveryStateV1"));
    }

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
        require_every_contract_role_is_consumed(handler, ARCHIVE_FAILURE_MARKET_SESSION_METAS_V2);
        assert!(handler.contains("false,\n        true,\n        true,\n        true,"));
        assert!(!handler.contains("close_failure_market_recovery_v2"));
        assert!(!handler.contains("ExternalRecoveryStateV1"));
    }
}
