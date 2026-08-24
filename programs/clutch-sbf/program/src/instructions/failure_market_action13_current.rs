// SPDX-License-Identifier: AGPL-3.0-or-later
//! Sole current Recovery78/action13 deterministic-exhaustion owner.
//!
//! Action13 accepts only the active RootV3/LinkV3/FundingV5 graph. It derives
//! the immutable Failure quote and interval funding from their complete wire
//! preimages, hostile-reopens every physical semantic owner, and then invokes
//! one atomic exhausted-cell -> history -> LinkV3 release -> runtime writer.
//! Recovery custody remains read-only and no Product terminal receipt is
//! created or inferred by this finite-session archive path.

use std::boxed::Box;

use crate::accounts::{require, require_distinct, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::failure_market_admission::authenticate_failure_market_root_v3;
use crate::instructions::failure_market_dispatch_v2::{
    account_for_role_v2, FailureMarketAccountRoleV2 as Role, FailureMarketActionPayloadV2,
};
use crate::instructions::failure_market_interval_v2::{
    authenticate_failure_market_recovery_quote_v3,
    plan_failure_market_exhausted_archive_v2, plan_failure_market_interval_exhaustion_v2,
    reopen_failure_market_interval_accounts_v2, write_failure_market_interval_archive_v3,
    write_failure_market_interval_exhaustion_plan_v2,
    AuthenticatedFailureMarketIntervalAccountsV2, FailureMarketIntervalFundingPreimageV2,
};
use crate::instructions::failure_market_runtime::{
    authenticate_failure_market_runtime_root_v1, write_failure_market_runtime_session_plan_v3,
    AuthenticatedFailureMarketRuntimeRootV1, AuthenticatedFailureMarketRuntimeSessionWriteV3,
    FailureMarketRuntimeSessionWriteFactsV3,
};
use crate::instructions::product_artifact::authenticate_product_artifact_v1;
use crate::instructions::product_market_lifecycle_v3_current::{
    authenticate_market_lifecycle_root_v3, authenticate_series_market_link_v3,
    AuthenticatedMarketLifecycleRootV3, AuthenticatedSeriesMarketLinkV3,
};
use crate::instructions::product_failure_link_v3_current::{
    authenticate_writable_failure_exhausted_link_v4, release_series_market_link_failure_v4,
    FailureSessionReleaseDispositionV4,
};
use crate::instructions::product_series_current::{
    authenticate_registry_capability_v5, authenticate_series_funding_account_v5,
    authenticate_series_registry_account_v4,
};
use clutch_product_series::{
    CompiledProductSeriesBundleV7, ContentId, MarketLifecyclePhaseV3, SeriesFundingPhaseV5,
    SeriesFundingQuoteV6, SeriesMarketLinkPhaseV3,
};
use clutch_failure_policy_runtime::market_runtime_v1::{
    plan_close_exhausted_failure_market_session_v3,
    AuthenticatedFailureMarketSessionExhaustionCloseV3,
    FailureMarketSessionExhaustionCloseFactsV3,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV3, SeriesMarketLinkAccountV3,
};
use clutch_solana_layout::registry::RecoveryAction;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CurrentFailureExhaustionCloseAuthorityV3 {
    expected: FailureMarketSessionExhaustionCloseFactsV3,
}

impl AuthenticatedFailureMarketSessionExhaustionCloseV3
    for CurrentFailureExhaustionCloseAuthorityV3
{
    fn authenticate_failure_market_session_exhaustion_close_v3(
        &self,
        expected: FailureMarketSessionExhaustionCloseFactsV3,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if expected.runtime_before == self.expected.runtime_before
            && expected.series_link_before == self.expected.series_link_before
            && expected.series_link_after == self.expected.series_link_after
            && expected.active_session_state_id == self.expected.active_session_state_id
            && expected.exhausted_session_state_id == self.expected.exhausted_session_state_id
            && expected.idle_session_state_id == self.expected.idle_session_state_id
            && expected.exhaustion_receipt_id == self.expected.exhaustion_receipt_id
            && expected.previous_session_history == self.expected.previous_session_history
            && expected.resulting_session_history == self.expected.resulting_session_history
            && expected.history_append_receipt_id == self.expected.history_append_receipt_id
            && expected.history_before == self.expected.history_before
            && expected.history_after == self.expected.history_after
            && expected.completed_session_count == self.expected.completed_session_count
            && expected.transition_receipt_id.bytes() != [0; 32]
        {
            Ok(())
        } else {
            Err(clutch_failure_policy_runtime::Error::BindingMismatch)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CurrentFailureExhaustionRuntimeWriteV3 {
    expected: FailureMarketRuntimeSessionWriteFactsV3,
    archive_idle_state: ContentId,
    runtime_idle_state: ContentId,
    release_link_after: ContentId,
    runtime_link_after: ContentId,
    release_terminal_receipt_id: ContentId,
    runtime_terminal_receipt_id: ContentId,
}

impl AuthenticatedFailureMarketRuntimeSessionWriteV3
    for CurrentFailureExhaustionRuntimeWriteV3
{
    fn authenticate_failure_market_runtime_session_write_v3(
        &self,
        expected: FailureMarketRuntimeSessionWriteFactsV3,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if expected == self.expected
            && self.archive_idle_state == self.runtime_idle_state
            && self.release_link_after == self.runtime_link_after
            && self.release_terminal_receipt_id == self.runtime_terminal_receipt_id
        {
            Ok(())
        } else {
            Err(clutch_failure_policy_runtime::Error::BindingMismatch)
        }
    }
}

/// Atomically persist one deterministic exhaustion, append/reset the reusable
/// Failure pair, release LinkV3, and commit the sole runtime transition last.
#[allow(clippy::too_many_arguments)]
fn compose_current_failure_exhaustion_archive_v3<'a>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'a>,
    link_account: &AccountInfo<'a>,
    admission_account: &AccountInfo<'a>,
    runtime_account: &AccountInfo<'a>,
    cell_account: &AccountInfo<'a>,
    history_account: &AccountInfo<'a>,
    liveness_policy: &AccountInfo<'a>,
    recovery: &AccountInfo<'a>,
    root: AuthenticatedMarketLifecycleRootV3<'_>,
    link: AuthenticatedSeriesMarketLinkV3<'_>,
    admission: crate::instructions::failure_market_admission::AuthenticatedFailureMarketRootV3,
    runtime: AuthenticatedFailureMarketRuntimeRootV1,
    interval: AuthenticatedFailureMarketIntervalAccountsV2,
    root_rebound: &mut MarketLifecycleRootAccountV3,
    release_link_rebound: &mut SeriesMarketLinkAccountV3,
    released_link_rebound: &mut SeriesMarketLinkAccountV3,
) -> Outcome<()> {
    let link_binding = *link.binding();
    let release_preauthorization = authenticate_writable_failure_exhausted_link_v4(
        program_id,
        root_account,
        root,
        link_account,
        root_rebound,
        release_link_rebound,
    )?;
    require(
        release_preauthorization.disposition()
            == FailureSessionReleaseDispositionV4::Exhausted
            && release_preauthorization.link_authentication_id() == link.authentication_id()
            && release_preauthorization.link_semantic_id() == link.semantic_id()
            && release_preauthorization.session_binding_id()
                == link.state().failure_session_transcript_id(),
        ClutchError::MismatchedState,
    )?;

    let exhaustion_plan = plan_failure_market_interval_exhaustion_v2(
        program_id,
        liveness_policy,
        recovery,
        admission,
        interval,
    )?;
    let exhaustion = exhaustion_plan.receipt();
    let exhausted_interval = write_failure_market_interval_exhaustion_plan_v2(
        program_id,
        cell_account,
        history_account,
        interval,
        exhaustion_plan,
    )?;
    let archive_plan =
        plan_failure_market_exhausted_archive_v2(admission, exhausted_interval, exhaustion)?;
    let append = archive_plan.append();
    let predicted_link_after = link
        .state()
        .release_failure_session(ContentId::from_bytes(exhaustion.id().bytes()))
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let expected_close = FailureMarketSessionExhaustionCloseFactsV3 {
        runtime_before: runtime.state_commitment(),
        series_link_before: link.semantic_id(),
        series_link_after: predicted_link_after
            .semantic_id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        active_session_state_id: runtime.state().session_state_commitment(),
        exhausted_session_state_id: exhaustion.facts().cell_after,
        idle_session_state_id: append.idle_state_commitment(),
        exhaustion_receipt_id: exhaustion.id(),
        previous_session_history: append.previous_root(),
        resulting_session_history: append.resulting_root(),
        history_append_receipt_id: append.id(),
        history_before: append.history_before(),
        history_after: append.history_after(),
        completed_session_count: append.completed_session_count(),
        transition_receipt_id:
            clutch_failure_policy_runtime::market_runtime_v1::FailureMarketSessionTransitionReceiptIdV3::from_bytes([0; 32]),
    };
    let close_authority = CurrentFailureExhaustionCloseAuthorityV3 {
        expected: expected_close,
    };
    let runtime_plan = plan_close_exhausted_failure_market_session_v3(
        &close_authority,
        runtime.state(),
        admission.state(),
        *link.state(),
        exhaustion,
        append,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let archive = write_failure_market_interval_archive_v3(
        program_id,
        cell_account,
        history_account,
        exhausted_interval,
        archive_plan.history_plan(),
        append,
        archive_plan.cell_plan(),
        archive_plan.reset(),
        link_binding.source_occurrence_id,
        None,
        None,
        release_preauthorization.id(),
        crate::instructions::product_series_current::FailureSessionReleaseDispositionV3::Exhausted,
    )?;
    let (released_link, release) = release_series_market_link_failure_v4(
        program_id,
        link_account,
        link,
        release_preauthorization,
        archive,
        released_link_rebound,
    )?;
    let runtime_link_after = runtime_plan
        .series_link_after()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        *released_link.state() == runtime_plan.series_link_after()
            && release.disposition() == FailureSessionReleaseDispositionV4::Exhausted
            && release.link_semantic_before() == runtime_plan.series_link_before().semantic_id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            && release.link_semantic_after() == runtime_link_after
            && release.archive_postwrite_id() == archive.id()
            && release.append_receipt_id().bytes() == archive.append().id().bytes()
            && release.reset_receipt_id().bytes() == archive.reset().id().bytes()
            && release.session_terminal_receipt_id().bytes() == exhaustion.id().bytes()
            && release.release_link_preauthorization_id()
                == archive.release_link_preauthorization_id(),
        ClutchError::MismatchedState,
    )?;
    let runtime_write_facts = FailureMarketRuntimeSessionWriteFactsV3 {
        runtime_before: runtime.state_commitment(),
        runtime_after: runtime_plan
            .resulting_runtime()
            .commitment()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        transition_receipt_id: runtime_plan.receipt_id(),
    };
    let runtime_postwrite = write_failure_market_runtime_session_plan_v3(
        program_id,
        admission_account,
        runtime_account,
        admission,
        runtime,
        runtime_plan,
        &CurrentFailureExhaustionRuntimeWriteV3 {
            expected: runtime_write_facts,
            archive_idle_state: ContentId::from_bytes(archive.accounts().cell_state_id().bytes()),
            runtime_idle_state: runtime_plan.resulting_runtime().session_state_commitment(),
            release_link_after: release.link_semantic_after().content_id(),
            runtime_link_after: runtime_link_after.content_id(),
            release_terminal_receipt_id: release.session_terminal_receipt_id(),
            runtime_terminal_receipt_id: runtime_plan
                .resulting_runtime()
                .interval_terminal_receipt_id(),
        },
    )?;
    require(
        runtime_postwrite.transition_receipt_id() == runtime_write_facts.transition_receipt_id
            && runtime_postwrite.root().state().completed_session_count()
                == archive.append().completed_session_count()
            && runtime_postwrite.root().state().session_history_commitment()
                == archive.append().resulting_root()
            && runtime_postwrite.root().state().source_product_release_binding_id()
                == ContentId::ZERO,
        ClutchError::MismatchedState,
    )
}

/// Fold one finite exhausted session and release its exact current Product pin.
#[allow(clippy::too_many_lines)]
pub(crate) fn process_archive_failure_market_session_v3(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: FailureMarketActionPayloadV2<'_>,
) -> Outcome<()> {
    let FailureMarketActionPayloadV2::Archive = payload
    else {
        return crate::instructions::failure_market_dispatch_v2::process_reserved_disabled(
            RecoveryAction::CloseIntervalConsensusWork,
        );
    };
    let action = RecoveryAction::CloseIntervalConsensusWork;
    require_distinct(accounts)?;
    let root_account = account_for_role_v2(action, accounts, Role::MarketLifecycleRoot)?;
    let link_account = account_for_role_v2(action, accounts, Role::SeriesMarketLink)?;
    let funding_account = account_for_role_v2(action, accounts, Role::SeriesFunding)?;
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

    let admission = authenticate_failure_market_root_v3(program_id, admission_account, false)?;
    let policy = admission.state().binding().facts();
    let mut root_decode = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let root = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        policy.market_instance_id,
        policy.generation,
        false,
        &mut root_decode,
    )?;
    let root_binding = *root.binding();
    require(
        root.state().phase() == MarketLifecyclePhaseV3::Active
            && root.state().resolution_semantic_id() == clutch_product_series::ContentId::ZERO
            && root.state().resolution_data_id() == clutch_product_series::ContentId::ZERO
            && root.state().resolution_activation_receipt_id()
                == clutch_product_series::ContentId::ZERO
            && root_binding.market_failure_policy_binding_id.bytes()
                == admission.state().binding().id().bytes()
            && root_binding.market_instance_id == policy.market_instance_id
            && root_binding.generation == policy.generation,
        ClutchError::MismatchedState,
    )?;

    let mut link_decode = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    let link_data = link_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    SeriesMarketLinkAccountV3::decode_into(&link_data, &mut link_decode)?;
    drop(link_data);
    let decoded_link = *link_decode.state.binding_ref();
    let link = authenticate_series_market_link_v3(
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
    let link_binding = *link.binding();
    require(
        link.state().phase() == SeriesMarketLinkPhaseV3::Active
            && link.state().active_failure_sessions() == 1
            && link.state().failure_sessions_started() != 0
            && link_binding.market_root_account_id.bytes() == root_account.key.to_bytes()
            && link_binding.market_binding_id == root.binding_id()
            && link_binding.market_instance_id == root_binding.market_instance_id
            && link_binding.generation == root_binding.generation,
        ClutchError::MismatchedState,
    )?;

    let registry_account = authenticate_series_registry_account_v4(
        program_id,
        registry_account,
        link_binding.series_plan_id,
        false,
    )?;
    let registry = authenticate_registry_capability_v5(
        program_id,
        registry_account,
        registry_program,
        registry_programdata,
        registry_release,
        capability_profile,
    )?;
    let funding = authenticate_series_funding_account_v5(
        program_id,
        funding_account,
        link_binding.series_plan_id,
        false,
    )?;
    let funding_state = funding.state();
    let bundle = authenticate_product_artifact_v1::<CompiledProductSeriesBundleV7>(
        program_id,
        compiler_bundle,
        link_binding.compiler_bundle_id.content_id(),
    )?;
    let quote = authenticate_product_artifact_v1::<SeriesFundingQuoteV6>(
        program_id,
        funding_quote,
        link_binding.funding_quote_id.content_id(),
    )?;
    require(
        funding.account().to_bytes() == link_binding.funding_state_account_id.bytes()
            && funding_state.series_plan_id == link_binding.series_plan_id
            && funding_state.funding_terms_id == link_binding.funding_terms_id
            && funding_state.funding_quote_id == link_binding.funding_quote_id
            && funding_state.attachment_plan_id == link_binding.attachment_plan_id
            && funding_state.compiler_bundle_id == link_binding.compiler_bundle_id
            && funding_state.phase != SeriesFundingPhaseV5::Pending
            && registry.series_plan_id() == link_binding.series_plan_id
            && registry.funding_terms_id() == link_binding.funding_terms_id
            && registry.compiler_bundle_id() == link_binding.compiler_bundle_id
            && registry.registry_release_id() == root_binding.registry_release_id
            && registry.capability_profile_id() == root_binding.capability_profile_id
            && bundle.semantic_id() == link_binding.compiler_bundle_id.content_id()
            && bundle.value().series_plan_id == link_binding.series_plan_id
            && bundle.value().funding_terms_id == link_binding.funding_terms_id
            && bundle.value().funding_quote_id == link_binding.funding_quote_id
            && bundle.value().attachment_plan_id == link_binding.attachment_plan_id
            && quote.semantic_id() == link_binding.funding_quote_id.content_id()
            && quote.value().failure_liveness_policy_id.bytes()
                == policy.liveness_policy_id.bytes()
            && quote.value().failure_recovery_quote_schedule_id.bytes()
                == policy.recovery_quote_schedule_id.bytes(),
        ClutchError::MismatchedState,
    )?;

    let recovery_quote = authenticate_failure_market_recovery_quote_v3(
        program_id,
        admission,
        &root,
        &registry,
        liveness_policy,
    )?;
    let interval_funding_preimage = admission.interval_funding_preimage();
    let interval_funding = FailureMarketIntervalFundingPreimageV2::decode(
        &interval_funding_preimage,
    )?;
    let interval = reopen_failure_market_interval_accounts_v2(
        program_id,
        cell_account,
        history_account,
        admission,
        recovery_quote.receipt(),
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

    let mut release_root = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let mut release_link = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    let mut released_link = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    compose_current_failure_exhaustion_archive_v3(
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
        &mut release_root,
        &mut release_link,
        &mut released_link,
    )?;
    Ok(())
}
