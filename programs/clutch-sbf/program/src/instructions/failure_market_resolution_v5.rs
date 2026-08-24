// SPDX-License-Identifier: AGPL-3.0-or-later
//! Atomic Product/Failure/Source Resolution V5 composer.
//!
//! Recovery78/v1 action 12 reaches only the concrete outer in this module.
//! It is the single live writer boundary which joins Product's authenticated active Market root,
//! pinned Series link, physical FundingV5, and Product-founded inactive
//! ResolutionV5 to Failure's private
//! exhaustive interval receipt and Collateral's exact Hoard/ClaimLedger
//! postimage verifier. The already initialized Resolution PDA supplies only its
//! separately itemized rent principal; no Recovery work capital, Hoard
//! principal, future fees, or caller-selected funding source participates.

use crate::accounts::{expect_pda, require, require_distinct, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::collateral_position_v3::{
    authenticate_current_market_resolution_activation_postwrite_v5,
    AuthenticatedMarketResolutionActivationPostwriteV5, GeneralMarketLiabilityAuthorityV5,
    RuntimeSha256,
};
use crate::instructions::failure_market_admission::AuthenticatedFailureMarketRootV3;
use crate::instructions::failure_market_foundation_v4::AuthenticatedInactiveFailureResolutionV5;
use crate::instructions::failure_market_interval_v2::{
    archive_resolved_failure_market_interval_link_v4,
    write_failure_market_interval_resolution_plan_v2,
    AuthenticatedFailureMarketIntervalAccountsV2, AuthenticatedFailureMarketProductResolutionV2,
    FailureMarketIntervalArchivePostwriteV3,
};
use crate::instructions::failure_market_family_terminal_v2::{
    persist_resolved_failure_market_family_v3,
    AuthenticatedFailureMarketFamilyTerminalPostwriteV2,
};
use crate::instructions::failure_market_recovery_terminal_v2::{
    close_failure_market_recovery_v2, AuthenticatedFailureMarketRecoveryClosePostwriteV2,
};
use crate::instructions::failure_market_replay_v2::AuthenticatedFailureMarketReplayV2;
use crate::instructions::failure_market_runtime::{
    write_failure_market_runtime_session_plan_v3, AuthenticatedFailureMarketRuntimeRootV1,
    AuthenticatedFailureMarketRuntimeSessionPostwriteV3,
    AuthenticatedFailureMarketRuntimeSessionWriteV3, FailureMarketRuntimeSessionWriteFactsV3,
};
use crate::instructions::product_failure_link_v3_current::{
    authenticate_writable_failure_resolution_link_v4,
    AuthenticatedSeriesFailureSessionReleaseV4,
    AuthenticatedWritableFailureSessionReleaseLinkV4, FailureSessionReleaseDispositionV4,
};
use crate::instructions::product_market_lifecycle_v3_current::{
    authenticate_market_lifecycle_root_v3, authenticate_series_market_link_v3,
    record_current_market_resolution_activation_v3,
    AuthenticatedCurrentMarketResolutionWriteV3, AuthenticatedMarketLifecycleRootV3,
    AuthenticatedSeriesMarketLinkV3,
};
use crate::instructions::product_series_current::{
    AuthenticatedRegistryCapabilityV5, AuthenticatedSeriesFundingAccountV5,
    FailureSessionReleaseDispositionV3,
};
use crate::instructions::product_source_current::AuthenticatedCompiledProductSeriesBundleV7;
use crate::instructions::product_source_current::AuthenticatedSourceResolutionInputV4;
use crate::instructions::source_terminal_resolution_v5::{
    close_successful_source_statistic_result_v1, compose_source_resolution_terminal_v1,
    AuthenticatedSourceResolutionStatisticResultCloseV1,
    AuthenticatedSourceResolutionTerminalPolicyV1, AuthenticatedSourceResolutionTerminalV1,
    AuthenticatedSourceResolutionV5TerminalV1,
};
use crate::instructions::source_resolution_product_release_v1::{
    bind_source_resolution_product_release_v1,
    AuthenticatedSourceResolutionProductReleaseAuthorityV1,
    AuthenticatedSourceResolutionProductReleaseV1, SourceResolutionProductReleaseFactsV1,
};
use crate::seeds;
use clutch_collateral_adapter_v2::{
    prepare_market_resolution_activation_v5, ClaimLedgerV3, HoardV2, Id as CollateralId,
    MarketLiabilityLifecycleV1, ResolutionFinalizationFactsV5, ResolutionPayoutUnitBoundaryV5,
    ResolutionV5, CLAIM_LEDGER_V3_BYTES, HOARD_V2_BYTES, RESOLUTION_V5_BYTES,
};
use clutch_failure_policy_runtime::market_interval_cell_v2::{
    plan_reset_failure_market_interval_cell_v2, plan_resolve_failure_market_interval_cell_v2,
    project_failure_market_interval_terminal_history_facts_v2,
    AuthenticatedFailureMarketIntervalCellResolutionV2, FailureMarketIntervalCellDispositionV2,
    FailureMarketIntervalCellPlanV2, FailureMarketIntervalCellResetReceiptV2,
    FailureMarketIntervalCellResolutionFactsV2, FailureMarketIntervalCellResolutionPlanV2,
    FailureMarketIntervalCellResolutionReceiptV2,
};
use clutch_failure_policy_runtime::market_interval_history_v2::{
    plan_append_failure_market_interval_history_v2,
    AuthenticatedFailureMarketIntervalTerminalV2, FailureMarketIntervalHistoryAppendReceiptV2,
    FailureMarketIntervalHistoryPlanV2, FailureMarketIntervalTerminalDispositionV2,
    FailureMarketIntervalTerminalFactsV2,
};
use clutch_failure_policy_runtime::market_runtime_v1::{
    plan_close_failure_market_session_v3, plan_resolve_failure_market_session_v3,
    AuthenticatedFailureMarketSessionV3, FailureMarketSessionCloseFactsV3,
    FailureMarketSessionResolutionFactsV3, FailureMarketSessionTransitionReceiptIdV3,
};
use clutch_product_series::{
    AuthenticatedQuantizedIntervalConsensusHistoryV1, ContentId,
    MarketLifecycleBindingV3, MarketLifecyclePhaseV3, MarketResolutionActivationV3,
    QuantizedIntervalConsensusCertificateV1, QuantizedIntervalConsensusContextV1,
    QuantizedIntervalConsensusRestorationV1, SeriesAttachmentPlanV6Id, SeriesFundingQuoteV6Id,
    SeriesFundingTermsV2Id, SeriesMarketLinkPhaseV3,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV3, SeriesMarketLinkAccountV3,
};
use clutch_solana_layout::failure_action12_projection::{
    project_failure_action12_resolution_v5, project_failure_resolution_activation_v5,
    project_failure_resolution_physical_postwrite_v6,
    FailureAction12FinalizationEvidenceProjectionV5, FailureAction12ReceiptProjectionV5,
    FailureAction12RegistryProjectionV5,
};
use clutch_source_plane_v3_runtime::{
    AuthenticatedReopenLineageV1, AuthenticatedSourceRouteV1, SourceWorkScheduleBindingV1,
    SuccessfulEvaluationHandoffV1,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const _: () = assert!(clutch_retirement::MAX_OUTCOMES * 8 == 128);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketResolvedArchiveAuthorityV5 {
    expected: FailureMarketIntervalTerminalFactsV2,
}

impl AuthenticatedFailureMarketIntervalTerminalV2 for FailureMarketResolvedArchiveAuthorityV5 {
    fn authenticate_failure_market_interval_terminal(
        &self,
        expected: FailureMarketIntervalTerminalFactsV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if expected != self.expected {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketResolvedArchivePlanV5 {
    history_plan: FailureMarketIntervalHistoryPlanV2,
    append: FailureMarketIntervalHistoryAppendReceiptV2,
    cell_plan: FailureMarketIntervalCellPlanV2,
    reset: FailureMarketIntervalCellResetReceiptV2,
}

impl AuthenticatedSourceResolutionProductReleaseAuthorityV1
    for FailureMarketIntervalArchivePostwriteV3
{
    fn authenticate_source_resolution_product_release_v1(
        &self,
        expected: SourceResolutionProductReleaseFactsV1,
    ) -> Outcome<()> {
        require(
            expected.product_archive_postwrite_id.bytes() == self.id().bytes()
                && expected.product_append_receipt_id.bytes() == self.append().id().bytes()
                && expected.product_reset_receipt_id.bytes() == self.reset().id().bytes()
                && expected.product_session_terminal_receipt_id.bytes()
                    == self.append().session_terminal_receipt_id().bytes()
                && expected.product_session_transcript_before.bytes()
                    == self.append().session_binding_id().bytes()
                && expected.product_release_preauthorization_id.bytes()
                    == self.release_link_preauthorization_id().bytes()
                && self.release_disposition() == FailureSessionReleaseDispositionV3::Resolved,
            ClutchError::MismatchedState,
        )
    }
}

/// Derive the sole resolved-session append/reset batch after Product,
/// Failure, Collateral, and Source terminal postwrites all exist. This is not
/// a generic terminal DTO: every fact comes from the hostile-reopened resolved
/// cell and its private resolution/Source receipts.
fn plan_resolved_failure_market_archive_v5(
    admission: AuthenticatedFailureMarketRootV3,
    interval: AuthenticatedFailureMarketIntervalAccountsV2,
    resolution: AuthenticatedFailureMarketResolutionPostwriteV5,
    source_terminal: AuthenticatedSourceResolutionTerminalV1,
) -> Outcome<FailureMarketResolvedArchivePlanV5> {
    let cell = interval.cell();
    let history = interval.history();
    let failure_resolution = resolution.failure_resolution();
    let source_resolution_postwrite_id = match source_terminal.policy() {
        crate::instructions::source_terminal_resolution_v5::PersistedSourceResolutionTerminalPolicyV1::NoReopen(
            value,
        ) => value
            .authenticated()
            .resolution_v5_terminal_postwrite_id(),
        crate::instructions::source_terminal_resolution_v5::PersistedSourceResolutionTerminalPolicyV1::ReopenRequest(
            _,
        ) => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
    };
    let terminal = project_failure_market_interval_terminal_history_facts_v2(cell, history)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        resolution.cell_account() == interval.cell_account()
            && resolution.cell_authentication_after() == interval.cell_authentication_id()
            && resolution.cell_state_after()
                == ContentId::from_bytes(interval.cell_state_id().bytes())
            && cell.disposition() == FailureMarketIntervalCellDispositionV2::Resolved
            && terminal.disposition == FailureMarketIntervalTerminalDispositionV2::Resolved
            && terminal.session_terminal_receipt_id.bytes()
                == failure_resolution.id().bytes()
            && terminal.terminal_state_commitment.bytes()
                == interval.cell_state_id().bytes()
            && source_terminal.id() != ContentId::ZERO
            && source_resolution_postwrite_id == resolution.id(),
        ClutchError::MismatchedState,
    )?;
    let authority = FailureMarketResolvedArchiveAuthorityV5 { expected: terminal };
    let (history_plan, append) = plan_append_failure_market_interval_history_v2(
        &authority,
        history,
        admission.state(),
        interval.quote(),
        terminal,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let (cell_plan, reset) = plan_reset_failure_market_interval_cell_v2(cell, append)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        append.session_terminal_receipt_id().bytes() == failure_resolution.id().bytes()
            && reset.append_receipt_id() == append.id()
            && reset.terminal_cell() == interval.cell_state_id(),
        ClutchError::MismatchedState,
    )?;
    Ok(FailureMarketResolvedArchivePlanV5 {
        history_plan,
        append,
        cell_plan,
        reset,
    })
}

/// Exact final-postwrite facts required before Source may terminalize.
///
/// This projection is only a testable equality boundary. The live adapter
/// reconstructs it from Product's private Source input, this module's private
/// postwrite, and the retained Failure receipt; callers cannot construct
/// authority from these fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceResolutionFinalJoinFactsV5 {
    postwrite_id: ContentId,
    final_cell_authentication_id: ContentId,
    final_cell_state_id: ContentId,
    failure_cell_after_id: ContentId,
    activation_failure_receipt_id: ContentId,
    failure_receipt_id: ContentId,
    activation_product_certificate_id: ContentId,
    failure_product_certificate_id: ContentId,
    activation_market_instance_id: [u8; 32],
    failure_market_instance_id: [u8; 32],
    source_market_instance_id: [u8; 32],
    activation_generation: u64,
    failure_generation: u64,
    source_failure_policy_binding_id: [u8; 32],
    failure_policy_binding_id: [u8; 32],
    source_successful_handoff_id: [u8; 32],
    failure_source_handoff_id: [u8; 32],
    resolution_account_id: ContentId,
    resolution_semantic_id: ContentId,
    resolution_data_id: ContentId,
    source_resolution_input_id: ContentId,
    runtime_postwrite_id: ContentId,
}

fn require_source_resolution_final_join_v5(facts: SourceResolutionFinalJoinFactsV5) -> Outcome<()> {
    require(
        !facts.postwrite_id.is_zero()
            && !facts.final_cell_authentication_id.is_zero()
            && facts.final_cell_state_id == facts.failure_cell_after_id
            && facts.activation_failure_receipt_id == facts.failure_receipt_id
            && facts.activation_product_certificate_id == facts.failure_product_certificate_id
            && facts.activation_market_instance_id == facts.failure_market_instance_id
            && facts.activation_market_instance_id == facts.source_market_instance_id
            && facts.activation_generation == facts.failure_generation
            && facts.source_failure_policy_binding_id == facts.failure_policy_binding_id
            && facts.source_successful_handoff_id == facts.failure_source_handoff_id
            && !facts.resolution_account_id.is_zero()
            && !facts.resolution_semantic_id.is_zero()
            && !facts.resolution_data_id.is_zero()
            && facts.resolution_account_id != facts.resolution_semantic_id
            && facts.resolution_account_id != facts.resolution_data_id
            && facts.resolution_semantic_id != facts.resolution_data_id
            && !facts.source_resolution_input_id.is_zero()
            && !facts.runtime_postwrite_id.is_zero(),
        ClutchError::MismatchedState,
    )
}

/// Private same-call proof consumed by the reusable Failure cell writer.
///
/// Construction is possible only after the Resolution, Hoard, ClaimLedger,
/// and Product root postimages have all been hostile-reauthenticated. It
/// retains the complete private Failure receipt so the final cell write cannot
/// substitute another payout, certificate, session, or disposition.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AuthenticatedFailureMarketResolutionActivationV5 {
    id: ContentId,
    failure_resolution: FailureMarketIntervalCellResolutionReceiptV2,
    product_activation: MarketResolutionActivationV3,
    collateral_postwrite: AuthenticatedMarketResolutionActivationPostwriteV5,
    market_root: Pubkey,
    market_root_authentication_before: ContentId,
    market_root_authentication_after: ContentId,
    series_link: Pubkey,
    series_link_authentication: ContentId,
    series_link_preauthorization_id: ContentId,
    inactive_resolution_authentication_id: ContentId,
    resolution_physical_postwrite_id: ContentId,
    finalization_evidence_id: ContentId,
}

impl AuthenticatedFailureMarketResolutionActivationV5 {
    /// Complete same-call authorization identity.
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    /// Product's exact once-only `0xaa` activation postimage.
    pub(crate) const fn product_activation(self) -> MarketResolutionActivationV3 {
        self.product_activation
    }

    /// Collateral's exact Resolution/Hoard/ClaimLedger postwrite proof.
    pub(crate) const fn collateral_postwrite(
        self,
    ) -> AuthenticatedMarketResolutionActivationPostwriteV5 {
        self.collateral_postwrite
    }

    /// Exact hostile inactive prestate consumed by the physical writer.
    pub(crate) const fn inactive_resolution_authentication_id(self) -> ContentId {
        self.inactive_resolution_authentication_id
    }

    /// Exact inactive-to-finalized Resolution/Hoard/ClaimLedger postwrite.
    pub(crate) const fn resolution_physical_postwrite_id(self) -> ContentId {
        self.resolution_physical_postwrite_id
    }

    /// Exact evidence identity embedded in Resolution V5 and Product `0xaa`.
    pub(crate) const fn finalization_evidence_id(self) -> ContentId {
        self.finalization_evidence_id
    }

    /// Exact shared root and its authenticated pre/post identities.
    pub(crate) const fn market_root(self) -> Pubkey {
        self.market_root
    }

    pub(crate) const fn market_root_authentication_before(self) -> ContentId {
        self.market_root_authentication_before
    }

    pub(crate) const fn market_root_authentication_after(self) -> ContentId {
        self.market_root_authentication_after
    }

    /// Exact read-only initiating link and its live pin authentication.
    pub(crate) const fn series_link(self) -> Pubkey {
        self.series_link
    }

    pub(crate) const fn series_link_authentication(self) -> ContentId {
        self.series_link_authentication
    }

    pub(crate) const fn series_link_preauthorization_id(self) -> ContentId {
        self.series_link_preauthorization_id
    }

    /// Exact private Failure resolution consumed by Product and the cell.
    pub(crate) const fn failure_resolution(self) -> FailureMarketIntervalCellResolutionReceiptV2 {
        self.failure_resolution
    }
}

/// Exact pure shared-runtime resolution authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketRuntimeResolutionAuthorityV1 {
    runtime_before:
        clutch_failure_policy_runtime::market_runtime_v1::FailureMarketRuntimeStateCommitmentV1,
    series_link_state_id: clutch_product_series::SeriesMarketLinkV3Id,
    session_before: ContentId,
    session_after: ContentId,
    resolution_receipt_id: ContentId,
}

impl AuthenticatedFailureMarketSessionV3 for FailureMarketRuntimeResolutionAuthorityV1 {
    fn authenticate_failure_market_session_resolution_v3(
        &self,
        expected: FailureMarketSessionResolutionFactsV3,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if expected.runtime_before != self.runtime_before
            || expected.series_link_state_id != self.series_link_state_id
            || expected.session_before != self.session_before
            || expected.session_after != self.session_after
            || expected.session_resolution_receipt_id != self.resolution_receipt_id
            || expected.transition_receipt_id.bytes() == [0; 32]
        {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketResolvedRuntimeArchiveAuthorityV5 {
    expected: FailureMarketSessionCloseFactsV3,
}

impl AuthenticatedFailureMarketSessionV3 for FailureMarketResolvedRuntimeArchiveAuthorityV5 {
    fn authenticate_failure_market_session_close_v3(
        &self,
        mut expected: FailureMarketSessionCloseFactsV3,
    ) -> clutch_failure_policy_runtime::Result<()> {
        let receipt = expected.transition_receipt_id;
        expected.transition_receipt_id =
            FailureMarketSessionTransitionReceiptIdV3::from_bytes([0; 32]);
        let mut retained = self.expected;
        retained.transition_receipt_id =
            FailureMarketSessionTransitionReceiptIdV3::from_bytes([0; 32]);
        if receipt.bytes() == [0; 32] || expected != retained {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketResolvedRuntimeArchiveWriteV5 {
    expected: FailureMarketRuntimeSessionWriteFactsV3,
    idle_cell: ContentId,
    idle_runtime: ContentId,
    source_product_release: ContentId,
    runtime_source_product_release: ContentId,
}

impl AuthenticatedFailureMarketRuntimeSessionWriteV3
    for FailureMarketResolvedRuntimeArchiveWriteV5
{
    fn authenticate_failure_market_runtime_session_write_v3(
        &self,
        expected: FailureMarketRuntimeSessionWriteFactsV3,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if expected != self.expected
            || self.idle_cell != self.idle_runtime
            || self.source_product_release != self.runtime_source_product_release
        {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

/// Private restoration owner minted only from the hostile-authenticated
/// complete `0xab/v2` prestate and its exact Source successful handoff.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketIntervalResolutionHistoryAuthorityV5 {
    restoration: QuantizedIntervalConsensusRestorationV1,
    cell_before:
        clutch_failure_policy_runtime::market_interval_cell_v2::FailureMarketIntervalCellStateIdV2,
    market_instance_id: clutch_product_series::MarketInstanceV2Id,
    generation: u64,
    session_binding_id: clutch_source_plane_v3_runtime::ContentId,
    source_handoff_id: clutch_source_plane_v3_runtime::ContentId,
    completed_work_calls: u64,
    exact_reward_lamports: u64,
}

impl AuthenticatedQuantizedIntervalConsensusHistoryV1
    for FailureMarketIntervalResolutionHistoryAuthorityV5
{
    fn authenticate_complete_history(
        &self,
        expected: QuantizedIntervalConsensusRestorationV1,
    ) -> clutch_product_series::Result<()> {
        if expected != self.restoration {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
    }
}

impl AuthenticatedFailureMarketIntervalCellResolutionV2
    for FailureMarketIntervalResolutionHistoryAuthorityV5
{
    fn authenticate_failure_market_interval_cell_resolution(
        &self,
        expected: FailureMarketIntervalCellResolutionFactsV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if expected.cell_before != self.cell_before
            || expected.cell_after == self.cell_before
            || expected.cell_after.bytes() == [0; 32]
            || expected.market_instance_id != self.market_instance_id
            || expected.generation != self.generation
            || expected.session_binding_id != self.session_binding_id
            || expected.source_handoff_id != self.source_handoff_id
            || expected.terminal_work_id != self.restoration.work_id
            || expected.product_certificate_id != self.restoration.certificate_id
            || expected.completed_work_calls != self.completed_work_calls
            || expected.exact_reward_lamports != self.exact_reward_lamports
        {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

fn plan_authenticated_failure_market_resolution_v5(
    admission: AuthenticatedFailureMarketRootV3,
    interval: AuthenticatedFailureMarketIntervalAccountsV2,
    source_success: SuccessfulEvaluationHandoffV1,
    context: QuantizedIntervalConsensusContextV1<'_>,
) -> Outcome<FailureMarketIntervalCellResolutionPlanV2> {
    let cell = interval.cell();
    let work = cell
        .product_work()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
    let certificate = QuantizedIntervalConsensusCertificateV1::from_complete_work(work)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let restoration = QuantizedIntervalConsensusRestorationV1 {
        work_id: work
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        certificate_id: certificate
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        market_instance_id: work.market_instance_id(),
        source_interval_id: work.source_interval_id(),
        interval_profile_id: work.interval_profile_id(),
        checked_coordinates: work.checked_coordinates(),
        transcript: work.transcript(),
    };
    let authority = FailureMarketIntervalResolutionHistoryAuthorityV5 {
        restoration,
        cell_before: interval.cell_state_id(),
        market_instance_id: cell.market_instance_id(),
        generation: cell.generation(),
        session_binding_id: cell.session_binding_id(),
        source_handoff_id: cell.source_handoff_id(),
        completed_work_calls: cell.completed_work_calls(),
        exact_reward_lamports: cell.exact_reward_lamports(),
    };
    plan_resolve_failure_market_interval_cell_v2(
        &authority,
        cell,
        admission.state(),
        interval.funding(),
        interval.history(),
        interval.quote(),
        source_success,
        context,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

/// Runtime write authority minted only after Product, collateral, and cell writes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketRuntimeResolutionWriteV1 {
    expected: FailureMarketRuntimeSessionWriteFactsV3,
    cell_state_after: ContentId,
    runtime_session_state_after: ContentId,
    activation_failure_receipt_id: ContentId,
    runtime_resolution_receipt_id: ContentId,
}

impl AuthenticatedFailureMarketRuntimeSessionWriteV3 for FailureMarketRuntimeResolutionWriteV1 {
    fn authenticate_failure_market_runtime_session_write_v3(
        &self,
        expected: FailureMarketRuntimeSessionWriteFactsV3,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if expected != self.expected
            || self.cell_state_after != self.runtime_session_state_after
            || self.activation_failure_receipt_id != self.runtime_resolution_receipt_id
        {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

/// Final post-cell/post-runtime Resolution V5 capability for Source terminalization.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AuthenticatedFailureMarketResolutionPostwriteV5 {
    id: ContentId,
    activation: AuthenticatedFailureMarketResolutionActivationV5,
    cell_account: Pubkey,
    cell_authentication_after: ContentId,
    cell_state_after: ContentId,
    runtime_postwrite_id: ContentId,
}

impl AuthenticatedFailureMarketResolutionPostwriteV5 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn activation(self) -> AuthenticatedFailureMarketResolutionActivationV5 {
        self.activation
    }

    pub(crate) const fn failure_resolution(self) -> FailureMarketIntervalCellResolutionReceiptV2 {
        self.activation.failure_resolution
    }

    pub(crate) const fn product_activation(self) -> MarketResolutionActivationV3 {
        self.activation.product_activation
    }

    pub(crate) const fn resolution_semantic_id(self) -> ContentId {
        self.activation.product_activation.resolution_semantic_id()
    }

    pub(crate) const fn resolution_data_id(self) -> ContentId {
        self.activation.product_activation.resolution_data_id()
    }

    pub(crate) const fn resolution_account_id(self) -> ContentId {
        self.activation.product_activation.resolution_account_id()
    }

    pub(crate) const fn cell_account(self) -> Pubkey {
        self.cell_account
    }

    pub(crate) const fn cell_authentication_after(self) -> ContentId {
        self.cell_authentication_after
    }

    pub(crate) const fn cell_state_after(self) -> ContentId {
        self.cell_state_after
    }

    pub(crate) const fn runtime_postwrite_id(self) -> ContentId {
        self.runtime_postwrite_id
    }
}

/// Complete durable poststate of the sole successful Market Failure session.
///
/// This capability exists only after Product/Collateral resolution, Source
/// no-reopen terminalization, interval archive and link release, physical
/// StatisticResult close, Recovery-custody close, and Failure-family seal have
/// all succeeded in one SVM instruction. Product's later Retiring latch
/// hostile-reopens the persisted accounts instead of trusting this value.
#[derive(Debug)]
pub(crate) struct AuthenticatedResolvedFailureMarketLifecycleV5 {
    resolution: AuthenticatedFailureMarketResolutionPostwriteV5,
    source_route: AuthenticatedSourceRouteV1,
    source_schedule: SourceWorkScheduleBindingV1,
    source_input: AuthenticatedSourceResolutionInputV4,
    source_terminal: AuthenticatedSourceResolutionTerminalV1,
    archive: FailureMarketIntervalArchivePostwriteV3,
    link_release: AuthenticatedSeriesFailureSessionReleaseV4,
    source_product_release: AuthenticatedSourceResolutionProductReleaseV1,
    source_result_close: AuthenticatedSourceResolutionStatisticResultCloseV1,
    recovery_close: AuthenticatedFailureMarketRecoveryClosePostwriteV2,
    family_terminal: AuthenticatedFailureMarketFamilyTerminalPostwriteV2,
}

impl AuthenticatedResolvedFailureMarketLifecycleV5 {
    pub(crate) const fn resolution(self) -> AuthenticatedFailureMarketResolutionPostwriteV5 {
        self.resolution
    }

    pub(crate) const fn source_route(self) -> AuthenticatedSourceRouteV1 {
        self.source_route
    }

    pub(crate) const fn source_schedule(self) -> SourceWorkScheduleBindingV1 {
        self.source_schedule
    }

    pub(crate) const fn source_input(self) -> AuthenticatedSourceResolutionInputV4 {
        self.source_input
    }

    pub(crate) const fn source_terminal(self) -> AuthenticatedSourceResolutionTerminalV1 {
        self.source_terminal
    }

    pub(crate) const fn source_product_release(
        self,
    ) -> AuthenticatedSourceResolutionProductReleaseV1 {
        self.source_product_release
    }

    pub(crate) const fn source_result_close(
        self,
    ) -> AuthenticatedSourceResolutionStatisticResultCloseV1 {
        self.source_result_close
    }

    pub(crate) const fn family_terminal(
        &self,
    ) -> &AuthenticatedFailureMarketFamilyTerminalPostwriteV2 {
        &self.family_terminal
    }
}

impl AuthenticatedFailureMarketProductResolutionV2
    for AuthenticatedFailureMarketResolutionActivationV5
{
    fn authenticate_failure_market_product_resolution(
        &self,
        expected: FailureMarketIntervalCellResolutionReceiptV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        let expected_certificate = expected.verified_payout().certificate();
        let retained_certificate = self.failure_resolution.verified_payout().certificate();
        if expected.id() != self.failure_resolution.id()
            || expected.failure_policy_binding_id()
                != self.failure_resolution.failure_policy_binding_id()
            || expected.facts() != self.failure_resolution.facts()
            || expected_certificate != retained_certificate
            || expected_certificate
                .id()
                .map_err(|_| clutch_failure_policy_runtime::Error::BindingMismatch)?
                .bytes()
                != self.product_activation.product_certificate_id().bytes()
            || expected.id().bytes()
                != self
                    .product_activation
                    .failure_resolution_receipt_id()
                    .bytes()
        {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

impl AuthenticatedSourceResolutionV5TerminalV1 for AuthenticatedFailureMarketResolutionPostwriteV5 {
    fn authenticate_source_resolution_v5_terminal_v1(
        &self,
        route: AuthenticatedSourceRouteV1,
        source: AuthenticatedSourceResolutionInputV4,
        failure: FailureMarketIntervalCellResolutionReceiptV2,
        lineage: AuthenticatedReopenLineageV1,
    ) -> Outcome<AuthenticatedSourceResolutionTerminalPolicyV1> {
        let retained_failure = self.failure_resolution();
        AuthenticatedFailureMarketProductResolutionV2::authenticate_failure_market_product_resolution(
            &self.activation,
            failure,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let failure_facts = retained_failure.facts();
        let certificate_id = retained_failure
            .verified_payout()
            .certificate()
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let product_activation = self.product_activation();
        require_source_resolution_final_join_v5(SourceResolutionFinalJoinFactsV5 {
            postwrite_id: self.id,
            final_cell_authentication_id: self.cell_authentication_after,
            final_cell_state_id: self.cell_state_after,
            failure_cell_after_id: ContentId::from_bytes(failure_facts.cell_after.bytes()),
            activation_failure_receipt_id: product_activation.failure_resolution_receipt_id(),
            failure_receipt_id: ContentId::from_bytes(retained_failure.id().bytes()),
            activation_product_certificate_id: product_activation.product_certificate_id(),
            failure_product_certificate_id: certificate_id.content_id(),
            activation_market_instance_id: product_activation.market_instance_id().bytes(),
            failure_market_instance_id: failure_facts.market_instance_id.bytes(),
            source_market_instance_id: source.market_instance_id().bytes(),
            activation_generation: product_activation.generation(),
            failure_generation: failure_facts.generation,
            source_failure_policy_binding_id: source.failure_policy_binding_id().bytes(),
            failure_policy_binding_id: retained_failure.failure_policy_binding_id().bytes(),
            source_successful_handoff_id: source.successful_evaluation_handoff_id().bytes(),
            failure_source_handoff_id: failure_facts.source_handoff_id.bytes(),
            resolution_account_id: product_activation.resolution_account_id(),
            resolution_semantic_id: product_activation.resolution_semantic_id(),
            resolution_data_id: product_activation.resolution_data_id(),
            source_resolution_input_id: ContentId::from_bytes(source.id().bytes()),
            runtime_postwrite_id: self.runtime_postwrite_id,
        })?;
        AuthenticatedSourceResolutionTerminalPolicyV1::successful_resolution_no_reopen(
            clutch_source_plane_v3::ContentId::from_bytes(self.id.bytes()),
            route,
            source,
            retained_failure,
            lineage,
        )
    }
}

/// Private bridge constructed only after the exact collateral postwrite.
#[derive(Clone, Copy, Debug)]
struct AuthenticatedProductResolutionRootWriteV6<'inactive> {
    root_account: Pubkey,
    root_authentication_before: ContentId,
    root_semantic_before: ContentId,
    root_data_before: ContentId,
    link_account: Pubkey,
    link_authentication_id: ContentId,
    link_semantic_id: clutch_product_series::SeriesMarketLinkV3Id,
    funding_account: Pubkey,
    funding_authentication_id: ContentId,
    funding_data_id: ContentId,
    inactive: &'inactive AuthenticatedInactiveFailureResolutionV5,
    activation: MarketResolutionActivationV3,
    collateral_plan_receipt_id: ContentId,
    failure_resolution_id: ContentId,
    collateral_postwrite_id: ContentId,
    resolution_physical_postwrite_id: ContentId,
    finalization_evidence_id: ContentId,
}

impl AuthenticatedCurrentMarketResolutionWriteV3
    for AuthenticatedProductResolutionRootWriteV6<'_>
{
    fn authenticate_current_market_resolution_write_v3(
        &self,
        root_account: Pubkey,
        root_authentication_before: ContentId,
        root_semantic_before: ContentId,
        root_data_before: ContentId,
        link_account: Pubkey,
        link_authentication_id: ContentId,
        link_semantic_id: clutch_product_series::SeriesMarketLinkV3Id,
        funding_account: Pubkey,
        funding_authentication_id: ContentId,
        funding_data_id: ContentId,
        resolution_account: Pubkey,
        inactive_resolution_authentication_id: ContentId,
        resolution_physical_postwrite_id: ContentId,
        expected: MarketResolutionActivationV3,
    ) -> Outcome<()> {
        require(
            root_account == self.root_account
                && root_authentication_before == self.root_authentication_before
                && root_semantic_before == self.root_semantic_before
                && root_data_before == self.root_data_before
                && link_account == self.link_account
                && link_authentication_id == self.link_authentication_id
                && link_semantic_id == self.link_semantic_id
                && funding_account == self.funding_account
                && funding_authentication_id == self.funding_authentication_id
                && funding_data_id == self.funding_data_id
                && resolution_account == self.inactive.account()
                && inactive_resolution_authentication_id == self.inactive.authentication_id()
                && resolution_physical_postwrite_id == self.resolution_physical_postwrite_id
                && expected == self.activation
                && expected.failure_resolution_receipt_id() == self.failure_resolution_id
                && expected.composite_finalization_evidence_id() == self.finalization_evidence_id
                && self.collateral_plan_receipt_id != ContentId::ZERO
                && self.collateral_postwrite_id != ContentId::ZERO,
            ClutchError::MismatchedState,
        )
    }
}

/// Consume the exact Product-founded inactive Resolution V5, atomically write
/// the three collateral postimages, advance Product's active root once, and mint
/// the sole private authority accepted by the Failure resolved-cell writer.
///
/// `root_before`, `link_before`, `funding`, and `inactive` are private receipts
/// constructed by their physical semantic owners. No quote, funding, graph, or
/// placeholder bytes are accepted from the instruction payload.
#[allow(clippy::too_many_arguments)]
fn activate_failure_market_resolution_v5<'a>(
    program_id: &Pubkey,
    market_root_account: &AccountInfo<'a>,
    series_link_account: &AccountInfo<'a>,
    funding: &AuthenticatedSeriesFundingAccountV5,
    resolution_account: &AccountInfo<'a>,
    hoard_account: &AccountInfo<'a>,
    claim_ledger_account: &AccountInfo<'a>,
    root_before: AuthenticatedMarketLifecycleRootV3<'_>,
    link_before: &AuthenticatedSeriesMarketLinkV3<'_>,
    link_release: &AuthenticatedWritableFailureSessionReleaseLinkV4,
    registry: &AuthenticatedRegistryCapabilityV5,
    bundle: &AuthenticatedCompiledProductSeriesBundleV7,
    inactive: &AuthenticatedInactiveFailureResolutionV5,
    liabilities: GeneralMarketLiabilityAuthorityV5,
    failure_resolution: FailureMarketIntervalCellResolutionReceiptV2,
    root_decode_after: &mut MarketLifecycleRootAccountV3,
) -> Outcome<AuthenticatedFailureMarketResolutionActivationV5> {
    require_distinct(&[
        market_root_account.clone(),
        series_link_account.clone(),
        resolution_account.clone(),
        hoard_account.clone(),
        claim_ledger_account.clone(),
    ])?;
    let root = root_before.state();
    let root_transition_sequence_before = root.transition_sequence();
    let root_binding = *root_before.binding();
    let link = link_before.state();
    let link_binding = link_before.binding();
    let registry_projection = registry.projection();
    let failure_facts = failure_resolution.facts();
    let verified_payout = failure_resolution.verified_payout();
    let certificate = verified_payout.certificate();
    let payout = verified_payout.payout();
    certificate
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    payout
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let certificate_id = certificate
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_binding_id = root_before.binding_id();

    require_current_product_failure_join(
        market_root_account,
        &root_before,
        root,
        root_binding_id,
        link_before,
        link_release,
        link,
        funding,
        registry,
        bundle,
        failure_resolution,
        certificate,
        certificate_id,
    )?;
    require_resolution_authority_facts_v5(ResolutionAuthorityFactsV5 {
        root_phase: root.phase(),
        root_resolution_semantic_id: root.resolution_semantic_id(),
        root_resolution_data_id: root.resolution_data_id(),
        root_resolution_activation_receipt_id: root.resolution_activation_receipt_id(),
        link_phase: link.phase(),
        active_failure_sessions: link.active_failure_sessions(),
        link_session_binding_id: link.failure_session_transcript_id().bytes(),
        failure_session_binding_id: failure_facts.session_binding_id.bytes(),
        failure_market_instance_id: failure_facts.market_instance_id.bytes(),
        root_market_instance_id: root_binding.market_instance_id.bytes(),
        failure_generation: failure_facts.generation,
        root_generation: root_binding.generation,
        failure_policy_binding_id: failure_resolution.failure_policy_binding_id().bytes(),
        root_failure_policy_binding_id: root_binding.market_failure_policy_binding_id.bytes(),
        failure_product_certificate_id: failure_facts.product_certificate_id.bytes(),
        product_certificate_id: certificate_id.bytes(),
        certificate_source_occurrence_id: certificate.source_occurrence_id().bytes(),
        link_source_occurrence_id: link_binding.source_occurrence_id.bytes(),
        payout_active_len: payout.active_len,
        root_outcome_count: root_binding.outcome_count,
        registry_neutral_lamport_sink: registry_projection
            .realm_collateral
            .neutral_lamport_sink
            .bytes(),
        root_neutral_lamport_sink: root.capital().neutral_lamport_sink.bytes(),
    })?;
    require(
        certificate.product_template_id().bytes() == root_binding.product_template_id.bytes()
            && certificate.market_genesis_profile_id().bytes()
                == root_binding.market_genesis_profile_id.bytes()
            && certificate.native_claim_basis_id().bytes()
                == root_binding.native_claim_basis_id.bytes()
            && certificate.price_measure_policy_id().bytes()
                == root_binding.price_measure_policy_id.bytes()
            && certificate.capability_profile_id() == root_binding.capability_profile_id
            && certificate.interval_profile_id().bytes()
                == root_binding.interval_consensus_profile_id.bytes(),
        ClutchError::MismatchedState,
    )?;

    require_exact_collateral_prestate(
        program_id,
        hoard_account,
        claim_ledger_account,
        liabilities,
        root_binding,
        registry_projection,
    )?;
    let inactive_resolution = inactive.resolution();
    require(
        inactive.account() == *resolution_account.key
            && inactive_resolution.facts.market_instance_id.bytes()
                == root_binding.market_instance_id.bytes()
            && inactive_resolution.facts.native_claim_basis_id.bytes()
                == root_binding.native_claim_basis_id.bytes()
            && inactive_resolution.facts.outcome_count == root_binding.outcome_count
            && inactive_resolution.facts.generation == root_binding.generation
            && inactive_resolution.rent.payer.bytes()
                == root.capital().rent_refund_owner.bytes(),
        ClutchError::MismatchedState,
    )?;

    let expected_hoard_after_id = HoardV2 {
        lifecycle: MarketLiabilityLifecycleV1::Resolved,
        ..liabilities.hoard
    }
    .semantic_id(&RuntimeSha256)
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let expected_claim_ledger_after_id = ClaimLedgerV3 {
        resolution_account: CollateralId::from_bytes(resolution_account.key.to_bytes()),
        lifecycle: MarketLiabilityLifecycleV1::Resolved,
        ..liabilities.claim_ledger
    }
    .semantic_id(&RuntimeSha256)
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let finalization_evidence_id = derive_finalization_evidence_id_v5(
        &root_before,
        link_before,
        link_release,
        funding,
        registry,
        bundle,
        inactive,
        liabilities,
        expected_hoard_after_id,
        expected_claim_ledger_after_id,
        failure_resolution,
        root_binding_id,
        certificate_id.content_id(),
        resolution_account.key,
        payout.active_len,
        payout.denominator,
        &payout.weights,
    )?;
    let (expected_resolution, resolution_bump) =
        seeds::resolution_v5_pda(program_id, &root_binding.market_instance_id.bytes());
    let resolution = ResolutionV5::finalized(
        ResolutionFinalizationFactsV5 {
            market_instance_id: CollateralId::from_bytes(root_binding.market_instance_id.bytes()),
            native_claim_basis_id: CollateralId::from_bytes(
                root_binding.native_claim_basis_id.bytes(),
            ),
            finalization_evidence_id: CollateralId::from_bytes(finalization_evidence_id.bytes()),
            outcome_count: payout.active_len,
            payout_denominator: payout.denominator,
            payout_weights: payout.weights,
            generation: root_binding.generation,
            payout_unit_boundary: ResolutionPayoutUnitBoundaryV5::ExactWholeCollateralAtoms,
        },
        resolution_bump,
        inactive_resolution.rent,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        expected_resolution == *resolution_account.key
            && resolution_bump == inactive_resolution.stored_bump,
        ClutchError::MismatchedState,
    )?;
    let activation_plan = prepare_market_resolution_activation_v5(
        CollateralId::from_bytes(resolution_account.key.to_bytes()),
        resolution,
        liabilities.hoard,
        liabilities.claim_ledger,
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        activation_plan.hoard_after_id() == expected_hoard_after_id
            && activation_plan.claim_ledger_after_id() == expected_claim_ledger_after_id,
        ClutchError::MismatchedState,
    )?;

    let product_activation = MarketResolutionActivationV3::new(
        root_binding,
        ContentId::from_bytes(activation_plan.resolution_id().bytes()),
        ContentId::from_bytes(activation_plan.resolution_data_id().bytes()),
        ContentId::from_bytes(failure_resolution.id().bytes()),
        certificate_id.content_id(),
        finalization_evidence_id,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    write_collateral_activation_postimages_v6(
        program_id,
        resolution_account,
        hoard_account,
        claim_ledger_account,
        inactive,
        resolution,
        activation_plan.hoard_after(),
        activation_plan.claim_ledger_after(),
    )?;
    let collateral_postwrite = authenticate_current_market_resolution_activation_postwrite_v5(
        program_id,
        liabilities,
        activation_plan,
        resolution_account,
        hoard_account,
        claim_ledger_account,
    )?;
    require(
        collateral_postwrite.plan() == activation_plan
            && collateral_postwrite.liability_authority_receipt_id() == liabilities.receipt_id,
        ClutchError::MismatchedState,
    )?;
    let resolution_physical_postwrite_id = project_failure_resolution_physical_postwrite_v6(
        &RuntimeSha256,
        program_id.to_bytes(),
        resolution_account.key.to_bytes(),
        inactive.authentication_id(),
        inactive.semantic_id(),
        inactive.data_id(),
        ContentId::from_bytes(activation_plan.resolution_id().bytes()),
        ContentId::from_bytes(activation_plan.resolution_data_id().bytes()),
        ContentId::from_bytes(collateral_postwrite.receipt_id().bytes()),
        inactive.observed_lamports(),
    );
    require_live_content_id(resolution_physical_postwrite_id)?;
    let root_write_authority = AuthenticatedProductResolutionRootWriteV6 {
        root_account: root_before.account(),
        root_authentication_before: root_before.authentication_id(),
        root_data_before: root_before.data_id(),
        root_semantic_before: root_before.semantic_id(),
        link_account: link_before.account(),
        link_authentication_id: link_before.authentication_id(),
        link_semantic_id: link_before.semantic_id(),
        funding_account: funding.account(),
        funding_authentication_id: funding.authentication_id(),
        funding_data_id: funding.data_id(),
        inactive,
        activation: product_activation,
        collateral_plan_receipt_id: ContentId::from_bytes(activation_plan.receipt_id().bytes()),
        failure_resolution_id: ContentId::from_bytes(failure_resolution.id().bytes()),
        collateral_postwrite_id: ContentId::from_bytes(collateral_postwrite.receipt_id().bytes()),
        resolution_physical_postwrite_id,
        finalization_evidence_id,
    };
    let product_postwrite = record_current_market_resolution_activation_v3(
        program_id,
        market_root_account,
        root_before,
        link_before,
        funding,
        resolution_account,
        inactive.authentication_id(),
        resolution_physical_postwrite_id,
        product_activation,
        &root_write_authority,
        root_decode_after,
    )?;
    require(
        product_postwrite.root_after().state().resolution_activation_receipt_id()
                == product_activation.id()
            && product_postwrite.root_after().state().resolution_semantic_id()
                == product_activation.resolution_semantic_id()
            && product_postwrite.root_after().state().resolution_data_id()
                == product_activation.resolution_data_id()
            && product_postwrite.root_after().state().transition_sequence()
                == root_transition_sequence_before
                    .checked_add(1)
                    .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?,
        ClutchError::MismatchedState,
    )?;
    require(
        product_postwrite.activation() == product_activation
            && product_postwrite.inactive_resolution_authentication_id()
                == inactive.authentication_id()
            && product_postwrite.resolution_physical_postwrite_id()
                == resolution_physical_postwrite_id
            && product_postwrite.root_authentication_before()
                == root_write_authority.root_authentication_before,
        ClutchError::MismatchedState,
    )?;
    let root_authentication_after = product_postwrite.root_after().authentication_id();

    let id = project_failure_resolution_activation_v5(
        &RuntimeSha256,
        market_root_account.key.to_bytes(),
        root_write_authority.root_authentication_before,
        root_authentication_after,
        series_link_account.key.to_bytes(),
        link_before.authentication_id(),
        link_release.id(),
        resolution_account.key.to_bytes(),
        inactive.authentication_id(),
        resolution_physical_postwrite_id,
        ContentId::from_bytes(failure_resolution.id().bytes()),
        certificate_id,
        finalization_evidence_id,
        product_activation.id(),
        ContentId::from_bytes(activation_plan.receipt_id().bytes()),
        ContentId::from_bytes(collateral_postwrite.receipt_id().bytes()),
        inactive.resolution().rent.refundable_principal,
        inactive.resolution().rent.donation_floor,
    );
    require_live_content_id(id)?;
    Ok(AuthenticatedFailureMarketResolutionActivationV5 {
        id,
        failure_resolution,
        product_activation,
        collateral_postwrite,
        market_root: *market_root_account.key,
        market_root_authentication_before: root_write_authority.root_authentication_before,
        market_root_authentication_after: root_authentication_after,
        series_link: *series_link_account.key,
        series_link_authentication: link_before.authentication_id(),
        series_link_preauthorization_id: link_release.id(),
        inactive_resolution_authentication_id: inactive.authentication_id(),
        resolution_physical_postwrite_id,
        finalization_evidence_id,
    })
}

/// Execute the only complete Market interval-resolution write batch.
///
/// This is the module's sole crate-visible mutation entry point. Product root,
/// Resolution V5, Hoard, ClaimLedger, and Failure cell writes either all
/// succeed in the same SVM instruction or all roll back. The narrower
/// composer above is private so no sibling module can persist Product's
/// once-only activation and omit the exact Resolved `0xab/v2` postimage.
#[allow(clippy::too_many_arguments)]
fn resolve_failure_market_interval_v5<'a>(
    program_id: &Pubkey,
    admission_root_account: &AccountInfo<'a>,
    runtime_root_account: &AccountInfo<'a>,
    market_root_account: &AccountInfo<'a>,
    series_link_account: &AccountInfo<'a>,
    interval_cell_account: &AccountInfo<'a>,
    interval_history_account: &AccountInfo<'a>,
    resolution_account: &AccountInfo<'a>,
    hoard_account: &AccountInfo<'a>,
    claim_ledger_account: &AccountInfo<'a>,
    funding: &AuthenticatedSeriesFundingAccountV5,
    root_before: AuthenticatedMarketLifecycleRootV3<'_>,
    link_before: &AuthenticatedSeriesMarketLinkV3<'_>,
    link_release: &AuthenticatedWritableFailureSessionReleaseLinkV4,
    admission: AuthenticatedFailureMarketRootV3,
    runtime_before: AuthenticatedFailureMarketRuntimeRootV1,
    interval_before: AuthenticatedFailureMarketIntervalAccountsV2,
    registry: &AuthenticatedRegistryCapabilityV5,
    bundle: &AuthenticatedCompiledProductSeriesBundleV7,
    inactive: &AuthenticatedInactiveFailureResolutionV5,
    liabilities: GeneralMarketLiabilityAuthorityV5,
    resolution: FailureMarketIntervalCellResolutionPlanV2,
    root_decode_after: &mut MarketLifecycleRootAccountV3,
) -> Outcome<(
    AuthenticatedFailureMarketResolutionPostwriteV5,
    AuthenticatedFailureMarketIntervalAccountsV2,
    AuthenticatedFailureMarketRuntimeSessionPostwriteV3,
)> {
    require_distinct_resolution_role_keys_v5([
        *admission_root_account.key,
        *runtime_root_account.key,
        *market_root_account.key,
        *series_link_account.key,
        *interval_cell_account.key,
        *interval_history_account.key,
        *resolution_account.key,
        *hoard_account.key,
        *claim_ledger_account.key,
        funding.account(),
    ])?;
    let failure_resolution = resolution.receipt();
    let failure_facts = failure_resolution.facts();
    let session_after = ContentId::from_bytes(failure_facts.cell_after.bytes());
    let runtime_resolution_receipt_id = ContentId::from_bytes(failure_resolution.id().bytes());
    let link_state_id = link_before.semantic_id();
    let runtime_authority = FailureMarketRuntimeResolutionAuthorityV1 {
        runtime_before: runtime_before.state_commitment(),
        series_link_state_id: link_state_id,
        session_before: ContentId::from_bytes(failure_facts.cell_before.bytes()),
        session_after,
        resolution_receipt_id: runtime_resolution_receipt_id,
    };
    let runtime_plan = plan_resolve_failure_market_session_v3(
        &runtime_authority,
        runtime_before.state(),
        admission.state(),
        *link_before.state(),
        session_after,
        runtime_resolution_receipt_id,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        runtime_plan.series_link_before() == *link_before.state()
            && runtime_plan.series_link_after() == *link_before.state()
            && runtime_plan.resulting_runtime().session_state_commitment() == session_after
            && runtime_plan
                .resulting_runtime()
                .session_resolution_receipt_id()
                == runtime_resolution_receipt_id,
        ClutchError::MismatchedState,
    )?;
    let activation = activate_failure_market_resolution_v5(
        program_id,
        market_root_account,
        series_link_account,
        funding,
        resolution_account,
        hoard_account,
        claim_ledger_account,
        root_before,
        link_before,
        link_release,
        registry,
        bundle,
        inactive,
        liabilities,
        failure_resolution,
        root_decode_after,
    )?;
    let interval_after = write_failure_market_interval_resolution_plan_v2(
        program_id,
        interval_cell_account,
        interval_history_account,
        interval_before,
        resolution,
        &activation,
    )?;
    let runtime_write_facts = FailureMarketRuntimeSessionWriteFactsV3 {
        runtime_before: runtime_before.state_commitment(),
        runtime_after: runtime_plan
            .resulting_runtime()
            .commitment()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        transition_receipt_id: runtime_plan.receipt_id(),
    };
    let runtime_write_authority = FailureMarketRuntimeResolutionWriteV1 {
        expected: runtime_write_facts,
        cell_state_after: ContentId::from_bytes(interval_after.cell_state_id().bytes()),
        runtime_session_state_after: runtime_plan.resulting_runtime().session_state_commitment(),
        activation_failure_receipt_id: ContentId::from_bytes(
            activation.failure_resolution().id().bytes(),
        ),
        runtime_resolution_receipt_id,
    };
    let runtime_postwrite = write_failure_market_runtime_session_plan_v3(
        program_id,
        admission_root_account,
        runtime_root_account,
        admission,
        runtime_before,
        runtime_plan,
        &runtime_write_authority,
    )?;
    require(
        runtime_postwrite.transition_receipt_id() == runtime_write_facts.transition_receipt_id
            && runtime_postwrite.root().state().session_state_commitment() == session_after
            && runtime_postwrite
                .root()
                .state()
                .session_resolution_receipt_id()
                == runtime_resolution_receipt_id,
        ClutchError::MismatchedState,
    )?;
    let projection = project_failure_action12_resolution_v5(
        &RuntimeSha256,
        admission_root_account.key.to_bytes(),
        runtime_root_account.key.to_bytes(),
        market_root_account.key.to_bytes(),
        series_link_account.key.to_bytes(),
        interval_cell_account.key.to_bytes(),
        activation.id(),
        activation.market_root_authentication_after(),
        interval_after.cell_authentication_id(),
        ContentId::from_bytes(interval_after.cell_state_id().bytes()),
        runtime_postwrite.id(),
        ContentId::from_bytes(runtime_postwrite.transition_receipt_id().bytes()),
        activation.product_activation().resolution_semantic_id(),
        activation.product_activation().resolution_data_id(),
        activation.product_activation().resolution_account_id(),
        ContentId::from_bytes(failure_resolution.id().bytes()),
    );
    let postwrite_id = projection.postwrite_id();
    require_live_content_id(postwrite_id)?;
    let postwrite = AuthenticatedFailureMarketResolutionPostwriteV5 {
        id: postwrite_id,
        activation,
        cell_account: interval_after.cell_account(),
        cell_authentication_after: interval_after.cell_authentication_id(),
        cell_state_after: ContentId::from_bytes(interval_after.cell_state_id().bytes()),
        runtime_postwrite_id: runtime_postwrite.id(),
    };
    Ok((postwrite, interval_after, runtime_postwrite))
}

/// Resolve one shared Market and atomically finish its entire Failure family.
///
/// The Product-owned writable-link preauthorization is minted before any
/// mutation. A late Source, archive, Product-link release, StatisticResult
/// close, Recovery close, or family-seal refusal therefore rolls every prior
/// write back. No caller-selected terminal, reopen, or partial writer exists.
#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_failure_market_interval_and_source_v5<'a>(
    program_id: &Pubkey,
    admission_root_account: &AccountInfo<'a>,
    runtime_root_account: &AccountInfo<'a>,
    market_root_account: &AccountInfo<'a>,
    series_link_account: &AccountInfo<'a>,
    interval_cell_account: &AccountInfo<'a>,
    interval_history_account: &AccountInfo<'a>,
    resolution_account: &AccountInfo<'a>,
    hoard_account: &AccountInfo<'a>,
    claim_ledger_account: &AccountInfo<'a>,
    replay_account: &AccountInfo<'a>,
    source_result_account: &AccountInfo<'a>,
    source_lineage_account: &AccountInfo<'a>,
    source_terminal_policy_account: &AccountInfo<'a>,
    source_terminal_receipt_account: &AccountInfo<'a>,
    source_liveness_policy_account: &AccountInfo<'a>,
    source_liveness_compartment_account: &AccountInfo<'a>,
    source_payer_refund: &AccountInfo<'a>,
    source_neutral_sink: &AccountInfo<'a>,
    source_account_payer: &AccountInfo<'a>,
    recovery_liveness_policy_account: &AccountInfo<'a>,
    recovery_account: &AccountInfo<'a>,
    recovery_refund_owner: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    root_before: AuthenticatedMarketLifecycleRootV3<'_>,
    link_before: AuthenticatedSeriesMarketLinkV3<'_>,
    funding: &AuthenticatedSeriesFundingAccountV5,
    admission: AuthenticatedFailureMarketRootV3,
    runtime_before: AuthenticatedFailureMarketRuntimeRootV1,
    interval_before: AuthenticatedFailureMarketIntervalAccountsV2,
    replay_before: AuthenticatedFailureMarketReplayV2,
    registry: &AuthenticatedRegistryCapabilityV5,
    bundle: &AuthenticatedCompiledProductSeriesBundleV7,
    inactive: &AuthenticatedInactiveFailureResolutionV5,
    liabilities: GeneralMarketLiabilityAuthorityV5,
    source_route: AuthenticatedSourceRouteV1,
    source_schedule: SourceWorkScheduleBindingV1,
    source_success: SuccessfulEvaluationHandoffV1,
    source_input: AuthenticatedSourceResolutionInputV4,
    source_lineage: AuthenticatedReopenLineageV1,
    context: QuantizedIntervalConsensusContextV1<'_>,
    root_decode_before: &mut MarketLifecycleRootAccountV3,
    link_decode_before: &mut SeriesMarketLinkAccountV3,
    root_decode_after: &mut MarketLifecycleRootAccountV3,
    link_release_decode: &mut SeriesMarketLinkAccountV3,
    link_rebound_output: &mut SeriesMarketLinkAccountV3,
    resolved_root_auth_decode: &mut MarketLifecycleRootAccountV3,
    resolved_root_persist_decode: &mut MarketLifecycleRootAccountV3,
) -> Outcome<AuthenticatedResolvedFailureMarketLifecycleV5> {
    require_source_resolution_outer_aliases_v5(
        [
            *admission_root_account.key,
            *runtime_root_account.key,
            *market_root_account.key,
            *series_link_account.key,
            *interval_cell_account.key,
            *interval_history_account.key,
            *resolution_account.key,
            *hoard_account.key,
            *claim_ledger_account.key,
            *replay_account.key,
            *source_terminal_policy_account.key,
            *source_terminal_receipt_account.key,
            *source_liveness_policy_account.key,
            *source_liveness_compartment_account.key,
            *source_lineage_account.key,
            *source_result_account.key,
            *recovery_liveness_policy_account.key,
            *recovery_account.key,
            *rent_sysvar.key,
            *system_program.key,
        ],
        [
            *source_payer_refund.key,
            *source_neutral_sink.key,
            *source_account_payer.key,
            *recovery_refund_owner.key,
        ],
    )?;
    require(
        Pubkey::new_from_array(source_lineage.lineage().lineage_account.bytes())
                == *source_lineage_account.key
            && Pubkey::new_from_array(source_input.result_account().bytes())
                == *source_result_account.key,
        ClutchError::MismatchedState,
    )?;
    let resolution_link = authenticate_writable_failure_resolution_link_v4(
        program_id,
        market_root_account,
        root_before,
        series_link_account,
        root_decode_before,
        link_decode_before,
    )?;
    let policy = admission.state().binding().facts();
    let live_root = authenticate_market_lifecycle_root_v3(
        program_id,
        market_root_account,
        policy.market_instance_id,
        policy.generation,
        true,
        root_decode_before,
    )?;
    require(
        live_root.authentication_id() == resolution_link.root_authentication_id()
            && live_root.semantic_id() == resolution_link.root_semantic_id()
            && live_root.binding_id() == resolution_link.root_binding_id(),
        ClutchError::MismatchedState,
    )?;
    let resolution = plan_authenticated_failure_market_resolution_v5(
        admission,
        interval_before,
        source_success,
        context,
    )?;
    let (postwrite, interval_after, runtime_after) = resolve_failure_market_interval_v5(
        program_id,
        admission_root_account,
        runtime_root_account,
        market_root_account,
        series_link_account,
        interval_cell_account,
        interval_history_account,
        resolution_account,
        hoard_account,
        claim_ledger_account,
        funding,
        live_root,
        &link_before,
        &resolution_link,
        admission,
        runtime_before,
        interval_before,
        registry,
        bundle,
        inactive,
        liabilities,
        resolution,
        root_decode_after,
    )?;
    let source_terminal = compose_source_resolution_terminal_v1(
        program_id,
        source_route,
        source_schedule,
        source_input,
        postwrite.failure_resolution(),
        &postwrite,
        source_lineage,
        source_terminal_policy_account,
        source_terminal_receipt_account,
        source_liveness_policy_account,
        source_liveness_compartment_account,
        source_payer_refund,
        source_neutral_sink,
        source_account_payer,
        system_program,
        rent_sysvar,
    )?;
    let archive_plan = plan_resolved_failure_market_archive_v5(
        admission,
        interval_after,
        postwrite,
        source_terminal,
    )?;
    let link_binding = *link_before.binding();
    let link_for_release = authenticate_series_market_link_v3(
        program_id,
        series_link_account,
        link_binding.series_plan_id,
        link_binding.ordinal,
        link_binding.market_instance_id,
        link_binding.generation,
        *market_root_account.key,
        true,
        link_release_decode,
    )?;
    let link_for_release_state = *link_for_release.state();
    let link_for_release_id = link_for_release.semantic_id();
    let (archive, released_link, link_release) = archive_resolved_failure_market_interval_link_v4(
        program_id,
        interval_cell_account,
        interval_history_account,
        series_link_account,
        interval_after,
        link_for_release,
        resolution_link,
        archive_plan.history_plan,
        archive_plan.append,
        archive_plan.cell_plan,
        archive_plan.reset,
        link_rebound_output,
    )?;
    require(
        link_release.release_link_preauthorization_id()
            == postwrite.activation().series_link_preauthorization_id(),
        ClutchError::MismatchedState,
    )?;
    let source_product_release = bind_source_resolution_product_release_v1(
        source_input,
        source_terminal,
        postwrite,
        &link_release,
        &archive,
    )?;
    let released_link_id = released_link.semantic_id();
    let expected_runtime_close = FailureMarketSessionCloseFactsV3 {
        runtime_before: runtime_after.root().state_commitment(),
        series_link_before: link_for_release_id,
        series_link_after: released_link_id,
        session_before: ContentId::from_bytes(archive.append().terminal_state_commitment().bytes()),
        session_after: ContentId::from_bytes(archive.append().idle_state_commitment().bytes()),
        interval_terminal_receipt_id: archive.append().session_terminal_receipt_id(),
        previous_session_history: archive.append().previous_root(),
        resulting_session_history: archive.append().resulting_root(),
        history_append_receipt_id: archive.append().id(),
        history_before: archive.append().history_before(),
        history_after: archive.append().history_after(),
        completed_session_count: archive.append().completed_session_count(),
        source_product_release_binding_id: ContentId::from_bytes(source_product_release.id().bytes()),
        transition_receipt_id: FailureMarketSessionTransitionReceiptIdV3::from_bytes([0; 32]),
    };
    let archive_runtime_plan = plan_close_failure_market_session_v3(
        &FailureMarketResolvedRuntimeArchiveAuthorityV5 {
            expected: expected_runtime_close,
        },
        runtime_after.root().state(),
        admission.state(),
        link_for_release_state,
        archive.append(),
        ContentId::from_bytes(source_product_release.id().bytes()),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        archive_runtime_plan.series_link_after() == *released_link.state()
            && archive_runtime_plan
                .resulting_runtime()
                .source_product_release_binding_id()
                == ContentId::from_bytes(source_product_release.id().bytes()),
        ClutchError::MismatchedState,
    )?;
    let archive_runtime_facts = FailureMarketRuntimeSessionWriteFactsV3 {
        runtime_before: runtime_after.root().state_commitment(),
        runtime_after: archive_runtime_plan
            .resulting_runtime()
            .commitment()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        transition_receipt_id: archive_runtime_plan.receipt_id(),
    };
    let archive_runtime = write_failure_market_runtime_session_plan_v3(
        program_id,
        admission_root_account,
        runtime_root_account,
        admission,
        runtime_after.root(),
        archive_runtime_plan,
        &FailureMarketResolvedRuntimeArchiveWriteV5 {
            expected: archive_runtime_facts,
            idle_cell: ContentId::from_bytes(archive.accounts().cell_state_id().bytes()),
            idle_runtime: archive_runtime_plan.resulting_runtime().session_state_commitment(),
            source_product_release: ContentId::from_bytes(source_product_release.id().bytes()),
            runtime_source_product_release: archive_runtime_plan
                .resulting_runtime()
                .source_product_release_binding_id(),
        },
    )?;
    let source_result_close = close_successful_source_statistic_result_v1(
        program_id,
        source_route,
        source_input,
        source_terminal,
        source_result_account,
        source_lineage_account,
        source_payer_refund,
        source_neutral_sink,
    )?;
    let recovery_close = close_failure_market_recovery_v2(
        program_id,
        admission_root_account,
        runtime_root_account,
        interval_cell_account,
        interval_history_account,
        recovery_liveness_policy_account,
        recovery_account,
        recovery_refund_owner,
        source_neutral_sink,
        admission,
        archive,
        archive_runtime,
        postwrite,
        source_terminal,
        source_product_release,
        source_result_close,
    )?;
    let resolved_root = authenticate_market_lifecycle_root_v3(
        program_id,
        market_root_account,
        policy.market_instance_id,
        policy.generation,
        true,
        resolved_root_auth_decode,
    )?;
    require(
        resolved_root.authentication_id()
                == postwrite.activation().market_root_authentication_after()
            && resolved_root.state().resolution_activation_receipt_id()
                == postwrite.product_activation().id(),
        ClutchError::MismatchedState,
    )?;
    let family_terminal = persist_resolved_failure_market_family_v3(
        program_id,
        market_root_account,
        admission_root_account,
        runtime_root_account,
        interval_cell_account,
        interval_history_account,
        replay_account,
        &resolved_root,
        admission,
        recovery_close,
        replay_before,
        resolved_root_persist_decode,
    )?;
    Ok(AuthenticatedResolvedFailureMarketLifecycleV5 {
        resolution: postwrite,
        source_route,
        source_schedule,
        source_input,
        source_terminal,
        archive,
        link_release,
        source_product_release,
        source_result_close,
        recovery_close,
        family_terminal,
    })
}

#[allow(clippy::too_many_arguments)]
fn require_current_product_failure_join(
    market_root_account: &AccountInfo<'_>,
    root: &AuthenticatedMarketLifecycleRootV3<'_>,
    root_state: &clutch_product_series::MarketLifecycleRootV3,
    root_binding_id: ContentId,
    link: &AuthenticatedSeriesMarketLinkV3<'_>,
    release: &AuthenticatedWritableFailureSessionReleaseLinkV4,
    link_state: &clutch_product_series::SeriesMarketLinkV3,
    funding: &AuthenticatedSeriesFundingAccountV5,
    registry: &AuthenticatedRegistryCapabilityV5,
    bundle: &AuthenticatedCompiledProductSeriesBundleV7,
    failure_resolution: FailureMarketIntervalCellResolutionReceiptV2,
    certificate: clutch_product_series::QuantizedIntervalConsensusCertificateV1,
    certificate_id: clutch_product_series::QuantizedIntervalConsensusCertificateV1Id,
) -> Outcome<()> {
    let root_binding = root_state.binding_ref();
    let link_binding = link_state.binding_ref();
    let projection = registry.projection();
    let bundle_value = bundle.bundle();
    require_current_series_funding_graph_v6(
        registry.funding_terms_id(),
        bundle_value.funding_terms_id,
        link_binding.funding_terms_id,
        bundle_value.funding_quote_id,
        link_binding.funding_quote_id,
        bundle_value.attachment_plan_id,
        link_binding.attachment_plan_id,
        funding,
    )?;
    require(
        root.is_writable()
            && link.owner_program() == root.owner_program()
            && link.account() != *market_root_account.key
            && release.root_account() == root.account()
            && release.root_authentication_id() == root.authentication_id()
            && release.link_account() == link.account()
            && release.link_authentication_id() == link.authentication_id()
            && release.disposition() == FailureSessionReleaseDispositionV4::Resolved
            && root.account() == *market_root_account.key
            && root_binding
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == root_binding_id
            && link_binding.market_root_account_id.bytes() == market_root_account.key.to_bytes()
            && link_binding.market_binding_id == root_binding_id
            && link_binding.market_instance_id == root_binding.market_instance_id
            && link_binding.generation == root_binding.generation
            && registry.series_plan_id() == link_binding.series_plan_id
            && registry.compiler_bundle_id() == bundle.bundle_id()
            && registry.registry_release_id() == root_binding.registry_release_id
            && registry.capability_profile_id() == root_binding.capability_profile_id
            && projection.registry_release_id == root_binding.registry_release_id
            && projection.capability_profile_id == root_binding.capability_profile_id
            && link_binding.capability_profile_id == root_binding.capability_profile_id
            && bundle_value.registry_release_id == root_binding.registry_release_id
            && bundle_value.capability_profile_id.content_id()
                == root_binding.capability_profile_id
            && bundle_value.series_plan_id == link_binding.series_plan_id
            && bundle_value.product_template_id.content_id() == root_binding.product_template_id
            && bundle_value.native_claim_basis_id.content_id()
                == root_binding.native_claim_basis_id
            && bundle_value.market_genesis_profile_id.content_id()
                == root_binding.market_genesis_profile_id
            && bundle_value.price_measure_policy_id.content_id()
                == root_binding.price_measure_policy_id
            && bundle_value.evidence_only_recovery_policy_id.content_id()
                == root_binding.recovery_policy_id
            && bundle_value.source_release_manifest_id == root_binding.source_release_id
            && bundle_value.source_plane_contract_id == root_binding.source_plane_contract_id
            && bundle_value.source_spec_id == root_binding.source_spec_id
            && link_binding.compiler_bundle_id == bundle.bundle_id()
            && funding.account().to_bytes() == link_binding.funding_state_account_id.bytes()
            && !funding.is_writable()
            && link_binding.source_release_id == root_binding.source_release_id
            && link_binding.source_plane_contract_id == root_binding.source_plane_contract_id
            && link_binding.source_spec_id == root_binding.source_spec_id
            && link_binding.source_route_id == root_binding.source_route_id
            && link_binding.clock_policy_id == root_binding.clock_policy_id
            && certificate.product_template_id() == bundle_value.product_template_id
            && certificate.market_genesis_profile_id() == bundle_value.market_genesis_profile_id
            && certificate.native_claim_basis_id() == bundle_value.native_claim_basis_id
            && certificate.price_measure_policy_id() == bundle_value.price_measure_policy_id
            && certificate.capability_profile_id() == registry.capability_profile_id()
            && certificate_id.bytes() == failure_resolution.facts().product_certificate_id.bytes(),
        ClutchError::MismatchedState,
    )
}

fn require_exact_collateral_prestate(
    program_id: &Pubkey,
    hoard_account: &AccountInfo<'_>,
    claim_ledger_account: &AccountInfo<'_>,
    liabilities: GeneralMarketLiabilityAuthorityV5,
    root_binding: MarketLifecycleBindingV3,
    registry: clutch_product_series::RegistryCapabilityProjectionV2,
) -> Outcome<()> {
    require_program_owned_writable(hoard_account, program_id, HOARD_V2_BYTES)?;
    require_program_owned_writable(claim_ledger_account, program_id, CLAIM_LEDGER_V3_BYTES)?;
    let hoard = HoardV2::decode(&hoard_account.data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let claim_ledger = ClaimLedgerV3::decode(&claim_ledger_account.data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let market = root_binding.market_instance_id.bytes();
    expect_pda(
        hoard_account.key,
        seeds::hoard_v2_pda(program_id, &market),
        Some(hoard.stored_bump),
    )?;
    expect_pda(
        claim_ledger_account.key,
        seeds::claim_ledger_v3_pda(program_id, &market),
        Some(claim_ledger.stored_bump),
    )?;
    let hoard_id = hoard
        .semantic_id(&RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let claim_id = claim_ledger
        .semantic_id(&RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let relation_market = liabilities.market_binding.base().base();
    let bound_realm = liabilities.bound.realm_bound().realm();
    let bound_release_id = liabilities
        .bound
        .release()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require_liability_prestate_join_v5(
        hoard == liabilities.hoard,
        claim_ledger == liabilities.claim_ledger,
        hoard_id,
        liabilities.hoard_semantic_id,
        claim_id,
        liabilities.claim_ledger_semantic_id,
    )?;
    require(
        liabilities
            .market_instance
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == root_binding.market_instance_id
            && relation_market.market_instance_v2_id.bytes() == market
            && relation_market.market_genesis_profile_v2_id.bytes()
                == root_binding.market_genesis_profile_id.bytes()
            && relation_market.native_claim_basis_id.bytes()
                == root_binding.native_claim_basis_id.bytes()
            && relation_market.price_measure_policy_v1_id.bytes()
                == root_binding.price_measure_policy_id.bytes()
            && relation_market.outcome_count == root_binding.outcome_count
            && bound_realm.realm.bytes() == root_binding.realm_id.bytes()
            && bound_realm.profile.bytes() == root_binding.collateral_profile_id.bytes()
            && liabilities.bound.policy_id().bytes() == root_binding.collateral_policy_id.bytes()
            && bound_release_id.bytes() == root_binding.collateral_release_id.bytes()
            && registry.realm_collateral.realm_id == root_binding.realm_id
            && registry.realm_collateral.profile_id == root_binding.collateral_profile_id
            && liabilities.bound.market().collateral_cap_atoms
                == liabilities.market_instance.collateral_cap
            && liabilities.market_instance.collateral_cap
                <= registry.realm_collateral.market_collateral_cap_ceiling
            && liabilities.bound.market().market.bytes() == market,
        ClutchError::MismatchedState,
    )
}

#[allow(clippy::too_many_arguments)]
fn derive_finalization_evidence_id_v5(
    root: &AuthenticatedMarketLifecycleRootV3<'_>,
    link: &AuthenticatedSeriesMarketLinkV3<'_>,
    release: &AuthenticatedWritableFailureSessionReleaseLinkV4,
    funding: &AuthenticatedSeriesFundingAccountV5,
    registry: &AuthenticatedRegistryCapabilityV5,
    bundle: &AuthenticatedCompiledProductSeriesBundleV7,
    inactive: &AuthenticatedInactiveFailureResolutionV5,
    liabilities: GeneralMarketLiabilityAuthorityV5,
    expected_hoard_after_id: CollateralId,
    expected_claim_ledger_after_id: CollateralId,
    failure_resolution: FailureMarketIntervalCellResolutionReceiptV2,
    root_binding_id: ContentId,
    certificate_id: ContentId,
    resolution_account: &Pubkey,
    outcome_count: u8,
    denominator: u64,
    weights: &[u64; clutch_retirement::MAX_OUTCOMES],
) -> Outcome<ContentId> {
    let failure = failure_resolution.facts();
    let link_state_id = link.semantic_id();
    let projection = FailureAction12FinalizationEvidenceProjectionV5 {
        root_binding_id,
        root: (root.account().to_bytes(), root.authentication_id()),
        link: (link.account().to_bytes(), link.authentication_id()),
        link_release_id: release.id(),
        link_state_id: ContentId::from_bytes(link_state_id.bytes()),
        registry: FailureAction12RegistryProjectionV5 {
            series_registry_account: registry.series_registry_account().to_bytes(),
            program_account: registry.program_account().to_bytes(),
            programdata_account: registry.programdata_account().to_bytes(),
            release_artifact_account: registry.release_artifact_account().to_bytes(),
            profile_artifact_account: registry.profile_artifact_account().to_bytes(),
            registry_release_id: registry.registry_release_id(),
            capability_profile_id: registry.capability_profile_id(),
        },
        bundle: (bundle.artifact_account().to_bytes(), bundle.bundle_id()),
        funding: (
            funding.account().to_bytes(),
            funding.authentication_id(),
            funding.data_id(),
        ),
        foundation: (
            root.binding().foundation_schedule_id,
            root.binding().foundation_account_graph_id,
            root.state().foundation().transcript_id,
        ),
        inactive: (
            inactive.authentication_id(),
            inactive.semantic_id(),
            inactive.data_id(),
        ),
        inactive_rent: (
            inactive.resolution().rent.refundable_principal,
            inactive.resolution().rent.donation_floor,
            inactive.resolution().rent.payer.bytes(),
        ),
        liabilities: (
            ContentId::from_bytes(liabilities.receipt_id.bytes()),
            ContentId::from_bytes(liabilities.hoard_semantic_id.bytes()),
            ContentId::from_bytes(expected_hoard_after_id.bytes()),
            ContentId::from_bytes(liabilities.claim_ledger_semantic_id.bytes()),
            ContentId::from_bytes(expected_claim_ledger_after_id.bytes()),
        ),
        failure: FailureAction12ReceiptProjectionV5 {
            resolution_id: ContentId::from_bytes(failure_resolution.id().bytes()),
            failure_policy_binding_id: ContentId::from_bytes(
                failure_resolution.failure_policy_binding_id().bytes(),
            ),
            cell_before: ContentId::from_bytes(failure.cell_before.bytes()),
            cell_after: ContentId::from_bytes(failure.cell_after.bytes()),
            session_binding_id: ContentId::from_bytes(failure.session_binding_id.bytes()),
            source_handoff_id: ContentId::from_bytes(failure.source_handoff_id.bytes()),
            terminal_work_id: ContentId::from_bytes(failure.terminal_work_id.bytes()),
            product_certificate_id: certificate_id,
            last_runtime_work_receipt_id: ContentId::from_bytes(
                failure.last_runtime_work_receipt_id.bytes(),
            ),
            completed_work_calls: failure.completed_work_calls,
            exact_reward_lamports: failure.exact_reward_lamports,
        },
        resolution_account: resolution_account.to_bytes(),
        outcome_count,
        denominator,
        weights: *weights,
    };
    let id = projection.id(&RuntimeSha256);
    require_live_content_id(id)?;
    Ok(id)
}

#[allow(clippy::too_many_arguments)]
fn write_collateral_activation_postimages_v6<'a>(
    program_id: &Pubkey,
    resolution_account: &AccountInfo<'a>,
    hoard_account: &AccountInfo<'a>,
    claim_ledger_account: &AccountInfo<'a>,
    inactive: &AuthenticatedInactiveFailureResolutionV5,
    resolution: ResolutionV5,
    hoard_after: HoardV2,
    claim_ledger_after: ClaimLedgerV3,
) -> Outcome<()> {
    require_program_owned_writable(resolution_account, program_id, RESOLUTION_V5_BYTES)?;
    require_program_owned_writable(hoard_account, program_id, HOARD_V2_BYTES)?;
    require_program_owned_writable(claim_ledger_account, program_id, CLAIM_LEDGER_V3_BYTES)?;
    require(
        inactive.account() == *resolution_account.key
            && inactive.observed_lamports() == resolution_account.lamports()
            && inactive.resolution().state == clutch_collateral_adapter_v2::ResolutionStateV5::Inactive
            && resolution.state == clutch_collateral_adapter_v2::ResolutionStateV5::Finalized
            && resolution.stored_bump == inactive.resolution().stored_bump
            && resolution.rent == inactive.resolution().rent
            && resolution.facts.market_instance_id
                == inactive.resolution().facts.market_instance_id
            && resolution.facts.native_claim_basis_id
                == inactive.resolution().facts.native_claim_basis_id
            && resolution.facts.outcome_count == inactive.resolution().facts.outcome_count
            && resolution.facts.generation == inactive.resolution().facts.generation,
        ClutchError::MismatchedState,
    )?;
    {
        let data = resolution_account
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let observed = ResolutionV5::decode(&data)
            .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
        require(observed == inactive.resolution(), ClutchError::MismatchedState)?;
    }
    let resolution_lamports_before = resolution_account.lamports();
    let hoard_lamports_before = hoard_account.lamports();
    let claim_ledger_lamports_before = claim_ledger_account.lamports();
    {
        let mut data = resolution_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        resolution
            .encode(&mut data)
            .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    }
    {
        let mut data = hoard_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        hoard_after
            .encode(&mut data)
            .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    }
    {
        let mut data = claim_ledger_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        claim_ledger_after
            .encode(&mut data)
            .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    }
    require(
        resolution_account.lamports() == resolution_lamports_before
            && hoard_account.lamports() == hoard_lamports_before
            && claim_ledger_account.lamports() == claim_ledger_lamports_before,
        ClutchError::MismatchedState,
    )
}

fn require_program_owned_writable(
    account: &AccountInfo<'_>,
    program_id: &Pubkey,
    expected_len: usize,
) -> Outcome<()> {
    require(
        account.owner == program_id
            && account.is_writable
            && !account.is_signer
            && !account.executable
            && account.data_len() == expected_len,
        ClutchError::MismatchedState,
    )
}

fn require_distinct_resolution_role_keys_v5<const N: usize>(keys: [Pubkey; N]) -> Outcome<()> {
    let mut index = 0usize;
    while index < keys.len() {
        let mut prior = 0usize;
        while prior < index {
            require(keys[index] != keys[prior], ClutchError::AccountAlias)?;
            prior += 1;
        }
        index += 1;
    }
    Ok(())
}

fn require_source_resolution_outer_aliases_v5<const P: usize, const E: usize>(
    protocol_roles: [Pubkey; P],
    external_roles: [Pubkey; E],
) -> Outcome<()> {
    require_distinct_resolution_role_keys_v5(protocol_roles)?;
    let mut external_index = 0usize;
    while external_index < external_roles.len() {
        let mut protocol_index = 0usize;
        while protocol_index < protocol_roles.len() {
            require(
                external_roles[external_index] != protocol_roles[protocol_index],
                ClutchError::AccountAlias,
            )?;
            protocol_index += 1;
        }
        external_index += 1;
    }
    Ok(())
}

fn require_current_series_funding_graph_v6(
    registry_funding_terms_id: SeriesFundingTermsV2Id,
    bundle_funding_terms_id: SeriesFundingTermsV2Id,
    link_funding_terms_id: SeriesFundingTermsV2Id,
    bundle_funding_quote_id: SeriesFundingQuoteV6Id,
    link_funding_quote_id: SeriesFundingQuoteV6Id,
    bundle_attachment_plan_id: SeriesAttachmentPlanV6Id,
    link_attachment_plan_id: SeriesAttachmentPlanV6Id,
    funding: &AuthenticatedSeriesFundingAccountV5,
) -> Outcome<()> {
    let state = funding.state();
    require(
        registry_funding_terms_id == bundle_funding_terms_id
            && registry_funding_terms_id == link_funding_terms_id
            && bundle_funding_quote_id == link_funding_quote_id
            && bundle_attachment_plan_id == link_attachment_plan_id
            && state.funding_terms_id == registry_funding_terms_id
            && state.funding_quote_id == bundle_funding_quote_id
            && state.attachment_plan_id == bundle_attachment_plan_id,
        ClutchError::MismatchedState,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionAuthorityFactsV5 {
    root_phase: MarketLifecyclePhaseV3,
    root_resolution_semantic_id: ContentId,
    root_resolution_data_id: ContentId,
    root_resolution_activation_receipt_id: ContentId,
    link_phase: SeriesMarketLinkPhaseV3,
    active_failure_sessions: u32,
    link_session_binding_id: [u8; 32],
    failure_session_binding_id: [u8; 32],
    failure_market_instance_id: [u8; 32],
    root_market_instance_id: [u8; 32],
    failure_generation: u64,
    root_generation: u64,
    failure_policy_binding_id: [u8; 32],
    root_failure_policy_binding_id: [u8; 32],
    failure_product_certificate_id: [u8; 32],
    product_certificate_id: [u8; 32],
    certificate_source_occurrence_id: [u8; 32],
    link_source_occurrence_id: [u8; 32],
    payout_active_len: u8,
    root_outcome_count: u8,
    registry_neutral_lamport_sink: [u8; 32],
    root_neutral_lamport_sink: [u8; 32],
}

fn require_resolution_authority_facts_v5(facts: ResolutionAuthorityFactsV5) -> Outcome<()> {
    require(
        facts.root_phase == MarketLifecyclePhaseV3::Active
            && facts.root_resolution_semantic_id == ContentId::ZERO
            && facts.root_resolution_data_id == ContentId::ZERO
            && facts.root_resolution_activation_receipt_id == ContentId::ZERO
            && facts.link_phase == SeriesMarketLinkPhaseV3::Active
            && facts.active_failure_sessions == 1
            && facts.link_session_binding_id == facts.failure_session_binding_id
            && facts.failure_market_instance_id == facts.root_market_instance_id
            && facts.failure_generation == facts.root_generation
            && facts.failure_policy_binding_id == facts.root_failure_policy_binding_id
            && facts.failure_product_certificate_id == facts.product_certificate_id
            && facts.certificate_source_occurrence_id == facts.link_source_occurrence_id
            && facts.payout_active_len == facts.root_outcome_count
            && facts.registry_neutral_lamport_sink == facts.root_neutral_lamport_sink,
        ClutchError::MismatchedState,
    )
}

fn require_liability_prestate_join_v5(
    hoard_body_matches: bool,
    claim_ledger_body_matches: bool,
    live_hoard_id: CollateralId,
    authenticated_hoard_id: CollateralId,
    live_claim_ledger_id: CollateralId,
    authenticated_claim_ledger_id: CollateralId,
) -> Outcome<()> {
    require(
        hoard_body_matches
            && claim_ledger_body_matches
            && live_hoard_id == authenticated_hoard_id
            && live_claim_ledger_id == authenticated_claim_ledger_id,
        ClutchError::MismatchedState,
    )
}

fn require_live_content_id(id: ContentId) -> Outcome<()> {
    require(id != ContentId::ZERO, ClutchError::MismatchedState)
}

#[cfg(test)]
mod adversarial_tests {
    #[test]
    fn current_resolution_is_a_physical_inactive_to_finalized_transition() {
        let source = include_str!("failure_market_resolution_v5.rs");
        let production = source.split("#[cfg(test)]").next().expect("production");
        for required in [
            "AuthenticatedInactiveFailureResolutionV5",
            "write_collateral_activation_postimages_v6",
            "authenticate_current_market_resolution_activation_postwrite_v5",
            "record_current_market_resolution_activation_v3",
            "AuthenticatedSeriesFundingAccountV5",
            "archive_resolved_failure_market_interval_link_v4",
            "write_failure_market_runtime_session_plan_v3",
            "persist_resolved_failure_market_family_v3",
        ] {
            assert!(production.contains(required), "missing current owner: {required}");
        }
        for forbidden in [
            "allocate_data(",
            "assign_data(",
            "AuthenticatedMarketFoundationPreallocationV3",
            "MarketResolutionActivationV2",
            "AuthenticatedMarketLifecycleRootV2",
            "AuthenticatedSeriesMarketLinkV2",
            "write_failure_market_runtime_session_plan_v2",
        ] {
            assert!(!production.contains(forbidden), "obsolete authority: {forbidden}");
        }
    }

    #[test]
    fn finalization_evidence_binds_physical_product_and_failure_owners() {
        let source = include_str!("failure_market_resolution_v5.rs");
        let production = source.split("#[cfg(test)]").next().expect("production");
        for required in [
            "inactive.authentication_id()",
            "funding.authentication_id()",
            "foundation_account_graph_id",
            "foundation().transcript_id",
            "failure_resolution.id()",
            "collateral_postwrite.receipt_id()",
        ] {
            assert!(production.contains(required), "missing evidence edge: {required}");
        }
    }
}
