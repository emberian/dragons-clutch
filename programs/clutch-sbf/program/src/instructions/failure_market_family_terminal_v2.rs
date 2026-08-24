// SPDX-License-Identifier: AGPL-3.0-or-later
//! Atomic Failure-family aggregate, permanent replay, and history seal.
//!
//! Recovery close alone is not Product authority. This private composer joins
//! the hostile-reopened `RecoveryClosed` runtime and canonical Idle interval
//! pair, mints the exact pre-replay aggregate, writes fresh permanent
//! `0xa3/v2`, advances `0xa0/v3` to `FamilyTerminal`, and only then seals the
//! append-only `0xac/v2` history. The same-instruction postwrite is not Product
//! authority: a later consumer must hostile-reopen the complete tuple and move
//! the resulting V3 Failure-family receipt into the Product RootV3 writer.

use crate::accounts::{require, require_distinct, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::SYSTEM_PROGRAM_ID;
use crate::instructions::failure_market_admission::{
    authenticate_failure_market_root_v2, AuthenticatedFailureMarketRootV2,
};
use crate::instructions::failure_market_interval_v2::{
    authenticate_failure_market_interval_accounts_v2, write_failure_market_interval_family_seal_v2,
    AuthenticatedFailureMarketIntervalAccountsV2,
};
use crate::instructions::failure_market_recovery_terminal_v2::AuthenticatedFailureMarketRecoveryClosePostwriteV2;
use crate::instructions::failure_market_replay_v2::{
    authenticate_failure_market_replay_v2, write_failure_market_replay_terminal_v2,
    AuthenticatedFailureMarketReplayV2,
};
use crate::instructions::failure_market_runtime::{
    authenticate_failure_market_runtime_root_v1, write_failure_market_runtime_terminal_plan_v2,
    AuthenticatedFailureMarketRuntimeRootV1,
};
use crate::instructions::product_series_current::{
    authenticate_market_lifecycle_root_v2, AuthenticatedMarketLifecycleRootV2,
};
use crate::instructions::product_market_lifecycle_v3_current::{
    authenticate_market_lifecycle_root_v3, AuthenticatedMarketLifecycleRootV3,
    AuthenticatedSeriesMarketLinkV3,
};
use crate::instructions::product_series_current::AuthenticatedSeriesFundingAccountV5;
use crate::instructions::source_failure_product_release_v1::{
    authenticate_persisted_source_failure_product_release_v3,
    AuthenticatedPersistedSourceFailureProductReleaseV3,
};
use crate::instructions::source_funding_custody_retirement_v1::{
    authenticate_source_family_terminal_authority_v3,
    consume_source_family_terminal_into_product_v3, retire_source_funding_custody_v3,
    AuthenticatedSourceFundingCustodyLifecycleTerminalAuthorityV1,
    SourceFundingCustodyLifecycleTerminalEvidenceV1,
    SourceFundingCustodyLifecycleTerminalFactsV1, SourceFundingCustodyLiveFounderFactsV1,
    SourceFundingCustodyTerminalDispositionV1,
};
use crate::source_plane_v3_actions::authenticate_source_funding_custody_v1;
use clutch_failure_policy_runtime::market_interval_history_v2::{
    plan_close_failure_market_interval_accounts_v2,
    plan_seal_failure_market_interval_history_v2,
    reconstruct_failure_market_interval_family_seal_v2,
    AuthenticatedFailureMarketIntervalFamilySealV2, FailureMarketIntervalFamilySealFactsV2,
    FailureMarketIntervalFamilySealReceiptV2, FailureMarketIntervalFundingReceiptV2,
};
use clutch_failure_policy_runtime::market_replay_v2::{
    plan_terminalize_failure_market_replay_v2, AuthenticatedFailureMarketReplayTerminalV2,
    FailureMarketReplayFundingReceiptV2, FailureMarketReplayTerminalFactsV2,
    FailureMarketReplayTerminalReceiptV2,
};
use clutch_failure_policy_runtime::market_quote_v1::FailureMarketRecoveryQuoteAdmissionReceiptV1;
use clutch_failure_policy_runtime::market_runtime_v1::{
    admit_failure_market_family_aggregate_v2, plan_finalize_failure_market_family_v2,
    AuthenticatedFailureMarketFamilyAggregateV2, AuthenticatedFailureMarketFamilyTerminalV2,
    FailureMarketFamilyAggregateFactsV2, FailureMarketFamilyAggregateReceiptV2,
    FailureMarketFamilyTerminalDispositionV2, FailureMarketFamilyTerminalFactsV2,
    FailureMarketFamilyTerminalReceiptV2,
};
use clutch_product_series::{ContentId, MarketLifecyclePhaseV2, MarketLifecyclePhaseV3};
use clutch_source_plane_v3::ContentId as SourceContentId;
use clutch_source_plane_v3_runtime::{
    AuthenticatedSourceRouteV1, SourceFailureProductReleaseDispositionV3,
    SourceWorkScheduleBindingV1,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV2, MarketLifecycleRootAccountV3, SeriesMarketLinkAccountV3,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const FAMILY_TERMINAL_POSTWRITE_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/failure-market-family-terminal-postwrite/v2";
const FAMILY_TERMINAL_OWNER_RELEASE_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/failure-market-terminal-owner-release/v2";
const FAMILY_TERMINAL_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/failure-market-terminal-authentication/v2";
const FAMILY_TERMINAL_RECEIPT_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/failure-market-family-terminal-receipt/v3";
const FAMILY_PHYSICAL_CLOSE_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/failure-market-family-physical-close/v3";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketFamilyAggregateAuthorityV2 {
    expected: FailureMarketFamilyAggregateFactsV2,
}

impl AuthenticatedFailureMarketFamilyAggregateV2 for FailureMarketFamilyAggregateAuthorityV2 {
    fn authenticate_failure_market_family_aggregate(
        &self,
        expected: FailureMarketFamilyAggregateFactsV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if expected != self.expected {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketReplayTerminalAuthorityV2 {
    replay_before: clutch_failure_policy_runtime::market_replay_v2::FailureMarketReplayStateIdV2,
    replay_account: clutch_failure_policy_runtime::market_policy_v1::FailureMarketAccountIdV1,
    funding_receipt_id:
        clutch_failure_policy_runtime::market_replay_v2::FailureMarketReplayFundingReceiptIdV2,
    family_aggregate_receipt_id:
        clutch_failure_policy_runtime::market_runtime_v1::FailureMarketFamilyAggregateReceiptIdV2,
    runtime_terminal_state_commitment:
        clutch_failure_policy_runtime::market_runtime_v1::FailureMarketRuntimeStateCommitmentV1,
}

impl AuthenticatedFailureMarketReplayTerminalV2 for FailureMarketReplayTerminalAuthorityV2 {
    fn authenticate_failure_market_replay_terminal(
        &self,
        expected: FailureMarketReplayTerminalFactsV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if expected.replay_before != self.replay_before
            || expected.replay_account != self.replay_account
            || expected.funding_receipt_id != self.funding_receipt_id
            || expected.family_aggregate_receipt_id != self.family_aggregate_receipt_id
            || expected.runtime_terminal_state_commitment != self.runtime_terminal_state_commitment
            || expected.replay_after.bytes() == [0; 32]
            || expected.replay_after == expected.replay_before
        {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketFamilyTerminalAuthorityV2 {
    expected: FailureMarketFamilyTerminalFactsV2,
}

impl AuthenticatedFailureMarketFamilyTerminalV2 for FailureMarketFamilyTerminalAuthorityV2 {
    fn authenticate_failure_market_family_terminal(
        &self,
        expected: FailureMarketFamilyTerminalFactsV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if expected != self.expected {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketHistorySealAuthorityV2 {
    expected: FailureMarketIntervalFamilySealFactsV2,
}

impl AuthenticatedFailureMarketIntervalFamilySealV2 for FailureMarketHistorySealAuthorityV2 {
    fn authenticate_failure_market_interval_family_seal(
        &self,
        expected: FailureMarketIntervalFamilySealFactsV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if expected != self.expected {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

/// Same-instruction Failure-family postwrite.
///
/// This value proves that all Failure writes in the resolving instruction
/// completed, but it is deliberately not Product terminal authority. A later
/// Product RootV3 transition must hostile-reopen the durable tuple and consume
/// [`AuthenticatedFailureMarketFamilyTerminalReceiptV3`] instead.
#[derive(Debug)]
pub(crate) struct AuthenticatedFailureMarketFamilyTerminalPostwriteV2 {
    id: ContentId,
    owner_release_id: ContentId,
    admission: AuthenticatedFailureMarketRootV2,
    aggregate: FailureMarketFamilyAggregateReceiptV2,
    replay_terminal: FailureMarketReplayTerminalReceiptV2,
    family_terminal: FailureMarketFamilyTerminalReceiptV2,
    family_seal: FailureMarketIntervalFamilySealReceiptV2,
    replay: AuthenticatedFailureMarketReplayV2,
    runtime: AuthenticatedFailureMarketRuntimeRootV1,
    interval: AuthenticatedFailureMarketIntervalAccountsV2,
}

impl AuthenticatedFailureMarketFamilyTerminalPostwriteV2 {
    /// Complete physical/semantic terminal identity.
    pub(crate) const fn id(&self) -> ContentId {
        self.id
    }

    /// Fresh release of the complete authenticated Failure terminal owner.
    pub(crate) const fn owner_release_id(&self) -> ContentId {
        self.owner_release_id
    }

    /// Exact immutable Failure admission root retained through Product consume.
    pub(crate) const fn admission(&self) -> AuthenticatedFailureMarketRootV2 {
        self.admission
    }

    /// Full-width shared Market.
    pub(crate) const fn market_instance_id(
        &self,
    ) -> clutch_product_series::MarketInstanceV2Id {
        self.family_terminal.facts().market_instance_id
    }

    /// Shared Failure/liveness generation.
    pub(crate) const fn generation(&self) -> u64 {
        self.family_terminal.facts().generation
    }

    /// Permanent replay is the Product shared-core owner account.
    pub(crate) const fn owner_account_id(&self) -> ContentId {
        ContentId::from_bytes(self.replay.account().to_bytes())
    }

    /// Exact immutable admission-root account.
    pub(crate) const fn admission_root_account(&self) -> Pubkey {
        self.admission.account()
    }

    /// Exact immutable admission semantic state.
    pub(crate) const fn admission_state_id(
        &self,
    ) -> clutch_failure_policy_runtime::market_policy_v1::FailureMarketAdmissionStateIdV1 {
        self.family_terminal.facts().admission_state_id
    }

    /// Exact mutable runtime-root account.
    pub(crate) const fn runtime_root_account(&self) -> Pubkey {
        self.runtime.account()
    }

    /// Exact final runtime semantic commitment.
    pub(crate) const fn runtime_state_commitment(
        &self,
    ) -> clutch_failure_policy_runtime::market_runtime_v1::FailureMarketRuntimeStateCommitmentV1 {
        self.runtime.state_commitment()
    }

    /// Exact permanent replay account.
    pub(crate) const fn replay_account(&self) -> Pubkey {
        self.replay.account()
    }

    /// Complete terminal replay semantic state.
    pub(crate) const fn replay_state_id(
        &self,
    ) -> clutch_failure_policy_runtime::market_replay_v2::FailureMarketReplayStateIdV2 {
        self.replay.state_id()
    }

    /// Owner/PDA/frame/body/balance authentication of terminal replay.
    pub(crate) const fn replay_authentication_id(&self) -> ContentId {
        self.replay.authentication_id()
    }

    /// Exact sealed append-only history account.
    pub(crate) const fn history_account(&self) -> Pubkey {
        self.interval.history_account()
    }

    /// Complete sealed history semantic state.
    pub(crate) const fn history_state_id(
        &self,
    ) -> clutch_failure_policy_runtime::market_interval_history_v2::FailureMarketIntervalHistoryStateIdV2 {
        self.interval.history_state_id()
    }

    /// Owner/PDA/frame/body/balance authentication of sealed history.
    pub(crate) const fn history_authentication_id(&self) -> ContentId {
        self.interval.history_authentication_id()
    }

    /// Sole append-only Market history root.
    pub(crate) const fn history_root(
        &self,
    ) -> clutch_failure_policy_runtime::market_interval_history_v2::FailureMarketIntervalHistoryRootV2 {
        self.interval.history().history_root()
    }

    /// Intermediate pre-replay aggregate.
    pub(crate) const fn aggregate(&self) -> FailureMarketFamilyAggregateReceiptV2 {
        self.aggregate
    }

    /// Exact permanent replay terminal receipt.
    pub(crate) const fn replay_terminal(&self) -> FailureMarketReplayTerminalReceiptV2 {
        self.replay_terminal
    }

    /// Sole typed Failure-family terminal receipt for Product.
    pub(crate) const fn family_terminal(&self) -> FailureMarketFamilyTerminalReceiptV2 {
        self.family_terminal
    }

    /// Durable Source-to-current-Product release binding folded before
    /// Recovery close and repeated in the terminal receipt.
    pub(crate) const fn source_product_release_binding_id(&self) -> ContentId {
        self.family_terminal.facts().source_product_release_binding_id
    }

    /// Exact current Product LinkV2 account whose resolved release was folded.
    pub(crate) const fn source_product_link_account_id(&self) -> ContentId {
        self.family_terminal.facts().source_product_link_account_id
    }

    /// Exact successful Source terminal folded through Recovery and replay.
    pub(crate) const fn source_resolution_terminal_postwrite_id(&self) -> ContentId {
        self.family_terminal
            .facts()
            .source_resolution_terminal_receipt_id
    }

    /// Exact successful StatisticResult/lineage physical-close postwrite.
    pub(crate) const fn source_result_close_receipt_id(&self) -> ContentId {
        self.family_terminal.facts().source_result_close_receipt_id
    }

    /// Exact append-only history seal.
    pub(crate) const fn family_seal(&self) -> FailureMarketIntervalFamilySealReceiptV2 {
        self.family_seal
    }

    /// Hostile-reopened permanent replay postimage.
    pub(crate) const fn replay(&self) -> AuthenticatedFailureMarketReplayV2 {
        self.replay
    }

    /// Hostile-reopened `FamilyTerminal` runtime postimage.
    pub(crate) const fn runtime(&self) -> AuthenticatedFailureMarketRuntimeRootV1 {
        self.runtime
    }

    /// Canonical Idle cell and sealed full history postimage.
    pub(crate) const fn interval(&self) -> AuthenticatedFailureMarketIntervalAccountsV2 {
        self.interval
    }

    fn derived_owner_release_id(&self) -> Outcome<ContentId> {
        derive_terminal_owner_release_id_v2(
            self.admission,
            self.runtime,
            self.replay,
            self.interval,
        )
    }

    fn derived_postwrite_id(&self) -> ContentId {
        ContentId::from_bytes(
            solana_sha256_hasher::hashv(&[
                FAMILY_TERMINAL_POSTWRITE_DOMAIN_V2,
                &self.owner_release_id.bytes(),
                &self.aggregate.id().bytes(),
                &self.replay_terminal.id().bytes(),
                &self.family_terminal.id().bytes(),
                &self.family_seal.id().bytes(),
                &self.replay.authentication_id().bytes(),
                &self.runtime.state_commitment().bytes(),
                &self.interval.history_authentication_id().bytes(),
            ])
            .to_bytes(),
        )
    }
}

/// Internal hostile-reopened durable terminal owner.
///
/// The resolution instruction cannot carry its private postwrite across
/// transactions. Product never receives this internal owner directly: the
/// RootV3 boundary wraps it in one move-only V3 receipt after authenticating
/// the unique canonical a0/a3/ab/ac poststate.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedFailureMarketFamilyTerminalOwnerV2 {
    id: ContentId,
    owner_release_id: ContentId,
    family_terminal_receipt_id: ContentId,
    admission: AuthenticatedFailureMarketRootV2,
    runtime: AuthenticatedFailureMarketRuntimeRootV1,
    replay: AuthenticatedFailureMarketReplayV2,
    interval: AuthenticatedFailureMarketIntervalAccountsV2,
}

impl AuthenticatedFailureMarketFamilyTerminalOwnerV2 {
    pub(crate) const fn id(&self) -> ContentId {
        self.id
    }

    pub(crate) const fn owner_release_id(&self) -> ContentId {
        self.owner_release_id
    }

    pub(crate) const fn family_terminal_receipt_id(&self) -> ContentId {
        self.family_terminal_receipt_id
    }

    /// Durable Source-to-current-Product release binding recovered from the
    /// hostile-decoded terminal runtime, never from a caller projection.
    pub(crate) const fn source_product_release_binding_id(&self) -> ContentId {
        self.runtime.state().source_product_release_binding_id()
    }

    /// Exact current Product link account recovered from terminal runtime.
    pub(crate) const fn source_product_link_account_id(&self) -> ContentId {
        self.runtime.state().source_product_link_account_id()
    }

    /// Exact successful Source terminal recovered from the terminal runtime.
    pub(crate) const fn source_resolution_terminal_postwrite_id(&self) -> ContentId {
        self.runtime
            .state()
            .source_resolution_terminal_postwrite_id()
    }

    /// Exact successful StatisticResult/lineage close recovered from runtime.
    pub(crate) const fn source_result_close_receipt_id(&self) -> ContentId {
        self.runtime.state().source_result_close_receipt_id()
    }

    pub(crate) const fn admission(&self) -> AuthenticatedFailureMarketRootV2 {
        self.admission
    }

    pub(crate) const fn runtime(&self) -> AuthenticatedFailureMarketRuntimeRootV1 {
        self.runtime
    }

    pub(crate) const fn replay(&self) -> AuthenticatedFailureMarketReplayV2 {
        self.replay
    }

    pub(crate) const fn interval(&self) -> AuthenticatedFailureMarketIntervalAccountsV2 {
        self.interval
    }

    /// Reconstruct the unique private seal receipt from the durable terminal
    /// owner. No earlier transaction-local seal token is required.
    pub(crate) fn family_seal(&self) -> Outcome<FailureMarketIntervalFamilySealReceiptV2> {
        reconstruct_failure_market_interval_family_seal_v2(
            &self,
            self.interval.history(),
            self.admission.state(),
            self.interval.quote(),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
    }

    fn derived_owner_release_id(&self) -> Outcome<ContentId> {
        derive_terminal_owner_release_id_v2(
            self.admission,
            self.runtime,
            self.replay,
            self.interval,
        )
    }

    fn derived_authentication_id(&self) -> ContentId {
        ContentId::from_bytes(
            solana_sha256_hasher::hashv(&[
                FAMILY_TERMINAL_AUTHENTICATION_DOMAIN_V2,
                &self.owner_release_id.bytes(),
                &self.family_terminal_receipt_id.bytes(),
                &self.admission.account().to_bytes(),
                &self.runtime.account().to_bytes(),
                &self.runtime.state_commitment().bytes(),
                &self.replay.account().to_bytes(),
                &self.replay.state_id().bytes(),
                &self.replay.authentication_id().bytes(),
                &self.interval.cell_account().to_bytes(),
                &self.interval.cell_state_id().bytes(),
                &self.interval.cell_authentication_id().bytes(),
                &self.interval.history_account().to_bytes(),
                &self.interval.history_state_id().bytes(),
                &self.interval.history_authentication_id().bytes(),
            ])
            .to_bytes(),
        )
    }
}

/// Exact Failure-owned facts an incoming Product RootV3 consumer must bind.
///
/// Product supplies its own live RootV3 transition sequence and constructs its
/// own shared-core projection. Failure owns only this complete physical owner
/// tuple; no Product root, link, phase, or postwrite receipt is accepted here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FailureMarketFamilyTerminalConsumerFactsV3 {
    pub market_instance_id: clutch_product_series::MarketInstanceV2Id,
    pub generation: u64,
    pub owner_account_id: ContentId,
    pub owner_release_id: ContentId,
    pub owner_terminal_receipt_id: ContentId,
    pub admission_root_account: Pubkey,
    pub admission_state_id: ContentId,
    pub failure_policy_binding_id: ContentId,
    pub runtime_root_account: Pubkey,
    pub runtime_state_commitment: ContentId,
    pub recovery_terminal_receipt_id: ContentId,
    pub replay_account: Pubkey,
    pub replay_state_id: ContentId,
    pub replay_authentication_id: ContentId,
    pub interval_cell_account: Pubkey,
    pub interval_cell_state_id: ContentId,
    pub interval_cell_authentication_id: ContentId,
    pub interval_history_account: Pubkey,
    pub interval_history_state_id: ContentId,
    pub interval_history_authentication_id: ContentId,
    pub interval_history_root: ContentId,
    pub source_resolution_terminal_postwrite_id: ContentId,
    pub source_result_close_receipt_id: ContentId,
    pub source_product_release_binding_id: ContentId,
    pub source_product_link_account_id: ContentId,
}

/// Move-only durable Failure-family terminal receipt for Product RootV3.
///
/// This is minted only by hostile-reopening the immutable admission root,
/// `FamilyTerminal` runtime, terminal replay, canonical Idle cell, and sealed
/// history. It is intentionally not `Clone` or `Copy`, and it contains no
/// Product final receipt or LinkV2/LinkV3 object.
#[derive(Debug)]
pub(crate) struct AuthenticatedFailureMarketFamilyTerminalReceiptV3 {
    id: ContentId,
    facts: FailureMarketFamilyTerminalConsumerFactsV3,
    family_seal_id: ContentId,
    owner: AuthenticatedFailureMarketFamilyTerminalOwnerV2,
}

impl AuthenticatedFailureMarketFamilyTerminalReceiptV3 {
    pub(crate) const fn id(&self) -> ContentId {
        self.id
    }

    pub(crate) const fn facts(&self) -> FailureMarketFamilyTerminalConsumerFactsV3 {
        self.facts
    }

    pub(crate) const fn family_seal_id(&self) -> ContentId {
        self.family_seal_id
    }

    /// Borrow the hostile durable owner without duplicating the capability.
    pub(crate) const fn owner(&self) -> &AuthenticatedFailureMarketFamilyTerminalOwnerV2 {
        &self.owner
    }

    /// Consume the one durable Failure terminal into Product's current V3
    /// root facts and the unique Source-lifecycle owner.  Product must persist
    /// the exact facts before it may move the owner into Source custody
    /// retirement; neither half is constructible independently.
    pub(crate) fn into_product_v3_parts(
        self,
    ) -> (
        FailureMarketFamilyTerminalConsumerFactsV3,
        AuthenticatedFailureMarketFamilyTerminalOwnerV2,
    ) {
        (self.facts, self.owner)
    }

    fn from_owner(owner: AuthenticatedFailureMarketFamilyTerminalOwnerV2) -> Outcome<Self> {
        let policy = owner.admission().state().binding().facts();
        let admission_state_id = owner
            .admission()
            .state()
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let failure_policy_binding_id = owner.admission().state().binding().id();
        let runtime = owner.runtime();
        let replay = owner.replay();
        let interval = owner.interval();
        let family_seal_id = ContentId::from_bytes(owner.family_seal()?.id().bytes());
        let facts = FailureMarketFamilyTerminalConsumerFactsV3 {
            market_instance_id: policy.market_instance_id,
            generation: policy.generation,
            owner_account_id: ContentId::from_bytes(replay.account().to_bytes()),
            owner_release_id: owner.owner_release_id(),
            owner_terminal_receipt_id: owner.family_terminal_receipt_id(),
            admission_root_account: owner.admission().account(),
            admission_state_id: ContentId::from_bytes(admission_state_id.bytes()),
            failure_policy_binding_id: ContentId::from_bytes(failure_policy_binding_id.bytes()),
            runtime_root_account: runtime.account(),
            runtime_state_commitment: ContentId::from_bytes(
                runtime.state_commitment().bytes(),
            ),
            recovery_terminal_receipt_id: runtime.state().recovery_terminal_receipt_id(),
            replay_account: replay.account(),
            replay_state_id: ContentId::from_bytes(replay.state_id().bytes()),
            replay_authentication_id: replay.authentication_id(),
            interval_cell_account: interval.cell_account(),
            interval_cell_state_id: ContentId::from_bytes(interval.cell_state_id().bytes()),
            interval_cell_authentication_id: interval.cell_authentication_id(),
            interval_history_account: interval.history_account(),
            interval_history_state_id: ContentId::from_bytes(
                interval.history_state_id().bytes(),
            ),
            interval_history_authentication_id: interval.history_authentication_id(),
            interval_history_root: ContentId::from_bytes(
                interval.history().history_root().bytes(),
            ),
            source_resolution_terminal_postwrite_id: runtime
                .state()
                .source_resolution_terminal_postwrite_id(),
            source_result_close_receipt_id: runtime.state().source_result_close_receipt_id(),
            source_product_release_binding_id: runtime
                .state()
                .source_product_release_binding_id(),
            source_product_link_account_id: runtime.state().source_product_link_account_id(),
        };
        let id = ContentId::from_bytes(
            solana_sha256_hasher::hashv(&[
                FAMILY_TERMINAL_RECEIPT_DOMAIN_V3,
                &owner.id().bytes(),
                &facts.market_instance_id.bytes(),
                &facts.generation.to_le_bytes(),
                &facts.owner_account_id.bytes(),
                &facts.owner_release_id.bytes(),
                &facts.owner_terminal_receipt_id.bytes(),
                &facts.admission_root_account.to_bytes(),
                &facts.admission_state_id.bytes(),
                &facts.failure_policy_binding_id.bytes(),
                &facts.runtime_root_account.to_bytes(),
                &facts.runtime_state_commitment.bytes(),
                &facts.recovery_terminal_receipt_id.bytes(),
                &facts.replay_account.to_bytes(),
                &facts.replay_state_id.bytes(),
                &facts.replay_authentication_id.bytes(),
                &facts.interval_cell_account.to_bytes(),
                &facts.interval_cell_state_id.bytes(),
                &facts.interval_cell_authentication_id.bytes(),
                &facts.interval_history_account.to_bytes(),
                &facts.interval_history_state_id.bytes(),
                &facts.interval_history_authentication_id.bytes(),
                &facts.interval_history_root.bytes(),
                &facts.source_resolution_terminal_postwrite_id.bytes(),
                &facts.source_result_close_receipt_id.bytes(),
                &facts.source_product_release_binding_id.bytes(),
                &facts.source_product_link_account_id.bytes(),
                &family_seal_id.bytes(),
            ])
            .to_bytes(),
        );
        require(
            !id.is_zero()
                && id != facts.owner_release_id
                && id != facts.owner_terminal_receipt_id
                && id != facts.owner_account_id
                && family_seal_id != facts.owner_terminal_receipt_id,
            ClutchError::MismatchedState,
        )?;
        Ok(Self {
            id,
            facts,
            family_seal_id,
            owner,
        })
    }
}

/// Default-refusing boundary for the future narrow Product RootV3 writer.
///
/// The Product owner must exact-compare these Failure-derived facts against
/// its hostile live RootV3/LinkV3 retirement state before recording the
/// `MarketSharedCoreV3::Failure` projection.
pub(crate) trait AuthenticatedFailureMarketFamilyTerminalAuthorityV3 {
    fn authenticate_failure_market_family_terminal_v3(
        &self,
        _expected: FailureMarketFamilyTerminalConsumerFactsV3,
    ) -> clutch_failure_policy_runtime::Result<()> {
        Err(clutch_failure_policy_runtime::Error::BindingMismatch)
    }
}

impl AuthenticatedFailureMarketFamilyTerminalAuthorityV3
    for AuthenticatedFailureMarketFamilyTerminalReceiptV3
{
    fn authenticate_failure_market_family_terminal_v3(
        &self,
        expected: FailureMarketFamilyTerminalConsumerFactsV3,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if expected != self.facts {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

impl AuthenticatedSourceFundingCustodyLifecycleTerminalAuthorityV1
    for AuthenticatedFailureMarketFamilyTerminalReceiptV3
{
    fn into_source_funding_custody_lifecycle_terminal_evidence_v1(
        self,
        founder: SourceFundingCustodyLiveFounderFactsV1,
    ) -> Outcome<SourceFundingCustodyLifecycleTerminalEvidenceV1> {
        let owner = self.owner;
        let facts = successful_source_custody_terminal_facts_v1(
            owner.admission.state().binding().facts(),
            owner.runtime.state().source_resolution_terminal_postwrite_id(),
            owner.runtime.state().source_result_close_receipt_id(),
            owner.runtime.state().source_product_release_binding_id(),
            owner.runtime.state().source_product_link_account_id(),
            owner.family_terminal_receipt_id,
            founder,
        )?;
        Ok(SourceFundingCustodyLifecycleTerminalEvidenceV1::successful(facts))
    }
}

/// Hostile durable join for a SourceAbsent or SourceRefused occurrence whose
/// exact per-Link terminal preceded the later Market-wide Failure family
/// terminal. The move-only V3 Source projection owns the two physical Source
/// lifecycle identities; the Failure family owner supplies only the exact
/// final Market-family receipt.
#[derive(Debug)]
pub(crate) struct AuthenticatedFailureMarketSourceFailureLifecycleTerminalV3 {
    source: AuthenticatedPersistedSourceFailureProductReleaseV3,
    family: AuthenticatedFailureMarketFamilyTerminalReceiptV3,
}

impl AuthenticatedFailureMarketSourceFailureLifecycleTerminalV3 {
    pub(crate) const fn source(
        &self,
    ) -> &AuthenticatedPersistedSourceFailureProductReleaseV3 {
        &self.source
    }

    pub(crate) const fn family(&self) -> &AuthenticatedFailureMarketFamilyTerminalReceiptV3 {
        &self.family
    }
}

/// Reopen the one-way bound Source failure account and join it to the unique
/// hostile Failure family owner. No terminal ID, branch, or Product-Link fact
/// is supplied by the instruction payload.
pub(crate) fn authenticate_failure_market_source_failure_lifecycle_terminal_v3(
    program_id: &Pubkey,
    route: AuthenticatedSourceRouteV1,
    source_terminal_account: &AccountInfo<'_>,
    family: AuthenticatedFailureMarketFamilyTerminalReceiptV3,
) -> Outcome<AuthenticatedFailureMarketSourceFailureLifecycleTerminalV3> {
    let source = authenticate_persisted_source_failure_product_release_v3(
        program_id,
        route,
        source_terminal_account,
    )?;
    let terminal = source.terminal();
    let family_owner = family.owner();
    let policy_binding = family_owner.admission.state().binding();
    let policy = policy_binding.facts();
    let policy_binding_id = policy_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        terminal.source_release_manifest_id() == route.release_manifest_id()
            && terminal.source_release_authentication_id() == route.release_authentication_id()
            && terminal.route_id() == route.route_id()
            && terminal.source_plane_contract_id() == route.source_plane_contract_id()
            && terminal.source_spec_id() == route.source_spec_id()
            && terminal.source_work_schedule_id() == route.source_work_schedule_id()
            && terminal.market_instance_id().bytes() == policy.market_instance_id.bytes()
            && terminal.failure_policy_binding_id().bytes() == policy_binding_id.bytes()
            && terminal.failure_generation() == policy.generation
            && source.product_link_account().bytes() != [0; 32]
            && source.product_link_account().bytes()
                != family_owner.admission.account().to_bytes()
            && source.product_link_account().bytes() != family_owner.runtime.account().to_bytes()
            && source.product_link_account().bytes() != family_owner.replay.account().to_bytes()
            && source.product_link_account().bytes()
                != family_owner.interval.cell_account().to_bytes()
            && source.product_link_account().bytes()
                != family_owner.interval.history_account().to_bytes()
            && !source.source_terminal_postwrite_id().is_zero()
            && !source.source_physical_disposition_id().is_zero()
            && !source.id().is_zero()
            && !family_owner.family_terminal_receipt_id.is_zero(),
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedFailureMarketSourceFailureLifecycleTerminalV3 { source, family })
}

impl AuthenticatedSourceFundingCustodyLifecycleTerminalAuthorityV1
    for AuthenticatedFailureMarketSourceFailureLifecycleTerminalV3
{
    fn into_source_funding_custody_lifecycle_terminal_evidence_v1(
        self,
        founder: SourceFundingCustodyLiveFounderFactsV1,
    ) -> Outcome<SourceFundingCustodyLifecycleTerminalEvidenceV1> {
        let terminal = self.source.terminal();
        let disposition = match self.source.disposition() {
            Some(SourceFailureProductReleaseDispositionV3::SourceAbsent) => {
                SourceFundingCustodyTerminalDispositionV1::SourceAbsent
            }
            Some(SourceFailureProductReleaseDispositionV3::SourceRefused) => {
                SourceFundingCustodyTerminalDispositionV1::SourceRefused
            }
            None => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
        };
        let family_owner = self.family.owner();
        let policy_binding = family_owner.admission.state().binding();
        let policy = policy_binding.facts();
        let policy_binding_id = policy_binding
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            founder.product_link_account == self.source.product_link_account()
                && founder.market_instance_id == terminal.market_instance_id()
                && founder.market_instance_id.bytes() == policy.market_instance_id.bytes()
                && terminal.failure_policy_binding_id().bytes() == policy_binding_id.bytes()
                && founder.source_release_manifest_id == terminal.source_release_manifest_id()
                && founder.source_release_authentication_id
                    == terminal.source_release_authentication_id()
                && founder.source_route_id == terminal.route_id()
                && founder.source_work_schedule_id == terminal.source_work_schedule_id()
                && founder.source_generation == terminal.failure_generation()
                && founder.source_repair_generation == terminal.source_repair_generation()
                && founder.neutral_lamport_sink.bytes() == policy.neutral_sink.bytes()
                && self.source.source_terminal_postwrite_id()
                    != self.source.source_physical_disposition_id()
                && self.source.id() != self.source.source_terminal_postwrite_id()
                && self.source.id() != self.source.source_physical_disposition_id()
                && ContentId::from_bytes(self.source.id().bytes())
                    != family_owner.family_terminal_receipt_id,
            ClutchError::MismatchedState,
        )?;
        let facts = SourceFundingCustodyLifecycleTerminalFactsV1 {
            disposition,
            capitalization_authority_id: founder.capitalization_authority_id,
            capitalization_receipt_id: founder.capitalization_receipt_id,
            pre_root_source_occurrence_id: founder.pre_root_source_occurrence_id,
            product_link_account: founder.product_link_account,
            product_link_account_data_id: founder.product_link_account_data_id,
            product_link_authentication_id: founder.product_link_authentication_id,
            product_link_semantic_id: founder.product_link_semantic_id,
            product_link_transition_sequence: founder.product_link_transition_sequence,
            source_terminal_postwrite_id: self.source.source_terminal_postwrite_id(),
            source_result_or_absence_close_receipt_id: self
                .source
                .source_physical_disposition_id(),
            source_product_release_binding_id: self.source.product_release_binding_id(),
            failure_family_terminal_receipt_id: SourceContentId::from_bytes(
                family_owner.family_terminal_receipt_id.bytes(),
            ),
            market_instance_id: founder.market_instance_id,
            series_plan_id: founder.series_plan_id,
            ordinal: founder.ordinal,
            source_generation: founder.source_generation,
            source_release_manifest_id: founder.source_release_manifest_id,
            source_release_authentication_id: founder.source_release_authentication_id,
            source_route_id: founder.source_route_id,
            source_work_schedule_id: founder.source_work_schedule_id,
            source_lifecycle_id: founder.source_lifecycle_id,
            source_occurrence_id: founder.source_occurrence_id,
            source_occurrence_account: founder.source_occurrence_account,
            source_occurrence_authentication_id: founder.source_occurrence_authentication_id,
            source_repair_generation: founder.source_repair_generation,
            source_funding_custody: founder.source_funding_custody,
            lamport_principal_refund: founder.lamport_principal_refund,
            neutral_lamport_sink: founder.neutral_lamport_sink,
        };
        Ok(SourceFundingCustodyLifecycleTerminalEvidenceV1::failed(
            facts,
            self.source,
        ))
    }
}

#[allow(clippy::too_many_arguments)]
fn successful_source_custody_terminal_facts_v1(
    policy: clutch_failure_policy_runtime::market_policy_v1::FailureMarketPolicyFactsV1,
    source_terminal: ContentId,
    source_result_close: ContentId,
    source_product_release: ContentId,
    source_product_link_account: ContentId,
    family_terminal_receipt_id: ContentId,
    founder: SourceFundingCustodyLiveFounderFactsV1,
) -> Outcome<SourceFundingCustodyLifecycleTerminalFactsV1> {
    require(
        founder.market_instance_id.bytes() == policy.market_instance_id.bytes()
            && founder.source_release_manifest_id.bytes()
                == policy.source_release_manifest_id.bytes()
            && founder.source_release_authentication_id.bytes()
                == policy.source_release_authentication_id.bytes()
            && founder.source_generation == policy.generation
            && founder.neutral_lamport_sink.bytes() == policy.neutral_sink.bytes()
            && founder.product_link_account.bytes() == source_product_link_account.bytes()
            && !source_terminal.is_zero()
            && !source_result_close.is_zero()
            && !source_product_release.is_zero()
            && !source_product_link_account.is_zero()
            && !family_terminal_receipt_id.is_zero()
            && source_terminal != source_result_close
            && source_terminal != source_product_release
            && source_terminal != family_terminal_receipt_id
            && source_result_close != source_product_release
            && source_result_close != family_terminal_receipt_id
            && source_product_release != family_terminal_receipt_id
            && source_product_link_account != source_terminal
            && source_product_link_account != source_result_close
            && source_product_link_account != source_product_release
            && source_product_link_account != family_terminal_receipt_id,
        ClutchError::MismatchedState,
    )?;
    Ok(SourceFundingCustodyLifecycleTerminalFactsV1 {
        disposition: SourceFundingCustodyTerminalDispositionV1::Successful,
        capitalization_authority_id: founder.capitalization_authority_id,
        capitalization_receipt_id: founder.capitalization_receipt_id,
        pre_root_source_occurrence_id: founder.pre_root_source_occurrence_id,
        product_link_account: founder.product_link_account,
        product_link_account_data_id: founder.product_link_account_data_id,
        product_link_authentication_id: founder.product_link_authentication_id,
        product_link_semantic_id: founder.product_link_semantic_id,
        product_link_transition_sequence: founder.product_link_transition_sequence,
        source_terminal_postwrite_id: SourceContentId::from_bytes(source_terminal.bytes()),
        source_result_or_absence_close_receipt_id: SourceContentId::from_bytes(
            source_result_close.bytes(),
        ),
        source_product_release_binding_id: SourceContentId::from_bytes(
            source_product_release.bytes(),
        ),
        failure_family_terminal_receipt_id: SourceContentId::from_bytes(
            family_terminal_receipt_id.bytes(),
        ),
        market_instance_id: founder.market_instance_id,
        series_plan_id: founder.series_plan_id,
        ordinal: founder.ordinal,
        source_generation: founder.source_generation,
        source_release_manifest_id: founder.source_release_manifest_id,
        source_release_authentication_id: founder.source_release_authentication_id,
        source_route_id: founder.source_route_id,
        source_work_schedule_id: founder.source_work_schedule_id,
        source_lifecycle_id: founder.source_lifecycle_id,
        source_occurrence_id: founder.source_occurrence_id,
        source_occurrence_account: founder.source_occurrence_account,
        source_occurrence_authentication_id: founder.source_occurrence_authentication_id,
        source_repair_generation: founder.source_repair_generation,
        source_funding_custody: founder.source_funding_custody,
        lamport_principal_refund: founder.lamport_principal_refund,
        neutral_lamport_sink: founder.neutral_lamport_sink,
    })
}

impl AuthenticatedFailureMarketIntervalFamilySealV2
    for AuthenticatedFailureMarketFamilyTerminalOwnerV2
{
    fn authenticate_failure_market_interval_family_seal(
        &self,
        expected: FailureMarketIntervalFamilySealFactsV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        let history = self.interval.history();
        if expected.history_before.bytes() == [0; 32]
            || expected.history_before == self.interval.history_state_id()
            || expected.history_root != history.history_root()
            || expected.completed_session_count != history.completed_session_count()
            || expected.completed_work_calls != history.completed_work_calls()
            || expected.exact_reward_lamports != history.exact_reward_lamports()
            || expected.family_terminal_receipt_id.bytes()
                != self.family_terminal_receipt_id.bytes()
            || expected.family_terminal_receipt_id != history.family_terminal_receipt_id()
        {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

fn derive_terminal_owner_release_id_v2(
    admission: AuthenticatedFailureMarketRootV2,
    runtime: AuthenticatedFailureMarketRuntimeRootV1,
    replay: AuthenticatedFailureMarketReplayV2,
    interval: AuthenticatedFailureMarketIntervalAccountsV2,
) -> Outcome<ContentId> {
    let policy = admission.state().binding().facts();
    let admission_state_id = admission
        .state()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            FAMILY_TERMINAL_OWNER_RELEASE_DOMAIN_V2,
            &policy.market_instance_id.bytes(),
            &policy.generation.to_le_bytes(),
            &admission.account().to_bytes(),
            &admission_state_id.bytes(),
            &admission.state().binding().id().bytes(),
            &runtime.account().to_bytes(),
            &runtime.state_commitment().bytes(),
            &runtime.state().recovery_terminal_receipt_id().bytes(),
            &runtime.state().family_terminal_receipt_id().bytes(),
            &runtime.state().source_product_release_binding_id().bytes(),
            &runtime.state().source_product_link_account_id().bytes(),
            &runtime
                .state()
                .source_resolution_terminal_postwrite_id()
                .bytes(),
            &runtime.state().source_result_close_receipt_id().bytes(),
            &replay.account().to_bytes(),
            &replay.state_id().bytes(),
            &replay.authentication_id().bytes(),
            &replay.replay().family_aggregate_receipt_id().bytes(),
            &replay
                .replay()
                .runtime_terminal_state_commitment()
                .bytes(),
            &interval.cell_account().to_bytes(),
            &interval.cell_state_id().bytes(),
            &interval.cell_authentication_id().bytes(),
            &interval.history_account().to_bytes(),
            &interval.history_state_id().bytes(),
            &interval.history_authentication_id().bytes(),
            &interval.history().history_root().bytes(),
            &interval.history().family_terminal_receipt_id().bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(id)
}

/// Reopen the complete persisted Failure terminal tuple after Product has
/// independently retired every Series link. This is the only bridge from the
/// earlier Active-root resolution transaction into the later Retiring-root
/// shared-core latch.
#[allow(clippy::too_many_arguments)]
fn authenticate_failure_market_family_terminal_owner_v2(
    program_id: &Pubkey,
    admission_root_account: &AccountInfo<'_>,
    runtime_root_account: &AccountInfo<'_>,
    interval_cell_account: &AccountInfo<'_>,
    interval_history_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    admission: AuthenticatedFailureMarketRootV2,
    interval_funding: FailureMarketIntervalFundingReceiptV2,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    replay_funding: FailureMarketReplayFundingReceiptV2,
    admission_writable: bool,
    runtime_writable: bool,
    interval_cell_writable: bool,
    interval_history_writable: bool,
    replay_writable: bool,
) -> Outcome<AuthenticatedFailureMarketFamilyTerminalOwnerV2> {
    require_distinct(&[
        admission_root_account.clone(),
        runtime_root_account.clone(),
        interval_cell_account.clone(),
        interval_history_account.clone(),
        replay_account.clone(),
    ])?;
    let live_admission = authenticate_failure_market_root_v2(
        program_id,
        admission_root_account,
        admission_writable,
    )?;
    require(live_admission == admission, ClutchError::MismatchedState)?;
    let runtime = authenticate_failure_market_runtime_root_v1(
        program_id,
        admission_root_account,
        runtime_root_account,
        live_admission,
        runtime_writable,
    )?;
    let interval = authenticate_failure_market_interval_accounts_v2(
        program_id,
        interval_cell_account,
        interval_history_account,
        live_admission,
        interval_funding,
        quote,
        interval_cell_writable,
        interval_history_writable,
    )?;
    let replay = authenticate_failure_market_replay_v2(
        program_id,
        replay_account,
        live_admission,
        replay_funding,
        replay_writable,
    )?;
    let policy = live_admission.state().binding().facts();
    let runtime_state = runtime.state();
    let history = interval.history();
    let replay_state = replay.replay();
    let family_terminal_receipt_id = runtime_state.family_terminal_receipt_id();
    require(
        runtime_state.phase()
            == clutch_failure_policy_runtime::market_runtime_v1::FailureMarketRuntimePhaseV1::FamilyTerminal
            && runtime_state.policy_binding_id() == live_admission.state().binding().id()
            && runtime_state.admission_state_id()
                == live_admission
                    .state()
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            && runtime_state.runtime_account_id().bytes() == runtime.account().to_bytes()
            && runtime_state.recovery_terminal_receipt_id() != ContentId::ZERO
            && family_terminal_receipt_id != ContentId::ZERO
            && runtime_state.completed_session_count() == history.completed_session_count()
            && runtime_state.session_history_commitment() == history.history_root()
            && interval.cell().phase()
                == clutch_failure_policy_runtime::market_interval_cell_v2::FailureMarketIntervalCellPhaseV2::Idle
            && interval.cell().completed_session_count() == history.completed_session_count()
            && history.family_terminal_receipt_id().bytes()
                == family_terminal_receipt_id.bytes()
            && replay_state.phase()
                == clutch_failure_policy_runtime::market_replay_v2::FailureMarketReplayPhaseV2::Terminal
            && replay_state.failure_policy_binding_id() == live_admission.state().binding().id()
            && replay_state.market_instance_id() == policy.market_instance_id
            && replay_state.generation() == policy.generation
            && replay_state.family_aggregate_receipt_id().bytes() != [0; 32]
            && replay_state.runtime_terminal_state_commitment().bytes() != [0; 32]
            && replay_state.runtime_terminal_state_commitment() != runtime.state_commitment(),
        ClutchError::MismatchedState,
    )?;
    let mut authenticated = AuthenticatedFailureMarketFamilyTerminalOwnerV2 {
        id: ContentId::ZERO,
        owner_release_id: ContentId::ZERO,
        family_terminal_receipt_id,
        admission: live_admission,
        runtime,
        replay,
        interval,
    };
    authenticated.owner_release_id = authenticated.derived_owner_release_id()?;
    authenticated.id = authenticated.derived_authentication_id();
    require(
        !authenticated.id.is_zero()
            && authenticated.id != authenticated.owner_release_id
            && authenticated.id != authenticated.family_terminal_receipt_id,
        ClutchError::MismatchedState,
    )?;
    Ok(authenticated)
}

/// Hostile-reopen the complete Failure-owned terminal tuple for the incoming
/// Product RootV3/LinkV3 lifecycle consumer.
///
/// All Failure accounts remain read-only. This function neither accepts nor
/// constructs a Product root, link, shared-core projection, or final receipt;
/// the narrow Product V3 writer must consume the returned move-only receipt in
/// the same instruction that it reauthenticates and advances Product state.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_failure_market_family_terminal_receipt_v3(
    program_id: &Pubkey,
    admission_root_account: &AccountInfo<'_>,
    runtime_root_account: &AccountInfo<'_>,
    interval_cell_account: &AccountInfo<'_>,
    interval_history_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    admission: AuthenticatedFailureMarketRootV2,
    interval_funding: FailureMarketIntervalFundingReceiptV2,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    replay_funding: FailureMarketReplayFundingReceiptV2,
) -> Outcome<AuthenticatedFailureMarketFamilyTerminalReceiptV3> {
    let owner = authenticate_failure_market_family_terminal_owner_v2(
        program_id,
        admission_root_account,
        runtime_root_account,
        interval_cell_account,
        interval_history_account,
        replay_account,
        admission,
        interval_funding,
        quote,
        replay_funding,
        false,
        false,
        false,
        false,
        false,
    )?;
    AuthenticatedFailureMarketFamilyTerminalReceiptV3::from_owner(owner)
}

/// Reopen the exact durable Failure terminal with all deletable accounts
/// writable and the permanent replay read-only. Only the concrete RootV3
/// terminal close below may consume this private prestate.
#[allow(clippy::too_many_arguments)]
fn authenticate_failure_market_family_terminal_for_close_v3(
    program_id: &Pubkey,
    admission_root_account: &AccountInfo<'_>,
    runtime_root_account: &AccountInfo<'_>,
    interval_cell_account: &AccountInfo<'_>,
    interval_history_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    admission: AuthenticatedFailureMarketRootV2,
    interval_funding: FailureMarketIntervalFundingReceiptV2,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    replay_funding: FailureMarketReplayFundingReceiptV2,
) -> Outcome<AuthenticatedFailureMarketFamilyTerminalOwnerV2> {
    authenticate_failure_market_family_terminal_owner_v2(
        program_id,
        admission_root_account,
        runtime_root_account,
        interval_cell_account,
        interval_history_account,
        replay_account,
        admission,
        interval_funding,
        quote,
        replay_funding,
        true,
        true,
        true,
        true,
        false,
    )
}

/// Reopen the durable terminal tuple read-only for Source custody retirement.
/// Source independently hostile-authenticates the exact Retiring LinkV3,
/// route, schedule, and live custody before consuming the move-only receipt.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_failure_market_family_terminal_for_source_retirement_v3(
    program_id: &Pubkey,
    admission_root_account: &AccountInfo<'_>,
    runtime_root_account: &AccountInfo<'_>,
    interval_cell_account: &AccountInfo<'_>,
    interval_history_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    admission: AuthenticatedFailureMarketRootV2,
    interval_funding: FailureMarketIntervalFundingReceiptV2,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    replay_funding: FailureMarketReplayFundingReceiptV2,
) -> Outcome<AuthenticatedFailureMarketFamilyTerminalReceiptV3> {
    authenticate_failure_market_family_terminal_receipt_v3(
        program_id,
        admission_root_account,
        runtime_root_account,
        interval_cell_account,
        interval_history_account,
        replay_account,
        admission,
        interval_funding,
        quote,
        replay_funding,
    )
}

/// Retire the successful Source custody and consume its exact Failure terminal
/// into Product RootV3/LinkV3 in one atomic instruction.
///
/// Failure accounts are hostile-reopened read-only first. Source then consumes
/// the move-only Failure receipt and physically closes custody before Product
/// advances either writable lifecycle account. Any Product reauthentication or
/// postwrite refusal rolls the preceding Source close back with the instruction.
/// No detached Product receipt or caller terminal projection is returned.
#[allow(clippy::too_many_arguments)]
pub(crate) fn retire_successful_failure_source_family_into_product_v3<
    'root,
    'link,
    'post,
>(
    program_id: &Pubkey,
    admission_root_account: &AccountInfo<'_>,
    runtime_root_account: &AccountInfo<'_>,
    interval_cell_account: &AccountInfo<'_>,
    interval_history_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    market_root_account: &AccountInfo<'_>,
    series_link_account: &AccountInfo<'_>,
    source_custody_account: &AccountInfo<'_>,
    principal_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    admission: AuthenticatedFailureMarketRootV2,
    interval_funding: FailureMarketIntervalFundingReceiptV2,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    replay_funding: FailureMarketReplayFundingReceiptV2,
    source_route: AuthenticatedSourceRouteV1,
    source_schedule: SourceWorkScheduleBindingV1,
    funding: &AuthenticatedSeriesFundingAccountV5,
    root_before: AuthenticatedMarketLifecycleRootV3<'root>,
    link_before: AuthenticatedSeriesMarketLinkV3<'link>,
    root_successor: &mut MarketLifecycleRootAccountV3,
    link_successor: &mut SeriesMarketLinkAccountV3,
    root_reopen: &'post mut MarketLifecycleRootAccountV3,
    link_reopen: &'post mut SeriesMarketLinkAccountV3,
) -> Outcome<()> {
    require_distinct(&[
        admission_root_account.clone(),
        runtime_root_account.clone(),
        interval_cell_account.clone(),
        interval_history_account.clone(),
        replay_account.clone(),
        market_root_account.clone(),
        series_link_account.clone(),
        source_custody_account.clone(),
        principal_refund.clone(),
        neutral_sink.clone(),
        system_program.clone(),
    ])?;
    let failure_terminal = authenticate_failure_market_family_terminal_receipt_v3(
        program_id,
        admission_root_account,
        runtime_root_account,
        interval_cell_account,
        interval_history_account,
        replay_account,
        admission,
        interval_funding,
        quote,
        replay_funding,
    )?;
    let custody = authenticate_source_funding_custody_v1(
        program_id,
        source_route,
        source_schedule,
        source_custody_account,
    )?;
    let source_authority = authenticate_source_family_terminal_authority_v3(
        failure_terminal,
        source_route,
        source_schedule,
        &link_before,
        custody,
    )?;
    let source_terminal = retire_source_funding_custody_v3(
        program_id,
        source_route,
        source_schedule,
        source_authority,
        &link_before,
        funding,
        source_custody_account,
        principal_refund,
        neutral_sink,
        system_program,
    )?;
    consume_source_family_terminal_into_product_v3(
        program_id,
        market_root_account,
        series_link_account,
        root_before,
        link_before,
        source_terminal,
        root_successor,
        link_successor,
        root_reopen,
        link_reopen,
    )
}

/// Retire one SourceAbsent/SourceRefused custody and consume the same durable
/// Failure family into Product RootV3/LinkV3.
///
/// The branch is recovered only from the one-way Source V3 terminal account;
/// the payload cannot select a disposition or substitute either physical
/// Source terminal identity.
#[allow(clippy::too_many_arguments)]
pub(crate) fn retire_failed_failure_source_family_into_product_v3<'root, 'link, 'post>(
    program_id: &Pubkey,
    admission_root_account: &AccountInfo<'_>,
    runtime_root_account: &AccountInfo<'_>,
    interval_cell_account: &AccountInfo<'_>,
    interval_history_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    source_terminal_account: &AccountInfo<'_>,
    market_root_account: &AccountInfo<'_>,
    series_link_account: &AccountInfo<'_>,
    source_custody_account: &AccountInfo<'_>,
    principal_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    admission: AuthenticatedFailureMarketRootV2,
    interval_funding: FailureMarketIntervalFundingReceiptV2,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    replay_funding: FailureMarketReplayFundingReceiptV2,
    source_route: AuthenticatedSourceRouteV1,
    source_schedule: SourceWorkScheduleBindingV1,
    funding: &AuthenticatedSeriesFundingAccountV5,
    root_before: AuthenticatedMarketLifecycleRootV3<'root>,
    link_before: AuthenticatedSeriesMarketLinkV3<'link>,
    root_successor: &mut MarketLifecycleRootAccountV3,
    link_successor: &mut SeriesMarketLinkAccountV3,
    root_reopen: &'post mut MarketLifecycleRootAccountV3,
    link_reopen: &'post mut SeriesMarketLinkAccountV3,
) -> Outcome<()> {
    require_distinct(&[
        admission_root_account.clone(),
        runtime_root_account.clone(),
        interval_cell_account.clone(),
        interval_history_account.clone(),
        replay_account.clone(),
        source_terminal_account.clone(),
        market_root_account.clone(),
        series_link_account.clone(),
        source_custody_account.clone(),
        principal_refund.clone(),
        neutral_sink.clone(),
        system_program.clone(),
    ])?;
    let failure_terminal = authenticate_failure_market_family_terminal_receipt_v3(
        program_id,
        admission_root_account,
        runtime_root_account,
        interval_cell_account,
        interval_history_account,
        replay_account,
        admission,
        interval_funding,
        quote,
        replay_funding,
    )?;
    let failure_source_terminal =
        authenticate_failure_market_source_failure_lifecycle_terminal_v3(
            program_id,
            source_route,
            source_terminal_account,
            failure_terminal,
        )?;
    let custody = authenticate_source_funding_custody_v1(
        program_id,
        source_route,
        source_schedule,
        source_custody_account,
    )?;
    let source_authority = authenticate_source_family_terminal_authority_v3(
        failure_source_terminal,
        source_route,
        source_schedule,
        &link_before,
        custody,
    )?;
    let source_terminal = retire_source_funding_custody_v3(
        program_id,
        source_route,
        source_schedule,
        source_authority,
        &link_before,
        funding,
        source_custody_account,
        principal_refund,
        neutral_sink,
        system_program,
    )?;
    consume_source_family_terminal_into_product_v3(
        program_id,
        market_root_account,
        series_link_account,
        root_before,
        link_before,
        source_terminal,
        root_successor,
        link_successor,
        root_reopen,
        link_reopen,
    )
}

/// Move-only proof that every deletable Failure account was closed only after
/// the hostile live Product RootV3 reached whole-Market terminality.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedFailureMarketPhysicalCloseV3 {
    id: ContentId,
    market_terminal_projection_id: ContentId,
    failure_terminal_receipt_id: ContentId,
    interval_close_authorization_id: ContentId,
    admission_root_account: Pubkey,
    runtime_root_account: Pubkey,
    interval_cell_account: Pubkey,
    interval_history_account: Pubkey,
    replay_account: Pubkey,
    rent_refund_owner: Pubkey,
    neutral_sink: Pubkey,
    refunded_principal_lamports: u64,
    neutralized_donation_lamports: u64,
}

impl AuthenticatedFailureMarketPhysicalCloseV3 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn market_terminal_projection_id(&self) -> ContentId {
        self.market_terminal_projection_id
    }
    pub(crate) const fn failure_terminal_receipt_id(&self) -> ContentId {
        self.failure_terminal_receipt_id
    }
    pub(crate) const fn refunded_principal_lamports(&self) -> u64 {
        self.refunded_principal_lamports
    }
    pub(crate) const fn neutralized_donation_lamports(&self) -> u64 {
        self.neutralized_donation_lamports
    }
}

fn require_failure_close_destination(
    account: &AccountInfo<'_>,
    expected: [u8; 32],
) -> Outcome<()> {
    require(
        account.key.to_bytes() == expected
            && account.owner == &SYSTEM_PROGRAM_ID
            && account.data_is_empty()
            && account.is_writable
            && !account.is_signer
            && !account.executable,
        ClutchError::MismatchedState,
    )
}

/// Close the reusable interval pair, mutable runtime root, and immutable
/// admission root in reverse dependency order after Product RootV3 is
/// terminal and has consumed this exact Failure V3 receipt.
///
/// The permanent replay account remains read-only. Exact prepaid rent from all
/// four deleted accounts returns to the single immutable refund owner; every
/// initial or later donation goes only to the shared neutral sink. All
/// postbalances and all close plans are computed before the first write.
#[allow(clippy::too_many_arguments)]
pub(crate) fn close_failure_market_family_after_product_terminal_v3(
    program_id: &Pubkey,
    market_root_account: &AccountInfo<'_>,
    admission_root_account: &AccountInfo<'_>,
    runtime_root_account: &AccountInfo<'_>,
    interval_cell_account: &AccountInfo<'_>,
    interval_history_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    rent_refund_owner: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    admission: AuthenticatedFailureMarketRootV2,
    interval_funding: FailureMarketIntervalFundingReceiptV2,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    replay_funding: FailureMarketReplayFundingReceiptV2,
    market_root_decode: &mut MarketLifecycleRootAccountV3,
) -> Outcome<AuthenticatedFailureMarketPhysicalCloseV3> {
    require_distinct(&[
        market_root_account.clone(),
        admission_root_account.clone(),
        runtime_root_account.clone(),
        interval_cell_account.clone(),
        interval_history_account.clone(),
        replay_account.clone(),
        rent_refund_owner.clone(),
        neutral_sink.clone(),
    ])?;
    require(!market_root_account.is_writable, ClutchError::UnexpectedWritable)?;
    let policy = admission.state().binding().facts();
    let market_root = authenticate_market_lifecycle_root_v3(
        program_id,
        market_root_account,
        policy.market_instance_id,
        policy.generation,
        false,
        market_root_decode,
    )?;
    let terminal_owner = authenticate_failure_market_family_terminal_for_close_v3(
        program_id,
        admission_root_account,
        runtime_root_account,
        interval_cell_account,
        interval_history_account,
        replay_account,
        admission,
        interval_funding,
        quote,
        replay_funding,
    )?;
    let failure_terminal = AuthenticatedFailureMarketFamilyTerminalReceiptV3::from_owner(
        terminal_owner,
    )?;
    let terminal_projection = market_root
        .state()
        .terminal_projection()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        market_root.state().phase() == MarketLifecyclePhaseV3::Terminal
            && market_root.account() == *market_root_account.key
            && market_root.owner_program() == *program_id
            && market_root.binding().market_instance_id == failure_terminal.facts.market_instance_id
            && market_root.binding().generation == failure_terminal.facts.generation
            && market_root.state().failure_terminal_receipt_id() == failure_terminal.id()
            && terminal_projection.root_semantic_id() == market_root.semantic_id()
            && terminal_projection.market_instance_id() == failure_terminal.facts.market_instance_id
            && terminal_projection.generation() == failure_terminal.facts.generation,
        ClutchError::MismatchedState,
    )?;

    let owner = failure_terminal.owner();
    let seal = owner.family_seal()?;
    require(
        seal.facts().family_terminal_receipt_id
            == failure_terminal.facts.owner_terminal_receipt_id
            && owner.replay.account() == *replay_account.key,
        ClutchError::MismatchedState,
    )?;
    let interval_close = plan_close_failure_market_interval_accounts_v2(
        owner.interval.history(),
        seal,
        interval_cell_account.lamports(),
        interval_history_account.lamports(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let admission_close = owner
        .admission
        .state()
        .project_root_balance_disposition(admission_root_account.lamports())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let runtime_funding = owner.runtime.state().root_funding();
    let runtime_principal = runtime_funding.rent_principal_lamports;
    let runtime_donation = runtime_root_account
        .lamports()
        .checked_sub(runtime_principal)
        .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        interval_close.work_account.bytes() == interval_cell_account.key.to_bytes()
            && interval_close.history_account.bytes() == interval_history_account.key.to_bytes()
            && interval_close.rent_refund_owner.bytes() == rent_refund_owner.key.to_bytes()
            && interval_close.neutral_sink.bytes() == neutral_sink.key.to_bytes()
            && admission_close.root_account_id().bytes() == admission_root_account.key.to_bytes()
            && admission_close.rent_refund_owner().bytes() == rent_refund_owner.key.to_bytes()
            && admission_close.neutral_sink().bytes() == neutral_sink.key.to_bytes()
            && runtime_funding.rent_refund_owner.bytes() == rent_refund_owner.key.to_bytes()
            && runtime_funding.neutral_sink.bytes() == neutral_sink.key.to_bytes()
            && owner.runtime.state().runtime_account_id().bytes()
                == runtime_root_account.key.to_bytes()
            && runtime_root_account.lamports() >= runtime_funding.observed_balance_lamports
            && runtime_donation >= runtime_funding.donation_floor_lamports,
        ClutchError::MismatchedState,
    )?;
    require_failure_close_destination(rent_refund_owner, rent_refund_owner.key.to_bytes())?;
    require_failure_close_destination(neutral_sink, neutral_sink.key.to_bytes())?;

    let interval_principal = interval_close
        .work_rent_refund_lamports
        .checked_add(interval_close.history_rent_refund_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let interval_donation = interval_close
        .work_donation_lamports
        .checked_add(interval_close.history_donation_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let refunded_principal_lamports = interval_principal
        .checked_add(runtime_principal)
        .and_then(|value| value.checked_add(admission_close.rent_refund_lamports()))
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let neutralized_donation_lamports = interval_donation
        .checked_add(runtime_donation)
        .and_then(|value| value.checked_add(admission_close.donation_neutral_lamports()))
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let deleted_balance = interval_cell_account
        .lamports()
        .checked_add(interval_history_account.lamports())
        .and_then(|value| value.checked_add(runtime_root_account.lamports()))
        .and_then(|value| value.checked_add(admission_root_account.lamports()))
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        refunded_principal_lamports
            .checked_add(neutralized_donation_lamports)
            == Some(deleted_balance),
        ClutchError::MismatchedState,
    )?;
    let refund_after = rent_refund_owner
        .lamports()
        .checked_add(refunded_principal_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let sink_after = neutral_sink
        .lamports()
        .checked_add(neutralized_donation_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;

    {
        let mut cell_lamports = interval_cell_account
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut history_lamports = interval_history_account
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut runtime_lamports = runtime_root_account
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut admission_lamports = admission_root_account
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut refund_lamports = rent_refund_owner
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut sink_lamports = neutral_sink
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **cell_lamports = 0;
        **history_lamports = 0;
        **runtime_lamports = 0;
        **admission_lamports = 0;
        **refund_lamports = refund_after;
        **sink_lamports = sink_after;
    }
    interval_cell_account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    interval_cell_account.assign(&SYSTEM_PROGRAM_ID);
    interval_history_account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    interval_history_account.assign(&SYSTEM_PROGRAM_ID);
    runtime_root_account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    runtime_root_account.assign(&SYSTEM_PROGRAM_ID);
    admission_root_account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    admission_root_account.assign(&SYSTEM_PROGRAM_ID);

    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            FAMILY_PHYSICAL_CLOSE_DOMAIN_V3,
            &market_root.authentication_id().bytes(),
            &terminal_projection.id().bytes(),
            &failure_terminal.id().bytes(),
            &interval_close.authorization_id.bytes(),
            admission_root_account.key.as_ref(),
            runtime_root_account.key.as_ref(),
            interval_cell_account.key.as_ref(),
            interval_history_account.key.as_ref(),
            replay_account.key.as_ref(),
            rent_refund_owner.key.as_ref(),
            neutral_sink.key.as_ref(),
            &refunded_principal_lamports.to_le_bytes(),
            &neutralized_donation_lamports.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedFailureMarketPhysicalCloseV3 {
        id,
        market_terminal_projection_id: terminal_projection.id(),
        failure_terminal_receipt_id: failure_terminal.id(),
        interval_close_authorization_id: ContentId::from_bytes(
            interval_close.authorization_id.bytes(),
        ),
        admission_root_account: *admission_root_account.key,
        runtime_root_account: *runtime_root_account.key,
        interval_cell_account: *interval_cell_account.key,
        interval_history_account: *interval_history_account.key,
        replay_account: *replay_account.key,
        rent_refund_owner: *rent_refund_owner.key,
        neutral_sink: *neutral_sink.key,
        refunded_principal_lamports,
        neutralized_donation_lamports,
    })
}

/// Produce one durable Failure terminal chain without a caller terminal DTO.
#[allow(clippy::too_many_arguments)]
fn write_failure_market_family_terminal_v2<'a>(
    program_id: &Pubkey,
    admission_root_account: &AccountInfo<'a>,
    runtime_root_account: &AccountInfo<'a>,
    interval_cell_account: &AccountInfo<'a>,
    interval_history_account: &AccountInfo<'a>,
    replay_account: &AccountInfo<'a>,
    admission: AuthenticatedFailureMarketRootV2,
    recovery_close: AuthenticatedFailureMarketRecoveryClosePostwriteV2,
    replay_before: AuthenticatedFailureMarketReplayV2,
) -> Outcome<AuthenticatedFailureMarketFamilyTerminalPostwriteV2> {
    require_distinct(&[
        admission_root_account.clone(),
        runtime_root_account.clone(),
        interval_cell_account.clone(),
        interval_history_account.clone(),
        replay_account.clone(),
    ])?;
    let live_admission =
        authenticate_failure_market_root_v2(program_id, admission_root_account, false)?;
    require(live_admission == admission, ClutchError::MismatchedState)?;
    let live_runtime = authenticate_failure_market_runtime_root_v1(
        program_id,
        admission_root_account,
        runtime_root_account,
        live_admission,
        true,
    )?;
    require(
        live_runtime == recovery_close.runtime(),
        ClutchError::MismatchedState,
    )?;
    let retained_interval = recovery_close.interval();
    let live_interval = authenticate_failure_market_interval_accounts_v2(
        program_id,
        interval_cell_account,
        interval_history_account,
        live_admission,
        retained_interval.funding(),
        retained_interval.quote(),
        false,
        true,
    )?;
    require(
        live_interval == retained_interval,
        ClutchError::MismatchedState,
    )?;
    let live_replay = authenticate_failure_market_replay_v2(
        program_id,
        replay_account,
        live_admission,
        replay_before.funding(),
        true,
    )?;
    require(live_replay == replay_before, ClutchError::MismatchedState)?;

    let runtime = live_runtime.state();
    let history = live_interval.history();
    let policy = live_admission.state().binding().facts();
    let recovery_facts = recovery_close.close().facts();
    let expected_aggregate = FailureMarketFamilyAggregateFactsV2 {
        disposition: FailureMarketFamilyTerminalDispositionV2::Resolved,
        runtime_before: live_runtime.state_commitment(),
        admission_state_id: live_admission
            .state()
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        failure_policy_binding_id: live_admission.state().binding().id(),
        market_instance_id: policy.market_instance_id,
        generation: policy.generation,
        admission_root_account_id: live_admission
            .state()
            .root_funding()
            .facts()
            .root_account_id,
        runtime_root_account_id: runtime.runtime_account_id(),
        interval_work_account_id: history.work_account(),
        interval_history_account_id: history.history_account(),
        interval_cell_state_id: live_interval.cell_state_id(),
        interval_history_state_id: live_interval.history_state_id(),
        interval_history_root: history.history_root(),
        completed_session_count: history.completed_session_count(),
        completed_work_calls: history.completed_work_calls(),
        exact_reward_lamports: history.exact_reward_lamports(),
        recovery_close_receipt_id: recovery_close.close().id(),
        resolution_activation_receipt_id: recovery_facts.resolution_activation_receipt_id,
        source_resolution_terminal_receipt_id:
            recovery_facts.source_resolution_terminal_receipt_id,
        source_result_close_receipt_id: recovery_facts.source_result_close_receipt_id,
        source_product_release_binding_id: recovery_facts.source_product_release_binding_id,
        source_product_link_account_id: recovery_facts.source_product_link_account_id,
    };
    let aggregate = admit_failure_market_family_aggregate_v2(
        &FailureMarketFamilyAggregateAuthorityV2 {
            expected: expected_aggregate,
        },
        runtime,
        live_admission.state(),
        live_interval.funding(),
        live_interval.quote(),
        live_interval.cell(),
        history,
        recovery_close.close(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let replay_authority = FailureMarketReplayTerminalAuthorityV2 {
        replay_before: live_replay.state_id(),
        replay_account: live_replay.funding().facts().replay_account,
        funding_receipt_id: live_replay.funding().id(),
        family_aggregate_receipt_id: aggregate.id(),
        runtime_terminal_state_commitment: live_runtime.state_commitment(),
    };
    let (replay_plan, replay_terminal) = plan_terminalize_failure_market_replay_v2(
        &replay_authority,
        live_replay.replay(),
        live_admission.state(),
        live_replay.funding(),
        aggregate,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let expected_family_terminal = FailureMarketFamilyTerminalFactsV2 {
        disposition: FailureMarketFamilyTerminalDispositionV2::Resolved,
        family_aggregate_receipt_id: aggregate.id(),
        failure_replay_account_id: live_replay.funding().facts().replay_account,
        failure_replay_terminal_receipt_id: replay_terminal.id(),
        runtime_before: live_runtime.state_commitment(),
        admission_state_id: expected_aggregate.admission_state_id,
        failure_policy_binding_id: expected_aggregate.failure_policy_binding_id,
        market_instance_id: expected_aggregate.market_instance_id,
        generation: expected_aggregate.generation,
        interval_history_state_id: live_interval.history_state_id(),
        interval_history_root: history.history_root(),
        completed_session_count: history.completed_session_count(),
        source_resolution_terminal_receipt_id:
            expected_aggregate.source_resolution_terminal_receipt_id,
        source_result_close_receipt_id: expected_aggregate.source_result_close_receipt_id,
        source_product_release_binding_id: expected_aggregate.source_product_release_binding_id,
        source_product_link_account_id: expected_aggregate.source_product_link_account_id,
    };
    let (runtime_plan, family_terminal) = plan_finalize_failure_market_family_v2(
        &FailureMarketFamilyTerminalAuthorityV2 {
            expected: expected_family_terminal,
        },
        runtime,
        live_admission.state(),
        live_interval.funding(),
        live_interval.quote(),
        live_interval.cell(),
        history,
        aggregate,
        replay_terminal,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let expected_seal = FailureMarketIntervalFamilySealFactsV2 {
        history_before: live_interval.history_state_id(),
        history_root: history.history_root(),
        completed_session_count: history.completed_session_count(),
        completed_work_calls: history.completed_work_calls(),
        exact_reward_lamports: history.exact_reward_lamports(),
        family_terminal_receipt_id: family_terminal.id(),
    };
    let (history_plan, family_seal) = plan_seal_failure_market_interval_history_v2(
        &FailureMarketHistorySealAuthorityV2 {
            expected: expected_seal,
        },
        history,
        live_admission.state(),
        live_interval.quote(),
        family_terminal.id(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    // Every semantic postimage is fixed before the first write. Any later
    // refusal rolls replay, runtime, and history back together.
    let replay_after = write_failure_market_replay_terminal_v2(
        program_id,
        replay_account,
        live_admission,
        live_replay,
        replay_plan,
        replay_terminal,
    )?;
    let runtime_after = write_failure_market_runtime_terminal_plan_v2(
        program_id,
        admission_root_account,
        runtime_root_account,
        live_admission,
        live_runtime,
        runtime_plan,
    )?;
    let interval_after = write_failure_market_interval_family_seal_v2(
        program_id,
        interval_cell_account,
        interval_history_account,
        live_interval,
        history_plan,
        family_seal,
    )?;
    require(
        replay_after.replay().family_aggregate_receipt_id() == aggregate.id()
            && runtime_after.state().family_terminal_receipt_id().bytes()
                == family_terminal.id().bytes()
            && interval_after.history().family_terminal_receipt_id() == family_terminal.id()
            && interval_after.cell_state_id() == live_interval.cell_state_id(),
        ClutchError::MismatchedState,
    )?;
    let mut postwrite = AuthenticatedFailureMarketFamilyTerminalPostwriteV2 {
        id: ContentId::ZERO,
        owner_release_id: ContentId::ZERO,
        admission: live_admission,
        aggregate,
        replay_terminal,
        family_terminal,
        family_seal,
        replay: replay_after,
        runtime: runtime_after,
        interval: interval_after,
    };
    postwrite.owner_release_id = postwrite.derived_owner_release_id()?;
    postwrite.id = postwrite.derived_postwrite_id();
    let owner_account_id = postwrite.owner_account_id();
    let owner_terminal_receipt_id = ContentId::from_bytes(postwrite.family_terminal.id().bytes());
    require(
        !postwrite.owner_release_id.is_zero()
            && postwrite.owner_release_id != owner_account_id
            && postwrite.owner_release_id != owner_terminal_receipt_id
            && owner_account_id != owner_terminal_receipt_id,
        ClutchError::MismatchedState,
    )?;
    require(!postwrite.id.is_zero(), ClutchError::MismatchedState)?;
    Ok(postwrite)
}

/// Persist the complete Failure terminal tuple during the successful
/// resolution transaction while Product is still `Active`.
///
/// The Product resolution activation in the live root must be the exact one
/// retained by the Recovery-close receipt. This wrapper cannot latch Product:
/// convergent Series links retire later. Its durable poststate is reopened by
/// the current RootV2 Product terminal owner after Product enters `Retiring`;
/// no historical RootV1 writer is accepted here.
#[allow(clippy::too_many_arguments)]
pub(crate) fn persist_resolved_failure_market_family_v2(
    program_id: &Pubkey,
    market_root_account: &AccountInfo<'_>,
    admission_root_account: &AccountInfo<'_>,
    runtime_root_account: &AccountInfo<'_>,
    interval_cell_account: &AccountInfo<'_>,
    interval_history_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    resolved_market_root: AuthenticatedMarketLifecycleRootV2<'_>,
    admission: AuthenticatedFailureMarketRootV2,
    recovery_close: AuthenticatedFailureMarketRecoveryClosePostwriteV2,
    replay_before: AuthenticatedFailureMarketReplayV2,
    resolved_root_decode: &mut MarketLifecycleRootAccountV2,
) -> Outcome<AuthenticatedFailureMarketFamilyTerminalPostwriteV2> {
    require_distinct(&[
        market_root_account.clone(),
        admission_root_account.clone(),
        runtime_root_account.clone(),
        interval_cell_account.clone(),
        interval_history_account.clone(),
        replay_account.clone(),
    ])?;
    let policy = admission.state().binding().facts();
    let live_root = authenticate_market_lifecycle_root_v2(
        program_id,
        market_root_account,
        policy.market_instance_id,
        policy.generation,
        true,
        resolved_root_decode,
    )?;
    require(live_root == resolved_market_root, ClutchError::MismatchedState)?;
    let root = live_root.state();
    let close = recovery_close.close().facts();
    require(
        live_root.account() == *market_root_account.key
            && live_root.owner_program() == *program_id
            && root.phase() == MarketLifecyclePhaseV2::Active
            && root.binding().market_instance_id
                == admission.state().binding().facts().market_instance_id
            && root.binding().generation == admission.state().binding().facts().generation
            && root.resolution_activation_receipt_id()
                == close.resolution_activation_receipt_id
            && root.failure_terminal_receipt_id() == ContentId::ZERO,
        ClutchError::MismatchedState,
    )?;
    write_failure_market_family_terminal_v2(
        program_id,
        admission_root_account,
        runtime_root_account,
        interval_cell_account,
        interval_history_account,
        replay_account,
        admission,
        recovery_close,
        replay_before,
    )
}

/// Persist the complete Failure terminal tuple against the current Product
/// RootV3 resolution poststate.
///
/// This is the Link-independent half of the current lifecycle. It consumes no
/// Product link and emits no Product receipt: the later Retiring RootV3
/// shared-core writer must hostile-reopen the durable Failure tuple through
/// [`authenticate_failure_market_family_terminal_receipt_v3`].
#[allow(clippy::too_many_arguments)]
pub(crate) fn persist_resolved_failure_market_family_v3(
    program_id: &Pubkey,
    market_root_account: &AccountInfo<'_>,
    admission_root_account: &AccountInfo<'_>,
    runtime_root_account: &AccountInfo<'_>,
    interval_cell_account: &AccountInfo<'_>,
    interval_history_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    resolved_market_root: &AuthenticatedMarketLifecycleRootV3<'_>,
    admission: AuthenticatedFailureMarketRootV2,
    recovery_close: AuthenticatedFailureMarketRecoveryClosePostwriteV2,
    replay_before: AuthenticatedFailureMarketReplayV2,
    resolved_root_decode: &mut MarketLifecycleRootAccountV3,
) -> Outcome<AuthenticatedFailureMarketFamilyTerminalPostwriteV2> {
    require_distinct(&[
        market_root_account.clone(),
        admission_root_account.clone(),
        runtime_root_account.clone(),
        interval_cell_account.clone(),
        interval_history_account.clone(),
        replay_account.clone(),
    ])?;
    let policy_binding = admission.state().binding();
    let policy = policy_binding.facts();
    let policy_binding_id = policy_binding.id();
    let live_root = authenticate_market_lifecycle_root_v3(
        program_id,
        market_root_account,
        policy.market_instance_id,
        policy.generation,
        true,
        resolved_root_decode,
    )?;
    require(
        live_root.account() == resolved_market_root.account()
            && live_root.owner_program() == resolved_market_root.owner_program()
            && live_root.observed_lamports() == resolved_market_root.observed_lamports()
            && live_root.is_writable()
            && resolved_market_root.is_writable()
            && live_root.data_id() == resolved_market_root.data_id()
            && live_root.semantic_id() == resolved_market_root.semantic_id()
            && live_root.binding_id() == resolved_market_root.binding_id()
            && live_root.authentication_id() == resolved_market_root.authentication_id()
            && live_root.state() == resolved_market_root.state(),
        ClutchError::MismatchedState,
    )?;
    let root = live_root.state();
    let binding = live_root.binding();
    let close = recovery_close.close().facts();
    require(
        live_root.account() == *market_root_account.key
            && live_root.owner_program() == *program_id
            && root.phase() == MarketLifecyclePhaseV3::Active
            && binding.market_instance_id == policy.market_instance_id
            && binding.generation == policy.generation
            && binding.market_failure_policy_binding_id.bytes() == policy_binding_id.bytes()
            && binding.recovery_state_id.bytes() == policy.recovery_state_id.bytes()
            && binding.failure_liveness_policy_id.bytes() == policy.liveness_policy_id.bytes()
            && binding.failure_liveness_quote_schedule_id.bytes()
                == policy.recovery_quote_schedule_id.bytes()
            && root.resolution_semantic_id() != ContentId::ZERO
            && root.resolution_data_id() != ContentId::ZERO
            && root.resolution_semantic_id() != root.resolution_data_id()
            && root.resolution_activation_receipt_id()
                == close.resolution_activation_receipt_id
            && root.failure_terminal_receipt_id() == ContentId::ZERO,
        ClutchError::MismatchedState,
    )?;
    write_failure_market_family_terminal_v2(
        program_id,
        admission_root_account,
        runtime_root_account,
        interval_cell_account,
        interval_history_account,
        replay_account,
        admission,
        recovery_close,
        replay_before,
    )
}

#[cfg(test)]
mod adversarial_family_terminal_tests {
    #[test]
    fn current_terminal_persistence_uses_only_live_root_v3_authority() {
        let source = include_str!("failure_market_family_terminal_v2.rs");
        let current = source
            .split("pub(crate) fn persist_resolved_failure_market_family_v3")
            .nth(1)
            .and_then(|value| value.split("#[cfg(test)]").next())
            .expect("current RootV3 terminal persistence");
        for predicate in [
            "resolved_market_root: &AuthenticatedMarketLifecycleRootV3",
            "resolved_root_decode: &mut MarketLifecycleRootAccountV3",
            "authenticate_market_lifecycle_root_v3(",
            "live_root.authentication_id() == resolved_market_root.authentication_id()",
            "live_root.state() == resolved_market_root.state()",
            "MarketLifecyclePhaseV3::Active",
            "binding.market_failure_policy_binding_id.bytes() == policy_binding_id.bytes()",
            "binding.recovery_state_id.bytes() == policy.recovery_state_id.bytes()",
            "binding.failure_liveness_policy_id.bytes() == policy.liveness_policy_id.bytes()",
            "binding.failure_liveness_quote_schedule_id.bytes()",
            "root.resolution_activation_receipt_id()",
            "root.failure_terminal_receipt_id() == ContentId::ZERO",
            "write_failure_market_family_terminal_v2(",
        ] {
            assert!(current.contains(predicate), "missing RootV3 guard {predicate}");
        }
        assert!(!current.contains("AuthenticatedMarketLifecycleRootV2"));
        assert!(!current.contains("MarketLifecycleRootAccountV2"));
        assert!(!current.contains("AuthenticatedSeriesMarketLinkV2"));
        assert!(!current.contains("AuthenticatedSeriesMarketLinkV3"));
        assert!(!current.contains("record_failure_shared_core_terminal"));
    }

    #[test]
    fn product_v3_terminal_receipt_is_move_only_and_failure_owned() {
        let source = include_str!("failure_market_family_terminal_v2.rs");
        let receipt = source
            .split("pub(crate) struct AuthenticatedFailureMarketFamilyTerminalReceiptV3")
            .nth(1)
            .and_then(|value| {
                value
                    .split("pub(crate) trait AuthenticatedFailureMarketFamilyTerminalAuthorityV3")
                    .next()
            })
            .expect("move-only Failure terminal receipt");
        assert!(receipt.contains("owner: AuthenticatedFailureMarketFamilyTerminalOwnerV2"));
        assert!(receipt.contains("family_seal_id: ContentId"));
        assert!(!receipt.contains("derive(Clone"));
        assert!(!receipt.contains("derive(Copy"));
        assert!(!receipt.contains("AuthenticatedMarketLifecycleRootV2"));
        assert!(!receipt.contains("AuthenticatedSeriesMarketLinkV2"));
        assert!(!receipt.contains("AuthenticatedMarketLifecycleRootV3"));
        assert!(!receipt.contains("AuthenticatedSeriesMarketLinkV3"));
        assert!(!receipt.contains("ProductPostwrite"));
    }

    #[test]
    fn product_v3_receipt_reopens_every_failure_poststate_read_only() {
        let source = include_str!("failure_market_family_terminal_v2.rs");
        let reopen = source
            .split("pub(crate) fn authenticate_failure_market_family_terminal_receipt_v3")
            .nth(1)
            .and_then(|value| {
                value
                    .split("/// Reopen the exact durable Failure terminal")
                    .next()
            })
            .expect("RootV3 consumer receipt authenticator");
        for role in [
            "admission_root_account",
            "runtime_root_account",
            "interval_cell_account",
            "interval_history_account",
            "replay_account",
        ] {
            assert!(reopen.contains(role), "missing terminal role {role}");
        }
        assert!(reopen.contains("false,\n        false,\n        false,\n        false,"));
        assert!(reopen.contains(
            "AuthenticatedFailureMarketFamilyTerminalReceiptV3::from_owner(owner)"
        ));
        assert!(!reopen.contains("market_root_account"));
        assert!(!reopen.contains("link_account"));
        assert!(!reopen.contains("terminal_projection"));
    }

    #[test]
    fn product_v3_receipt_commits_terminal_and_reopen_evidence() {
        let source = include_str!("failure_market_family_terminal_v2.rs");
        let constructor = source
            .split("fn from_owner(owner: AuthenticatedFailureMarketFamilyTerminalOwnerV2)")
            .nth(1)
            .and_then(|value| {
                value
                    .split("/// Default-refusing boundary for the future narrow Product RootV3 writer")
                    .next()
            })
            .expect("V3 receipt constructor");
        for committed in [
            "owner.id().bytes()",
            "facts.market_instance_id.bytes()",
            "facts.generation.to_le_bytes()",
            "facts.owner_account_id.bytes()",
            "facts.owner_release_id.bytes()",
            "facts.owner_terminal_receipt_id.bytes()",
            "facts.admission_root_account.to_bytes()",
            "facts.admission_state_id.bytes()",
            "facts.failure_policy_binding_id.bytes()",
            "facts.runtime_root_account.to_bytes()",
            "facts.runtime_state_commitment.bytes()",
            "facts.recovery_terminal_receipt_id.bytes()",
            "facts.replay_authentication_id.bytes()",
            "facts.interval_cell_authentication_id.bytes()",
            "facts.interval_history_authentication_id.bytes()",
            "facts.interval_history_root.bytes()",
            "facts.source_resolution_terminal_postwrite_id.bytes()",
            "facts.source_result_close_receipt_id.bytes()",
            "facts.source_product_release_binding_id.bytes()",
            "facts.source_product_link_account_id.bytes()",
            "family_seal_id.bytes()",
        ] {
            assert!(constructor.contains(committed), "missing V3 fact {committed}");
        }
        assert!(constructor.contains("owner.family_seal()?"));
    }

    #[test]
    fn transaction_postwrite_is_not_a_detached_product_capability() {
        let source = include_str!("failure_market_family_terminal_v2.rs");
        let postwrite = source
            .split("pub(crate) struct AuthenticatedFailureMarketFamilyTerminalPostwriteV2")
            .nth(1)
            .and_then(|value| {
                value
                    .split("pub(crate) struct AuthenticatedFailureMarketFamilyTerminalOwnerV2")
                    .next()
            })
            .expect("same-instruction terminal postwrite");
        assert!(!postwrite.contains("derive(Clone"));
        assert!(!postwrite.contains("derive(Copy"));
        assert!(postwrite.contains("pub(crate) const fn id(&self)"));
        assert!(postwrite.contains("pub(crate) const fn family_terminal(&self)"));
    }

    #[test]
    fn terminal_chain_plans_every_poststate_before_replay_write() {
        let source = include_str!("failure_market_family_terminal_v2.rs");
        let outer = source
            .split("fn write_failure_market_family_terminal_v2")
            .nth(1)
            .and_then(|value| value.split("/// Persist the complete Failure terminal").next())
            .expect("private family terminal writer");
        let aggregate = outer
            .find("admit_failure_market_family_aggregate_v2")
            .expect("aggregate");
        let replay_plan = outer
            .find("plan_terminalize_failure_market_replay_v2")
            .expect("replay plan");
        let runtime_plan = outer
            .find("plan_finalize_failure_market_family_v2")
            .expect("runtime plan");
        let seal_plan = outer
            .find("plan_seal_failure_market_interval_history_v2")
            .expect("history seal plan");
        let first_write = outer
            .find("write_failure_market_replay_terminal_v2")
            .expect("first write");
        assert!(aggregate < replay_plan && replay_plan < runtime_plan);
        assert!(runtime_plan < seal_plan && seal_plan < first_write);
    }

    #[test]
    fn terminal_chain_rejects_account_aliases_and_legacy_replay() {
        let source = include_str!("failure_market_family_terminal_v2.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("production source");
        assert!(!production.contains("failure_replay_tombstone_pda"));
        assert!(!production.contains("FailureReplayV1"));
        let outer = production
            .split("fn write_failure_market_family_terminal_v2")
            .nth(1)
            .expect("private family terminal writer");
        let aliases = outer
            .split("require_distinct(&[")
            .nth(1)
            .and_then(|value| value.split("])?;").next())
            .expect("explicit aliases");
        for role in [
            "admission_root_account",
            "runtime_root_account",
            "interval_cell_account",
            "interval_history_account",
            "replay_account",
        ] {
            assert!(aliases.contains(role), "missing role {role}");
        }
    }

    #[test]
    fn terminal_chain_reauthenticates_every_cached_mutable_prestate() {
        let source = include_str!("failure_market_family_terminal_v2.rs");
        let outer = source
            .split("fn write_failure_market_family_terminal_v2")
            .nth(1)
            .and_then(|value| value.split("/// Persist the complete Failure terminal").next())
            .expect("private family terminal writer");
        let first_write = outer
            .find("write_failure_market_replay_terminal_v2")
            .expect("first write");
        let prewrite = &outer[..first_write];
        for guard in [
            "authenticate_failure_market_root_v2",
            "live_admission == admission",
            "authenticate_failure_market_runtime_root_v1",
            "live_runtime == recovery_close.runtime()",
            "authenticate_failure_market_interval_accounts_v2",
            "live_interval == retained_interval",
            "authenticate_failure_market_replay_v2",
            "live_replay == replay_before",
        ] {
            assert!(prewrite.contains(guard), "missing stale-prestate guard {guard}");
        }
    }

    #[test]
    fn owner_release_commits_the_complete_terminal_owner_tuple() {
        let source = include_str!("failure_market_family_terminal_v2.rs");
        let release = source
            .split("fn derive_terminal_owner_release_id_v2")
            .nth(1)
            .and_then(|value| {
                value.split("/// Reopen the complete persisted Failure terminal tuple")
                    .next()
            })
            .expect("owner release derivation");
        for committed in [
            "policy.market_instance_id",
            "policy.generation",
            "admission.account()",
            "admission_state_id",
            "admission.state().binding().id()",
            "runtime.account()",
            "runtime.state_commitment()",
            "runtime.state().recovery_terminal_receipt_id()",
            "runtime.state().family_terminal_receipt_id()",
            "runtime.state().source_resolution_terminal_postwrite_id()",
            "runtime.state().source_result_close_receipt_id()",
            "replay.account()",
            "replay.state_id()",
            "replay.authentication_id()",
            "replay.replay().family_aggregate_receipt_id()",
            "interval.cell_account()",
            "interval.cell_state_id()",
            "interval.cell_authentication_id()",
            "interval.history_account()",
            "interval.history_state_id()",
            "interval.history_authentication_id()",
            "interval.history().history_root()",
            "interval.history().family_terminal_receipt_id()",
        ] {
            assert!(release.contains(committed), "missing owner fact {committed}");
        }
        assert!(!release.contains("recovery_close"));
        assert!(!release.contains("family_seal"));
        assert!(!release.contains("replay_terminal"));
        assert!(!release.contains("FailureReplayV1"));
        assert!(!release.contains("ExternalV2"));
    }

    #[test]
    fn delayed_product_latch_reauthenticates_the_complete_persisted_tuple() {
        let source = include_str!("failure_market_family_terminal_v2.rs");
        let auth = source
            .split("fn authenticate_failure_market_family_terminal_owner_v2")
            .nth(1)
            .and_then(|value| {
                value.split("/// Produce one durable Failure terminal chain").next()
            })
            .expect("persisted current terminal authority");
        for predicate in [
            "authenticate_failure_market_root_v2",
            "authenticate_failure_market_runtime_root_v1",
            "authenticate_failure_market_interval_accounts_v2",
            "authenticate_failure_market_replay_v2",
            "FailureMarketRuntimePhaseV1::FamilyTerminal",
            "FailureMarketIntervalCellPhaseV2::Idle",
            "history.family_terminal_receipt_id().bytes()",
            "FailureMarketReplayPhaseV2::Terminal",
            "replay_state.family_aggregate_receipt_id().bytes() != [0; 32]",
            "authenticated.derived_owner_release_id()?",
            "authenticated.derived_authentication_id()",
        ] {
            assert!(auth.contains(predicate), "missing durable guard {predicate}");
        }
    }

    #[test]
    fn durable_terminal_owner_reconstructs_the_only_interval_seal() {
        let source = include_str!("failure_market_family_terminal_v2.rs");
        let owner = source
            .split("impl AuthenticatedFailureMarketFamilyTerminalOwnerV2")
            .nth(1)
            .and_then(|value| {
                value.split("fn derive_terminal_owner_release_id_v2")
                    .next()
            })
            .expect("durable owner");
        for predicate in [
            "reconstruct_failure_market_interval_family_seal_v2",
            "self.interval.history()",
            "self.admission.state()",
            "self.interval.quote()",
            "expected.history_root != history.history_root()",
            "expected.family_terminal_receipt_id.bytes()",
            "history.family_terminal_receipt_id()",
        ] {
            assert!(owner.contains(predicate));
        }
        assert!(!owner.contains("FailureMarketIntervalFamilySealReceiptV2 {"));
    }

    #[test]
    fn source_custody_terminal_is_derived_from_live_founder_and_durable_runtime() {
        let source = include_str!("failure_market_family_terminal_v2.rs");
        let owner = source
            .split("fn successful_source_custody_terminal_facts_v1")
            .nth(1)
            .and_then(|value| value.split("fn derive_terminal_owner_release_id_v2").next())
            .expect("successful Source custody terminal owner");
        for predicate in [
            "founder.market_instance_id.bytes() == policy.market_instance_id.bytes()",
            "founder.source_release_manifest_id.bytes()",
            "founder.source_release_authentication_id.bytes()",
            "founder.source_generation == policy.generation",
            "founder.neutral_lamport_sink.bytes() == policy.neutral_sink.bytes()",
            "founder.product_link_account.bytes() == source_product_link_account.bytes()",
            "product_link_account_data_id: founder.product_link_account_data_id",
            "product_link_authentication_id: founder.product_link_authentication_id",
            "product_link_semantic_id: founder.product_link_semantic_id",
            "product_link_transition_sequence: founder.product_link_transition_sequence",
            "source_terminal_postwrite_id: source_terminal",
            "source_result_or_absence_close_receipt_id: source_result_close",
            "source_product_release_binding_id: source_product_release",
            "!source_product_link_account.is_zero()",
            "source_product_link_account != source_product_release",
            "failure_family_terminal_receipt_id: family_terminal_receipt_id",
            "pre_root_source_occurrence_id: founder.pre_root_source_occurrence_id",
            "source_occurrence_authentication_id: founder.source_occurrence_authentication_id",
        ] {
            assert!(owner.contains(predicate), "missing Source terminal fact {predicate}");
        }
        assert!(!owner.contains("SourceFundingCustodyLifecycleTerminalFactsV1::decode"));
    }

    #[test]
    fn failed_source_custody_terminal_uses_only_hostile_v3_and_live_link_founder() {
        let source = include_str!("failure_market_family_terminal_v2.rs");
        let owner = source
            .split("pub(crate) fn authenticate_failure_market_source_failure_lifecycle_terminal_v3")
            .nth(1)
            .and_then(|value| {
                value
                    .split("fn successful_source_custody_terminal_facts_v1")
                    .next()
            })
            .expect("failed Source lifecycle terminal owner");
        for predicate in [
            "authenticate_persisted_source_failure_product_release_v3(",
            "terminal.market_instance_id().bytes() == policy.market_instance_id.bytes()",
            "terminal.failure_policy_binding_id().bytes() == policy_binding_id.bytes()",
            "founder.product_link_account == self.source.product_link_account()",
            "founder.source_repair_generation == terminal.source_repair_generation()",
            "source_terminal_postwrite_id: self.source.source_terminal_postwrite_id()",
            ".source_physical_disposition_id()",
            "source_product_release_binding_id: self.source.product_release_binding_id()",
            "failure_family_terminal_receipt_id: SourceContentId::from_bytes(",
            "SourceFundingCustodyLifecycleTerminalEvidenceV1::failed(",
        ] {
            assert!(owner.contains(predicate), "missing hostile terminal join {predicate}");
        }
        assert!(!owner.contains("product_link_authentication_after() =="));
        assert!(!owner.contains("SourceFailureTerminalAccountV2"));
        assert!(!owner.contains("expected_terminal_id"));
        assert!(!owner.contains("expected_physical_id"));
    }

    #[test]
    fn terminal_owner_release_commits_the_exact_resolved_product_link_account() {
        let source = include_str!("failure_market_family_terminal_v2.rs");
        let derivation = source
            .split("fn derive_terminal_owner_release_id_v2")
            .nth(1)
            .expect("owner release derivation");
        assert!(derivation.contains("runtime.state().source_product_link_account_id().bytes()"));
        assert!(source.contains("source_product_link_account_id: recovery_facts.source_product_link_account_id"));
        assert!(source.contains("source_product_link_account_id: expected_aggregate.source_product_link_account_id"));
    }

    #[test]
    fn close_reopen_widens_only_deletable_failure_accounts() {
        let source = include_str!("failure_market_family_terminal_v2.rs");
        let close = source
            .split("fn authenticate_failure_market_family_terminal_for_close_v3")
            .nth(1)
            .and_then(|value| {
                value.split("/// Reopen the durable terminal tuple read-only").next()
            })
            .expect("close-scoped durable owner");
        assert!(close.contains("true,\n        true,\n        true,\n        true,\n        false,"));
        assert!(!close.contains("pub(crate) fn"));
    }

    #[test]
    fn active_resolution_persists_before_the_later_retiring_product_latch() {
        let source = include_str!("failure_market_family_terminal_v2.rs");
        let persist = source
            .split("pub(crate) fn persist_resolved_failure_market_family_v2")
            .nth(1)
            .and_then(|value| {
                value.split("/// After all Series links retire").next()
            })
            .expect("Active-root persistence outer");
        assert!(persist.contains("MarketLifecyclePhaseV2::Active"));
        assert!(persist.contains("authenticate_market_lifecycle_root_v2"));
        assert!(persist.contains("live_root == resolved_market_root"));
        assert!(persist.contains("root.resolution_activation_receipt_id()"));
        assert!(persist.contains("close.resolution_activation_receipt_id"));
        assert!(persist.contains("write_failure_market_family_terminal_v2"));
        assert!(!persist.contains("record_failure_shared_core_terminal_v1"));
        assert!(
            persist.find("authenticate_market_lifecycle_root_v2")
                < persist.find("write_failure_market_family_terminal_v2")
        );
        assert!(!persist.contains("AuthenticatedMarketLifecycleRootV1"));

        let production = source.split("#[cfg(test)]").next().expect("production");
        assert!(!production.contains(concat!(
            "record_persisted_failure_market_family_terminal_",
            "v2"
        )));
        assert!(!production.contains(concat!(
            "record_failure_shared_core_terminal_",
            "v1"
        )));
        assert!(!production.contains(concat!(
            "AuthenticatedMarketLifecycleRoot",
            "V1"
        )));
    }

    #[test]
    fn raw_family_writer_is_private_and_product_never_accepts_ephemeral_close() {
        let source = include_str!("failure_market_family_terminal_v2.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("production source");
        assert!(production.contains("fn write_failure_market_family_terminal_v2"));
        assert!(!production.contains(
            "pub(crate) fn write_failure_market_family_terminal_v2"
        ));
        assert!(!production.contains("AuthenticatedFailureSharedCoreTerminalOwnerV1"));
        assert!(!production.contains("record_failure_shared_core_terminal_v1"));
    }

    #[test]
    fn source_and_product_v3_terminal_outer_is_linear_and_source_first() {
        let source = include_str!("failure_market_family_terminal_v2.rs");
        let receipt = source
            .split("pub(crate) struct AuthenticatedFailureMarketFamilyTerminalReceiptV3")
            .nth(1)
            .and_then(|value| value.split("pub(crate) trait AuthenticatedFailureMarketFamilyTerminalAuthorityV3").next())
            .expect("move-only Failure terminal receipt");
        assert!(!receipt.contains("derive(Clone"));
        assert!(!receipt.contains("derive(Copy"));

        let outer = source
            .split("pub(crate) fn retire_successful_failure_source_family_into_product_v3")
            .nth(1)
            .and_then(|value| value.split("/// Retire one SourceAbsent").next())
            .expect("successful Source/Product terminal outer");
        let reopen = outer
            .find("authenticate_failure_market_family_terminal_receipt_v3")
            .expect("Failure terminal reopen");
        let authority = outer
            .find("authenticate_source_family_terminal_authority_v3")
            .expect("Source terminal authority");
        let source_close = outer
            .find("retire_source_funding_custody_v3")
            .expect("Source custody close");
        let product_write = outer
            .find("consume_source_family_terminal_into_product_v3")
            .expect("Product RootV3/LinkV3 write");
        assert!(reopen < authority && authority < source_close && source_close < product_write);
        assert!(!outer.contains("AuthenticatedMarketLifecycleRootV2"));
        assert!(!outer.contains("AuthenticatedSeriesMarketLinkV2"));
        assert!(!outer.contains("ProductPostwrite"));
    }

    #[test]
    fn failed_source_v3_terminal_cannot_select_branch_or_detach_product_write() {
        let source = include_str!("failure_market_family_terminal_v2.rs");
        let outer = source
            .split("pub(crate) fn retire_failed_failure_source_family_into_product_v3")
            .nth(1)
            .and_then(|value| value.split("/// Produce one durable Failure terminal chain").next())
            .expect("failed Source/Product terminal outer");
        for required in [
            "authenticate_failure_market_family_terminal_receipt_v3",
            "authenticate_failure_market_source_failure_lifecycle_terminal_v3",
            "authenticate_source_family_terminal_authority_v3",
            "retire_source_funding_custody_v3",
            "consume_source_family_terminal_into_product_v3",
        ] {
            assert!(outer.contains(required), "missing failed terminal step {required}");
        }
        assert!(!outer.contains("SourceFailureProductReleaseDispositionV3::"));
        assert!(!outer.contains("expected_terminal_id"));
        assert!(!outer.contains("expected_physical_id"));
        assert!(!outer.contains("AuthenticatedMarketLifecycleRootV2"));
    }

    #[test]
    fn physical_close_requires_live_terminal_root_and_conserves_every_lamport() {
        let source = include_str!("failure_market_family_terminal_v2.rs");
        let close = source
            .split("pub(crate) fn close_failure_market_family_after_product_terminal_v3")
            .nth(1)
            .and_then(|value| value.split("/// Produce one durable Failure terminal chain").next())
            .expect("RootV3 physical Failure close");
        for guard in [
            "authenticate_market_lifecycle_root_v3",
            "MarketLifecyclePhaseV3::Terminal",
            "failure_terminal_receipt_id() == failure_terminal.id()",
            "terminal_projection.root_semantic_id() == market_root.semantic_id()",
            "plan_close_failure_market_interval_accounts_v2",
            "project_root_balance_disposition",
            "runtime_donation >= runtime_funding.donation_floor_lamports",
            "Some(deleted_balance)",
            "require_failure_close_destination",
        ] {
            assert!(close.contains(guard), "missing physical-close guard {guard}");
        }
        let cell = close.find("interval_cell_account\n        .resize(0)").expect("cell close");
        let history = close.find("interval_history_account\n        .resize(0)").expect("history close");
        let runtime = close.find("runtime_root_account\n        .resize(0)").expect("runtime close");
        let admission = close.find("admission_root_account\n        .resize(0)").expect("admission close");
        assert!(cell < history && history < runtime && runtime < admission);
        assert!(!close.contains("AuthenticatedMarketInstanceTerminalV1"));
        assert!(!close.contains("AuthenticatedMarketLifecycleRootV2"));
    }

    #[test]
    fn physical_close_receipt_is_noncopy_and_not_product_authority() {
        let source = include_str!("failure_market_family_terminal_v2.rs");
        let receipt = source
            .split("pub(crate) struct AuthenticatedFailureMarketPhysicalCloseV3")
            .nth(1)
            .and_then(|value| value.split("fn require_failure_close_destination").next())
            .expect("physical close receipt");
        assert!(!receipt.contains("derive(Clone"));
        assert!(!receipt.contains("derive(Copy"));
        assert!(receipt.contains("pub(crate) const fn id(&self)"));
        assert!(!receipt.contains("ProductPostwrite"));
    }
}
