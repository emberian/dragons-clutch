// SPDX-License-Identifier: AGPL-3.0-or-later
//! Capability-disabled SBF seam for reusable Failure interval accounts.
//!
//! The Failure runtime owns every semantic byte in `0xab/v2` and `0xac/v2`.
//! `clutch-solana-layout` owns only their four-byte physical frames. This
//! module authenticates owner, fresh successor PDA, exact frame/body, present
//! principal, and stale preimages before writing a pure private-field plan.
//! It deliberately exposes no instruction route. Initialization is available
//! only through a crate-private Product adapter that joins the two accepted
//! retained-slot preallocation receipts for slots 8 and 9.

use crate::accounts::{expect_pda, require, require_distinct, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::failure_market_admission::{
    authenticate_failure_market_root_v2, AuthenticatedFailureMarketRootV2,
};
use crate::instructions::failure_market_runtime::{
    write_failure_market_runtime_session_plan_v1, AuthenticatedFailureMarketRuntimeRootV1,
    AuthenticatedFailureMarketRuntimeSessionPostwriteV1,
    AuthenticatedFailureMarketRuntimeSessionWriteV1, FailureMarketRuntimeSessionWriteFactsV1,
};
use crate::instructions::genesis::{
    allocate_data, assign_data, read_rent, require_system_program, SYSTEM_PROGRAM_ID,
};
use crate::instructions::product_market::{
    authenticate_market_instance_terminal_v1, release_series_market_link_failure_v1,
    AuthenticatedMarketFoundationPreallocationV2, AuthenticatedMarketInstanceTerminalV1,
    AuthenticatedMarketRecoveryScheduleV1, AuthenticatedSeriesFailureArchivePostwriteV2,
    AuthenticatedSeriesFailureSessionReleaseV1, AuthenticatedSeriesMarketLinkV1,
    AuthenticatedWritableFailureResolutionLinkV1,
};
use crate::seeds;
use clutch_failure_policy_runtime::market_interval_cell_v2::{
    initialize_failure_market_interval_cell_v2, plan_exhaust_failure_market_interval_cell_v2,
    AuthenticatedFailureMarketIntervalCellExhaustionV2,
    FailureMarketIntervalCellActivationReceiptV2,
    FailureMarketIntervalCellAdvancePlanV2, FailureMarketIntervalCellAdvanceReceiptV2,
    FailureMarketIntervalCellDispositionV2, FailureMarketIntervalCellExhaustionPlanV2,
    FailureMarketIntervalCellExhaustionFactsV2,
    FailureMarketIntervalCellPhaseV2, FailureMarketIntervalCellPlanV2,
    FailureMarketIntervalCellResetReceiptV2, FailureMarketIntervalCellResolutionPlanV2,
    FailureMarketIntervalCellResolutionReceiptV2, FailureMarketIntervalCellStateIdV2,
    FailureMarketIntervalCellV2, FAILURE_MARKET_INTERVAL_CELL_BYTES_V2,
};
use clutch_failure_policy_runtime::market_interval_history_v2::{
    admit_failure_market_interval_history_v2, plan_close_failure_market_interval_accounts_v2,
    reopen_failure_market_interval_funding_v2,
    AuthenticatedFailureMarketIntervalFundingV2, FailureMarketIntervalCloseAuthorizationIdV2,
    FailureMarketIntervalFamilySealReceiptV2, FailureMarketIntervalFundingFactsV2,
    FailureMarketIntervalFundingReceiptV2, FailureMarketIntervalHistoryAppendReceiptV2,
    FailureMarketIntervalHistoryPlanV2, FailureMarketIntervalHistoryStateIdV2,
    FailureMarketIntervalHistoryV2, FAILURE_MARKET_INTERVAL_HISTORY_BYTES_V2,
};
use clutch_failure_policy_runtime::market_policy_v1::{
    FailureMarketAccountIdV1, FailureMarketAdmissionStateIdV1,
};
use clutch_failure_policy_runtime::market_quote_v1::{
    admit_failure_market_recovery_quote_v1, AuthenticatedFailureMarketRecoveryQuoteV1,
    FailureMarketRecoveryQuoteAdmissionFactsV1, FailureMarketRecoveryQuoteAdmissionReceiptV1,
    FailureMarketRecoveryQuoteScheduleV1, FAILURE_MARKET_RECOVERY_QUOTE_SCHEDULE_BYTES_V1,
};
use clutch_failure_policy_runtime::market_runtime_v1::{
    plan_close_failure_market_session_v1, AuthenticatedFailureMarketSessionV1,
    FailureMarketSessionCloseFactsV1,
};
use clutch_product_series::{
    ContentId as ProductContentId, MarketFoundationSlotV2, SourceOccurrenceV1Id,
};
use clutch_liveness::runtime_adapter_v1::{
    decode_runtime_policy_account_v1, RuntimePersistedAccountViewV1,
};
use clutch_liveness::runtime_v1::{
    RuntimeCompartmentKindV1, RuntimeCompartmentPhaseV1, RuntimeCompartmentV1,
};
use clutch_liveness::Id as LivenessId;
use clutch_solana_layout::failure_market_interval_v2::{
    FailureMarketIntervalCellAccountV2, FailureMarketIntervalHistoryAccountV2,
    FAILURE_MARKET_INTERVAL_CELL_BODY_BYTES_V2, FAILURE_MARKET_INTERVAL_HISTORY_BODY_BYTES_V2,
};
use clutch_solana_layout::failure_recovery::{
    decode_failure_account_body_v1, FAILURE_EXTERNAL_RECOVERY_ACCOUNT_BYTES_V1,
    FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1, FAILURE_LIVENESS_POLICY_ACCOUNT_BYTES_V1,
    FAILURE_LIVENESS_POLICY_BODY_BYTES_V1,
};
use clutch_solana_layout::product_series::MarketLifecycleRootAccountV1;
use clutch_solana_layout::product_series::SeriesMarketLinkAccountV1;
use clutch_solana_layout::registry::{
    FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES, FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES,
};
use clutch_solana_layout::registry;
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const CELL_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/failure-market-interval-cell-account-authentication/v2";
const HISTORY_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/failure-market-interval-history-account-authentication/v2";
const CLOSE_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/failure-market-interval-physical-close/v2";
const ARCHIVE_POSTWRITE_DOMAIN_V2: &[u8] =
    b"dragons-clutch/failure-market-interval-archive-postwrite/v2";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProductFailureMarketRecoveryQuoteAuthorityV1 {
    expected: FailureMarketRecoveryQuoteAdmissionFactsV1,
}

impl AuthenticatedFailureMarketRecoveryQuoteV1 for ProductFailureMarketRecoveryQuoteAuthorityV1 {
    fn authenticate_failure_market_recovery_quote(
        &self,
        expected: FailureMarketRecoveryQuoteAdmissionFactsV1,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if expected != self.expected {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketExhaustionLivenessAuthorityV2 {
    cell_state_id: FailureMarketIntervalCellStateIdV2,
    completed_calls: u64,
    keeper_paid_lamports: u64,
    remaining_calls: u32,
    remaining_work_lamports: u64,
}

impl AuthenticatedFailureMarketIntervalCellExhaustionV2
    for FailureMarketExhaustionLivenessAuthorityV2
{
    fn authenticate_failure_market_interval_cell_exhaustion(
        &self,
        expected: FailureMarketIntervalCellExhaustionFactsV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        let boundary_matches = match expected.reason {
            clutch_failure_policy_runtime::market_interval_cell_v2::FailureMarketIntervalExhaustionReasonV2::AttemptProgress => true,
            clutch_failure_policy_runtime::market_interval_cell_v2::FailureMarketIntervalExhaustionReasonV2::MarketCalls => {
                self.remaining_calls == 0
            }
            clutch_failure_policy_runtime::market_interval_cell_v2::FailureMarketIntervalExhaustionReasonV2::MarketPrincipal => {
                self.remaining_work_lamports == 0
            }
        };
        if expected.cell_before != self.cell_state_id
            || expected.aggregate_work_calls != self.completed_calls
            || expected.aggregate_reward_lamports != self.keeper_paid_lamports
            || !boundary_matches
        {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

/// Authenticate the read-only Recovery custody and derive the only canonical
/// finite exhaustion transition for the current reusable session.
pub(crate) fn plan_failure_market_interval_exhaustion_v2(
    program_id: &Pubkey,
    liveness_policy_account: &AccountInfo<'_>,
    recovery_account: &AccountInfo<'_>,
    admission: AuthenticatedFailureMarketRootV2,
    interval: AuthenticatedFailureMarketIntervalAccountsV2,
) -> Outcome<FailureMarketIntervalCellExhaustionPlanV2> {
    require(
        liveness_policy_account.owner == program_id
            && !liveness_policy_account.is_writable
            && !liveness_policy_account.is_signer
            && !liveness_policy_account.executable
            && liveness_policy_account.data_len() == FAILURE_LIVENESS_POLICY_ACCOUNT_BYTES_V1
            && recovery_account.owner == program_id
            && !recovery_account.is_writable
            && !recovery_account.is_signer
            && !recovery_account.executable
            && recovery_account.data_len() == FAILURE_EXTERNAL_RECOVERY_ACCOUNT_BYTES_V1,
        ClutchError::MismatchedState,
    )?;
    let policy_data = liveness_policy_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let recovery_data = recovery_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let policy_frame = decode_failure_account_body_v1(
        &policy_data,
        registry::FAILURE_LIVENESS_POLICY_ACCOUNT_TAG,
        registry::FAILURE_LIVENESS_POLICY_ACCOUNT_VERSION,
        FAILURE_LIVENESS_POLICY_BODY_BYTES_V1,
    )?;
    let recovery_frame = decode_failure_account_body_v1(
        &recovery_data,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_TAG,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_VERSION,
        FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1,
    )?;
    let policy = decode_runtime_policy_account_v1(
        liveness_id(program_id),
        liveness_id(liveness_policy_account.key),
        RuntimePersistedAccountViewV1 {
            account_id: liveness_id(liveness_policy_account.key),
            owner_program_id: liveness_id(liveness_policy_account.owner),
            lamports: liveness_policy_account.lamports(),
            data: policy_frame.body,
            writable: false,
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let recovery = RuntimeCompartmentV1::decode(recovery_frame.body)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    recovery
        .validate_against_policy(policy)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let facts = admission.state().binding().facts();
    let quote = interval.quote().facts();
    require(
        recovery.kind == RuntimeCompartmentKindV1::Recovery
            && recovery.phase == RuntimeCompartmentPhaseV1::Active
            && recovery.identity.account_id == liveness_id(recovery_account.key)
            && recovery.identity.account_id == facts.recovery_compartment_account_id
            && recovery.identity.owner == liveness_id(program_id)
            && recovery.identity.policy_id == policy.policy_id
            && policy.policy_id == facts.liveness_policy_id
            && recovery.identity.lifecycle_id == facts.liveness_lifecycle_id
            && recovery.identity.generation == facts.generation
            && recovery.quote_schedule_id == facts.recovery_quote_schedule_id
            && recovery.quote_schedule_id.bytes() == quote.quote_schedule_id.bytes()
            && recovery.maximum_calls == quote.maximum_calls
            && recovery.maximum_lamports_per_call == quote.maximum_lamports_per_call
            && recovery.capitalized_work_lamports == quote.work_principal_lamports
            && recovery.completed_work_ceiling_lamports == recovery.keeper_paid_lamports
            && recovery_account.lamports()
                >= recovery
                    .expected_account_balance_lamports()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        liveness_policy_account.key,
        seeds::failure_liveness_policy_pda(program_id, &policy.policy_id.bytes()),
        Some(policy_frame.stored_bump),
    )?;
    expect_pda(
        recovery_account.key,
        seeds::failure_external_recovery_pda(
            program_id,
            &recovery.identity.lifecycle_id.bytes(),
            recovery.identity.generation,
        ),
        Some(recovery_frame.stored_bump),
    )?;
    let authority = FailureMarketExhaustionLivenessAuthorityV2 {
        cell_state_id: interval.cell_state_id(),
        completed_calls: u64::from(recovery.completed_calls),
        keeper_paid_lamports: recovery.keeper_paid_lamports,
        remaining_calls: recovery.remaining_calls,
        remaining_work_lamports: recovery.remaining_work_lamports,
    };
    plan_exhaust_failure_market_interval_cell_v2(
        &authority,
        interval.cell(),
        admission.state(),
        interval.funding(),
        interval.history(),
        interval.quote(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

/// Hostile-decode and admit the exact content-addressed shared Recovery reward
/// schedule against Product's live Registry/liveness-policy authentication.
/// The schedule bytes are a preimage only; their recomputed typed identity and
/// every bound/capital field must equal the private Product receipt.
pub(crate) fn authenticate_failure_market_recovery_quote_v1(
    admission: AuthenticatedFailureMarketRootV2,
    product: AuthenticatedMarketRecoveryScheduleV1,
    body: &[u8],
) -> Outcome<FailureMarketRecoveryQuoteAdmissionReceiptV1> {
    let body: &[u8; FAILURE_MARKET_RECOVERY_QUOTE_SCHEDULE_BYTES_V1] = body
        .try_into()
        .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
    let schedule = FailureMarketRecoveryQuoteScheduleV1::decode(body)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let schedule_id = schedule
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let policy = admission.state().binding().facts();
    let expected = FailureMarketRecoveryQuoteAdmissionFactsV1 {
        failure_policy_binding_id: admission.state().binding().id(),
        quote_schedule_id: schedule_id,
        maximum_calls: schedule.maximum_calls,
        maximum_progress_units_per_call: schedule.maximum_progress_units_per_call,
        maximum_lamports_per_call: schedule
            .maximum_lamports_per_call()
            .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?,
        work_principal_lamports: schedule
            .work_principal_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?,
    };
    require(
        product.market_root_account() != admission.account()
            && product.recovery_quote_schedule_id().bytes() == schedule_id.bytes()
            && product.maximum_calls() == expected.maximum_calls
            && product.maximum_progress_units_per_call()
                == expected.maximum_progress_units_per_call
            && product.maximum_lamports_per_call() == expected.maximum_lamports_per_call
            && product.work_capital_lamports() == expected.work_principal_lamports
            && product.liveness_policy_id().bytes() == policy.liveness_policy_id.bytes()
            && product.receipt_program_id().bytes()
                == policy.recovery_receipt_program_id.bytes()
            && product.capability_profile_id().bytes()
                == policy.capability_profile_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    let authority = ProductFailureMarketRecoveryQuoteAuthorityV1 { expected };
    admit_failure_market_recovery_quote_v1(
        &authority,
        admission.state().binding(),
        admission.state().recovery_funding(),
        schedule,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

/// Exact content preimage needed to reopen the Product-authenticated interval
/// capitalization receipt after the retained accounts have been allocated.
/// The permanent history account stores the domain-separated receipt ID, so
/// these hostile bytes cannot create authority or alter any funding fact.
pub(crate) const FAILURE_MARKET_INTERVAL_FUNDING_PREIMAGE_BYTES_V2: usize = 176;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FailureMarketIntervalFundingPreimageV2 {
    work_preallocation_receipt_id: ProductContentId,
    history_preallocation_receipt_id: ProductContentId,
    foundation_schedule_id: ProductContentId,
    foundation_account_graph_id: ProductContentId,
    foundation_transcript_id: ProductContentId,
    work_donation_floor_lamports: u64,
    history_donation_floor_lamports: u64,
}

impl FailureMarketIntervalFundingPreimageV2 {
    /// Hostile-decode every byte of the immutable funding receipt preimage.
    pub(crate) fn decode(input: &[u8]) -> Outcome<Self> {
        let input: &[u8; FAILURE_MARKET_INTERVAL_FUNDING_PREIMAGE_BYTES_V2] = input
            .try_into()
            .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
        let mut at = 0usize;
        let mut take_id = || -> Outcome<ProductContentId> {
            let end = at.checked_add(32).ok_or(ClutchError::Arithmetic)?;
            let bytes: [u8; 32] = input[at..end]
                .try_into()
                .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
            at = end;
            let id = ProductContentId::from_bytes(bytes);
            require(!id.is_zero(), ClutchError::MismatchedState)?;
            Ok(id)
        };
        let work_preallocation_receipt_id = take_id()?;
        let history_preallocation_receipt_id = take_id()?;
        let foundation_schedule_id = take_id()?;
        let foundation_account_graph_id = take_id()?;
        let foundation_transcript_id = take_id()?;
        let work_donation_floor_lamports = u64::from_le_bytes(
            input[160..168]
                .try_into()
                .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?,
        );
        let history_donation_floor_lamports = u64::from_le_bytes(
            input[168..176]
                .try_into()
                .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?,
        );
        require(
            work_preallocation_receipt_id != history_preallocation_receipt_id,
            ClutchError::MismatchedState,
        )?;
        Ok(Self {
            work_preallocation_receipt_id,
            history_preallocation_receipt_id,
            foundation_schedule_id,
            foundation_account_graph_id,
            foundation_transcript_id,
            work_donation_floor_lamports,
            history_donation_floor_lamports,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketRuntimeArchiveAuthorityV1 {
    expected: FailureMarketSessionCloseFactsV1,
}

impl AuthenticatedFailureMarketSessionV1 for FailureMarketRuntimeArchiveAuthorityV1 {
    fn authenticate_failure_market_session_close(
        &self,
        expected: FailureMarketSessionCloseFactsV1,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if expected.runtime_before != self.expected.runtime_before
            || expected.series_link_before != self.expected.series_link_before
            || expected.series_link_after != self.expected.series_link_after
            || expected.session_before != self.expected.session_before
            || expected.session_after != self.expected.session_after
            || expected.interval_terminal_receipt_id != self.expected.interval_terminal_receipt_id
            || expected.previous_session_history != self.expected.previous_session_history
            || expected.resulting_session_history != self.expected.resulting_session_history
            || expected.history_append_receipt_id != self.expected.history_append_receipt_id
            || expected.history_before != self.expected.history_before
            || expected.history_after != self.expected.history_after
            || expected.completed_session_count != self.expected.completed_session_count
            || expected.transition_receipt_id.bytes() == [0; 32]
        {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketRuntimeArchiveWriteV1 {
    expected: FailureMarketRuntimeSessionWriteFactsV1,
    archive_idle_state: ProductContentId,
    runtime_idle_state: ProductContentId,
    release_link_after: ProductContentId,
    runtime_link_after: ProductContentId,
    release_terminal_receipt_id: ProductContentId,
    runtime_terminal_receipt_id: ProductContentId,
}

impl AuthenticatedFailureMarketRuntimeSessionWriteV1 for FailureMarketRuntimeArchiveWriteV1 {
    fn authenticate_failure_market_runtime_session_write_v1(
        &self,
        expected: FailureMarketRuntimeSessionWriteFactsV1,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if expected != self.expected
            || self.archive_idle_state != self.runtime_idle_state
            || self.release_link_after != self.runtime_link_after
            || self.release_terminal_receipt_id != self.runtime_terminal_receipt_id
        {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

const _: () =
    assert!(FAILURE_MARKET_INTERVAL_CELL_BODY_BYTES_V2 == FAILURE_MARKET_INTERVAL_CELL_BYTES_V2);
const _: () = assert!(
    FAILURE_MARKET_INTERVAL_HISTORY_BODY_BYTES_V2 == FAILURE_MARKET_INTERVAL_HISTORY_BYTES_V2
);

/// Exact authenticated reusable-cell and append-only-history pair.
///
/// Private fields prevent an instruction module from lowering caller IDs into
/// account authority. The pure funding and quote receipts used to decode the
/// bodies are retained for same-instruction postwrite reauthentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedFailureMarketIntervalAccountsV2 {
    cell_account: Pubkey,
    cell_bump: u8,
    cell: FailureMarketIntervalCellV2,
    cell_state_id: FailureMarketIntervalCellStateIdV2,
    cell_data_id: ProductContentId,
    cell_authentication_id: ProductContentId,
    cell_observed_lamports: u64,
    history_account: Pubkey,
    history_bump: u8,
    history: FailureMarketIntervalHistoryV2,
    history_state_id: FailureMarketIntervalHistoryStateIdV2,
    history_data_id: ProductContentId,
    history_authentication_id: ProductContentId,
    history_observed_lamports: u64,
    admission_root_account: Pubkey,
    admission_state_id: FailureMarketAdmissionStateIdV1,
    funding: FailureMarketIntervalFundingReceiptV2,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
}

impl AuthenticatedFailureMarketIntervalAccountsV2 {
    /// Exact reusable cell account.
    pub(crate) const fn cell_account(self) -> Pubkey {
        self.cell_account
    }

    /// Complete authenticated reusable-cell state.
    pub(crate) const fn cell(self) -> FailureMarketIntervalCellV2 {
        self.cell
    }

    /// Complete reusable-cell semantic commitment.
    pub(crate) const fn cell_state_id(self) -> FailureMarketIntervalCellStateIdV2 {
        self.cell_state_id
    }

    /// Owner/PDA/frame/body/balance authentication for the cell preimage.
    pub(crate) const fn cell_authentication_id(self) -> ProductContentId {
        self.cell_authentication_id
    }

    /// Exact append-only history account.
    pub(crate) const fn history_account(self) -> Pubkey {
        self.history_account
    }

    /// Complete authenticated append-only history.
    pub(crate) const fn history(self) -> FailureMarketIntervalHistoryV2 {
        self.history
    }

    /// Complete append-only-history semantic commitment.
    pub(crate) const fn history_state_id(self) -> FailureMarketIntervalHistoryStateIdV2 {
        self.history_state_id
    }

    /// Owner/PDA/frame/body/balance authentication for the history preimage.
    pub(crate) const fn history_authentication_id(self) -> ProductContentId {
        self.history_authentication_id
    }

    /// Exact immutable Failure admission account used for both body joins.
    pub(crate) const fn admission_root_account(self) -> Pubkey {
        self.admission_root_account
    }

    /// Complete immutable Failure admission state used for hostile decoding.
    pub(crate) const fn admission_state_id(self) -> FailureMarketAdmissionStateIdV1 {
        self.admission_state_id
    }

    /// Product-authenticated reusable-account capitalization.
    pub(crate) const fn funding(self) -> FailureMarketIntervalFundingReceiptV2 {
        self.funding
    }

    /// Market-scoped liveness quote admission used for hostile decoding.
    pub(crate) const fn quote(self) -> FailureMarketRecoveryQuoteAdmissionReceiptV1 {
        self.quote
    }
}

/// Atomic postimage of Product-authorized slot-8/slot-9 allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FailureMarketIntervalPostimageV2 {
    accounts: AuthenticatedFailureMarketIntervalAccountsV2,
    funding: FailureMarketIntervalFundingReceiptV2,
}

/// Exact paired `0xac` append and `0xab` Idle-reset postwrite.
///
/// This receipt remains private to the Failure owner. Product's narrow link
/// release must consume it in the same outer instruction before the archive
/// operation can return to a routed caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FailureMarketIntervalArchivePostwriteV2 {
    id: ProductContentId,
    accounts: AuthenticatedFailureMarketIntervalAccountsV2,
    append: FailureMarketIntervalHistoryAppendReceiptV2,
    reset: FailureMarketIntervalCellResetReceiptV2,
    source_occurrence_id: SourceOccurrenceV1Id,
    resolution_link_preauthorization_id: ProductContentId,
}

impl FailureMarketIntervalArchivePostwriteV2 {
    /// Exact paired physical/semantic postwrite identity.
    pub(crate) const fn id(self) -> ProductContentId {
        self.id
    }

    /// Reauthenticated canonical Idle cell and appended history.
    pub(crate) const fn accounts(self) -> AuthenticatedFailureMarketIntervalAccountsV2 {
        self.accounts
    }

    /// Exact terminal session folded into append-only history.
    pub(crate) const fn append(self) -> FailureMarketIntervalHistoryAppendReceiptV2 {
        self.append
    }

    /// Exact reset receipt paired to the append.
    pub(crate) const fn reset(self) -> FailureMarketIntervalCellResetReceiptV2 {
        self.reset
    }

    /// Exact Source occurrence retained from the terminal Product work before
    /// canonical Idle reset clears the reusable cell's session-local body.
    pub(crate) const fn source_occurrence_id(self) -> SourceOccurrenceV1Id {
        self.source_occurrence_id
    }

    /// Exact Product-owned writable-link preauthorization consumed at release.
    pub(crate) const fn resolution_link_preauthorization_id(self) -> ProductContentId {
        self.resolution_link_preauthorization_id
    }
}

impl AuthenticatedSeriesFailureArchivePostwriteV2 for FailureMarketIntervalArchivePostwriteV2 {
    fn archive_postwrite_id(&self) -> Outcome<ProductContentId> {
        Ok(self.id)
    }

    fn append_receipt_id(&self) -> Outcome<ProductContentId> {
        Ok(ProductContentId::from_bytes(self.append.id().bytes()))
    }

    fn reset_receipt_id(&self) -> Outcome<ProductContentId> {
        Ok(ProductContentId::from_bytes(self.reset.id().bytes()))
    }

    fn market_instance_id(&self) -> Outcome<clutch_product_series::MarketInstanceV2Id> {
        Ok(self.append.market_instance_id())
    }

    fn generation(&self) -> Outcome<u64> {
        Ok(self.append.generation())
    }

    fn source_occurrence_id(&self) -> Outcome<SourceOccurrenceV1Id> {
        Ok(self.source_occurrence_id)
    }

    fn session_binding_id(&self) -> Outcome<ProductContentId> {
        Ok(ProductContentId::from_bytes(
            self.append.session_binding_id().bytes(),
        ))
    }

    fn session_terminal_receipt_id(&self) -> Outcome<ProductContentId> {
        Ok(ProductContentId::from_bytes(
            self.append.session_terminal_receipt_id().bytes(),
        ))
    }

    fn resolution_link_preauthorization_id(&self) -> Outcome<ProductContentId> {
        Ok(self.resolution_link_preauthorization_id)
    }

    fn authenticate_series_failure_archive_postwrite_v2(
        &self,
        archive_postwrite_id: ProductContentId,
        append_receipt_id: ProductContentId,
        reset_receipt_id: ProductContentId,
        market_instance_id: clutch_product_series::MarketInstanceV2Id,
        generation: u64,
        source_occurrence_id: SourceOccurrenceV1Id,
        session_binding_id: ProductContentId,
        session_terminal_receipt_id: ProductContentId,
        resolution_link_preauthorization_id: ProductContentId,
    ) -> Outcome<()> {
        require(
            archive_postwrite_id == self.id
                && append_receipt_id.bytes() == self.append.id().bytes()
                && reset_receipt_id.bytes() == self.reset.id().bytes()
                && market_instance_id == self.append.market_instance_id()
                && generation == self.append.generation()
                && source_occurrence_id == self.source_occurrence_id
                && session_binding_id.bytes() == self.append.session_binding_id().bytes()
                && session_terminal_receipt_id.bytes()
                    == self.append.session_terminal_receipt_id().bytes()
                && resolution_link_preauthorization_id
                    == self.resolution_link_preauthorization_id,
            ClutchError::MismatchedState,
        )
    }
}

impl FailureMarketIntervalPostimageV2 {
    /// Newly persisted canonical Idle cell and empty append-only history.
    pub(crate) const fn accounts(self) -> AuthenticatedFailureMarketIntervalAccountsV2 {
        self.accounts
    }

    /// Exact paired Product-preallocation funding receipt.
    pub(crate) const fn funding(self) -> FailureMarketIntervalFundingReceiptV2 {
        self.funding
    }
}

/// Module-private bridge over Product's paired retained-slot receipts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProductFailureMarketIntervalFundingV2 {
    expected: FailureMarketIntervalFundingFactsV2,
}

impl AuthenticatedFailureMarketIntervalFundingV2 for ProductFailureMarketIntervalFundingV2 {
    fn authenticate_failure_market_interval_funding(
        &self,
        expected: FailureMarketIntervalFundingFactsV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if self.expected != expected {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

/// Product-private proof that slot10 Resolution V5 and the shared active root
/// were atomically persisted from one exact Failure interval receipt.
///
/// The method defaults to refusal and accepts the private receipt itself, not
/// a caller-authored payout/postimage projection. Product's same-program SBF
/// postwrite receipt is the only intended implementation.
pub(crate) trait AuthenticatedFailureMarketProductResolutionV2 {
    /// Authenticate the exact sole payout truth consumed by Product.
    fn authenticate_failure_market_product_resolution(
        &self,
        _expected: FailureMarketIntervalCellResolutionReceiptV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        Err(clutch_failure_policy_runtime::Error::BindingMismatch)
    }
}

/// Default-refusing authority for the sole atomic Idle-to-Active writer.
///
/// The implementation lives in the Product/Source/Failure Begin composer and
/// is minted only after deriving the noncircular preauthorization and predicted
/// post-pin Product transcript. A pure cell plan is not sufficient authority.
pub(crate) trait AuthenticatedFailureMarketIntervalBeginV2 {
    fn authenticate_failure_market_interval_begin_v2(
        &self,
        _expected: FailureMarketIntervalCellActivationReceiptV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        Err(clutch_failure_policy_runtime::Error::BindingMismatch)
    }
}

/// Default-refusing authority for one exact liveness-paid Active transition.
pub(crate) trait AuthenticatedFailureMarketIntervalPaidAdvanceV2 {
    fn authenticate_failure_market_interval_paid_advance_v2(
        &self,
        _expected: FailureMarketIntervalCellAdvanceReceiptV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        Err(clutch_failure_policy_runtime::Error::BindingMismatch)
    }
}

/// Exact authenticated physical close of the sealed reusable interval pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedFailureMarketIntervalCloseV2 {
    id: ProductContentId,
    close_authorization_id: FailureMarketIntervalCloseAuthorizationIdV2,
    market_terminal_authentication_id: ProductContentId,
    cell_account: Pubkey,
    history_account: Pubkey,
    rent_refund_owner: Pubkey,
    neutral_sink: Pubkey,
    refunded_principal_lamports: u64,
    neutralized_donation_lamports: u64,
}

impl AuthenticatedFailureMarketIntervalCloseV2 {
    /// Complete Product/Failure/physical close authentication.
    pub(crate) const fn id(self) -> ProductContentId {
        self.id
    }

    /// Pure Failure close authorization consumed by the writer.
    pub(crate) const fn close_authorization_id(
        self,
    ) -> FailureMarketIntervalCloseAuthorizationIdV2 {
        self.close_authorization_id
    }

    /// Live Product terminal-account authentication consumed by this close.
    pub(crate) const fn market_terminal_authentication_id(self) -> ProductContentId {
        self.market_terminal_authentication_id
    }

    /// Deleted reusable-cell account.
    pub(crate) const fn cell_account(self) -> Pubkey {
        self.cell_account
    }

    /// Deleted append-only history account.
    pub(crate) const fn history_account(self) -> Pubkey {
        self.history_account
    }

    /// Immutable principal recipient.
    pub(crate) const fn rent_refund_owner(self) -> Pubkey {
        self.rent_refund_owner
    }

    /// Immutable unsolicited-lamport sink.
    pub(crate) const fn neutral_sink(self) -> Pubkey {
        self.neutral_sink
    }

    /// Exact principal returned to the immutable refund owner.
    pub(crate) const fn refunded_principal_lamports(self) -> u64 {
        self.refunded_principal_lamports
    }

    /// Exact unsolicited surplus sent only to the immutable neutral sink.
    pub(crate) const fn neutralized_donation_lamports(self) -> u64 {
        self.neutralized_donation_lamports
    }
}

/// Allocate and write exact Product-prepaid slot-8/slot-9 successors.
///
/// This helper is crate-private and non-routable. The authority must be a
/// private adapter joining both Product preallocation receipts, their common
/// root/schedule/graph/transcript, live Rent, and the exact current zero-data
/// balances. No caller-built amount or generic funding ID is sufficient.
#[allow(clippy::too_many_arguments)]
pub(crate) fn initialize_failure_market_interval_accounts_v2<'a>(
    program_id: &Pubkey,
    admission_root_account: &AccountInfo<'a>,
    cell_account: &AccountInfo<'a>,
    history_account: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    admission: AuthenticatedFailureMarketRootV2,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    work_preallocation: AuthenticatedMarketFoundationPreallocationV2,
    history_preallocation: AuthenticatedMarketFoundationPreallocationV2,
) -> Outcome<FailureMarketIntervalPostimageV2> {
    require_system_program(system_program)?;
    require_distinct(&[
        admission_root_account.clone(),
        cell_account.clone(),
        history_account.clone(),
        rent_sysvar.clone(),
        system_program.clone(),
    ])?;
    let live_admission =
        authenticate_failure_market_root_v2(program_id, admission_root_account, false)?;
    require(live_admission == admission, ClutchError::MismatchedState)?;
    let admission = live_admission;
    let admission_state = admission.state();
    let policy = admission_state.binding().facts();
    require(
        work_preallocation.slot() == MarketFoundationSlotV2::FailureIntervalWork
            && history_preallocation.slot() == MarketFoundationSlotV2::FailureIntervalHistory
            && work_preallocation.id() != history_preallocation.id()
            && work_preallocation.root_account() == history_preallocation.root_account()
            && work_preallocation.root_authentication_id()
                == history_preallocation.root_authentication_id()
            && work_preallocation.market_instance_id() == policy.market_instance_id
            && history_preallocation.market_instance_id() == policy.market_instance_id
            && work_preallocation.generation() == policy.generation
            && history_preallocation.generation() == policy.generation
            && work_preallocation.account() == *cell_account.key
            && history_preallocation.account() == *history_account.key
            && work_preallocation.foundation_schedule_id()
                == history_preallocation.foundation_schedule_id()
            && work_preallocation.foundation_account_graph_id()
                == history_preallocation.foundation_account_graph_id()
            && work_preallocation.foundation_transcript_id()
                == history_preallocation.foundation_transcript_id()
            && work_preallocation.rent_refund_owner() == history_preallocation.rent_refund_owner()
            && work_preallocation.neutral_lamport_sink()
                == history_preallocation.neutral_lamport_sink()
            && work_preallocation.root_account() != admission.account()
            && work_preallocation.root_account() != *cell_account.key
            && work_preallocation.root_account() != *history_account.key,
        ClutchError::MismatchedState,
    )?;
    let funding_facts = FailureMarketIntervalFundingFactsV2 {
        failure_policy_binding_id: admission_state.binding().id(),
        market_instance_id: policy.market_instance_id,
        generation: policy.generation,
        work_preallocation_receipt_id: work_preallocation.id(),
        history_preallocation_receipt_id: history_preallocation.id(),
        foundation_schedule_id: work_preallocation.foundation_schedule_id(),
        foundation_account_graph_id: work_preallocation.foundation_account_graph_id(),
        foundation_transcript_id: work_preallocation.foundation_transcript_id(),
        work_account: FailureMarketAccountIdV1::from_bytes(cell_account.key.to_bytes()),
        history_account: FailureMarketAccountIdV1::from_bytes(history_account.key.to_bytes()),
        rent_refund_owner: FailureMarketAccountIdV1::from_bytes(
            work_preallocation.rent_refund_owner().to_bytes(),
        ),
        neutral_sink: FailureMarketAccountIdV1::from_bytes(
            work_preallocation.neutral_lamport_sink().to_bytes(),
        ),
        work_rent_principal_lamports: work_preallocation.principal_lamports(),
        history_rent_principal_lamports: history_preallocation.principal_lamports(),
        work_donation_floor_lamports: work_preallocation.donation_lamports(),
        work_observed_balance_lamports: work_preallocation.observed_balance_lamports(),
        history_donation_floor_lamports: history_preallocation.donation_lamports(),
        history_observed_balance_lamports: history_preallocation.observed_balance_lamports(),
    };
    let product_preallocation_authority = ProductFailureMarketIntervalFundingV2 {
        expected: funding_facts,
    };
    let work_balance = funding_facts
        .work_rent_principal_lamports
        .checked_add(funding_facts.work_donation_floor_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    let history_balance = funding_facts
        .history_rent_principal_lamports
        .checked_add(funding_facts.history_donation_floor_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    let rent = read_rent(rent_sysvar)?;
    require(
        funding_facts.work_account.bytes() == cell_account.key.to_bytes()
            && funding_facts.history_account.bytes() == history_account.key.to_bytes()
            && funding_facts.failure_policy_binding_id == admission_state.binding().id()
            && funding_facts.market_instance_id == policy.market_instance_id
            && funding_facts.generation == policy.generation
            && funding_facts.work_observed_balance_lamports == work_balance
            && funding_facts.history_observed_balance_lamports == history_balance
            && funding_facts.work_rent_principal_lamports
                == rent.minimum_balance(FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES)?
            && funding_facts.history_rent_principal_lamports
                == rent.minimum_balance(FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES)?,
        ClutchError::MismatchedState,
    )?;
    for (account, expected_balance) in [
        (cell_account, work_balance),
        (history_account, history_balance),
    ] {
        require(
            account.owner.to_bytes() == SYSTEM_PROGRAM_ID
                && account.is_writable
                && !account.is_signer
                && !account.executable
                && account.data_len() == 0
                && account.lamports() == expected_balance,
            ClutchError::MismatchedState,
        )?;
    }
    let (expected_cell, cell_bump) = seeds::failure_market_interval_cell_v2_pda(
        program_id,
        &policy.market_instance_id.bytes(),
        policy.generation,
    );
    let (expected_history, history_bump) = seeds::failure_market_interval_history_v2_pda(
        program_id,
        &policy.market_instance_id.bytes(),
        policy.generation,
    );
    expect_pda(cell_account.key, (expected_cell, cell_bump), None)?;
    expect_pda(history_account.key, (expected_history, history_bump), None)?;
    let (history, funding) = admit_failure_market_interval_history_v2(
        &product_preallocation_authority,
        admission_state,
        quote,
        funding_facts,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let cell = initialize_failure_market_interval_cell_v2(admission_state, funding, history, quote)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    allocate_assign_interval_account_v2(
        program_id,
        cell_account,
        system_program,
        seeds::SEED_FAILURE_MARKET_INTERVAL_CELL_V2,
        policy.market_instance_id.bytes(),
        policy.generation,
        cell_bump,
        FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES,
    )?;
    allocate_assign_interval_account_v2(
        program_id,
        history_account,
        system_program,
        seeds::SEED_FAILURE_MARKET_INTERVAL_HISTORY_V2,
        policy.market_instance_id.bytes(),
        policy.generation,
        history_bump,
        FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES,
    )?;
    let cell_output = encode_cell(cell_bump, cell)?;
    let history_output = encode_history(history_bump, history)?;
    {
        let mut cell_data = cell_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut history_data = history_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        require(
            cell_data.iter().all(|byte| *byte == 0) && history_data.iter().all(|byte| *byte == 0),
            ClutchError::AlreadyInitialized,
        )?;
        let cell_destination: &mut [u8; FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES] = cell_data
            .as_mut()
            .try_into()
            .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
        let history_destination: &mut [u8; FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES] =
            history_data
                .as_mut()
                .try_into()
                .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
        cell_destination.copy_from_slice(&cell_output);
        history_destination.copy_from_slice(&history_output);
    }
    require(
        cell_account.lamports() == work_balance && history_account.lamports() == history_balance,
        ClutchError::MismatchedState,
    )?;
    let accounts = authenticate_failure_market_interval_accounts_v2(
        program_id,
        cell_account,
        history_account,
        admission,
        funding,
        quote,
        true,
        true,
    )?;
    require(
        accounts.cell == cell && accounts.history == history,
        ClutchError::MismatchedState,
    )?;
    Ok(FailureMarketIntervalPostimageV2 { accounts, funding })
}

/// Authenticate exact existing `0xab/v2` and `0xac/v2` accounts.
///
/// The funding receipt must have been minted from Product's private account
/// graph/foundation authority. Fresh v2 PDA domains keep withdrawn one-shot v1
/// accounts from aliasing these reusable successors.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_failure_market_interval_accounts_v2<'a>(
    program_id: &Pubkey,
    cell_account: &AccountInfo<'a>,
    history_account: &AccountInfo<'a>,
    admission: AuthenticatedFailureMarketRootV2,
    funding: FailureMarketIntervalFundingReceiptV2,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    cell_writable: bool,
    history_writable: bool,
) -> Outcome<AuthenticatedFailureMarketIntervalAccountsV2> {
    require(
        *cell_account.key != *history_account.key
            && *cell_account.key != admission.account()
            && *history_account.key != admission.account(),
        ClutchError::AccountAlias,
    )?;
    authenticate_account_metadata(
        program_id,
        cell_account,
        FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES,
        cell_writable,
    )?;
    authenticate_account_metadata(
        program_id,
        history_account,
        FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES,
        history_writable,
    )?;

    let facts = funding.facts();
    let policy = admission.state().binding().facts();
    require(
        facts.work_account.bytes() == cell_account.key.to_bytes()
            && facts.history_account.bytes() == history_account.key.to_bytes()
            && facts.failure_policy_binding_id == admission.state().binding().id()
            && facts.market_instance_id == policy.market_instance_id
            && facts.generation == policy.generation
            && cell_account.lamports() >= facts.work_observed_balance_lamports
            && history_account.lamports() >= facts.history_observed_balance_lamports,
        ClutchError::MismatchedState,
    )?;

    let history_data = history_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let history_input: &[u8; FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES] = history_data
        .as_ref()
        .try_into()
        .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
    let history_frame = FailureMarketIntervalHistoryAccountV2::decode(history_input)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let history = FailureMarketIntervalHistoryV2::decode_for_admission(
        history_frame.semantic_body(),
        admission.state(),
        quote,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let history_data_id = framed_data_id(history_data.as_ref());
    drop(history_data);

    let cell_data = cell_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let cell_input: &[u8; FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES] = cell_data
        .as_ref()
        .try_into()
        .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
    let cell_frame = FailureMarketIntervalCellAccountV2::decode(cell_input)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let cell = FailureMarketIntervalCellV2::decode_for_admission(
        cell_frame.semantic_body(),
        admission.state(),
        funding,
        history,
        quote,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let cell_data_id = framed_data_id(cell_data.as_ref());
    drop(cell_data);

    expect_pda(
        cell_account.key,
        seeds::failure_market_interval_cell_v2_pda(
            program_id,
            &policy.market_instance_id.bytes(),
            policy.generation,
        ),
        Some(cell_frame.bump()),
    )?;
    expect_pda(
        history_account.key,
        seeds::failure_market_interval_history_v2_pda(
            program_id,
            &policy.market_instance_id.bytes(),
            policy.generation,
        ),
        Some(history_frame.bump()),
    )?;

    let cell_state_id = cell
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let history_state_id = history
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let admission_state_id = admission
        .state()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let cell_authentication_id = account_authentication_id(
        CELL_AUTHENTICATION_DOMAIN_V2,
        cell_account,
        cell_data_id,
        cell_state_id.bytes(),
        admission_state_id.bytes(),
    );
    let history_authentication_id = account_authentication_id(
        HISTORY_AUTHENTICATION_DOMAIN_V2,
        history_account,
        history_data_id,
        history_state_id.bytes(),
        admission_state_id.bytes(),
    );
    require_live_data_id(cell_authentication_id)?;
    require_live_data_id(history_authentication_id)?;
    Ok(AuthenticatedFailureMarketIntervalAccountsV2 {
        cell_account: *cell_account.key,
        cell_bump: cell_frame.bump(),
        cell,
        cell_state_id,
        cell_data_id,
        cell_authentication_id,
        cell_observed_lamports: cell_account.lamports(),
        history_account: *history_account.key,
        history_bump: history_frame.bump(),
        history,
        history_state_id,
        history_data_id,
        history_authentication_id,
        history_observed_lamports: history_account.lamports(),
        admission_root_account: admission.account(),
        admission_state_id,
        funding,
        quote,
    })
}

/// Reopen the immutable interval capitalization receipt from the exact
/// content preimage committed by the live history account, then authenticate
/// the paired live accounts. Product's creation receipts are intentionally not
/// treated as portable runtime capabilities after allocation.
#[allow(clippy::too_many_arguments)]
pub(crate) fn reopen_failure_market_interval_accounts_v2<'a>(
    program_id: &Pubkey,
    cell_account: &AccountInfo<'a>,
    history_account: &AccountInfo<'a>,
    admission: AuthenticatedFailureMarketRootV2,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    funding_preimage: FailureMarketIntervalFundingPreimageV2,
    cell_writable: bool,
    history_writable: bool,
) -> Outcome<AuthenticatedFailureMarketIntervalAccountsV2> {
    authenticate_account_metadata(
        program_id,
        history_account,
        FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES,
        history_writable,
    )?;
    let history_data = history_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let history_input: &[u8; FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES] = history_data
        .as_ref()
        .try_into()
        .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
    let history_frame = FailureMarketIntervalHistoryAccountV2::decode(history_input)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let history = FailureMarketIntervalHistoryV2::decode_for_admission(
        history_frame.semantic_body(),
        admission.state(),
        quote,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(history_data);
    let work_observed_balance_lamports = history
        .work_rent_principal_lamports()
        .checked_add(funding_preimage.work_donation_floor_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    let history_observed_balance_lamports = history
        .history_rent_principal_lamports()
        .checked_add(funding_preimage.history_donation_floor_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    let policy = admission.state().binding().facts();
    let facts = FailureMarketIntervalFundingFactsV2 {
        failure_policy_binding_id: admission.state().binding().id(),
        market_instance_id: policy.market_instance_id,
        generation: policy.generation,
        work_preallocation_receipt_id: funding_preimage.work_preallocation_receipt_id,
        history_preallocation_receipt_id: funding_preimage.history_preallocation_receipt_id,
        foundation_schedule_id: funding_preimage.foundation_schedule_id,
        foundation_account_graph_id: funding_preimage.foundation_account_graph_id,
        foundation_transcript_id: funding_preimage.foundation_transcript_id,
        work_account: history.work_account(),
        history_account: history.history_account(),
        rent_refund_owner: history.rent_refund_owner(),
        neutral_sink: history.neutral_sink(),
        work_rent_principal_lamports: history.work_rent_principal_lamports(),
        history_rent_principal_lamports: history.history_rent_principal_lamports(),
        work_donation_floor_lamports: funding_preimage.work_donation_floor_lamports,
        work_observed_balance_lamports,
        history_donation_floor_lamports: funding_preimage.history_donation_floor_lamports,
        history_observed_balance_lamports,
    };
    let funding = reopen_failure_market_interval_funding_v2(
        admission.state(),
        quote,
        history,
        facts,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    authenticate_failure_market_interval_accounts_v2(
        program_id,
        cell_account,
        history_account,
        admission,
        funding,
        quote,
        cell_writable,
        history_writable,
    )
}

/// Persist only the deterministic finite-exhaustion terminal.
///
/// The resolved-payout terminal intentionally has no corresponding writer:
/// it remains unavailable until Product atomically persists Resolution V5 and
/// returns its private once-only activation receipt.
pub(crate) fn write_failure_market_interval_exhaustion_plan_v2(
    program_id: &Pubkey,
    cell_account: &AccountInfo<'_>,
    history_account: &AccountInfo<'_>,
    authenticated: AuthenticatedFailureMarketIntervalAccountsV2,
    exhaustion: FailureMarketIntervalCellExhaustionPlanV2,
) -> Outcome<AuthenticatedFailureMarketIntervalAccountsV2> {
    let receipt = exhaustion.receipt();
    let facts = receipt.facts();
    let resulting_cell = exhaustion.resulting_cell();
    require(
        receipt.failure_policy_binding_id() == authenticated.cell.failure_policy_binding_id()
            && facts.cell_before == authenticated.cell_state_id
            && facts.cell_after
                == resulting_cell
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?
            && facts.market_instance_id == authenticated.cell.market_instance_id()
            && facts.generation == authenticated.cell.generation()
            && resulting_cell.disposition() == FailureMarketIntervalCellDispositionV2::Exhausted,
        ClutchError::MismatchedState,
    )?;
    write_failure_market_interval_cell_plan_inner_v2(
        program_id,
        cell_account,
        history_account,
        authenticated,
        exhaustion.cell_plan(),
        Some(FailureMarketIntervalCellDispositionV2::Exhausted),
    )
}

/// Persist exactly one Product/Source-authorized Idle-to-Active Begin.
///
/// There is intentionally no generic nonterminal writer. The caller must hold
/// the private same-instruction authority which is later consumed by Product's
/// narrow `0xad` pin, and any refusal after this write rolls the whole SVM
/// instruction back.
pub(crate) fn write_failure_market_interval_begin_plan_v2<
    A: AuthenticatedFailureMarketIntervalBeginV2 + ?Sized,
>(
    program_id: &Pubkey,
    cell_account: &AccountInfo<'_>,
    history_account: &AccountInfo<'_>,
    admission: AuthenticatedFailureMarketRootV2,
    authenticated: AuthenticatedFailureMarketIntervalAccountsV2,
    plan: FailureMarketIntervalCellPlanV2,
    receipt: FailureMarketIntervalCellActivationReceiptV2,
    authority: &A,
) -> Outcome<AuthenticatedFailureMarketIntervalAccountsV2> {
    authority
        .authenticate_failure_market_interval_begin_v2(receipt)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let facts = receipt.facts();
    let resulting_cell = plan.resulting_cell();
    require(
        authenticated.cell.phase() == FailureMarketIntervalCellPhaseV2::Idle
            && authenticated.cell.disposition() == FailureMarketIntervalCellDispositionV2::None
            && resulting_cell.phase() == FailureMarketIntervalCellPhaseV2::Active
            && resulting_cell.disposition() == FailureMarketIntervalCellDispositionV2::None
            && facts.cell_before == authenticated.cell_state_id
            && facts.history_root == authenticated.history.history_root()
            && facts.completed_session_count == authenticated.cell.completed_session_count()
            && receipt.cell_after()
                == resulting_cell
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?,
        ClutchError::MismatchedState,
    )?;
    require(
        admission.account() == authenticated.admission_root_account
            && admission
                .state()
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?
                == authenticated.admission_state_id,
        ClutchError::MismatchedState,
    )?;
    let projected = write_failure_market_interval_cell_plan_inner_v2(
        program_id,
        cell_account,
        history_account,
        authenticated,
        plan,
        None,
    )?;
    let rebound = authenticate_failure_market_interval_accounts_v2(
        program_id,
        cell_account,
        history_account,
        admission,
        projected.funding,
        projected.quote,
        true,
        false,
    )?;
    require(rebound == projected, ClutchError::MismatchedState)?;
    Ok(rebound)
}

/// Persist exactly one liveness-paid Active-to-Active structural advance.
///
/// The caller must have already applied and hostile-reauthenticated the sole
/// Recovery-compartment debit/payment postimage. Any refusal here rolls that
/// earlier same-instruction mutation back.
pub(crate) fn write_failure_market_interval_paid_advance_v2<
    A: AuthenticatedFailureMarketIntervalPaidAdvanceV2 + ?Sized,
>(
    program_id: &Pubkey,
    cell_account: &AccountInfo<'_>,
    history_account: &AccountInfo<'_>,
    admission: AuthenticatedFailureMarketRootV2,
    authenticated: AuthenticatedFailureMarketIntervalAccountsV2,
    advance: FailureMarketIntervalCellAdvancePlanV2,
    authority: &A,
) -> Outcome<AuthenticatedFailureMarketIntervalAccountsV2> {
    let receipt = advance.receipt();
    authority
        .authenticate_failure_market_interval_paid_advance_v2(receipt)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let facts = receipt.facts();
    let resulting_cell = advance.resulting_cell();
    require(
        admission.account() == authenticated.admission_root_account
            && admission
                .state()
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?
                == authenticated.admission_state_id
            && authenticated.cell.phase() == FailureMarketIntervalCellPhaseV2::Active
            && authenticated.cell.disposition() == FailureMarketIntervalCellDispositionV2::None
            && resulting_cell.phase() == FailureMarketIntervalCellPhaseV2::Active
            && resulting_cell.disposition() == FailureMarketIntervalCellDispositionV2::None
            && facts.cell_before == authenticated.cell_state_id
            && facts.history_state == authenticated.history_state_id
            && facts.cell_after
                == resulting_cell
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?
            && facts.processed_coordinates != 0,
        ClutchError::MismatchedState,
    )?;
    let projected = write_failure_market_interval_cell_plan_inner_v2(
        program_id,
        cell_account,
        history_account,
        authenticated,
        advance.cell_plan(),
        None,
    )?;
    let rebound = authenticate_failure_market_interval_accounts_v2(
        program_id,
        cell_account,
        history_account,
        admission,
        projected.funding,
        projected.quote,
        true,
        false,
    )?;
    require(rebound == projected, ClutchError::MismatchedState)?;
    Ok(rebound)
}

/// Persist the Resolved cell only after Product's same-call slot10/0xaa
/// postwrite receipt authenticates this exact private interval capability.
pub(crate) fn write_failure_market_interval_resolution_plan_v2<
    A: AuthenticatedFailureMarketProductResolutionV2 + ?Sized,
>(
    program_id: &Pubkey,
    cell_account: &AccountInfo<'_>,
    history_account: &AccountInfo<'_>,
    authenticated: AuthenticatedFailureMarketIntervalAccountsV2,
    resolution: FailureMarketIntervalCellResolutionPlanV2,
    product_activation: &A,
) -> Outcome<AuthenticatedFailureMarketIntervalAccountsV2> {
    let receipt = resolution.receipt();
    product_activation
        .authenticate_failure_market_product_resolution(receipt)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let facts = receipt.facts();
    let resulting_cell = resolution.resulting_cell();
    require(
        receipt.failure_policy_binding_id() == authenticated.cell.failure_policy_binding_id()
            && facts.cell_before == authenticated.cell_state_id
            && facts.cell_after
                == resulting_cell
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?
            && facts.market_instance_id == authenticated.cell.market_instance_id()
            && facts.generation == authenticated.cell.generation()
            && resulting_cell.disposition() == FailureMarketIntervalCellDispositionV2::Resolved,
        ClutchError::MismatchedState,
    )?;
    write_failure_market_interval_cell_plan_inner_v2(
        program_id,
        cell_account,
        history_account,
        authenticated,
        resolution.cell_plan(),
        Some(FailureMarketIntervalCellDispositionV2::Resolved),
    )
}

fn write_failure_market_interval_cell_plan_inner_v2(
    program_id: &Pubkey,
    cell_account: &AccountInfo<'_>,
    history_account: &AccountInfo<'_>,
    authenticated: AuthenticatedFailureMarketIntervalAccountsV2,
    plan: FailureMarketIntervalCellPlanV2,
    admitted_terminal: Option<FailureMarketIntervalCellDispositionV2>,
) -> Outcome<AuthenticatedFailureMarketIntervalAccountsV2> {
    authenticate_unchanged_account_prestate(
        program_id,
        history_account,
        authenticated.history_account,
        authenticated.history_data_id,
        authenticated.history_observed_lamports,
        FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES,
        false,
    )?;
    let mut next = authenticated.cell;
    next.commit_plan(plan)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let admitted = match admitted_terminal {
        None => matches!(
            (authenticated.cell.phase(), next.phase()),
            (
                FailureMarketIntervalCellPhaseV2::Idle,
                FailureMarketIntervalCellPhaseV2::Active
            ) | (
                FailureMarketIntervalCellPhaseV2::Active,
                FailureMarketIntervalCellPhaseV2::Active
            )
        ),
        Some(disposition) => {
            authenticated.cell.phase() == FailureMarketIntervalCellPhaseV2::Active
                && next.phase() == FailureMarketIntervalCellPhaseV2::Resolved
                && next.disposition() == disposition
        }
    };
    require(admitted, ClutchError::MismatchedState)?;
    let encoded = encode_cell(authenticated.cell_bump, next)?;
    authenticate_write_prestate(
        program_id,
        cell_account,
        authenticated.cell_account,
        authenticated.cell_data_id,
        authenticated.cell_observed_lamports,
        &encoded,
    )?;
    let mut data = cell_account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    data.copy_from_slice(&encoded);
    drop(data);
    let cell_data_id = framed_data_id(&encoded);
    let cell_state_id = next
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let cell_authentication_id = account_authentication_id(
        CELL_AUTHENTICATION_DOMAIN_V2,
        cell_account,
        cell_data_id,
        cell_state_id.bytes(),
        authenticated.admission_state_id.bytes(),
    );
    require_live_data_id(cell_authentication_id)?;
    Ok(AuthenticatedFailureMarketIntervalAccountsV2 {
        cell: next,
        cell_state_id,
        cell_data_id,
        cell_authentication_id,
        ..authenticated
    })
}

/// Atomically fold one exact terminal into `0xac/v2` and reset `0xab/v2` to
/// canonical Idle. Both complete postimages are derived before either borrow
/// is mutated, and the append/reset receipts are cross-checked explicitly.
#[allow(clippy::too_many_arguments)]
fn write_failure_market_interval_archive_v2<'a>(
    program_id: &Pubkey,
    cell_account: &AccountInfo<'a>,
    history_account: &AccountInfo<'a>,
    authenticated: AuthenticatedFailureMarketIntervalAccountsV2,
    history_plan: FailureMarketIntervalHistoryPlanV2,
    append: FailureMarketIntervalHistoryAppendReceiptV2,
    cell_plan: FailureMarketIntervalCellPlanV2,
    reset: FailureMarketIntervalCellResetReceiptV2,
    resolution_link_preauthorization_id: ProductContentId,
) -> Outcome<FailureMarketIntervalArchivePostwriteV2> {
    require_live_data_id(resolution_link_preauthorization_id)?;
    let source_occurrence_id = authenticated
        .cell
        .product_work()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?
        .source_occurrence_id();
    let mut next_history = authenticated.history;
    next_history
        .commit_plan(history_plan)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut next_cell = authenticated.cell;
    next_cell
        .commit_plan(cell_plan)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let next_history_id = next_history
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let next_cell_id = next_cell
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    require(
        append.history_before() == authenticated.history_state_id
            && append.history_after() == next_history_id
            && reset.terminal_cell() == authenticated.cell_state_id
            && reset.idle_cell() == next_cell_id
            && reset.append_receipt_id() == append.id(),
        ClutchError::MismatchedState,
    )?;
    let encoded_history = encode_history(authenticated.history_bump, next_history)?;
    let encoded_cell = encode_cell(authenticated.cell_bump, next_cell)?;
    authenticate_write_prestate(
        program_id,
        history_account,
        authenticated.history_account,
        authenticated.history_data_id,
        authenticated.history_observed_lamports,
        &encoded_history,
    )?;
    authenticate_write_prestate(
        program_id,
        cell_account,
        authenticated.cell_account,
        authenticated.cell_data_id,
        authenticated.cell_observed_lamports,
        &encoded_cell,
    )?;
    let mut history_data = history_account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let mut cell_data = cell_account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    history_data.copy_from_slice(&encoded_history);
    cell_data.copy_from_slice(&encoded_cell);
    drop(cell_data);
    drop(history_data);
    let cell_data_id = framed_data_id(&encoded_cell);
    let history_data_id = framed_data_id(&encoded_history);
    let cell_authentication_id = account_authentication_id(
        CELL_AUTHENTICATION_DOMAIN_V2,
        cell_account,
        cell_data_id,
        next_cell_id.bytes(),
        authenticated.admission_state_id.bytes(),
    );
    let history_authentication_id = account_authentication_id(
        HISTORY_AUTHENTICATION_DOMAIN_V2,
        history_account,
        history_data_id,
        next_history_id.bytes(),
        authenticated.admission_state_id.bytes(),
    );
    require_live_data_id(cell_authentication_id)?;
    require_live_data_id(history_authentication_id)?;
    let accounts = AuthenticatedFailureMarketIntervalAccountsV2 {
        cell: next_cell,
        cell_state_id: next_cell_id,
        cell_data_id,
        cell_authentication_id,
        history: next_history,
        history_state_id: next_history_id,
        history_data_id,
        history_authentication_id,
        ..authenticated
    };
    let id = ProductContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            ARCHIVE_POSTWRITE_DOMAIN_V2,
            cell_account.key.as_ref(),
            history_account.key.as_ref(),
            &authenticated.cell_authentication_id.bytes(),
            &accounts.cell_authentication_id.bytes(),
            &authenticated.history_authentication_id.bytes(),
            &accounts.history_authentication_id.bytes(),
            &append.id().bytes(),
            &reset.id().bytes(),
            &append.session_binding_id().bytes(),
            &append.session_terminal_receipt_id().bytes(),
            &append.terminal_state_commitment().bytes(),
            &append.idle_state_commitment().bytes(),
            &append.previous_root().bytes(),
            &append.resulting_root().bytes(),
            &append.completed_session_count().to_le_bytes(),
            &append.market_instance_id().bytes(),
            &append.generation().to_le_bytes(),
            &source_occurrence_id.bytes(),
            &resolution_link_preauthorization_id.bytes(),
        ])
        .to_bytes(),
    );
    require_live_data_id(id)?;
    Ok(FailureMarketIntervalArchivePostwriteV2 {
        id,
        accounts,
        append,
        reset,
        source_occurrence_id,
        resolution_link_preauthorization_id,
    })
}

/// Atomically archive one exact terminal Failure session and release its
/// initiating Product link pin.
///
/// This is the only crate-visible archive mutation entry point. The private
/// paired `0xac` append/`0xab` Idle reset is the sole authority accepted by
/// Product's narrow `0xad` release writer. A refusal while stale-reopening or
/// writing the link therefore rolls the complete instruction back, including
/// both Failure postwrites.
#[allow(clippy::too_many_arguments)]
pub(crate) fn archive_failure_market_interval_session_v2<'a, 'link>(
    program_id: &Pubkey,
    admission_root_account: &AccountInfo<'a>,
    runtime_root_account: &AccountInfo<'a>,
    cell_account: &AccountInfo<'a>,
    history_account: &AccountInfo<'a>,
    series_link_account: &AccountInfo<'a>,
    interval_before: AuthenticatedFailureMarketIntervalAccountsV2,
    link_before: AuthenticatedSeriesMarketLinkV1<'link>,
    resolution_link: AuthenticatedWritableFailureResolutionLinkV1,
    admission: AuthenticatedFailureMarketRootV2,
    runtime_before: AuthenticatedFailureMarketRuntimeRootV1,
    history_plan: FailureMarketIntervalHistoryPlanV2,
    append: FailureMarketIntervalHistoryAppendReceiptV2,
    cell_plan: FailureMarketIntervalCellPlanV2,
    reset: FailureMarketIntervalCellResetReceiptV2,
    link_rebound_output: &mut SeriesMarketLinkAccountV1,
) -> Outcome<(
    FailureMarketIntervalArchivePostwriteV2,
    AuthenticatedSeriesFailureSessionReleaseV1,
    AuthenticatedFailureMarketRuntimeSessionPostwriteV1,
)> {
    require_distinct(&[
        admission_root_account.clone(),
        runtime_root_account.clone(),
        cell_account.clone(),
        history_account.clone(),
        series_link_account.clone(),
    ])?;
    let expected_close = FailureMarketSessionCloseFactsV1 {
        runtime_before: runtime_before.state_commitment(),
        series_link_before: link_before
            .state()
            .semantic_id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        series_link_after: link_before
            .state()
            .release_failure_session(append.session_terminal_receipt_id())
            .and_then(|link| link.semantic_id())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        session_before: ProductContentId::from_bytes(append.terminal_state_commitment().bytes()),
        session_after: ProductContentId::from_bytes(append.idle_state_commitment().bytes()),
        interval_terminal_receipt_id: append.session_terminal_receipt_id(),
        previous_session_history: append.previous_root(),
        resulting_session_history: append.resulting_root(),
        history_append_receipt_id: append.id(),
        history_before: append.history_before(),
        history_after: append.history_after(),
        completed_session_count: append.completed_session_count(),
        transition_receipt_id: clutch_failure_policy_runtime::market_runtime_v1::FailureMarketSessionTransitionReceiptIdV1::from_bytes([0; 32]),
    };
    let runtime_authority = FailureMarketRuntimeArchiveAuthorityV1 {
        expected: expected_close,
    };
    let runtime_plan = plan_close_failure_market_session_v1(
        &runtime_authority,
        runtime_before.state(),
        admission.state(),
        *link_before.state(),
        append,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let archive = write_failure_market_interval_archive_v2(
        program_id,
        cell_account,
        history_account,
        interval_before,
        history_plan,
        append,
        cell_plan,
        reset,
        resolution_link.id(),
    )?;
    let release = release_series_market_link_failure_v1(
        program_id,
        series_link_account,
        link_before,
        resolution_link,
        &archive,
        link_rebound_output,
    )?;
    let runtime_link_after = runtime_plan
        .series_link_after()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        release.link_semantic_before().bytes()
            == runtime_plan
                .series_link_before()
                .semantic_id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .bytes()
            && release.link_semantic_after().bytes() == runtime_link_after.bytes()
            && release.archive_postwrite_id() == archive.id()
            && release.append_receipt_id().bytes() == archive.append().id().bytes()
            && release.reset_receipt_id().bytes() == archive.reset().id().bytes()
            && release.session_terminal_receipt_id()
                == archive.append().session_terminal_receipt_id()
            && release.resolution_link_preauthorization_id()
                == archive.resolution_link_preauthorization_id(),
        ClutchError::MismatchedState,
    )?;
    let runtime_write_facts = FailureMarketRuntimeSessionWriteFactsV1 {
        runtime_before: runtime_before.state_commitment(),
        runtime_after: runtime_plan
            .resulting_runtime()
            .commitment()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        transition_receipt_id: runtime_plan.receipt_id(),
    };
    let runtime_write_authority = FailureMarketRuntimeArchiveWriteV1 {
        expected: runtime_write_facts,
        archive_idle_state: ProductContentId::from_bytes(
            archive.accounts().cell_state_id().bytes(),
        ),
        runtime_idle_state: runtime_plan.resulting_runtime().session_state_commitment(),
        release_link_after: release.link_semantic_after(),
        runtime_link_after: ProductContentId::from_bytes(runtime_link_after.bytes()),
        release_terminal_receipt_id: release.session_terminal_receipt_id(),
        runtime_terminal_receipt_id: runtime_plan
            .resulting_runtime()
            .interval_terminal_receipt_id(),
    };
    let runtime_postwrite = write_failure_market_runtime_session_plan_v1(
        program_id,
        admission_root_account,
        runtime_root_account,
        admission,
        runtime_before,
        runtime_plan,
        &runtime_write_authority,
    )?;
    require(
        runtime_postwrite.transition_receipt_id() == runtime_write_facts.transition_receipt_id
            && runtime_postwrite.root().state().completed_session_count()
                == archive.append().completed_session_count()
            && runtime_postwrite
                .root()
                .state()
                .session_history_commitment()
                == archive.append().resulting_root(),
        ClutchError::MismatchedState,
    )?;
    Ok((archive, release, runtime_postwrite))
}

/// Persist the exhaustive family seal. Session appends cannot use this
/// history-only writer and must use the paired archive writer above.
pub(crate) fn write_failure_market_interval_family_seal_v2(
    program_id: &Pubkey,
    cell_account: &AccountInfo<'_>,
    history_account: &AccountInfo<'_>,
    authenticated: AuthenticatedFailureMarketIntervalAccountsV2,
    plan: FailureMarketIntervalHistoryPlanV2,
    seal: FailureMarketIntervalFamilySealReceiptV2,
) -> Outcome<AuthenticatedFailureMarketIntervalAccountsV2> {
    authenticate_unchanged_account_prestate(
        program_id,
        cell_account,
        authenticated.cell_account,
        authenticated.cell_data_id,
        authenticated.cell_observed_lamports,
        FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES,
        false,
    )?;
    require(
        authenticated.cell.phase() == FailureMarketIntervalCellPhaseV2::Idle,
        ClutchError::MismatchedState,
    )?;
    let mut next = authenticated.history;
    next.commit_plan(plan)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let encoded = encode_history(authenticated.history_bump, next)?;
    let history_state_id = next
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    require(
        seal.facts().history_before == authenticated.history_state_id
            && seal.history_after() == history_state_id
            && next.family_terminal_receipt_id() == seal.facts().family_terminal_receipt_id,
        ClutchError::MismatchedState,
    )?;
    authenticate_write_prestate(
        program_id,
        history_account,
        authenticated.history_account,
        authenticated.history_data_id,
        authenticated.history_observed_lamports,
        &encoded,
    )?;
    let mut data = history_account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    data.copy_from_slice(&encoded);
    drop(data);
    let history_data_id = framed_data_id(&encoded);
    let history_authentication_id = account_authentication_id(
        HISTORY_AUTHENTICATION_DOMAIN_V2,
        history_account,
        history_data_id,
        history_state_id.bytes(),
        authenticated.admission_state_id.bytes(),
    );
    require_live_data_id(history_authentication_id)?;
    Ok(AuthenticatedFailureMarketIntervalAccountsV2 {
        history: next,
        history_state_id,
        history_data_id,
        history_authentication_id,
        ..authenticated
    })
}

/// Close the reusable cell first and append-only history second, only after
/// the exact sealed Failure-family receipt has been consumed by Product's
/// authenticated whole-Market terminal root.
///
/// Both accounts remain readable until every semantic and physical prestate
/// check succeeds. The two accounts and both recipients are then mutated in
/// one outer instruction; any later refusal rolls the entire batch back.
#[allow(clippy::too_many_arguments)]
pub(crate) fn close_failure_market_interval_accounts_v2<'a>(
    program_id: &Pubkey,
    admission_root_account: &AccountInfo<'a>,
    market_root_account: &AccountInfo<'a>,
    cell_account: &AccountInfo<'a>,
    history_account: &AccountInfo<'a>,
    rent_refund_owner: &AccountInfo<'a>,
    neutral_sink: &AccountInfo<'a>,
    authenticated: AuthenticatedFailureMarketIntervalAccountsV2,
    seal: FailureMarketIntervalFamilySealReceiptV2,
    market_terminal: AuthenticatedMarketInstanceTerminalV1,
    market_root_output: &mut MarketLifecycleRootAccountV1,
) -> Outcome<AuthenticatedFailureMarketIntervalCloseV2> {
    require(
        !admission_root_account.is_writable && !market_root_account.is_writable,
        ClutchError::UnexpectedWritable,
    )?;
    let live_admission =
        authenticate_failure_market_root_v2(program_id, admission_root_account, false)?;
    let live_market_terminal = authenticate_market_instance_terminal_v1(
        program_id,
        market_root_account,
        authenticated.cell.market_instance_id(),
        authenticated.cell.generation(),
        market_root_output,
    )?;
    require(
        live_admission.account() == authenticated.admission_root_account
            && live_admission
                .state()
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?
                == authenticated.admission_state_id
            && live_market_terminal == market_terminal
            && market_terminal.root_account() == *market_root_account.key
            && authenticated.cell.phase() == FailureMarketIntervalCellPhaseV2::Idle
            && market_terminal.owner_program() == *program_id
            && market_terminal.market_instance_id() == authenticated.cell.market_instance_id()
            && market_terminal.generation() == authenticated.cell.generation()
            && market_terminal.failure_terminal_receipt_id().bytes()
                == seal.facts().family_terminal_receipt_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    require(
        *admission_root_account.key != *market_root_account.key
            && *admission_root_account.key != *cell_account.key
            && *admission_root_account.key != *history_account.key
            && *admission_root_account.key != *rent_refund_owner.key
            && *admission_root_account.key != *neutral_sink.key
            && *market_root_account.key != *cell_account.key
            && *market_root_account.key != *history_account.key
            && *market_root_account.key != *rent_refund_owner.key
            && *market_root_account.key != *neutral_sink.key
            && *cell_account.key != *history_account.key
            && *cell_account.key != *rent_refund_owner.key
            && *cell_account.key != *neutral_sink.key
            && *history_account.key != *rent_refund_owner.key
            && *history_account.key != *neutral_sink.key
            && *rent_refund_owner.key != *neutral_sink.key,
        ClutchError::AccountAlias,
    )?;
    for recipient in [rent_refund_owner, neutral_sink] {
        require(recipient.is_writable, ClutchError::NotWritable)?;
        require(!recipient.is_signer, ClutchError::NonCanonical)?;
        require(!recipient.executable, ClutchError::ExecutableAccount)?;
        require(
            recipient.owner.to_bytes() == SYSTEM_PROGRAM_ID && recipient.data_len() == 0,
            ClutchError::WrongProgramOwner,
        )?;
    }
    authenticate_close_account_prestate(
        program_id,
        cell_account,
        authenticated.cell_account,
        authenticated.cell_data_id,
        authenticated.cell_observed_lamports,
    )?;
    authenticate_close_account_prestate(
        program_id,
        history_account,
        authenticated.history_account,
        authenticated.history_data_id,
        authenticated.history_observed_lamports,
    )?;
    let plan = plan_close_failure_market_interval_accounts_v2(
        authenticated.history,
        seal,
        cell_account.lamports(),
        history_account.lamports(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        plan.work_account.bytes() == cell_account.key.to_bytes()
            && plan.history_account.bytes() == history_account.key.to_bytes()
            && plan.rent_refund_owner.bytes() == rent_refund_owner.key.to_bytes()
            && plan.neutral_sink.bytes() == neutral_sink.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let refunded_principal_lamports = plan
        .work_rent_refund_lamports
        .checked_add(plan.history_rent_refund_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let neutralized_donation_lamports = plan
        .work_donation_lamports
        .checked_add(plan.history_donation_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let closed_account_lamports = cell_account
        .lamports()
        .checked_add(history_account.lamports())
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        refunded_principal_lamports
            .checked_add(neutralized_donation_lamports)
            .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?
            == closed_account_lamports,
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
        let mut cell_lamports = cell_account
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut history_lamports = history_account
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
        **refund_lamports = refund_after;
        **sink_lamports = sink_after;
    }
    cell_account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    cell_account.assign(&SYSTEM_PROGRAM_ID);
    history_account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    history_account.assign(&SYSTEM_PROGRAM_ID);
    let id = ProductContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            CLOSE_AUTHENTICATION_DOMAIN_V2,
            &plan.authorization_id.bytes(),
            &market_terminal.id().bytes(),
            cell_account.key.as_ref(),
            history_account.key.as_ref(),
            rent_refund_owner.key.as_ref(),
            neutral_sink.key.as_ref(),
            &refunded_principal_lamports.to_le_bytes(),
            &neutralized_donation_lamports.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require_live_data_id(id)?;
    Ok(AuthenticatedFailureMarketIntervalCloseV2 {
        id,
        close_authorization_id: plan.authorization_id,
        market_terminal_authentication_id: market_terminal.id(),
        cell_account: *cell_account.key,
        history_account: *history_account.key,
        rent_refund_owner: *rent_refund_owner.key,
        neutral_sink: *neutral_sink.key,
        refunded_principal_lamports,
        neutralized_donation_lamports,
    })
}

fn authenticate_account_metadata(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_len: usize,
    writable: bool,
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.is_signer, ClutchError::NonCanonical)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(
        account.is_writable == writable,
        if writable {
            ClutchError::NotWritable
        } else {
            ClutchError::UnexpectedWritable
        },
    )?;
    require(
        account.data_len() == expected_len,
        ClutchError::WrongDataLength,
    )
}

#[allow(clippy::too_many_arguments)]
fn allocate_assign_interval_account_v2<'a>(
    program_id: &Pubkey,
    account: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    seed_domain: &[u8],
    market_instance_id: [u8; 32],
    generation: u64,
    bump: u8,
    account_len: usize,
) -> Outcome<()> {
    let generation_bytes = generation.to_le_bytes();
    let bump_seed = [bump];
    let signer_seeds: [&[u8]; 4] = [
        seed_domain,
        &market_instance_id,
        &generation_bytes,
        &bump_seed,
    ];
    let observed_lamports = account.lamports();
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(account_len),
        vec![AccountMeta::new(*account.key, true)],
    );
    invoke_signed(
        &allocate,
        &[account.clone(), system_program.clone()],
        &[&signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*account.key, true)],
    );
    invoke_signed(
        &assign,
        &[account.clone(), system_program.clone()],
        &[&signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        account.owner == program_id
            && account.data_len() == account_len
            && account.lamports() == observed_lamports,
        ClutchError::AccountCreationFailed,
    )
}

fn authenticate_write_prestate<const N: usize>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_account: Pubkey,
    expected_data_id: ProductContentId,
    expected_lamports: u64,
    output: &[u8; N],
) -> Outcome<()> {
    require(
        account.owner == program_id
            && account.is_writable
            && !account.is_signer
            && !account.executable
            && *account.key == expected_account
            && account.data_len() == N
            && account.lamports() == expected_lamports
            && framed_data_id(
                account
                    .try_borrow_data()
                    .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
                    .as_ref(),
            ) == expected_data_id
            && output.iter().any(|byte| *byte != 0),
        ClutchError::MismatchedState,
    )
}

fn authenticate_close_account_prestate(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_account: Pubkey,
    expected_data_id: ProductContentId,
    expected_lamports: u64,
) -> Outcome<()> {
    require(
        account.owner == program_id
            && account.is_writable
            && !account.is_signer
            && !account.executable
            && *account.key == expected_account
            && account.lamports() == expected_lamports
            && framed_data_id(
                account
                    .try_borrow_data()
                    .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
                    .as_ref(),
            ) == expected_data_id,
        ClutchError::MismatchedState,
    )
}

#[allow(clippy::too_many_arguments)]
fn authenticate_unchanged_account_prestate(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_account: Pubkey,
    expected_data_id: ProductContentId,
    expected_lamports: u64,
    expected_len: usize,
    writable: bool,
) -> Outcome<()> {
    authenticate_account_metadata(program_id, account, expected_len, writable)?;
    require(
        *account.key == expected_account
            && account.lamports() == expected_lamports
            && framed_data_id(
                account
                    .try_borrow_data()
                    .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
                    .as_ref(),
            ) == expected_data_id,
        ClutchError::MismatchedState,
    )
}

fn encode_cell(
    bump: u8,
    value: FailureMarketIntervalCellV2,
) -> Outcome<[u8; FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES]> {
    let mut semantic = [0; FAILURE_MARKET_INTERVAL_CELL_BYTES_V2];
    value
        .encode_into(&mut semantic)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let frame = FailureMarketIntervalCellAccountV2::new(bump, semantic)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let mut output = [0; FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES];
    frame
        .encode_into(&mut output)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    Ok(output)
}

fn encode_history(
    bump: u8,
    value: FailureMarketIntervalHistoryV2,
) -> Outcome<[u8; FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES]> {
    let mut semantic = [0; FAILURE_MARKET_INTERVAL_HISTORY_BYTES_V2];
    value
        .encode_into(&mut semantic)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let frame = FailureMarketIntervalHistoryAccountV2::new(bump, semantic)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let mut output = [0; FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES];
    frame
        .encode_into(&mut output)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    Ok(output)
}

fn account_authentication_id(
    domain: &[u8],
    account: &AccountInfo<'_>,
    data_id: ProductContentId,
    state_id: [u8; 32],
    admission_state_id: [u8; 32],
) -> ProductContentId {
    ProductContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            domain,
            account.key.as_ref(),
            account.owner.as_ref(),
            &data_id.bytes(),
            &state_id,
            &admission_state_id,
            &account.lamports().to_le_bytes(),
        ])
        .to_bytes(),
    )
}

fn framed_data_id(data: &[u8]) -> ProductContentId {
    ProductContentId::from_bytes(solana_sha256_hasher::hashv(&[data]).to_bytes())
}

fn require_live_data_id(id: ProductContentId) -> Outcome<()> {
    require(!id.is_zero(), ClutchError::MismatchedState)
}

#[cfg(test)]
mod adversarial_account_tests {
    use super::*;

    struct Cell {
        key: Pubkey,
        owner: Pubkey,
        lamports: u64,
        data: Vec<u8>,
    }

    impl Cell {
        fn info(&mut self) -> AccountInfo<'_> {
            AccountInfo::new(
                &self.key,
                false,
                true,
                &mut self.lamports,
                &mut self.data,
                &self.owner,
                false,
            )
        }
    }

    #[test]
    fn stale_cell_and_history_snapshots_refuse_before_write() {
        let program_id = Pubkey::new_from_array([1; 32]);
        let key = Pubkey::new_from_array([2; 32]);
        let output = [9_u8; 8];
        for _role in ["cell", "history"] {
            let mut cell = Cell {
                key,
                owner: program_id,
                lamports: 10,
                data: vec![3; 8],
            };
            let expected_data_id = framed_data_id(&cell.data);
            assert!(authenticate_write_prestate(
                &program_id,
                &cell.info(),
                key,
                expected_data_id,
                10,
                &output,
            )
            .is_ok());
            assert!(authenticate_write_prestate(
                &program_id,
                &cell.info(),
                Pubkey::new_from_array([4; 32]),
                expected_data_id,
                10,
                &output,
            )
            .is_err());
            assert!(authenticate_write_prestate(
                &program_id,
                &cell.info(),
                key,
                ProductContentId::from_bytes([5; 32]),
                10,
                &output,
            )
            .is_err());
            assert!(authenticate_write_prestate(
                &program_id,
                &cell.info(),
                key,
                expected_data_id,
                11,
                &output,
            )
            .is_err());
            cell.data[0] ^= 1;
            assert!(authenticate_write_prestate(
                &program_id,
                &cell.info(),
                key,
                expected_data_id,
                10,
                &output,
            )
            .is_err());
        }
    }

    #[test]
    fn archive_orders_append_reset_link_release_and_runtime_transcript_atomically() {
        let source = include_str!("failure_market_interval_v2.rs");
        let outer = source
            .find("pub(crate) fn archive_failure_market_interval_session_v2")
            .unwrap();
        let archive = source[outer..]
            .find("let archive = write_failure_market_interval_archive_v2")
            .unwrap();
        let release = source[outer..]
            .find("let release = release_series_market_link_failure_v1")
            .unwrap();
        let runtime = source[outer..]
            .find("let runtime_postwrite = write_failure_market_runtime_session_plan_v1")
            .unwrap();
        let success = source[outer..]
            .find("Ok((archive, release, runtime_postwrite))")
            .unwrap();
        assert!(archive < release && release < runtime && runtime < success);
        let archive_writer = source
            .split("fn write_failure_market_interval_archive_v2")
            .nth(1)
            .and_then(|value| value.split("/// Atomically archive one exact terminal").next())
            .expect("private paired archive writer");
        assert!(archive_writer.contains("resolution_link_preauthorization_id"));
        assert!(source[outer..].contains("resolution_link.id()"));
        assert!(source[outer..]
            .contains("release.resolution_link_preauthorization_id()"));
        assert_eq!(
            source
                .matches("pub(crate) fn archive_failure_market_interval_session_v2")
                .count(),
            1
        );
    }

    #[test]
    fn funding_preimage_refuses_truncation_zero_ids_and_receipt_aliases() {
        let mut input = [1_u8; FAILURE_MARKET_INTERVAL_FUNDING_PREIMAGE_BYTES_V2];
        assert!(FailureMarketIntervalFundingPreimageV2::decode(&input).is_err());
        input[32..64].fill(2);
        assert!(FailureMarketIntervalFundingPreimageV2::decode(&input).is_ok());
        assert!(FailureMarketIntervalFundingPreimageV2::decode(&input[..175]).is_err());
        input[..32].fill(0);
        assert!(FailureMarketIntervalFundingPreimageV2::decode(&input).is_err());
    }

    #[test]
    fn quote_and_funding_reopen_are_content_joined_not_cached_authority() {
        let source = include_str!("failure_market_interval_v2.rs");
        let quote = source
            .split("fn authenticate_failure_market_recovery_quote_v1")
            .nth(1)
            .expect("quote owner");
        for predicate in [
            "recovery_quote_schedule_id().bytes() == schedule_id.bytes()",
            "maximum_progress_units_per_call()",
            "work_capital_lamports()",
            "capability_profile_id().bytes()",
        ] {
            assert!(quote.contains(predicate));
        }
        let reopen = source
            .split("fn reopen_failure_market_interval_accounts_v2")
            .nth(1)
            .expect("funding reopen");
        assert!(reopen.contains("reopen_failure_market_interval_funding_v2"));
        assert!(reopen.contains("work_donation_floor_lamports"));
        assert!(reopen.contains("history_donation_floor_lamports"));
    }

    #[test]
    fn exhaustion_is_derived_from_read_only_single_custody() {
        let source = include_str!("failure_market_interval_v2.rs");
        let exhaustion = source
            .split("fn plan_failure_market_interval_exhaustion_v2")
            .nth(1)
            .expect("exhaustion owner");
        for predicate in [
            "!recovery_account.is_writable",
            "recovery.completed_work_ceiling_lamports == recovery.keeper_paid_lamports",
            "completed_calls: u64::from(recovery.completed_calls)",
            "remaining_work_lamports: recovery.remaining_work_lamports",
            "plan_exhaust_failure_market_interval_cell_v2",
        ] {
            assert!(exhaustion.contains(predicate));
        }
    }
}
