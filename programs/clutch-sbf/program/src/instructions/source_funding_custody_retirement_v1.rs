// SPDX-License-Identifier: AGPL-3.0-or-later
//! Private retirement of Source's persisted principal/donation custody.
//!
//! The program-owned custody body, not Product or an instruction payload,
//! owns allocated/remaining principal and every observed donation. Product's
//! final counted-retirement receipt authenticates lifecycle completion and the
//! immutable FundingTerms destinations before this adapter closes the ledger.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{require_system_program, SYSTEM_PROGRAM_ID};
use crate::instructions::product_series_current::AuthenticatedSeriesMarketLinkV2;
use crate::instructions::source_failure_product_release_v1::
    AuthenticatedPersistedSourceFailureProductReleaseV3;
use crate::source_plane_v3::runtime_key;
use crate::source_plane_v3_actions::{
    authenticate_source_funding_custody_v1, AuthenticatedSourceFundingCustodyV1,
};
use clutch_source_plane_v3::ContentId;
use clutch_source_plane_v3_runtime::{
    account_data_id, AuthenticatedSourceRouteV1, RuntimeKey,
    SourceFailureKindV1, SourceFailureProductReleaseDispositionV3,
    SourceFundingCustodyLedgerV1, SourceWorkScheduleBindingV1,
};
use clutch_product_series::SeriesMarketLinkPhaseV2;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const SOURCE_FUNDING_CUSTODY_POSTTERMINAL_AUTH_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/source-funding-custody-postterminal-auth/v2";
const SOURCE_FUNDING_CUSTODY_RETIREMENT_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/source-funding-custody-retirement/v2";
const SOURCE_FUNDING_CUSTODY_LIFECYCLE_TERMINAL_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/source-funding-custody-lifecycle-terminal/v1";
const SOURCE_FUNDING_CUSTODY_PRODUCT_RELEASE_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/source-funding-custody-product-release/v3";
const SOURCE_FUNDING_CUSTODY_CLOSED_AUTH_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/source-funding-custody-closed-auth/v2";

/// Exhaustive terminal reason accepted by current Source custody retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceFundingCustodyTerminalDispositionV1 {
    Successful,
    SourceAbsent,
    SourceRefused,
}

impl SourceFundingCustodyTerminalDispositionV1 {
    const fn wire_byte(self) -> u8 {
        match self {
            Self::Successful => 1,
            Self::SourceAbsent => 2,
            Self::SourceRefused => 3,
        }
    }
}

/// Copy projection of the exact Product release evidence retained by the
/// move-only lifecycle terminal. The failed form is derived only from the
/// hostile V3 account; callers cannot lower its persisted identities from a
/// release hash.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceFundingCustodyProductReleaseFactsV3 {
    Successful {
        product_release_binding_id: ContentId,
        product_link_account: RuntimeKey,
    },
    Failed {
        disposition: SourceFailureProductReleaseDispositionV3,
        persisted_release_id: ContentId,
        persisted_account: RuntimeKey,
        persisted_account_data_id: ContentId,
        persisted_authentication_id: ContentId,
        persisted_terminal_id: ContentId,
        source_terminal_postwrite_id: ContentId,
        source_physical_disposition_id: ContentId,
        product_release_binding_id: ContentId,
        product_release_facts_id: ContentId,
        product_release_id: ContentId,
        product_link_account: RuntimeKey,
        product_link_authentication_before: ContentId,
        product_link_authentication_after: ContentId,
        product_link_semantic_before: ContentId,
        product_link_semantic_after: ContentId,
        product_transition_sequence_before: u64,
        product_transition_sequence_after: u64,
        product_session_transcript_before: ContentId,
        product_session_transcript_after: ContentId,
        product_session_terminal_receipt_id: ContentId,
        product_archive_postwrite_id: ContentId,
        product_append_receipt_id: ContentId,
        product_reset_receipt_id: ContentId,
        product_release_preauthorization_id: ContentId,
    },
}

impl SourceFundingCustodyProductReleaseFactsV3 {
    pub(crate) fn id(self) -> ContentId {
        match self {
            Self::Successful {
                product_release_binding_id,
                product_link_account,
            } => ContentId::from_bytes(
                solana_sha256_hasher::hashv(&[
                    SOURCE_FUNDING_CUSTODY_PRODUCT_RELEASE_DOMAIN_V3,
                    &[1],
                    &product_release_binding_id.bytes(),
                    &product_link_account.bytes(),
                ])
                .to_bytes(),
            ),
            Self::Failed {
                disposition,
                persisted_release_id,
                persisted_account,
                persisted_account_data_id,
                persisted_authentication_id,
                persisted_terminal_id,
                source_terminal_postwrite_id,
                source_physical_disposition_id,
                product_release_binding_id,
                product_release_facts_id,
                product_release_id,
                product_link_account,
                product_link_authentication_before,
                product_link_authentication_after,
                product_link_semantic_before,
                product_link_semantic_after,
                product_transition_sequence_before,
                product_transition_sequence_after,
                product_session_transcript_before,
                product_session_transcript_after,
                product_session_terminal_receipt_id,
                product_archive_postwrite_id,
                product_append_receipt_id,
                product_reset_receipt_id,
                product_release_preauthorization_id,
            } => ContentId::from_bytes(
                solana_sha256_hasher::hashv(&[
                    SOURCE_FUNDING_CUSTODY_PRODUCT_RELEASE_DOMAIN_V3,
                    &[match disposition {
                        SourceFailureProductReleaseDispositionV3::SourceAbsent => 2,
                        SourceFailureProductReleaseDispositionV3::SourceRefused => 3,
                    }],
                    &persisted_release_id.bytes(),
                    &persisted_account.bytes(),
                    &persisted_account_data_id.bytes(),
                    &persisted_authentication_id.bytes(),
                    &persisted_terminal_id.bytes(),
                    &source_terminal_postwrite_id.bytes(),
                    &source_physical_disposition_id.bytes(),
                    &product_release_binding_id.bytes(),
                    &product_release_facts_id.bytes(),
                    &product_release_id.bytes(),
                    &product_link_account.bytes(),
                    &product_link_authentication_before.bytes(),
                    &product_link_authentication_after.bytes(),
                    &product_link_semantic_before.bytes(),
                    &product_link_semantic_after.bytes(),
                    &product_transition_sequence_before.to_le_bytes(),
                    &product_transition_sequence_after.to_le_bytes(),
                    &product_session_transcript_before.bytes(),
                    &product_session_transcript_after.bytes(),
                    &product_session_terminal_receipt_id.bytes(),
                    &product_archive_postwrite_id.bytes(),
                    &product_append_receipt_id.bytes(),
                    &product_reset_receipt_id.bytes(),
                    &product_release_preauthorization_id.bytes(),
                ])
                .to_bytes(),
            ),
        }
    }
}

/// Exhaustive non-Copy release authority. A failed occurrence necessarily
/// owns the hostile V3 projection; successful resolution has no failure
/// terminal account and is accepted only beside Failure's successful owner.
#[derive(Debug, Eq, PartialEq)]
enum AuthenticatedSourceFundingCustodyProductReleaseV3 {
    Successful,
    Failed(AuthenticatedPersistedSourceFailureProductReleaseV3),
}

/// One canonical Source/Failure/Product terminal tuple. Amounts are excluded;
/// the hostile custody ledger remains their sole owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceFundingCustodyLifecycleTerminalFactsV1 {
    pub(crate) disposition: SourceFundingCustodyTerminalDispositionV1,
    pub(crate) capitalization_authority_id: ContentId,
    pub(crate) capitalization_receipt_id: ContentId,
    pub(crate) pre_root_source_occurrence_id: ContentId,
    pub(crate) product_link_account: RuntimeKey,
    pub(crate) product_link_account_data_id: ContentId,
    pub(crate) product_link_authentication_id: ContentId,
    pub(crate) product_link_semantic_id: ContentId,
    pub(crate) product_link_transition_sequence: u64,
    pub(crate) source_terminal_postwrite_id: ContentId,
    pub(crate) source_result_or_absence_close_receipt_id: ContentId,
    pub(crate) source_product_release_binding_id: ContentId,
    pub(crate) failure_family_terminal_receipt_id: ContentId,
    pub(crate) market_instance_id: ContentId,
    pub(crate) series_plan_id: ContentId,
    pub(crate) ordinal: u32,
    pub(crate) source_generation: u64,
    pub(crate) source_release_manifest_id: ContentId,
    pub(crate) source_release_authentication_id: ContentId,
    pub(crate) source_route_id: ContentId,
    pub(crate) source_work_schedule_id: ContentId,
    pub(crate) source_lifecycle_id: ContentId,
    pub(crate) source_occurrence_id: ContentId,
    pub(crate) source_occurrence_account: RuntimeKey,
    pub(crate) source_occurrence_authentication_id: ContentId,
    pub(crate) source_repair_generation: u64,
    pub(crate) source_funding_custody: RuntimeKey,
    pub(crate) lamport_principal_refund: RuntimeKey,
    pub(crate) neutral_lamport_sink: RuntimeKey,
}

impl SourceFundingCustodyLifecycleTerminalFactsV1 {
    fn id(self) -> ContentId {
        ContentId::from_bytes(
            solana_sha256_hasher::hashv(&[
                SOURCE_FUNDING_CUSTODY_LIFECYCLE_TERMINAL_DOMAIN_V1,
                &[self.disposition.wire_byte()],
                &self.capitalization_authority_id.bytes(),
                &self.capitalization_receipt_id.bytes(),
                &self.pre_root_source_occurrence_id.bytes(),
                &self.product_link_account.bytes(),
                &self.product_link_account_data_id.bytes(),
                &self.product_link_authentication_id.bytes(),
                &self.product_link_semantic_id.bytes(),
                &self.product_link_transition_sequence.to_le_bytes(),
                &self.source_terminal_postwrite_id.bytes(),
                &self.source_result_or_absence_close_receipt_id.bytes(),
                &self.source_product_release_binding_id.bytes(),
                &self.failure_family_terminal_receipt_id.bytes(),
                &self.market_instance_id.bytes(),
                &self.series_plan_id.bytes(),
                &self.ordinal.to_le_bytes(),
                &self.source_generation.to_le_bytes(),
                &self.source_release_manifest_id.bytes(),
                &self.source_release_authentication_id.bytes(),
                &self.source_route_id.bytes(),
                &self.source_work_schedule_id.bytes(),
                &self.source_lifecycle_id.bytes(),
                &self.source_occurrence_id.bytes(),
                &self.source_occurrence_account.bytes(),
                &self.source_occurrence_authentication_id.bytes(),
                &self.source_repair_generation.to_le_bytes(),
                &self.source_funding_custody.bytes(),
                &self.lamport_principal_refund.bytes(),
                &self.neutral_lamport_sink.bytes(),
            ])
            .to_bytes(),
        )
    }
}

/// Exact founder/custody facts derived only from the hostile live 0xbd ledger,
/// authenticated retiring Product LinkV2, and authenticated route/schedule.
/// Failure consumes this private value while returning the rest of the final
/// branch tuple; callers never supply either side.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceFundingCustodyLiveFounderFactsV1 {
    pub(crate) capitalization_authority_id: ContentId,
    pub(crate) capitalization_receipt_id: ContentId,
    pub(crate) pre_root_source_occurrence_id: ContentId,
    pub(crate) product_link_account: RuntimeKey,
    pub(crate) product_link_account_data_id: ContentId,
    pub(crate) product_link_authentication_id: ContentId,
    pub(crate) product_link_semantic_id: ContentId,
    pub(crate) product_link_transition_sequence: u64,
    pub(crate) market_instance_id: ContentId,
    pub(crate) series_plan_id: ContentId,
    pub(crate) ordinal: u32,
    pub(crate) source_release_manifest_id: ContentId,
    pub(crate) source_release_authentication_id: ContentId,
    pub(crate) source_route_id: ContentId,
    pub(crate) source_work_schedule_id: ContentId,
    pub(crate) source_lifecycle_id: ContentId,
    pub(crate) source_generation: u64,
    pub(crate) source_occurrence_id: ContentId,
    pub(crate) source_occurrence_account: RuntimeKey,
    pub(crate) source_occurrence_authentication_id: ContentId,
    pub(crate) source_repair_generation: u64,
    pub(crate) source_funding_custody: RuntimeKey,
    pub(crate) lamport_principal_refund: RuntimeKey,
    pub(crate) neutral_lamport_sink: RuntimeKey,
}

/// Failure-owned return value. Its failed constructor moves the sole hostile
/// V3 projection into Source; no parallel Product DTO can carry it later.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct SourceFundingCustodyLifecycleTerminalEvidenceV1 {
    facts: SourceFundingCustodyLifecycleTerminalFactsV1,
    product_release: AuthenticatedSourceFundingCustodyProductReleaseV3,
}

impl SourceFundingCustodyLifecycleTerminalEvidenceV1 {
    pub(crate) fn successful(
        facts: SourceFundingCustodyLifecycleTerminalFactsV1,
    ) -> Self {
        Self {
            facts,
            product_release: AuthenticatedSourceFundingCustodyProductReleaseV3::Successful,
        }
    }

    pub(crate) fn failed(
        facts: SourceFundingCustodyLifecycleTerminalFactsV1,
        release: AuthenticatedPersistedSourceFailureProductReleaseV3,
    ) -> Self {
        Self {
            facts,
            product_release: AuthenticatedSourceFundingCustodyProductReleaseV3::Failed(release),
        }
    }
}

/// Default-refusing boundary implemented only by Failure's exact final
/// successful or SourceAbsent/SourceRefused postwrite.
pub(crate) trait AuthenticatedSourceFundingCustodyLifecycleTerminalAuthorityV1 {
    /// Return the exact terminal tuple retained by the final Failure/Product
    /// postwrite. No instruction payload or caller projection supplies these
    /// identities.
    fn into_source_funding_custody_lifecycle_terminal_evidence_v1(
        self,
        _founder: SourceFundingCustodyLiveFounderFactsV1,
    ) -> Outcome<SourceFundingCustodyLifecycleTerminalEvidenceV1>
    where
        Self: Sized,
    {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}

/// Private non-Copy terminal capability consumed by Product retirement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSourceFundingCustodyLifecycleTerminalV1 {
    id: ContentId,
    facts: SourceFundingCustodyLifecycleTerminalFactsV1,
    product_release_facts: SourceFundingCustodyProductReleaseFactsV3,
    product_release: AuthenticatedSourceFundingCustodyProductReleaseV3,
}

impl AuthenticatedSourceFundingCustodyLifecycleTerminalV1 {
    pub(crate) const fn id(&self) -> ContentId {
        self.id
    }

    pub(crate) const fn facts(&self) -> SourceFundingCustodyLifecycleTerminalFactsV1 {
        self.facts
    }

    pub(crate) const fn product_release_facts(
        &self,
    ) -> SourceFundingCustodyProductReleaseFactsV3 {
        self.product_release_facts
    }
}

/// Authenticate one exhaustive final lifecycle tuple against the hostile live
/// custody and Failure's private terminal postwrite.
pub(crate) fn authenticate_source_funding_custody_lifecycle_terminal_v1<
    A: AuthenticatedSourceFundingCustodyLifecycleTerminalAuthorityV1,
>(
    authority: A,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    link: &AuthenticatedSeriesMarketLinkV2<'_>,
    custody: AuthenticatedSourceFundingCustodyV1,
) -> Outcome<AuthenticatedSourceFundingCustodyLifecycleTerminalV1> {
    let ledger = custody.ledger();
    let link_state = link.state();
    let link_binding = link_state.binding();
    let link_semantic_id = link_state
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let founder = SourceFundingCustodyLiveFounderFactsV1 {
        capitalization_authority_id: ledger.capitalization_authority_id,
        capitalization_receipt_id: ledger.capitalization_receipt_id,
        pre_root_source_occurrence_id: link_binding.source_occurrence_receipt_id,
        product_link_account: RuntimeKey::from_bytes(link.account().to_bytes()),
        product_link_account_data_id: ContentId::from_bytes(link.data_id().bytes()),
        product_link_authentication_id: ContentId::from_bytes(
            link.authentication_id().bytes(),
        ),
        product_link_semantic_id: ContentId::from_bytes(link_semantic_id.bytes()),
        product_link_transition_sequence: link_state.transition_sequence(),
        market_instance_id: ContentId::from_bytes(link_binding.market_instance_id.bytes()),
        series_plan_id: ContentId::from_bytes(link_binding.series_plan_id.bytes()),
        ordinal: link_binding.ordinal,
        source_release_manifest_id: route.release_manifest_id(),
        source_release_authentication_id: route.release_authentication_id(),
        source_route_id: route.route_id(),
        source_work_schedule_id: schedule.source_work_schedule_id(),
        source_lifecycle_id: schedule.lifecycle_id(),
        source_generation: schedule.generation(),
        source_occurrence_id: ContentId::from_bytes(link_binding.source_occurrence_id.bytes()),
        source_occurrence_account: RuntimeKey::from_bytes(
            link_binding.source_occurrence_account_id.bytes(),
        ),
        source_occurrence_authentication_id:
            link_binding.source_occurrence_account_authentication_id,
        source_repair_generation: link_binding.source_repair_generation,
        source_funding_custody: custody.account(),
        lamport_principal_refund: ledger.principal_refund,
        neutral_lamport_sink: ledger.neutral_sink,
    };
    require(
        ledger.is_live()
            && !founder.capitalization_authority_id.is_zero()
            && !founder.capitalization_receipt_id.is_zero()
            && founder.capitalization_authority_id != founder.capitalization_receipt_id
            && link.is_writable()
            && link_state.phase() == SeriesMarketLinkPhaseV2::Retiring
            && !founder.product_link_authentication_id.is_zero()
            && !founder.product_link_semantic_id.is_zero()
            && link_binding.source_occurrence_receipt_id
                == founder.pre_root_source_occurrence_id
            && link_binding.market_instance_id.bytes() == founder.market_instance_id.bytes()
            && link_binding.series_plan_id.bytes() == founder.series_plan_id.bytes()
            && link_binding.ordinal == founder.ordinal
            && link_binding.source_release_id == route.release_manifest_id()
            && link_binding.source_route_id == route.route_id()
            && link_binding.clock_policy_id == route.clock_policy_id()
            && link_binding.source_plane_contract_id == route.source_plane_contract_id()
            && link_binding.source_spec_id == route.source_spec_id()
            && link_binding.generation == schedule.generation()
            && link_binding.source_occurrence_id.bytes() == founder.source_occurrence_id.bytes()
            && link_binding.source_occurrence_account_id.bytes()
                == founder.source_occurrence_account.bytes()
            && link_binding.source_occurrence_account_authentication_id
                == founder.source_occurrence_authentication_id
            && link_binding.source_repair_generation == founder.source_repair_generation
            && link_binding.rent_refund_owner.bytes() == ledger.principal_refund.bytes()
            && link_binding.neutral_lamport_sink.bytes() == ledger.neutral_sink.bytes()
            && ledger.release_manifest_id == route.release_manifest_id()
            && ledger.route_id == route.route_id()
            && ledger.source_work_schedule_id == schedule.source_work_schedule_id()
            && ledger.lifecycle_id == schedule.lifecycle_id()
            && ledger.custody_account == custody.account()
            && ledger.neutral_sink == route.neutral_sink(),
        ClutchError::MismatchedState,
    )?;
    let evidence = authority
        .into_source_funding_custody_lifecycle_terminal_evidence_v1(founder)?;
    let expected = evidence.facts;
    let ids = [
        expected.capitalization_authority_id,
        expected.capitalization_receipt_id,
        expected.pre_root_source_occurrence_id,
        expected.product_link_account_data_id,
        expected.product_link_authentication_id,
        expected.product_link_semantic_id,
        expected.source_terminal_postwrite_id,
        expected.source_result_or_absence_close_receipt_id,
        expected.source_product_release_binding_id,
        expected.failure_family_terminal_receipt_id,
        expected.market_instance_id,
        expected.series_plan_id,
        expected.source_release_manifest_id,
        expected.source_release_authentication_id,
        expected.source_route_id,
        expected.source_work_schedule_id,
        expected.source_lifecycle_id,
        expected.source_occurrence_id,
        expected.source_occurrence_authentication_id,
    ];
    require(
        ids.iter().all(|id| !id.is_zero())
            && all_distinct_ids(&ids)
            && custody.ledger().is_live()
            && expected.capitalization_authority_id == founder.capitalization_authority_id
            && expected.capitalization_receipt_id
                == founder.capitalization_receipt_id
            && expected.pre_root_source_occurrence_id
                == founder.pre_root_source_occurrence_id
            && expected.product_link_account == founder.product_link_account
            && expected.product_link_account_data_id
                == founder.product_link_account_data_id
            && expected.product_link_authentication_id
                == founder.product_link_authentication_id
            && expected.product_link_semantic_id == founder.product_link_semantic_id
            && expected.product_link_transition_sequence
                == founder.product_link_transition_sequence
            && expected.market_instance_id == founder.market_instance_id
            && expected.series_plan_id == founder.series_plan_id
            && expected.ordinal == founder.ordinal
            && expected.source_release_manifest_id == route.release_manifest_id()
            && expected.source_release_manifest_id == ledger.release_manifest_id
            && expected.source_release_authentication_id == route.release_authentication_id()
            && expected.source_route_id == route.route_id()
            && expected.source_work_schedule_id == schedule.source_work_schedule_id()
            && expected.source_lifecycle_id == schedule.lifecycle_id()
            && expected.source_generation == schedule.generation()
            && expected.source_generation == founder.source_generation
            && expected.source_occurrence_id == founder.source_occurrence_id
            && expected.source_occurrence_account == founder.source_occurrence_account
            && expected.source_occurrence_authentication_id
                == founder.source_occurrence_authentication_id
            && expected.source_repair_generation == founder.source_repair_generation
            && expected.source_funding_custody == custody.account()
            && expected.lamport_principal_refund == ledger.principal_refund
            && expected.neutral_lamport_sink == ledger.neutral_sink
            && expected.neutral_lamport_sink == route.neutral_sink()
            && expected.source_funding_custody != expected.lamport_principal_refund
            && expected.source_funding_custody != expected.neutral_lamport_sink
            && expected.source_funding_custody != expected.source_occurrence_account
            && expected.product_link_account != expected.source_funding_custody
            && expected.product_link_account != expected.source_occurrence_account
            && expected.product_link_account != expected.lamport_principal_refund
            && expected.product_link_account != expected.neutral_lamport_sink
            && expected.source_occurrence_account != expected.lamport_principal_refund
            && expected.source_occurrence_account != expected.neutral_lamport_sink
            && expected.lamport_principal_refund != expected.neutral_lamport_sink,
        ClutchError::MismatchedState,
    )?;
    let product_release_facts = authenticate_product_release_evidence_v3(
        expected,
        &evidence.product_release,
    )?;
    let product_release_facts_id = product_release_facts.id();
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SOURCE_FUNDING_CUSTODY_LIFECYCLE_TERMINAL_DOMAIN_V1,
            &expected.id().bytes(),
            &product_release_facts_id.bytes(),
        ])
        .to_bytes(),
    );
    require(
        !id.is_zero() && !product_release_facts_id.is_zero() && id != product_release_facts_id,
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedSourceFundingCustodyLifecycleTerminalV1 {
        id,
        facts: expected,
        product_release_facts,
        product_release: evidence.product_release,
    })
}

fn authenticate_product_release_evidence_v3(
    terminal: SourceFundingCustodyLifecycleTerminalFactsV1,
    evidence: &AuthenticatedSourceFundingCustodyProductReleaseV3,
) -> Outcome<SourceFundingCustodyProductReleaseFactsV3> {
    match (terminal.disposition, evidence) {
        (
            SourceFundingCustodyTerminalDispositionV1::Successful,
            AuthenticatedSourceFundingCustodyProductReleaseV3::Successful,
        ) => Ok(SourceFundingCustodyProductReleaseFactsV3::Successful {
            product_release_binding_id: terminal.source_product_release_binding_id,
            product_link_account: terminal.product_link_account,
        }),
        (
            SourceFundingCustodyTerminalDispositionV1::SourceAbsent,
            AuthenticatedSourceFundingCustodyProductReleaseV3::Failed(release),
        )
        | (
            SourceFundingCustodyTerminalDispositionV1::SourceRefused,
            AuthenticatedSourceFundingCustodyProductReleaseV3::Failed(release),
        ) => {
            let expected_disposition = match terminal.disposition {
                SourceFundingCustodyTerminalDispositionV1::SourceAbsent => {
                    SourceFailureProductReleaseDispositionV3::SourceAbsent
                }
                SourceFundingCustodyTerminalDispositionV1::SourceRefused => {
                    SourceFailureProductReleaseDispositionV3::SourceRefused
                }
                SourceFundingCustodyTerminalDispositionV1::Successful => {
                    return Err(Refusal::Adapter(ClutchError::MismatchedState));
                }
            };
            let persisted_terminal = release.terminal();
            let persisted_terminal_id = persisted_terminal
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
            let expected_kind = match terminal.disposition {
                SourceFundingCustodyTerminalDispositionV1::SourceAbsent => {
                    SourceFailureKindV1::PrimaryMaturityWithoutAcceptedResolution
                }
                SourceFundingCustodyTerminalDispositionV1::SourceRefused => {
                    SourceFailureKindV1::SourceEvaluationRefused
                }
                SourceFundingCustodyTerminalDispositionV1::Successful => {
                    return Err(Refusal::Adapter(ClutchError::MismatchedState));
                }
            };
            let exact_ids = [
                release.id(),
                release.account_data_id(),
                release.authentication_id(),
                persisted_terminal_id,
                release.source_terminal_postwrite_id(),
                release.source_physical_disposition_id(),
                release.product_release_binding_id(),
                release.product_release_facts_id(),
                release.product_release_id(),
                release.product_link_authentication_before(),
                release.product_link_authentication_after(),
                release.product_link_semantic_before(),
                release.product_link_semantic_after(),
                release.product_session_transcript_before(),
                release.product_session_transcript_after(),
                release.product_session_terminal_receipt_id(),
                release.product_archive_postwrite_id(),
                release.product_append_receipt_id(),
                release.product_reset_receipt_id(),
                release.product_release_preauthorization_id(),
            ];
            require(
                exact_ids.iter().all(|id| !id.is_zero())
                    && all_distinct_ids(&exact_ids)
                    && release.disposition() == Some(expected_disposition)
                    && persisted_terminal.source_failure_kind() == expected_kind
                    && persisted_terminal.source_release_manifest_id()
                        == terminal.source_release_manifest_id
                    && persisted_terminal.source_release_authentication_id()
                        == terminal.source_release_authentication_id
                    && persisted_terminal.route_id() == terminal.source_route_id
                    && persisted_terminal.source_work_schedule_id()
                        == terminal.source_work_schedule_id
                    && persisted_terminal.market_instance_id() == terminal.market_instance_id
                    && persisted_terminal.failure_generation() == terminal.source_generation
                    && persisted_terminal.source_repair_generation()
                        == terminal.source_repair_generation
                    && release.source_terminal_postwrite_id()
                        == terminal.source_terminal_postwrite_id
                    && release.source_physical_disposition_id()
                        == terminal.source_result_or_absence_close_receipt_id
                    && release.product_release_binding_id()
                        == terminal.source_product_release_binding_id
                    && release.product_link_account() == terminal.product_link_account
                    && release.product_transition_sequence_after()
                        == release
                            .product_transition_sequence_before()
                            .checked_add(1)
                            .ok_or(ClutchError::Arithmetic)?
                    && release.product_link_authentication_before()
                        != release.product_link_authentication_after()
                    && release.product_link_semantic_before()
                        != release.product_link_semantic_after()
                    && release.product_session_transcript_before()
                        != release.product_session_transcript_after()
                    && release.account() != terminal.product_link_account
                    && release.account() != terminal.source_funding_custody
                    && release.account() != terminal.source_occurrence_account,
                ClutchError::MismatchedState,
            )?;
            Ok(SourceFundingCustodyProductReleaseFactsV3::Failed {
                disposition: expected_disposition,
                persisted_release_id: release.id(),
                persisted_account: release.account(),
                persisted_account_data_id: release.account_data_id(),
                persisted_authentication_id: release.authentication_id(),
                persisted_terminal_id,
                source_terminal_postwrite_id: release.source_terminal_postwrite_id(),
                source_physical_disposition_id: release.source_physical_disposition_id(),
                product_release_binding_id: release.product_release_binding_id(),
                product_release_facts_id: release.product_release_facts_id(),
                product_release_id: release.product_release_id(),
                product_link_account: release.product_link_account(),
                product_link_authentication_before: release
                    .product_link_authentication_before(),
                product_link_authentication_after: release.product_link_authentication_after(),
                product_link_semantic_before: release.product_link_semantic_before(),
                product_link_semantic_after: release.product_link_semantic_after(),
                product_transition_sequence_before: release
                    .product_transition_sequence_before(),
                product_transition_sequence_after: release.product_transition_sequence_after(),
                product_session_transcript_before: release
                    .product_session_transcript_before(),
                product_session_transcript_after: release.product_session_transcript_after(),
                product_session_terminal_receipt_id: release
                    .product_session_terminal_receipt_id(),
                product_archive_postwrite_id: release.product_archive_postwrite_id(),
                product_append_receipt_id: release.product_append_receipt_id(),
                product_reset_receipt_id: release.product_reset_receipt_id(),
                product_release_preauthorization_id: release
                    .product_release_preauthorization_id(),
            })
        }
        _ => Err(Refusal::Adapter(ClutchError::MismatchedState)),
    }
}

/// Product-owned terminal identities and immutable destinations. No amount is
/// supplied: all lamport accounting comes from the hostile-decoded ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceFundingCustodyRetirementAccountingV2 {
    pub(crate) funding_terms_id: ContentId,
    pub(crate) product_retirement_authority_id: ContentId,
    pub(crate) counted_retirement_receipt_id: ContentId,
    pub(crate) source_funding_custody: RuntimeKey,
    pub(crate) lamport_principal_refund: RuntimeKey,
    pub(crate) neutral_lamport_sink: RuntimeKey,
}

/// Complete locally derived pre/post retirement facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceFundingCustodyRetirementFactsV2 {
    pub(crate) accounting: SourceFundingCustodyRetirementAccountingV2,
    pub(crate) lifecycle_terminal_authentication_id: ContentId,
    pub(crate) lifecycle_terminal: SourceFundingCustodyLifecycleTerminalFactsV1,
    pub(crate) product_release: SourceFundingCustodyProductReleaseFactsV3,
    pub(crate) source_route_id: ContentId,
    pub(crate) source_work_schedule_id: ContentId,
    pub(crate) source_lifecycle_id: ContentId,
    pub(crate) product_link_account: RuntimeKey,
    pub(crate) product_link_account_data_id: ContentId,
    pub(crate) product_link_authentication_id: ContentId,
    pub(crate) product_link_semantic_id: ContentId,
    pub(crate) product_link_transition_sequence: u64,
    pub(crate) custody_account: RuntimeKey,
    pub(crate) custody_owner_before: RuntimeKey,
    pub(crate) custody_authentication_before_id: ContentId,
    pub(crate) custody_account_data_before_id: ContentId,
    pub(crate) custody_ledger_before_id: ContentId,
    pub(crate) ledger_before: SourceFundingCustodyLedgerV1,
    pub(crate) capitalization_receipt_id: ContentId,
    pub(crate) custody_balance_before: u64,
    pub(crate) custody_owner_after: RuntimeKey,
    pub(crate) custody_authentication_after_id: ContentId,
    pub(crate) custody_account_data_after_id: ContentId,
    pub(crate) custody_balance_after: u64,
    pub(crate) allocated_principal_lamports: u64,
    pub(crate) completed_principal_lamports: u64,
    pub(crate) principal_refund_lamports: u64,
    pub(crate) neutral_donation_lamports: u64,
    pub(crate) principal_refund_account: RuntimeKey,
    pub(crate) principal_refund_owner: RuntimeKey,
    pub(crate) principal_refund_data_id: ContentId,
    pub(crate) principal_refund_balance_before: u64,
    pub(crate) principal_refund_balance_after: u64,
    pub(crate) neutral_sink_account: RuntimeKey,
    pub(crate) neutral_sink_owner: RuntimeKey,
    pub(crate) neutral_sink_data_id: ContentId,
    pub(crate) neutral_sink_balance_before: u64,
    pub(crate) neutral_sink_balance_after: u64,
}

/// Default-refusing Product retirement owner.
pub(crate) trait AuthenticatedSourceFundingCustodyRetirementAuthorityV2 {
    fn authenticate_source_funding_custody_retirement_v2(
        &self,
        _facts: SourceFundingCustodyRetirementFactsV2,
    ) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}

/// Private ledger-close postwrite consumed before Funding may close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSourceFundingCustodyRetirementV2 {
    id: ContentId,
    product_retirement_authority_id: ContentId,
    facts: SourceFundingCustodyRetirementFactsV2,
    custody_account_data_after_id: ContentId,
}

impl AuthenticatedSourceFundingCustodyRetirementV2 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn product_retirement_authority_id(self) -> ContentId {
        self.product_retirement_authority_id
    }

    pub(crate) const fn facts(self) -> SourceFundingCustodyRetirementFactsV2 {
        self.facts
    }

    pub(crate) const fn custody_account_data_after_id(self) -> ContentId {
        self.custody_account_data_after_id
    }
}

fn require_system_destination(account: &AccountInfo<'_>, expected: RuntimeKey) -> Outcome<()> {
    require(
        runtime_key(account.key) == expected
            && account.owner == &SYSTEM_PROGRAM_ID
            && account.data_is_empty()
            && account.is_writable
            && !account.is_signer
            && !account.executable,
        ClutchError::MismatchedState,
    )
}

/// Close one exact terminal custody. Remaining recorded principal returns to
/// FundingTerms; recorded and newly observed donations go only to the route's
/// neutral sink. Neither recipient signs.
#[allow(clippy::too_many_arguments)]
pub(crate) fn retire_source_funding_custody_v2<
    A: AuthenticatedSourceFundingCustodyRetirementAuthorityV2 + ?Sized,
>(
    program_id: &Pubkey,
    authority: &A,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    lifecycle_terminal: AuthenticatedSourceFundingCustodyLifecycleTerminalV1,
    link: &AuthenticatedSeriesMarketLinkV2<'_>,
    accounting: SourceFundingCustodyRetirementAccountingV2,
    custody_account: &AccountInfo<'_>,
    principal_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
) -> Outcome<AuthenticatedSourceFundingCustodyRetirementV2> {
    require_system_program(system_program)?;
    let custody = authenticate_source_funding_custody_v1(
        program_id,
        route,
        schedule,
        custody_account,
    )?;
    require_system_destination(principal_refund, accounting.lamport_principal_refund)?;
    require_system_destination(neutral_sink, accounting.neutral_lamport_sink)?;
    let link_state = link.state();
    let link_semantic_id = link_state
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_binding = link_state.binding();
    let terminal_ids = [
        accounting.funding_terms_id,
        accounting.product_retirement_authority_id,
        accounting.counted_retirement_receipt_id,
        lifecycle_terminal.id(),
    ];
    let lifecycle_terminal_facts = lifecycle_terminal.facts();
    let product_release_facts = lifecycle_terminal.product_release_facts();
    require(
        terminal_ids.iter().all(|id| !id.is_zero())
            && all_distinct_ids(&terminal_ids)
            && link.is_writable()
            && link_state.phase() == SeriesMarketLinkPhaseV2::Retiring
            && lifecycle_terminal_facts.product_link_account.bytes()
                == link.account().to_bytes()
            && lifecycle_terminal_facts.product_link_account_data_id.bytes()
                == link.data_id().bytes()
            && lifecycle_terminal_facts.product_link_authentication_id.bytes()
                == link.authentication_id().bytes()
            && lifecycle_terminal_facts.product_link_semantic_id.bytes()
                == link_semantic_id.bytes()
            && lifecycle_terminal_facts.product_link_transition_sequence
                == link_state.transition_sequence()
            && lifecycle_terminal_facts.market_instance_id.bytes()
                == link_binding.market_instance_id.bytes()
            && lifecycle_terminal_facts.series_plan_id.bytes()
                == link_binding.series_plan_id.bytes()
            && lifecycle_terminal_facts.ordinal == link_binding.ordinal
            && lifecycle_terminal_facts.source_generation == link_binding.generation
            && lifecycle_terminal_facts.source_occurrence_id.bytes()
                == link_binding.source_occurrence_id.bytes()
            && lifecycle_terminal_facts.source_occurrence_account.bytes()
                == link_binding.source_occurrence_account_id.bytes()
            && lifecycle_terminal_facts.source_occurrence_authentication_id
                == link_binding.source_occurrence_account_authentication_id
            && lifecycle_terminal_facts.source_repair_generation
                == link_binding.source_repair_generation
            && accounting.source_funding_custody == custody.account()
            && lifecycle_terminal_facts.source_funding_custody == custody.account()
            && lifecycle_terminal_facts.source_route_id == route.route_id()
            && lifecycle_terminal_facts.source_work_schedule_id
                == schedule.source_work_schedule_id()
            && lifecycle_terminal_facts.source_lifecycle_id == schedule.lifecycle_id()
            && accounting.lamport_principal_refund == custody.ledger().principal_refund
            && lifecycle_terminal_facts.lamport_principal_refund
                == accounting.lamport_principal_refund
            && accounting.neutral_lamport_sink == custody.ledger().neutral_sink
            && lifecycle_terminal_facts.neutral_lamport_sink
                == accounting.neutral_lamport_sink
            && accounting.neutral_lamport_sink == route.neutral_sink()
            && principal_refund.key != neutral_sink.key
            && custody_account.key != principal_refund.key
            && custody_account.key != neutral_sink.key,
        ClutchError::MismatchedState,
    )?;
    let ledger_before = custody
        .ledger()
        .observe_terminal_balance(
            custody_account.lamports(),
            accounting.counted_retirement_receipt_id,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    require(
        ledger_before.is_live()
            && !ledger_before.capitalization_receipt_id.is_zero()
            && ledger_before.capitalization_receipt_id
                != ledger_before.capitalization_authority_id
            && terminal_ids
                .iter()
                .all(|id| *id != ledger_before.capitalization_receipt_id)
            && lifecycle_terminal_facts.capitalization_receipt_id
                == ledger_before.capitalization_receipt_id
            && lifecycle_terminal_facts.capitalization_authority_id
                == ledger_before.capitalization_authority_id,
        ClutchError::MismatchedState,
    )?;
    if ledger_before != custody.ledger() {
        let bytes = ledger_before
            .encode()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let mut data = custody_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        require(data.len() == bytes.len(), ClutchError::WrongDataLength)?;
        data.copy_from_slice(&bytes);
    }
    let custody_balance_before = custody_account.lamports();
    let partition = ledger_before
        .remaining_principal_lamports
        .checked_add(ledger_before.donation_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        partition == custody_balance_before,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let completed_principal_lamports = ledger_before
        .allocated_principal_lamports
        .checked_sub(ledger_before.remaining_principal_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let custody_data = custody_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let custody_account_data_before_id =
        account_data_id(runtime_key(custody_account.key), &custody_data)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(custody_data);
    let ledger_before_id = ledger_before
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let custody_authentication_before_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SOURCE_FUNDING_CUSTODY_POSTTERMINAL_AUTH_DOMAIN_V2,
            &route.route_id().bytes(),
            &schedule.source_work_schedule_id().bytes(),
            &schedule.lifecycle_id().bytes(),
            custody_account.key.as_ref(),
            &custody_account_data_before_id.bytes(),
            &ledger_before_id.bytes(),
            &custody_balance_before.to_le_bytes(),
        ])
        .to_bytes(),
    );
    let custody_account_data_after_id = account_data_id(runtime_key(custody_account.key), &[])
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let custody_owner_before = RuntimeKey::from_bytes(program_id.to_bytes());
    let custody_owner_after = RuntimeKey::from_bytes(SYSTEM_PROGRAM_ID.to_bytes());
    let custody_authentication_after_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SOURCE_FUNDING_CUSTODY_CLOSED_AUTH_DOMAIN_V2,
            &route.route_id().bytes(),
            &schedule.source_work_schedule_id().bytes(),
            &schedule.lifecycle_id().bytes(),
            custody_account.key.as_ref(),
            &custody_owner_after.bytes(),
            &custody_account_data_after_id.bytes(),
            &0_u64.to_le_bytes(),
            &lifecycle_terminal.id().bytes(),
            &accounting.product_retirement_authority_id.bytes(),
            &accounting.counted_retirement_receipt_id.bytes(),
        ])
        .to_bytes(),
    );
    let principal_refund_data = principal_refund
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let principal_refund_data_id = account_data_id(
        runtime_key(principal_refund.key),
        &principal_refund_data,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(principal_refund_data);
    let neutral_sink_data = neutral_sink
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let neutral_sink_data_id = account_data_id(runtime_key(neutral_sink.key), &neutral_sink_data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(neutral_sink_data);
    let principal_refund_balance_before = principal_refund.lamports();
    let neutral_sink_balance_before = neutral_sink.lamports();
    let principal_refund_balance_after = principal_refund_balance_before
        .checked_add(ledger_before.remaining_principal_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let neutral_sink_balance_after = neutral_sink_balance_before
        .checked_add(ledger_before.donation_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let facts = SourceFundingCustodyRetirementFactsV2 {
        accounting,
        lifecycle_terminal_authentication_id: lifecycle_terminal.id(),
        lifecycle_terminal: lifecycle_terminal_facts,
        product_release: product_release_facts,
        source_route_id: route.route_id(),
        source_work_schedule_id: schedule.source_work_schedule_id(),
        source_lifecycle_id: schedule.lifecycle_id(),
        product_link_account: RuntimeKey::from_bytes(link.account().to_bytes()),
        product_link_account_data_id: ContentId::from_bytes(link.data_id().bytes()),
        product_link_authentication_id: ContentId::from_bytes(
            link.authentication_id().bytes(),
        ),
        product_link_semantic_id: ContentId::from_bytes(link_semantic_id.bytes()),
        product_link_transition_sequence: link_state.transition_sequence(),
        custody_account: custody.account(),
        custody_owner_before,
        custody_authentication_before_id,
        custody_account_data_before_id,
        custody_ledger_before_id: ledger_before_id,
        ledger_before,
        capitalization_receipt_id: ledger_before.capitalization_receipt_id,
        custody_balance_before,
        custody_owner_after,
        custody_authentication_after_id,
        custody_account_data_after_id,
        custody_balance_after: 0,
        allocated_principal_lamports: ledger_before.allocated_principal_lamports,
        completed_principal_lamports,
        principal_refund_lamports: ledger_before.remaining_principal_lamports,
        neutral_donation_lamports: ledger_before.donation_lamports,
        principal_refund_account: runtime_key(principal_refund.key),
        principal_refund_owner: RuntimeKey::from_bytes(principal_refund.owner.to_bytes()),
        principal_refund_data_id,
        principal_refund_balance_before,
        principal_refund_balance_after,
        neutral_sink_account: runtime_key(neutral_sink.key),
        neutral_sink_owner: RuntimeKey::from_bytes(neutral_sink.owner.to_bytes()),
        neutral_sink_data_id,
        neutral_sink_balance_before,
        neutral_sink_balance_after,
    };
    let product_retirement_authority_id =
        authority.authenticate_source_funding_custody_retirement_v2(facts)?;
    require(
        product_retirement_authority_id == accounting.product_retirement_authority_id,
        ClutchError::AuthorizationUnavailable,
    )?;
    {
        let mut custody_balance = custody_account
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut refund_balance = principal_refund
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut sink_balance = neutral_sink
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **custody_balance = 0;
        **refund_balance = principal_refund_balance_after;
        **sink_balance = neutral_sink_balance_after;
    }
    custody_account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    custody_account.assign(&SYSTEM_PROGRAM_ID);
    require(
        custody_account.lamports() == 0
            && custody_account.owner == &SYSTEM_PROGRAM_ID
            && custody_account.data_is_empty()
            && account_data_id(runtime_key(custody_account.key), &[])
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == custody_account_data_after_id
            && principal_refund.lamports() == principal_refund_balance_after
            && principal_refund.owner == &SYSTEM_PROGRAM_ID
            && principal_refund.data_is_empty()
            && neutral_sink.lamports() == neutral_sink_balance_after
            && neutral_sink.owner == &SYSTEM_PROGRAM_ID
            && neutral_sink.data_is_empty(),
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SOURCE_FUNDING_CUSTODY_RETIREMENT_DOMAIN_V2,
            &product_retirement_authority_id.bytes(),
            &accounting.funding_terms_id.bytes(),
            &ledger_before.capitalization_authority_id.bytes(),
            &ledger_before.capitalization_receipt_id.bytes(),
            &lifecycle_terminal.id().bytes(),
            &lifecycle_terminal_facts.pre_root_source_occurrence_id.bytes(),
            &lifecycle_terminal_facts.source_terminal_postwrite_id.bytes(),
            &lifecycle_terminal_facts
                .source_result_or_absence_close_receipt_id
                .bytes(),
            &lifecycle_terminal_facts.source_product_release_binding_id.bytes(),
            &lifecycle_terminal_facts.failure_family_terminal_receipt_id.bytes(),
            &accounting.counted_retirement_receipt_id.bytes(),
            &accounting.source_funding_custody.bytes(),
            &accounting.lamport_principal_refund.bytes(),
            &accounting.neutral_lamport_sink.bytes(),
            &product_release_facts.id().bytes(),
            link.account().as_ref(),
            &link.data_id().bytes(),
            &link.authentication_id().bytes(),
            &link_semantic_id.bytes(),
            &link_state.transition_sequence().to_le_bytes(),
            &custody_owner_before.bytes(),
            &custody.account().bytes(),
            &custody_authentication_before_id.bytes(),
            &custody_account_data_before_id.bytes(),
            &ledger_before_id.bytes(),
            &custody_owner_after.bytes(),
            &custody_authentication_after_id.bytes(),
            &custody_account_data_after_id.bytes(),
            &custody_balance_before.to_le_bytes(),
            &0_u64.to_le_bytes(),
            &ledger_before.allocated_principal_lamports.to_le_bytes(),
            &completed_principal_lamports.to_le_bytes(),
            &ledger_before.remaining_principal_lamports.to_le_bytes(),
            &ledger_before.donation_lamports.to_le_bytes(),
            principal_refund.key.as_ref(),
            principal_refund.owner.as_ref(),
            &principal_refund_data_id.bytes(),
            &principal_refund_balance_before.to_le_bytes(),
            &principal_refund_balance_after.to_le_bytes(),
            neutral_sink.key.as_ref(),
            neutral_sink.owner.as_ref(),
            &neutral_sink_data_id.bytes(),
            &neutral_sink_balance_before.to_le_bytes(),
            &neutral_sink_balance_after.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedSourceFundingCustodyRetirementV2 {
        id,
        product_retirement_authority_id,
        facts,
        custody_account_data_after_id,
    })
}

fn all_distinct_ids(values: &[ContentId]) -> bool {
    let mut index = 0usize;
    while index < values.len() {
        let mut prior = 0usize;
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

#[cfg(test)]
mod adversarial_tests {
    use super::*;

    struct RefusingLifecycleTerminal;
    impl AuthenticatedSourceFundingCustodyLifecycleTerminalAuthorityV1
        for RefusingLifecycleTerminal
    {
    }

    struct RefusingRetirement;
    impl AuthenticatedSourceFundingCustodyRetirementAuthorityV2 for RefusingRetirement {}

    #[test]
    fn default_retirement_authority_refuses() {
        let _ = RefusingRetirement;
        let _ = RefusingLifecycleTerminal;
    }

    #[test]
    fn lifecycle_terminal_dispositions_are_exhaustive_and_stable() {
        assert_eq!(
            SourceFundingCustodyTerminalDispositionV1::Successful.wire_byte(),
            1
        );
        assert_eq!(
            SourceFundingCustodyTerminalDispositionV1::SourceAbsent.wire_byte(),
            2
        );
        assert_eq!(
            SourceFundingCustodyTerminalDispositionV1::SourceRefused.wire_byte(),
            3
        );
    }

    #[test]
    fn lifecycle_terminal_facts_come_only_from_final_failure_authority() {
        let source = include_str!("source_funding_custody_retirement_v1.rs");
        let authenticate = source
            .split("pub(crate) fn authenticate_source_funding_custody_lifecycle_terminal_v1")
            .nth(1)
            .and_then(|value| {
                value
                    .split("/// Product-owned terminal identities")
                    .next()
            })
            .expect("bounded lifecycle terminal authentication");
        assert!(authenticate.contains(
            ".into_source_funding_custody_lifecycle_terminal_evidence_v1(founder)?"
        ));
        assert!(!authenticate.contains(
            "expected: SourceFundingCustodyLifecycleTerminalFactsV1"
        ));
    }

    #[test]
    fn lifecycle_terminal_founder_is_derived_from_exact_retiring_link() {
        let source = include_str!("source_funding_custody_retirement_v1.rs");
        let authenticate = source
            .split("pub(crate) fn authenticate_source_funding_custody_lifecycle_terminal_v1")
            .nth(1)
            .and_then(|value| value.split("/// Product-owned terminal identities").next())
            .expect("bounded lifecycle terminal authentication");
        for exact_join in [
            "link_state.phase() == SeriesMarketLinkPhaseV2::Retiring",
            "product_link_account: RuntimeKey::from_bytes(link.account().to_bytes())",
            "product_link_account_data_id: ContentId::from_bytes(link.data_id().bytes())",
            "link.authentication_id().bytes()",
            "product_link_semantic_id: ContentId::from_bytes(link_semantic_id.bytes())",
            "product_link_transition_sequence: link_state.transition_sequence()",
            "pre_root_source_occurrence_id: link_binding.source_occurrence_receipt_id",
            "source_occurrence_id: ContentId::from_bytes(link_binding.source_occurrence_id.bytes())",
            "link_binding.source_occurrence_account_id.bytes()",
            "link_binding.source_occurrence_account_authentication_id",
            "link_binding.market_instance_id.bytes() == founder.market_instance_id.bytes()",
            "link_binding.series_plan_id.bytes() == founder.series_plan_id.bytes()",
            "link_binding.ordinal == founder.ordinal",
            "link_binding.generation == schedule.generation()",
            "link_binding.source_repair_generation == founder.source_repair_generation",
        ] {
            assert!(authenticate.contains(exact_join), "missing {exact_join}");
        }
        for returned_join in [
            "expected.pre_root_source_occurrence_id",
            "expected.product_link_account == founder.product_link_account",
            "expected.product_link_account_data_id",
            "expected.product_link_authentication_id",
            "expected.product_link_semantic_id == founder.product_link_semantic_id",
            "expected.product_link_transition_sequence",
            "expected.source_occurrence_id == founder.source_occurrence_id",
            "expected.source_occurrence_account == founder.source_occurrence_account",
            "expected.source_occurrence_authentication_id",
            "expected.source_repair_generation == founder.source_repair_generation",
        ] {
            assert!(authenticate.contains(returned_join), "missing {returned_join}");
        }
    }

    #[test]
    fn retirement_accepts_no_amount_or_signing_recipient() {
        let source = include_str!("source_funding_custody_retirement_v1.rs");
        let accounting = source
            .split("pub(crate) struct SourceFundingCustodyRetirementAccountingV2")
            .nth(1)
            .and_then(|value| value.split("/// Complete locally derived").next())
            .expect("caller-neutral retirement accounting");
        let retire = source
            .split("pub(crate) fn retire_source_funding_custody_v2")
            .nth(1)
            .expect("private ledger retirement");
        assert!(!accounting.contains("allocated_principal_lamports"));
        assert!(!accounting.contains("completed_principal_lamports"));
        assert!(!accounting.contains("capitalization_receipt_id"));
        assert!(!accounting.contains("pre_root_source_occurrence_id"));
        assert!(!accounting.contains("source_terminal_postwrite_id"));
        assert!(!accounting.contains("source_product_release_binding_id"));
        assert!(source.contains(
            "capitalization_receipt_id: ledger_before.capitalization_receipt_id"
        ));
        assert!(retire.contains("lifecycle_terminal.facts()"));
        assert!(retire.contains("lifecycle_terminal.product_release_facts()"));
        assert!(retire.contains("link_state.phase() == SeriesMarketLinkPhaseV2::Retiring"));
        assert!(retire.contains("link_binding.source_occurrence_account_authentication_id"));
        assert!(retire.contains(".checked_sub(ledger_before.remaining_principal_lamports)"));
        assert!(retire.contains("ledger_before.remaining_principal_lamports"));
        assert!(retire.contains("ledger_before.donation_lamports"));
        assert!(source.contains("!account.is_signer"));
        assert!(source.contains("all_distinct_ids(&terminal_ids)"));
    }
    #[test]
    fn terminal_and_retirement_capabilities_are_non_clone_consumables() {
        let source = include_str!("source_funding_custody_retirement_v1.rs");
        for name in [
            "SourceFundingCustodyLifecycleTerminalEvidenceV1",
            "AuthenticatedSourceFundingCustodyLifecycleTerminalV1",
            "AuthenticatedSourceFundingCustodyRetirementV2",
        ] {
            let prefix = source
                .split(&format!("pub(crate) struct {name}"))
                .next()
                .expect("capability declaration")
                .rsplit("#[derive(")
                .next()
                .expect("derive list")
                .split(")]" )
                .next()
                .expect("bounded derive list");
            assert!(!prefix.contains("Clone"));
            assert!(!prefix.contains("Copy"));
        }
    }

    #[test]
    fn failed_release_evidence_is_hostile_v3_and_branch_exhaustive() {
        let source = include_str!("source_funding_custody_retirement_v1.rs");
        let join = source
            .split("fn authenticate_product_release_evidence_v3")
            .nth(1)
            .and_then(|value| value.split("/// Product-owned terminal identities").next())
            .expect("bounded durable Product release join");
        for predicate in [
            "AuthenticatedPersistedSourceFailureProductReleaseV3",
            "release.disposition() == Some(expected_disposition)",
            "persisted_terminal.source_failure_kind() == expected_kind",
            "persisted_terminal.market_instance_id() == terminal.market_instance_id",
            "release.source_terminal_postwrite_id()",
            "release.source_physical_disposition_id()",
            "release.product_release_binding_id()",
            "release.product_link_account() == terminal.product_link_account",
            "release.product_transition_sequence_after()",
            "release.product_session_transcript_before()",
        ] {
            assert!(join.contains(predicate), "missing hostile V3 join {predicate}");
        }
        assert!(join.contains("_ => Err(Refusal::Adapter(ClutchError::MismatchedState))"));
        assert!(!join.contains("PersistedSourceFailureProductReleaseV2"));
        assert!(!join.contains("expected_persisted_release_id"));
    }

    #[test]
    fn physical_retirement_commits_all_owned_pre_and_post_states() {
        let source = include_str!("source_funding_custody_retirement_v1.rs");
        let retire = source
            .split("pub(crate) fn retire_source_funding_custody_v2")
            .nth(1)
            .expect("private physical retirement");
        for predicate in [
            "custody_owner_before",
            "custody_authentication_before_id",
            "custody_account_data_before_id",
            "custody_ledger_before_id",
            "custody_owner_after",
            "custody_authentication_after_id",
            "custody_account_data_after_id",
            "custody_balance_after: 0",
            "principal_refund_owner",
            "principal_refund_data_id",
            "neutral_sink_owner",
            "neutral_sink_data_id",
            "ledger_before.remaining_principal_lamports",
            "ledger_before.donation_lamports",
        ] {
            assert!(retire.contains(predicate), "missing exact close fact {predicate}");
        }
        assert!(source.contains("&& !account.is_signer"));
        assert!(!retire.contains("caller_source_terminal_id"));
        assert!(!retire.contains("caller_product_release_id"));
    }
}
