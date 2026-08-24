//! Private binding from one completed Source failure terminal to Product's
//! exact current LinkV2 release postwrite.
//!
//! Source terminal execution necessarily precedes Failure archival and the
//! Product link release, so this is a post-release bridge rather than a
//! circular terminal prerequisite. Failure's exact archive owner implements
//! the default-refusing authority and proves that the Product session terminal
//! receipt is the archive which consumed the Source postwrite.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::product_series_current::{
    AuthenticatedSeriesFailureSessionReleaseV3, FailureSessionReleaseDispositionV3,
};
use crate::instructions::source_failure_terminal_v1::{
    AuthenticatedSourceFailureTerminalPostwriteV1, SourceFailureTerminalAuthorityFactsV1,
};
use clutch_product_series::SeriesMarketLinkV2Id;
use clutch_source_plane_v3::ContentId;
use clutch_source_plane_v3_runtime::SourceFailureKindV1;
use solana_pubkey::Pubkey;

const SOURCE_FAILURE_PRODUCT_RELEASE_FACTS_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/source-failure-product-release-facts/v1";
const SOURCE_FAILURE_PRODUCT_RELEASE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/source-failure-product-release/v1";

fn source_id(value: clutch_product_series::ContentId) -> ContentId {
    ContentId::from_bytes(value.bytes())
}

/// Exact Source terminal and Product LinkV2 release facts offered to Failure's
/// non-public archive authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceFailureProductReleaseFactsV1 {
    pub(crate) source_terminal_postwrite_id: ContentId,
    pub(crate) source_terminal_authority_facts: SourceFailureTerminalAuthorityFactsV1,
    pub(crate) source_terminal_receipt_id: ContentId,
    pub(crate) source_terminal_receipt_authentication_id: ContentId,
    pub(crate) source_physical_disposition_id: ContentId,
    pub(crate) product_release_id: ContentId,
    pub(crate) product_release_disposition: FailureSessionReleaseDispositionV3,
    pub(crate) product_link_account: Pubkey,
    pub(crate) product_link_authentication_before: ContentId,
    pub(crate) product_link_authentication_after: ContentId,
    pub(crate) product_link_semantic_before: SeriesMarketLinkV2Id,
    pub(crate) product_link_semantic_after: SeriesMarketLinkV2Id,
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

impl SourceFailureProductReleaseFactsV1 {
    pub(crate) fn id(self) -> ContentId {
        ContentId::from_bytes(
            solana_sha256_hasher::hashv(&[
                SOURCE_FAILURE_PRODUCT_RELEASE_FACTS_DOMAIN_V1,
                &self.source_terminal_postwrite_id.bytes(),
                &self.source_terminal_authority_facts.id().bytes(),
                &self.source_terminal_receipt_id.bytes(),
                &self.source_terminal_receipt_authentication_id.bytes(),
                &self.source_physical_disposition_id.bytes(),
                &self.product_release_id.bytes(),
                &[self.product_release_disposition.wire_byte()],
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

/// Default-refusing bridge implemented only by Failure's archive postwrite.
pub(crate) trait AuthenticatedSourceFailureProductReleaseAuthorityV1 {
    fn authenticate_source_failure_product_release_v1(
        &self,
        _expected: SourceFailureProductReleaseFactsV1,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}

/// Private postwrite proving one exact Source terminal reached the matching
/// current Product LinkV2 release disposition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSourceFailureProductReleaseV1 {
    id: ContentId,
    facts: SourceFailureProductReleaseFactsV1,
}

impl AuthenticatedSourceFailureProductReleaseV1 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn facts(self) -> SourceFailureProductReleaseFactsV1 {
        self.facts
    }
}

/// Bind a completed Source terminal to the exact same-disposition Product
/// release. No caller-supplied ID participates in this composition.
pub(crate) fn bind_source_failure_product_release_v1<
    A: AuthenticatedSourceFailureProductReleaseAuthorityV1 + ?Sized,
>(
    source: AuthenticatedSourceFailureTerminalPostwriteV1,
    release: &AuthenticatedSeriesFailureSessionReleaseV3,
    authority: &A,
) -> Outcome<AuthenticatedSourceFailureProductReleaseV1> {
    let expected_disposition = match source.source_failure_kind() {
        SourceFailureKindV1::PrimaryMaturityWithoutAcceptedResolution => {
            FailureSessionReleaseDispositionV3::SourceAbsent
        }
        SourceFailureKindV1::SourceEvaluationRefused => {
            FailureSessionReleaseDispositionV3::SourceRefused
        }
    };
    let facts = SourceFailureProductReleaseFactsV1 {
        source_terminal_postwrite_id: source.id(),
        source_terminal_authority_facts: source.authority_facts(),
        source_terminal_receipt_id: source.terminal_receipt_id(),
        source_terminal_receipt_authentication_id: source
            .terminal_receipt_authentication_id(),
        source_physical_disposition_id: source.physical_disposition_id(),
        product_release_id: source_id(release.id()),
        product_release_disposition: release.disposition(),
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
    let exact_ids = [
        facts.source_terminal_postwrite_id,
        facts.source_terminal_authority_facts.id(),
        facts.source_terminal_receipt_id,
        facts.source_terminal_receipt_authentication_id,
        facts.source_physical_disposition_id,
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
        facts.product_release_disposition == expected_disposition
            && exact_ids.iter().all(|id| !id.is_zero())
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
    authority.authenticate_source_failure_product_release_v1(facts)?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SOURCE_FAILURE_PRODUCT_RELEASE_DOMAIN_V1,
            &facts.id().bytes(),
            &source.id().bytes(),
            &release.id().bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedSourceFailureProductReleaseV1 { id, facts })
}

#[cfg(test)]
mod adversarial_tests {
    use super::*;

    struct RefusingAuthority;
    impl AuthenticatedSourceFailureProductReleaseAuthorityV1 for RefusingAuthority {}

    #[test]
    fn default_product_release_bridge_refuses() {
        let _ = RefusingAuthority;
    }

    #[test]
    fn bridge_is_exhaustive_over_source_terminal_dispositions() {
        assert_eq!(
            FailureSessionReleaseDispositionV3::SourceAbsent.wire_byte(),
            3
        );
        assert_eq!(
            FailureSessionReleaseDispositionV3::SourceRefused.wire_byte(),
            4
        );
        let source = include_str!("source_failure_product_release_v1.rs");
        let compose = source
            .split("pub(crate) fn bind_source_failure_product_release_v1")
            .nth(1)
            .expect("private post-release bridge");
        assert!(compose.contains("PrimaryMaturityWithoutAcceptedResolution"));
        assert!(compose.contains("SourceEvaluationRefused"));
        assert!(!compose.contains("FailureSessionReleaseDispositionV3::Resolved"));
        assert!(!compose.contains("FailureSessionReleaseDispositionV3::Exhausted"));
    }
}
