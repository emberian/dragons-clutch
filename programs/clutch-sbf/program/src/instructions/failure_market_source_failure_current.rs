//! Sole current Product/Source/Failure composition for a zero-payout attempt.
//!
//! The only admitted order is Product LinkV2 pin, Source physical terminal,
//! Failure terminal-cell write, history append and Idle reset, Product LinkV2
//! release, one-way Source post-release persistence, and shared Failure
//! runtime write.
//! Any refusal rolls every earlier mutation back under SVM instruction
//! atomicity. No Product work or Failure Recovery liveness call is present.

use crate::accounts::{require, require_distinct, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::failure_market_admission::AuthenticatedFailureMarketRootV2;
use crate::instructions::failure_market_interval_v2::{
    plan_failure_market_source_failure_archive_v2,
    plan_failure_market_source_failure_cell_v2,
    write_failure_market_interval_archive_v3,
    write_failure_market_interval_source_failure_plan_v2,
    AuthenticatedFailureMarketIntervalAccountsV2,
    AuthenticatedFailureMarketRecoveryQuoteV2,
    AuthenticatedFailureMarketSourceFailurePostwriteV2,
    FailureMarketIntervalArchivePostwriteV3,
};
use crate::instructions::failure_market_runtime::{
    write_failure_market_runtime_source_failure_plan_v3,
    AuthenticatedFailureMarketRuntimeRootV1,
    AuthenticatedFailureMarketRuntimeSourceFailurePostwriteV3,
    AuthenticatedFailureMarketRuntimeSourceFailureWriteV3,
    FailureMarketRuntimeSourceFailureWriteFactsV3,
};
use crate::instructions::product_failure_begin_current::AuthenticatedProductFailureBeginScheduleV2;
use crate::instructions::product_series_current::{
    authenticate_writable_failure_source_absent_link_v3,
    authenticate_writable_failure_source_refused_link_v3,
    pin_series_market_link_failure_v2,
    release_series_market_link_failure_v3,
    AuthenticatedMarketLifecycleRootV2,
    AuthenticatedSeriesFailureSessionBeginV3,
    AuthenticatedSeriesFailureSessionPinV2,
    AuthenticatedSeriesFailureSessionReleaseV3,
    AuthenticatedSeriesMarketLinkV2,
    FailureSessionReleaseDispositionV3,
};
use crate::instructions::source_failure_product_release_v1::{
    bind_persisted_source_failure_product_release_v2,
    bind_source_failure_product_release_v1,
    AuthenticatedPersistedSourceFailureProductReleaseV2,
};
use crate::instructions::source_failure_terminal_v1::{
    compose_source_failure_terminal_v1,
    AuthenticatedSourceFailureHandoffV1,
    AuthenticatedSourceFailureTerminalAuthorityV1,
    AuthenticatedSourceFailureTerminalPostwriteV1,
    SourceFailureTerminalAuthorityFactsV1,
};
use clutch_failure_policy_runtime::market_interval_cell_v2::FailureMarketIntervalCellSourceFailureReceiptV2;
use clutch_failure_policy_runtime::market_runtime_v1::{
    plan_archive_failure_market_source_failure_v3,
    AuthenticatedFailureMarketSourceFailureTransitionV3,
    FailureMarketSessionDescriptorV1,
    FailureMarketSessionScheduleIdV1,
    FailureMarketSourceFailureTransitionFactsV3,
};
use clutch_product_series::ContentId as ProductContentId;
use clutch_source_plane_v3::ContentId as SourceContentId;
use clutch_source_plane_v3_runtime::{
    AuthenticatedSourceRouteV1,
    FailurePolicySourceHandoffV1,
    SourceFailureKindV1,
    SourcePolicyHandoffJoinV1,
    SourceWorkScheduleBindingV1,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV2,
    SeriesMarketLinkAccountV2,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const SOURCE_ATTEMPT_BEGIN_PREAUTHORIZATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/failure-market-source-attempt-begin-preauthorization/v2";
const SOURCE_ATTEMPT_POSTPIN_AUTHORIZATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/failure-market-source-attempt-postpin-authorization/v2";
const SOURCE_ATTEMPT_COMPOSITE_POSTWRITE_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/failure-market-source-attempt-composite-postwrite/v3";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketSourceAttemptBeginPreauthorizationV2 {
    id: ProductContentId,
    root_account: Pubkey,
    root_authentication_id: ProductContentId,
    link_account: Pubkey,
    link_authentication_id: ProductContentId,
    series_plan_id: clutch_product_series::SeriesPlanV5Id,
    ordinal: u32,
    market_instance_id: clutch_product_series::MarketInstanceV2Id,
    generation: u64,
    source_occurrence_id: clutch_product_series::SourceOccurrenceV1Id,
}

impl AuthenticatedSeriesFailureSessionBeginV3
    for FailureMarketSourceAttemptBeginPreauthorizationV2
{
    fn authenticate_series_failure_session_begin_v3(
        &self,
        root_account: Pubkey,
        root_authentication_id: ProductContentId,
        link_account: Pubkey,
        link_authentication_id: ProductContentId,
        series_plan_id: clutch_product_series::SeriesPlanV5Id,
        ordinal: u32,
        market_instance_id: clutch_product_series::MarketInstanceV2Id,
        generation: u64,
        source_occurrence_id: clutch_product_series::SourceOccurrenceV1Id,
        begin_admission_receipt_id: ProductContentId,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedFailureMarketSourceAttemptPreauthorizationV2 {
    id: ProductContentId,
    begin: FailureMarketSourceAttemptBeginPreauthorizationV2,
    product_pin_id: ProductContentId,
    session_binding_id: ProductContentId,
    release_preauthorization_id: ProductContentId,
    source_facts: SourceFailureTerminalAuthorityFactsV1,
}

impl AuthenticatedSourceFailureTerminalAuthorityV1
    for AuthenticatedFailureMarketSourceAttemptPreauthorizationV2
{
    fn authenticate_source_failure_terminal_authority_v1(
        &self,
        expected: SourceFailureTerminalAuthorityFactsV1,
    ) -> Outcome<()> {
        require(
            expected == self.source_facts
                && expected.id() != SourceContentId::ZERO
                && self.id != ProductContentId::ZERO
                && self.begin.id != ProductContentId::ZERO
                && self.product_pin_id != ProductContentId::ZERO
                && self.session_binding_id != ProductContentId::ZERO
                && self.release_preauthorization_id != ProductContentId::ZERO,
            ClutchError::MismatchedState,
        )
    }
}

impl AuthenticatedFailureMarketSourceFailurePostwriteV2
    for AuthenticatedSourceFailureTerminalPostwriteV1
{
    fn source_terminal_postwrite_id(&self) -> Outcome<SourceContentId> {
        Ok(self.id())
    }

    fn authenticate_source_failure_attempt_terminal_v2(
        &self,
        handoff: FailurePolicySourceHandoffV1,
        join: SourcePolicyHandoffJoinV1,
    ) -> Outcome<()> {
        let facts = self.authority_facts();
        require(
            facts.source_handoff_id == handoff.id()
                && facts.source_handoff_join_id == join.id()
                && facts.source_failure_kind == handoff.kind()
                && facts.source_occurrence_id == handoff.occurrence().occurrence_record_id()
                && facts.result_or_absence_account == join.result_or_absence_account()
                && facts.work_receipt_account == join.work_receipt_account(),
            ClutchError::MismatchedState,
        )
    }

    fn authenticate_failure_market_source_failure_postwrite_v2(
        &self,
        receipt: FailureMarketIntervalCellSourceFailureReceiptV2,
    ) -> Outcome<()> {
        let facts = receipt.facts();
        require(
            facts.source_terminal_postwrite_id == self.id()
                && facts.source_kind == self.source_failure_kind()
                && facts.source_handoff_id == self.authority_facts().source_handoff_id
                && facts.source_join_id == self.authority_facts().source_handoff_join_id
                && facts.result_or_absence_account
                    == self.authority_facts().result_or_absence_account,
            ClutchError::MismatchedState,
        )
    }
}

struct FailureMarketSourceTransitionAuthorityV3<'a> {
    begin: FailureMarketSourceAttemptBeginPreauthorizationV2,
    pin: &'a AuthenticatedSeriesFailureSessionPinV2,
    release: &'a AuthenticatedSeriesFailureSessionReleaseV3,
    source_terminal: AuthenticatedSourceFailureTerminalPostwriteV1,
    source_product_release: &'a AuthenticatedPersistedSourceFailureProductReleaseV2,
    archive: FailureMarketIntervalArchivePostwriteV3,
}

impl AuthenticatedFailureMarketSourceFailureTransitionV3
    for FailureMarketSourceTransitionAuthorityV3<'_>
{
    fn authenticate_failure_market_source_failure_transition_v3(
        &self,
        expected: FailureMarketSourceFailureTransitionFactsV3,
    ) -> clutch_failure_policy_runtime::Result<()> {
        let valid = expected.series_link_before == self.pin.link_semantic_before()
            && expected.series_link_pinned == self.pin.link_semantic_after()
            && expected.series_link_after == self.release.link_semantic_after()
            && expected.begin_preauthorization_id == self.begin.id
            && expected.product_pin_receipt_id == self.pin.id()
            && expected.product_release_receipt_id == self.release.id()
            && expected.source_product_release_id.bytes()
                == self.source_product_release.id().bytes()
            && expected.session_binding_id == self.pin.session_binding_id()
            && expected.source_terminal_postwrite_id.bytes() == self.source_terminal.id().bytes()
            && expected.source_failure_receipt_id.bytes()
                == self.archive.append().session_terminal_receipt_id().bytes()
            && expected.terminal_cell_state_id == self.archive.reset().terminal_cell()
            && expected.idle_cell_state_id.bytes() == self.archive.reset().idle_cell().bytes()
            && expected.history_append_receipt_id == self.archive.append().id()
            && expected.history_before == self.archive.append().history_before()
            && expected.history_after == self.archive.append().history_after()
            && expected.previous_session_history == self.archive.append().previous_root()
            && expected.resulting_session_history == self.archive.append().resulting_root()
            && expected.completed_session_count == self.archive.append().completed_session_count()
            && expected.transition_receipt_id.bytes() != [0; 32];
        if valid {
            Ok(())
        } else {
            Err(clutch_failure_policy_runtime::Error::BindingMismatch)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketSourceRuntimeWriteAuthorityV3 {
    expected: FailureMarketRuntimeSourceFailureWriteFactsV3,
}

impl AuthenticatedFailureMarketRuntimeSourceFailureWriteV3
    for FailureMarketSourceRuntimeWriteAuthorityV3
{
    fn authenticate_failure_market_runtime_source_failure_write_v3(
        &self,
        expected: FailureMarketRuntimeSourceFailureWriteFactsV3,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if expected == self.expected {
            Ok(())
        } else {
            Err(clutch_failure_policy_runtime::Error::BindingMismatch)
        }
    }
}

#[derive(Debug)]
pub(crate) struct AuthenticatedFailureMarketSourceFailurePostwriteV3<'link> {
    id: ProductContentId,
    link: AuthenticatedSeriesMarketLinkV2<'link>,
    release: AuthenticatedSeriesFailureSessionReleaseV3,
    source_release: AuthenticatedPersistedSourceFailureProductReleaseV2,
    runtime: AuthenticatedFailureMarketRuntimeSourceFailurePostwriteV3,
}

impl AuthenticatedFailureMarketSourceFailurePostwriteV3<'_> {
    pub(crate) const fn id(&self) -> ProductContentId { self.id }
    pub(crate) const fn link(&self) -> &AuthenticatedSeriesMarketLinkV2<'_> { &self.link }
    pub(crate) const fn release(&self) -> &AuthenticatedSeriesFailureSessionReleaseV3 {
        &self.release
    }
    pub(crate) const fn source_release(
        &self,
    ) -> &AuthenticatedPersistedSourceFailureProductReleaseV2 {
        &self.source_release
    }
    pub(crate) const fn runtime(&self) -> AuthenticatedFailureMarketRuntimeSourceFailurePostwriteV3 {
        self.runtime
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn compose_failure_market_source_failure_attempt_v3<'root, 'link, 'released>(
    program_id: &Pubkey,
    market_root_account: &AccountInfo<'_>,
    series_link_account: &AccountInfo<'_>,
    admission_root_account: &AccountInfo<'_>,
    runtime_root_account: &AccountInfo<'_>,
    cell_account: &AccountInfo<'_>,
    history_account: &AccountInfo<'_>,
    root_before: AuthenticatedMarketLifecycleRootV2<'root>,
    link_before: AuthenticatedSeriesMarketLinkV2<'link>,
    admission: AuthenticatedFailureMarketRootV2,
    runtime_before: AuthenticatedFailureMarketRuntimeRootV1,
    interval_before: AuthenticatedFailureMarketIntervalAccountsV2,
    quote: AuthenticatedFailureMarketRecoveryQuoteV2,
    product_schedule: &AuthenticatedProductFailureBeginScheduleV2,
    source_route: AuthenticatedSourceRouteV1,
    source_schedule: SourceWorkScheduleBindingV1,
    source: AuthenticatedSourceFailureHandoffV1,
    result_or_absence_account: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    terminal_policy_account: &AccountInfo<'_>,
    terminal_receipt_account: &AccountInfo<'_>,
    source_liveness_policy: &AccountInfo<'_>,
    source_liveness_compartment: &AccountInfo<'_>,
    source_funding_custody: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
    pin_root_output: &mut MarketLifecycleRootAccountV2,
    pin_link_output: &mut SeriesMarketLinkAccountV2,
    release_root_output: &mut MarketLifecycleRootAccountV2,
    release_link_output: &mut SeriesMarketLinkAccountV2,
    released_link_output: &'released mut SeriesMarketLinkAccountV2,
) -> Outcome<AuthenticatedFailureMarketSourceFailurePostwriteV3<'released>> {
    require_distinct(&[
        market_root_account.clone(),
        series_link_account.clone(),
        admission_root_account.clone(),
        runtime_root_account.clone(),
        cell_account.clone(),
        history_account.clone(),
        result_or_absence_account.clone(),
        lineage_account.clone(),
        terminal_policy_account.clone(),
        terminal_receipt_account.clone(),
        source_liveness_policy.clone(),
        source_liveness_compartment.clone(),
        source_funding_custody.clone(),
        neutral_sink.clone(),
        system_program.clone(),
        rent_sysvar.clone(),
    ])?;
    let link_binding = link_before.state().binding();
    let root_binding = root_before.state().binding();
    let source_facts = source.authority_facts(
        source_route,
        source_schedule,
        source_key(terminal_policy_account.key),
        source_key(terminal_receipt_account.key),
        source_key(source_liveness_policy.key),
        source_key(source_liveness_compartment.key),
        source_key(source_funding_custody.key),
        source_key(source_funding_custody.key),
        source_key(neutral_sink.key),
    );
    let attempt_index = u8::try_from(runtime_before.state().completed_session_count())
        .map_err(|_| ClutchError::Arithmetic)?;
    require(
        !root_before.is_writable()
            && link_before.is_writable()
            && product_schedule.root_account() == root_before.account()
            && product_schedule.root_authentication_id() == root_before.authentication_id()
            && product_schedule.link_account() == link_before.account()
            && product_schedule.link_authentication_id() == link_before.authentication_id()
            && product_schedule.series_plan_id() == link_binding.series_plan_id
            && product_schedule.ordinal() == link_binding.ordinal
            && product_schedule.market_instance_id() == root_binding.market_instance_id
            && product_schedule.generation() == root_binding.generation
            && product_schedule.source_occurrence_id() == link_binding.source_occurrence_id
            && product_schedule.attempt_index() == attempt_index
            && product_schedule.source_repair_generation() == link_binding.source_repair_generation
            && product_schedule.failure_quote_receipt_id()
                == quote.attempt_authorization_id(
                    attempt_index,
                    link_binding.source_repair_generation,
                )?
            && source_facts.market_instance_id.bytes() == root_binding.market_instance_id.bytes()
            && source_facts.series_plan_id.bytes() == link_binding.series_plan_id.bytes()
            && source_facts.ordinal == link_binding.ordinal
            && source_facts.source_occurrence_id.bytes() == link_binding.source_occurrence_id.bytes()
            && source_facts.source_repair_generation == link_binding.source_repair_generation
            && source_facts.source_work_schedule_id == source_schedule.source_work_schedule_id()
            && interval_before.cell().phase()
                == clutch_failure_policy_runtime::market_interval_cell_v2::FailureMarketIntervalCellPhaseV2::Idle
            && interval_before.history().completed_session_count()
                == runtime_before.state().completed_session_count(),
        ClutchError::MismatchedState,
    )?;
    let admission_id = admission
        .state()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let begin_id = ProductContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SOURCE_ATTEMPT_BEGIN_PREAUTHORIZATION_DOMAIN_V2,
            program_id.as_ref(),
            market_root_account.key.as_ref(),
            &root_before.authentication_id().bytes(),
            series_link_account.key.as_ref(),
            &link_before.authentication_id().bytes(),
            admission_root_account.key.as_ref(),
            &admission_id.bytes(),
            runtime_root_account.key.as_ref(),
            &runtime_before.state_commitment().bytes(),
            cell_account.key.as_ref(),
            &interval_before.cell_authentication_id().bytes(),
            history_account.key.as_ref(),
            &interval_before.history_authentication_id().bytes(),
            &product_schedule.id().bytes(),
            &product_schedule.schedule_projection_id().bytes(),
            &source_facts.id().bytes(),
            &[attempt_index],
        ])
        .to_bytes(),
    );
    require(begin_id != ProductContentId::ZERO, ClutchError::MismatchedState)?;
    let begin = FailureMarketSourceAttemptBeginPreauthorizationV2 {
        id: begin_id,
        root_account: root_before.account(),
        root_authentication_id: root_before.authentication_id(),
        link_account: link_before.account(),
        link_authentication_id: link_before.authentication_id(),
        series_plan_id: link_binding.series_plan_id,
        ordinal: link_binding.ordinal,
        market_instance_id: link_binding.market_instance_id,
        generation: link_binding.generation,
        source_occurrence_id: link_binding.source_occurrence_id,
    };
    let (link_pinned, pin) = pin_series_market_link_failure_v2(
        program_id,
        market_root_account,
        root_before,
        series_link_account,
        link_before,
        begin.id,
        &begin,
        pin_root_output,
        pin_link_output,
    )?;
    let release_preauthorization = match source.source_failure_kind() {
        SourceFailureKindV1::PrimaryMaturityWithoutAcceptedResolution => {
            authenticate_writable_failure_source_absent_link_v3(
                program_id,
                market_root_account,
                root_before,
                series_link_account,
                release_root_output,
                release_link_output,
            )?
        }
        SourceFailureKindV1::SourceEvaluationRefused => {
            authenticate_writable_failure_source_refused_link_v3(
                program_id,
                market_root_account,
                root_before,
                series_link_account,
                release_root_output,
                release_link_output,
            )?
        }
    };
    let postpin_id = ProductContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SOURCE_ATTEMPT_POSTPIN_AUTHORIZATION_DOMAIN_V2,
            &begin.id.bytes(),
            &pin.id().bytes(),
            &pin.session_binding_id().bytes(),
            &release_preauthorization.id().bytes(),
            &source_facts.id().bytes(),
        ])
        .to_bytes(),
    );
    require(postpin_id != ProductContentId::ZERO, ClutchError::MismatchedState)?;
    let source_authority = AuthenticatedFailureMarketSourceAttemptPreauthorizationV2 {
        id: postpin_id,
        begin,
        product_pin_id: pin.id(),
        session_binding_id: pin.session_binding_id(),
        release_preauthorization_id: release_preauthorization.id(),
        source_facts,
    };
    let source_terminal = compose_source_failure_terminal_v1(
        program_id,
        source_route,
        source_schedule,
        source,
        &source_authority,
        result_or_absence_account,
        lineage_account,
        terminal_policy_account,
        terminal_receipt_account,
        source_liveness_policy,
        source_liveness_compartment,
        source_funding_custody,
        neutral_sink,
        system_program,
        rent_sysvar,
    )?;
    let (source_handoff, source_join) = source_handoff_and_join(source);
    let session_schedule_id = SourceContentId::from_bytes(
        product_schedule.schedule_projection_id().bytes(),
    );
    let (cell_plan, source_failure_receipt) = plan_failure_market_source_failure_cell_v2(
        admission,
        interval_before,
        SourceContentId::from_bytes(pin.session_binding_id().bytes()),
        session_schedule_id,
        source_handoff,
        source_join,
        &source_terminal,
    )?;
    let terminal_interval = write_failure_market_interval_source_failure_plan_v2(
        program_id,
        cell_account,
        history_account,
        interval_before,
        cell_plan,
        source_failure_receipt,
        &source_terminal,
    )?;
    let archive_plan = plan_failure_market_source_failure_archive_v2(
        admission,
        terminal_interval,
        source_failure_receipt,
    )?;
    let release_disposition = release_preauthorization.disposition();
    let archive = write_failure_market_interval_archive_v3(
        program_id,
        cell_account,
        history_account,
        terminal_interval,
        archive_plan.history_plan(),
        archive_plan.append(),
        archive_plan.cell_plan(),
        archive_plan.reset(),
        link_binding.source_occurrence_id,
        source_failure_receipt,
        source_terminal,
        release_preauthorization.id(),
        release_disposition,
    )?;
    let (link_released, release) = release_series_market_link_failure_v3(
        program_id,
        series_link_account,
        link_pinned,
        &release_preauthorization,
        &archive,
        released_link_output,
    )?;
    let source_product_release = bind_source_failure_product_release_v1(
        source_terminal,
        &release,
        &archive,
    )?;
    let persisted_source_product_release = bind_persisted_source_failure_product_release_v2(
        program_id,
        source_route,
        source_product_release,
        terminal_policy_account,
    )?;
    let session = FailureMarketSessionDescriptorV1 {
        series_plan_id: link_binding.series_plan_id,
        ordinal: link_binding.ordinal,
        source_occurrence_id: link_binding.source_occurrence_id,
        schedule_id: FailureMarketSessionScheduleIdV1::from_bytes(
            product_schedule.schedule_projection_id().bytes(),
        ),
        interval_funding_receipt_id: interval_before.funding().id(),
        session_state_commitment: ProductContentId::from_bytes(
            source_failure_receipt.cell_after().bytes(),
        ),
    };
    let transition_authority = FailureMarketSourceTransitionAuthorityV3 {
        begin,
        pin: &pin,
        release: &release,
        source_terminal,
        source_product_release: &persisted_source_product_release,
        archive,
    };
    let transition_plan = plan_archive_failure_market_source_failure_v3(
        &transition_authority,
        runtime_before.state(),
        admission.state(),
        *link_before.state(),
        begin.id,
        pin.id(),
        release.id(),
        ProductContentId::from_bytes(persisted_source_product_release.id().bytes()),
        session,
        interval_before.funding(),
        interval_before.history(),
        quote.receipt(),
        source_failure_receipt,
        archive.append(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        transition_plan.series_link_pinned() == *link_pinned.state()
            && transition_plan.series_link_after() == *link_released.state()
            && transition_plan.resulting_runtime().interval_terminal_receipt_id().bytes()
                == source_failure_receipt.id().bytes(),
        ClutchError::MismatchedState,
    )?;
    let runtime_write_facts = FailureMarketRuntimeSourceFailureWriteFactsV3 {
        runtime_before: runtime_before.state_commitment(),
        runtime_after: transition_plan
            .resulting_runtime()
            .commitment()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        transition_receipt_id: transition_plan.receipt_id(),
    };
    let runtime_authority = FailureMarketSourceRuntimeWriteAuthorityV3 {
        expected: runtime_write_facts,
    };
    let runtime = write_failure_market_runtime_source_failure_plan_v3(
        program_id,
        admission_root_account,
        runtime_root_account,
        admission,
        runtime_before,
        transition_plan,
        &runtime_authority,
    )?;
    let id = ProductContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SOURCE_ATTEMPT_COMPOSITE_POSTWRITE_DOMAIN_V3,
            &begin.id.bytes(),
            &pin.id().bytes(),
            &source_terminal.id().bytes(),
            &source_failure_receipt.id().bytes(),
            &archive.id().bytes(),
            &release.id().bytes(),
            &persisted_source_product_release.id().bytes(),
            &runtime.id().bytes(),
        ])
        .to_bytes(),
    );
    require(id != ProductContentId::ZERO, ClutchError::MismatchedState)?;
    Ok(AuthenticatedFailureMarketSourceFailurePostwriteV3 {
        id,
        link: link_released,
        release,
        source_release: persisted_source_product_release,
        runtime,
    })
}

fn source_handoff_and_join(
    source: AuthenticatedSourceFailureHandoffV1,
) -> (FailurePolicySourceHandoffV1, SourcePolicyHandoffJoinV1) {
    match source {
        AuthenticatedSourceFailureHandoffV1::Absence(value) => (value.handoff(), value.join()),
        AuthenticatedSourceFailureHandoffV1::Refused(value) => (value.handoff(), value.join()),
    }
}

fn source_key(key: &Pubkey) -> clutch_source_plane_v3_runtime::RuntimeKey {
    clutch_source_plane_v3_runtime::RuntimeKey::from_bytes(key.to_bytes())
}

#[cfg(test)]
mod adversarial_source_contract_tests {
    #[test]
    fn current_source_failure_order_is_single_and_rollback_atomic() {
        let source = include_str!("failure_market_source_failure_current.rs");
        let compose = source
            .split("pub(crate) fn compose_failure_market_source_failure_attempt_v3")
            .nth(1)
            .expect("sole current source-failure outer");
        let pin = compose.find("pin_series_market_link_failure_v2(").unwrap();
        let source_terminal = compose.find("compose_source_failure_terminal_v1(").unwrap();
        let cell = compose
            .find("write_failure_market_interval_source_failure_plan_v2(")
            .unwrap();
        let archive = compose.find("write_failure_market_interval_archive_v3(").unwrap();
        let release = compose.find("release_series_market_link_failure_v3(").unwrap();
        let source_release = compose.find("bind_source_failure_product_release_v1(").unwrap();
        let persisted_source_release = compose
            .find("bind_persisted_source_failure_product_release_v2(")
            .unwrap();
        let runtime = compose
            .find("write_failure_market_runtime_source_failure_plan_v3(")
            .unwrap();
        assert!(pin < source_terminal);
        assert!(source_terminal < cell);
        assert!(cell < archive);
        assert!(archive < release);
        assert!(release < source_release);
        assert!(source_release < persisted_source_release);
        assert!(persisted_source_release < runtime);
        assert!(!compose.contains("keeper_payment_lamports"));
        assert!(!compose.contains("plan_runtime_transition_v1"));
    }

    #[test]
    fn source_failure_dispositions_are_exhaustive_and_never_resolution() {
        let source = include_str!("failure_market_source_failure_current.rs");
        assert!(source.contains("authenticate_writable_failure_source_absent_link_v3"));
        assert!(source.contains("authenticate_writable_failure_source_refused_link_v3"));
        assert!(!source.contains("authenticate_writable_failure_resolution_link_v3"));
        assert!(!source.contains("authenticate_writable_failure_exhausted_link_v3"));
    }
}
