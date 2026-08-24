// SPDX-License-Identifier: AGPL-3.0-or-later
//! Capability-disabled atomic Product/Failure Resolution V5 composer.
//!
//! This module is deliberately not routed by dispatch. It is the single live
//! writer boundary which joins Product's authenticated active Market root,
//! pinned Series link, and retained slot-10 preallocation to Failure's private
//! exhaustive interval receipt and Collateral's exact Hoard/ClaimLedger
//! postimage verifier. The preallocated Resolution PDA supplies only its
//! separately itemized rent principal; no Recovery work capital, Hoard
//! principal, future fees, or caller-selected funding source participates.

use crate::accounts::{expect_pda, require, require_distinct, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::collateral_position_v3::{
    authenticate_market_resolution_activation_postwrite_v5,
    AuthenticatedMarketResolutionActivationPostwriteV5, GeneralMarketLiabilityAuthorityV2,
    RuntimeSha256,
};
use crate::instructions::failure_market_admission::AuthenticatedFailureMarketRootV2;
use crate::instructions::failure_market_interval_v2::{
    archive_failure_market_interval_session_v2, write_failure_market_interval_resolution_plan_v2,
    AuthenticatedFailureMarketIntervalAccountsV2, AuthenticatedFailureMarketProductResolutionV2,
    FailureMarketIntervalArchivePostwriteV2,
};
use crate::instructions::failure_market_family_terminal_v2::{
    persist_resolved_failure_market_family_v2,
    AuthenticatedFailureMarketFamilyTerminalPostwriteV2,
};
use crate::instructions::failure_market_recovery_terminal_v2::{
    close_failure_market_recovery_v2, AuthenticatedFailureMarketRecoveryClosePostwriteV2,
};
use crate::instructions::failure_market_replay_v2::AuthenticatedFailureMarketReplayV2;
use crate::instructions::failure_market_runtime::{
    write_failure_market_runtime_session_plan_v1, AuthenticatedFailureMarketRuntimeRootV1,
    AuthenticatedFailureMarketRuntimeSessionPostwriteV1,
    AuthenticatedFailureMarketRuntimeSessionWriteV1, FailureMarketRuntimeSessionWriteFactsV1,
};
use crate::instructions::genesis::{
    allocate_data, assign_data, read_rent, require_system_program, SYSTEM_PROGRAM_ID,
};
use crate::instructions::product_artifact::{
    AuthenticatedProductArtifactV1, AuthenticatedRegistryCapabilityV3,
};
use crate::instructions::product_market::{
    authenticate_market_lifecycle_root_v1, authenticate_series_market_link_v1,
    authenticate_writable_failure_resolution_link_v1, record_market_resolution_activation_v1,
    AuthenticatedMarketFoundationPreallocationV2, AuthenticatedMarketLifecycleRootV1,
    AuthenticatedMarketResolutionActivationWriteV1, AuthenticatedSeriesFailureSessionReleaseV1,
    AuthenticatedWritableFailureResolutionLinkV1,
};
use crate::instructions::product_series::AuthenticatedSourceResolutionInputV3;
use crate::instructions::source_terminal_resolution_v5::{
    close_successful_source_statistic_result_v1, compose_source_resolution_terminal_v1,
    AuthenticatedSourceResolutionStatisticResultCloseV1,
    AuthenticatedSourceResolutionTerminalPolicyV1, AuthenticatedSourceResolutionTerminalV1,
    AuthenticatedSourceResolutionV5TerminalV1,
};
use crate::seeds;
use clutch_collateral_adapter_v2::{
    prepare_market_resolution_activation_v5, ClaimLedgerV3, HoardV2, Id as CollateralId,
    MarketLiabilityLifecycleV1, ResolutionFinalizationFactsV5, ResolutionPayoutUnitBoundaryV5,
    ResolutionV5, CLAIM_LEDGER_V3_BYTES, HOARD_V2_BYTES, RESOLUTION_V5_BYTES,
};
use clutch_failure_policy_runtime::market_interval_cell_v2::{
    plan_reset_failure_market_interval_cell_v2,
    project_failure_market_interval_terminal_history_facts_v2,
    FailureMarketIntervalCellDispositionV2, FailureMarketIntervalCellPlanV2,
    FailureMarketIntervalCellResetReceiptV2, FailureMarketIntervalCellResolutionPlanV2,
    FailureMarketIntervalCellResolutionReceiptV2,
};
use clutch_failure_policy_runtime::market_interval_history_v2::{
    plan_append_failure_market_interval_history_v2,
    AuthenticatedFailureMarketIntervalTerminalV2, FailureMarketIntervalHistoryAppendReceiptV2,
    FailureMarketIntervalHistoryPlanV2, FailureMarketIntervalTerminalDispositionV2,
    FailureMarketIntervalTerminalFactsV2,
};
use clutch_failure_policy_runtime::market_runtime_v1::{
    plan_resolve_failure_market_session_v1, AuthenticatedFailureMarketSessionV1,
    FailureMarketSessionResolutionFactsV1,
};
use clutch_product_series::{
    CompiledProductSeriesBundleV5, ContentId, MarketFoundationSlotV2, MarketLifecyclePhaseV1,
    MarketResolutionActivationV1, SeriesAttachmentPlanV4Id, SeriesFundingQuoteV4Id,
    SeriesFundingTermsV2Id, SeriesMarketLinkPhaseV1,
};
use clutch_retirement::{DeletableRentOwnerV1, Identity32V1};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV1, SeriesMarketLinkAccountV1,
};
use clutch_source_plane_v3_runtime::{
    AuthenticatedReopenLineageV1, AuthenticatedSourceRouteV1, SourceWorkScheduleBindingV1,
};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const FAILURE_MARKET_RESOLUTION_ACTIVATION_AUTHENTICATION_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/failure-market-resolution-activation/v5\0";
const FAILURE_MARKET_RESOLUTION_FINALIZATION_EVIDENCE_DOMAIN_V5: &[u8] =
    b"dragons-clutch/failure-market-resolution-finalization-evidence/v5\0";
const FAILURE_MARKET_RESOLUTION_POSTWRITE_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/failure-market-resolution-postwrite/v5\0";

/// Stable byte committed for the only disposition admitted by this composer.
const RESOLVED_DISPOSITION_BYTE_V2: u8 = 1;
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

/// Derive the sole resolved-session append/reset batch after Product,
/// Failure, Collateral, and Source terminal postwrites all exist. This is not
/// a generic terminal DTO: every fact comes from the hostile-reopened resolved
/// cell and its private resolution/Source receipts.
fn plan_resolved_failure_market_archive_v5(
    admission: AuthenticatedFailureMarketRootV2,
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
    product_activation: MarketResolutionActivationV1,
    collateral_postwrite: AuthenticatedMarketResolutionActivationPostwriteV5,
    market_root: Pubkey,
    market_root_authentication_before: ContentId,
    market_root_authentication_after: ContentId,
    series_link: Pubkey,
    series_link_authentication: ContentId,
    series_link_preauthorization_id: ContentId,
    slot10_preallocation_id: ContentId,
    finalization_evidence_id: ContentId,
}

impl AuthenticatedFailureMarketResolutionActivationV5 {
    /// Complete same-call authorization identity.
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    /// Product's exact once-only `0xaa` activation postimage.
    pub(crate) const fn product_activation(self) -> MarketResolutionActivationV1 {
        self.product_activation
    }

    /// Collateral's exact Resolution/Hoard/ClaimLedger postwrite proof.
    pub(crate) const fn collateral_postwrite(
        self,
    ) -> AuthenticatedMarketResolutionActivationPostwriteV5 {
        self.collateral_postwrite
    }

    /// Exact Product-retained slot-10 preallocation consumed by the writer.
    pub(crate) const fn slot10_preallocation_id(self) -> ContentId {
        self.slot10_preallocation_id
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
    series_link_state_id: clutch_product_series::SeriesMarketLinkV1Id,
    session_before: ContentId,
    session_after: ContentId,
    resolution_receipt_id: ContentId,
}

impl AuthenticatedFailureMarketSessionV1 for FailureMarketRuntimeResolutionAuthorityV1 {
    fn authenticate_failure_market_session_resolution(
        &self,
        expected: FailureMarketSessionResolutionFactsV1,
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

/// Runtime write authority minted only after Product, collateral, and cell writes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketRuntimeResolutionWriteV1 {
    expected: FailureMarketRuntimeSessionWriteFactsV1,
    cell_state_after: ContentId,
    runtime_session_state_after: ContentId,
    activation_failure_receipt_id: ContentId,
    runtime_resolution_receipt_id: ContentId,
}

impl AuthenticatedFailureMarketRuntimeSessionWriteV1 for FailureMarketRuntimeResolutionWriteV1 {
    fn authenticate_failure_market_runtime_session_write_v1(
        &self,
        expected: FailureMarketRuntimeSessionWriteFactsV1,
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

    pub(crate) const fn product_activation(self) -> MarketResolutionActivationV1 {
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
#[derive(Clone, Copy, Debug)]
pub(crate) struct AuthenticatedResolvedFailureMarketLifecycleV5 {
    resolution: AuthenticatedFailureMarketResolutionPostwriteV5,
    source_terminal: AuthenticatedSourceResolutionTerminalV1,
    archive: FailureMarketIntervalArchivePostwriteV2,
    link_release: AuthenticatedSeriesFailureSessionReleaseV1,
    source_result_close: AuthenticatedSourceResolutionStatisticResultCloseV1,
    recovery_close: AuthenticatedFailureMarketRecoveryClosePostwriteV2,
    family_terminal: AuthenticatedFailureMarketFamilyTerminalPostwriteV2,
}

impl AuthenticatedResolvedFailureMarketLifecycleV5 {
    pub(crate) const fn family_terminal(
        self,
    ) -> AuthenticatedFailureMarketFamilyTerminalPostwriteV2 {
        self.family_terminal
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
        source: AuthenticatedSourceResolutionInputV3,
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
struct AuthenticatedProductResolutionRootWriteV5 {
    root_authentication_before: ContentId,
    activation: MarketResolutionActivationV1,
    slot10_preallocation_id: ContentId,
    collateral_plan_receipt_id: ContentId,
    failure_resolution_id: ContentId,
    collateral_postwrite_id: ContentId,
    finalization_evidence_id: ContentId,
}

impl AuthenticatedMarketResolutionActivationWriteV1 for AuthenticatedProductResolutionRootWriteV5 {
    fn authenticate_market_resolution_activation_write_v1(
        &self,
        root_authentication_before: ContentId,
        expected: MarketResolutionActivationV1,
        slot10_preallocation_id: ContentId,
        collateral_plan_receipt_id: ContentId,
        collateral_postwrite_receipt_id: ContentId,
        failure_resolution_receipt_id: ContentId,
    ) -> Outcome<()> {
        require(
            root_authentication_before == self.root_authentication_before
                && expected == self.activation
                && slot10_preallocation_id == self.slot10_preallocation_id
                && collateral_plan_receipt_id == self.collateral_plan_receipt_id
                && collateral_postwrite_receipt_id == self.collateral_postwrite_id
                && failure_resolution_receipt_id == self.failure_resolution_id
                && expected.failure_resolution_receipt_id() == self.failure_resolution_id
                && expected.composite_finalization_evidence_id() == self.finalization_evidence_id
                && self.collateral_postwrite_id != ContentId::ZERO,
            ClutchError::MismatchedState,
        )
    }
}

/// Claim the exact retained Resolution V5 preallocation, atomically write the
/// three collateral postimages, advance Product's active root once, and mint
/// the sole private authority accepted by the Failure resolved-cell writer.
///
/// `root_before` and `link_before` are private Product receipts constructed by
/// their semantic owners. This function does not trust their copied values: it
/// hostile-reopens both physical accounts and requires byte-for-byte identical
/// authentication before any write. The link stays read-only and pinned; the
/// Failure cell is written only by the caller after this function returns.
#[allow(clippy::too_many_arguments)]
fn activate_failure_market_resolution_v5<'a, 'root, 'post>(
    program_id: &Pubkey,
    market_root_account: &AccountInfo<'a>,
    series_link_account: &AccountInfo<'a>,
    resolution_account: &AccountInfo<'a>,
    hoard_account: &AccountInfo<'a>,
    claim_ledger_account: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    root_before: AuthenticatedMarketLifecycleRootV1<'root>,
    link_before: &AuthenticatedWritableFailureResolutionLinkV1,
    registry: AuthenticatedRegistryCapabilityV3,
    bundle: AuthenticatedProductArtifactV1<CompiledProductSeriesBundleV5>,
    slot10: AuthenticatedMarketFoundationPreallocationV2,
    liabilities: GeneralMarketLiabilityAuthorityV2,
    failure_resolution: FailureMarketIntervalCellResolutionReceiptV2,
    root_decode_before: &'root mut MarketLifecycleRootAccountV1,
    root_decode_after: &'post mut MarketLifecycleRootAccountV1,
) -> Outcome<AuthenticatedFailureMarketResolutionActivationV5> {
    require_system_program(system_program)?;
    require_distinct(&[
        market_root_account.clone(),
        series_link_account.clone(),
        resolution_account.clone(),
        hoard_account.clone(),
        claim_ledger_account.clone(),
        rent_sysvar.clone(),
        system_program.clone(),
    ])?;

    let expected_root_binding = root_before.state().binding();
    let live_root = authenticate_market_lifecycle_root_v1(
        program_id,
        market_root_account,
        expected_root_binding.market_instance_id,
        expected_root_binding.generation,
        true,
        root_decode_before,
    )?;
    require_exact_cached_account_v5(
        live_root.account(),
        root_before.account(),
        live_root.authentication_id(),
        root_before.authentication_id(),
        live_root.value(),
        root_before.value(),
    )?;
    let root = live_root.state();
    let root_binding = root.binding();
    let link = link_before.state();
    let link_binding = link.binding();
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
    let root_binding_id = root_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    require_current_product_failure_join(
        market_root_account,
        &live_root,
        root,
        root_binding_id,
        link_before,
        link,
        registry,
        &bundle,
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
    require_exact_slot10_preallocation(
        program_id,
        resolution_account,
        rent_sysvar,
        live_root,
        slot10,
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
        live_root,
        link_before,
        registry,
        &bundle,
        slot10,
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
    let rent = DeletableRentOwnerV1::from_persisted(
        Identity32V1::new(slot10.rent_refund_owner().to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        slot10.principal_lamports(),
        slot10.donation_lamports(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
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
        rent,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        expected_resolution == *resolution_account.key,
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

    let product_activation = MarketResolutionActivationV1::new(
        root_binding,
        ContentId::from_bytes(activation_plan.resolution_id().bytes()),
        ContentId::from_bytes(activation_plan.resolution_data_id().bytes()),
        ContentId::from_bytes(failure_resolution.id().bytes()),
        certificate_id.content_id(),
        finalization_evidence_id,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    write_collateral_activation_postimages_v5(
        program_id,
        system_program,
        resolution_account,
        hoard_account,
        claim_ledger_account,
        root_binding.market_instance_id.bytes(),
        resolution_bump,
        resolution,
        activation_plan.hoard_after(),
        activation_plan.claim_ledger_after(),
        slot10.observed_balance_lamports(),
    )?;
    let collateral_postwrite = authenticate_market_resolution_activation_postwrite_v5(
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
    let root_write_authority = AuthenticatedProductResolutionRootWriteV5 {
        root_authentication_before: live_root.authentication_id(),
        activation: product_activation,
        slot10_preallocation_id: slot10.id(),
        collateral_plan_receipt_id: ContentId::from_bytes(activation_plan.receipt_id().bytes()),
        failure_resolution_id: ContentId::from_bytes(failure_resolution.id().bytes()),
        collateral_postwrite_id: ContentId::from_bytes(collateral_postwrite.receipt_id().bytes()),
        finalization_evidence_id,
    };
    let root_after = record_market_resolution_activation_v1(
        program_id,
        market_root_account,
        live_root,
        product_activation,
        slot10.id(),
        ContentId::from_bytes(activation_plan.receipt_id().bytes()),
        ContentId::from_bytes(collateral_postwrite.receipt_id().bytes()),
        ContentId::from_bytes(failure_resolution.id().bytes()),
        &root_write_authority,
        root_decode_after,
    )?;
    require(
        root_after.state().resolution_activation_receipt_id() == product_activation.id()
            && root_after.state().resolution_semantic_id()
                == product_activation.resolution_semantic_id()
            && root_after.state().resolution_data_id() == product_activation.resolution_data_id()
            && root_after.state().transition_sequence()
                == root
                    .transition_sequence()
                    .checked_add(1)
                    .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?,
        ClutchError::MismatchedState,
    )?;

    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            FAILURE_MARKET_RESOLUTION_ACTIVATION_AUTHENTICATION_DOMAIN_V5,
            market_root_account.key.as_ref(),
            &live_root.authentication_id().bytes(),
            &root_after.authentication_id().bytes(),
            series_link_account.key.as_ref(),
            &link_before.authentication_id().bytes(),
            &link_before.id().bytes(),
            resolution_account.key.as_ref(),
            &slot10.id().bytes(),
            &failure_resolution.id().bytes(),
            &certificate_id.bytes(),
            &finalization_evidence_id.bytes(),
            &product_activation.id().bytes(),
            &activation_plan.receipt_id().bytes(),
            &collateral_postwrite.receipt_id().bytes(),
            &slot10.principal_lamports().to_le_bytes(),
            &slot10.donation_lamports().to_le_bytes(),
            &[RESOLVED_DISPOSITION_BYTE_V2],
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(AuthenticatedFailureMarketResolutionActivationV5 {
        id,
        failure_resolution,
        product_activation,
        collateral_postwrite,
        market_root: *market_root_account.key,
        market_root_authentication_before: live_root.authentication_id(),
        market_root_authentication_after: root_after.authentication_id(),
        series_link: *series_link_account.key,
        series_link_authentication: link_before.authentication_id(),
        series_link_preauthorization_id: link_before.id(),
        slot10_preallocation_id: slot10.id(),
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
fn resolve_failure_market_interval_v5<'a, 'root, 'post>(
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
    rent_sysvar: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    root_before: AuthenticatedMarketLifecycleRootV1<'root>,
    link_before: &AuthenticatedWritableFailureResolutionLinkV1,
    admission: AuthenticatedFailureMarketRootV2,
    runtime_before: AuthenticatedFailureMarketRuntimeRootV1,
    interval_before: AuthenticatedFailureMarketIntervalAccountsV2,
    registry: AuthenticatedRegistryCapabilityV3,
    bundle: AuthenticatedProductArtifactV1<CompiledProductSeriesBundleV5>,
    slot10: AuthenticatedMarketFoundationPreallocationV2,
    liabilities: GeneralMarketLiabilityAuthorityV2,
    resolution: FailureMarketIntervalCellResolutionPlanV2,
    root_decode_before: &'root mut MarketLifecycleRootAccountV1,
    root_decode_after: &'post mut MarketLifecycleRootAccountV1,
) -> Outcome<(
    AuthenticatedFailureMarketResolutionPostwriteV5,
    AuthenticatedFailureMarketIntervalAccountsV2,
    AuthenticatedFailureMarketRuntimeSessionPostwriteV1,
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
        *rent_sysvar.key,
        *system_program.key,
    ])?;
    let failure_resolution = resolution.receipt();
    let failure_facts = failure_resolution.facts();
    let session_after = ContentId::from_bytes(failure_facts.cell_after.bytes());
    let runtime_resolution_receipt_id = ContentId::from_bytes(failure_resolution.id().bytes());
    let link_state_id = link_before
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let runtime_authority = FailureMarketRuntimeResolutionAuthorityV1 {
        runtime_before: runtime_before.state_commitment(),
        series_link_state_id: link_state_id,
        session_before: ContentId::from_bytes(failure_facts.cell_before.bytes()),
        session_after,
        resolution_receipt_id: runtime_resolution_receipt_id,
    };
    let runtime_plan = plan_resolve_failure_market_session_v1(
        &runtime_authority,
        runtime_before.state(),
        admission.state(),
        link_before.state(),
        session_after,
        runtime_resolution_receipt_id,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        runtime_plan.series_link_before() == link_before.state()
            && runtime_plan.series_link_after() == link_before.state()
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
        resolution_account,
        hoard_account,
        claim_ledger_account,
        rent_sysvar,
        system_program,
        root_before,
        link_before,
        registry,
        bundle,
        slot10,
        liabilities,
        failure_resolution,
        root_decode_before,
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
    let runtime_write_facts = FailureMarketRuntimeSessionWriteFactsV1 {
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
    let runtime_postwrite = write_failure_market_runtime_session_plan_v1(
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
    let postwrite_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            FAILURE_MARKET_RESOLUTION_POSTWRITE_DOMAIN_V5,
            admission_root_account.key.as_ref(),
            runtime_root_account.key.as_ref(),
            market_root_account.key.as_ref(),
            series_link_account.key.as_ref(),
            interval_cell_account.key.as_ref(),
            &activation.id().bytes(),
            &activation.market_root_authentication_after().bytes(),
            &interval_after.cell_authentication_id().bytes(),
            &interval_after.cell_state_id().bytes(),
            &runtime_postwrite.id().bytes(),
            &runtime_postwrite.transition_receipt_id().bytes(),
            &activation
                .product_activation()
                .resolution_semantic_id()
                .bytes(),
            &activation.product_activation().resolution_data_id().bytes(),
            &activation
                .product_activation()
                .resolution_account_id()
                .bytes(),
        ])
        .to_bytes(),
    );
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
pub(crate) fn resolve_failure_market_interval_and_source_v5<'a, 'root, 'link, 'post>(
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
    root_before: AuthenticatedMarketLifecycleRootV1<'root>,
    admission: AuthenticatedFailureMarketRootV2,
    runtime_before: AuthenticatedFailureMarketRuntimeRootV1,
    interval_before: AuthenticatedFailureMarketIntervalAccountsV2,
    replay_before: AuthenticatedFailureMarketReplayV2,
    registry: AuthenticatedRegistryCapabilityV3,
    bundle: AuthenticatedProductArtifactV1<CompiledProductSeriesBundleV5>,
    slot10: AuthenticatedMarketFoundationPreallocationV2,
    liabilities: GeneralMarketLiabilityAuthorityV2,
    resolution: FailureMarketIntervalCellResolutionPlanV2,
    source_route: AuthenticatedSourceRouteV1,
    source_schedule: SourceWorkScheduleBindingV1,
    source_input: AuthenticatedSourceResolutionInputV3,
    source_lineage: AuthenticatedReopenLineageV1,
    root_decode_before: &'root mut MarketLifecycleRootAccountV1,
    link_decode_before: &'link mut SeriesMarketLinkAccountV1,
    root_decode_after: &'post mut MarketLifecycleRootAccountV1,
    link_release_decode: &mut SeriesMarketLinkAccountV1,
    link_rebound_output: &mut SeriesMarketLinkAccountV1,
    resolved_root_auth_decode: &mut MarketLifecycleRootAccountV1,
    resolved_root_persist_decode: &mut MarketLifecycleRootAccountV1,
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
    let resolution_link = authenticate_writable_failure_resolution_link_v1(
        program_id,
        series_link_account,
        root_before,
        link_decode_before,
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
        rent_sysvar,
        system_program,
        root_before,
        &resolution_link,
        admission,
        runtime_before,
        interval_before,
        registry,
        bundle,
        slot10,
        liabilities,
        resolution,
        root_decode_before,
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
    let link_binding = resolution_link.state().binding();
    let link_for_release = authenticate_series_market_link_v1(
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
    let (archive, link_release, archive_runtime) = archive_failure_market_interval_session_v2(
        program_id,
        admission_root_account,
        runtime_root_account,
        interval_cell_account,
        interval_history_account,
        series_link_account,
        interval_after,
        link_for_release,
        resolution_link,
        admission,
        runtime_after.root(),
        archive_plan.history_plan,
        archive_plan.append,
        archive_plan.cell_plan,
        archive_plan.reset,
        link_rebound_output,
    )?;
    require(
        link_release.resolution_link_preauthorization_id()
            == postwrite.activation().series_link_preauthorization_id(),
        ClutchError::MismatchedState,
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
        source_result_close,
    )?;
    let policy = admission.state().binding().facts();
    let resolved_root = authenticate_market_lifecycle_root_v1(
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
    let family_terminal = persist_resolved_failure_market_family_v2(
        program_id,
        market_root_account,
        admission_root_account,
        runtime_root_account,
        interval_cell_account,
        interval_history_account,
        replay_account,
        resolved_root,
        admission,
        recovery_close,
        replay_before,
        resolved_root_persist_decode,
    )?;
    Ok(AuthenticatedResolvedFailureMarketLifecycleV5 {
        resolution: postwrite,
        source_terminal,
        archive,
        link_release,
        source_result_close,
        recovery_close,
        family_terminal,
    })
}

#[allow(clippy::too_many_arguments)]
fn require_current_product_failure_join(
    market_root_account: &AccountInfo<'_>,
    root: &AuthenticatedMarketLifecycleRootV1<'_>,
    root_state: &clutch_product_series::MarketLifecycleRootV1,
    root_binding_id: ContentId,
    link: &AuthenticatedWritableFailureResolutionLinkV1,
    link_state: clutch_product_series::SeriesMarketLinkV1,
    registry: AuthenticatedRegistryCapabilityV3,
    bundle: &AuthenticatedProductArtifactV1<CompiledProductSeriesBundleV5>,
    failure_resolution: FailureMarketIntervalCellResolutionReceiptV2,
    certificate: clutch_product_series::QuantizedIntervalConsensusCertificateV1,
    certificate_id: clutch_product_series::QuantizedIntervalConsensusCertificateV1Id,
) -> Outcome<()> {
    let root_binding = root_state.binding();
    let link_binding = link_state.binding();
    let projection = registry.projection();
    let bundle_value = bundle.value();
    require_current_series_funding_graph_v5(
        registry.funding_terms_id(),
        bundle_value.funding_terms_id,
        link_binding.funding_terms_id,
        bundle_value.funding_quote_id,
        link_binding.funding_quote_id,
        bundle_value.attachment_plan_id,
        link_binding.attachment_plan_id,
    )?;
    require(
        root.is_writable()
            && link.owner_program() == root.owner_program()
            && link.link_account() != *market_root_account.key
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
            && registry.compiler_bundle_id() == bundle.semantic_id()
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
            && link_binding.compiler_output_id == bundle.semantic_id()
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
    liabilities: GeneralMarketLiabilityAuthorityV2,
    root_binding: clutch_product_series::MarketLifecycleBindingV1,
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
    let relation_market = liabilities.market_binding.base();
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

fn require_exact_slot10_preallocation(
    program_id: &Pubkey,
    resolution_account: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
    root: AuthenticatedMarketLifecycleRootV1<'_>,
    slot10: AuthenticatedMarketFoundationPreallocationV2,
) -> Outcome<()> {
    let binding = root.state().binding();
    let capital = root.state().capital();
    let expected_balance = slot10
        .principal_lamports()
        .checked_add(slot10.donation_lamports())
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let rent = read_rent(rent_sysvar)?;
    let (expected_resolution, _) =
        seeds::resolution_v5_pda(program_id, &binding.market_instance_id.bytes());
    require_slot10_preallocation_facts_v5(Slot10PreallocationFactsV5 {
        root_phase: root.state().phase(),
        slot_root_account: slot10.root_account(),
        root_account: root.account(),
        slot_root_authentication_id: slot10.root_authentication_id(),
        root_authentication_id: root.authentication_id(),
        slot_market_instance_id: slot10.market_instance_id().bytes(),
        root_market_instance_id: binding.market_instance_id.bytes(),
        slot_generation: slot10.generation(),
        root_generation: binding.generation,
        slot: slot10.slot(),
        slot_account: slot10.account(),
        resolution_account: *resolution_account.key,
        slot_foundation_schedule_id: slot10.foundation_schedule_id().bytes(),
        root_foundation_schedule_id: binding.foundation_schedule_id.bytes(),
        slot_foundation_account_graph_id: slot10.foundation_account_graph_id().bytes(),
        root_foundation_account_graph_id: binding.foundation_account_graph_id.bytes(),
        slot_foundation_transcript_id: slot10.foundation_transcript_id().bytes(),
        root_foundation_transcript_id: root.state().foundation().transcript_id.bytes(),
        slot_rent_refund_owner: slot10.rent_refund_owner().to_bytes(),
        root_rent_refund_owner: capital.rent_refund_owner.bytes(),
        slot_neutral_lamport_sink: slot10.neutral_lamport_sink().to_bytes(),
        root_neutral_lamport_sink: capital.neutral_lamport_sink.bytes(),
        slot_principal_lamports: slot10.principal_lamports(),
        minimum_rent_lamports: rent.minimum_balance(RESOLUTION_V5_BYTES)?,
        slot_observed_balance_lamports: slot10.observed_balance_lamports(),
        expected_balance_lamports: expected_balance,
        root_resolution_account_id: binding.resolution_account_id.bytes(),
        expected_resolution_account: expected_resolution,
        resolution_owner: *resolution_account.owner,
        resolution_is_writable: resolution_account.is_writable,
        resolution_is_signer: resolution_account.is_signer,
        resolution_is_executable: resolution_account.executable,
        resolution_data_len: resolution_account.data_len(),
        resolution_lamports: resolution_account.lamports(),
    })
}

#[allow(clippy::too_many_arguments)]
fn derive_finalization_evidence_id_v5(
    root: AuthenticatedMarketLifecycleRootV1<'_>,
    link: &AuthenticatedWritableFailureResolutionLinkV1,
    registry: AuthenticatedRegistryCapabilityV3,
    bundle: &AuthenticatedProductArtifactV1<CompiledProductSeriesBundleV5>,
    slot10: AuthenticatedMarketFoundationPreallocationV2,
    liabilities: GeneralMarketLiabilityAuthorityV2,
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
    let link_state_id = link
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let weights_bytes = encode_weights(weights);
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            FAILURE_MARKET_RESOLUTION_FINALIZATION_EVIDENCE_DOMAIN_V5,
            &root_binding_id.bytes(),
            root.account().as_ref(),
            &root.authentication_id().bytes(),
            link.link_account().as_ref(),
            &link.authentication_id().bytes(),
            &link.id().bytes(),
            &link_state_id.bytes(),
            registry.series_registry_account().as_ref(),
            registry.program_account().as_ref(),
            registry.programdata_account().as_ref(),
            registry.release_artifact_account().as_ref(),
            registry.profile_artifact_account().as_ref(),
            &registry.registry_release_id().bytes(),
            &registry.capability_profile_id().bytes(),
            bundle.account().as_ref(),
            &bundle.semantic_id().bytes(),
            &slot10.id().bytes(),
            &slot10.foundation_schedule_id().bytes(),
            &slot10.foundation_account_graph_id().bytes(),
            &slot10.foundation_transcript_id().bytes(),
            &slot10.principal_lamports().to_le_bytes(),
            &slot10.donation_lamports().to_le_bytes(),
            slot10.rent_refund_owner().as_ref(),
            slot10.neutral_lamport_sink().as_ref(),
            &liabilities.receipt_id.bytes(),
            &liabilities.hoard_semantic_id.bytes(),
            &expected_hoard_after_id.bytes(),
            &liabilities.claim_ledger_semantic_id.bytes(),
            &expected_claim_ledger_after_id.bytes(),
            &failure_resolution.id().bytes(),
            &failure_resolution.failure_policy_binding_id().bytes(),
            &failure.cell_before.bytes(),
            &failure.cell_after.bytes(),
            &failure.session_binding_id.bytes(),
            &failure.source_handoff_id.bytes(),
            &failure.terminal_work_id.bytes(),
            &certificate_id.bytes(),
            &failure.last_runtime_work_receipt_id.bytes(),
            &failure.completed_work_calls.to_le_bytes(),
            &failure.exact_reward_lamports.to_le_bytes(),
            resolution_account.as_ref(),
            &[outcome_count],
            &denominator.to_le_bytes(),
            &weights_bytes,
            &[RESOLVED_DISPOSITION_BYTE_V2],
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(id)
}

#[allow(clippy::too_many_arguments)]
fn write_collateral_activation_postimages_v5<'a>(
    program_id: &Pubkey,
    system_program: &AccountInfo<'a>,
    resolution_account: &AccountInfo<'a>,
    hoard_account: &AccountInfo<'a>,
    claim_ledger_account: &AccountInfo<'a>,
    market_instance_id: [u8; 32],
    resolution_bump: u8,
    resolution: ResolutionV5,
    hoard_after: HoardV2,
    claim_ledger_after: ClaimLedgerV3,
    expected_resolution_balance: u64,
) -> Outcome<()> {
    let hoard_lamports_before = hoard_account.lamports();
    let claim_ledger_lamports_before = claim_ledger_account.lamports();
    let bump_seed = [resolution_bump];
    let signer_seeds: [&[u8]; 3] = [seeds::SEED_RESOLUTION_V5, &market_instance_id, &bump_seed];
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(RESOLUTION_V5_BYTES),
        vec![AccountMeta::new(*resolution_account.key, true)],
    );
    invoke_signed(
        &allocate,
        &[resolution_account.clone(), system_program.clone()],
        &[&signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*resolution_account.key, true)],
    );
    invoke_signed(
        &assign,
        &[resolution_account.clone(), system_program.clone()],
        &[&signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        resolution_account.owner == program_id
            && resolution_account.data_len() == RESOLUTION_V5_BYTES
            && resolution_account.lamports() == expected_resolution_balance,
        ClutchError::AccountCreationFailed,
    )?;
    {
        let mut data = resolution_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        require(
            data.iter().all(|byte| *byte == 0),
            ClutchError::AlreadyInitialized,
        )?;
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
        resolution_account.lamports() == expected_resolution_balance
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

fn require_exact_cached_account_v5<T: Eq>(
    live_account: Pubkey,
    cached_account: Pubkey,
    live_authentication_id: ContentId,
    cached_authentication_id: ContentId,
    live_value: &T,
    cached_value: &T,
) -> Outcome<()> {
    require(
        live_account == cached_account
            && live_authentication_id == cached_authentication_id
            && live_value == cached_value,
        ClutchError::MismatchedState,
    )
}

fn require_current_series_funding_graph_v5(
    registry_funding_terms_id: SeriesFundingTermsV2Id,
    bundle_funding_terms_id: SeriesFundingTermsV2Id,
    link_funding_terms_id: SeriesFundingTermsV2Id,
    bundle_funding_quote_id: SeriesFundingQuoteV4Id,
    link_funding_quote_id: SeriesFundingQuoteV4Id,
    bundle_attachment_plan_id: SeriesAttachmentPlanV4Id,
    link_attachment_plan_id: ContentId,
) -> Outcome<()> {
    require(
        registry_funding_terms_id == bundle_funding_terms_id
            && registry_funding_terms_id == link_funding_terms_id
            && bundle_funding_quote_id == link_funding_quote_id
            && bundle_attachment_plan_id.content_id() == link_attachment_plan_id,
        ClutchError::MismatchedState,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Slot10PreallocationFactsV5 {
    root_phase: MarketLifecyclePhaseV1,
    slot_root_account: Pubkey,
    root_account: Pubkey,
    slot_root_authentication_id: ContentId,
    root_authentication_id: ContentId,
    slot_market_instance_id: [u8; 32],
    root_market_instance_id: [u8; 32],
    slot_generation: u64,
    root_generation: u64,
    slot: MarketFoundationSlotV2,
    slot_account: Pubkey,
    resolution_account: Pubkey,
    slot_foundation_schedule_id: [u8; 32],
    root_foundation_schedule_id: [u8; 32],
    slot_foundation_account_graph_id: [u8; 32],
    root_foundation_account_graph_id: [u8; 32],
    slot_foundation_transcript_id: [u8; 32],
    root_foundation_transcript_id: [u8; 32],
    slot_rent_refund_owner: [u8; 32],
    root_rent_refund_owner: [u8; 32],
    slot_neutral_lamport_sink: [u8; 32],
    root_neutral_lamport_sink: [u8; 32],
    slot_principal_lamports: u64,
    minimum_rent_lamports: u64,
    slot_observed_balance_lamports: u64,
    expected_balance_lamports: u64,
    root_resolution_account_id: [u8; 32],
    expected_resolution_account: Pubkey,
    resolution_owner: Pubkey,
    resolution_is_writable: bool,
    resolution_is_signer: bool,
    resolution_is_executable: bool,
    resolution_data_len: usize,
    resolution_lamports: u64,
}

fn require_slot10_preallocation_facts_v5(facts: Slot10PreallocationFactsV5) -> Outcome<()> {
    require(
        facts.root_phase == MarketLifecyclePhaseV1::Active
            && facts.slot_root_account == facts.root_account
            && facts.slot_root_authentication_id == facts.root_authentication_id
            && facts.slot_market_instance_id == facts.root_market_instance_id
            && facts.slot_generation == facts.root_generation
            && facts.slot == MarketFoundationSlotV2::ResolutionV5
            && facts.slot_account == facts.resolution_account
            && facts.slot_foundation_schedule_id == facts.root_foundation_schedule_id
            && facts.slot_foundation_account_graph_id == facts.root_foundation_account_graph_id
            && facts.slot_foundation_transcript_id == facts.root_foundation_transcript_id
            && facts.slot_rent_refund_owner == facts.root_rent_refund_owner
            && facts.slot_neutral_lamport_sink == facts.root_neutral_lamport_sink
            && facts.slot_principal_lamports == facts.minimum_rent_lamports
            && facts.slot_observed_balance_lamports == facts.expected_balance_lamports
            && facts.root_resolution_account_id == facts.resolution_account.to_bytes()
            && facts.expected_resolution_account == facts.resolution_account
            && facts.resolution_owner.to_bytes() == SYSTEM_PROGRAM_ID
            && facts.resolution_is_writable
            && !facts.resolution_is_signer
            && !facts.resolution_is_executable
            && facts.resolution_data_len == 0
            && facts.resolution_lamports == facts.expected_balance_lamports,
        ClutchError::MismatchedState,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionAuthorityFactsV5 {
    root_phase: MarketLifecyclePhaseV1,
    root_resolution_semantic_id: ContentId,
    root_resolution_data_id: ContentId,
    root_resolution_activation_receipt_id: ContentId,
    link_phase: SeriesMarketLinkPhaseV1,
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
        facts.root_phase == MarketLifecyclePhaseV1::Active
            && facts.root_resolution_semantic_id == ContentId::ZERO
            && facts.root_resolution_data_id == ContentId::ZERO
            && facts.root_resolution_activation_receipt_id == ContentId::ZERO
            && facts.link_phase == SeriesMarketLinkPhaseV1::Active
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

fn encode_weights(weights: &[u64; clutch_retirement::MAX_OUTCOMES]) -> [u8; 128] {
    let mut output = [0u8; 128];
    let mut index = 0usize;
    while index < clutch_retirement::MAX_OUTCOMES {
        let start = index * 8;
        output[start..start + 8].copy_from_slice(&weights[index].to_le_bytes());
        index += 1;
    }
    output
}

fn require_live_content_id(id: ContentId) -> Outcome<()> {
    require(id != ContentId::ZERO, ClutchError::MismatchedState)
}

#[cfg(test)]
mod adversarial_tests {
    use super::*;

    fn content(byte: u8) -> ContentId {
        ContentId::from_bytes([byte; 32])
    }

    fn key(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn funding_graph() -> (
        SeriesFundingTermsV2Id,
        SeriesFundingTermsV2Id,
        SeriesFundingTermsV2Id,
        SeriesFundingQuoteV4Id,
        SeriesFundingQuoteV4Id,
        SeriesAttachmentPlanV4Id,
        ContentId,
    ) {
        (
            SeriesFundingTermsV2Id::from_bytes([1; 32]),
            SeriesFundingTermsV2Id::from_bytes([1; 32]),
            SeriesFundingTermsV2Id::from_bytes([1; 32]),
            SeriesFundingQuoteV4Id::from_bytes([2; 32]),
            SeriesFundingQuoteV4Id::from_bytes([2; 32]),
            SeriesAttachmentPlanV4Id::from_bytes([3; 32]),
            content(3),
        )
    }

    fn slot10_facts() -> Slot10PreallocationFactsV5 {
        Slot10PreallocationFactsV5 {
            root_phase: MarketLifecyclePhaseV1::Active,
            slot_root_account: key(1),
            root_account: key(1),
            slot_root_authentication_id: content(2),
            root_authentication_id: content(2),
            slot_market_instance_id: [3; 32],
            root_market_instance_id: [3; 32],
            slot_generation: 1,
            root_generation: 1,
            slot: MarketFoundationSlotV2::ResolutionV5,
            slot_account: key(4),
            resolution_account: key(4),
            slot_foundation_schedule_id: [5; 32],
            root_foundation_schedule_id: [5; 32],
            slot_foundation_account_graph_id: [6; 32],
            root_foundation_account_graph_id: [6; 32],
            slot_foundation_transcript_id: [7; 32],
            root_foundation_transcript_id: [7; 32],
            slot_rent_refund_owner: [8; 32],
            root_rent_refund_owner: [8; 32],
            slot_neutral_lamport_sink: [9; 32],
            root_neutral_lamport_sink: [9; 32],
            slot_principal_lamports: 304,
            minimum_rent_lamports: 304,
            slot_observed_balance_lamports: 309,
            expected_balance_lamports: 309,
            root_resolution_account_id: key(4).to_bytes(),
            expected_resolution_account: key(4),
            resolution_owner: Pubkey::new_from_array(SYSTEM_PROGRAM_ID),
            resolution_is_writable: true,
            resolution_is_signer: false,
            resolution_is_executable: false,
            resolution_data_len: 0,
            resolution_lamports: 309,
        }
    }

    fn resolution_authority() -> ResolutionAuthorityFactsV5 {
        ResolutionAuthorityFactsV5 {
            root_phase: MarketLifecyclePhaseV1::Active,
            root_resolution_semantic_id: ContentId::ZERO,
            root_resolution_data_id: ContentId::ZERO,
            root_resolution_activation_receipt_id: ContentId::ZERO,
            link_phase: SeriesMarketLinkPhaseV1::Active,
            active_failure_sessions: 1,
            link_session_binding_id: [1; 32],
            failure_session_binding_id: [1; 32],
            failure_market_instance_id: [2; 32],
            root_market_instance_id: [2; 32],
            failure_generation: 3,
            root_generation: 3,
            failure_policy_binding_id: [4; 32],
            root_failure_policy_binding_id: [4; 32],
            failure_product_certificate_id: [5; 32],
            product_certificate_id: [5; 32],
            certificate_source_occurrence_id: [6; 32],
            link_source_occurrence_id: [6; 32],
            payout_active_len: 2,
            root_outcome_count: 2,
            registry_neutral_lamport_sink: [7; 32],
            root_neutral_lamport_sink: [7; 32],
        }
    }

    fn source_final_join() -> SourceResolutionFinalJoinFactsV5 {
        SourceResolutionFinalJoinFactsV5 {
            postwrite_id: content(1),
            final_cell_authentication_id: content(13),
            final_cell_state_id: content(2),
            failure_cell_after_id: content(2),
            activation_failure_receipt_id: content(3),
            failure_receipt_id: content(3),
            activation_product_certificate_id: content(4),
            failure_product_certificate_id: content(4),
            activation_market_instance_id: [5; 32],
            failure_market_instance_id: [5; 32],
            source_market_instance_id: [5; 32],
            activation_generation: 6,
            failure_generation: 6,
            source_failure_policy_binding_id: [7; 32],
            failure_policy_binding_id: [7; 32],
            source_successful_handoff_id: [8; 32],
            failure_source_handoff_id: [8; 32],
            resolution_account_id: content(9),
            resolution_semantic_id: content(10),
            resolution_data_id: content(11),
            source_resolution_input_id: content(12),
            runtime_postwrite_id: content(14),
        }
    }

    #[test]
    fn every_resolution_role_alias_refuses() {
        let distinct = [
            key(1),
            key(2),
            key(3),
            key(4),
            key(5),
            key(6),
            key(7),
            key(8),
            key(9),
            key(10),
            key(11),
        ];
        assert!(require_distinct_resolution_role_keys_v5(distinct).is_ok());
        let mut left = 0usize;
        while left < distinct.len() {
            let mut right = left + 1;
            while right < distinct.len() {
                let mut aliased = distinct;
                aliased[right] = aliased[left];
                assert!(require_distinct_resolution_role_keys_v5(aliased).is_err());
                right += 1;
            }
            left += 1;
        }
    }

    #[test]
    fn source_terminal_accounts_cannot_alias_any_resolution_protocol_role() {
        let protocol = [
            key(1),
            key(2),
            key(3),
            key(4),
            key(5),
            key(6),
            key(7),
            key(8),
            key(9),
            key(10),
            key(11),
            key(12),
            key(13),
            key(14),
            key(15),
            key(16),
            key(17),
        ];
        let external = [key(18), key(19), key(20)];
        assert!(require_source_resolution_outer_aliases_v5(protocol, external).is_ok());
        let mut protocol_index = 0usize;
        while protocol_index < protocol.len() {
            let mut external_index = 0usize;
            while external_index < external.len() {
                let mut aliased = external;
                aliased[external_index] = protocol[protocol_index];
                assert!(require_source_resolution_outer_aliases_v5(protocol, aliased).is_err());
                external_index += 1;
            }
            protocol_index += 1;
        }
    }

    #[test]
    fn stale_root_and_link_snapshots_refuse_account_auth_or_body_substitution() {
        assert!(require_exact_cached_account_v5(
            key(1),
            key(1),
            content(2),
            content(2),
            &3_u64,
            &3_u64,
        )
        .is_ok());
        for _role in ["root", "link"] {
            assert!(require_exact_cached_account_v5(
                key(9),
                key(1),
                content(2),
                content(2),
                &3_u64,
                &3_u64,
            )
            .is_err());
            assert!(require_exact_cached_account_v5(
                key(1),
                key(1),
                content(9),
                content(2),
                &3_u64,
                &3_u64,
            )
            .is_err());
            assert!(require_exact_cached_account_v5(
                key(1),
                key(1),
                content(2),
                content(2),
                &9_u64,
                &3_u64,
            )
            .is_err());
        }
    }

    #[test]
    fn registry_bundle_and_link_funding_graph_cannot_be_spliced() {
        let valid = funding_graph();
        assert!(require_current_series_funding_graph_v5(
            valid.0, valid.1, valid.2, valid.3, valid.4, valid.5, valid.6,
        )
        .is_ok());
        assert!(require_current_series_funding_graph_v5(
            SeriesFundingTermsV2Id::from_bytes([9; 32]),
            valid.1,
            valid.2,
            valid.3,
            valid.4,
            valid.5,
            valid.6,
        )
        .is_err());
        assert!(require_current_series_funding_graph_v5(
            valid.0,
            SeriesFundingTermsV2Id::from_bytes([9; 32]),
            valid.2,
            valid.3,
            valid.4,
            valid.5,
            valid.6,
        )
        .is_err());
        assert!(require_current_series_funding_graph_v5(
            valid.0,
            valid.1,
            SeriesFundingTermsV2Id::from_bytes([9; 32]),
            valid.3,
            valid.4,
            valid.5,
            valid.6,
        )
        .is_err());
        assert!(require_current_series_funding_graph_v5(
            valid.0,
            valid.1,
            valid.2,
            SeriesFundingQuoteV4Id::from_bytes([9; 32]),
            valid.4,
            valid.5,
            valid.6,
        )
        .is_err());
        assert!(require_current_series_funding_graph_v5(
            valid.0,
            valid.1,
            valid.2,
            valid.3,
            valid.4,
            SeriesAttachmentPlanV4Id::from_bytes([9; 32]),
            valid.6,
        )
        .is_err());
    }

    #[test]
    fn slot10_refuses_reuse_wrong_role_rent_donation_and_recipients() {
        assert!(require_slot10_preallocation_facts_v5(slot10_facts()).is_ok());
        macro_rules! refuses {
            ($field:ident, $value:expr) => {{
                let mut facts = slot10_facts();
                facts.$field = $value;
                assert!(require_slot10_preallocation_facts_v5(facts).is_err());
            }};
        }
        refuses!(root_phase, MarketLifecyclePhaseV1::Retiring);
        refuses!(slot_root_account, key(99));
        refuses!(slot_market_instance_id, [99; 32]);
        refuses!(slot_generation, 2);
        refuses!(slot, MarketFoundationSlotV2::FailureIntervalWork);
        refuses!(slot_account, key(99));
        refuses!(slot_root_authentication_id, content(99));
        refuses!(slot_foundation_schedule_id, [99; 32]);
        refuses!(slot_foundation_account_graph_id, [99; 32]);
        refuses!(slot_foundation_transcript_id, [99; 32]);
        refuses!(slot_principal_lamports, 303);
        refuses!(slot_observed_balance_lamports, 310);
        refuses!(slot_rent_refund_owner, [99; 32]);
        refuses!(slot_neutral_lamport_sink, [99; 32]);
        refuses!(root_resolution_account_id, [99; 32]);
        refuses!(expected_resolution_account, key(99));
        refuses!(resolution_owner, key(99));
        refuses!(resolution_is_writable, false);
        refuses!(resolution_is_signer, true);
        refuses!(resolution_is_executable, true);
        refuses!(resolution_data_len, 304);
        refuses!(resolution_lamports, 310);
    }

    #[test]
    fn payout_certificate_session_and_once_only_substitutions_refuse() {
        assert!(require_resolution_authority_facts_v5(resolution_authority()).is_ok());
        macro_rules! refuses {
            ($field:ident, $value:expr) => {{
                let mut facts = resolution_authority();
                facts.$field = $value;
                assert!(require_resolution_authority_facts_v5(facts).is_err());
            }};
        }
        refuses!(root_phase, MarketLifecyclePhaseV1::Retiring);
        refuses!(root_resolution_semantic_id, content(9));
        refuses!(root_resolution_data_id, content(9));
        refuses!(root_resolution_activation_receipt_id, content(9));
        refuses!(link_phase, SeriesMarketLinkPhaseV1::Retiring);
        refuses!(active_failure_sessions, 0);
        refuses!(failure_session_binding_id, [9; 32]);
        refuses!(failure_market_instance_id, [9; 32]);
        refuses!(failure_generation, 9);
        refuses!(failure_policy_binding_id, [9; 32]);
        refuses!(failure_product_certificate_id, [9; 32]);
        refuses!(certificate_source_occurrence_id, [9; 32]);
        refuses!(payout_active_len, 3);
        refuses!(registry_neutral_lamport_sink, [9; 32]);
    }

    #[test]
    fn source_terminal_refuses_every_prewrite_or_cross_market_substitution() {
        assert!(require_source_resolution_final_join_v5(source_final_join()).is_ok());
        macro_rules! refuses {
            ($field:ident, $value:expr) => {{
                let mut facts = source_final_join();
                facts.$field = $value;
                assert!(require_source_resolution_final_join_v5(facts).is_err());
            }};
        }
        refuses!(postwrite_id, ContentId::ZERO);
        refuses!(final_cell_authentication_id, ContentId::ZERO);
        refuses!(final_cell_state_id, content(99));
        refuses!(activation_failure_receipt_id, content(99));
        refuses!(activation_product_certificate_id, content(99));
        refuses!(activation_market_instance_id, [99; 32]);
        refuses!(source_market_instance_id, [99; 32]);
        refuses!(activation_generation, 99);
        refuses!(source_failure_policy_binding_id, [99; 32]);
        refuses!(source_successful_handoff_id, [99; 32]);
        refuses!(resolution_account_id, ContentId::ZERO);
        refuses!(resolution_semantic_id, content(9));
        refuses!(resolution_data_id, content(10));
        refuses!(source_resolution_input_id, ContentId::ZERO);
        refuses!(runtime_postwrite_id, ContentId::ZERO);
    }

    #[test]
    fn hoard_and_claim_successors_cannot_substitute_authenticated_prestates() {
        let hoard = CollateralId::from_bytes([1; 32]);
        let claim = CollateralId::from_bytes([2; 32]);
        assert!(require_liability_prestate_join_v5(true, true, hoard, hoard, claim, claim).is_ok());
        assert!(
            require_liability_prestate_join_v5(false, true, hoard, hoard, claim, claim).is_err()
        );
        assert!(
            require_liability_prestate_join_v5(true, false, hoard, hoard, claim, claim).is_err()
        );
        assert!(require_liability_prestate_join_v5(
            true,
            true,
            CollateralId::from_bytes([9; 32]),
            hoard,
            claim,
            claim,
        )
        .is_err());
        assert!(require_liability_prestate_join_v5(
            true,
            true,
            hoard,
            hoard,
            CollateralId::from_bytes([9; 32]),
            claim,
        )
        .is_err());
    }

    #[test]
    fn product_cell_and_market_runtime_writes_remain_in_one_atomic_resolution_entrypoint() {
        let source = include_str!("failure_market_resolution_v5.rs");
        let outer = source
            .split("fn resolve_failure_market_interval_v5")
            .nth(1)
            .expect("private resolution inner");
        let activation = outer
            .find("let activation = activate_failure_market_resolution_v5")
            .expect("Product/Collateral activation stage");
        let final_cell = outer
            .find("let interval_after = write_failure_market_interval_resolution_plan_v2")
            .expect("final Failure cell stage");
        let runtime_write = outer
            .find("let runtime_postwrite = write_failure_market_runtime_session_plan_v1")
            .expect("shared Failure runtime stage");
        let success = outer
            .find("Ok((postwrite, interval_after, runtime_postwrite))")
            .expect("success");
        assert!(activation < final_cell && final_cell < runtime_write && runtime_write < success);
        let forbidden_partial = concat!("pub(crate) fn activate_", "failure_market_resolution_v5");
        assert!(!source.contains(forbidden_partial));
    }

    #[test]
    fn sole_crate_visible_resolution_finishes_every_required_terminal_stage() {
        let source = include_str!("failure_market_resolution_v5.rs");
        assert_eq!(
            source
                .matches("pub(crate) fn resolve_failure_market_interval_and_source_v5")
                .count(),
            1
        );
        assert!(!source.contains("pub(crate) fn resolve_failure_market_interval_v5"));
        let outer = source
            .split("pub(crate) fn resolve_failure_market_interval_and_source_v5")
            .nth(1)
            .unwrap();
        let failure = outer
            .find("resolve_failure_market_interval_v5")
            .expect("complete Product/Collateral/Failure stage");
        let source_terminal = outer
            .find("compose_source_resolution_terminal_v1")
            .expect("Source terminal policy and liveness stage");
        let archive_plan = outer
            .find("plan_resolved_failure_market_archive_v5")
            .expect("resolved append/reset plan");
        let archive = outer
            .find("archive_failure_market_interval_session_v2")
            .expect("archive and Product link release");
        let source_close = outer
            .find("close_successful_source_statistic_result_v1")
            .expect("physical Source result close");
        let recovery_close = outer
            .find("close_failure_market_recovery_v2")
            .expect("single-custody Recovery close");
        let family_terminal = outer
            .find("persist_resolved_failure_market_family_v2")
            .expect("durable Failure-family seal");
        let success = outer
            .find("Ok(AuthenticatedResolvedFailureMarketLifecycleV5")
            .expect("sole successful return");
        assert!(failure < source_terminal);
        assert!(source_terminal < archive_plan && archive_plan < archive);
        assert!(archive < source_close && source_close < recovery_close);
        assert!(recovery_close < family_terminal && family_terminal < success);
        assert!(outer.contains("authenticate_writable_failure_resolution_link_v1"));
        assert!(outer.contains("authenticate_series_market_link_v1"));
        assert!(outer.contains("resolved_root.authentication_id()"));
    }

    #[test]
    fn late_terminal_refusal_is_inside_the_single_svm_rollback_boundary() {
        let source = include_str!("failure_market_resolution_v5.rs");
        let outer = source
            .split("pub(crate) fn resolve_failure_market_interval_and_source_v5")
            .nth(1)
            .expect("sole terminal outer");
        for stage in [
            "resolve_failure_market_interval_v5",
            "compose_source_resolution_terminal_v1",
            "archive_failure_market_interval_session_v2",
            "close_successful_source_statistic_result_v1",
            "close_failure_market_recovery_v2",
            "persist_resolved_failure_market_family_v2",
        ] {
            assert!(outer.contains(stage), "missing atomic stage {stage}");
        }
        assert!(!source.contains("pub(crate) fn resolve_failure_market_interval_v5"));
        assert!(!source.contains("pub(crate) fn activate_failure_market_resolution_v5"));
    }

    #[test]
    fn resolved_archive_plan_accepts_only_the_final_cell_and_source_terminal() {
        let source = include_str!("failure_market_resolution_v5.rs");
        let planner = source
            .split("fn plan_resolved_failure_market_archive_v5")
            .nth(1)
            .and_then(|value| {
                value.split("/// Stable byte committed for the only disposition")
                    .next()
            })
            .expect("resolved archive planner");
        for predicate in [
            "resolution.cell_authentication_after() == interval.cell_authentication_id()",
            "resolution.cell_state_after()",
            "cell.disposition() == FailureMarketIntervalCellDispositionV2::Resolved",
            "terminal.disposition == FailureMarketIntervalTerminalDispositionV2::Resolved",
            "terminal.session_terminal_receipt_id.bytes()",
            "failure_resolution.id().bytes()",
            "source_terminal.id() != ContentId::ZERO",
            "source_resolution_postwrite_id == resolution.id()",
            "plan_append_failure_market_interval_history_v2",
            "plan_reset_failure_market_interval_cell_v2",
            "reset.append_receipt_id() == append.id()",
        ] {
            assert!(planner.contains(predicate), "missing archive guard {predicate}");
        }
    }
}
