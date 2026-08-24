// SPDX-License-Identifier: AGPL-3.0-or-later
//! Shared hostile authentication for current callable Market Failure actions.
//!
//! This owner rebuilds the mutable Product root/link, current loader-backed
//! Registry graph, market liveness schedule, immutable Failure admission,
//! mutable runtime, and durable interval capitalization on every call. It
//! accepts only content preimages whose IDs are already committed by those
//! program-owned accounts; no caller ID becomes authority.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::failure_market_admission::{
    authenticate_failure_market_root_v2, AuthenticatedFailureMarketRootV2,
};
use crate::instructions::failure_market_interval_v2::{
    authenticate_failure_market_recovery_quote_v1,
    reopen_failure_market_interval_accounts_v2, AuthenticatedFailureMarketIntervalAccountsV2,
    FailureMarketIntervalFundingPreimageV2,
};
use crate::instructions::failure_market_runtime::{
    authenticate_failure_market_runtime_root_v1, AuthenticatedFailureMarketRuntimeRootV1,
};
use crate::instructions::product_artifact::{
    authenticate_product_artifact_v1, authenticate_registry_capability_v3,
    authenticate_series_registry_capability_refs_v2, AuthenticatedProductArtifactV1,
    AuthenticatedRegistryCapabilityV3,
};
use crate::instructions::product_market::{
    authenticate_market_lifecycle_root_v1, authenticate_market_recovery_schedule_v1,
    authenticate_series_market_link_v1, AuthenticatedMarketLifecycleRootV1,
    AuthenticatedSeriesMarketLinkV1,
};
use clutch_failure_policy_runtime::market_quote_v1::FailureMarketRecoveryQuoteAdmissionReceiptV1;
use clutch_product_series::{
    CompiledProductSeriesBundleV5, ContentId, MarketGenesisProfileV2, MarketInstancePreimageV2,
    NativeClaimBasisV1, PriceMeasurePolicyV1, ProductTemplateV4,
    QuantizedIntervalConsensusContextV1, QuantizedIntervalConsensusProfileV1,
    SeriesFundingQuoteV4,
};
use clutch_source_plane_v3::{
    CompiledSourceOccurrenceV3, StatisticKeyV3, StatisticResultV3, SummaryProgramV3, WindowSealV3,
    WindowSpecV3,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV1, SeriesMarketLinkAccountV1,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// Complete current Product/Failure authentication reused by actions 10-13.
#[derive(Debug)]
pub(crate) struct AuthenticatedFailureMarketExecutionV2<'root, 'link> {
    root: AuthenticatedMarketLifecycleRootV1<'root>,
    link: AuthenticatedSeriesMarketLinkV1<'link>,
    registry: AuthenticatedRegistryCapabilityV3,
    bundle: AuthenticatedProductArtifactV1<CompiledProductSeriesBundleV5>,
    admission: AuthenticatedFailureMarketRootV2,
    runtime: AuthenticatedFailureMarketRuntimeRootV1,
    interval: AuthenticatedFailureMarketIntervalAccountsV2,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
}

impl<'root, 'link> AuthenticatedFailureMarketExecutionV2<'root, 'link> {
    pub(crate) const fn root(&self) -> AuthenticatedMarketLifecycleRootV1<'root> {
        self.root
    }
    pub(crate) const fn link(&self) -> AuthenticatedSeriesMarketLinkV1<'link> {
        self.link
    }
    pub(crate) const fn registry(&self) -> AuthenticatedRegistryCapabilityV3 {
        self.registry
    }
    pub(crate) const fn bundle(
        &self,
    ) -> &AuthenticatedProductArtifactV1<CompiledProductSeriesBundleV5> {
        &self.bundle
    }
    pub(crate) const fn admission(&self) -> AuthenticatedFailureMarketRootV2 {
        self.admission
    }
    pub(crate) const fn runtime(&self) -> AuthenticatedFailureMarketRuntimeRootV1 {
        self.runtime
    }
    pub(crate) const fn interval(&self) -> AuthenticatedFailureMarketIntervalAccountsV2 {
        self.interval
    }
    pub(crate) const fn quote(&self) -> FailureMarketRecoveryQuoteAdmissionReceiptV1 {
        self.quote
    }
}

/// Current immutable Product bodies and central work profile admitted for a
/// caller-free interval context.
#[derive(Debug)]
pub(crate) struct AuthenticatedFailureMarketProductContextV2 {
    template: AuthenticatedProductArtifactV1<ProductTemplateV4>,
    basis: AuthenticatedProductArtifactV1<NativeClaimBasisV1>,
    price: AuthenticatedProductArtifactV1<PriceMeasurePolicyV1>,
    genesis: AuthenticatedProductArtifactV1<MarketGenesisProfileV2>,
    market: AuthenticatedProductArtifactV1<MarketInstancePreimageV2>,
    work_profile: QuantizedIntervalConsensusProfileV1,
    resolved_edge_policy: clutch_product_series::QuantizedEdgePolicyV1,
}

impl AuthenticatedFailureMarketProductContextV2 {
    /// Borrow the exact authenticated Product/Source context without
    /// persisting another copy of any body.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn context<'a>(
        &'a self,
        source_occurrence: &'a CompiledSourceOccurrenceV3,
        source_interval: &'a StatisticResultV3,
        statistic_key: &'a StatisticKeyV3,
        summary_program: &'a SummaryProgramV3,
        window_seal: &'a WindowSealV3,
        window: &'a WindowSpecV3,
    ) -> QuantizedIntervalConsensusContextV1<'a> {
        QuantizedIntervalConsensusContextV1 {
            market: self.market.value(),
            product_template: self.template.value(),
            native_claim_basis: self.basis.value(),
            price_measure_policy: self.price.value(),
            market_genesis: self.genesis.value(),
            resolved_edge_policy: self.resolved_edge_policy,
            source_occurrence,
            source_interval,
            statistic_key,
            summary_program,
            window_seal,
            window,
            work_profile: &self.work_profile,
        }
    }
}

/// Authenticate every immutable Product body used by Begin/Advance/Resolve
/// against the current Bundle, Market root, Failure policy, and ProfileV4.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_failure_market_product_context_v2(
    program_id: &Pubkey,
    execution: &AuthenticatedFailureMarketExecutionV2<'_, '_>,
    product_template_account: &AccountInfo<'_>,
    native_claim_basis_account: &AccountInfo<'_>,
    price_measure_policy_account: &AccountInfo<'_>,
    market_genesis_account: &AccountInfo<'_>,
    market_instance_account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedFailureMarketProductContextV2> {
    let bundle = execution.bundle().value();
    let template = authenticate_product_artifact_v1::<ProductTemplateV4>(
        program_id,
        product_template_account,
        bundle.product_template_id.content_id(),
    )?;
    let basis = authenticate_product_artifact_v1::<NativeClaimBasisV1>(
        program_id,
        native_claim_basis_account,
        bundle.native_claim_basis_id.content_id(),
    )?;
    let price = authenticate_product_artifact_v1::<PriceMeasurePolicyV1>(
        program_id,
        price_measure_policy_account,
        bundle.price_measure_policy_id.content_id(),
    )?;
    let genesis = authenticate_product_artifact_v1::<MarketGenesisProfileV2>(
        program_id,
        market_genesis_account,
        bundle.market_genesis_profile_id.content_id(),
    )?;
    let root_binding = execution.root().state().binding();
    let market = authenticate_product_artifact_v1::<MarketInstancePreimageV2>(
        program_id,
        market_instance_account,
        root_binding.market_instance_id.content_id(),
    )?;
    let work_profile = execution
        .registry()
        .profile()
        .interval_consensus_profile()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let policy = execution.admission().state().binding().facts();
    let work_profile_id = work_profile
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        template.semantic_id() == bundle.product_template_id.content_id()
            && basis.semantic_id() == bundle.native_claim_basis_id.content_id()
            && price.semantic_id() == bundle.price_measure_policy_id.content_id()
            && genesis.semantic_id() == bundle.market_genesis_profile_id.content_id()
            && market.semantic_id() == root_binding.market_instance_id.content_id()
            && template.semantic_id().bytes() == policy.product_template_id.bytes()
            && basis.semantic_id().bytes() == policy.native_claim_basis_id.bytes()
            && price.semantic_id().bytes() == policy.price_measure_policy_id.bytes()
            && genesis.semantic_id().bytes() == policy.market_genesis_profile_id.bytes()
            && work_profile_id.bytes() == policy.interval_consensus_profile_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedFailureMarketProductContextV2 {
        template,
        basis,
        price,
        genesis,
        market,
        work_profile,
        resolved_edge_policy: execution.registry().resolved_edge_policy(),
    })
}

/// Reopen the full shared authority prefix for one action.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_failure_market_execution_v2<'root, 'link>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    admission_account: &AccountInfo<'_>,
    runtime_account: &AccountInfo<'_>,
    interval_cell_account: &AccountInfo<'_>,
    interval_history_account: &AccountInfo<'_>,
    series_registry_account: &AccountInfo<'_>,
    registry_program: &AccountInfo<'_>,
    registry_programdata: &AccountInfo<'_>,
    registry_release_artifact: &AccountInfo<'_>,
    capability_profile_artifact: &AccountInfo<'_>,
    compiler_bundle_artifact: &AccountInfo<'_>,
    funding_quote_artifact: &AccountInfo<'_>,
    liveness_policy_account: &AccountInfo<'_>,
    recovery_quote_schedule_body: &[u8],
    interval_funding_preimage_body: &[u8],
    root_writable: bool,
    link_writable: bool,
    interval_cell_writable: bool,
    interval_history_writable: bool,
    root_output: &'root mut MarketLifecycleRootAccountV1,
    link_output: &'link mut SeriesMarketLinkAccountV1,
) -> Outcome<AuthenticatedFailureMarketExecutionV2<'root, 'link>> {
    let admission = authenticate_failure_market_root_v2(program_id, admission_account, false)?;
    let policy = admission.state().binding().facts();
    let root = authenticate_market_lifecycle_root_v1(
        program_id,
        root_account,
        policy.market_instance_id,
        policy.generation,
        root_writable,
        root_output,
    )?;

    // The candidate Series/ordinal is hostile-decoded only to select the
    // canonical link PDA. Registry, Bundle, root, and Failure equalities below
    // independently authenticate every semantic relation.
    {
        let data = link_account
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        SeriesMarketLinkAccountV1::decode_into(&data, link_output)
            .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    }
    let candidate = link_output.state.binding();
    let link = authenticate_series_market_link_v1(
        program_id,
        link_account,
        candidate.series_plan_id,
        candidate.ordinal,
        policy.market_instance_id,
        policy.generation,
        *root_account.key,
        link_writable,
        link_output,
    )?;
    let link_binding = link.state().binding();
    let root_binding = root.state().binding();
    require(
        root_binding.market_failure_policy_binding_id.bytes()
            == admission.state().binding().id().bytes()
            && root_binding.failure_liveness_policy_id.bytes()
                == policy.liveness_policy_id.bytes()
            && root_binding.failure_liveness_quote_schedule_id.bytes()
                == policy.recovery_quote_schedule_id.bytes()
            && link_binding.market_instance_id == root_binding.market_instance_id
            && link_binding.generation == root_binding.generation,
        ClutchError::MismatchedState,
    )?;
    let registry_refs = authenticate_series_registry_capability_refs_v2(
        program_id,
        series_registry_account,
        link_binding.series_plan_id,
    )?;
    let registry = authenticate_registry_capability_v3(
        program_id,
        registry_refs,
        registry_program,
        registry_programdata,
        registry_release_artifact,
        capability_profile_artifact,
    )?;
    let bundle = authenticate_product_artifact_v1::<CompiledProductSeriesBundleV5>(
        program_id,
        compiler_bundle_artifact,
        registry.compiler_bundle_id(),
    )?;
    let bundle_value = bundle.value();
    require(
        bundle_value.series_plan_id == link_binding.series_plan_id
            && bundle_value.funding_terms_id == registry.funding_terms_id()
            && bundle_value.funding_terms_id == link_binding.funding_terms_id
            && bundle_value.funding_quote_id == link_binding.funding_quote_id
            && bundle_value.attachment_plan_id == link_binding.attachment_plan_id
            && bundle_value.registry_release_id == registry.registry_release_id()
            && bundle_value.capability_profile_id.bytes()
                == registry.capability_profile_id().bytes()
            && bundle_value.registry_release_id == root_binding.registry_release_id
            && bundle_value.capability_profile_id.bytes()
                == root_binding.capability_profile_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    let product_quote = authenticate_market_recovery_schedule_v1(
        program_id,
        root,
        link,
        registry,
        funding_quote_artifact,
        liveness_policy_account,
    )?;
    require(
        product_quote.funding_quote_id() == bundle_value.funding_quote_id.content_id()
            && product_quote.market_root_authentication_id() == root.authentication_id()
            && product_quote.series_link_authentication_id() == link.authentication_id(),
        ClutchError::MismatchedState,
    )?;
    let quote = authenticate_failure_market_recovery_quote_v1(
        admission,
        product_quote,
        recovery_quote_schedule_body,
    )?;
    let funding_preimage =
        FailureMarketIntervalFundingPreimageV2::decode(interval_funding_preimage_body)?;
    let interval = reopen_failure_market_interval_accounts_v2(
        program_id,
        interval_cell_account,
        interval_history_account,
        admission,
        quote,
        funding_preimage,
        interval_cell_writable,
        interval_history_writable,
    )?;
    let runtime = authenticate_failure_market_runtime_root_v1(
        program_id,
        admission_account,
        runtime_account,
        admission,
        true,
    )?;
    require(
        runtime.state().active_interval_funding_receipt_id().is_zero()
            || runtime.state().active_interval_funding_receipt_id().bytes()
                == interval.funding().id().bytes(),
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedFailureMarketExecutionV2 {
        root,
        link,
        registry,
        bundle,
        admission,
        runtime,
        interval,
        quote,
    })
}

#[cfg(test)]
mod adversarial_join_tests {
    #[test]
    fn shared_join_contains_every_current_splice_guard() {
        let source = include_str!("failure_market_execution_v2.rs");
        for predicate in [
            "market_failure_policy_binding_id",
            "failure_liveness_quote_schedule_id",
            "bundle_value.funding_terms_id == link_binding.funding_terms_id",
            "bundle_value.funding_quote_id == link_binding.funding_quote_id",
            "bundle_value.attachment_plan_id == link_binding.attachment_plan_id",
            "product_quote.market_root_authentication_id() == root.authentication_id()",
            "product_quote.series_link_authentication_id() == link.authentication_id()",
            "reopen_failure_market_interval_accounts_v2",
        ] {
            assert!(source.contains(predicate));
        }
    }

    #[test]
    fn caller_preimages_never_bypass_semantic_owners() {
        let source = include_str!("failure_market_execution_v2.rs");
        assert!(source.contains("authenticate_failure_market_recovery_quote_v1"));
        assert!(source.contains("FailureMarketIntervalFundingPreimageV2::decode"));
        assert!(!source.contains("FailureMarketRecoveryQuoteAdmissionReceiptV1 {"));
        assert!(!source.contains("FailureMarketIntervalFundingReceiptV2 {"));
    }
}
