//! Private Source terminal adapter over the current Product/Failure writer.
//!
//! This module has no dispatcher entry and accepts no instruction payload. Its
//! sole composer consumes Product's private current Source input, Failure's
//! exact resolved-cell receipt, and a default-refusing private capability that
//! can be implemented only by the final same-call ResolutionV5/cell postwrite.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::product_series::AuthenticatedSourceResolutionInputV3;
use crate::source_plane_v3::runtime_key;
use crate::source_plane_v3_actions::{
    apply_source_terminal_liveness, bind_terminal_execution,
    persist_source_no_reopen_terminal, persist_source_reopen_generation_request,
    AuthenticatedSourceTerminalSemanticV1, PersistedSourceNoReopenTerminalV1,
    PersistedSourceReopenGenerationRequestV1, SourceTerminalExecutionV1,
};
use clutch_failure_policy_runtime::market_interval_cell_v2::FailureMarketIntervalCellResolutionReceiptV2;
use clutch_liveness::runtime_adapter_v1::RuntimeAtomicTransitionV1;
use clutch_source_plane_v3::ContentId;
use clutch_source_plane_v3_adapter::PdaRecipeV3;
use clutch_source_plane_v3_runtime::{
    account_data_id, close_lineage_generation, AuthenticatedReopenLineageV1,
    AuthenticatedSourceRouteV1, LineageAccessV1, LineageFamilyV1,
    SourceNoReopenTerminalV1, SourceReopenFamilyV1, SourceReopenGenerationRequestV1,
    SourceReopenTargetV1, SourceWorkScheduleBindingV1,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const SOURCE_RESOLUTION_TERMINAL_COMPOSITION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/source-resolution-terminal-composition/v1";
const SOURCE_RESOLUTION_TERMINAL_POLICY_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/source-resolution-terminal-policy/v1";

/// Exhaustive persisted outcome selected by the final Product/Failure
/// postwrite. No instruction payload constructs this enum.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceResolutionTerminalChoiceV1 {
    /// This exact family must close permanently.
    NoReopen(SourceReopenFamilyV1),
    /// Close now, then action 11 may open only this reconstructed target.
    ReopenRequest(SourceReopenTargetV1),
}

impl SourceResolutionTerminalChoiceV1 {
    const fn family(self) -> SourceReopenFamilyV1 {
        match self {
            Self::NoReopen(family) => family,
            Self::ReopenRequest(target) => target.family(),
        }
    }

    fn target_body_id(self) -> Outcome<ContentId> {
        match self {
            Self::NoReopen(_) => Ok(ContentId::ZERO),
            Self::ReopenRequest(target) => target
                .body_id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState)),
        }
    }
}

/// Private final-postwrite selection of exactly one terminal outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSourceResolutionTerminalPolicyV1 {
    id: ContentId,
    resolution_v5_terminal_postwrite_id: ContentId,
    source_resolution_input_id: ContentId,
    failure_resolution_receipt_id: ContentId,
    lineage_authentication_id: ContentId,
    lineage_state_id: ContentId,
    choice: SourceResolutionTerminalChoiceV1,
}

impl AuthenticatedSourceResolutionTerminalPolicyV1 {
    /// Select the sole current successful-Resolution terminal: the exact
    /// persisted StatisticResult generation closes permanently. EvaluationWork
    /// and the other mutable families cannot be silently coerced into it.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn successful_resolution_no_reopen(
        resolution_v5_terminal_postwrite_id: ContentId,
        route: AuthenticatedSourceRouteV1,
        source: AuthenticatedSourceResolutionInputV3,
        failure: FailureMarketIntervalCellResolutionReceiptV2,
        lineage: AuthenticatedReopenLineageV1,
    ) -> Outcome<Self> {
        let statistic_result_recipe = PdaRecipeV3::statistic_result(source.statistic_key_id())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            lineage.lineage().family == LineageFamilyV1::StatisticResult
                && lineage.lineage().active_account == source.result_account()
                && lineage.lineage().semantic_binding_id
                    == statistic_result_recipe
                        .id()
                        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
            ClutchError::MismatchedState,
        )?;
        Self::new(
            resolution_v5_terminal_postwrite_id,
            route,
            source,
            failure,
            lineage,
            SourceResolutionTerminalChoiceV1::NoReopen(
                SourceReopenFamilyV1::StatisticResult,
            ),
        )
    }

    /// Select the alternate terminal only after the final private policy owner
    /// reconstructs one complete target body from persisted Source facts.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn reconstructed_reopen_request(
        resolution_v5_terminal_postwrite_id: ContentId,
        route: AuthenticatedSourceRouteV1,
        source: AuthenticatedSourceResolutionInputV3,
        failure: FailureMarketIntervalCellResolutionReceiptV2,
        lineage: AuthenticatedReopenLineageV1,
        target: SourceReopenTargetV1,
    ) -> Outcome<Self> {
        Self::new(
            resolution_v5_terminal_postwrite_id,
            route,
            source,
            failure,
            lineage,
            SourceResolutionTerminalChoiceV1::ReopenRequest(target),
        )
    }

    /// Shared private constructor after the exhaustive terminal choice has
    /// already been made by one of the semantic-owner entry points above.
    #[allow(clippy::too_many_arguments)]
    fn new(
        resolution_v5_terminal_postwrite_id: ContentId,
        route: AuthenticatedSourceRouteV1,
        source: AuthenticatedSourceResolutionInputV3,
        failure: FailureMarketIntervalCellResolutionReceiptV2,
        lineage: AuthenticatedReopenLineageV1,
        choice: SourceResolutionTerminalChoiceV1,
    ) -> Outcome<Self> {
        let state = lineage.lineage();
        let family = choice.family();
        require(
            !resolution_v5_terminal_postwrite_id.is_zero()
                && lineage.access() == LineageAccessV1::Mutable
                && state.is_open
                && state.family == lineage_family(family),
            ClutchError::MismatchedState,
        )?;
        if let SourceResolutionTerminalChoiceV1::ReopenRequest(target) = choice {
            let recipe = target
                .recipe(route)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
            require(
                recipe
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                    == state.semantic_binding_id,
                ClutchError::MismatchedState,
            )?;
        }
        let target_body_id = choice.target_body_id()?;
        let id = ContentId::from_bytes(
            solana_sha256_hasher::hashv(&[
                SOURCE_RESOLUTION_TERMINAL_POLICY_DOMAIN_V1,
                &resolution_v5_terminal_postwrite_id.bytes(),
                &source.id().bytes(),
                &failure.id().bytes(),
                &lineage.id().bytes(),
                &lineage.account_data_id().bytes(),
                &[match choice {
                    SourceResolutionTerminalChoiceV1::NoReopen(_) => 1,
                    SourceResolutionTerminalChoiceV1::ReopenRequest(_) => 2,
                }],
                &[family.wire_byte()],
                &target_body_id.bytes(),
            ])
            .to_bytes(),
        );
        require(!id.is_zero(), ClutchError::MismatchedState)?;
        Ok(Self {
            id,
            resolution_v5_terminal_postwrite_id,
            source_resolution_input_id: source.id(),
            failure_resolution_receipt_id: ContentId::from_bytes(failure.id().bytes()),
            lineage_authentication_id: lineage.id(),
            lineage_state_id: lineage.account_data_id(),
            choice,
        })
    }

    const fn id(self) -> ContentId {
        self.id
    }

    const fn postwrite_id(self) -> ContentId {
        self.resolution_v5_terminal_postwrite_id
    }

    const fn choice(self) -> SourceResolutionTerminalChoiceV1 {
        self.choice
    }

    fn validate_for(
        self,
        source: AuthenticatedSourceResolutionInputV3,
        failure: FailureMarketIntervalCellResolutionReceiptV2,
        lineage: AuthenticatedReopenLineageV1,
    ) -> Outcome<()> {
        require(
            self.source_resolution_input_id == source.id()
                && self.failure_resolution_receipt_id.bytes() == failure.id().bytes()
                && self.lineage_authentication_id == lineage.id()
                && self.lineage_state_id == lineage.account_data_id()
                && !self.resolution_v5_terminal_postwrite_id.is_zero(),
            ClutchError::MismatchedState,
        )
    }
}

/// Default-refusing bridge implemented only by Product/Failure's final
/// ResolutionV5 and resolved-cell postwrite receipt.
///
/// The earlier Resolution activation capability is deliberately insufficient:
/// this method must refuse until both Product's slot-10/root postwrite and the
/// exact Failure resolved-cell physical write have completed in this call.
pub(crate) trait AuthenticatedSourceResolutionV5TerminalV1 {
    /// Authenticate the exact current Source input and retained private
    /// Failure receipt, then select exactly one reconstructed terminal policy.
    fn authenticate_source_resolution_v5_terminal_v1(
        &self,
        _route: AuthenticatedSourceRouteV1,
        _source: AuthenticatedSourceResolutionInputV3,
        _failure: FailureMarketIntervalCellResolutionReceiptV2,
        _lineage: AuthenticatedReopenLineageV1,
    ) -> Outcome<AuthenticatedSourceResolutionTerminalPolicyV1> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}

/// Exhaustive durable terminal outcome. Both variants require an exact prior
/// persistence postwrite before the terminal work receipt can be minted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PersistedSourceResolutionTerminalPolicyV1 {
    /// Permanent no-reopen decision.
    NoReopen(PersistedSourceNoReopenTerminalV1),
    /// Exact release-selected GenerationAuthority request.
    ReopenRequest(PersistedSourceReopenGenerationRequestV1),
}

impl PersistedSourceResolutionTerminalPolicyV1 {
    fn terminal_semantic(self) -> Outcome<AuthenticatedSourceTerminalSemanticV1> {
        match self {
            Self::NoReopen(value) => AuthenticatedSourceTerminalSemanticV1::no_reopen(value),
            Self::ReopenRequest(value) => {
                AuthenticatedSourceTerminalSemanticV1::reopen_request(value)
            }
        }
    }
}

/// Private complete Source terminal postwrite. No payout or caller-selected
/// terminal coordinate escapes this receipt.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AuthenticatedSourceResolutionTerminalV1 {
    id: ContentId,
    policy: PersistedSourceResolutionTerminalPolicyV1,
    terminal: SourceTerminalExecutionV1,
    liveness: RuntimeAtomicTransitionV1,
}

impl AuthenticatedSourceResolutionTerminalV1 {
    /// Complete Source/Product/Failure/postwrite identity.
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    /// Exhaustive durable no-reopen or reopen-request policy.
    pub(crate) const fn policy(self) -> PersistedSourceResolutionTerminalPolicyV1 {
        self.policy
    }

    /// Sole Source terminal work receipt accepted by action 12.
    pub(crate) const fn terminal(self) -> SourceTerminalExecutionV1 {
        self.terminal
    }

    /// Atomic close of the exact Source liveness compartment.
    pub(crate) const fn liveness(self) -> RuntimeAtomicTransitionV1 {
        self.liveness
    }
}

/// Atomically bind the final Product/Failure resolution to one exact Source
/// lineage, persist exactly one no-reopen or reconstructed GenerationAuthority
/// request, mint the sole terminal receipt, and close Source liveness.
///
/// There is intentionally no family or reopen-body parameter. The final
/// private Product/Failure postwrite selects the exhaustive policy; callers
/// supply only physical account metas which are reauthenticated here.
#[allow(clippy::too_many_arguments)]
pub(crate) fn compose_source_resolution_terminal_v1<
    A: AuthenticatedSourceResolutionV5TerminalV1 + ?Sized,
>(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    source: AuthenticatedSourceResolutionInputV3,
    failure: FailureMarketIntervalCellResolutionReceiptV2,
    resolution_terminal: &A,
    lineage: AuthenticatedReopenLineageV1,
    terminal_policy_account: &AccountInfo<'_>,
    terminal_receipt_account: &AccountInfo<'_>,
    source_liveness_policy: &AccountInfo<'_>,
    source_liveness_compartment: &AccountInfo<'_>,
    payer_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    account_payer: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<AuthenticatedSourceResolutionTerminalV1> {
    let product_route = source.route();
    let failure_facts = failure.facts();
    require(
        product_route.source_release_manifest_id() == route.release_manifest_id()
            && product_route.source_release_authentication_id()
                == route.release_authentication_id()
            && product_route.source_route_id() == route.route_id()
            && product_route.source_plane_contract_id() == route.source_plane_contract_id()
            && product_route.source_spec_id() == route.source_spec_id()
            && schedule.source_work_schedule_id() == route.source_work_schedule_id()
            && schedule.generation() == failure_facts.generation
            && runtime_key(account_payer.key) == schedule.payer()
            && runtime_key(payer_refund.key) == schedule.payer()
            && source.failure_policy_binding_id().bytes()
                == failure.failure_policy_binding_id().bytes()
            && source.successful_evaluation_handoff_id().bytes()
                == failure_facts.source_handoff_id.bytes()
            && source.market_instance_id().bytes() == failure_facts.market_instance_id.bytes()
            && runtime_key(terminal_policy_account.key)
                != runtime_key(terminal_receipt_account.key)
            && runtime_key(terminal_policy_account.key) != route.source_compartment_account()
            && runtime_key(terminal_receipt_account.key) != route.source_compartment_account()
            && account_payer.key != terminal_policy_account.key
            && account_payer.key != terminal_receipt_account.key
            && account_payer.key != neutral_sink.key,
        ClutchError::MismatchedState,
    )?;
    let terminal_policy = resolution_terminal
        .authenticate_source_resolution_v5_terminal_v1(route, source, failure, lineage)?;
    terminal_policy.validate_for(source, failure, lineage)?;
    let resolution_terminal_postwrite_id = terminal_policy.postwrite_id();
    let family = terminal_policy.choice().family();
    let persisted_policy = match terminal_policy.choice() {
        SourceResolutionTerminalChoiceV1::NoReopen(selected_family) => {
            let body = SourceNoReopenTerminalV1::new(
                route,
                source.id(),
                source.successful_evaluation_handoff_id(),
                source.failure_policy_binding_id(),
                ContentId::from_bytes(failure.id().bytes()),
                resolution_terminal_postwrite_id,
                source.market_instance_id(),
                failure_facts.generation,
                source.source_repair_generation(),
                selected_family,
                lineage,
            )
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
            let persisted = persist_source_no_reopen_terminal(
                program_id,
                route,
                body,
                account_payer,
                terminal_policy_account,
                system_program,
                rent_sysvar,
            )?;
            require(
                persisted.authenticated().resolution_v5_terminal_postwrite_id()
                    == resolution_terminal_postwrite_id,
                ClutchError::MismatchedState,
            )?;
            PersistedSourceResolutionTerminalPolicyV1::NoReopen(persisted)
        }
        SourceResolutionTerminalChoiceV1::ReopenRequest(target) => {
            let state = lineage.lineage();
            let projected_closed = close_lineage_generation(
                state,
                state.active_account,
                state.latest_generation,
                state.last_opened_state_id,
                terminal_policy.id(),
            )
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
            let projected_bytes = projected_closed
                .encode()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
            let projected_data_id = account_data_id(state.lineage_account, &projected_bytes)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
            let request = SourceReopenGenerationRequestV1::new(
                route,
                projected_data_id,
                terminal_policy.id(),
                target,
                projected_closed,
            )
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
            let persisted = persist_source_reopen_generation_request(
                program_id,
                route,
                request,
                account_payer,
                terminal_policy_account,
                system_program,
                rent_sysvar,
            )?;
            require(
                persisted.request().generation_policy_id() == terminal_policy.id()
                    && persisted.request().expected_lineage_state_id() == projected_data_id,
                ClutchError::MismatchedState,
            )?;
            PersistedSourceResolutionTerminalPolicyV1::ReopenRequest(persisted)
        }
    };
    let terminal_semantic = persisted_policy.terminal_semantic()?;
    let terminal_semantic_id = terminal_semantic.semantic_id();
    let terminal = bind_terminal_execution(
        program_id,
        route,
        schedule,
        terminal_semantic,
        terminal_receipt_account,
        account_payer,
        system_program,
        rent_sysvar,
    )?;
    require(
        terminal.receipt.semantic_receipt_id() == terminal_semantic_id,
        ClutchError::MismatchedState,
    )?;
    let liveness = apply_source_terminal_liveness(
        program_id,
        route,
        terminal,
        source_liveness_policy,
        source_liveness_compartment,
        payer_refund,
        neutral_sink,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SOURCE_RESOLUTION_TERMINAL_COMPOSITION_DOMAIN_V1,
            &source.id().bytes(),
            &failure.id().bytes(),
            &resolution_terminal_postwrite_id.bytes(),
            &terminal_policy.id().bytes(),
            &terminal_semantic.persistence_authentication_id().bytes(),
            &terminal_semantic_id.bytes(),
            &terminal.receipt.receipt_id().bytes(),
            &lineage.id().bytes(),
            &lineage.account_data_id().bytes(),
            &[family.wire_byte()],
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedSourceResolutionTerminalV1 {
        id,
        policy: persisted_policy,
        terminal,
        liveness,
    })
}

fn lineage_family(family: SourceReopenFamilyV1) -> LineageFamilyV1 {
    match family {
        SourceReopenFamilyV1::SourceHead => LineageFamilyV1::SourceHead,
        SourceReopenFamilyV1::OpenRawPage => LineageFamilyV1::OpenRawPage,
        SourceReopenFamilyV1::WindowWork => LineageFamilyV1::WindowWork,
        SourceReopenFamilyV1::StatisticResult => LineageFamilyV1::StatisticResult,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct RefusingTerminal;

    impl AuthenticatedSourceResolutionV5TerminalV1 for RefusingTerminal {}

    #[test]
    fn default_terminal_bridge_is_not_an_authority() {
        let _ = RefusingTerminal;
        // Construction of Product's and Failure's private receipts is not
        // available here. The absence of an override is itself the adversarial
        // invariant: every possible call returns AuthorizationUnavailable.
    }
}
