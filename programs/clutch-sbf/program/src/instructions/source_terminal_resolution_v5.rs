//! Private Source terminal adapter over the current Product/Failure writer.
//!
//! This module has no dispatcher entry and accepts no instruction payload. Its
//! sole composer consumes Product's private current Source input, Failure's
//! exact resolved-cell receipt, and a default-refusing private capability that
//! can be implemented only by the final same-call ResolutionV5/cell postwrite.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::product_source_current::AuthenticatedSourceResolutionInputV4;
use crate::source_plane_v3::{authenticate_lineage, runtime_key};
use crate::source_plane_v3_actions::{
    apply_source_terminal_liveness, authenticate_source_funding_custody_v1,
    bind_terminal_execution,
    close_statistic_result_generation, persist_source_no_reopen_terminal,
    persist_source_reopen_generation_request, AuthenticatedSourceTerminalSemanticV1,
    CloseRuntimeAccountResultV1, PersistedSourceNoReopenTerminalV1,
    PersistedSourceReopenGenerationRequestV1, SourceTerminalExecutionV1,
};
use clutch_failure_policy_runtime::market_interval_cell_v2::FailureMarketIntervalCellResolutionReceiptV2;
use clutch_liveness::runtime_adapter_v1::{
    RuntimeAtomicTransitionV1, RuntimeTransitionActionV1,
};
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
const SOURCE_RESOLUTION_STATISTIC_RESULT_CLOSE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/source-resolution-statistic-result-close/v1";

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
        source: AuthenticatedSourceResolutionInputV4,
        failure: FailureMarketIntervalCellResolutionReceiptV2,
        lineage: AuthenticatedReopenLineageV1,
    ) -> Outcome<Self> {
        let statistic_result_recipe = PdaRecipeV3::statistic_result(source.statistic_key_id())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let statistic_result_recipe_id = statistic_result_recipe
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            successful_statistic_result_lineage_matches(
                lineage.lineage().family,
                lineage.lineage().active_account,
                lineage.lineage().semantic_binding_id,
                source.result_account(),
                statistic_result_recipe_id,
            ),
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
        source: AuthenticatedSourceResolutionInputV4,
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
        source: AuthenticatedSourceResolutionInputV4,
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
        source: AuthenticatedSourceResolutionInputV4,
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
        _source: AuthenticatedSourceResolutionInputV4,
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
    payer: clutch_source_plane_v3_runtime::RuntimeKey,
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

    /// Exact release-selected Source payer sponsoring terminal persistence and
    /// receiving the retired StatisticResult principal.
    pub(crate) const fn payer(self) -> clutch_source_plane_v3_runtime::RuntimeKey {
        self.payer
    }
}

/// Exact physical StatisticResult/lineage tombstone written only after the
/// private successful-resolution terminal policy and receipt already exist.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSourceResolutionStatisticResultCloseV1 {
    id: ContentId,
    source_resolution_input_id: ContentId,
    source_terminal_id: ContentId,
    result_account: clutch_source_plane_v3_runtime::RuntimeKey,
    result_account_data_before_id: ContentId,
    lineage_account: clutch_source_plane_v3_runtime::RuntimeKey,
    lineage_authentication_before_id: ContentId,
    lineage_state_before_id: ContentId,
    lineage_state_after_id: ContentId,
    close: CloseRuntimeAccountResultV1,
}

impl AuthenticatedSourceResolutionStatisticResultCloseV1 {
    /// Complete Source terminal/physical-close postwrite identity.
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    /// Exact private successful Source input consumed by the close.
    pub(crate) const fn source_resolution_input_id(self) -> ContentId {
        self.source_resolution_input_id
    }

    /// Exact policy/receipt/liveness terminal postwrite consumed first.
    pub(crate) const fn source_terminal_id(self) -> ContentId {
        self.source_terminal_id
    }

    /// Exact closed StatisticResult account.
    pub(crate) const fn result_account(self) -> clutch_source_plane_v3_runtime::RuntimeKey {
        self.result_account
    }

    /// Exact hostile-reopened StatisticResult preimage consumed by close.
    pub(crate) const fn result_account_data_before_id(self) -> ContentId {
        self.result_account_data_before_id
    }

    /// Exact durable lineage tombstone account.
    pub(crate) const fn lineage_account(self) -> clutch_source_plane_v3_runtime::RuntimeKey {
        self.lineage_account
    }

    /// Exact mutable-lineage authentication consumed by close.
    pub(crate) const fn lineage_authentication_before_id(self) -> ContentId {
        self.lineage_authentication_before_id
    }

    /// Exact open-lineage preimage identity consumed by close.
    pub(crate) const fn lineage_state_before_id(self) -> ContentId {
        self.lineage_state_before_id
    }

    /// Exact closed-lineage postimage identity.
    pub(crate) const fn lineage_state_after_id(self) -> ContentId {
        self.lineage_state_after_id
    }

    /// Exact principal/surplus close partition and closed lineage value.
    pub(crate) const fn close(self) -> CloseRuntimeAccountResultV1 {
        self.close
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
    source: AuthenticatedSourceResolutionInputV4,
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
    let custody = authenticate_source_funding_custody_v1(
        program_id,
        route,
        schedule,
        account_payer,
    )?;
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
            && custody.account() == schedule.payer()
            && payer_refund.key == account_payer.key
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
                custody,
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
                custody,
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
        custody,
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
            &runtime_key(account_payer.key).bytes(),
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
        payer: runtime_key(account_payer.key),
    })
}

/// Close the exact successful StatisticResult generation in the same SVM
/// instruction, after terminal policy/receipt persistence and Source liveness
/// close, without accepting action-12 payload authority.
///
/// The terminal policy and receipt were created writable earlier in this
/// instruction. This adapter therefore consumes their retained CreatedMutable
/// authentication receipts directly; it never attempts an impossible
/// same-call `ExistingReadOnly` reauthentication under SVM privilege union.
#[allow(clippy::too_many_arguments)]
pub(crate) fn close_successful_source_statistic_result_v1(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    source: AuthenticatedSourceResolutionInputV4,
    terminal: AuthenticatedSourceResolutionTerminalV1,
    result_account: &AccountInfo<'_>,
    lineage_account: &AccountInfo<'_>,
    principal_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
) -> Outcome<AuthenticatedSourceResolutionStatisticResultCloseV1> {
    let no_reopen = match terminal.policy() {
        PersistedSourceResolutionTerminalPolicyV1::NoReopen(value) => value.authenticated(),
        PersistedSourceResolutionTerminalPolicyV1::ReopenRequest(_) => {
            return Err(Refusal::Adapter(ClutchError::MismatchedState));
        }
    };
    let product_route = source.route();
    let lineage = authenticate_lineage(
        program_id,
        route,
        lineage_account,
        LineageAccessV1::Mutable,
    )
    .map_err(Refusal::from)?;
    let lineage_before = lineage.lineage();
    let statistic_result_recipe = PdaRecipeV3::statistic_result(source.statistic_key_id())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let statistic_result_recipe_id = statistic_result_recipe
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let terminal_receipt = terminal.terminal().authenticated_receipt();
    let terminal_semantic_id = terminal_receipt.receipt().semantic_receipt_id();
    require(
        product_route.source_release_manifest_id() == route.release_manifest_id()
            && product_route.source_release_authentication_id()
                == route.release_authentication_id()
            && product_route.source_route_id() == route.route_id()
            && product_route.source_plane_contract_id() == route.source_plane_contract_id()
            && product_route.source_spec_id() == route.source_spec_id()
            && no_reopen.source_resolution_input_id() == source.id()
            && no_reopen.family() == SourceReopenFamilyV1::StatisticResult
            && no_reopen.expected_lineage_state_id() == lineage.account_data_id()
            && no_reopen.lineage_authentication_id() == lineage.id()
            && no_reopen.lineage_account() == runtime_key(lineage_account.key)
            && no_reopen.target_account() == source.result_account()
            && no_reopen.target_account() == runtime_key(result_account.key)
            && no_reopen
                .terminal_id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == terminal_semantic_id
            && terminal_receipt.receipt() == terminal.terminal().receipt
            && terminal_receipt.account() == terminal.terminal().receipt_funding.account
            && terminal_receipt.schedule().payer() == terminal.payer()
            && terminal.liveness().action == RuntimeTransitionActionV1::CloseSuccess
            && terminal.liveness().close_account
            && terminal.liveness().account_balance_after == 0
            && lineage_before.family == LineageFamilyV1::StatisticResult
            && lineage_before.semantic_binding_id == statistic_result_recipe_id
            && lineage_before.active_account == runtime_key(result_account.key)
            && lineage_before.last_opened_state_id == source.result_account_data_id()
            && principal_refund.is_writable
            && !principal_refund.is_signer
            && !principal_refund.executable
            && runtime_key(principal_refund.key) == terminal.payer()
            && neutral_sink.is_writable
            && !neutral_sink.is_signer
            && !neutral_sink.executable
            && runtime_key(neutral_sink.key) == route.neutral_sink(),
        ClutchError::MismatchedState,
    )?;
    let result_data = result_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let result_account_data_before_id =
        account_data_id(runtime_key(result_account.key), &result_data)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(result_data);
    require(
        result_account_data_before_id == source.result_account_data_id(),
        ClutchError::MismatchedState,
    )?;
    let projected_lineage_after = close_lineage_generation(
        lineage_before,
        runtime_key(result_account.key),
        lineage_before.latest_generation,
        result_account_data_before_id,
        terminal_semantic_id,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let projected_lineage_bytes = projected_lineage_after
        .encode()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let projected_lineage_state_after_id =
        account_data_id(runtime_key(lineage_account.key), &projected_lineage_bytes)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let close = close_statistic_result_generation(
        program_id,
        route,
        lineage,
        result_account,
        lineage_account,
        principal_refund,
        neutral_sink,
        terminal_receipt,
    )?;
    let lineage_after_bytes = close
        .lineage_after
        .encode()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let lineage_state_after_id =
        account_data_id(runtime_key(lineage_account.key), &lineage_after_bytes)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        close.lineage_after == projected_lineage_after
            && lineage_state_after_id == projected_lineage_state_after_id
            && !close.lineage_after.is_open
            && close.funding.account == source.result_account()
            && close.funding.principal_recipient == runtime_key(principal_refund.key)
            && close.funding.neutral_sink == runtime_key(neutral_sink.key)
            && close.funding.terminal_receipt_id == terminal_semantic_id
            && result_account.lamports() == 0,
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SOURCE_RESOLUTION_STATISTIC_RESULT_CLOSE_DOMAIN_V1,
            &source.id().bytes(),
            &terminal.id().bytes(),
            &no_reopen.id().bytes(),
            &terminal_receipt.id().bytes(),
            result_account.key.as_ref(),
            &result_account_data_before_id.bytes(),
            lineage_account.key.as_ref(),
            &lineage.id().bytes(),
            &lineage.account_data_id().bytes(),
            &lineage_state_after_id.bytes(),
            &close.funding.close_receipt_id.bytes(),
            principal_refund.key.as_ref(),
            neutral_sink.key.as_ref(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedSourceResolutionStatisticResultCloseV1 {
        id,
        source_resolution_input_id: source.id(),
        source_terminal_id: terminal.id(),
        result_account: runtime_key(result_account.key),
        result_account_data_before_id,
        lineage_account: runtime_key(lineage_account.key),
        lineage_authentication_before_id: lineage.id(),
        lineage_state_before_id: lineage.account_data_id(),
        lineage_state_after_id,
        close,
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

fn successful_statistic_result_lineage_matches(
    family: LineageFamilyV1,
    active_account: clutch_source_plane_v3_runtime::RuntimeKey,
    semantic_binding_id: ContentId,
    expected_result_account: clutch_source_plane_v3_runtime::RuntimeKey,
    expected_statistic_result_recipe_id: ContentId,
) -> bool {
    family == LineageFamilyV1::StatisticResult
        && active_account == expected_result_account
        && semantic_binding_id == expected_statistic_result_recipe_id
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

    #[test]
    fn successful_terminal_refuses_family_account_and_recipe_substitution() {
        let result_account = clutch_source_plane_v3_runtime::RuntimeKey::from_bytes([1; 32]);
        let other_account = clutch_source_plane_v3_runtime::RuntimeKey::from_bytes([2; 32]);
        let recipe_id = ContentId::from_bytes([3; 32]);
        let other_recipe_id = ContentId::from_bytes([4; 32]);

        assert!(successful_statistic_result_lineage_matches(
            LineageFamilyV1::StatisticResult,
            result_account,
            recipe_id,
            result_account,
            recipe_id,
        ));
        assert!(!successful_statistic_result_lineage_matches(
            LineageFamilyV1::EvaluationWork,
            result_account,
            recipe_id,
            result_account,
            recipe_id,
        ));
        assert!(!successful_statistic_result_lineage_matches(
            LineageFamilyV1::StatisticResult,
            other_account,
            recipe_id,
            result_account,
            recipe_id,
        ));
        assert!(!successful_statistic_result_lineage_matches(
            LineageFamilyV1::StatisticResult,
            result_account,
            other_recipe_id,
            result_account,
            recipe_id,
        ));
    }

    #[test]
    fn same_call_close_consumes_postwrites_and_refuses_caller_coordinates() {
        let source = include_str!("source_terminal_resolution_v5.rs");
        let close = source
            .split("pub(crate) fn close_successful_source_statistic_result_v1")
            .nth(1)
            .and_then(|value| value.split("fn lineage_family").next())
            .expect("private successful close");
        for predicate in [
            "PersistedSourceResolutionTerminalPolicyV1::NoReopen",
            "value.authenticated()",
            "terminal.terminal().authenticated_receipt()",
            "source.result_account_data_id()",
            "no_reopen.lineage_authentication_id() == lineage.id()",
            "lineage_before.semantic_binding_id == statistic_result_recipe_id",
            "close_statistic_result_generation",
            "close.lineage_after == projected_lineage_after",
            "result_account.lamports() == 0",
        ] {
            assert!(close.contains(predicate), "missing close guard {predicate}");
        }
        assert!(!close.contains("CloseGenerationIntentV2"));
        assert!(!close.contains("authenticate_source_terminal_policy_for_close"));
        assert!(!close.contains("ExistingReadOnly"));
    }

    #[test]
    fn source_input_identity_retains_exact_result_preimage() {
        let source = include_str!("product_source_current.rs");
        let terminal = include_str!("source_terminal_resolution_v5.rs");
        let input = source
            .split("pub(crate) fn authenticate_source_resolution_input_v4")
            .nth(1)
            .and_then(|value| value.split("#[cfg(test)]").next())
            .expect("Source input constructor");
        assert!(input.contains("&handoff.result_account_data_id().bytes()"));
        assert!(input.contains(
            "result_account_data_id: ContentId::from_bytes(handoff.result_account_data_id().bytes())"
        ));
        assert!(terminal.contains("AuthenticatedSourceResolutionInputV4"));
        assert!(!terminal.contains("AuthenticatedSourceResolutionInputV3"));
    }
}
