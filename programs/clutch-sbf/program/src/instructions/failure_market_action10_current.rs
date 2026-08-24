// SPDX-License-Identifier: AGPL-3.0-or-later
//! Sole current Recovery78/action10 Product/Source/Failure owner.
//!
//! This handler starts from RootV2/LinkV2, RegistryV3/ProfileV4, BundleV6,
//! QuoteV5 and AttachmentV5. It reconstructs the persisted Source handoff from
//! accounts and requires the three success/absence/refusal predicates to be
//! disjoint. Absence and refusal are consumed through the non-detachable
//! Product pin -> Source terminal -> Failure archive -> Product release outer.

use std::boxed::Box;

use crate::accounts::{require, require_distinct, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::failure_market_admission::authenticate_failure_market_root_v3;
use crate::instructions::failure_market_dispatch_v2::{
    account_for_role_v2, FailureMarketAccountRoleV2 as Role, FailureMarketActionPayloadV2,
};
use crate::instructions::failure_market_interval_v2::{
    authenticate_failure_market_recovery_quote_v2, reopen_failure_market_interval_accounts_v2,
    write_failure_market_interval_begin_plan_v2, AuthenticatedFailureMarketIntervalAccountsV2,
    AuthenticatedFailureMarketIntervalBeginV2, FailureMarketIntervalFundingPreimageV2,
};
use crate::instructions::failure_market_runtime::{
    authenticate_failure_market_runtime_root_v1, write_failure_market_runtime_begin_plan_v2,
    AuthenticatedFailureMarketRuntimeRootV1,
    AuthenticatedFailureMarketRuntimeSessionPostwriteV1,
    AuthenticatedFailureMarketRuntimeSessionWriteV1, FailureMarketRuntimeSessionWriteFactsV1,
};
use crate::instructions::failure_market_source_failure_current::compose_failure_market_source_failure_attempt_v3;
use crate::instructions::product_artifact::authenticate_product_artifact_v1;
use crate::instructions::product_failure_begin_current::authenticate_product_failure_begin_schedule_v2;
use crate::instructions::product_series_current::{
    authenticate_market_lifecycle_root_v2, authenticate_registry_capability_v4,
    authenticate_series_market_link_v2, authenticate_series_registry_account_v3,
    pin_series_market_link_failure_v2, AuthenticatedMarketLifecycleRootV2,
    AuthenticatedRegistryCapabilityV4, AuthenticatedSeriesFailureSessionBeginV3,
    AuthenticatedSeriesFailureSessionPinV2, AuthenticatedSeriesMarketLinkV2,
};
use crate::source_plane_v3::{
    authenticate_failure_absence_source_handoff_for_terminal_v1,
    authenticate_failure_result_source_handoff_for_terminal_v1,
    authenticate_route, authenticate_successful_source_handoff_for_resolution_v1,
};
use crate::source_plane_v3_actions::authenticate_source_work_schedule_artifact;
use crate::instructions::source_failure_terminal_v1::AuthenticatedSourceFailureHandoffV1;
use clutch_failure_policy_runtime::market_interval_cell_v2::{
    plan_activate_failure_market_interval_cell_v2,
    AuthenticatedFailureMarketIntervalCellActivationV2, FailureMarketIntervalCellActivationFactsV2,
    FailureMarketIntervalCellActivationReceiptV2,
};
use clutch_failure_policy_runtime::market_runtime_v1::{
    plan_begin_failure_market_session_v2, AuthenticatedFailureMarketSessionBeginV2,
    FailureMarketSessionBeginFactsV2, FailureMarketSessionDescriptorV1,
    FailureMarketSessionScheduleIdV1,
};
use clutch_product_series::{
    begin_quantized_interval_consensus_v1, ContentId, MarketGenesisProfileV2,
    MarketInstancePreimageV2, MarketLifecyclePhaseV2, NativeClaimBasisV1,
    PriceMeasurePolicyV1, ProductTemplateV4, QuantizedIntervalConsensusContextV1,
    QuantizedIntervalConsensusProfileV1,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV2, SeriesMarketLinkAccountV2,
};
use clutch_solana_layout::registry::RecoveryAction;
use clutch_source_plane_v3_runtime::{AuthenticatedSourceRouteV1, SourceWorkScheduleBindingV1};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReconstructedAction10SourceV1 {
    Successful(crate::source_plane_v3::AuthenticatedSuccessfulSourceHandoffV1),
    Failure(AuthenticatedSourceFailureHandoffV1),
}

const CURRENT_SUCCESS_BEGIN_PREAUTHORIZATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/current-failure-success-begin-preauthorization/v2";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CurrentSuccessBeginPreauthorizationV2 {
    id: ContentId,
    root_account: Pubkey,
    root_authentication_id: ContentId,
    link_account: Pubkey,
    link_authentication_id: ContentId,
    series_plan_id: clutch_product_series::SeriesPlanV5Id,
    ordinal: u32,
    market_instance_id: clutch_product_series::MarketInstanceV2Id,
    generation: u64,
    source_occurrence_id: clutch_product_series::SourceOccurrenceV1Id,
    predicted_session_transcript_id: ContentId,
    activation_facts: FailureMarketIntervalCellActivationFactsV2,
}

impl AuthenticatedSeriesFailureSessionBeginV3 for CurrentSuccessBeginPreauthorizationV2 {
    fn authenticate_series_failure_session_begin_v3(
        &self,
        root_account: Pubkey,
        root_authentication_id: ContentId,
        link_account: Pubkey,
        link_authentication_id: ContentId,
        series_plan_id: clutch_product_series::SeriesPlanV5Id,
        ordinal: u32,
        market_instance_id: clutch_product_series::MarketInstanceV2Id,
        generation: u64,
        source_occurrence_id: clutch_product_series::SourceOccurrenceV1Id,
        begin_admission_receipt_id: ContentId,
    ) -> Outcome<()> {
        require(
            begin_admission_receipt_id == self.id
                && root_account == self.root_account
                && root_authentication_id == self.root_authentication_id
                && link_account == self.link_account
                && link_authentication_id == self.link_authentication_id
                && series_plan_id == self.series_plan_id
                && ordinal == self.ordinal
                && market_instance_id == self.market_instance_id
                && generation == self.generation
                && source_occurrence_id == self.source_occurrence_id,
            ClutchError::MismatchedState,
        )
    }
}

impl AuthenticatedFailureMarketIntervalCellActivationV2
    for CurrentSuccessBeginPreauthorizationV2
{
    fn authenticate_failure_market_interval_cell_activation(
        &self,
        expected: FailureMarketIntervalCellActivationFactsV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if expected == self.activation_facts
            && expected.session_binding_id.bytes()
                == self.predicted_session_transcript_id.bytes()
        {
            Ok(())
        } else {
            Err(clutch_failure_policy_runtime::Error::BindingMismatch)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CurrentSuccessBeginCellWriteV2 {
    preauthorization_id: ContentId,
    activation: FailureMarketIntervalCellActivationReceiptV2,
}

impl AuthenticatedFailureMarketIntervalBeginV2 for CurrentSuccessBeginCellWriteV2 {
    fn authenticate_failure_market_interval_begin_v2(
        &self,
        expected: FailureMarketIntervalCellActivationReceiptV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if self.preauthorization_id != ContentId::ZERO && expected == self.activation {
            Ok(())
        } else {
            Err(clutch_failure_policy_runtime::Error::BindingMismatch)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CurrentSuccessRuntimeBeginAuthorityV2 {
    expected: FailureMarketSessionBeginFactsV2,
}

impl AuthenticatedFailureMarketSessionBeginV2 for CurrentSuccessRuntimeBeginAuthorityV2 {
    fn authenticate_failure_market_session_begin_v2(
        &self,
        mut expected: FailureMarketSessionBeginFactsV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        let receipt = expected.begin_receipt_id;
        expected.begin_receipt_id =
            clutch_failure_policy_runtime::market_runtime_v1::FailureMarketSessionTransitionReceiptIdV1::from_bytes([0; 32]);
        if receipt.bytes() != [0; 32] && expected == self.expected {
            Ok(())
        } else {
            Err(clutch_failure_policy_runtime::Error::BindingMismatch)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CurrentSuccessRuntimeWriteV2 {
    expected: FailureMarketRuntimeSessionWriteFactsV1,
}

impl AuthenticatedFailureMarketRuntimeSessionWriteV1 for CurrentSuccessRuntimeWriteV2 {
    fn authenticate_failure_market_runtime_session_write_v1(
        &self,
        expected: FailureMarketRuntimeSessionWriteFactsV1,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if expected == self.expected {
            Ok(())
        } else {
            Err(clutch_failure_policy_runtime::Error::BindingMismatch)
        }
    }
}

/// Dispatch current action10 from exact hostile account authority.
#[allow(clippy::too_many_lines)]
pub(crate) fn process_begin_failure_market_session_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: FailureMarketActionPayloadV2<'_>,
) -> Outcome<()> {
    let FailureMarketActionPayloadV2::Begin = payload
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
    let recovery = account_for_role_v2(action, accounts, Role::RecoveryPolicyArtifact)?;
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
    let failure_liveness_policy =
        account_for_role_v2(action, accounts, Role::FailureLivenessPolicy)?;
    let source_terminal_policy =
        account_for_role_v2(action, accounts, Role::SourceTerminalPolicy)?;
    let source_terminal_receipt =
        account_for_role_v2(action, accounts, Role::SourceTerminalReceipt)?;
    let source_liveness_policy =
        account_for_role_v2(action, accounts, Role::SourceLivenessPolicy)?;
    let source_liveness_compartment =
        account_for_role_v2(action, accounts, Role::SourceLivenessCompartment)?;
    let source_funding_custody =
        account_for_role_v2(action, accounts, Role::SourceFundingCustody)?;
    let source_neutral_sink = account_for_role_v2(action, accounts, Role::SourceNeutralSink)?;
    let system_program = account_for_role_v2(action, accounts, Role::SystemProgram)?;
    let rent_sysvar = account_for_role_v2(action, accounts, Role::RentSysvar)?;

    let admission = authenticate_failure_market_root_v3(program_id, admission_account, false)?;
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
    require(
        root.state().phase() == MarketLifecyclePhaseV2::Active
            && root_binding.market_failure_policy_binding_id.bytes()
                == admission.state().binding().id().bytes()
            && root_binding.market_instance_id == policy.market_instance_id
            && root_binding.generation == policy.generation,
        ClutchError::MismatchedState,
    )?;

    let mut link_decode = Box::new(SeriesMarketLinkAccountV2::decode_buffer());
    let link_data = link_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    SeriesMarketLinkAccountV2::decode_into(&link_data, &mut link_decode)?;
    drop(link_data);
    let decoded_link_binding = link_decode.state.binding();
    let link = authenticate_series_market_link_v2(
        program_id,
        link_account,
        decoded_link_binding.series_plan_id,
        decoded_link_binding.ordinal,
        policy.market_instance_id,
        policy.generation,
        *root_account.key,
        true,
        &mut link_decode,
    )?;
    let link_binding = link.state().binding();
    require(
        link_binding.market_binding_id == root_binding_id
            && link_binding.market_instance_id == root_binding.market_instance_id
            && link_binding.generation == root_binding.generation,
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
    let quote = authenticate_failure_market_recovery_quote_v2(
        program_id,
        admission,
        root,
        &registry,
        failure_liveness_policy,
    )?;
    let funding_preimage = admission.interval_funding_preimage();
    let funding = FailureMarketIntervalFundingPreimageV2::decode(&funding_preimage)?;
    let interval = reopen_failure_market_interval_accounts_v2(
        program_id,
        cell_account,
        history_account,
        admission,
        quote.receipt(),
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
    require(sequence == expected_sequence, ClutchError::Replay)?;
    let attempt_index = u8::try_from(runtime.state().completed_session_count())
        .map_err(|_| ClutchError::Arithmetic)?;
    let mut schedule_root = Box::new(MarketLifecycleRootAccountV2::decode_buffer());
    let mut schedule_link = Box::new(SeriesMarketLinkAccountV2::decode_buffer());
    let product_schedule = authenticate_product_failure_begin_schedule_v2(
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
        recovery,
        price,
        genesis,
        attachment,
        market,
        &quote,
        attempt_index,
        &mut schedule_root,
        &mut schedule_link,
    )?;
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
    require_current_source_authority(
        route,
        source_schedule,
        root_binding,
        link_binding,
        policy,
        &registry,
        &product_schedule,
    )?;
    let source = reconstruct_exact_action10_source(
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
    match source {
        ReconstructedAction10SourceV1::Failure(source) => {
            let mut pin_root = Box::new(MarketLifecycleRootAccountV2::decode_buffer());
            let mut pinned_link = Box::new(SeriesMarketLinkAccountV2::decode_buffer());
            let mut release_root = Box::new(MarketLifecycleRootAccountV2::decode_buffer());
            let mut release_link = Box::new(SeriesMarketLinkAccountV2::decode_buffer());
            let mut released_link = Box::new(SeriesMarketLinkAccountV2::decode_buffer());
            let _ = compose_failure_market_source_failure_attempt_v3(
                program_id,
                root_account,
                link_account,
                admission_account,
                runtime_account,
                cell_account,
                history_account,
                root,
                link,
                admission,
                runtime,
                interval,
                quote,
                &product_schedule,
                route,
                source_schedule,
                source,
                source_result,
                source_lineage,
                source_terminal_policy,
                source_terminal_receipt,
                source_liveness_policy,
                source_liveness_compartment,
                source_funding_custody,
                source_neutral_sink,
                system_program,
                rent_sysvar,
                &mut pin_root,
                &mut pinned_link,
                &mut release_root,
                &mut release_link,
                &mut released_link,
            )?;
            Ok(())
        }
        ReconstructedAction10SourceV1::Successful(source) => {
            let mut pin_root = Box::new(MarketLifecycleRootAccountV2::decode_buffer());
            let mut pinned_link = Box::new(SeriesMarketLinkAccountV2::decode_buffer());
            let _ = compose_current_successful_failure_begin_v2(
                program_id,
                root_account,
                link_account,
                admission_account,
                runtime_account,
                cell_account,
                history_account,
                root,
                link,
                admission,
                runtime,
                interval,
                &registry,
                &product_schedule,
                source,
                template,
                basis,
                price,
                genesis,
                market,
                &mut pin_root,
                &mut pinned_link,
            )?;
            Ok(())
        }
    }
}

/// Atomically write one successful Idle-to-Active cell, pin LinkV2, and write
/// the shared runtime transcript. History is writable only because action10's
/// absence/refusal branches append it; this success branch reauthenticates it
/// byte-for-byte and never mutates it.
#[allow(clippy::too_many_arguments)]
fn compose_current_successful_failure_begin_v2<'next>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    admission_account: &AccountInfo<'_>,
    runtime_account: &AccountInfo<'_>,
    cell_account: &AccountInfo<'_>,
    history_account: &AccountInfo<'_>,
    root: AuthenticatedMarketLifecycleRootV2<'_>,
    link: AuthenticatedSeriesMarketLinkV2<'_>,
    admission: crate::instructions::failure_market_admission::AuthenticatedFailureMarketRootV3,
    runtime: AuthenticatedFailureMarketRuntimeRootV1,
    interval: AuthenticatedFailureMarketIntervalAccountsV2,
    registry: &AuthenticatedRegistryCapabilityV4,
    schedule: &crate::instructions::product_failure_begin_current::AuthenticatedProductFailureBeginScheduleV2,
    source: crate::source_plane_v3::AuthenticatedSuccessfulSourceHandoffV1,
    template_account: &AccountInfo<'_>,
    basis_account: &AccountInfo<'_>,
    price_account: &AccountInfo<'_>,
    genesis_account: &AccountInfo<'_>,
    market_account: &AccountInfo<'_>,
    root_rebound: &mut MarketLifecycleRootAccountV2,
    link_rebound: &'next mut SeriesMarketLinkAccountV2,
) -> Outcome<(
    AuthenticatedSeriesMarketLinkV2<'next>,
    AuthenticatedSeriesFailureSessionPinV2,
    AuthenticatedFailureMarketRuntimeSessionPostwriteV1,
)> {
    let root_binding = root.state().binding();
    let link_binding = link.state().binding();
    let policy = admission.state().binding().facts();
    let template = authenticate_product_artifact_v1::<ProductTemplateV4>(
        program_id,
        template_account,
        root_binding.product_template_id,
    )?;
    let basis = authenticate_product_artifact_v1::<NativeClaimBasisV1>(
        program_id,
        basis_account,
        root_binding.native_claim_basis_id,
    )?;
    let price = authenticate_product_artifact_v1::<PriceMeasurePolicyV1>(
        program_id,
        price_account,
        root_binding.price_measure_policy_id,
    )?;
    let genesis = authenticate_product_artifact_v1::<MarketGenesisProfileV2>(
        program_id,
        genesis_account,
        root_binding.market_genesis_profile_id,
    )?;
    let market = authenticate_product_artifact_v1::<MarketInstancePreimageV2>(
        program_id,
        market_account,
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
        template.semantic_id() == root_binding.product_template_id
            && basis.semantic_id() == root_binding.native_claim_basis_id
            && price.semantic_id() == root_binding.price_measure_policy_id
            && genesis.semantic_id() == root_binding.market_genesis_profile_id
            && market.semantic_id() == root_binding.market_instance_id.content_id()
            && template.semantic_id().bytes() == policy.product_template_id.bytes()
            && basis.semantic_id().bytes() == policy.native_claim_basis_id.bytes()
            && price.semantic_id().bytes() == policy.price_measure_policy_id.bytes()
            && genesis.semantic_id().bytes() == policy.market_genesis_profile_id.bytes()
            && work_profile_id.bytes() == policy.interval_consensus_profile_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    require_exact_successful_source_join_current_v2(
        source.join(),
        source.handoff(),
        link_binding,
        root_binding,
        policy,
    )?;
    let source_interval = source.interval();
    let occurrence = source_interval.occurrence();
    let statistic_result = source_interval.statistic_result();
    let statistic_key = source_interval.statistic_key();
    let summary = source_interval.summary_program();
    let seal = source_interval.window_seal();
    let window = source_interval.window();
    let context = QuantizedIntervalConsensusContextV1 {
        market: market.value(),
        product_template: template.value(),
        native_claim_basis: basis.value(),
        price_measure_policy: price.value(),
        market_genesis: genesis.value(),
        resolved_edge_policy: projection.resolved_edge_policy,
        source_occurrence: &occurrence,
        source_interval: &statistic_result,
        statistic_key: &statistic_key,
        summary_program: &summary,
        window_seal: &seal,
        window: &window,
        work_profile: &work_profile,
    };
    let initial_work = *begin_quantized_interval_consensus_v1(context)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .work();
    let initial_work_id = initial_work
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let attempt_index = u8::try_from(runtime.state().completed_session_count())
        .map_err(|_| ClutchError::Arithmetic)?;
    require(
        schedule.attempt_index() == attempt_index
            && schedule.source_repair_generation() == link_binding.source_repair_generation
            && schedule.source_repair_generation() == source.handoff().occurrence().repair_generation()
            && interval.cell().completed_session_count()
                == runtime.state().completed_session_count()
            && interval.history().completed_session_count()
                == runtime.state().completed_session_count(),
        ClutchError::MismatchedState,
    )?;
    let preauthorization_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            CURRENT_SUCCESS_BEGIN_PREAUTHORIZATION_DOMAIN_V2,
            program_id.as_ref(),
            root.account().as_ref(),
            &root.authentication_id().bytes(),
            link.account().as_ref(),
            &link.authentication_id().bytes(),
            admission.account().as_ref(),
            &admission.state().binding().id().bytes(),
            runtime.account().as_ref(),
            &runtime.state_commitment().bytes(),
            interval.cell_account().as_ref(),
            &interval.cell_authentication_id().bytes(),
            interval.history_account().as_ref(),
            &interval.history_authentication_id().bytes(),
            &schedule.id().bytes(),
            &schedule.schedule_projection_id().bytes(),
            &source.id().bytes(),
            &source.join().id().bytes(),
            &initial_work_id.bytes(),
            &[attempt_index],
        ])
        .to_bytes(),
    );
    require(preauthorization_id != ContentId::ZERO, ClutchError::MismatchedState)?;
    let predicted_link = link
        .state()
        .pin_failure_session(preauthorization_id)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let predicted_session_transcript_id = predicted_link.failure_session_transcript_id();
    let activation_facts = FailureMarketIntervalCellActivationFactsV2 {
        cell_before: interval.cell_state_id(),
        history_root: interval.history().history_root(),
        completed_session_count: interval.cell().completed_session_count(),
        session_binding_id: clutch_source_plane_v3::ContentId::from_bytes(
            predicted_session_transcript_id.bytes(),
        ),
        source_handoff_id: source.handoff().id(),
        source_repair_generation: source.handoff().occurrence().repair_generation(),
        session_schedule_id: clutch_source_plane_v3::ContentId::from_bytes(
            schedule.schedule_projection_id().bytes(),
        ),
        attempt_index,
        product_work_id: initial_work_id,
    };
    let preauthorization = CurrentSuccessBeginPreauthorizationV2 {
        id: preauthorization_id,
        root_account: root.account(),
        root_authentication_id: root.authentication_id(),
        link_account: link.account(),
        link_authentication_id: link.authentication_id(),
        series_plan_id: link_binding.series_plan_id,
        ordinal: link_binding.ordinal,
        market_instance_id: link_binding.market_instance_id,
        generation: link_binding.generation,
        source_occurrence_id: link_binding.source_occurrence_id,
        predicted_session_transcript_id,
        activation_facts,
    };
    let (cell_plan, activation) = plan_activate_failure_market_interval_cell_v2(
        &preauthorization,
        interval.cell(),
        admission.state(),
        interval.funding(),
        interval.history(),
        interval.quote(),
        clutch_source_plane_v3::ContentId::from_bytes(predicted_session_transcript_id.bytes()),
        clutch_source_plane_v3::ContentId::from_bytes(schedule.schedule_projection_id().bytes()),
        source.handoff(),
        context,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(activation.facts() == activation_facts, ClutchError::MismatchedState)?;
    let interval_after = write_failure_market_interval_begin_plan_v2(
        program_id,
        cell_account,
        history_account,
        admission,
        interval,
        cell_plan,
        activation,
        &CurrentSuccessBeginCellWriteV2 {
            preauthorization_id,
            activation,
        },
    )?;
    let (link_after, pin) = pin_series_market_link_failure_v2(
        program_id,
        root_account,
        root,
        link_account,
        link,
        preauthorization_id,
        &preauthorization,
        root_rebound,
        link_rebound,
    )?;
    require(
        *link_after.state() == predicted_link
            && pin.session_binding_id() == predicted_session_transcript_id
            && interval_after.cell().session_binding_id().bytes()
                == predicted_session_transcript_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    let cell_state_after = interval_after
        .cell()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let session = FailureMarketSessionDescriptorV1 {
        series_plan_id: link_binding.series_plan_id,
        ordinal: link_binding.ordinal,
        source_occurrence_id: link_binding.source_occurrence_id,
        schedule_id: FailureMarketSessionScheduleIdV1::from_bytes(
            schedule.schedule_projection_id().bytes(),
        ),
        interval_funding_receipt_id: interval.funding().id(),
        session_state_commitment: ContentId::from_bytes(cell_state_after.bytes()),
    };
    let expected_runtime_facts = FailureMarketSessionBeginFactsV2 {
        runtime_before: runtime.state_commitment(),
        series_link_before: link
            .state()
            .semantic_id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        series_link_after: predicted_link
            .semantic_id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        product_pin_receipt_id: pin.id(),
        previous_session_history: runtime.state().session_history_commitment(),
        previous_interval_terminal_receipt_id: runtime.state().interval_terminal_receipt_id(),
        interval_work_account: interval.funding().facts().work_account,
        interval_history_account: interval.funding().facts().history_account,
        interval_history_state_id: interval.history_state_id(),
        completed_session_count: runtime.state().completed_session_count(),
        begin_preauthorization_id: preauthorization_id,
        session_binding_id: predicted_session_transcript_id,
        session,
        begin_receipt_id:
            clutch_failure_policy_runtime::market_runtime_v1::FailureMarketSessionTransitionReceiptIdV1::from_bytes([0; 32]),
    };
    let runtime_plan = plan_begin_failure_market_session_v2(
        &CurrentSuccessRuntimeBeginAuthorityV2 {
            expected: expected_runtime_facts,
        },
        runtime.state(),
        admission.state(),
        *link.state(),
        preauthorization_id,
        pin.id(),
        session,
        interval.funding(),
        interval.history(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        runtime_plan.series_link_before() == *link.state()
            && runtime_plan.series_link_after() == *link_after.state()
            && runtime_plan.resulting_runtime().active_session_pin_id()
                == predicted_session_transcript_id
            && runtime_plan.resulting_runtime().session_state_commitment()
                == ContentId::from_bytes(cell_state_after.bytes()),
        ClutchError::MismatchedState,
    )?;
    let runtime_after = runtime_plan
        .resulting_runtime()
        .commitment()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let runtime_write_facts = FailureMarketRuntimeSessionWriteFactsV1 {
        runtime_before: runtime.state_commitment(),
        runtime_after,
        transition_receipt_id: runtime_plan.receipt_id(),
    };
    let runtime_postwrite = write_failure_market_runtime_begin_plan_v2(
        program_id,
        admission_account,
        runtime_account,
        admission,
        runtime,
        runtime_plan,
        &CurrentSuccessRuntimeWriteV2 {
            expected: runtime_write_facts,
        },
    )?;
    Ok((link_after, pin, runtime_postwrite))
}

#[allow(clippy::too_many_arguments)]
fn reconstruct_exact_action10_source(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    occurrence: &AccountInfo<'_>,
    window: &AccountInfo<'_>,
    key: &AccountInfo<'_>,
    summary: &AccountInfo<'_>,
    seal: &AccountInfo<'_>,
    result: &AccountInfo<'_>,
    lineage: &AccountInfo<'_>,
    handoff: &AccountInfo<'_>,
    work_receipt: &AccountInfo<'_>,
) -> Outcome<ReconstructedAction10SourceV1> {
    let successful = authenticate_successful_source_handoff_for_resolution_v1(
        program_id, route, schedule, occurrence, window, key, summary, seal, result, lineage,
        handoff, work_receipt,
    )
    .ok();
    let absence = authenticate_failure_absence_source_handoff_for_terminal_v1(
        program_id, route, schedule, occurrence, window, key, summary, seal, result, lineage,
        handoff, work_receipt,
    )
    .ok();
    let refused = authenticate_failure_result_source_handoff_for_terminal_v1(
        program_id, route, schedule, occurrence, window, key, summary, seal, result, lineage,
        handoff, work_receipt,
    )
    .ok();
    match (successful, absence, refused) {
        (Some(value), None, None) => Ok(ReconstructedAction10SourceV1::Successful(value)),
        (None, Some(value), None) => Ok(ReconstructedAction10SourceV1::Failure(
            AuthenticatedSourceFailureHandoffV1::Absence(value),
        )),
        (None, None, Some(value)) => Ok(ReconstructedAction10SourceV1::Failure(
            AuthenticatedSourceFailureHandoffV1::Refused(value),
        )),
        _ => Err(Refusal::Adapter(ClutchError::MismatchedState)),
    }
}

pub(crate) fn require_exact_successful_source_join_current_v2(
    source_join: clutch_source_plane_v3_runtime::SourcePolicyHandoffJoinV1,
    source_success: clutch_source_plane_v3_runtime::SuccessfulEvaluationHandoffV1,
    link: clutch_product_series::SeriesMarketLinkBindingV2,
    root: clutch_product_series::MarketLifecycleBindingV2,
    policy: clutch_failure_policy_runtime::market_policy_v1::FailureMarketPolicyFactsV1,
) -> Outcome<()> {
    let occurrence = source_success.occurrence();
    let statistic_result_id = source_success
        .statistic_result_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        source_join.handoff_id() == source_success.id()
            && source_join.failure_policy_binding_id()
                == source_success.failure_policy_binding_id()
            && source_join.failure_policy_binding_id().bytes()
                == root.market_failure_policy_binding_id.bytes()
            && source_join.release_authentication_id().bytes()
                == policy.source_release_authentication_id.bytes()
            && source_join.route_id() == occurrence.route_id()
            && source_join.route_id().bytes() == root.source_route_id.bytes()
            && source_join.occurrence_account().bytes()
                == occurrence.occurrence_account().bytes()
            && source_join.occurrence_account().bytes()
                == link.source_occurrence_account_id.bytes()
            && source_join.source_fact_authentication_id()
                == source_success.result_account_authentication_id()
            && source_join.clock_policy_id() == source_success.clock_policy_id()
            && source_join.clock_policy_id() == occurrence.clock_policy_id()
            && source_join.clock_policy_id().bytes() == root.clock_policy_id.bytes()
            && source_join.clock() == source_success.clock()
            && source_join.source_spec_id() == occurrence.source_spec_id()
            && source_join.source_spec_id().bytes() == root.source_spec_id.bytes()
            && source_join.window_id() == occurrence.window_id()
            && source_join.statistic_key_id() == occurrence.statistic_key_id()
            && occurrence.occurrence_record_id().bytes() == link.source_occurrence_id.bytes()
            && occurrence.occurrence_account_authentication_id().bytes()
                == link.source_occurrence_account_authentication_id.bytes()
            && occurrence.series_plan_id().bytes() == link.series_plan_id.bytes()
            && occurrence.ordinal() == link.ordinal
            && occurrence.market_instance_id().bytes() == link.market_instance_id.bytes()
            && occurrence.attachment_plan_id().bytes() == link.attachment_plan_id.bytes()
            && occurrence.source_plane_contract_id().bytes()
                == root.source_plane_contract_id.bytes()
            && occurrence.source_spec_id().bytes() == root.source_spec_id.bytes()
            && statistic_result_id
                == source_success
                    .result()
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            && source_join.work_receipt_account().bytes() != [0; 32]
            && source_join.work_receipt_authentication_id()
                != clutch_source_plane_v3::ContentId::ZERO
            && source_join.id() != clutch_source_plane_v3::ContentId::ZERO,
        ClutchError::MismatchedState,
    )
}

#[allow(clippy::too_many_arguments)]
fn require_current_source_authority(
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    root: clutch_product_series::MarketLifecycleBindingV2,
    link: clutch_product_series::SeriesMarketLinkBindingV2,
    policy: clutch_failure_policy_runtime::market_policy_v1::FailureMarketPolicyFactsV1,
    registry: &crate::instructions::product_series_current::AuthenticatedRegistryCapabilityV4,
    product_schedule: &crate::instructions::product_failure_begin_current::AuthenticatedProductFailureBeginScheduleV2,
) -> Outcome<()> {
    require(
        route.release_manifest_id().bytes() == root.source_release_id.bytes()
            && route.release_manifest_id().bytes() == link.source_release_id.bytes()
            && route.release_manifest_id().bytes() == policy.source_release_manifest_id.bytes()
            && route.release_authentication_id().bytes()
                == policy.source_release_authentication_id.bytes()
            && route.route_id().bytes() == root.source_route_id.bytes()
            && route.route_id().bytes() == link.source_route_id.bytes()
            && route.source_plane_contract_id().bytes() == root.source_plane_contract_id.bytes()
            && route.source_plane_contract_id().bytes() == link.source_plane_contract_id.bytes()
            && route.source_plane_contract_id().bytes()
                == policy.source_plane_contract_id.bytes()
            && route.source_spec_id().bytes() == root.source_spec_id.bytes()
            && route.source_spec_id().bytes() == link.source_spec_id.bytes()
            && route.source_spec_id().bytes() == policy.source_spec_id.bytes()
            && route.clock_policy_id().bytes() == root.clock_policy_id.bytes()
            && route.clock_policy_id().bytes() == link.clock_policy_id.bytes()
            && route.clock_policy_id().bytes() == policy.clock_policy_id.bytes()
            && schedule.source_work_schedule_id() == route.source_work_schedule_id()
            && schedule.generation() == root.generation
            && schedule.source_compartment_account() == route.source_compartment_account()
            && schedule.source_compartment_owner() == route.source_compartment_owner()
            && registry.registry_release_id() == root.registry_release_id
            && registry.capability_profile_id() == root.capability_profile_id
            && product_schedule.registry_capability_id() == registry.id()
            && product_schedule.compiler_bundle_id() == registry.compiler_bundle_id()
            && product_schedule.series_plan_id() == link.series_plan_id
            && product_schedule.ordinal() == link.ordinal
            && product_schedule.market_instance_id() == root.market_instance_id
            && product_schedule.source_occurrence_id() == link.source_occurrence_id,
        ClutchError::MismatchedState,
    )
}

#[cfg(test)]
mod adversarial_source_contract_tests {
    #[test]
    fn action10_dispatch_reaches_the_current_atomic_failure_composer() {
        let source = include_str!("failure_market_action10_current.rs");
        assert!(source.contains("compose_failure_market_source_failure_attempt_v3("));
        assert!(source.contains("authenticate_product_failure_begin_schedule_v2("));
        assert!(source.contains("authenticate_failure_absence_source_handoff_for_terminal_v1("));
        assert!(source.contains("authenticate_failure_result_source_handoff_for_terminal_v1("));
        assert!(!source.contains("AuthenticatedSourceResolutionInputV3"));
        assert!(!source.contains("MarketLifecycleRootAccountV1"));
        assert!(!source.contains("SeriesMarketLinkAccountV1"));
    }

    #[test]
    fn source_branch_is_derived_by_exact_disjoint_reconstruction() {
        let source = include_str!("failure_market_action10_current.rs");
        let reconstruction = source
            .split("fn reconstruct_exact_action10_source")
            .nth(1)
            .expect("branch reconstruction");
        assert!(reconstruction.contains("(Some(value), None, None)"));
        assert!(reconstruction.contains("(None, Some(value), None)"));
        assert!(reconstruction.contains("(None, None, Some(value))"));
        assert!(reconstruction.contains("ClutchError::MismatchedState"));
    }

    #[test]
    fn successful_join_keeps_occurrence_identity_separate_from_pre_root_receipt() {
        let source = include_str!("failure_market_action10_current.rs");
        let join = source
            .split("fn require_exact_successful_source_join_current_v2")
            .nth(1)
            .and_then(|value| value.split("fn require_current_source_authority").next())
            .expect("current successful Source join");
        assert!(join.contains(
            "occurrence.occurrence_record_id().bytes() == link.source_occurrence_id.bytes()"
        ));
        assert!(join.contains(
            "occurrence.occurrence_account_authentication_id().bytes()"
        ));
        assert!(!join.contains(
            "occurrence.id().bytes() == link.source_occurrence_receipt_id.bytes()"
        ));
    }

    #[test]
    fn successful_branch_orders_cell_pin_and_runtime_under_rollback() {
        let source = include_str!("failure_market_action10_current.rs");
        let outer = source
            .split("fn compose_current_successful_failure_begin_v2")
            .nth(1)
            .expect("successful current outer");
        let cell = outer
            .find("write_failure_market_interval_begin_plan_v2(")
            .expect("cell write");
        let pin = outer
            .find("pin_series_market_link_failure_v2(")
            .expect("Product pin");
        let runtime = outer
            .find("write_failure_market_runtime_begin_plan_v2(")
            .expect("runtime write");
        assert!(cell < pin && pin < runtime);
        assert!(outer.contains("predicted_session_transcript_id"));
        assert!(outer.contains("pin.session_binding_id() == predicted_session_transcript_id"));
        assert!(!outer.contains("MarketLifecycleRootAccountV1"));
    }

    #[test]
    fn scratch_buffers_are_sequential_views_of_one_root_and_link() {
        let source = include_str!("failure_market_action10_current.rs");
        let failure_call = source
            .split("compose_failure_market_source_failure_attempt_v3(")
            .nth(1)
            .expect("failure outer call");
        for scratch in [
            "&mut pin_root",
            "&mut pinned_link",
            "&mut release_root",
            "&mut release_link",
            "&mut released_link",
        ] {
            assert!(failure_call.contains(scratch));
        }
        assert!(source.contains("require_distinct(accounts)"));
    }
}
