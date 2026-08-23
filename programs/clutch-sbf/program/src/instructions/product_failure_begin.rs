// SPDX-License-Identifier: AGPL-3.0-or-later
//! Product-owned current compiler authority for a Failure interval Begin.
//!
//! This module is deliberately not routed by dispatch and persists no schedule
//! artifact. It hostile-authenticates the current Product graph, recompiles one
//! exact V5 Series ordinal, and returns a private receipt over the complete
//! canonical schedule body and its compiler provenance. Possession of a caller
//! supplied schedule or digest is never authority.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::failure_market_admission::AuthenticatedFailureMarketRootV2;
use crate::instructions::failure_market_interval_v2::{
    write_failure_market_interval_begin_plan_v2, AuthenticatedFailureMarketIntervalAccountsV2,
    AuthenticatedFailureMarketIntervalBeginV2,
};
use crate::instructions::failure_market_runtime::{
    write_failure_market_runtime_session_plan_v1, AuthenticatedFailureMarketRuntimeRootV1,
    AuthenticatedFailureMarketRuntimeSessionPostwriteV1,
    AuthenticatedFailureMarketRuntimeSessionWriteV1, FailureMarketRuntimeSessionWriteFactsV1,
};
use crate::instructions::product_artifact::{
    authenticate_product_artifact_v1, AuthenticatedProductArtifactV1,
    AuthenticatedRegistryCapabilityV3,
};
use crate::instructions::product_market::{
    authenticate_market_lifecycle_root_v1, authenticate_series_market_link_v1,
    pin_series_market_link_failure_v1, AuthenticatedMarketLifecycleRootV1,
    AuthenticatedSeriesFailureSessionBeginV2, AuthenticatedSeriesMarketLinkV1,
};
use crate::source_plane_v3_actions::SourcePolicyHandoffJoinV1;
use clutch_failure_policy_runtime::market_interval_cell_v2::{
    plan_activate_failure_market_interval_cell_v2,
    AuthenticatedFailureMarketIntervalCellActivationV2, FailureMarketIntervalCellActivationFactsV2,
    FailureMarketIntervalCellActivationReceiptV2,
};
use clutch_failure_policy_runtime::market_runtime_v1::{
    plan_begin_failure_market_session_v1, AuthenticatedFailureMarketSessionV1,
    FailureMarketSessionBeginFactsV1, FailureMarketSessionDescriptorV1,
    FailureMarketSessionScheduleIdV1,
};
use clutch_product_series::{
    begin_quantized_interval_consensus_v1, compile_ordinal_v5,
    derive_product_failure_begin_schedule_projection_v1, CompiledProductSeriesBundleV5,
    CompiledScheduleV1, ContentId, EvidenceOnlyRecoveryPolicyV1, MarketGenesisProfileV2,
    MarketInstancePreimageV2, MarketInstanceV2Id, MarketLifecyclePhaseV1, NativeClaimBasisV1,
    PriceMeasurePolicyV1, ProductFailureBeginCompilerProvenanceV1,
    ProductFailureBeginScheduleProjectionV1Id, ProductTemplateV4,
    QuantizedIntervalConsensusContextV1, SeriesAttachmentPlanV4, SeriesMarketLinkPhaseV1,
    SeriesPlanV5, SeriesPlanV5Id, SourceOccurrenceV1Id,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV1, SeriesMarketLinkAccountV1,
};
use clutch_source_plane_v3::ContentId as SourceContentId;
use clutch_source_plane_v3_runtime::SuccessfulEvaluationHandoffV1;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const PRODUCT_FAILURE_BEGIN_SCHEDULE_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/product-failure-begin-schedule-authentication/v1\0";
const PRODUCT_FAILURE_BEGIN_PREAUTHORIZATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/product-failure-begin-preauthorization/v2\0";
const PRODUCT_FAILURE_BEGIN_POSTWRITE_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/product-failure-begin-postwrite/v2\0";

/// Testable exact equality partition for the current mutable/immutable graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProductFailureBeginGraphJoinV1 {
    registry_terms: ContentId,
    bundle_terms: ContentId,
    link_terms: ContentId,
    bundle_quote: ContentId,
    link_quote: ContentId,
    attachment_quote: ContentId,
    bundle_attachment: ContentId,
    link_attachment: ContentId,
    attachment_semantic: ContentId,
    registry_release: ContentId,
    projection_release: ContentId,
    bundle_release: ContentId,
    root_release: ContentId,
    registry_profile: ContentId,
    projection_profile: ContentId,
    bundle_profile: ContentId,
    link_profile: ContentId,
    root_profile: ContentId,
    bundle_source_release: ContentId,
    link_source_release: ContentId,
    root_source_release: ContentId,
    bundle_source_plane: ContentId,
    link_source_plane: ContentId,
    root_source_plane: ContentId,
    bundle_source_spec: ContentId,
    link_source_spec: ContentId,
    root_source_spec: ContentId,
    link_source_route: ContentId,
    root_source_route: ContentId,
    link_clock_policy: ContentId,
    root_clock_policy: ContentId,
}

impl ProductFailureBeginGraphJoinV1 {
    fn validate(self) -> Outcome<()> {
        require(
            self.registry_terms == self.bundle_terms
                && self.registry_terms == self.link_terms
                && self.bundle_quote == self.link_quote
                && self.bundle_quote == self.attachment_quote
                && self.bundle_attachment == self.link_attachment
                && self.bundle_attachment == self.attachment_semantic
                && self.registry_release == self.projection_release
                && self.registry_release == self.bundle_release
                && self.registry_release == self.root_release
                && self.registry_profile == self.projection_profile
                && self.registry_profile == self.bundle_profile
                && self.registry_profile == self.link_profile
                && self.registry_profile == self.root_profile
                && self.bundle_source_release == self.link_source_release
                && self.bundle_source_release == self.root_source_release
                && self.bundle_source_plane == self.link_source_plane
                && self.bundle_source_plane == self.root_source_plane
                && self.bundle_source_spec == self.link_source_spec
                && self.bundle_source_spec == self.root_source_spec
                && self.link_source_route == self.root_source_route
                && self.link_clock_policy == self.root_clock_policy,
            ClutchError::MismatchedState,
        )
    }
}

/// Product-private exact current schedule projection for one subordinate Begin.
///
/// The receipt is intentionally not decodable and its constructor is private to
/// this module. Failure may consume its crate getters only inside the atomic
/// Begin composer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedProductFailureBeginScheduleV1 {
    id: ContentId,
    schedule_projection_id: ProductFailureBeginScheduleProjectionV1Id,
    schedule: CompiledScheduleV1,
    root_account: Pubkey,
    root_authentication_id: ContentId,
    link_account: Pubkey,
    link_authentication_id: ContentId,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    source_occurrence_id: SourceOccurrenceV1Id,
    registry_release_id: ContentId,
    capability_profile_id: ContentId,
    compiler_bundle_id: ContentId,
    edge_policy_registry_value: u8,
    resolved_edge_policy: clutch_product_series::QuantizedEdgePolicyV1,
}

impl AuthenticatedProductFailureBeginScheduleV1 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn schedule_projection_id(self) -> ProductFailureBeginScheduleProjectionV1Id {
        self.schedule_projection_id
    }

    pub(crate) const fn schedule(self) -> CompiledScheduleV1 {
        self.schedule
    }

    pub(crate) const fn root_account(self) -> Pubkey {
        self.root_account
    }

    pub(crate) const fn root_authentication_id(self) -> ContentId {
        self.root_authentication_id
    }

    pub(crate) const fn link_account(self) -> Pubkey {
        self.link_account
    }

    pub(crate) const fn link_authentication_id(self) -> ContentId {
        self.link_authentication_id
    }

    pub(crate) const fn series_plan_id(self) -> SeriesPlanV5Id {
        self.series_plan_id
    }

    pub(crate) const fn ordinal(self) -> u32 {
        self.ordinal
    }

    pub(crate) const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    pub(crate) const fn generation(self) -> u64 {
        self.generation
    }

    pub(crate) const fn source_occurrence_id(self) -> SourceOccurrenceV1Id {
        self.source_occurrence_id
    }

    pub(crate) const fn registry_release_id(self) -> ContentId {
        self.registry_release_id
    }

    pub(crate) const fn capability_profile_id(self) -> ContentId {
        self.capability_profile_id
    }

    pub(crate) const fn compiler_bundle_id(self) -> ContentId {
        self.compiler_bundle_id
    }

    pub(crate) const fn edge_policy_registry_value(self) -> u8 {
        self.edge_policy_registry_value
    }

    pub(crate) const fn resolved_edge_policy(self) -> clutch_product_series::QuantizedEdgePolicyV1 {
        self.resolved_edge_policy
    }
}

/// Exact Product/Source/Failure preauthorization computed before either write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketIntervalBeginPreauthorizationV2 {
    id: ContentId,
    predicted_session_transcript_id: ContentId,
    activation_facts: FailureMarketIntervalCellActivationFactsV2,
    schedule_projection_id: ProductFailureBeginScheduleProjectionV1Id,
    source_join_id: SourceContentId,
}

impl AuthenticatedFailureMarketIntervalCellActivationV2
    for FailureMarketIntervalBeginPreauthorizationV2
{
    fn authenticate_failure_market_interval_cell_activation(
        &self,
        expected: FailureMarketIntervalCellActivationFactsV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if self.id.is_zero()
            || expected != self.activation_facts
            || expected.session_binding_id.bytes() != self.predicted_session_transcript_id.bytes()
            || expected.session_schedule_id.bytes() != self.schedule_projection_id.bytes()
            || expected.source_handoff_id.bytes() != self.source_join_id.bytes()
        {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

/// Module-private authority accepted by the narrow Idle-to-Active writer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketIntervalBeginWriteV2 {
    preauthorization_id: ContentId,
    activation: FailureMarketIntervalCellActivationReceiptV2,
}

impl AuthenticatedFailureMarketIntervalBeginV2 for FailureMarketIntervalBeginWriteV2 {
    fn authenticate_failure_market_interval_begin_v2(
        &self,
        expected: FailureMarketIntervalCellActivationReceiptV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if expected != self.activation || self.preauthorization_id.is_zero() {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

/// Private post-cell authority consumed by Product's sole guarded `0xad` pin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProductFailureSessionPinPostwriteV2 {
    root_account: Pubkey,
    root_authentication_id: ContentId,
    link_account: Pubkey,
    link_authentication_id: ContentId,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    source_occurrence_id: SourceOccurrenceV1Id,
    begin_admission_receipt_id: ContentId,
    predicted_session_transcript_id: ContentId,
    cell_authentication_after: ContentId,
    activation: FailureMarketIntervalCellActivationReceiptV2,
}

/// Exact pure shared-runtime Begin authority derived before any account write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketRuntimeBeginAuthorityV1 {
    runtime_before:
        clutch_failure_policy_runtime::market_runtime_v1::FailureMarketRuntimeStateCommitmentV1,
    series_link_before: clutch_product_series::SeriesMarketLinkV1Id,
    series_link_after: clutch_product_series::SeriesMarketLinkV1Id,
    previous_session_history:
        clutch_failure_policy_runtime::market_interval_history_v2::FailureMarketIntervalHistoryRootV2,
    previous_interval_terminal_receipt_id: ContentId,
    interval_work_account:
        clutch_failure_policy_runtime::market_policy_v1::FailureMarketAccountIdV1,
    interval_history_account:
        clutch_failure_policy_runtime::market_policy_v1::FailureMarketAccountIdV1,
    interval_history_state_id: clutch_failure_policy_runtime::market_interval_history_v2::FailureMarketIntervalHistoryStateIdV2,
    completed_session_count: u64,
    begin_preauthorization_id: ContentId,
    session_binding_id: ContentId,
    session: FailureMarketSessionDescriptorV1,
}

impl AuthenticatedFailureMarketSessionV1 for FailureMarketRuntimeBeginAuthorityV1 {
    fn authenticate_failure_market_session_begin(
        &self,
        expected: FailureMarketSessionBeginFactsV1,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if expected.runtime_before != self.runtime_before
            || expected.series_link_before != self.series_link_before
            || expected.series_link_after != self.series_link_after
            || expected.previous_session_history != self.previous_session_history
            || expected.previous_interval_terminal_receipt_id
                != self.previous_interval_terminal_receipt_id
            || expected.interval_work_account != self.interval_work_account
            || expected.interval_history_account != self.interval_history_account
            || expected.interval_history_state_id != self.interval_history_state_id
            || expected.completed_session_count != self.completed_session_count
            || expected.begin_preauthorization_id != self.begin_preauthorization_id
            || expected.session_binding_id != self.session_binding_id
            || expected.session != self.session
            || expected.begin_receipt_id.bytes() == [0; 32]
        {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

/// Exact physical shared-runtime write admitted after cell and link postwrites.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketRuntimeBeginWriteV1 {
    expected: FailureMarketRuntimeSessionWriteFactsV1,
    cell_state_after: ContentId,
    runtime_session_state_after: ContentId,
    link_state_after: clutch_product_series::SeriesMarketLinkV1,
    planned_link_state_after: clutch_product_series::SeriesMarketLinkV1,
    session_binding_id: ContentId,
}

impl AuthenticatedFailureMarketRuntimeSessionWriteV1 for FailureMarketRuntimeBeginWriteV1 {
    fn authenticate_failure_market_runtime_session_write_v1(
        &self,
        expected: FailureMarketRuntimeSessionWriteFactsV1,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if expected != self.expected
            || self.cell_state_after != self.runtime_session_state_after
            || self.link_state_after != self.planned_link_state_after
            || self.link_state_after.failure_session_transcript_id() != self.session_binding_id
        {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

impl AuthenticatedSeriesFailureSessionBeginV2 for ProductFailureSessionPinPostwriteV2 {
    fn authenticate_series_failure_session_begin_v2(
        &self,
        root_account: Pubkey,
        root_authentication_id: ContentId,
        link_account: Pubkey,
        link_authentication_id: ContentId,
        series_plan_id: SeriesPlanV5Id,
        ordinal: u32,
        market_instance_id: MarketInstanceV2Id,
        generation: u64,
        source_occurrence_id: SourceOccurrenceV1Id,
        begin_admission_receipt_id: ContentId,
    ) -> Outcome<()> {
        require(
            root_account == self.root_account
                && root_authentication_id == self.root_authentication_id
                && link_account == self.link_account
                && link_authentication_id == self.link_authentication_id
                && series_plan_id == self.series_plan_id
                && ordinal == self.ordinal
                && market_instance_id == self.market_instance_id
                && generation == self.generation
                && source_occurrence_id == self.source_occurrence_id
                && begin_admission_receipt_id == self.begin_admission_receipt_id
                && self.activation.facts().session_binding_id.bytes()
                    == self.predicted_session_transcript_id.bytes()
                && self.cell_authentication_after != ContentId::ZERO,
            ClutchError::MismatchedState,
        )
    }
}

/// Complete same-instruction Idle-to-Active cell plus `0xad` pin postwrite.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedFailureMarketIntervalBeginPostwriteV2 {
    id: ContentId,
    preauthorization_id: ContentId,
    schedule_projection_id: ProductFailureBeginScheduleProjectionV1Id,
    activation: FailureMarketIntervalCellActivationReceiptV2,
    predicted_session_transcript_id: ContentId,
    cell_authentication_before: ContentId,
    cell_authentication_after: ContentId,
    link_authentication_before: ContentId,
    link_authentication_after: ContentId,
    runtime_postwrite_id: ContentId,
}

impl AuthenticatedFailureMarketIntervalBeginPostwriteV2 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn preauthorization_id(self) -> ContentId {
        self.preauthorization_id
    }

    pub(crate) const fn schedule_projection_id(self) -> ProductFailureBeginScheduleProjectionV1Id {
        self.schedule_projection_id
    }

    pub(crate) const fn activation(self) -> FailureMarketIntervalCellActivationReceiptV2 {
        self.activation
    }

    pub(crate) const fn predicted_session_transcript_id(self) -> ContentId {
        self.predicted_session_transcript_id
    }

    pub(crate) const fn cell_authentication_before(self) -> ContentId {
        self.cell_authentication_before
    }

    pub(crate) const fn cell_authentication_after(self) -> ContentId {
        self.cell_authentication_after
    }

    pub(crate) const fn link_authentication_before(self) -> ContentId {
        self.link_authentication_before
    }

    pub(crate) const fn link_authentication_after(self) -> ContentId {
        self.link_authentication_after
    }

    pub(crate) const fn runtime_postwrite_id(self) -> ContentId {
        self.runtime_postwrite_id
    }
}

/// Atomically activate one reusable Failure cell and pin its exact Series link.
///
/// The noncircular transcript chain is `P -> predict pin(P) = T -> cell(T) ->
/// pin(P)`. The cell write deliberately precedes the Product link write; any
/// Product refusal after it causes the SVM to roll the whole instruction back.
#[allow(clippy::too_many_arguments)]
pub(crate) fn begin_failure_market_interval_session_v2<'next>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    cell_account: &AccountInfo<'_>,
    history_account: &AccountInfo<'_>,
    admission_root_account: &AccountInfo<'_>,
    runtime_root_account: &AccountInfo<'_>,
    root_before: AuthenticatedMarketLifecycleRootV1<'_>,
    link_before: AuthenticatedSeriesMarketLinkV1<'_>,
    admission: AuthenticatedFailureMarketRootV2,
    runtime_before: AuthenticatedFailureMarketRuntimeRootV1,
    interval_before: AuthenticatedFailureMarketIntervalAccountsV2,
    schedule: AuthenticatedProductFailureBeginScheduleV1,
    source_join: SourcePolicyHandoffJoinV1,
    source_success: SuccessfulEvaluationHandoffV1,
    context: QuantizedIntervalConsensusContextV1<'_>,
    root_rebound_output: &mut MarketLifecycleRootAccountV1,
    link_rebound_output: &'next mut SeriesMarketLinkAccountV1,
) -> Outcome<(
    AuthenticatedFailureMarketIntervalBeginPostwriteV2,
    AuthenticatedFailureMarketIntervalAccountsV2,
    AuthenticatedSeriesMarketLinkV1<'next>,
    AuthenticatedFailureMarketRuntimeSessionPostwriteV1,
)> {
    require_distinct_product_failure_begin_transition_accounts_v2(
        root_account,
        link_account,
        cell_account,
        history_account,
        admission_root_account,
        runtime_root_account,
        admission,
        source_join,
    )?;
    let root_binding = root_before.state().binding();
    let link_binding = link_before.state().binding();
    let policy = admission.state().binding().facts();
    require_begin_receipt_prestates_v2(
        root_account,
        link_account,
        cell_account,
        history_account,
        root_before,
        link_before,
        admission,
        interval_before,
        schedule,
    )?;
    require_exact_successful_source_join_v2(
        source_join,
        source_success,
        link_binding,
        root_binding,
        policy,
    )?;
    let attempt_index = u8::try_from(interval_before.cell().completed_session_count())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let attempt_slot = usize::from(attempt_index);
    require(
        attempt_slot < usize::from(schedule.schedule().recovery_attempt_count)
            && schedule.schedule().recovery_attempt_count
                == interval_before.quote().schedule().attempt_count
            && schedule.schedule().recovery_attempts[attempt_slot].repair_generation
                == link_binding.source_repair_generation
            && link_binding.source_repair_generation
                == source_success.occurrence().repair_generation(),
        ClutchError::MismatchedState,
    )?;
    require_exact_resolved_edge_policy_v1(
        context.resolved_edge_policy,
        schedule.resolved_edge_policy(),
    )?;
    let initial_work = *begin_quantized_interval_consensus_v1(context)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .work();
    let initial_work_id = initial_work
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let preauthorization_id = derive_failure_market_interval_begin_preauthorization_id_v2(
        root_before,
        link_before,
        admission,
        runtime_before,
        interval_before,
        schedule,
        source_join,
        source_success,
        initial_work_id,
        attempt_index,
    )?;
    let predicted_link = link_before
        .state()
        .pin_failure_session(preauthorization_id)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let predicted_session_transcript_id = predicted_link.failure_session_transcript_id();
    require(
        predicted_link.active_failure_sessions() == 1
            && predicted_link.failure_sessions_started()
                == link_before
                    .state()
                    .failure_sessions_started()
                    .checked_add(1)
                    .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?
            && predicted_session_transcript_id
                != link_before.state().failure_session_transcript_id(),
        ClutchError::MismatchedState,
    )?;
    let activation_facts = FailureMarketIntervalCellActivationFactsV2 {
        cell_before: interval_before.cell_state_id(),
        history_root: interval_before.history().history_root(),
        completed_session_count: interval_before.cell().completed_session_count(),
        session_binding_id: SourceContentId::from_bytes(predicted_session_transcript_id.bytes()),
        source_handoff_id: source_success.id(),
        source_repair_generation: source_success.occurrence().repair_generation(),
        session_schedule_id: SourceContentId::from_bytes(schedule.schedule_projection_id().bytes()),
        attempt_index,
        product_work_id: initial_work_id,
    };
    let preauthorization = FailureMarketIntervalBeginPreauthorizationV2 {
        id: preauthorization_id,
        predicted_session_transcript_id,
        activation_facts,
        schedule_projection_id: schedule.schedule_projection_id(),
        source_join_id: source_success.id(),
    };
    let (cell_plan, activation) = plan_activate_failure_market_interval_cell_v2(
        &preauthorization,
        interval_before.cell(),
        admission.state(),
        interval_before.funding(),
        interval_before.history(),
        interval_before.quote(),
        SourceContentId::from_bytes(predicted_session_transcript_id.bytes()),
        SourceContentId::from_bytes(schedule.schedule_projection_id().bytes()),
        source_success,
        context,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        activation.facts() == activation_facts,
        ClutchError::MismatchedState,
    )?;
    let cell_state_after = cell_plan
        .resulting_cell()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let session = FailureMarketSessionDescriptorV1 {
        series_plan_id: link_binding.series_plan_id,
        ordinal: link_binding.ordinal,
        source_occurrence_id: link_binding.source_occurrence_id,
        schedule_id: FailureMarketSessionScheduleIdV1::from_bytes(
            schedule.schedule_projection_id().bytes(),
        ),
        interval_funding_receipt_id: interval_before.funding().id(),
        session_state_commitment: ContentId::from_bytes(cell_state_after.bytes()),
    };
    let runtime_begin_authority = FailureMarketRuntimeBeginAuthorityV1 {
        runtime_before: runtime_before.state_commitment(),
        series_link_before: link_before
            .state()
            .semantic_id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        series_link_after: predicted_link
            .semantic_id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        previous_session_history: runtime_before.state().session_history_commitment(),
        previous_interval_terminal_receipt_id: runtime_before
            .state()
            .interval_terminal_receipt_id(),
        interval_work_account: interval_before.funding().facts().work_account,
        interval_history_account: interval_before.funding().facts().history_account,
        interval_history_state_id: interval_before.history_state_id(),
        completed_session_count: interval_before.history().completed_session_count(),
        begin_preauthorization_id: preauthorization_id,
        session_binding_id: predicted_session_transcript_id,
        session,
    };
    let runtime_plan = plan_begin_failure_market_session_v1(
        &runtime_begin_authority,
        runtime_before.state(),
        admission.state(),
        *link_before.state(),
        preauthorization_id,
        session,
        interval_before.funding(),
        interval_before.history(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        runtime_plan.series_link_before() == *link_before.state()
            && runtime_plan.series_link_after() == predicted_link
            && runtime_plan.resulting_runtime().active_session_pin_id()
                == predicted_session_transcript_id
            && runtime_plan.resulting_runtime().session_state_commitment()
                == ContentId::from_bytes(cell_state_after.bytes()),
        ClutchError::MismatchedState,
    )?;
    let cell_authentication_before = interval_before.cell_authentication_id();
    let interval_after = write_failure_market_interval_begin_plan_v2(
        program_id,
        cell_account,
        history_account,
        admission,
        interval_before,
        cell_plan,
        activation,
        &FailureMarketIntervalBeginWriteV2 {
            preauthorization_id,
            activation,
        },
    )?;
    let cell_authentication_after = interval_after.cell_authentication_id();
    let pin_authority = ProductFailureSessionPinPostwriteV2 {
        root_account: *root_account.key,
        root_authentication_id: root_before.authentication_id(),
        link_account: *link_account.key,
        link_authentication_id: link_before.authentication_id(),
        series_plan_id: link_binding.series_plan_id,
        ordinal: link_binding.ordinal,
        market_instance_id: link_binding.market_instance_id,
        generation: link_binding.generation,
        source_occurrence_id: link_binding.source_occurrence_id,
        begin_admission_receipt_id: preauthorization_id,
        predicted_session_transcript_id,
        cell_authentication_after,
        activation,
    };
    let link_after = pin_series_market_link_failure_v1(
        program_id,
        root_account,
        root_before,
        link_account,
        link_before,
        preauthorization_id,
        &pin_authority,
        root_rebound_output,
        link_rebound_output,
    )?;
    require(
        link_after.state() == &predicted_link
            && link_after.state().failure_session_transcript_id()
                == predicted_session_transcript_id
            && interval_after.cell().session_binding_id().bytes()
                == predicted_session_transcript_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    let runtime_after_commitment = runtime_plan
        .resulting_runtime()
        .commitment()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let runtime_write_facts = FailureMarketRuntimeSessionWriteFactsV1 {
        runtime_before: runtime_before.state_commitment(),
        runtime_after: runtime_after_commitment,
        transition_receipt_id: runtime_plan.receipt_id(),
    };
    let runtime_postwrite = write_failure_market_runtime_session_plan_v1(
        program_id,
        admission_root_account,
        runtime_root_account,
        admission,
        runtime_before,
        runtime_plan,
        &FailureMarketRuntimeBeginWriteV1 {
            expected: runtime_write_facts,
            cell_state_after: ContentId::from_bytes(interval_after.cell_state_id().bytes()),
            runtime_session_state_after: runtime_plan
                .resulting_runtime()
                .session_state_commitment(),
            link_state_after: *link_after.state(),
            planned_link_state_after: runtime_plan.series_link_after(),
            session_binding_id: predicted_session_transcript_id,
        },
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            PRODUCT_FAILURE_BEGIN_POSTWRITE_DOMAIN_V2,
            &preauthorization_id.bytes(),
            &schedule.schedule_projection_id().bytes(),
            &activation.id().bytes(),
            &predicted_session_transcript_id.bytes(),
            &cell_authentication_before.bytes(),
            &cell_authentication_after.bytes(),
            &link_before.authentication_id().bytes(),
            &link_after.authentication_id().bytes(),
            &runtime_postwrite.id().bytes(),
            &runtime_postwrite.transition_receipt_id().bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok((
        AuthenticatedFailureMarketIntervalBeginPostwriteV2 {
            id,
            preauthorization_id,
            schedule_projection_id: schedule.schedule_projection_id(),
            activation,
            predicted_session_transcript_id,
            cell_authentication_before,
            cell_authentication_after,
            link_authentication_before: link_before.authentication_id(),
            link_authentication_after: link_after.authentication_id(),
            runtime_postwrite_id: runtime_postwrite.id(),
        },
        interval_after,
        link_after,
        runtime_postwrite,
    ))
}

#[allow(clippy::too_many_arguments)]
fn require_begin_receipt_prestates_v2(
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    cell_account: &AccountInfo<'_>,
    history_account: &AccountInfo<'_>,
    root: AuthenticatedMarketLifecycleRootV1<'_>,
    link: AuthenticatedSeriesMarketLinkV1<'_>,
    admission: AuthenticatedFailureMarketRootV2,
    interval: AuthenticatedFailureMarketIntervalAccountsV2,
    schedule: AuthenticatedProductFailureBeginScheduleV1,
) -> Outcome<()> {
    let root_binding = root.state().binding();
    let link_binding = link.state().binding();
    let policy = admission.state().binding().facts();
    let admission_state_id = admission
        .state()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        *root_account.key == root.account()
            && *link_account.key == link.account()
            && *cell_account.key == interval.cell_account()
            && *history_account.key == interval.history_account()
            && !root_account.is_writable
            && link_account.is_writable
            && cell_account.is_writable
            && !history_account.is_writable
            && schedule.root_account() == root.account()
            && schedule.root_authentication_id() == root.authentication_id()
            && schedule.link_account() == link.account()
            && schedule.link_authentication_id() == link.authentication_id()
            && schedule.series_plan_id() == link_binding.series_plan_id
            && schedule.ordinal() == link_binding.ordinal
            && schedule.market_instance_id() == root_binding.market_instance_id
            && schedule.market_instance_id() == link_binding.market_instance_id
            && schedule.generation() == root_binding.generation
            && schedule.generation() == link_binding.generation
            && schedule.source_occurrence_id() == link_binding.source_occurrence_id
            && schedule.registry_release_id() == root_binding.registry_release_id
            && schedule.capability_profile_id() == root_binding.capability_profile_id
            && schedule.compiler_bundle_id() == link_binding.compiler_output_id
            && interval.cell_account() != interval.history_account()
            && interval.admission_root_account() == admission.account()
            && interval.admission_state_id() == admission_state_id
            && interval.cell().phase()
                == clutch_failure_policy_runtime::market_interval_cell_v2::FailureMarketIntervalCellPhaseV2::Idle
            && interval.cell().failure_policy_binding_id().bytes()
                == admission.state().binding().id().bytes()
            && interval.cell().market_instance_id() == root_binding.market_instance_id
            && interval.cell().generation() == root_binding.generation
            && policy.market_instance_id == root_binding.market_instance_id
            && policy.generation == root_binding.generation
            && policy.product_template_id.content_id() == root_binding.product_template_id
            && policy.native_claim_basis_id.content_id() == root_binding.native_claim_basis_id
            && policy.recovery_policy_id.content_id() == root_binding.recovery_policy_id
            && policy.price_measure_policy_id.content_id()
                == root_binding.price_measure_policy_id
            && policy.market_genesis_profile_id.content_id()
                == root_binding.market_genesis_profile_id
            && policy.registry_release_id.content_id() == root_binding.registry_release_id
            && policy.capability_profile_id.content_id() == root_binding.capability_profile_id
            && policy.interval_consensus_profile_id.content_id()
                == root_binding.interval_consensus_profile_id
            && policy.source_release_manifest_id.bytes() == root_binding.source_release_id.bytes()
            && policy.source_plane_contract_id.bytes()
                == root_binding.source_plane_contract_id.bytes()
            && policy.source_spec_id.bytes() == root_binding.source_spec_id.bytes()
            && policy.clock_policy_id.bytes() == root_binding.clock_policy_id.bytes()
            && admission.state().binding().id().bytes()
                == root_binding.market_failure_policy_binding_id.bytes(),
        ClutchError::MismatchedState,
    )
}

pub(crate) fn require_exact_successful_source_join_v2(
    source_join: SourcePolicyHandoffJoinV1,
    source_success: SuccessfulEvaluationHandoffV1,
    link_binding: clutch_product_series::SeriesMarketLinkBindingV1,
    root_binding: clutch_product_series::MarketLifecycleBindingV1,
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
                == root_binding.market_failure_policy_binding_id.bytes()
            && source_join.release_authentication_id().bytes()
                == policy.source_release_authentication_id.bytes()
            && source_join.route_id() == occurrence.route_id()
            && source_join.route_id().bytes() == root_binding.source_route_id.bytes()
            && source_join.occurrence_account().bytes() == occurrence.occurrence_account().bytes()
            && source_join.occurrence_account().bytes()
                == link_binding.source_occurrence_account_id.bytes()
            && source_join.source_fact_authentication_id()
                == source_success.result_account_authentication_id()
            && source_join.clock_policy_id() == source_success.clock_policy_id()
            && source_join.clock_policy_id() == occurrence.clock_policy_id()
            && source_join.clock_policy_id().bytes() == root_binding.clock_policy_id.bytes()
            && source_join.clock() == source_success.clock()
            && source_join.source_spec_id() == occurrence.source_spec_id()
            && source_join.source_spec_id().bytes() == root_binding.source_spec_id.bytes()
            && source_join.window_id() == occurrence.window_id()
            && source_join.statistic_key_id() == occurrence.statistic_key_id()
            && occurrence.occurrence_record_id().bytes()
                == link_binding.source_occurrence_id.bytes()
            && occurrence.id().bytes() == link_binding.source_occurrence_receipt_id.bytes()
            && occurrence.occurrence_account_authentication_id().bytes()
                == link_binding
                    .source_occurrence_account_authentication_id
                    .bytes()
            && occurrence.series_plan_id().bytes() == link_binding.series_plan_id.bytes()
            && occurrence.ordinal() == link_binding.ordinal
            && occurrence.market_instance_id().bytes() == link_binding.market_instance_id.bytes()
            && occurrence.attachment_plan_id().bytes() == link_binding.attachment_plan_id.bytes()
            && occurrence.source_plane_contract_id().bytes()
                == root_binding.source_plane_contract_id.bytes()
            && occurrence.source_spec_id().bytes() == root_binding.source_spec_id.bytes()
            && statistic_result_id
                == source_success
                    .result()
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            && source_join.work_receipt_account().bytes() != [0; 32]
            && source_join.work_receipt_authentication_id() != SourceContentId::ZERO
            && source_join.id() != SourceContentId::ZERO,
        ClutchError::MismatchedState,
    )
}

#[allow(clippy::too_many_arguments)]
fn derive_failure_market_interval_begin_preauthorization_id_v2(
    root: AuthenticatedMarketLifecycleRootV1<'_>,
    link: AuthenticatedSeriesMarketLinkV1<'_>,
    admission: AuthenticatedFailureMarketRootV2,
    runtime: AuthenticatedFailureMarketRuntimeRootV1,
    interval: AuthenticatedFailureMarketIntervalAccountsV2,
    schedule: AuthenticatedProductFailureBeginScheduleV1,
    source_join: SourcePolicyHandoffJoinV1,
    source_success: SuccessfulEvaluationHandoffV1,
    initial_work_id: clutch_product_series::QuantizedIntervalConsensusWorkV1Id,
    attempt_index: u8,
) -> Outcome<ContentId> {
    let admission_state_id = admission
        .state()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let statistic_result_id = source_success
        .statistic_result_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            PRODUCT_FAILURE_BEGIN_PREAUTHORIZATION_DOMAIN_V2,
            root.account().as_ref(),
            &root.authentication_id().bytes(),
            link.account().as_ref(),
            &link.authentication_id().bytes(),
            &link.state().failure_session_transcript_id().bytes(),
            &link.state().transition_sequence().to_le_bytes(),
            admission.account().as_ref(),
            &admission_state_id.bytes(),
            runtime.account().as_ref(),
            &runtime.state_commitment().bytes(),
            interval.cell_account().as_ref(),
            &interval.cell_authentication_id().bytes(),
            &interval.cell_state_id().bytes(),
            interval.history_account().as_ref(),
            &interval.history_authentication_id().bytes(),
            &interval.history_state_id().bytes(),
            &interval.history().history_root().bytes(),
            &interval.cell().completed_session_count().to_le_bytes(),
            &schedule.id().bytes(),
            &schedule.schedule_projection_id().bytes(),
            &source_join.id().bytes(),
            &source_join.release_authentication_id().bytes(),
            &source_join.route_id().bytes(),
            &source_join.occurrence_account().bytes(),
            &source_join.result_or_absence_account().bytes(),
            &source_join.source_fact_authentication_id().bytes(),
            &source_join.work_receipt_account().bytes(),
            &source_join.work_receipt_authentication_id().bytes(),
            &source_join.generation().to_le_bytes(),
            &source_success.id().bytes(),
            &source_success.result_account_authentication_id().bytes(),
            &statistic_result_id.bytes(),
            &source_success.occurrence().occurrence_record_id().bytes(),
            &source_success
                .occurrence()
                .repair_generation()
                .to_le_bytes(),
            &initial_work_id.bytes(),
            &[attempt_index],
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(id)
}

fn require_distinct_product_failure_begin_transition_accounts_v2(
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    cell_account: &AccountInfo<'_>,
    history_account: &AccountInfo<'_>,
    admission_root_account: &AccountInfo<'_>,
    runtime_root_account: &AccountInfo<'_>,
    admission: AuthenticatedFailureMarketRootV2,
    source_join: SourcePolicyHandoffJoinV1,
) -> Outcome<()> {
    let accounts = [
        *root_account.key,
        *link_account.key,
        *cell_account.key,
        *history_account.key,
        *admission_root_account.key,
        *runtime_root_account.key,
        Pubkey::new_from_array(source_join.occurrence_account().bytes()),
        Pubkey::new_from_array(source_join.result_or_absence_account().bytes()),
        Pubkey::new_from_array(source_join.work_receipt_account().bytes()),
    ];
    require(
        *admission_root_account.key == admission.account(),
        ClutchError::MismatchedState,
    )?;
    require_distinct_begin_transition_keys_v2(accounts)
}

fn require_distinct_begin_transition_keys_v2(accounts: [Pubkey; 10]) -> Outcome<()> {
    let mut index = 0_usize;
    while index < accounts.len() {
        let mut other = index + 1;
        while other < accounts.len() {
            require(
                accounts[index] != accounts[other],
                ClutchError::AccountAlias,
            )?;
            other += 1;
        }
        index += 1;
    }
    Ok(())
}

/// Authenticate and deterministically recompile one current V5 ordinal.
///
/// Both mutable lifecycle receipts are hostile-reopened before their facts are
/// committed. The link remains unmodified; the atomic Failure Begin composer
/// consumes this receipt and performs the sole cell-activation/link-pin batch.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_product_failure_begin_schedule_v1<'root, 'link>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    root_before: AuthenticatedMarketLifecycleRootV1<'_>,
    link_before: AuthenticatedSeriesMarketLinkV1<'_>,
    registry: AuthenticatedRegistryCapabilityV3,
    bundle_account: &AccountInfo<'_>,
    series_account: &AccountInfo<'_>,
    template_account: &AccountInfo<'_>,
    basis_account: &AccountInfo<'_>,
    recovery_account: &AccountInfo<'_>,
    price_policy_account: &AccountInfo<'_>,
    genesis_account: &AccountInfo<'_>,
    attachment_account: &AccountInfo<'_>,
    market_account: &AccountInfo<'_>,
    root_decode: &'root mut MarketLifecycleRootAccountV1,
    link_decode: &'link mut SeriesMarketLinkAccountV1,
) -> Outcome<AuthenticatedProductFailureBeginScheduleV1> {
    let expected_root_binding = root_before.state().binding();
    let expected_link_binding = link_before.state().binding();
    let root = authenticate_market_lifecycle_root_v1(
        program_id,
        root_account,
        expected_root_binding.market_instance_id,
        expected_root_binding.generation,
        false,
        root_decode,
    )?;
    let link = authenticate_series_market_link_v1(
        program_id,
        link_account,
        expected_link_binding.series_plan_id,
        expected_link_binding.ordinal,
        expected_link_binding.market_instance_id,
        expected_link_binding.generation,
        *root_account.key,
        true,
        link_decode,
    )?;
    require_cached_root_and_link(root_before, root, link_before, link)?;
    require_distinct_product_failure_begin_accounts(
        registry,
        [
            *root_account.key,
            *link_account.key,
            *bundle_account.key,
            *series_account.key,
            *template_account.key,
            *basis_account.key,
            *recovery_account.key,
            *price_policy_account.key,
            *genesis_account.key,
            *attachment_account.key,
            *market_account.key,
        ],
    )?;

    let root_binding = root.state().binding();
    let link_binding = link.state().binding();
    let bundle = authenticate_product_artifact_v1::<CompiledProductSeriesBundleV5>(
        program_id,
        bundle_account,
        registry.compiler_bundle_id(),
    )?;
    let bundle_value = bundle.value();
    let series = authenticate_product_artifact_v1::<SeriesPlanV5>(
        program_id,
        series_account,
        bundle_value.series_plan_id.content_id(),
    )?;
    let template = authenticate_product_artifact_v1::<ProductTemplateV4>(
        program_id,
        template_account,
        bundle_value.product_template_id.content_id(),
    )?;
    let basis = authenticate_product_artifact_v1::<NativeClaimBasisV1>(
        program_id,
        basis_account,
        bundle_value.native_claim_basis_id.content_id(),
    )?;
    let recovery = authenticate_product_artifact_v1::<EvidenceOnlyRecoveryPolicyV1>(
        program_id,
        recovery_account,
        bundle_value.evidence_only_recovery_policy_id.content_id(),
    )?;
    let price = authenticate_product_artifact_v1::<PriceMeasurePolicyV1>(
        program_id,
        price_policy_account,
        bundle_value.price_measure_policy_id.content_id(),
    )?;
    let genesis = authenticate_product_artifact_v1::<MarketGenesisProfileV2>(
        program_id,
        genesis_account,
        bundle_value.market_genesis_profile_id.content_id(),
    )?;
    let attachment = authenticate_product_artifact_v1::<SeriesAttachmentPlanV4>(
        program_id,
        attachment_account,
        bundle_value.attachment_plan_id.content_id(),
    )?;
    let market = authenticate_product_artifact_v1::<MarketInstancePreimageV2>(
        program_id,
        market_account,
        root_binding.market_instance_id.content_id(),
    )?;

    require_current_product_failure_begin_graph_v1(
        root,
        link,
        registry,
        &bundle,
        &series,
        &template,
        &basis,
        &recovery,
        &price,
        &genesis,
        &attachment,
    )?;
    let compiled = compile_ordinal_v5(
        series.value(),
        template.value(),
        basis.value(),
        recovery.value(),
        price.value(),
        genesis.value(),
        attachment.value(),
        &registry.projection(),
        link_binding.ordinal,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        compiled.series_plan_id == link_binding.series_plan_id
            && compiled.ordinal == link_binding.ordinal
            && compiled.market_instance_id == root_binding.market_instance_id
            && compiled.market_instance_id == link_binding.market_instance_id
            && compiled.market == *market.value()
            && compiled.attachment_plan_id.bytes() == bundle_value.attachment_plan_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    compiled
        .schedule
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let provenance = ProductFailureBeginCompilerProvenanceV1 {
        registry_release_id: registry.registry_release_id(),
        capability_profile_id: registry.capability_profile_id(),
        compiler_bundle_id: bundle.semantic_id(),
        series_plan_id: compiled.series_plan_id,
        ordinal: compiled.ordinal,
        market_instance_id: compiled.market_instance_id,
        product_template_id: template.semantic_id(),
        native_claim_basis_id: basis.semantic_id(),
        recovery_policy_id: recovery.semantic_id(),
        price_measure_policy_id: price.semantic_id(),
        market_genesis_profile_id: genesis.semantic_id(),
        attachment_plan_id: attachment.semantic_id(),
    };
    let schedule_projection_id =
        derive_product_failure_begin_schedule_projection_v1(compiled.schedule, provenance)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            PRODUCT_FAILURE_BEGIN_SCHEDULE_AUTHENTICATION_DOMAIN_V1,
            &schedule_projection_id.bytes(),
            root.account().as_ref(),
            &root.authentication_id().bytes(),
            link.account().as_ref(),
            &link.authentication_id().bytes(),
            registry.series_registry_account().as_ref(),
            registry.program_account().as_ref(),
            registry.programdata_account().as_ref(),
            registry.release_artifact_account().as_ref(),
            registry.profile_artifact_account().as_ref(),
            bundle.account().as_ref(),
            series.account().as_ref(),
            template.account().as_ref(),
            basis.account().as_ref(),
            recovery.account().as_ref(),
            price.account().as_ref(),
            genesis.account().as_ref(),
            attachment.account().as_ref(),
            market.account().as_ref(),
            &link_binding.source_occurrence_id.bytes(),
            &root_binding.generation.to_le_bytes(),
            &[registry.edge_policy_registry_value()],
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(AuthenticatedProductFailureBeginScheduleV1 {
        id,
        schedule_projection_id,
        schedule: compiled.schedule,
        root_account: root.account(),
        root_authentication_id: root.authentication_id(),
        link_account: link.account(),
        link_authentication_id: link.authentication_id(),
        series_plan_id: compiled.series_plan_id,
        ordinal: compiled.ordinal,
        market_instance_id: compiled.market_instance_id,
        generation: root_binding.generation,
        source_occurrence_id: link_binding.source_occurrence_id,
        registry_release_id: provenance.registry_release_id,
        capability_profile_id: provenance.capability_profile_id,
        compiler_bundle_id: provenance.compiler_bundle_id,
        edge_policy_registry_value: registry.edge_policy_registry_value(),
        resolved_edge_policy: registry.resolved_edge_policy(),
    })
}

#[allow(clippy::too_many_arguments)]
fn require_current_product_failure_begin_graph_v1(
    root: AuthenticatedMarketLifecycleRootV1<'_>,
    link: AuthenticatedSeriesMarketLinkV1<'_>,
    registry: AuthenticatedRegistryCapabilityV3,
    bundle: &AuthenticatedProductArtifactV1<CompiledProductSeriesBundleV5>,
    series: &AuthenticatedProductArtifactV1<SeriesPlanV5>,
    template: &AuthenticatedProductArtifactV1<ProductTemplateV4>,
    basis: &AuthenticatedProductArtifactV1<NativeClaimBasisV1>,
    recovery: &AuthenticatedProductArtifactV1<EvidenceOnlyRecoveryPolicyV1>,
    price: &AuthenticatedProductArtifactV1<PriceMeasurePolicyV1>,
    genesis: &AuthenticatedProductArtifactV1<MarketGenesisProfileV2>,
    attachment: &AuthenticatedProductArtifactV1<SeriesAttachmentPlanV4>,
) -> Outcome<()> {
    let root_state = root.state();
    let root_binding = root_state.binding();
    let root_binding_id = root_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_state = *link.state();
    let link_binding = link_state.binding();
    let bundle_value = bundle.value();
    let projection = registry.projection();
    ProductFailureBeginGraphJoinV1 {
        registry_terms: registry.funding_terms_id().content_id(),
        bundle_terms: bundle_value.funding_terms_id.content_id(),
        link_terms: link_binding.funding_terms_id.content_id(),
        bundle_quote: bundle_value.funding_quote_id.content_id(),
        link_quote: link_binding.funding_quote_id.content_id(),
        attachment_quote: attachment.value().funding_quote_id.content_id(),
        bundle_attachment: bundle_value.attachment_plan_id.content_id(),
        link_attachment: link_binding.attachment_plan_id,
        attachment_semantic: attachment.semantic_id(),
        registry_release: registry.registry_release_id(),
        projection_release: projection.registry_release_id,
        bundle_release: bundle_value.registry_release_id,
        root_release: root_binding.registry_release_id,
        registry_profile: registry.capability_profile_id(),
        projection_profile: projection.capability_profile_id,
        bundle_profile: bundle_value.capability_profile_id.content_id(),
        link_profile: link_binding.capability_profile_id,
        root_profile: root_binding.capability_profile_id,
        bundle_source_release: bundle_value.source_release_manifest_id,
        link_source_release: link_binding.source_release_id,
        root_source_release: root_binding.source_release_id,
        bundle_source_plane: bundle_value.source_plane_contract_id,
        link_source_plane: link_binding.source_plane_contract_id,
        root_source_plane: root_binding.source_plane_contract_id,
        bundle_source_spec: bundle_value.source_spec_id,
        link_source_spec: link_binding.source_spec_id,
        root_source_spec: root_binding.source_spec_id,
        link_source_route: link_binding.source_route_id,
        root_source_route: root_binding.source_route_id,
        link_clock_policy: link_binding.clock_policy_id,
        root_clock_policy: root_binding.clock_policy_id,
    }
    .validate()?;
    require(
        !root.is_writable()
            && link.is_writable()
            && root_state.phase() == MarketLifecyclePhaseV1::Active
            && root_state.resolution_semantic_id() == ContentId::ZERO
            && root_state.resolution_data_id() == ContentId::ZERO
            && root_state.resolution_activation_receipt_id() == ContentId::ZERO
            && link_state.phase() == SeriesMarketLinkPhaseV1::Active
            && link_state.active_failure_sessions() == 0
            && link_binding.market_root_account_id.bytes() == root.account().to_bytes()
            && link_binding.market_binding_id == root_binding_id
            && link_binding.market_instance_id == root_binding.market_instance_id
            && link_binding.generation == root_binding.generation
            && registry.series_plan_id() == link_binding.series_plan_id
            && registry.compiler_bundle_id() == bundle.semantic_id()
            && bundle_value.series_plan_id == link_binding.series_plan_id
            && series.semantic_id() == bundle_value.series_plan_id.content_id()
            && template.semantic_id() == bundle_value.product_template_id.content_id()
            && basis.semantic_id() == bundle_value.native_claim_basis_id.content_id()
            && recovery.semantic_id() == bundle_value.evidence_only_recovery_policy_id.content_id()
            && price.semantic_id() == bundle_value.price_measure_policy_id.content_id()
            && genesis.semantic_id() == bundle_value.market_genesis_profile_id.content_id()
            && attachment.semantic_id() == bundle_value.attachment_plan_id.content_id()
            && bundle_value.product_template_id.content_id() == root_binding.product_template_id
            && bundle_value.native_claim_basis_id.content_id()
                == root_binding.native_claim_basis_id
            && bundle_value.evidence_only_recovery_policy_id.content_id()
                == root_binding.recovery_policy_id
            && bundle_value.price_measure_policy_id.content_id()
                == root_binding.price_measure_policy_id
            && bundle_value.market_genesis_profile_id.content_id()
                == root_binding.market_genesis_profile_id
            && link_binding.compiler_output_id == bundle.semantic_id(),
        ClutchError::MismatchedState,
    )
}

pub(crate) fn require_cached_root_and_link(
    expected_root: AuthenticatedMarketLifecycleRootV1<'_>,
    live_root: AuthenticatedMarketLifecycleRootV1<'_>,
    expected_link: AuthenticatedSeriesMarketLinkV1<'_>,
    live_link: AuthenticatedSeriesMarketLinkV1<'_>,
) -> Outcome<()> {
    require(
        expected_root.account() == live_root.account()
            && expected_root.owner_program() == live_root.owner_program()
            && expected_root.state() == live_root.state()
            && expected_root.observed_lamports() == live_root.observed_lamports()
            && expected_root.data_id() == live_root.data_id()
            && expected_root.authentication_id() == live_root.authentication_id()
            && expected_link.account() == live_link.account()
            && expected_link.owner_program() == live_link.owner_program()
            && expected_link.state() == live_link.state()
            && expected_link.observed_lamports() == live_link.observed_lamports()
            && expected_link.data_id() == live_link.data_id()
            && expected_link.authentication_id() == live_link.authentication_id(),
        ClutchError::MismatchedState,
    )
}

fn require_distinct_product_failure_begin_accounts(
    registry: AuthenticatedRegistryCapabilityV3,
    operation_accounts: [Pubkey; 11],
) -> Outcome<()> {
    let authority_accounts = [
        registry.series_registry_account(),
        registry.program_account(),
        registry.programdata_account(),
        registry.release_artifact_account(),
        registry.profile_artifact_account(),
    ];
    let mut operation_index = 0_usize;
    while operation_index < operation_accounts.len() {
        let mut other_operation_index = operation_index + 1;
        while other_operation_index < operation_accounts.len() {
            require(
                operation_accounts[operation_index] != operation_accounts[other_operation_index],
                ClutchError::AccountAlias,
            )?;
            other_operation_index += 1;
        }
        let mut authority_index = 0_usize;
        while authority_index < authority_accounts.len() {
            require(
                operation_accounts[operation_index] != authority_accounts[authority_index],
                ClutchError::AccountAlias,
            )?;
            authority_index += 1;
        }
        operation_index += 1;
    }
    let mut authority_index = 0_usize;
    while authority_index < authority_accounts.len() {
        let mut other_authority_index = authority_index + 1;
        while other_authority_index < authority_accounts.len() {
            require(
                authority_accounts[authority_index] != authority_accounts[other_authority_index],
                ClutchError::AccountAlias,
            )?;
            other_authority_index += 1;
        }
        authority_index += 1;
    }
    Ok(())
}

fn require_live_content_id(id: ContentId) -> Outcome<()> {
    id.validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

pub(crate) fn require_exact_resolved_edge_policy_v1(
    supplied: clutch_product_series::QuantizedEdgePolicyV1,
    authenticated: clutch_product_series::QuantizedEdgePolicyV1,
) -> Outcome<()> {
    require(supplied == authenticated, ClutchError::MismatchedState)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_product_series::{AbsoluteRecoveryAttemptV1, MAX_RECOVERY_ATTEMPTS};

    fn id(byte: u8) -> ContentId {
        ContentId::from_bytes([byte; 32])
    }

    fn schedule() -> CompiledScheduleV1 {
        let mut attempts = [AbsoluteRecoveryAttemptV1::ZERO; MAX_RECOVERY_ATTEMPTS];
        attempts[0] = AbsoluteRecoveryAttemptV1 {
            repair_generation: 7,
            opens_at_bucket: 30,
            closes_at_bucket: 40,
        };
        CompiledScheduleV1 {
            start_bucket: 10,
            end_bucket_exclusive: 20,
            primary_maturity_bucket_exclusive: 25,
            recovery_attempt_count: 1,
            recovery_attempts: attempts,
        }
    }

    fn provenance() -> ProductFailureBeginCompilerProvenanceV1 {
        ProductFailureBeginCompilerProvenanceV1 {
            registry_release_id: id(1),
            capability_profile_id: id(2),
            compiler_bundle_id: id(3),
            series_plan_id: SeriesPlanV5Id::from_bytes([4; 32]),
            ordinal: 5,
            market_instance_id: MarketInstanceV2Id::from_bytes([6; 32]),
            product_template_id: id(7),
            native_claim_basis_id: id(8),
            recovery_policy_id: id(9),
            price_measure_policy_id: id(10),
            market_genesis_profile_id: id(11),
            attachment_plan_id: id(12),
        }
    }

    fn graph() -> ProductFailureBeginGraphJoinV1 {
        ProductFailureBeginGraphJoinV1 {
            registry_terms: id(1),
            bundle_terms: id(1),
            link_terms: id(1),
            bundle_quote: id(2),
            link_quote: id(2),
            attachment_quote: id(2),
            bundle_attachment: id(3),
            link_attachment: id(3),
            attachment_semantic: id(3),
            registry_release: id(4),
            projection_release: id(4),
            bundle_release: id(4),
            root_release: id(4),
            registry_profile: id(5),
            projection_profile: id(5),
            bundle_profile: id(5),
            link_profile: id(5),
            root_profile: id(5),
            bundle_source_release: id(6),
            link_source_release: id(6),
            root_source_release: id(6),
            bundle_source_plane: id(7),
            link_source_plane: id(7),
            root_source_plane: id(7),
            bundle_source_spec: id(8),
            link_source_spec: id(8),
            root_source_spec: id(8),
            link_source_route: id(9),
            root_source_route: id(9),
            link_clock_policy: id(10),
            root_clock_policy: id(10),
        }
    }

    #[test]
    fn full_schedule_body_and_every_provenance_role_change_projection() {
        let original =
            derive_product_failure_begin_schedule_projection_v1(schedule(), provenance()).unwrap();

        let mut altered_schedule = schedule();
        altered_schedule.recovery_attempts[0].closes_at_bucket = 41;
        assert_ne!(
            derive_product_failure_begin_schedule_projection_v1(altered_schedule, provenance())
                .unwrap(),
            original
        );

        let mut altered = provenance();
        altered.registry_release_id = id(13);
        assert_ne!(
            derive_product_failure_begin_schedule_projection_v1(schedule(), altered).unwrap(),
            original
        );
        let mut altered = provenance();
        altered.capability_profile_id = id(13);
        assert_ne!(
            derive_product_failure_begin_schedule_projection_v1(schedule(), altered).unwrap(),
            original
        );
        let mut altered = provenance();
        altered.compiler_bundle_id = id(13);
        assert_ne!(
            derive_product_failure_begin_schedule_projection_v1(schedule(), altered).unwrap(),
            original
        );
        let mut altered = provenance();
        altered.series_plan_id = SeriesPlanV5Id::from_bytes([13; 32]);
        assert_ne!(
            derive_product_failure_begin_schedule_projection_v1(schedule(), altered).unwrap(),
            original
        );
        let mut altered = provenance();
        altered.ordinal = 13;
        assert_ne!(
            derive_product_failure_begin_schedule_projection_v1(schedule(), altered).unwrap(),
            original
        );
        let mut altered = provenance();
        altered.market_instance_id = MarketInstanceV2Id::from_bytes([13; 32]);
        assert_ne!(
            derive_product_failure_begin_schedule_projection_v1(schedule(), altered).unwrap(),
            original
        );
        for role in 0_u8..6_u8 {
            let mut altered = provenance();
            match role {
                0 => altered.product_template_id = id(13),
                1 => altered.native_claim_basis_id = id(13),
                2 => altered.recovery_policy_id = id(13),
                3 => altered.price_measure_policy_id = id(13),
                4 => altered.market_genesis_profile_id = id(13),
                5 => altered.attachment_plan_id = id(13),
                _ => unreachable!(),
            }
            assert_ne!(
                derive_product_failure_begin_schedule_projection_v1(schedule(), altered).unwrap(),
                original
            );
        }
    }

    #[test]
    fn noncanonical_schedule_is_never_projected() {
        let mut invalid = schedule();
        invalid.recovery_attempts[1] = invalid.recovery_attempts[0];
        assert!(
            derive_product_failure_begin_schedule_projection_v1(invalid, provenance()).is_err()
        );
    }

    #[test]
    fn registry_bundle_link_and_root_splices_refuse() {
        assert!(graph().validate().is_ok());

        let mut altered = graph();
        altered.link_terms = id(20);
        assert!(altered.validate().is_err());
        let mut altered = graph();
        altered.link_quote = id(20);
        assert!(altered.validate().is_err());
        let mut altered = graph();
        altered.attachment_quote = id(20);
        assert!(altered.validate().is_err());
        let mut altered = graph();
        altered.link_attachment = id(20);
        assert!(altered.validate().is_err());
        let mut altered = graph();
        altered.root_release = id(20);
        assert!(altered.validate().is_err());
        let mut altered = graph();
        altered.bundle_profile = id(20);
        assert!(altered.validate().is_err());
        let mut altered = graph();
        altered.root_source_release = id(20);
        assert!(altered.validate().is_err());
        let mut altered = graph();
        altered.link_source_plane = id(20);
        assert!(altered.validate().is_err());
        let mut altered = graph();
        altered.bundle_source_spec = id(20);
        assert!(altered.validate().is_err());
        let mut altered = graph();
        altered.link_source_route = id(20);
        assert!(altered.validate().is_err());
        let mut altered = graph();
        altered.link_clock_policy = id(20);
        assert!(altered.validate().is_err());
    }

    #[test]
    fn resolved_edge_substitution_refuses() {
        assert!(require_exact_resolved_edge_policy_v1(
            clutch_product_series::QuantizedEdgePolicyV1::Clamp,
            clutch_product_series::QuantizedEdgePolicyV1::Clamp,
        )
        .is_ok());
        assert!(require_exact_resolved_edge_policy_v1(
            clutch_product_series::QuantizedEdgePolicyV1::Refuse,
            clutch_product_series::QuantizedEdgePolicyV1::Clamp,
        )
        .is_err());
    }

    #[test]
    fn every_begin_transition_account_alias_refuses() {
        let original = [
            Pubkey::new_from_array([1; 32]),
            Pubkey::new_from_array([2; 32]),
            Pubkey::new_from_array([3; 32]),
            Pubkey::new_from_array([4; 32]),
            Pubkey::new_from_array([5; 32]),
            Pubkey::new_from_array([6; 32]),
            Pubkey::new_from_array([7; 32]),
            Pubkey::new_from_array([8; 32]),
            Pubkey::new_from_array([9; 32]),
            Pubkey::new_from_array([10; 32]),
        ];
        assert!(require_distinct_begin_transition_keys_v2(original).is_ok());
        let mut left = 0_usize;
        while left < original.len() {
            let mut right = left + 1;
            while right < original.len() {
                let mut aliased = original;
                aliased[right] = aliased[left];
                assert!(require_distinct_begin_transition_keys_v2(aliased).is_err());
                right += 1;
            }
            left += 1;
        }
    }

    #[test]
    fn sole_outer_begin_orders_cell_before_product_pin_for_atomic_rollback() {
        let source = include_str!("product_failure_begin.rs");
        let outer = source
            .find("pub(crate) fn begin_failure_market_interval_session_v2")
            .unwrap();
        let cell_write = source[outer..]
            .find("let interval_after = write_failure_market_interval_begin_plan_v2")
            .unwrap();
        let product_pin = source[outer..]
            .find("let link_after = pin_series_market_link_failure_v1")
            .unwrap();
        let runtime_write = source[outer..]
            .find("let runtime_postwrite = write_failure_market_runtime_session_plan_v1")
            .unwrap();
        assert!(cell_write < product_pin && product_pin < runtime_write);
        assert_eq!(
            source
                .matches("pub(crate) fn begin_failure_market_interval_session_v2")
                .count(),
            1
        );
        assert!(!source.contains("write_failure_market_interval_cell_plan_v2"));
    }
}
