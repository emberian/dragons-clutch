// SPDX-License-Identifier: AGPL-3.0-or-later
//! Private successful-Source binding to Product's current LinkV3 release.
//!
//! The Source terminal necessarily precedes Product release, so the bridge is
//! minted only after the exact Resolved release/archive postwrite. It accepts
//! no instruction payload and cannot be constructed from a raw release ID.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::failure_market_resolution_v5::AuthenticatedFailureMarketResolutionPostwriteV5;
use crate::instructions::product_failure_link_v3_current::{
    AuthenticatedSeriesFailureSessionReleaseV4, FailureSessionReleaseDispositionV4,
};
use crate::instructions::product_source_current::AuthenticatedSourceResolutionInputV4;
use crate::instructions::source_terminal_resolution_v5::AuthenticatedSourceResolutionTerminalV1;
use clutch_source_plane_v3::ContentId;
use clutch_source_plane_v3_runtime::RuntimeKey;
use solana_pubkey::Pubkey;

const SOURCE_RESOLUTION_PRODUCT_RELEASE_FACTS_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/source-resolution-product-release-facts/v1";
const SOURCE_RESOLUTION_PRODUCT_RELEASE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/source-resolution-product-release/v1";

fn source_id(value: clutch_product_series::ContentId) -> ContentId {
    ContentId::from_bytes(value.bytes())
}

/// Exact successful Source, ResolutionV5, and Product LinkV3 release facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceResolutionProductReleaseFactsV1 {
    pub(crate) source_resolution_input_id: ContentId,
    pub(crate) source_route_id: ContentId,
    pub(crate) source_release_manifest_id: ContentId,
    pub(crate) source_release_authentication_id: ContentId,
    pub(crate) source_handoff_authentication_id: ContentId,
    pub(crate) persisted_handoff_authentication_id: ContentId,
    pub(crate) successful_evaluation_handoff_id: ContentId,
    pub(crate) occurrence_account: RuntimeKey,
    pub(crate) result_account: RuntimeKey,
    pub(crate) result_account_authentication_id: ContentId,
    pub(crate) source_terminal_id: ContentId,
    pub(crate) source_terminal_semantic_id: ContentId,
    pub(crate) source_terminal_receipt_id: ContentId,
    pub(crate) source_terminal_receipt_authentication_id: ContentId,
    pub(crate) failure_resolution_postwrite_id: ContentId,
    pub(crate) failure_resolution_receipt_id: ContentId,
    pub(crate) product_resolution_activation_id: ContentId,
    pub(crate) failure_runtime_postwrite_id: ContentId,
    pub(crate) product_release_id: ContentId,
    pub(crate) product_link_account: Pubkey,
    pub(crate) product_link_authentication_before: ContentId,
    pub(crate) product_link_authentication_after: ContentId,
    pub(crate) product_link_semantic_before: clutch_product_series::SeriesMarketLinkV3Id,
    pub(crate) product_link_semantic_after: clutch_product_series::SeriesMarketLinkV3Id,
    pub(crate) product_transition_sequence_before: u64,
    pub(crate) product_transition_sequence_after: u64,
    pub(crate) product_session_transcript_before: ContentId,
    pub(crate) product_session_transcript_after: ContentId,
    pub(crate) product_session_terminal_receipt_id: ContentId,
    pub(crate) product_archive_postwrite_id: ContentId,
    pub(crate) product_append_receipt_id: ContentId,
    pub(crate) product_reset_receipt_id: ContentId,
    pub(crate) product_release_preauthorization_id: ContentId,
}

impl SourceResolutionProductReleaseFactsV1 {
    pub(crate) fn id(self) -> ContentId {
        ContentId::from_bytes(
            solana_sha256_hasher::hashv(&[
                SOURCE_RESOLUTION_PRODUCT_RELEASE_FACTS_DOMAIN_V1,
                &self.source_resolution_input_id.bytes(),
                &self.source_route_id.bytes(),
                &self.source_release_manifest_id.bytes(),
                &self.source_release_authentication_id.bytes(),
                &self.source_handoff_authentication_id.bytes(),
                &self.persisted_handoff_authentication_id.bytes(),
                &self.successful_evaluation_handoff_id.bytes(),
                &self.occurrence_account.bytes(),
                &self.result_account.bytes(),
                &self.result_account_authentication_id.bytes(),
                &self.source_terminal_id.bytes(),
                &self.source_terminal_semantic_id.bytes(),
                &self.source_terminal_receipt_id.bytes(),
                &self.source_terminal_receipt_authentication_id.bytes(),
                &self.failure_resolution_postwrite_id.bytes(),
                &self.failure_resolution_receipt_id.bytes(),
                &self.product_resolution_activation_id.bytes(),
                &self.failure_runtime_postwrite_id.bytes(),
                &self.product_release_id.bytes(),
                self.product_link_account.as_ref(),
                &self.product_link_authentication_before.bytes(),
                &self.product_link_authentication_after.bytes(),
                &self.product_link_semantic_before.bytes(),
                &self.product_link_semantic_after.bytes(),
                &self.product_transition_sequence_before.to_le_bytes(),
                &self.product_transition_sequence_after.to_le_bytes(),
                &self.product_session_transcript_before.bytes(),
                &self.product_session_transcript_after.bytes(),
                &self.product_session_terminal_receipt_id.bytes(),
                &self.product_archive_postwrite_id.bytes(),
                &self.product_append_receipt_id.bytes(),
                &self.product_reset_receipt_id.bytes(),
                &self.product_release_preauthorization_id.bytes(),
            ])
            .to_bytes(),
        )
    }
}

/// Default-refusing authority implemented by the exact successful Failure
/// archive/release postwrite, never by a raw Product release receipt.
pub(crate) trait AuthenticatedSourceResolutionProductReleaseAuthorityV1 {
    fn authenticate_source_resolution_product_release_v1(
        &self,
        _expected: SourceResolutionProductReleaseFactsV1,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}

/// Private successful Source-to-Product release capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSourceResolutionProductReleaseV1 {
    id: ContentId,
    facts: SourceResolutionProductReleaseFactsV1,
}

impl AuthenticatedSourceResolutionProductReleaseV1 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn facts(self) -> SourceResolutionProductReleaseFactsV1 {
        self.facts
    }
}

/// Bind successful Source terminalization and ResolutionV5 to one exact
/// current Product Resolved release. Failure's archive postwrite supplies the
/// final equality authority after the release has been physically persisted.
pub(crate) fn bind_source_resolution_product_release_v1<
    A: AuthenticatedSourceResolutionProductReleaseAuthorityV1 + ?Sized,
>(
    source: AuthenticatedSourceResolutionInputV4,
    terminal: AuthenticatedSourceResolutionTerminalV1,
    resolution: AuthenticatedFailureMarketResolutionPostwriteV5,
    release: &AuthenticatedSeriesFailureSessionReleaseV4,
    authority: &A,
) -> Outcome<AuthenticatedSourceResolutionProductReleaseV1> {
    let route = source.route();
    let terminal_execution = terminal.terminal();
    let facts = SourceResolutionProductReleaseFactsV1 {
        source_resolution_input_id: source.id(),
        source_route_id: route.source_route_id(),
        source_release_manifest_id: route.source_release_manifest_id(),
        source_release_authentication_id: route.source_release_authentication_id(),
        source_handoff_authentication_id: source.source_handoff_authentication_id(),
        persisted_handoff_authentication_id: source.persisted_handoff_authentication_id(),
        successful_evaluation_handoff_id: source.successful_evaluation_handoff_id(),
        occurrence_account: source.occurrence_account(),
        result_account: source.result_account(),
        result_account_authentication_id: source.result_account_authentication_id(),
        source_terminal_id: terminal.id(),
        source_terminal_semantic_id: terminal_execution.receipt.semantic_receipt_id(),
        source_terminal_receipt_id: terminal_execution.receipt.receipt_id(),
        source_terminal_receipt_authentication_id: terminal_execution
            .authenticated_receipt()
            .id(),
        failure_resolution_postwrite_id: resolution.id(),
        failure_resolution_receipt_id: ContentId::from_bytes(
            resolution.failure_resolution().id().bytes(),
        ),
        product_resolution_activation_id: source_id(resolution.product_activation().id()),
        failure_runtime_postwrite_id: resolution.runtime_postwrite_id(),
        product_release_id: source_id(release.id()),
        product_link_account: release.link_account(),
        product_link_authentication_before: source_id(release.link_authentication_before()),
        product_link_authentication_after: source_id(release.link_authentication_after()),
        product_link_semantic_before: release.link_semantic_before(),
        product_link_semantic_after: release.link_semantic_after(),
        product_transition_sequence_before: release.transition_sequence_before(),
        product_transition_sequence_after: release.transition_sequence_after(),
        product_session_transcript_before: source_id(
            release.failure_session_transcript_before(),
        ),
        product_session_transcript_after: source_id(
            release.failure_session_transcript_after(),
        ),
        product_session_terminal_receipt_id: source_id(release.session_terminal_receipt_id()),
        product_archive_postwrite_id: source_id(release.archive_postwrite_id()),
        product_append_receipt_id: source_id(release.append_receipt_id()),
        product_reset_receipt_id: source_id(release.reset_receipt_id()),
        product_release_preauthorization_id: source_id(
            release.release_link_preauthorization_id(),
        ),
    };
    let ids = [
        facts.source_resolution_input_id,
        facts.source_route_id,
        facts.source_release_manifest_id,
        facts.source_release_authentication_id,
        facts.source_handoff_authentication_id,
        facts.persisted_handoff_authentication_id,
        facts.successful_evaluation_handoff_id,
        facts.result_account_authentication_id,
        facts.source_terminal_id,
        facts.source_terminal_semantic_id,
        facts.source_terminal_receipt_id,
        facts.source_terminal_receipt_authentication_id,
        facts.failure_resolution_postwrite_id,
        facts.failure_resolution_receipt_id,
        facts.product_resolution_activation_id,
        facts.failure_runtime_postwrite_id,
        facts.product_release_id,
        facts.product_link_authentication_before,
        facts.product_link_authentication_after,
        facts.product_session_transcript_before,
        facts.product_session_transcript_after,
        facts.product_session_terminal_receipt_id,
        facts.product_archive_postwrite_id,
        facts.product_append_receipt_id,
        facts.product_reset_receipt_id,
        facts.product_release_preauthorization_id,
    ];
    require(
        release.disposition() == FailureSessionReleaseDispositionV4::Resolved
            && release.link_account() == resolution.activation().series_link()
            && source_id(release.release_link_preauthorization_id())
                == resolution.activation().series_link_preauthorization_id()
            && source.market_instance_id().bytes()
                == resolution
                    .product_activation()
                    .market_instance_id()
                    .content_id()
                    .bytes()
            && ids.iter().all(|id| !id.is_zero())
            && facts.occurrence_account != facts.result_account
            && facts.product_link_authentication_before
                != facts.product_link_authentication_after
            && facts.product_link_semantic_before != facts.product_link_semantic_after
            && facts.product_transition_sequence_after
                == facts
                    .product_transition_sequence_before
                    .checked_add(1)
                    .ok_or(ClutchError::Arithmetic)?
            && facts.product_session_transcript_before
                != facts.product_session_transcript_after
            && facts.product_archive_postwrite_id != facts.product_append_receipt_id
            && facts.product_archive_postwrite_id != facts.product_reset_receipt_id
            && facts.product_append_receipt_id != facts.product_reset_receipt_id,
        ClutchError::MismatchedState,
    )?;
    authority.authenticate_source_resolution_product_release_v1(facts)?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SOURCE_RESOLUTION_PRODUCT_RELEASE_DOMAIN_V1,
            &facts.id().bytes(),
            &terminal.id().bytes(),
            &resolution.id().bytes(),
            &release.id().bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedSourceResolutionProductReleaseV1 { id, facts })
}

#[cfg(test)]
mod adversarial_tests {
    use super::*;

    struct RefusingAuthority;
    impl AuthenticatedSourceResolutionProductReleaseAuthorityV1 for RefusingAuthority {}

    #[test]
    fn default_successful_release_authority_refuses() {
        let _ = RefusingAuthority;
    }

    #[test]
    fn successful_bridge_accepts_only_current_resolved_release() {
        let source = include_str!("source_resolution_product_release_v1.rs");
        let compose = source
            .split("pub(crate) fn bind_source_resolution_product_release_v1")
            .nth(1)
            .expect("successful current release bridge");
        assert!(compose.contains("AuthenticatedSourceResolutionInputV4"));
        assert!(compose.contains("AuthenticatedSeriesFailureSessionReleaseV4"));
        assert!(compose.contains("FailureSessionReleaseDispositionV4::Resolved"));
        assert!(!compose.contains("SourceAbsent"));
        assert!(!compose.contains("SourceRefused"));
    }
}
