//! Sole current whole-Series retirement owner.
//!
//! Failure/Source first mint one exhaustive lifecycle terminal against the
//! still-live `Retiring` LinkV2. Product then derives the exact RootV2/LinkV2
//! counted successor and a private prewrite authority. Source consumes that
//! authority while physically closing `0xbd`; only the resulting move-only
//! postwrite may advance Product state and eventually dispose FundingV4.
//!
//! Source authenticates its versioned durable terminal before this boundary.
//! This module deliberately cannot decode an older terminal body or synthesize
//! an identity omitted by that body.

use core::cell::Cell;

use super::{
    authenticate_series_funding_account_v4, authenticate_series_registry_account_v3,
    AuthenticatedMarketLifecycleRootV2, AuthenticatedRegistryCapabilityV4,
    AuthenticatedSeriesFundingAccountV4, AuthenticatedSeriesLifecycleReplayV2,
    AuthenticatedSeriesMarketLinkV2,
};
use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::SYSTEM_PROGRAM_ID;
use crate::instructions::product_source_current::AuthenticatedSeriesSourceArtifactsV5;
use crate::instructions::source_funding_custody_retirement_v1::{
    AuthenticatedSourceFundingCustodyLifecycleTerminalV1,
    AuthenticatedSourceFundingCustodyRetirementAuthorityV2,
    AuthenticatedSourceFundingCustodyRetirementV2,
    retire_source_funding_custody_v2,
    SourceFundingCustodyLifecycleTerminalFactsV1, SourceFundingCustodyProductReleaseFactsV3,
    SourceFundingCustodyRetirementAccountingV2, SourceFundingCustodyRetirementFactsV2,
};
use clutch_product_series::{
    AuthenticatedSeriesFundingAuthorityV4, ComponentDebitV1, ContentId,
    MarketLifecyclePhaseV2, MarketLifecycleRootV2, SeriesFundingAbortBindingV4,
    SeriesFundingComponentV2, SeriesFundingCompletionBindingV4, SeriesFundingPhaseV4,
    SeriesFundingQuoteV5, SeriesFundingReservationBindingV4, SeriesFundingStateV4,
    SeriesFundingStateV4Id, SeriesFundingTerminalProjectionV4,
    SeriesLifecycleReplayBindingV2Id, SeriesLifecycleTerminalEvidenceV2,
    SeriesLifecycleTerminalProjectionV2,
    SeriesLifecycleLinkRetirementProjectionV2, SeriesLifecycleReplayPhaseV2,
    SeriesMarketLinkPhaseV2, SeriesAttachmentPlanV5,
    SeriesMarketLinkRetirementProjectionV2, SeriesMarketLinkV2, SeriesMarketLinkV2Id,
    SeriesPlanV5, SeriesPlanV5Id, CompiledProductSeriesBundleV6Id,
    SeriesFundingTermsV2Id, SERIES_FUNDING_COMPONENT_COUNT_V2,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV2, SeriesMarketLinkAccountV2,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;
use clutch_source_plane_v3_runtime::{
    AuthenticatedSourceRouteV1, SourceWorkScheduleBindingV1,
};

const PRODUCT_SERIES_COUNTED_RETIREMENT_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/product-series-counted-retirement/v4\0";
const PRODUCT_SERIES_RETIREMENT_PREAUTHORIZATION_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/product-series-retirement-preauthorization/v4\0";
const PRODUCT_SERIES_LINK_RETIREMENT_POSTWRITE_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/product-series-link-retirement-postwrite/v4\0";
const PRODUCT_SERIES_TERMINAL_AUTHORITY_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/product-series-terminal-authority/v4\0";
const PRODUCT_SERIES_LIFECYCLE_TERMINAL_POSTWRITE_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/product-series-lifecycle-terminal-postwrite/v4\0";

fn hashv(values: &[&[u8]]) -> ContentId {
    ContentId::from_bytes(solana_sha256_hasher::hashv(values).to_bytes())
}

fn require_live(id: ContentId) -> Outcome<()> {
    require(!id.is_zero(), ClutchError::MismatchedState)
}

/// Typed Product proof that one exact `Retiring` LinkV2 is the next counted
/// root successor. Source receives only its ID in the accounting body.
#[derive(Debug, Eq, PartialEq)]
struct ProductCountedSeriesLinkRetirementProjectionV4 {
    id: ContentId,
    link_retirement: SeriesMarketLinkRetirementProjectionV2,
    root_account: Pubkey,
    root_authentication_before: ContentId,
    root_data_before: ContentId,
    root_semantic_before: ContentId,
    root_semantic_after: ContentId,
    root_transition_sequence_before: u64,
    root_transition_sequence_after: u64,
    link_account: Pubkey,
    link_authentication_before: ContentId,
    link_data_before: ContentId,
    link_semantic_before: SeriesMarketLinkV2Id,
    link_semantic_after: SeriesMarketLinkV2Id,
    link_transition_sequence_before: u64,
    link_transition_sequence_after: u64,
}

/// Move-only exact physical close of the final live LinkV2 and its Source
/// custody, plus the hostile RootV2/ReplayV2 postwrites which counted it.
#[derive(Debug)]
struct AuthenticatedProductSeriesLinkRetirementV4 {
    id: ContentId,
    source: AuthenticatedSourceFundingCustodyRetirementV2,
    counted_id: ContentId,
    root_account: Pubkey,
    root_authentication_before: ContentId,
    root_authentication_after: ContentId,
    root_data_before: ContentId,
    root_data_after: ContentId,
    root_semantic_before: ContentId,
    root_semantic_after: ContentId,
    root_transition_sequence_before: u64,
    root_transition_sequence_after: u64,
    link_account: Pubkey,
    link_authentication_before: ContentId,
    link_authentication_retired: ContentId,
    link_data_before: ContentId,
    link_data_retired: ContentId,
    link_semantic_before: SeriesMarketLinkV2Id,
    link_semantic_retired: SeriesMarketLinkV2Id,
    link_transition_sequence_before: u64,
    link_transition_sequence_after: u64,
    link_retirement_projection_id: ContentId,
    link_observed_lamports: u64,
    link_rent_principal_lamports: u64,
    link_surplus_lamports: u64,
    refund_account: Pubkey,
    refund_balance_before: u64,
    refund_balance_after: u64,
    neutral_sink: Pubkey,
    neutral_sink_balance_before: u64,
    neutral_sink_balance_after: u64,
    replay_account: Pubkey,
    replay_binding_id: SeriesLifecycleReplayBindingV2Id,
    replay_authentication_before: ContentId,
    replay_authentication_after: ContentId,
    replay_data_before: ContentId,
    replay_data_after: ContentId,
    replay_state_before: ContentId,
    replay_state_after: ContentId,
    replay_retirement_projection_id: ContentId,
    replay: AuthenticatedSeriesLifecycleReplayV2,
    registry_account: Pubkey,
    registry_authentication_id: ContentId,
    registry_release_id: ContentId,
    capability_profile_id: ContentId,
    funding_account: Pubkey,
    funding_state_id: SeriesFundingStateV4Id,
    funding_data_id: ContentId,
    funding_authentication_id: ContentId,
    series_plan_id: SeriesPlanV5Id,
    funding_terms_id: ContentId,
    compiler_bundle_id: ContentId,
}

/// Private terminal seal. It owns the exact Source/Link close and the hostile
/// Terminal ReplayV2 postwrite; the physical FundingV4 disposer must consume
/// it by value before any receipt can escape the instruction.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductSeriesLifecycleTerminalV4 {
    id: ContentId,
    terminal_authority_id: ContentId,
    link_retirement: AuthenticatedProductSeriesLinkRetirementV4,
    funding: AuthenticatedSeriesFundingAccountV4,
    funding_terminal_projection: SeriesFundingTerminalProjectionV4,
    funding_terminal_projection_id: ContentId,
    registry_data_id: ContentId,
    registry_authentication_id: ContentId,
    replay_account: Pubkey,
    replay_binding_id: SeriesLifecycleReplayBindingV2Id,
    replay_data_before: ContentId,
    replay_data_after: ContentId,
    replay_authentication_before: ContentId,
    replay_authentication_after: ContentId,
    replay_state_before: ContentId,
    replay_state_after: ContentId,
    replay_terminal_projection: SeriesLifecycleTerminalProjectionV2,
    replay: AuthenticatedSeriesLifecycleReplayV2,
}

impl AuthenticatedProductSeriesLifecycleTerminalV4 {
    /// Exact pure terminal amounts authorized by the Product replay seal.
    pub(crate) const fn funding_terminal_projection(
        &self,
    ) -> SeriesFundingTerminalProjectionV4 {
        self.funding_terminal_projection
    }

    /// Require the physical layer to be operating on the same freshly reopened
    /// current Registry/Funding graph and terminal projection. No ID-only
    /// physical constructor can satisfy this boundary.
    pub(crate) fn authenticate_physical_preflight_v4(
        &self,
        registry: &AuthenticatedRegistryCapabilityV4,
        funding: &AuthenticatedSeriesFundingAccountV4,
        projection: SeriesFundingTerminalProjectionV4,
    ) -> Outcome<ContentId> {
        let projection_id = projection
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            self.id != ContentId::ZERO
                && self.terminal_authority_id == projection.terminal_receipt_id
                && self.funding_terminal_projection == projection
                && self.funding_terminal_projection_id == projection_id
                && &self.funding == funding
                && registry.activation_consumed()
                && registry.series_registry_account()
                    == self.link_retirement.registry_account
                && registry.series_registry_authentication_id()
                    == self.registry_authentication_id
                && registry.registry_release_id()
                    == self.link_retirement.registry_release_id
                && registry.capability_profile_id()
                    == self.link_retirement.capability_profile_id
                && funding.account() == self.link_retirement.funding_account
                && funding.data_id() == self.link_retirement.funding_data_id
                && funding.authentication_id()
                    == self.link_retirement.funding_authentication_id
                && funding.state().id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                    == self.link_retirement.funding_state_id
                && self.replay.state().phase() == SeriesLifecycleReplayPhaseV2::Terminal
                && self.replay.account() == self.replay_account
                && self.replay.authentication_id() == self.replay_authentication_after
                && self.replay.data_id() == self.replay_data_after
                && self.replay.state().id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                    .content_id()
                    == self.replay_state_after
                && self.replay_terminal_projection.id().content_id()
                    == self.replay.state().terminal_projection_id(),
            ClutchError::MismatchedState,
        )?;
        Ok(self.id)
    }
}

/// Close-only pure authority bound to the exact hostile Closed FundingV4
/// state and the private Product terminal prewrite.
#[derive(Debug)]
struct ExactProductSeriesFundingTerminalAuthorityV4 {
    id: ContentId,
    expected_state: SeriesFundingStateV4,
}

impl AuthenticatedSeriesFundingAuthorityV4 for ExactProductSeriesFundingTerminalAuthorityV4 {
    fn authenticate_activation(
        &self,
        _series: &SeriesPlanV5,
        _funding_terms_id: SeriesFundingTermsV2Id,
        _compiler_bundle_id: CompiledProductSeriesBundleV6Id,
        _quote: &SeriesFundingQuoteV5,
        _attachment: &SeriesAttachmentPlanV5,
        _principal: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
        _donations: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn current_bucket(&self, _series: &SeriesPlanV5) -> clutch_product_series::Result<u64> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_reservation(
        &self,
        _state: &SeriesFundingStateV4,
        _binding: &SeriesFundingReservationBindingV4,
        _reservation_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_pending_completion(
        &self,
        _state: &SeriesFundingStateV4,
        _binding: &SeriesFundingCompletionBindingV4,
        _completion_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_pending_abort(
        &self,
        _state: &SeriesFundingStateV4,
        _binding: &SeriesFundingAbortBindingV4,
        _abort_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_donation(
        &self,
        _state: &SeriesFundingStateV4,
        _component: SeriesFundingComponentV2,
        _amount: ComponentDebitV1,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_close(
        &self,
        state: &SeriesFundingStateV4,
        terminal_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        if self.id.is_zero()
            || terminal_receipt_id != self.id
            || state != &self.expected_state
            || state.phase != SeriesFundingPhaseV4::Closed
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
    }
}

impl ProductCountedSeriesLinkRetirementProjectionV4 {
    const fn id(&self) -> ContentId {
        self.id
    }
}

/// Move-only Product prewrite. Its private Source authority is accepted once;
/// merely knowing any retained digest cannot authorize custody movement.
#[derive(Debug)]
struct AuthenticatedProductSeriesRetirementPreauthorizationV4 {
    id: ContentId,
    series_plan_id: SeriesPlanV5Id,
    funding_terms_id: ContentId,
    compiler_bundle_id: ContentId,
    registry_account: Pubkey,
    registry_authentication_id: ContentId,
    registry_release_id: ContentId,
    capability_profile_id: ContentId,
    funding_account: Pubkey,
    funding_state_id: SeriesFundingStateV4Id,
    funding_data_id: ContentId,
    funding_authentication_id: ContentId,
    replay_account: Pubkey,
    replay_binding_id: SeriesLifecycleReplayBindingV2Id,
    replay_state_id: ContentId,
    replay_data_id: ContentId,
    replay_authentication_id: ContentId,
    source_terminal_id: ContentId,
    source_terminal_facts: SourceFundingCustodyLifecycleTerminalFactsV1,
    source_product_release_facts: SourceFundingCustodyProductReleaseFactsV3,
    counted: ProductCountedSeriesLinkRetirementProjectionV4,
    root_successor: Box<MarketLifecycleRootV2>,
    link_successor: Box<SeriesMarketLinkV2>,
    accepted_source_retirement: Cell<bool>,
}

impl AuthenticatedProductSeriesRetirementPreauthorizationV4 {
    const fn id(&self) -> ContentId {
        self.id
    }

    fn source_accounting(&self) -> SourceFundingCustodyRetirementAccountingV2 {
        SourceFundingCustodyRetirementAccountingV2 {
            funding_terms_id: self.funding_terms_id,
            product_retirement_authority_id: self.id,
            counted_retirement_receipt_id: self.counted.id(),
            source_funding_custody: self.source_terminal_facts.source_funding_custody,
            lamport_principal_refund: self.source_terminal_facts.lamport_principal_refund,
            neutral_lamport_sink: self.source_terminal_facts.neutral_lamport_sink,
        }
    }
}

impl AuthenticatedSourceFundingCustodyRetirementAuthorityV2
    for AuthenticatedProductSeriesRetirementPreauthorizationV4
{
    fn authenticate_source_funding_custody_retirement_v2(
        &self,
        facts: SourceFundingCustodyRetirementFactsV2,
    ) -> Outcome<ContentId> {
        require(
            !self.accepted_source_retirement.replace(true)
                && facts.accounting == self.source_accounting()
                && facts.lifecycle_terminal_authentication_id == self.source_terminal_id
                && facts.lifecycle_terminal == self.source_terminal_facts
                && facts.product_release == self.source_product_release_facts
                && facts.lifecycle_terminal.series_plan_id.bytes() == self.series_plan_id.bytes()
                && facts.lifecycle_terminal.product_link_account.bytes()
                    == self.counted.link_account.to_bytes()
                && facts.lifecycle_terminal.product_link_authentication_id
                    == self.counted.link_authentication_before
                && facts.lifecycle_terminal.product_link_semantic_id.bytes()
                    == self.counted.link_semantic_before.bytes()
                && facts.capitalization_receipt_id
                    == self.source_terminal_facts.capitalization_receipt_id,
            ClutchError::MismatchedState,
        )?;
        Ok(self.id)
    }
}

/// Derive the sole acyclic retirement prewrite from hostile current Product
/// accounts and the final Failure-derived Source lifecycle capability.
/// No count, account identity, receipt ID, or amount is caller supplied.
#[allow(clippy::too_many_arguments)]
fn preauthorize_product_series_retirement_v4(
    registry: &AuthenticatedRegistryCapabilityV4,
    artifacts: &AuthenticatedSeriesSourceArtifactsV5,
    root: AuthenticatedMarketLifecycleRootV2<'_>,
    link: AuthenticatedSeriesMarketLinkV2<'_>,
    replay: AuthenticatedSeriesLifecycleReplayV2,
    funding: &AuthenticatedSeriesFundingAccountV4,
    source_terminal: &AuthenticatedSourceFundingCustodyLifecycleTerminalV1,
) -> Outcome<AuthenticatedProductSeriesRetirementPreauthorizationV4> {
    artifacts.validate_registry_projection(&registry.projection())?;
    let series = artifacts.series();
    let quote = artifacts.quote();
    let attachment = artifacts.attachment();
    let terms = artifacts.funding_terms();
    let series_plan_id = series
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_terms_id = terms
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let quote_id = quote
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let attachment_id = attachment
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    funding
        .state()
        .validate_against(series, quote, attachment)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let root_state = root.state();
    let root_binding = root_state.binding();
    let root_binding_id = root_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_semantic_before = root_state
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_state = link.state();
    let link_binding = link_state.binding();
    let link_binding_id = link_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_semantic_before = link_state
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let replay_state = replay.state();
    let replay_binding = replay_state.binding();
    let replay_binding_id = replay_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let replay_state_id = replay_state
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    let funding_state_id = funding
        .state()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let source_terminal_id = source_terminal.id();
    let source_terminal_facts = source_terminal.facts();
    let source_product_release_facts = source_terminal.product_release_facts();
    let source_product_release_facts_id = source_product_release_facts.id();

    require(
        registry.activation_consumed()
            && registry.series_plan_id() == series_plan_id
            && registry.funding_terms_id() == funding_terms_id
            && registry.compiler_bundle_id() == replay_binding.compiler_bundle_id
            && root.is_writable()
            && link.is_writable()
            && replay.is_writable()
            && funding.is_writable()
            && root_state.phase() == MarketLifecyclePhaseV2::Active
            && link_state.phase() == SeriesMarketLinkPhaseV2::Retiring
            && replay_state.phase() == SeriesLifecycleReplayPhaseV2::Open
            && funding.state().phase == SeriesFundingPhaseV4::Closed
            && link_binding.market_root_account_id.bytes() == root.account().to_bytes()
            && link_binding.market_binding_id == root_binding_id
            && link_binding.market_instance_id == root_binding.market_instance_id
            && link_binding.generation == root_binding.generation
            && link_binding.series_plan_id == series_plan_id
            && link_binding.funding_terms_id == funding_terms_id
            && link_binding.funding_quote_id == quote_id
            && link_binding.attachment_plan_id == attachment_id
            && link_binding.compiler_bundle_id == replay_binding.compiler_bundle_id
            && link_binding.capability_profile_id == registry.capability_profile_id()
            && link_binding.funding_state_account_id.bytes() == funding.account().to_bytes()
            && link_binding.rent_refund_owner == terms.lamport_principal_refund
            && link_binding.neutral_lamport_sink == terms.neutral_lamport_sink
            && replay_binding.series_plan_id == series_plan_id
            && replay_binding.funding_terms_id == funding_terms_id
            && replay_binding.funding_quote_id == quote_id
            && replay_binding.attachment_plan_id == attachment_id
            && replay_binding.registry_account_id.bytes()
                == registry.series_registry_account().to_bytes()
            && replay_binding.funding_account_id.bytes() == funding.account().to_bytes()
            && replay_binding.lifecycle_replay_account_id.bytes() == replay.account().to_bytes()
            && replay_binding.registry_release_id.bytes() == registry.registry_release_id().bytes()
            && replay_binding.capability_profile_id.bytes()
                == registry.capability_profile_id().bytes()
            && replay_binding.neutral_lamport_sink == terms.neutral_lamport_sink
            && funding.state().series_plan_id == series_plan_id
            && funding.state().funding_terms_id == funding_terms_id
            && funding.state().funding_quote_id == quote_id
            && funding.state().attachment_plan_id == attachment_id
            && funding.state().compiler_bundle_id == replay_binding.compiler_bundle_id
            && source_terminal_facts.product_link_account.bytes() == link.account().to_bytes()
            && source_terminal_facts.product_link_authentication_id == link.authentication_id()
            && source_terminal_facts.product_link_semantic_id.bytes()
                == link_semantic_before.bytes()
            && source_terminal_facts.market_instance_id.bytes()
                == link_binding.market_instance_id.bytes()
            && source_terminal_facts.series_plan_id.bytes() == series_plan_id.bytes()
            && source_terminal_facts.ordinal == link_binding.ordinal
            && source_terminal_facts.source_generation == link_binding.generation
            && source_terminal_facts.source_release_manifest_id == link_binding.source_release_id
            && source_terminal_facts.source_route_id == link_binding.source_route_id
            && source_terminal_facts.source_occurrence_id.bytes()
                == link_binding.source_occurrence_id.bytes()
            && source_terminal_facts.source_occurrence_account.bytes()
                == link_binding.source_occurrence_account_id.bytes()
            && source_terminal_facts.source_occurrence_authentication_id
                == link_binding.source_occurrence_account_authentication_id
            && source_terminal_facts.source_repair_generation
                == link_binding.source_repair_generation
            && source_terminal_facts.lamport_principal_refund.bytes()
                == terms.lamport_principal_refund.bytes()
            && source_terminal_facts.neutral_lamport_sink.bytes()
                == terms.neutral_lamport_sink.bytes(),
        ClutchError::MismatchedState,
    )?;

    let link_retirement = link_state
        .retirement_projection()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut root_successor = Box::new(MarketLifecycleRootV2::decode_buffer());
    root_state
        .retire_series_link_into(link_retirement, &mut root_successor)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_semantic_after = root_successor
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut link_successor = Box::new(SeriesMarketLinkV2::decode_buffer());
    link_state
        .mark_retired_into(link_retirement, &mut link_successor)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_semantic_after = link_successor
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let counted_id = hashv(&[
        PRODUCT_SERIES_COUNTED_RETIREMENT_DOMAIN_V4,
        root.account().as_ref(),
        &root.authentication_id().bytes(),
        &root_semantic_before.bytes(),
        &root_semantic_after.bytes(),
        &root_state.transition_sequence().to_le_bytes(),
        &root_successor.transition_sequence().to_le_bytes(),
        link.account().as_ref(),
        &link.authentication_id().bytes(),
        &link_semantic_before.bytes(),
        &link_semantic_after.bytes(),
        &link_state.transition_sequence().to_le_bytes(),
        &link_successor.transition_sequence().to_le_bytes(),
        &link_retirement.id().bytes(),
        &link_binding_id.bytes(),
        &replay_binding_id.bytes(),
    ]);
    require_live(counted_id)?;
    let counted = ProductCountedSeriesLinkRetirementProjectionV4 {
        id: counted_id,
        link_retirement,
        root_account: root.account(),
        root_authentication_before: root.authentication_id(),
        root_data_before: root.data_id(),
        root_semantic_before,
        root_semantic_after,
        root_transition_sequence_before: root_state.transition_sequence(),
        root_transition_sequence_after: root_successor.transition_sequence(),
        link_account: link.account(),
        link_authentication_before: link.authentication_id(),
        link_data_before: link.data_id(),
        link_semantic_before,
        link_semantic_after,
        link_transition_sequence_before: link_state.transition_sequence(),
        link_transition_sequence_after: link_successor.transition_sequence(),
    };
    let id = hashv(&[
        PRODUCT_SERIES_RETIREMENT_PREAUTHORIZATION_DOMAIN_V4,
        &counted.id().bytes(),
        &source_terminal_id.bytes(),
        &source_product_release_facts_id.bytes(),
        registry.series_registry_account().as_ref(),
        &registry.series_registry_authentication_id().bytes(),
        &registry.registry_release_id().bytes(),
        &registry.capability_profile_id().bytes(),
        funding.account().as_ref(),
        &funding_state_id.bytes(),
        &funding.data_id().bytes(),
        &funding.authentication_id().bytes(),
        replay.account().as_ref(),
        &replay_binding_id.bytes(),
        &replay_state_id.bytes(),
        &replay.data_id().bytes(),
        &replay.authentication_id().bytes(),
        &series_plan_id.bytes(),
        &funding_terms_id.bytes(),
        &replay_binding.compiler_bundle_id.bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductSeriesRetirementPreauthorizationV4 {
        id,
        series_plan_id,
        funding_terms_id: funding_terms_id.content_id(),
        compiler_bundle_id: replay_binding.compiler_bundle_id.content_id(),
        registry_account: registry.series_registry_account(),
        registry_authentication_id: registry.series_registry_authentication_id(),
        registry_release_id: registry.registry_release_id(),
        capability_profile_id: registry.capability_profile_id(),
        funding_account: funding.account(),
        funding_state_id,
        funding_data_id: funding.data_id(),
        funding_authentication_id: funding.authentication_id(),
        replay_account: replay.account(),
        replay_binding_id,
        replay_state_id,
        replay_data_id: replay.data_id(),
        replay_authentication_id: replay.authentication_id(),
        source_terminal_id,
        source_terminal_facts,
        source_product_release_facts,
        counted,
        root_successor,
        link_successor,
        accepted_source_retirement: Cell::new(false),
    })
}

fn debit_program_lamports(account: &AccountInfo<'_>, amount: u64) -> Outcome<()> {
    let after = account
        .lamports()
        .checked_sub(amount)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let mut lamports = account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **lamports = after;
    Ok(())
}

fn credit_program_lamports(account: &AccountInfo<'_>, amount: u64) -> Outcome<()> {
    let after = account
        .lamports()
        .checked_add(amount)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let mut lamports = account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **lamports = after;
    Ok(())
}

fn require_lamport_destination(account: &AccountInfo<'_>, expected: Pubkey) -> Outcome<()> {
    require(
        *account.key == expected
            && account.is_writable
            && !account.is_signer
            && !account.executable
            && account.owner == &SYSTEM_PROGRAM_ID
            && account.data_is_empty(),
        ClutchError::MismatchedState,
    )
}

/// Consume the Source close, persist the exact counted RootV2/LinkV2
/// successor, physically close the canonical 0xad, and roll ReplayV2 once.
/// This is private because terminal Replay/Funding close must consume its
/// move-only result before the outer may return.
#[allow(clippy::too_many_arguments)]
fn retire_last_product_series_link_v4<'root, 'link>(
    program_id: &Pubkey,
    preauthorization: AuthenticatedProductSeriesRetirementPreauthorizationV4,
    source: AuthenticatedSourceFundingCustodyRetirementV2,
    root_account: &AccountInfo<'_>,
    root: AuthenticatedMarketLifecycleRootV2<'_>,
    link_account: &AccountInfo<'_>,
    link: AuthenticatedSeriesMarketLinkV2<'_>,
    replay_account: &AccountInfo<'_>,
    replay: AuthenticatedSeriesLifecycleReplayV2,
    refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    root_rebound_output: &'root mut MarketLifecycleRootAccountV2,
    link_rebound_output: &'link mut SeriesMarketLinkAccountV2,
) -> Outcome<AuthenticatedProductSeriesLinkRetirementV4> {
    let counted = &preauthorization.counted;
    let source_facts = source.facts();
    require(
        preauthorization.accepted_source_retirement.get()
            && source.product_retirement_authority_id() == preauthorization.id()
            && source_facts.accounting == preauthorization.source_accounting()
            && source_facts.lifecycle_terminal_authentication_id
                == preauthorization.source_terminal_id
            && source_facts.lifecycle_terminal == preauthorization.source_terminal_facts
            && source_facts.product_release
                == preauthorization.source_product_release_facts
            && *root_account.key == counted.root_account
            && root.account() == counted.root_account
            && root.authentication_id() == counted.root_authentication_before
            && root.data_id() == counted.root_data_before
            && *link_account.key == counted.link_account
            && link.account() == counted.link_account
            && link.authentication_id() == counted.link_authentication_before
            && link.data_id() == counted.link_data_before
            && *replay_account.key == preauthorization.replay_account
            && replay.account() == preauthorization.replay_account
            && replay
                .state()
                .binding()
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == preauthorization.replay_binding_id
            && replay.authentication_id() == preauthorization.replay_authentication_id
            && replay.data_id() == preauthorization.replay_data_id
            && replay.state().id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .content_id()
                == preauthorization.replay_state_id,
        ClutchError::MismatchedState,
    )?;
    require_lamport_destination(
        refund,
        Pubkey::new_from_array(preauthorization.source_terminal_facts
            .lamport_principal_refund.bytes()),
    )?;
    require_lamport_destination(
        neutral_sink,
        Pubkey::new_from_array(preauthorization.source_terminal_facts.neutral_lamport_sink.bytes()),
    )?;
    require(
        root_account.key != link_account.key
            && root_account.key != replay_account.key
            && root_account.key != refund.key
            && root_account.key != neutral_sink.key
            && link_account.key != replay_account.key
            && link_account.key != refund.key
            && link_account.key != neutral_sink.key
            && replay_account.key != refund.key
            && replay_account.key != neutral_sink.key
            && refund.key != neutral_sink.key,
        ClutchError::AccountAlias,
    )?;
    let link_observed_lamports = link.observed_lamports();
    let link_rent_principal_lamports = link.state().rent_principal_lamports();
    let link_surplus_lamports = link_observed_lamports
        .checked_sub(link_rent_principal_lamports)
        .ok_or(Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    require(
        link_observed_lamports == link_account.lamports()
            && link_surplus_lamports >= link.state().current_donation_lamports(),
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let refund_balance_before = refund.lamports();
    let refund_balance_after = refund_balance_before
        .checked_add(link_rent_principal_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let neutral_sink_balance_before = neutral_sink.lamports();
    let neutral_sink_balance_after = neutral_sink_balance_before
        .checked_add(link_surplus_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;

    let rebound_link = super::write_series_market_link_v2(
        program_id,
        link_account,
        link,
        &preauthorization.link_successor,
        link_rebound_output,
    )?;
    let rebound_root = super::write_market_lifecycle_root_v2(
        program_id,
        root_account,
        root,
        &preauthorization.root_successor,
        root_rebound_output,
    )?;
    let root_semantic_after = rebound_root
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_semantic_retired = rebound_link
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        root_semantic_after == counted.root_semantic_after
            && rebound_root.authentication_id() != counted.root_authentication_before
            && rebound_root.data_id() != counted.root_data_before
            && rebound_root.state().transition_sequence()
                == counted.root_transition_sequence_after
            && link_semantic_retired == counted.link_semantic_after
            && rebound_link.authentication_id() != counted.link_authentication_before
            && rebound_link.data_id() != counted.link_data_before
            && rebound_link.state().transition_sequence()
                == counted.link_transition_sequence_after
            && rebound_link.state().phase() == SeriesMarketLinkPhaseV2::Retired,
        ClutchError::MismatchedState,
    )?;

    debit_program_lamports(link_account, link_observed_lamports)?;
    credit_program_lamports(refund, link_rent_principal_lamports)?;
    credit_program_lamports(neutral_sink, link_surplus_lamports)?;
    link_account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    link_account.assign(&SYSTEM_PROGRAM_ID);
    require(
        link_account.lamports() == 0
            && link_account.data_is_empty()
            && link_account.owner == &SYSTEM_PROGRAM_ID
            && refund.lamports() == refund_balance_after
            && neutral_sink.lamports() == neutral_sink_balance_after,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let product_retirement_facts_id = hashv(&[
        PRODUCT_SERIES_LINK_RETIREMENT_POSTWRITE_DOMAIN_V4,
        &preauthorization.id().bytes(),
        &source.id().bytes(),
        root_account.key.as_ref(),
        &counted.root_authentication_before.bytes(),
        &rebound_root.authentication_id().bytes(),
        &counted.root_data_before.bytes(),
        &rebound_root.data_id().bytes(),
        &counted.root_semantic_before.bytes(),
        &root_semantic_after.bytes(),
        link_account.key.as_ref(),
        &counted.link_authentication_before.bytes(),
        &rebound_link.authentication_id().bytes(),
        &counted.link_data_before.bytes(),
        &rebound_link.data_id().bytes(),
        &counted.link_semantic_before.bytes(),
        &link_semantic_retired.bytes(),
        &counted.link_retirement.id().bytes(),
        &link_observed_lamports.to_le_bytes(),
        &link_rent_principal_lamports.to_le_bytes(),
        &link_surplus_lamports.to_le_bytes(),
        refund.key.as_ref(),
        &refund_balance_before.to_le_bytes(),
        &refund_balance_after.to_le_bytes(),
        neutral_sink.key.as_ref(),
        &neutral_sink_balance_before.to_le_bytes(),
        &neutral_sink_balance_after.to_le_bytes(),
    ]);
    require_live(product_retirement_facts_id)?;
    let link_binding = link.state().binding();
    let replay_event = SeriesLifecycleLinkRetirementProjectionV2 {
        binding_id: preauthorization.replay_binding_id,
        series_plan_id: preauthorization.series_plan_id,
        ordinal: link_binding.ordinal,
        link_account_id: ContentId::from_bytes(link_account.key.to_bytes()),
        market_root_account_id: ContentId::from_bytes(root_account.key.to_bytes()),
        market_instance_id: link_binding.market_instance_id,
        product_retirement_facts_id,
        link_retirement_projection_id: counted.link_retirement.id(),
        market_admission_receipt_id: counted
            .link_retirement
            .market_admission_receipt_id(),
        generation: link_binding.generation,
    };
    let replay_retirement_projection_id = replay_event
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let replay_state_before = replay
        .state()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    let replay_successor = replay
        .state()
        .record_link_retirement(replay_event)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let rebound_replay = super::write_series_lifecycle_replay_v2(
        program_id,
        replay_account,
        replay,
        replay_successor,
    )?;
    let replay_state_after = rebound_replay
        .state()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    let id = hashv(&[
        PRODUCT_SERIES_LINK_RETIREMENT_POSTWRITE_DOMAIN_V4,
        &product_retirement_facts_id.bytes(),
        replay_account.key.as_ref(),
        &preauthorization.replay_authentication_id.bytes(),
        &rebound_replay.authentication_id().bytes(),
        &preauthorization.replay_data_id.bytes(),
        &rebound_replay.data_id().bytes(),
        &replay_state_before.bytes(),
        &replay_state_after.bytes(),
        &replay_retirement_projection_id.bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductSeriesLinkRetirementV4 {
        id,
        source,
        counted_id: counted.id(),
        root_account: *root_account.key,
        root_authentication_before: counted.root_authentication_before,
        root_authentication_after: rebound_root.authentication_id(),
        root_data_before: counted.root_data_before,
        root_data_after: rebound_root.data_id(),
        root_semantic_before: counted.root_semantic_before,
        root_semantic_after,
        root_transition_sequence_before: counted.root_transition_sequence_before,
        root_transition_sequence_after: counted.root_transition_sequence_after,
        link_account: *link_account.key,
        link_authentication_before: counted.link_authentication_before,
        link_authentication_retired: rebound_link.authentication_id(),
        link_data_before: counted.link_data_before,
        link_data_retired: rebound_link.data_id(),
        link_semantic_before: counted.link_semantic_before,
        link_semantic_retired,
        link_transition_sequence_before: counted.link_transition_sequence_before,
        link_transition_sequence_after: counted.link_transition_sequence_after,
        link_retirement_projection_id: counted.link_retirement.id(),
        link_observed_lamports,
        link_rent_principal_lamports,
        link_surplus_lamports,
        refund_account: *refund.key,
        refund_balance_before,
        refund_balance_after,
        neutral_sink: *neutral_sink.key,
        neutral_sink_balance_before,
        neutral_sink_balance_after,
        replay_account: *replay_account.key,
        replay_binding_id: preauthorization.replay_binding_id,
        replay_authentication_before: preauthorization.replay_authentication_id,
        replay_authentication_after: rebound_replay.authentication_id(),
        replay_data_before: preauthorization.replay_data_id,
        replay_data_after: rebound_replay.data_id(),
        replay_state_before,
        replay_state_after,
        replay_retirement_projection_id,
        replay: rebound_replay,
        registry_account: preauthorization.registry_account,
        registry_authentication_id: preauthorization.registry_authentication_id,
        registry_release_id: preauthorization.registry_release_id,
        capability_profile_id: preauthorization.capability_profile_id,
        funding_account: preauthorization.funding_account,
        funding_state_id: preauthorization.funding_state_id,
        funding_data_id: preauthorization.funding_data_id,
        funding_authentication_id: preauthorization.funding_authentication_id,
        series_plan_id: preauthorization.series_plan_id,
        funding_terms_id: preauthorization.funding_terms_id,
        compiler_bundle_id: preauthorization.compiler_bundle_id,
    })
}

/// One-way Source-custody and Product-link retirement half. The lifecycle
/// terminal is borrowed only long enough to derive Product's exact prewrite,
/// then moved once into Source's physical close. The returned authority owns
/// that close and therefore cannot be detached from the counted Link/Replay
/// postwrite which follows it.
#[allow(clippy::too_many_arguments)]
fn retire_product_series_source_and_link_v4<'root, 'link>(
    program_id: &Pubkey,
    registry: &AuthenticatedRegistryCapabilityV4,
    artifacts: &AuthenticatedSeriesSourceArtifactsV5,
    root_account: &AccountInfo<'_>,
    root: AuthenticatedMarketLifecycleRootV2<'_>,
    link_account: &AccountInfo<'_>,
    link: AuthenticatedSeriesMarketLinkV2<'_>,
    replay_account: &AccountInfo<'_>,
    replay: AuthenticatedSeriesLifecycleReplayV2,
    funding: &AuthenticatedSeriesFundingAccountV4,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    source_terminal: AuthenticatedSourceFundingCustodyLifecycleTerminalV1,
    source_custody: &AccountInfo<'_>,
    refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    root_rebound_output: &'root mut MarketLifecycleRootAccountV2,
    link_rebound_output: &'link mut SeriesMarketLinkAccountV2,
) -> Outcome<AuthenticatedProductSeriesLinkRetirementV4> {
    let preauthorization = preauthorize_product_series_retirement_v4(
        registry,
        artifacts,
        root,
        link,
        replay,
        funding,
        &source_terminal,
    )?;
    let accounting = preauthorization.source_accounting();
    let source = retire_source_funding_custody_v2(
        program_id,
        &preauthorization,
        route,
        schedule,
        source_terminal,
        &link,
        accounting,
        source_custody,
        refund,
        neutral_sink,
        system_program,
    )?;
    retire_last_product_series_link_v4(
        program_id,
        preauthorization,
        source,
        root_account,
        root,
        link_account,
        link,
        replay_account,
        replay,
        refund,
        neutral_sink,
        root_rebound_output,
        link_rebound_output,
    )
}

/// Seal the permanent ReplayV2 from the exact physical Source/Link close and
/// a freshly reopened Closed FundingV4. Funding is not disposed here: the
/// returned move-only authority must be consumed by the physical vault close.
#[allow(clippy::too_many_arguments)]
fn terminalize_product_series_lifecycle_v4(
    program_id: &Pubkey,
    registry_account: &AccountInfo<'_>,
    registry: &AuthenticatedRegistryCapabilityV4,
    funding_account: &AccountInfo<'_>,
    artifacts: &AuthenticatedSeriesSourceArtifactsV5,
    replay_account: &AccountInfo<'_>,
    link_retirement: AuthenticatedProductSeriesLinkRetirementV4,
) -> Outcome<AuthenticatedProductSeriesLifecycleTerminalV4> {
    artifacts.validate_registry_projection(&registry.projection())?;
    let series = artifacts.series();
    let series_plan_id = series
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let live_registry = authenticate_series_registry_account_v3(
        program_id,
        registry_account,
        series_plan_id,
        false,
    )?;
    let live_funding = authenticate_series_funding_account_v4(
        program_id,
        funding_account,
        series_plan_id,
        true,
    )?;
    let funding_state_id = live_funding
        .state()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let replay = link_retirement.replay;
    let replay_state_before = replay
        .state()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    require(
        registry.activation_consumed()
            && *registry_account.key == link_retirement.registry_account
            && live_registry.account() == link_retirement.registry_account
            && live_registry.authentication_id()
                == link_retirement.registry_authentication_id
            && live_registry.authentication_id()
                == registry.series_registry_authentication_id()
            && live_registry.value().registry_release_id
                == link_retirement.registry_release_id
            && live_registry.value().capability_profile_id
                == link_retirement.capability_profile_id
            && live_registry.value().activation_consumed
            && *funding_account.key == link_retirement.funding_account
            && live_funding.account() == link_retirement.funding_account
            && funding_state_id == link_retirement.funding_state_id
            && live_funding.data_id() == link_retirement.funding_data_id
            && live_funding.authentication_id()
                == link_retirement.funding_authentication_id
            && live_funding.state().phase == SeriesFundingPhaseV4::Closed
            && live_funding.state().series_plan_id == link_retirement.series_plan_id
            && live_funding.state().funding_terms_id.bytes()
                == link_retirement.funding_terms_id.bytes()
            && live_funding.state().compiler_bundle_id.bytes()
                == link_retirement.compiler_bundle_id.bytes()
            && *replay_account.key == link_retirement.replay_account
            && replay.account() == link_retirement.replay_account
            && replay.authentication_id()
                == link_retirement.replay_authentication_after
            && replay.data_id() == link_retirement.replay_data_after
            && replay_state_before == link_retirement.replay_state_after
            && replay.state().phase() == SeriesLifecycleReplayPhaseV2::Open
            && replay.state().binding().id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == link_retirement.replay_binding_id,
        ClutchError::MismatchedState,
    )?;
    let terminal_authority_id = hashv(&[
        PRODUCT_SERIES_TERMINAL_AUTHORITY_DOMAIN_V4,
        &link_retirement.id.bytes(),
        registry_account.key.as_ref(),
        &live_registry.data_id().bytes(),
        &live_registry.authentication_id().bytes(),
        funding_account.key.as_ref(),
        &funding_state_id.bytes(),
        &live_funding.data_id().bytes(),
        &live_funding.authentication_id().bytes(),
        replay_account.key.as_ref(),
        &replay_state_before.bytes(),
        &replay.data_id().bytes(),
        &replay.authentication_id().bytes(),
    ]);
    require_live(terminal_authority_id)?;
    let authority = ExactProductSeriesFundingTerminalAuthorityV4 {
        id: terminal_authority_id,
        expected_state: *live_funding.state(),
    };
    let funding_terminal_projection = live_funding
        .state()
        .close(
            &authority,
            series,
            artifacts.quote(),
            artifacts.attachment(),
            terminal_authority_id,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_terminal_projection_id = funding_terminal_projection
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let evidence = SeriesLifecycleTerminalEvidenceV2 {
        binding_id: link_retirement.replay_binding_id,
        funding_account_id: ContentId::from_bytes(funding_account.key.to_bytes()),
        funding_state_id: funding_state_id.content_id(),
        funding_terminal_projection_id,
        registry_account_id: ContentId::from_bytes(registry_account.key.to_bytes()),
        registry_authentication_id: live_registry.authentication_id(),
        terminal_authority_receipt_id: terminal_authority_id,
    };
    let (replay_successor, replay_terminal_projection) = replay
        .state()
        .terminalize(evidence)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let rebound_replay = super::write_series_lifecycle_replay_v2(
        program_id,
        replay_account,
        replay,
        replay_successor,
    )?;
    let replay_state_after = rebound_replay
        .state()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    require(
        rebound_replay.state().phase() == SeriesLifecycleReplayPhaseV2::Terminal
            && rebound_replay.state().terminal_projection_id()
                == replay_terminal_projection.id().content_id()
            && replay_terminal_projection.funding_terminal_projection_id()
                == funding_terminal_projection_id
            && replay_terminal_projection.registry_authentication_id()
                == live_registry.authentication_id()
            && replay_terminal_projection.terminal_authority_receipt_id()
                == terminal_authority_id,
        ClutchError::MismatchedState,
    )?;
    let id = hashv(&[
        PRODUCT_SERIES_LIFECYCLE_TERMINAL_POSTWRITE_DOMAIN_V4,
        &terminal_authority_id.bytes(),
        &link_retirement.id.bytes(),
        &funding_terminal_projection_id.bytes(),
        registry_account.key.as_ref(),
        &live_registry.data_id().bytes(),
        &live_registry.authentication_id().bytes(),
        funding_account.key.as_ref(),
        &funding_state_id.bytes(),
        &live_funding.data_id().bytes(),
        &live_funding.authentication_id().bytes(),
        replay_account.key.as_ref(),
        &replay_terminal_projection.id().bytes(),
        &replay_state_before.bytes(),
        &replay_state_after.bytes(),
        &replay.data_id().bytes(),
        &rebound_replay.data_id().bytes(),
        &replay.authentication_id().bytes(),
        &rebound_replay.authentication_id().bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductSeriesLifecycleTerminalV4 {
        id,
        terminal_authority_id,
        link_retirement,
        funding: live_funding,
        funding_terminal_projection,
        funding_terminal_projection_id,
        registry_data_id: live_registry.data_id(),
        registry_authentication_id: live_registry.authentication_id(),
        replay_account: *replay_account.key,
        replay_binding_id: replay.state().binding().id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        replay_data_before: replay.data_id(),
        replay_data_after: rebound_replay.data_id(),
        replay_authentication_before: replay.authentication_id(),
        replay_authentication_after: rebound_replay.authentication_id(),
        replay_state_before,
        replay_state_after,
        replay_terminal_projection,
        replay: rebound_replay,
    })
}

#[cfg(test)]
mod adversarial_source_tests {
    #[test]
    fn preauthorization_is_private_move_only_and_source_acceptance_is_once_only() {
        let source = include_str!("retirement_v4.rs");
        let preauth = source
            .split("struct AuthenticatedProductSeriesRetirementPreauthorizationV4")
            .nth(1)
            .and_then(|value| {
                value
                    .split("impl AuthenticatedProductSeriesRetirementPreauthorizationV4")
                    .next()
            })
            .expect("bounded retirement preauthorization");
        assert!(!preauth.contains("pub(crate) struct"));
        assert!(!preauth.contains("Clone"));
        assert!(!preauth.contains("Copy"));
        assert!(preauth.contains("accepted_source_retirement: Cell<bool>"));
        let owner = source
            .split("impl AuthenticatedSourceFundingCustodyRetirementAuthorityV2")
            .nth(1)
            .and_then(|value| {
                value.split("/// Derive the sole acyclic retirement prewrite").next()
            })
            .expect("bounded Source retirement owner");
        assert!(owner.contains("!self.accepted_source_retirement.replace(true)"));
        assert!(owner.contains("facts.accounting == self.source_accounting()"));
        assert!(owner.contains("facts.lifecycle_terminal == self.source_terminal_facts"));
        assert!(owner.contains("facts.product_release == self.source_product_release_facts"));
    }

    #[test]
    fn failed_source_projection_is_owned_only_by_source_terminal() {
        let source = include_str!("retirement_v4.rs");
        assert!(!source.contains("ProductSourceFailureReleaseV3"));
        assert!(!source.contains("AuthenticatedPersistedSourceFailureProductReleaseV3"));
        assert!(source.contains("source_terminal.product_release_facts()"));
        assert!(source.contains("source_terminal: &AuthenticatedSourceFundingCustodyLifecycleTerminalV1"));
    }

    #[test]
    fn current_retirement_accepts_no_v3_funding_or_caller_count_bridge() {
        let source = include_str!("retirement_v4.rs");
        assert!(!source.contains("FundingV3"));
        assert!(!source.contains("SeriesFundingAccountV3"));
        assert!(!source.contains("caller_count"));
        assert!(source.contains("retire_series_link_into"));
        assert!(source.contains("mark_retired_into"));
        assert!(source.contains("SeriesFundingPhaseV4::Closed"));
    }

    #[test]
    fn source_terminal_is_borrowed_then_moved_once_into_the_physical_close() {
        let source = include_str!("retirement_v4.rs");
        let outer = source
            .split("fn retire_product_series_source_and_link_v4")
            .nth(1)
            .and_then(|value| value.split("#[cfg(test)]").next())
            .expect("bounded Source and Product retirement half");
        assert!(outer.contains("&source_terminal"));
        assert!(outer.contains("source_terminal,"));
        assert!(outer.contains("retire_source_funding_custody_v2"));
        assert!(outer.contains("retire_last_product_series_link_v4"));
        assert!(!outer.contains("clone()"));
    }

    #[test]
    fn replay_terminal_seal_is_close_only_and_hostile_postwritten() {
        let source = include_str!("retirement_v4.rs");
        let authority = source
            .split("impl AuthenticatedSeriesFundingAuthorityV4 for ExactProductSeriesFundingTerminalAuthorityV4")
            .nth(1)
            .and_then(|value| value.split("impl ProductCountedSeriesLinkRetirementProjectionV4").next())
            .expect("bounded close-only Funding authority");
        assert_eq!(authority.matches("UnauthenticatedAuthority").count(), 7);
        assert!(authority.contains("terminal_receipt_id != self.id"));
        let terminal = source
            .split("fn terminalize_product_series_lifecycle_v4")
            .nth(1)
            .and_then(|value| value.split("#[cfg(test)]").next())
            .expect("bounded Replay terminal seal");
        assert!(terminal.contains("authenticate_series_funding_account_v4"));
        assert!(terminal.contains("authenticate_series_registry_account_v3"));
        assert!(terminal.contains(".terminalize(evidence)"));
        assert!(terminal.contains("write_series_lifecycle_replay_v2"));
    }
}
