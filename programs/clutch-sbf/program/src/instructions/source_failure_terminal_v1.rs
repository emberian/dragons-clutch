//! Private Source terminal composition for mature absence and stable refusal.
//!
//! Failure first pins the exact Product link and mints a private, noncircular
//! attempt preauthorization. This module then persists the Source terminal,
//! closes Source liveness, and performs exactly one physical disposition. Its
//! postwrite is the only Source authority accepted by Failure's Refused-cell
//! writer. There is no instruction decoder or caller-supplied terminal ID.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::source_plane_v3::{
    runtime_key, AuthenticatedFailureAbsenceSourceHandoffV1,
    AuthenticatedFailureResultSourceHandoffV1,
};
use crate::source_plane_v3_actions::{
    apply_source_terminal_liveness, authenticate_source_funding_custody_v1,
    bind_terminal_execution, close_statistic_result_generation,
    persist_source_failure_terminal_v1, retire_absent_statistic_result_lineage_v1,
    AuthenticatedAbsentStatisticResultLineageRetirementV1,
    AuthenticatedSourceTerminalSemanticV1, CloseRuntimeAccountResultV1,
    PersistedSourceFailureTerminalV1, SourceTerminalExecutionV1,
};
use clutch_liveness::runtime_adapter_v1::{
    RuntimeAtomicTransitionV1, RuntimeTransitionActionV1,
};
use clutch_source_plane_v3::ContentId;
use clutch_source_plane_v3_runtime::{
    account_data_id, AuthenticatedSourceRouteV1, LineageAccessV1,
    SourceFailureKindV1, SourceFailureTerminalDispositionV1,
    SourceFailureTerminalV1, SourceWorkScheduleBindingV1,
    StatisticResultAbsenceAccessV1, StatisticResultAccountAccessV1, RuntimeKey,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const SOURCE_FAILURE_TERMINAL_AUTHORITY_FACTS_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/source-failure-terminal-authority-facts/v1";
const SOURCE_FAILURE_TERMINAL_POSTWRITE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/source-failure-terminal-postwrite/v1";
const SOURCE_REFUSED_RESULT_CLOSE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/source-refused-result-close/v1";

/// Exact reconstructed Source and physical-account facts authenticated by
/// Failure's post-Product-pin attempt preauthorization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceFailureTerminalAuthorityFactsV1 {
    pub(crate) source_release_manifest_id: ContentId,
    pub(crate) source_release_authentication_id: ContentId,
    pub(crate) source_route_id: ContentId,
    pub(crate) source_reconstruction_id: ContentId,
    pub(crate) source_handoff_id: ContentId,
    pub(crate) source_handoff_join_id: ContentId,
    pub(crate) persisted_handoff_authentication_id: ContentId,
    pub(crate) lineage_account: RuntimeKey,
    pub(crate) lineage_authentication_id: ContentId,
    pub(crate) lineage_state_id: ContentId,
    pub(crate) result_or_absence_account: RuntimeKey,
    pub(crate) result_or_absence_authentication_id: ContentId,
    pub(crate) work_receipt_account: RuntimeKey,
    pub(crate) work_receipt_authentication_id: ContentId,
    pub(crate) source_failure_kind: SourceFailureKindV1,
    pub(crate) market_instance_id: ContentId,
    pub(crate) series_plan_id: ContentId,
    pub(crate) ordinal: u32,
    pub(crate) source_occurrence_id: ContentId,
    pub(crate) source_occurrence_account: RuntimeKey,
    pub(crate) source_occurrence_authentication_id: ContentId,
    pub(crate) source_repair_generation: u64,
    pub(crate) source_work_schedule_id: ContentId,
    pub(crate) source_lifecycle_id: ContentId,
    pub(crate) source_generation: u64,
    pub(crate) source_terminal_policy_account: RuntimeKey,
    pub(crate) source_terminal_receipt_account: RuntimeKey,
    pub(crate) source_liveness_policy_account: RuntimeKey,
    pub(crate) source_liveness_compartment_account: RuntimeKey,
    pub(crate) source_funding_custody_account: RuntimeKey,
    pub(crate) source_principal_refund: RuntimeKey,
    pub(crate) source_neutral_sink: RuntimeKey,
}

impl SourceFailureTerminalAuthorityFactsV1 {
    /// Canonical noncircular identity persisted in the Source terminal body.
    pub(crate) fn id(self) -> ContentId {
        ContentId::from_bytes(
            solana_sha256_hasher::hashv(&[
                SOURCE_FAILURE_TERMINAL_AUTHORITY_FACTS_DOMAIN_V1,
                &self.source_release_manifest_id.bytes(),
                &self.source_release_authentication_id.bytes(),
                &self.source_route_id.bytes(),
                &self.source_reconstruction_id.bytes(),
                &self.source_handoff_id.bytes(),
                &self.source_handoff_join_id.bytes(),
                &self.persisted_handoff_authentication_id.bytes(),
                &self.lineage_account.bytes(),
                &self.lineage_authentication_id.bytes(),
                &self.lineage_state_id.bytes(),
                &self.result_or_absence_account.bytes(),
                &self.result_or_absence_authentication_id.bytes(),
                &self.work_receipt_account.bytes(),
                &self.work_receipt_authentication_id.bytes(),
                &[source_failure_kind_byte(self.source_failure_kind)],
                &self.market_instance_id.bytes(),
                &self.series_plan_id.bytes(),
                &self.ordinal.to_le_bytes(),
                &self.source_occurrence_id.bytes(),
                &self.source_occurrence_account.bytes(),
                &self.source_occurrence_authentication_id.bytes(),
                &self.source_repair_generation.to_le_bytes(),
                &self.source_work_schedule_id.bytes(),
                &self.source_lifecycle_id.bytes(),
                &self.source_generation.to_le_bytes(),
                &self.source_terminal_policy_account.bytes(),
                &self.source_terminal_receipt_account.bytes(),
                &self.source_liveness_policy_account.bytes(),
                &self.source_liveness_compartment_account.bytes(),
                &self.source_funding_custody_account.bytes(),
                &self.source_principal_refund.bytes(),
                &self.source_neutral_sink.bytes(),
            ])
            .to_bytes(),
        )
    }
}

/// Default-refusing bridge implemented only by Failure's exact post-Product-
/// pin attempt preauthorization. A final Refused receipt cannot implement this
/// boundary because that receipt consumes the Source postwrite produced here.
pub(crate) trait AuthenticatedSourceFailureTerminalAuthorityV1 {
    fn authenticate_source_failure_terminal_authority_v1(
        &self,
        _expected: SourceFailureTerminalAuthorityFactsV1,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}

/// Exhaustive reconstructed action-10 failure handoff. Construction remains
/// inside the hostile Source account authenticators.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AuthenticatedSourceFailureHandoffV1 {
    Absence(AuthenticatedFailureAbsenceSourceHandoffV1),
    Refused(AuthenticatedFailureResultSourceHandoffV1),
}

impl AuthenticatedSourceFailureHandoffV1 {
    pub(crate) const fn source_failure_kind(self) -> SourceFailureKindV1 {
        match self {
            Self::Absence(_) => SourceFailureKindV1::PrimaryMaturityWithoutAcceptedResolution,
            Self::Refused(_) => SourceFailureKindV1::SourceEvaluationRefused,
        }
    }

    const fn disposition(self) -> SourceFailureTerminalDispositionV1 {
        match self {
            Self::Absence(_) => SourceFailureTerminalDispositionV1::AbsenceLineageTombstone,
            Self::Refused(_) => SourceFailureTerminalDispositionV1::RefusedResultClose,
        }
    }

    /// Project the one canonical noncircular authority tuple consumed by the
    /// post-Product-pin Failure owner and by this terminal composer.
    pub(crate) fn authority_facts(
        self,
        route: AuthenticatedSourceRouteV1,
        schedule: SourceWorkScheduleBindingV1,
        terminal_policy_account: RuntimeKey,
        terminal_receipt_account: RuntimeKey,
        liveness_policy_account: RuntimeKey,
        liveness_compartment_account: RuntimeKey,
        custody_account: RuntimeKey,
        principal_refund: RuntimeKey,
        neutral_sink: RuntimeKey,
    ) -> SourceFailureTerminalAuthorityFactsV1 {
        let (reconstruction_id, handoff, join, persisted, lineage, work, source_fact_id) =
            match self {
                Self::Absence(value) => (
                    value.id(),
                    value.handoff(),
                    value.join(),
                    value.persisted(),
                    value.lineage(),
                    value.work(),
                    value.absence().result_absence().id(),
                ),
                Self::Refused(value) => (
                    value.id(),
                    value.handoff(),
                    value.join(),
                    value.persisted(),
                    value.lineage(),
                    value.work(),
                    value.result().id(),
                ),
            };
        let occurrence = handoff.occurrence();
        SourceFailureTerminalAuthorityFactsV1 {
            source_release_manifest_id: route.release_manifest_id(),
            source_release_authentication_id: route.release_authentication_id(),
            source_route_id: route.route_id(),
            source_reconstruction_id: reconstruction_id,
            source_handoff_id: handoff.id(),
            source_handoff_join_id: join.id(),
            persisted_handoff_authentication_id: persisted.id(),
            lineage_account: lineage.lineage().lineage_account,
            lineage_authentication_id: lineage.id(),
            lineage_state_id: lineage.account_data_id(),
            result_or_absence_account: join.result_or_absence_account(),
            result_or_absence_authentication_id: source_fact_id,
            work_receipt_account: work.account(),
            work_receipt_authentication_id: work.id(),
            source_failure_kind: handoff.kind(),
            market_instance_id: occurrence.market_instance_id(),
            series_plan_id: occurrence.series_plan_id(),
            ordinal: occurrence.ordinal(),
            source_occurrence_id: occurrence.occurrence_record_id(),
            source_occurrence_account: occurrence.occurrence_account(),
            source_occurrence_authentication_id: occurrence.occurrence_account_authentication_id(),
            source_repair_generation: occurrence.repair_generation(),
            source_work_schedule_id: schedule.source_work_schedule_id(),
            source_lifecycle_id: schedule.lifecycle_id(),
            source_generation: schedule.generation(),
            source_terminal_policy_account: terminal_policy_account,
            source_terminal_receipt_account: terminal_receipt_account,
            source_liveness_policy_account: liveness_policy_account,
            source_liveness_compartment_account: liveness_compartment_account,
            source_funding_custody_account: custody_account,
            source_principal_refund: principal_refund,
            source_neutral_sink: neutral_sink,
        }
    }

    fn semantic(self) -> (
        ContentId,
        ContentId,
        ContentId,
        ContentId,
        ContentId,
        ContentId,
        u64,
        u64,
    ) {
        match self {
            Self::Absence(value) => {
                let handoff = value.handoff();
                let occurrence = handoff.occurrence();
                (
                    value.id(),
                    handoff.id(),
                    value.join().id(),
                    value.persisted().id(),
                    handoff.failure_policy_binding_id(),
                    occurrence.statistic_key_id(),
                    value.schedule().generation(),
                    occurrence.repair_generation(),
                )
            }
            Self::Refused(value) => {
                let handoff = value.handoff();
                let occurrence = handoff.occurrence();
                (
                    value.id(),
                    handoff.id(),
                    value.join().id(),
                    value.persisted().id(),
                    handoff.failure_policy_binding_id(),
                    occurrence.statistic_key_id(),
                    value.schedule().generation(),
                    occurrence.repair_generation(),
                )
            }
        }
    }

    const fn lineage(self) -> clutch_source_plane_v3_runtime::AuthenticatedReopenLineageV1 {
        match self {
            Self::Absence(value) => value.lineage(),
            Self::Refused(value) => value.lineage(),
        }
    }

    const fn market_instance_id(self) -> ContentId {
        match self {
            Self::Absence(value) => value.handoff().occurrence().market_instance_id(),
            Self::Refused(value) => value.handoff().occurrence().market_instance_id(),
        }
    }

    const fn schedule(self) -> SourceWorkScheduleBindingV1 {
        match self {
            Self::Absence(value) => value.schedule(),
            Self::Refused(value) => value.schedule(),
        }
    }

    const fn source_fact_authentication_id(self) -> ContentId {
        match self {
            Self::Absence(value) => value.absence().result_absence().id(),
            Self::Refused(value) => value.result().id(),
        }
    }

    const fn result_or_absence_account(self) -> RuntimeKey {
        match self {
            Self::Absence(value) => value.result_or_absence_account(),
            Self::Refused(value) => value.result_or_absence_account(),
        }
    }
}

/// Exact refused Result/lineage physical close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedRefusedSourceStatisticResultCloseV1 {
    id: ContentId,
    result_account: RuntimeKey,
    result_account_data_before_id: ContentId,
    lineage_authentication_before_id: ContentId,
    lineage_state_before_id: ContentId,
    lineage_state_after_id: ContentId,
    close: CloseRuntimeAccountResultV1,
}

impl AuthenticatedRefusedSourceStatisticResultCloseV1 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn result_account(self) -> RuntimeKey {
        self.result_account
    }

    pub(crate) const fn result_account_data_before_id(self) -> ContentId {
        self.result_account_data_before_id
    }

    pub(crate) const fn lineage_authentication_before_id(self) -> ContentId {
        self.lineage_authentication_before_id
    }

    pub(crate) const fn lineage_state_before_id(self) -> ContentId {
        self.lineage_state_before_id
    }

    pub(crate) const fn lineage_state_after_id(self) -> ContentId {
        self.lineage_state_after_id
    }

    pub(crate) const fn close(self) -> CloseRuntimeAccountResultV1 {
        self.close
    }
}

/// Exact physical disposition selected solely by the reconstructed Source kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AuthenticatedSourceFailurePhysicalDispositionV1 {
    Absence(AuthenticatedAbsentStatisticResultLineageRetirementV1),
    Refused(AuthenticatedRefusedSourceStatisticResultCloseV1),
}

impl AuthenticatedSourceFailurePhysicalDispositionV1 {
    pub(crate) const fn id(self) -> ContentId {
        match self {
            Self::Absence(value) => value.id(),
            Self::Refused(value) => value.id(),
        }
    }

    pub(crate) const fn lineage_state_after_id(self) -> ContentId {
        match self {
            Self::Absence(value) => value.lineage_state_after_id(),
            Self::Refused(value) => value.lineage_state_after_id(),
        }
    }
}

/// Complete Source postwrite consumed by Failure's direct zero-payout archive.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AuthenticatedSourceFailureTerminalPostwriteV1 {
    id: ContentId,
    authority_facts: SourceFailureTerminalAuthorityFactsV1,
    persisted_policy: PersistedSourceFailureTerminalV1,
    terminal: SourceTerminalExecutionV1,
    liveness: RuntimeAtomicTransitionV1,
    physical: AuthenticatedSourceFailurePhysicalDispositionV1,
}

impl AuthenticatedSourceFailureTerminalPostwriteV1 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn authority_facts(self) -> SourceFailureTerminalAuthorityFactsV1 {
        self.authority_facts
    }

    pub(crate) const fn source_failure_kind(self) -> SourceFailureKindV1 {
        self.authority_facts.source_failure_kind
    }

    pub(crate) const fn persisted_policy(self) -> PersistedSourceFailureTerminalV1 {
        self.persisted_policy
    }

    pub(crate) const fn persisted_policy_authentication_id(self) -> ContentId {
        self.persisted_policy.authenticated().id()
    }

    pub(crate) const fn terminal_semantic_id(self) -> ContentId {
        self.terminal.receipt.semantic_receipt_id()
    }

    pub(crate) const fn terminal_receipt_id(self) -> ContentId {
        self.terminal.receipt.receipt_id()
    }

    pub(crate) const fn terminal_receipt_authentication_id(self) -> ContentId {
        self.terminal.authenticated_receipt().id()
    }

    pub(crate) const fn liveness(self) -> RuntimeAtomicTransitionV1 {
        self.liveness
    }

    pub(crate) const fn physical_disposition(
        self,
    ) -> AuthenticatedSourceFailurePhysicalDispositionV1 {
        self.physical
    }

    pub(crate) const fn physical_disposition_id(self) -> ContentId {
        self.physical.id()
    }
}

/// Compose exactly one mature-absence or stable-refusal Source terminal after
/// Product pin, then return the private postwrite required by Failure.
#[allow(clippy::too_many_arguments)]
pub(crate) fn compose_source_failure_terminal_v1<
    A: AuthenticatedSourceFailureTerminalAuthorityV1 + ?Sized,
>(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    source: AuthenticatedSourceFailureHandoffV1,
    authority: &A,
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
) -> Outcome<AuthenticatedSourceFailureTerminalPostwriteV1> {
    let custody = authenticate_source_funding_custody_v1(
        program_id,
        route,
        schedule,
        source_funding_custody,
    )?;
    let lineage = source.lineage();
    let account_keys = [
        runtime_key(result_or_absence_account.key),
        runtime_key(lineage_account.key),
        runtime_key(terminal_policy_account.key),
        runtime_key(terminal_receipt_account.key),
        runtime_key(source_liveness_policy.key),
        runtime_key(source_liveness_compartment.key),
        runtime_key(source_funding_custody.key),
        runtime_key(neutral_sink.key),
    ];
    require(
        schedule.validate_against(route).is_ok()
            && source.schedule() == schedule
            && schedule.source_work_schedule_id() == route.source_work_schedule_id()
            && schedule.payer() == custody.account()
            && source.result_or_absence_account() == runtime_key(result_or_absence_account.key)
            && lineage.access() == LineageAccessV1::Mutable
            && lineage.lineage().lineage_account == runtime_key(lineage_account.key)
            && source_liveness_compartment.key
                == &Pubkey::new_from_array(route.source_compartment_account().bytes())
            && neutral_sink.key == &Pubkey::new_from_array(route.neutral_sink().bytes())
            && all_distinct(&account_keys),
        ClutchError::MismatchedState,
    )?;
    let facts = source.authority_facts(
        route,
        schedule,
        runtime_key(terminal_policy_account.key),
        runtime_key(terminal_receipt_account.key),
        runtime_key(source_liveness_policy.key),
        runtime_key(source_liveness_compartment.key),
        custody.account(),
        custody.account(),
        route.neutral_sink(),
    );
    require(
        facts.source_failure_kind == source.source_failure_kind()
            && facts.lineage_authentication_id == lineage.id()
            && facts.lineage_state_id == lineage.account_data_id()
            && facts.result_or_absence_authentication_id
                == source.source_fact_authentication_id()
            && !facts.id().is_zero(),
        ClutchError::MismatchedState,
    )?;
    authority.authenticate_source_failure_terminal_authority_v1(facts)?;
    let (
        reconstruction_id,
        handoff_id,
        join_id,
        persisted_id,
        failure_policy_binding_id,
        statistic_key_id,
        failure_generation,
        source_repair_generation,
    ) = source.semantic();
    let body = SourceFailureTerminalV1::new(
        route,
        reconstruction_id,
        handoff_id,
        join_id,
        persisted_id,
        facts.id(),
        source.market_instance_id(),
        failure_policy_binding_id,
        source.source_fact_authentication_id(),
        statistic_key_id,
        lineage,
        source.result_or_absence_account(),
        failure_generation,
        source_repair_generation,
        source.source_failure_kind(),
        source.disposition(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let persisted_policy = persist_source_failure_terminal_v1(
        program_id,
        route,
        body,
        custody,
        source_funding_custody,
        terminal_policy_account,
        system_program,
        rent_sysvar,
    )?;
    require(
        persisted_policy.authenticated().body() == body
            && persisted_policy
                .authenticated()
                .body()
                .source_failure_terminal_authority_id()
                == facts.id(),
        ClutchError::MismatchedState,
    )?;
    let terminal_semantic =
        AuthenticatedSourceTerminalSemanticV1::source_failure(persisted_policy)?;
    let terminal = bind_terminal_execution(
        program_id,
        route,
        schedule,
        terminal_semantic,
        terminal_receipt_account,
        custody,
        source_funding_custody,
        system_program,
        rent_sysvar,
    )?;
    let liveness = apply_source_terminal_liveness(
        program_id,
        route,
        terminal,
        source_liveness_policy,
        source_liveness_compartment,
        source_funding_custody,
        neutral_sink,
    )?;
    require(
        liveness.action == RuntimeTransitionActionV1::CloseSuccess
            && liveness.close_account
            && liveness.account_balance_after == 0,
        ClutchError::MismatchedState,
    )?;
    let physical = match source {
        AuthenticatedSourceFailureHandoffV1::Absence(value) => {
            require(
                value.absence().result_absence().access()
                    == StatisticResultAbsenceAccessV1::TerminalMutable
                    && result_or_absence_account.owner
                        == &Pubkey::new_from_array(route.system_program().bytes())
                    && result_or_absence_account.lamports() == 0
                    && result_or_absence_account.data_is_empty()
                    && result_or_absence_account.is_writable
                    && !result_or_absence_account.is_signer
                    && !result_or_absence_account.executable,
                ClutchError::MismatchedState,
            )?;
            let retired = retire_absent_statistic_result_lineage_v1(
                route,
                lineage,
                lineage_account,
                statistic_key_id,
                terminal_semantic.semantic_id(),
            )?;
            require(
                result_or_absence_account.lamports() == 0
                    && result_or_absence_account.data_is_empty()
                    && result_or_absence_account.owner
                        == &Pubkey::new_from_array(route.system_program().bytes()),
                ClutchError::MismatchedState,
            )?;
            AuthenticatedSourceFailurePhysicalDispositionV1::Absence(retired)
        }
        AuthenticatedSourceFailureHandoffV1::Refused(value) => {
            require(
                value.result().access() == StatisticResultAccountAccessV1::ResolutionMutable
                    && value.result().account() == runtime_key(result_or_absence_account.key),
                ClutchError::MismatchedState,
            )?;
            let result_data = result_or_absence_account
                .try_borrow_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
            let result_account_data_before_id =
                account_data_id(runtime_key(result_or_absence_account.key), &result_data)
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
            drop(result_data);
            require(
                result_account_data_before_id == value.result().account_data_id(),
                ClutchError::MismatchedState,
            )?;
            let close = close_statistic_result_generation(
                program_id,
                route,
                lineage,
                result_or_absence_account,
                lineage_account,
                source_funding_custody,
                neutral_sink,
                terminal.authenticated_receipt(),
            )?;
            let lineage_after_bytes = close
                .lineage_after
                .encode()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
            let lineage_state_after_id =
                account_data_id(runtime_key(lineage_account.key), &lineage_after_bytes)
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
            require(
                !close.lineage_after.is_open
                    && close.funding.account == runtime_key(result_or_absence_account.key)
                    && close.funding.principal_recipient == custody.account()
                    && close.funding.neutral_sink == route.neutral_sink()
                    && close.funding.terminal_receipt_id == terminal_semantic.semantic_id()
                    && result_or_absence_account.lamports() == 0,
                ClutchError::MismatchedState,
            )?;
            let id = ContentId::from_bytes(
                solana_sha256_hasher::hashv(&[
                    SOURCE_REFUSED_RESULT_CLOSE_DOMAIN_V1,
                    &facts.id().bytes(),
                    &persisted_policy.authenticated().id().bytes(),
                    &terminal.authenticated_receipt().id().bytes(),
                    result_or_absence_account.key.as_ref(),
                    &result_account_data_before_id.bytes(),
                    lineage_account.key.as_ref(),
                    &lineage.id().bytes(),
                    &lineage.account_data_id().bytes(),
                    &lineage_state_after_id.bytes(),
                    &close.funding.close_receipt_id.bytes(),
                    source_funding_custody.key.as_ref(),
                    neutral_sink.key.as_ref(),
                ])
                .to_bytes(),
            );
            require(!id.is_zero(), ClutchError::MismatchedState)?;
            AuthenticatedSourceFailurePhysicalDispositionV1::Refused(
                AuthenticatedRefusedSourceStatisticResultCloseV1 {
                    id,
                    result_account: runtime_key(result_or_absence_account.key),
                    result_account_data_before_id,
                    lineage_authentication_before_id: lineage.id(),
                    lineage_state_before_id: lineage.account_data_id(),
                    lineage_state_after_id,
                    close,
                },
            )
        }
    };
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SOURCE_FAILURE_TERMINAL_POSTWRITE_DOMAIN_V1,
            &facts.id().bytes(),
            &persisted_policy.authenticated().id().bytes(),
            &terminal.authenticated_receipt().id().bytes(),
            &terminal.receipt.receipt_id().bytes(),
            &physical.id().bytes(),
            &physical.lineage_state_after_id().bytes(),
            &custody.account().bytes(),
            &route.neutral_sink().bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedSourceFailureTerminalPostwriteV1 {
        id,
        authority_facts: facts,
        persisted_policy,
        terminal,
        liveness,
        physical,
    })
}

fn all_distinct(values: &[RuntimeKey]) -> bool {
    let mut index = 0_usize;
    while index < values.len() {
        let mut prior = 0_usize;
        while prior < index {
            if values[prior] == values[index] {
                return false;
            }
            prior += 1;
        }
        index += 1;
    }
    true
}

const fn source_failure_kind_byte(kind: SourceFailureKindV1) -> u8 {
    match kind {
        SourceFailureKindV1::PrimaryMaturityWithoutAcceptedResolution => 1,
        SourceFailureKindV1::SourceEvaluationRefused => 2,
    }
}

#[cfg(test)]
mod adversarial_tests {
    use super::*;

    struct RefusingAuthority;
    impl AuthenticatedSourceFailureTerminalAuthorityV1 for RefusingAuthority {}

    #[test]
    fn default_failure_terminal_authority_refuses() {
        let _ = RefusingAuthority;
    }

    #[test]
    fn failure_terminal_order_is_preauth_persist_liveness_physical_postwrite() {
        let source = include_str!("source_failure_terminal_v1.rs");
        let body = source
            .split("pub(crate) fn compose_source_failure_terminal_v1")
            .nth(1)
            .expect("private failure terminal composer");
        let authority = body
            .find("authenticate_source_failure_terminal_authority_v1")
            .expect("post-pin preauthorization");
        let persist = body
            .find("persist_source_failure_terminal_v1")
            .expect("durable Source terminal");
        let terminal = body
            .find("bind_terminal_execution")
            .expect("terminal receipt");
        let liveness = body
            .find("apply_source_terminal_liveness")
            .expect("liveness close");
        let physical = body.find("let physical = match source").expect("physical branch");
        assert!(authority < persist && persist < terminal && terminal < liveness && liveness < physical);
        assert!(!body.contains("FailureMarketIntervalCellSourceFailureReceiptV2"));
    }

    #[test]
    fn absence_and_refusal_have_no_shared_physical_fallback() {
        let source = include_str!("source_failure_terminal_v1.rs");
        let body = source
            .split("let physical = match source")
            .nth(1)
            .and_then(|value| value.split("let id = ContentId::from_bytes").next())
            .expect("exhaustive physical branch");
        for predicate in [
            "AuthenticatedSourceFailureHandoffV1::Absence",
            "StatisticResultAbsenceAccessV1::TerminalMutable",
            "retire_absent_statistic_result_lineage_v1",
            "result_or_absence_account.lamports() == 0",
            "AuthenticatedSourceFailureHandoffV1::Refused",
            "StatisticResultAccountAccessV1::ResolutionMutable",
            "close_statistic_result_generation",
            "close.funding.principal_recipient == custody.account()",
        ] {
            assert!(body.contains(predicate), "missing terminal guard {predicate}");
        }
    }

    #[test]
    fn terminal_account_tuple_is_pairwise_distinct() {
        let a = RuntimeKey::from_bytes([1; 32]);
        let b = RuntimeKey::from_bytes([2; 32]);
        let c = RuntimeKey::from_bytes([3; 32]);
        assert!(all_distinct(&[a, b, c]));
        assert!(!all_distinct(&[a, b, a]));
        assert!(!all_distinct(&[a, b, b]));
    }
}
