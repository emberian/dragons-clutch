//! Private binding from one completed Source failure terminal to Product's
//! exact current LinkV3 release postwrite.
//!
//! Source terminal execution necessarily precedes Failure archival and the
//! Product link release, so this is a post-release bridge rather than a
//! circular terminal prerequisite. Failure's exact archive owner implements
//! the default-refusing authority and proves that the Product session terminal
//! receipt is the archive which consumed the Source postwrite.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::product_failure_link_v3_current::{
    AuthenticatedSeriesFailureSessionReleaseV4, FailureSessionReleaseDispositionV4,
};
use crate::instructions::source_failure_terminal_v1::{
    AuthenticatedSourceFailureTerminalPostwriteV1, SourceFailureTerminalAuthorityFactsV1,
};
use crate::source_plane_v3::{derive_runtime_pda, runtime_key};
use crate::source_plane_v3_actions::write_exact_account_data;
use clutch_product_series::SeriesMarketLinkV3Id;
use clutch_source_plane_v3::{ContentId, FixedCodec};
use clutch_source_plane_v3_adapter::PdaRecipeV3;
use clutch_source_plane_v3_runtime::{
    account_data_id, authenticate_source_failure_terminal_account_v3,
    AuthenticatedSourceFailureTerminalAccountV3, AuthenticatedSourceRouteV1,
    RuntimeAccountViewV1, SourceFailureKindV1, SourceFailureProductReleaseDispositionV3,
    SourceFailureTerminalAccountAccessV3, SourceFailureTerminalAccountV3,
    SOURCE_FAILURE_TERMINAL_ACCOUNT_V3_BYTES,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const SOURCE_FAILURE_PRODUCT_RELEASE_FACTS_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/source-failure-product-release-facts/v1";
const SOURCE_FAILURE_PRODUCT_RELEASE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/source-failure-product-release/v1";
const PERSISTED_SOURCE_FAILURE_PRODUCT_RELEASE_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/persisted-source-failure-product-release/v3";

fn source_id(value: clutch_product_series::ContentId) -> ContentId {
    ContentId::from_bytes(value.bytes())
}

/// Exact Source terminal and Product LinkV3 release facts offered to Failure's
/// non-public archive authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceFailureProductReleaseFactsV1 {
    pub(crate) source_terminal_postwrite_id: ContentId,
    pub(crate) source_terminal_authority_facts: SourceFailureTerminalAuthorityFactsV1,
    pub(crate) source_terminal_policy_authentication_id: ContentId,
    pub(crate) source_terminal_receipt_id: ContentId,
    pub(crate) source_terminal_receipt_authentication_id: ContentId,
    pub(crate) source_physical_disposition_id: ContentId,
    pub(crate) product_release_id: ContentId,
    pub(crate) product_release_disposition: FailureSessionReleaseDispositionV4,
    pub(crate) product_link_account: Pubkey,
    pub(crate) product_link_authentication_before: ContentId,
    pub(crate) product_link_authentication_after: ContentId,
    pub(crate) product_link_semantic_before: SeriesMarketLinkV3Id,
    pub(crate) product_link_semantic_after: SeriesMarketLinkV3Id,
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
                &self.source_terminal_policy_authentication_id.bytes(),
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
/// current Product LinkV3 release disposition.
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
    release: &AuthenticatedSeriesFailureSessionReleaseV4,
    authority: &A,
) -> Outcome<AuthenticatedSourceFailureProductReleaseV1> {
    let expected_disposition = match source.source_failure_kind() {
        SourceFailureKindV1::PrimaryMaturityWithoutAcceptedResolution => {
            FailureSessionReleaseDispositionV4::SourceAbsent
        }
        SourceFailureKindV1::SourceEvaluationRefused => {
            FailureSessionReleaseDispositionV4::SourceRefused
        }
    };
    let facts = SourceFailureProductReleaseFactsV1 {
        source_terminal_postwrite_id: source.id(),
        source_terminal_authority_facts: source.authority_facts(),
        source_terminal_policy_authentication_id: source.persisted_policy_authentication_id(),
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
        facts.source_terminal_policy_authentication_id,
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
            && facts.product_link_account != Pubkey::default()
            && facts.product_link_semantic_before.bytes() != [0; 32]
            && facts.product_link_semantic_after.bytes() != [0; 32]
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

/// Hostile, durable per-occurrence proof that the exact failed Source
/// terminal reached the matching current Product LinkV3 release. This value
/// is deliberately non-Copy so later retirement consumes one authenticated
/// projection rather than reconstructing the transaction-local bridge.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedPersistedSourceFailureProductReleaseV3 {
    id: ContentId,
    authenticated: AuthenticatedSourceFailureTerminalAccountV3,
}

impl AuthenticatedPersistedSourceFailureProductReleaseV3 {
    pub(crate) const fn id(&self) -> ContentId {
        self.id
    }

    pub(crate) const fn account(&self) -> clutch_source_plane_v3_runtime::RuntimeKey {
        self.authenticated.account()
    }

    pub(crate) const fn account_data_id(&self) -> ContentId {
        self.authenticated.account_data_id()
    }

    pub(crate) const fn authentication_id(&self) -> ContentId {
        self.authenticated.id()
    }

    pub(crate) const fn terminal(&self) -> clutch_source_plane_v3_runtime::SourceFailureTerminalV1 {
        self.authenticated.value().terminal()
    }

    pub(crate) const fn disposition(
        &self,
    ) -> Option<SourceFailureProductReleaseDispositionV3> {
        self.authenticated.value().disposition()
    }

    pub(crate) const fn source_terminal_postwrite_id(&self) -> ContentId {
        self.authenticated.value().source_terminal_postwrite_id()
    }

    pub(crate) const fn source_physical_disposition_id(&self) -> ContentId {
        self.authenticated.value().source_physical_disposition_id()
    }

    pub(crate) const fn product_release_binding_id(&self) -> ContentId {
        self.authenticated.value().product_release_binding_id()
    }

    pub(crate) const fn product_release_facts_id(&self) -> ContentId {
        self.authenticated.value().product_release_facts_id()
    }

    pub(crate) const fn product_release_id(&self) -> ContentId {
        self.authenticated.value().product_release_id()
    }

    pub(crate) const fn product_link_account(
        &self,
    ) -> clutch_source_plane_v3_runtime::RuntimeKey {
        self.authenticated.value().product_link_account()
    }

    pub(crate) const fn product_link_authentication_before(&self) -> ContentId {
        self.authenticated.value().product_link_authentication_before()
    }

    pub(crate) const fn product_link_authentication_after(&self) -> ContentId {
        self.authenticated.value().product_link_authentication_after()
    }

    pub(crate) const fn product_link_semantic_before(&self) -> ContentId {
        self.authenticated.value().product_link_semantic_before()
    }

    pub(crate) const fn product_link_semantic_after(&self) -> ContentId {
        self.authenticated.value().product_link_semantic_after()
    }

    pub(crate) const fn product_transition_sequence_before(&self) -> u64 {
        self.authenticated.value().product_transition_sequence_before()
    }

    pub(crate) const fn product_transition_sequence_after(&self) -> u64 {
        self.authenticated.value().product_transition_sequence_after()
    }

    pub(crate) const fn product_session_transcript_before(&self) -> ContentId {
        self.authenticated.value().product_session_transcript_before()
    }

    pub(crate) const fn product_session_transcript_after(&self) -> ContentId {
        self.authenticated.value().product_session_transcript_after()
    }

    pub(crate) const fn product_session_terminal_receipt_id(&self) -> ContentId {
        self.authenticated.value().product_session_terminal_receipt_id()
    }

    pub(crate) const fn product_archive_postwrite_id(&self) -> ContentId {
        self.authenticated.value().product_archive_postwrite_id()
    }

    pub(crate) const fn product_append_receipt_id(&self) -> ContentId {
        self.authenticated.value().product_append_receipt_id()
    }

    pub(crate) const fn product_reset_receipt_id(&self) -> ContentId {
        self.authenticated.value().product_reset_receipt_id()
    }

    pub(crate) const fn product_release_preauthorization_id(&self) -> ContentId {
        self.authenticated.value().product_release_preauthorization_id()
    }
}

fn authenticated_persisted_release(
    route: AuthenticatedSourceRouteV1,
    authenticated: AuthenticatedSourceFailureTerminalAccountV3,
) -> Outcome<AuthenticatedPersistedSourceFailureProductReleaseV3> {
    let value = authenticated.value();
    let value_id = value
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let terminal_id = value
        .terminal()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            PERSISTED_SOURCE_FAILURE_PRODUCT_RELEASE_DOMAIN_V3,
            &route.route_id().bytes(),
            &authenticated.account().bytes(),
            &authenticated.account_data_id().bytes(),
            &authenticated.id().bytes(),
            &terminal_id.bytes(),
            &value_id.bytes(),
            &value.product_release_binding_id().bytes(),
            &value.product_release_facts_id().bytes(),
        ])
        .to_bytes(),
    );
    require(
        !id.is_zero()
            && value.product_release_binding_id() != id
            && value.product_release_facts_id() != id,
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedPersistedSourceFailureProductReleaseV3 {
        id,
        authenticated,
    })
}

/// Rewrite one exact pending Source terminal account to BoundProductRelease.
/// The account is already fully prefunded; no caller amount, identity, or
/// publication hook participates in this same-instruction one-way write.
pub(crate) fn bind_persisted_source_failure_product_release_v3(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    bridge: AuthenticatedSourceFailureProductReleaseV1,
    terminal_policy_account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedPersistedSourceFailureProductReleaseV3> {
    let facts = bridge.facts();
    require(
        runtime_key(terminal_policy_account.key)
            == facts.source_terminal_authority_facts.source_terminal_policy_account
            && terminal_policy_account.is_writable
            && !terminal_policy_account.is_signer
            && !terminal_policy_account.executable
            && terminal_policy_account.owner == program_id
            && terminal_policy_account.data_len() == SOURCE_FAILURE_TERMINAL_ACCOUNT_V3_BYTES,
        ClutchError::MismatchedState,
    )?;
    let pending_data = terminal_policy_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let pending_body = SourceFailureTerminalAccountV3::decode(&pending_data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let terminal_id = pending_body
        .terminal()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let recipe = PdaRecipeV3::source_no_reopen_terminal(terminal_id)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let derived = derive_runtime_pda(program_id, &recipe).map_err(Refusal::from)?;
    let pending = authenticate_source_failure_terminal_account_v3(
        route,
        RuntimeAccountViewV1 {
            key: runtime_key(terminal_policy_account.key),
            owner: runtime_key(terminal_policy_account.owner),
            lamports: terminal_policy_account.lamports(),
            executable: terminal_policy_account.executable,
            writable: terminal_policy_account.is_writable,
            signer: terminal_policy_account.is_signer,
            data: &pending_data,
        },
        derived,
        SourceFailureTerminalAccountAccessV3::CreatedPendingMutable,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let pending_data_id = account_data_id(pending.account(), &pending_data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(pending_data);
    require(
        pending.account_data_id() == pending_data_id
            && pending.id() == facts.source_terminal_policy_authentication_id
            && pending.value().terminal().source_failure_terminal_authority_id()
                == facts.source_terminal_authority_facts.id()
            && pending.value().terminal().source_failure_kind()
                == facts.source_terminal_authority_facts.source_failure_kind,
        ClutchError::MismatchedState,
    )?;
    let disposition = match facts.product_release_disposition {
        FailureSessionReleaseDispositionV4::SourceAbsent => {
            SourceFailureProductReleaseDispositionV3::SourceAbsent
        }
        FailureSessionReleaseDispositionV4::SourceRefused => {
            SourceFailureProductReleaseDispositionV3::SourceRefused
        }
        FailureSessionReleaseDispositionV4::Resolved
        | FailureSessionReleaseDispositionV4::Exhausted => {
            return Err(Refusal::Adapter(ClutchError::MismatchedState));
        }
    };
    let bound_body = pending.value().bind_product_release(
        disposition,
        facts.source_terminal_postwrite_id,
        facts.source_physical_disposition_id,
        bridge.id(),
        facts.id(),
        facts.product_release_id,
        clutch_source_plane_v3_runtime::RuntimeKey::from_bytes(
            facts.product_link_account.to_bytes(),
        ),
        facts.product_link_authentication_before,
        facts.product_link_authentication_after,
        source_id(facts.product_link_semantic_before),
        source_id(facts.product_link_semantic_after),
        facts.product_transition_sequence_before,
        facts.product_transition_sequence_after,
        facts.product_session_transcript_before,
        facts.product_session_transcript_after,
        facts.product_session_terminal_receipt_id,
        facts.product_archive_postwrite_id,
        facts.product_append_receipt_id,
        facts.product_reset_receipt_id,
        facts.product_release_preauthorization_id,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut bytes = [0_u8; SOURCE_FAILURE_TERMINAL_ACCOUNT_V3_BYTES];
    bound_body
        .encode_into(&mut bytes)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    write_exact_account_data(terminal_policy_account, &bytes)?;
    let bound_data = terminal_policy_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let bound = authenticate_source_failure_terminal_account_v3(
        route,
        RuntimeAccountViewV1 {
            key: runtime_key(terminal_policy_account.key),
            owner: runtime_key(terminal_policy_account.owner),
            lamports: terminal_policy_account.lamports(),
            executable: terminal_policy_account.executable,
            writable: terminal_policy_account.is_writable,
            signer: terminal_policy_account.is_signer,
            data: &bound_data,
        },
        derived,
        SourceFailureTerminalAccountAccessV3::BoundMutable,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        bound.value() == bound_body
            && bound.value().terminal() == pending.value().terminal()
            && bound.value().source_terminal_postwrite_id()
                == facts.source_terminal_postwrite_id
            && bound.value().source_physical_disposition_id()
                == facts.source_physical_disposition_id
            && bound.value().product_release_binding_id() == bridge.id()
            && bound.value().product_release_facts_id() == facts.id()
            && bound.account_data_id() != pending_data_id
            && bound.id() != pending.id(),
        ClutchError::MismatchedState,
    )?;
    authenticated_persisted_release(route, bound)
}

/// Hostile-read an already bound per-occurrence release for later same-Link
/// lifecycle retirement. No caller-supplied terminal or bridge body is used.
pub(crate) fn authenticate_persisted_source_failure_product_release_v3(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    terminal_policy_account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedPersistedSourceFailureProductReleaseV3> {
    require(
        !terminal_policy_account.is_writable
            && !terminal_policy_account.is_signer
            && !terminal_policy_account.executable
            && terminal_policy_account.owner == program_id
            && terminal_policy_account.data_len() == SOURCE_FAILURE_TERMINAL_ACCOUNT_V3_BYTES,
        ClutchError::MismatchedState,
    )?;
    let data = terminal_policy_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let decoded = SourceFailureTerminalAccountV3::decode(&data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let terminal_id = decoded
        .terminal()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let recipe = PdaRecipeV3::source_no_reopen_terminal(terminal_id)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let derived = derive_runtime_pda(program_id, &recipe).map_err(Refusal::from)?;
    let authenticated = authenticate_source_failure_terminal_account_v3(
        route,
        RuntimeAccountViewV1 {
            key: runtime_key(terminal_policy_account.key),
            owner: runtime_key(terminal_policy_account.owner),
            lamports: terminal_policy_account.lamports(),
            executable: terminal_policy_account.executable,
            writable: terminal_policy_account.is_writable,
            signer: terminal_policy_account.is_signer,
            data: &data,
        },
        derived,
        SourceFailureTerminalAccountAccessV3::ExistingBoundReadOnly,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    authenticated_persisted_release(route, authenticated)
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
            FailureSessionReleaseDispositionV4::SourceAbsent.wire_byte(),
            3
        );
        assert_eq!(
            FailureSessionReleaseDispositionV4::SourceRefused.wire_byte(),
            4
        );
        let source = include_str!("source_failure_product_release_v1.rs");
        let compose = source
            .split("pub(crate) fn bind_source_failure_product_release_v1")
            .nth(1)
            .expect("private post-release bridge");
        assert!(compose.contains("PrimaryMaturityWithoutAcceptedResolution"));
        assert!(compose.contains("SourceEvaluationRefused"));
        assert!(!compose.contains("FailureSessionReleaseDispositionV4::Resolved"));
        assert!(!compose.contains("FailureSessionReleaseDispositionV4::Exhausted"));
    }

    #[test]
    fn persisted_binding_is_one_way_and_exact_over_release_facts() {
        let source = include_str!("source_failure_product_release_v1.rs");
        let bind = source
            .split("pub(crate) fn bind_persisted_source_failure_product_release_v3")
            .nth(1)
            .and_then(|value| {
                value.split("/// Hostile-read an already bound").next()
            })
            .expect("bounded persisted release binder");
        for exact in [
            "SourceFailureTerminalAccountAccessV3::CreatedPendingMutable",
            "pending.id() == facts.source_terminal_policy_authentication_id",
            "pending.value().terminal().source_failure_terminal_authority_id()",
            "facts.source_terminal_postwrite_id",
            "facts.source_physical_disposition_id",
            "facts.product_link_authentication_before",
            "facts.product_link_authentication_after",
            "source_id(facts.product_link_semantic_before)",
            "source_id(facts.product_link_semantic_after)",
            "facts.product_transition_sequence_before",
            "facts.product_transition_sequence_after",
            "facts.product_session_transcript_before",
            "facts.product_session_transcript_after",
            "SourceFailureTerminalAccountAccessV3::BoundMutable",
            "bound.value().terminal() == pending.value().terminal()",
            "bound.value().source_terminal_postwrite_id()",
            "bound.value().source_physical_disposition_id()",
        ] {
            assert!(bind.contains(exact), "missing {exact}");
        }
        assert!(!bind.contains("create_pda_account"));
        assert!(!bind.contains("resize("));
    }

    #[test]
    fn persisted_release_capability_is_non_clone_and_hostile_reopen_has_no_body() {
        let source = include_str!("source_failure_product_release_v1.rs");
        let prefix = source
            .split("pub(crate) struct AuthenticatedPersistedSourceFailureProductReleaseV3")
            .next()
            .expect("persisted capability")
            .rsplit("#[derive(")
            .next()
            .expect("derive list")
            .split(")]" )
            .next()
            .expect("bounded derive list");
        assert!(!prefix.contains("Clone"));
        assert!(!prefix.contains("Copy"));
        let projection_impl = source
            .split("impl AuthenticatedPersistedSourceFailureProductReleaseV3")
            .nth(1)
            .and_then(|value| value.split("fn authenticated_persisted_release").next())
            .expect("bounded projection getters");
        assert!(!projection_impl.contains("fn value("));
        assert!(projection_impl.contains("fn terminal("));
        assert!(projection_impl.contains("fn source_terminal_postwrite_id("));
        assert!(projection_impl.contains("fn source_physical_disposition_id("));
        let reopen = source
            .split("pub(crate) fn authenticate_persisted_source_failure_product_release_v3")
            .nth(1)
            .expect("hostile bound reopen");
        assert!(reopen.contains("SourceFailureTerminalAccountAccessV3::ExistingBoundReadOnly"));
        assert!(!reopen.contains("expected:"));
        assert!(!reopen.contains("bridge:"));
    }
}
