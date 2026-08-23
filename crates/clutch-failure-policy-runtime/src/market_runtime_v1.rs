// SPDX-License-Identifier: AGPL-3.0-or-later
//! Market-scoped Failure runtime successor.
//!
//! The legacy external runtime binds one Series and ordinal into its identity.
//! This successor instead derives its identity only from the immutable shared
//! Market policy. Series links, Source occurrences, and absolute recovery
//! schedules are subordinate session inputs and never alter the runtime account
//! identity. Per-occurrence recovery state lives in the dedicated session/work
//! owner; this root stores only the shared liveness-capital identity and one
//! active-session pin.

use clutch_product_series::{
    ContentId as ProductContentId, MarketInstanceV2Id, SeriesMarketLinkV1, SeriesMarketLinkV1Id,
    SeriesPlanV5Id, SourceOccurrenceV1Id,
};
use sha2::{Digest, Sha256};

use crate::market_interval_cell_v2::{
    FailureMarketIntervalCellPhaseV2, FailureMarketIntervalCellStateIdV2,
    FailureMarketIntervalCellV2,
};
use crate::market_interval_history_v2::{
    FailureMarketIntervalFundingReceiptV2, FailureMarketIntervalHistoryAppendReceiptV2,
    FailureMarketIntervalHistoryRootV2, FailureMarketIntervalHistoryStateIdV2,
    FailureMarketIntervalHistoryV2,
};
use crate::market_policy_v1::{
    FailureMarketAccountIdV1, FailureMarketAdmissionStateIdV1, FailureMarketAdmissionStateV1,
    FailureMarketRecoveryFundingReceiptIdV1,
};
use crate::market_quote_v1::FailureMarketRecoveryQuoteAdmissionReceiptV1;
use crate::market_recovery_terminal_v2::{
    FailureMarketClosedRecoveryJoinIdV2, FailureMarketRecoveryTerminalReceiptIdV2,
    FailureMarketRecoveryTerminalReceiptV2,
};
use crate::market_replay_v2::FailureMarketReplayTerminalReceiptV2;
use crate::{Error, FailurePolicyBindingId, Result};

const RUNTIME_ADMISSION_DOMAIN_V1: &[u8] = b"dragons-clutch/failure-market-runtime-admission/v1";
const RUNTIME_COMMITMENT_DOMAIN_V1: &[u8] = b"dragons-clutch/failure-market-runtime-commitment/v1";
const SESSION_BEGIN_DOMAIN_V1: &[u8] = b"dragons-clutch/failure-market-session-begin/v1";
const SESSION_ADVANCE_DOMAIN_V1: &[u8] = b"dragons-clutch/failure-market-session-advance/v1";
const SESSION_RESOLVE_DOMAIN_V1: &[u8] = b"dragons-clutch/failure-market-session-resolve/v1";
const SESSION_CLOSE_DOMAIN_V1: &[u8] = b"dragons-clutch/failure-market-session-close/v1";
const RECOVERY_CLOSE_DOMAIN_V2: &[u8] = b"dragons-clutch/failure-market-recovery-close/v2";
const FAMILY_TERMINAL_DOMAIN_V2: &[u8] = b"dragons-clutch/failure-market-family-terminal/v2";
const MAGIC_V1: [u8; 8] = *b"DCFMRUN1";
const VERSION_V1: u16 = 1;
const HEADER_BYTES_V1: usize = 16;
const ID_BYTES_V1: usize = 32;
const PREFIX_ID_COUNT_V1: usize = 5;
const ROOT_FUNDING_ID_COUNT_V1: usize = 2;
const ROOT_FUNDING_AMOUNT_COUNT_V1: usize = 3;
const PHASE_BYTES_V1: usize = 8;
const SESSION_ID_COUNT_V1: usize = 9;
const ACTIVE_SESSION_PIN_INDEX_V1: usize = 0;
const SERIES_LINK_AUTHENTICATION_INDEX_V1: usize = 1;
const SESSION_STATE_COMMITMENT_INDEX_V1: usize = 2;
const SESSION_RESOLUTION_RECEIPT_INDEX_V1: usize = 3;
const INTERVAL_TERMINAL_RECEIPT_INDEX_V1: usize = 4;
const RECOVERY_TERMINAL_RECEIPT_INDEX_V1: usize = 5;
const FAMILY_TERMINAL_RECEIPT_INDEX_V1: usize = 6;
const INTERVAL_HISTORY_ROOT_INDEX_V1: usize = 7;
const ACTIVE_INTERVAL_FUNDING_RECEIPT_INDEX_V1: usize = 8;

/// Canonical semantic body width inside the FailureRuntimeRoot account.
pub const FAILURE_MARKET_RUNTIME_BYTES_V1: usize = 2_048;

macro_rules! runtime_id {
    ($name:ident, $docs:literal) => {
        #[doc = $docs]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[repr(transparent)]
        pub struct $name([u8; 32]);

        impl $name {
            /// Construct from digest bytes without claiming authenticity.
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            /// Return exact digest bytes.
            pub const fn bytes(self) -> [u8; 32] {
                self.0
            }
        }
    };
}

runtime_id!(
    FailureMarketRuntimeAdmissionReceiptIdV1,
    "Typed identity of one authenticated Market runtime foundation."
);
runtime_id!(
    FailureMarketRuntimeStateCommitmentV1,
    "Typed commitment to one complete canonical Market runtime state."
);
runtime_id!(
    FailureMarketSessionScheduleIdV1,
    "Typed identity of one subordinate Series/ordinal recovery schedule."
);
runtime_id!(
    FailureMarketSessionTransitionReceiptIdV1,
    "Typed identity of one authenticated subordinate session transition."
);
runtime_id!(
    FailureMarketRecoveryCloseReceiptIdV2,
    "Typed identity of the exact shared Recovery close admitted by the Market runtime."
);
runtime_id!(
    FailureMarketFamilyAggregateReceiptIdV2,
    "Typed identity of the Recovery-closed runtime and exact reusable interval pair."
);
runtime_id!(
    FailureMarketFamilyTerminalReceiptIdV2,
    "Typed identity of one exhaustive reusable-session Failure-family terminal receipt."
);

/// Only successful terminal disposition currently consumable by Product.
///
/// Exhausted or refused subordinate sessions remain appendable history, but
/// they cannot terminalize the shared Market. Product must first authenticate
/// one exact Resolution V5 activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FailureMarketFamilyTerminalDispositionV2 {
    /// Product authenticated the exact final Resolution V5 postimage.
    Resolved = 1,
}

impl FailureMarketFamilyTerminalDispositionV2 {
    const fn byte(self) -> u8 {
        match self {
            Self::Resolved => 1,
        }
    }
}

/// Complete expected close of the sole shared liveness Recovery custody.
///
/// Public construction is not authority. The SBF adapter must reconstruct
/// these facts from the live runtime, exact Idle interval pair, Product's
/// private Resolution activation, and the complete liveness close poststate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketRecoveryCloseFactsV2 {
    /// Complete runtime prestate.
    pub runtime_before: FailureMarketRuntimeStateCommitmentV1,
    /// Complete immutable admission identity.
    pub admission_state_id: FailureMarketAdmissionStateIdV1,
    /// Shared Failure policy.
    pub failure_policy_binding_id: FailurePolicyBindingId,
    /// Full-width economic Market.
    pub market_instance_id: MarketInstanceV2Id,
    /// Shared Failure/liveness generation.
    pub generation: u64,
    /// Mutable Market-scoped Failure runtime account.
    pub runtime_account_id: FailureMarketAccountIdV1,
    /// Canonical Idle reusable interval-cell postimage.
    pub interval_cell_state_id: FailureMarketIntervalCellStateIdV2,
    /// Complete unsealed append-only history prestate.
    pub interval_history_state_id: FailureMarketIntervalHistoryStateIdV2,
    /// Sole append-only history root.
    pub interval_history_root: FailureMarketIntervalHistoryRootV2,
    /// Number of subordinate terminal sessions folded into history.
    pub completed_session_count: u64,
    /// Exact aggregate paid-call count.
    pub completed_work_calls: u64,
    /// Exact aggregate keeper rewards.
    pub exact_reward_lamports: u64,
    /// Latest folded subordinate terminal receipt.
    pub latest_interval_terminal_receipt_id: ProductContentId,
    /// Product-private once-only Resolution V5 activation.
    pub resolution_activation_receipt_id: ProductContentId,
    /// Failure semantic owner's exact Recovery terminal receipt.
    pub recovery_terminal_receipt_id: FailureMarketRecoveryTerminalReceiptIdV2,
    /// Liveness adapter's re-executed exact successful Recovery close.
    pub closed_recovery_join_id: FailureMarketClosedRecoveryJoinIdV2,
}

/// Private authority over Product Resolution and the full liveness close.
pub trait AuthenticatedFailureMarketRecoveryCloseV2 {
    /// Authenticate every expected fact without lowering either owner to IDs.
    fn authenticate_failure_market_recovery_close(
        &self,
        _expected: FailureMarketRecoveryCloseFactsV2,
    ) -> Result<()> {
        Err(Error::BindingMismatch)
    }
}

/// Private-field receipt for the successful shared Recovery close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketRecoveryCloseReceiptV2 {
    id: FailureMarketRecoveryCloseReceiptIdV2,
    facts: FailureMarketRecoveryCloseFactsV2,
}

impl FailureMarketRecoveryCloseReceiptV2 {
    /// Exact close receipt identity stored by the Market runtime.
    pub const fn id(self) -> FailureMarketRecoveryCloseReceiptIdV2 {
        self.id
    }

    /// Complete authenticated close facts.
    pub const fn facts(self) -> FailureMarketRecoveryCloseFactsV2 {
        self.facts
    }
}

/// Complete expected pre-replay Failure-family aggregate projection.
///
/// The append-only history is still unsealed in this prestate. The caller
/// must next seal this receipt into the permanent Failure replay. This
/// intermediate receipt breaks the otherwise recursive final-receipt/replay
/// identity dependency and is never directly consumable by Product.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketFamilyAggregateFactsV2 {
    /// Successful Market terminal classification.
    pub disposition: FailureMarketFamilyTerminalDispositionV2,
    /// Complete runtime prestate after Recovery closed.
    pub runtime_before: FailureMarketRuntimeStateCommitmentV1,
    /// Complete immutable admission identity.
    pub admission_state_id: FailureMarketAdmissionStateIdV1,
    /// Shared Failure policy.
    pub failure_policy_binding_id: FailurePolicyBindingId,
    /// Full-width economic Market.
    pub market_instance_id: MarketInstanceV2Id,
    /// Shared Failure/liveness generation.
    pub generation: u64,
    /// Immutable Failure admission root.
    pub admission_root_account_id: FailureMarketAccountIdV1,
    /// Distinct mutable Market-scoped Failure runtime root.
    pub runtime_root_account_id: FailureMarketAccountIdV1,
    /// Canonical reusable interval cell.
    pub interval_work_account_id: FailureMarketAccountIdV1,
    /// Append-only interval history account.
    pub interval_history_account_id: FailureMarketAccountIdV1,
    /// Canonical Idle cell postimage.
    pub interval_cell_state_id: FailureMarketIntervalCellStateIdV2,
    /// Complete unsealed history prestate.
    pub interval_history_state_id: FailureMarketIntervalHistoryStateIdV2,
    /// Sole aggregate history root.
    pub interval_history_root: FailureMarketIntervalHistoryRootV2,
    /// Exact number of folded terminal sessions.
    pub completed_session_count: u64,
    /// Exact aggregate paid calls.
    pub completed_work_calls: u64,
    /// Exact aggregate keeper rewards.
    pub exact_reward_lamports: u64,
    /// Fresh shared Recovery close receipt retained by the runtime.
    pub recovery_close_receipt_id: FailureMarketRecoveryCloseReceiptIdV2,
    /// Product-private once-only Resolution V5 activation.
    pub resolution_activation_receipt_id: ProductContentId,
}

/// Private authority over the exact Idle pair and Recovery-closed runtime.
pub trait AuthenticatedFailureMarketFamilyAggregateV2 {
    /// Authenticate the pre-replay aggregate without accepting caller IDs.
    fn authenticate_failure_market_family_aggregate(
        &self,
        _expected: FailureMarketFamilyAggregateFactsV2,
    ) -> Result<()> {
        Err(Error::BindingMismatch)
    }
}

/// Private-field pre-replay aggregate receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketFamilyAggregateReceiptV2 {
    id: FailureMarketFamilyAggregateReceiptIdV2,
    facts: FailureMarketFamilyAggregateFactsV2,
}

impl FailureMarketFamilyAggregateReceiptV2 {
    /// Exact aggregate identity consumed only by the permanent replay owner.
    pub const fn id(self) -> FailureMarketFamilyAggregateReceiptIdV2 {
        self.id
    }

    /// Complete authenticated aggregate facts.
    pub const fn facts(self) -> FailureMarketFamilyAggregateFactsV2 {
        self.facts
    }
}

/// Complete final Failure-family terminal projection.
///
/// This is the sole receipt Product may consume. It joins the exact aggregate
/// to the permanent replay owner's authenticated terminal postimage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketFamilyTerminalFactsV2 {
    /// Successful Market terminal classification.
    pub disposition: FailureMarketFamilyTerminalDispositionV2,
    /// Intermediate exact family aggregate.
    pub family_aggregate_receipt_id: FailureMarketFamilyAggregateReceiptIdV2,
    /// Permanent shared-Market Failure replay account.
    pub failure_replay_account_id: FailureMarketAccountIdV1,
    /// Exact terminal postimage receipt minted by the replay owner.
    pub failure_replay_terminal_receipt_id:
        crate::market_replay_v2::FailureMarketReplayTerminalReceiptIdV2,
    /// Complete runtime prestate after Recovery closed.
    pub runtime_before: FailureMarketRuntimeStateCommitmentV1,
    /// Complete immutable admission identity.
    pub admission_state_id: FailureMarketAdmissionStateIdV1,
    /// Shared Failure policy.
    pub failure_policy_binding_id: FailurePolicyBindingId,
    /// Full-width economic Market.
    pub market_instance_id: MarketInstanceV2Id,
    /// Shared Failure/liveness generation.
    pub generation: u64,
    /// Exact unsealed append-only history prestate.
    pub interval_history_state_id: FailureMarketIntervalHistoryStateIdV2,
    /// Sole append-only history root.
    pub interval_history_root: FailureMarketIntervalHistoryRootV2,
    /// Exact number of folded sessions.
    pub completed_session_count: u64,
}

/// Private authority over the same-call aggregate and permanent replay seal.
pub trait AuthenticatedFailureMarketFamilyTerminalV2 {
    /// Authenticate exhaustive terminality without accepting caller IDs.
    fn authenticate_failure_market_family_terminal(
        &self,
        _expected: FailureMarketFamilyTerminalFactsV2,
    ) -> Result<()> {
        Err(Error::BindingMismatch)
    }
}

/// Private-field exhaustive Failure-family receipt consumed by Product.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketFamilyTerminalReceiptV2 {
    id: FailureMarketFamilyTerminalReceiptIdV2,
    facts: FailureMarketFamilyTerminalFactsV2,
}

impl FailureMarketFamilyTerminalReceiptV2 {
    /// Exact receipt identity consumed by Product and sealed into history.
    pub const fn id(self) -> FailureMarketFamilyTerminalReceiptIdV2 {
        self.id
    }

    /// Complete authenticated family terminal facts.
    pub const fn facts(self) -> FailureMarketFamilyTerminalFactsV2 {
        self.facts
    }
}
/// Complete subordinate interval-session descriptor. This projection is not
/// authority and never changes the shared runtime account identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketSessionDescriptorV1 {
    /// Initiating recurring Series.
    pub series_plan_id: SeriesPlanV5Id,
    /// Exact finite Series ordinal.
    pub ordinal: u32,
    /// Product/Source-owned occurrence.
    pub source_occurrence_id: SourceOccurrenceV1Id,
    /// Per-occurrence absolute schedule identity owned by the session.
    pub schedule_id: FailureMarketSessionScheduleIdV1,
    /// Exact Product-authenticated reusable-cell/history capitalization.
    pub interval_funding_receipt_id:
        crate::market_interval_history_v2::FailureMarketIntervalFundingReceiptIdV2,
    /// Complete initial subordinate session postimage.
    pub session_state_commitment: ProductContentId,
}

/// Expected exact begin authority derived from Product link and session state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketSessionBeginFactsV1 {
    /// Shared runtime prestate.
    pub runtime_before: FailureMarketRuntimeStateCommitmentV1,
    /// Exact active Product link prestate.
    pub series_link_before: SeriesMarketLinkV1Id,
    /// Product link poststate after pinning this begin receipt.
    pub series_link_after: SeriesMarketLinkV1Id,
    /// Prior durable closed-session transcript, or zero for the first session.
    pub previous_session_history: FailureMarketIntervalHistoryRootV2,
    /// Prior interval terminal receipt, or zero for the first session.
    pub previous_interval_terminal_receipt_id: ProductContentId,
    /// Exact authenticated reusable `0xab/v2` cell.
    pub interval_work_account: FailureMarketAccountIdV1,
    /// Exact authenticated append-only `0xac/v2` history.
    pub interval_history_account: FailureMarketAccountIdV1,
    /// Complete authenticated history prestate.
    pub interval_history_state_id: FailureMarketIntervalHistoryStateIdV2,
    /// Exact number of already folded sessions.
    pub completed_session_count: u64,
    /// Noncircular Product/Failure preauthorization passed to the link pin.
    pub begin_preauthorization_id: ProductContentId,
    /// Exact Product post-pin transcript retained by the cell and runtime.
    pub session_binding_id: ProductContentId,
    /// Complete subordinate descriptor.
    pub session: FailureMarketSessionDescriptorV1,
    /// Unique shared-runtime begin transition receipt.
    pub begin_receipt_id: FailureMarketSessionTransitionReceiptIdV1,
}

/// Expected exact bounded session-state advance authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketSessionAdvanceFactsV1 {
    /// Shared runtime prestate.
    pub runtime_before: FailureMarketRuntimeStateCommitmentV1,
    /// Pinned Product link semantic state.
    pub series_link_state_id: SeriesMarketLinkV1Id,
    /// Prior subordinate session commitment.
    pub session_before: ProductContentId,
    /// Authenticated subordinate session postimage.
    pub session_after: ProductContentId,
    /// Exact liveness work receipt applied in the same atomic batch.
    pub liveness_work_receipt_id: ProductContentId,
    /// Unique transition receipt.
    pub transition_receipt_id: FailureMarketSessionTransitionReceiptIdV1,
}

/// Expected exact session resolution authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketSessionResolutionFactsV1 {
    /// Shared runtime prestate.
    pub runtime_before: FailureMarketRuntimeStateCommitmentV1,
    /// Pinned Product link semantic state.
    pub series_link_state_id: SeriesMarketLinkV1Id,
    /// Prior subordinate session commitment.
    pub session_before: ProductContentId,
    /// Authenticated resolved subordinate postimage.
    pub session_after: ProductContentId,
    /// Exact private interval resolution receipt.
    pub session_resolution_receipt_id: ProductContentId,
    /// Unique transition receipt.
    pub transition_receipt_id: FailureMarketSessionTransitionReceiptIdV1,
}

/// Expected exact session close and Product-link release authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketSessionCloseFactsV1 {
    /// Shared runtime prestate.
    pub runtime_before: FailureMarketRuntimeStateCommitmentV1,
    /// Pinned Product link prestate.
    pub series_link_before: SeriesMarketLinkV1Id,
    /// Released Product link poststate.
    pub series_link_after: SeriesMarketLinkV1Id,
    /// Prior resolved subordinate commitment.
    pub session_before: ProductContentId,
    /// Authenticated closed subordinate postimage.
    pub session_after: ProductContentId,
    /// Durable interval terminal receipt retained by `0xac`.
    pub interval_terminal_receipt_id: ProductContentId,
    /// Prior durable transcript, or zero while closing the first session.
    pub previous_session_history: FailureMarketIntervalHistoryRootV2,
    /// Resulting append-only transcript over this and every prior session.
    pub resulting_session_history: FailureMarketIntervalHistoryRootV2,
    /// Exact private append receipt consumed before reusable-cell reset.
    pub history_append_receipt_id:
        crate::market_interval_history_v2::FailureMarketIntervalHistoryAppendReceiptIdV2,
    /// Complete append-only history prestate.
    pub history_before: FailureMarketIntervalHistoryStateIdV2,
    /// Complete append-only history poststate.
    pub history_after: FailureMarketIntervalHistoryStateIdV2,
    /// Resulting one-based completed-session count.
    pub completed_session_count: u64,
    /// Unique shared-runtime transition receipt.
    pub transition_receipt_id: FailureMarketSessionTransitionReceiptIdV1,
}

/// Adapter authority for subordinate `0xab`/`0xac` and Product-link joins.
/// Every method defaults to refusal.
pub trait AuthenticatedFailureMarketSessionV1 {
    /// Authenticate one fresh Source/Product/session begin join.
    fn authenticate_failure_market_session_begin(
        &self,
        _expected: FailureMarketSessionBeginFactsV1,
    ) -> Result<()> {
        Err(Error::BindingMismatch)
    }

    /// Authenticate one bounded session+liveness atomic advance.
    fn authenticate_failure_market_session_advance(
        &self,
        _expected: FailureMarketSessionAdvanceFactsV1,
    ) -> Result<()> {
        Err(Error::BindingMismatch)
    }

    /// Authenticate one session resolution and exact V5 writer join.
    fn authenticate_failure_market_session_resolution(
        &self,
        _expected: FailureMarketSessionResolutionFactsV1,
    ) -> Result<()> {
        Err(Error::BindingMismatch)
    }

    /// Authenticate mutable-work close, durable replay, and link release.
    fn authenticate_failure_market_session_close(
        &self,
        _expected: FailureMarketSessionCloseFactsV1,
    ) -> Result<()> {
        Err(Error::BindingMismatch)
    }
}

/// Current Market runtime lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FailureMarketRuntimePhaseV1 {
    /// Funded and ready; no Series/source interval session is pinned.
    Ready = 1,
    /// A subordinate Series/source interval session is active.
    IntervalActive = 2,
    /// The interval resolved and awaits atomic history append/cell reset.
    IntervalResolved = 3,
    /// Reusable interval cell is Idle after terminal history append.
    IntervalArchived = 4,
    /// Sole liveness Recovery custody closed successfully.
    RecoveryClosed = 5,
    /// Durable market-level Failure terminal receipt persisted.
    FamilyTerminal = 6,
}

impl FailureMarketRuntimePhaseV1 {
    const fn byte(self) -> u8 {
        match self {
            Self::Ready => 1,
            Self::IntervalActive => 2,
            Self::IntervalResolved => 3,
            Self::IntervalArchived => 4,
            Self::RecoveryClosed => 5,
            Self::FamilyTerminal => 6,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Ready),
            2 => Ok(Self::IntervalActive),
            3 => Ok(Self::IntervalResolved),
            4 => Ok(Self::IntervalArchived),
            5 => Ok(Self::RecoveryClosed),
            6 => Ok(Self::FamilyTerminal),
            _ => Err(Error::WrongPhase),
        }
    }
}

/// Complete expected foundation facts. This projection is not authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketRuntimeAdmissionFactsV1 {
    /// Shared Market policy binding.
    pub failure_policy_binding_id: FailurePolicyBindingId,
    /// Full-width economic Market.
    pub market_instance_id: MarketInstanceV2Id,
    /// Shared Failure/liveness generation.
    pub generation: u64,
    /// Immutable admission-state content identity.
    pub admission_state_id: FailureMarketAdmissionStateIdV1,
    /// Distinct mutable runtime root account.
    pub runtime_account_id: FailureMarketAccountIdV1,
    /// Product private foundation poststate receipt.
    pub foundation_receipt_id: ProductContentId,
    /// Immutable Product-prepaid runtime-account rent ownership.
    pub root_funding: FailureMarketRuntimeRootFundingFactsV1,
    /// Present Recovery funding receipt retained by the admission root.
    pub recovery_funding_receipt_id: FailureMarketRecoveryFundingReceiptIdV1,
    /// Complete initial runtime postimage.
    pub runtime_state_commitment: FailureMarketRuntimeStateCommitmentV1,
}

/// Exact native-lamport ownership of the mutable Failure runtime account.
///
/// This is disjoint from liveness Recovery custody. The refund owner need not
/// sign or pay again: Product already debited the founder's prepaid MarketCore
/// custody before this state can be admitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketRuntimeRootFundingFactsV1 {
    /// Immutable refund recipient for the exact account-rent principal.
    pub rent_refund_owner: FailureMarketAccountIdV1,
    /// System-owned destination for prior and later unsolicited lamports.
    pub neutral_sink: FailureMarketAccountIdV1,
    /// Canonical Rent minimum for the framed runtime account at creation.
    pub rent_principal_lamports: u64,
    /// Unsolicited lamports already present before Product capitalization.
    pub donation_floor_lamports: u64,
    /// Exact post-capitalization account balance.
    pub observed_balance_lamports: u64,
}

/// Product/SBF-owned authority for the exact funded runtime foundation.
pub trait AuthenticatedFailureMarketRuntimeAdmissionV1 {
    /// Authenticate the expected graph and physical funded poststate.
    fn authenticate_failure_market_runtime_admission(
        &self,
        _expected: FailureMarketRuntimeAdmissionFactsV1,
    ) -> Result<()> {
        Err(Error::BindingMismatch)
    }
}

/// Private-field foundation receipt consumed by Product activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketRuntimeAdmissionReceiptV1 {
    id: FailureMarketRuntimeAdmissionReceiptIdV1,
    facts: FailureMarketRuntimeAdmissionFactsV1,
}

impl FailureMarketRuntimeAdmissionReceiptV1 {
    /// Complete foundation receipt identity.
    pub const fn id(self) -> FailureMarketRuntimeAdmissionReceiptIdV1 {
        self.id
    }

    /// Exact authenticated foundation facts.
    pub const fn facts(self) -> FailureMarketRuntimeAdmissionFactsV1 {
        self.facts
    }
}

/// Market-scoped dynamic Failure runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketRuntimeV1 {
    policy_binding_id: FailurePolicyBindingId,
    admission_state_id: FailureMarketAdmissionStateIdV1,
    runtime_account_id: FailureMarketAccountIdV1,
    foundation_receipt_id: ProductContentId,
    root_funding: FailureMarketRuntimeRootFundingFactsV1,
    recovery_funding_receipt_id: FailureMarketRecoveryFundingReceiptIdV1,
    phase: FailureMarketRuntimePhaseV1,
    transition_sequence: u64,
    completed_session_count: u64,
    session_ids: [ProductContentId; SESSION_ID_COUNT_V1],
}

impl FailureMarketRuntimeV1 {
    /// Immutable shared policy identity.
    pub const fn policy_binding_id(self) -> FailurePolicyBindingId {
        self.policy_binding_id
    }

    /// Immutable admission-state identity.
    pub const fn admission_state_id(self) -> FailureMarketAdmissionStateIdV1 {
        self.admission_state_id
    }

    /// Physical mutable runtime root.
    pub const fn runtime_account_id(self) -> FailureMarketAccountIdV1 {
        self.runtime_account_id
    }

    /// Product-private foundation step which capitalized this account.
    pub const fn foundation_receipt_id(self) -> ProductContentId {
        self.foundation_receipt_id
    }

    /// Current lifecycle phase.
    pub const fn phase(self) -> FailureMarketRuntimePhaseV1 {
        self.phase
    }

    /// Monotone wrapper transition sequence.
    pub const fn transition_sequence(self) -> u64 {
        self.transition_sequence
    }

    /// Exact number of terminal sessions folded into `0xac/v2`.
    pub const fn completed_session_count(self) -> u64 {
        self.completed_session_count
    }

    /// Immutable Product-prepaid runtime-account rent ownership.
    pub const fn root_funding(self) -> FailureMarketRuntimeRootFundingFactsV1 {
        self.root_funding
    }

    /// Immutable identity of the sole shared liveness-capital admission.
    pub const fn recovery_funding_receipt_id(self) -> FailureMarketRecoveryFundingReceiptIdV1 {
        self.recovery_funding_receipt_id
    }

    /// Exact currently pinned subordinate session, or zero when no session is active.
    pub const fn active_session_pin_id(self) -> ProductContentId {
        self.session_ids[ACTIVE_SESSION_PIN_INDEX_V1]
    }

    /// Product authentication of the initiating per-Series Market link.
    pub const fn series_link_authentication_id(self) -> ProductContentId {
        self.session_ids[SERIES_LINK_AUTHENTICATION_INDEX_V1]
    }

    /// Latest complete subordinate session-state commitment.
    pub const fn session_state_commitment(self) -> ProductContentId {
        self.session_ids[SESSION_STATE_COMMITMENT_INDEX_V1]
    }

    /// Accepted subordinate session resolution, if resolved.
    pub const fn session_resolution_receipt_id(self) -> ProductContentId {
        self.session_ids[SESSION_RESOLUTION_RECEIPT_INDEX_V1]
    }

    /// Durable terminal receipt for the latest archived interval.
    pub const fn interval_terminal_receipt_id(self) -> ProductContentId {
        self.session_ids[INTERVAL_TERMINAL_RECEIPT_INDEX_V1]
    }

    /// Sole shared Recovery-compartment close receipt, if closed.
    pub const fn recovery_terminal_receipt_id(self) -> ProductContentId {
        self.session_ids[RECOVERY_TERMINAL_RECEIPT_INDEX_V1]
    }

    /// Exhaustive Failure-family terminal receipt, if promoted.
    pub const fn family_terminal_receipt_id(self) -> ProductContentId {
        self.session_ids[FAMILY_TERMINAL_RECEIPT_INDEX_V1]
    }

    /// Append-only transcript over every completely archived interval session.
    pub const fn session_history_commitment(self) -> FailureMarketIntervalHistoryRootV2 {
        FailureMarketIntervalHistoryRootV2::from_bytes(
            self.session_ids[INTERVAL_HISTORY_ROOT_INDEX_V1].bytes(),
        )
    }

    /// Product-authenticated interval-account capitalization pinned by the
    /// active session, or zero while the reusable cell is Idle.
    pub const fn active_interval_funding_receipt_id(self) -> ProductContentId {
        self.session_ids[ACTIVE_INTERVAL_FUNDING_RECEIPT_INDEX_V1]
    }

    /// Canonical state commitment.
    pub fn commitment(self) -> Result<FailureMarketRuntimeStateCommitmentV1> {
        let mut bytes = [0u8; FAILURE_MARKET_RUNTIME_BYTES_V1];
        self.encode_into(&mut bytes)?;
        let mut hasher = Sha256::new();
        hasher.update(RUNTIME_COMMITMENT_DOMAIN_V1);
        hasher.update(bytes);
        Ok(FailureMarketRuntimeStateCommitmentV1::from_bytes(
            hasher.finalize().into(),
        ))
    }

    /// Commit one stale-checked runtime transition. Product link and account
    /// writes remain part of the same outer atomic batch.
    pub fn commit_plan(&mut self, plan: FailureMarketSessionTransitionPlanV1) -> Result<()> {
        self.validate()?;
        if *self != plan.before {
            return Err(Error::StalePlan);
        }
        plan.after.validate()?;
        *self = plan.after;
        Ok(())
    }

    /// Encode every semantic and reserved byte canonically.
    pub fn encode_into(self, output: &mut [u8; FAILURE_MARKET_RUNTIME_BYTES_V1]) -> Result<()> {
        self.validate()?;
        output.fill(0);
        output[..8].copy_from_slice(&MAGIC_V1);
        output[8..10].copy_from_slice(&VERSION_V1.to_le_bytes());
        let mut cursor = HEADER_BYTES_V1;
        for id in [
            self.policy_binding_id.bytes(),
            self.admission_state_id.bytes(),
            self.runtime_account_id.bytes(),
            self.foundation_receipt_id.bytes(),
            self.recovery_funding_receipt_id.bytes(),
        ] {
            put_id(output, &mut cursor, id)?;
        }
        for id in [
            self.root_funding.rent_refund_owner.bytes(),
            self.root_funding.neutral_sink.bytes(),
        ] {
            put_id(output, &mut cursor, id)?;
        }
        for amount in [
            self.root_funding.rent_principal_lamports,
            self.root_funding.donation_floor_lamports,
            self.root_funding.observed_balance_lamports,
        ] {
            put_u64(output, &mut cursor, amount)?;
        }
        output[cursor] = self.phase.byte();
        cursor = cursor
            .checked_add(PHASE_BYTES_V1)
            .ok_or(Error::WrongLength)?;
        put_u64(output, &mut cursor, self.transition_sequence)?;
        put_u64(output, &mut cursor, self.completed_session_count)?;
        for id in self.session_ids {
            put_id(output, &mut cursor, id.bytes())?;
        }
        if output
            .get(cursor..)
            .ok_or(Error::WrongLength)?
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(Error::WrongLength);
        }
        Ok(())
    }

    /// Decode only against the independently authenticated immutable
    /// admission root. Raw bytes cannot select their own policy binding.
    pub fn decode_for_admission(
        input: &[u8; FAILURE_MARKET_RUNTIME_BYTES_V1],
        admission: FailureMarketAdmissionStateV1,
    ) -> Result<Self> {
        if input[..8] != MAGIC_V1 {
            return Err(Error::BadMagic);
        }
        if input[8..10] != VERSION_V1.to_le_bytes() {
            return Err(Error::BadVersion);
        }
        if input[10..HEADER_BYTES_V1].iter().any(|byte| *byte != 0) {
            return Err(Error::NonCanonicalReserved);
        }
        let mut cursor = HEADER_BYTES_V1;
        let policy_binding_id = FailurePolicyBindingId::from_bytes(take_id(input, &mut cursor)?);
        let admission_state_id =
            FailureMarketAdmissionStateIdV1::from_bytes(take_id(input, &mut cursor)?);
        let runtime_account_id = FailureMarketAccountIdV1::from_bytes(take_id(input, &mut cursor)?);
        let foundation_receipt_id = ProductContentId::from_bytes(take_id(input, &mut cursor)?);
        let recovery_funding_receipt_id =
            FailureMarketRecoveryFundingReceiptIdV1::from_bytes(take_id(input, &mut cursor)?);
        let root_funding = FailureMarketRuntimeRootFundingFactsV1 {
            rent_refund_owner: FailureMarketAccountIdV1::from_bytes(take_id(input, &mut cursor)?),
            neutral_sink: FailureMarketAccountIdV1::from_bytes(take_id(input, &mut cursor)?),
            rent_principal_lamports: take_u64(input, &mut cursor)?,
            donation_floor_lamports: take_u64(input, &mut cursor)?,
            observed_balance_lamports: take_u64(input, &mut cursor)?,
        };
        let phase = FailureMarketRuntimePhaseV1::decode(input[cursor])?;
        if input[cursor + 1..cursor + PHASE_BYTES_V1]
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(Error::NonCanonicalReserved);
        }
        cursor = cursor
            .checked_add(PHASE_BYTES_V1)
            .ok_or(Error::WrongLength)?;
        let transition_sequence = take_u64(input, &mut cursor)?;
        let completed_session_count = take_u64(input, &mut cursor)?;
        let mut session_ids = [ProductContentId::ZERO; SESSION_ID_COUNT_V1];
        let mut index = 0usize;
        while index < SESSION_ID_COUNT_V1 {
            session_ids[index] = ProductContentId::from_bytes(take_id(input, &mut cursor)?);
            index += 1;
        }
        if input
            .get(cursor..)
            .ok_or(Error::WrongLength)?
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(Error::NonCanonicalReserved);
        }
        let value = Self {
            policy_binding_id,
            admission_state_id,
            runtime_account_id,
            foundation_receipt_id,
            root_funding,
            recovery_funding_receipt_id,
            phase,
            transition_sequence,
            completed_session_count,
            session_ids,
        };
        value.validate_against_admission(admission)?;
        Ok(value)
    }

    fn validate(self) -> Result<()> {
        require_live(self.policy_binding_id.bytes())?;
        require_live(self.admission_state_id.bytes())?;
        require_live(self.runtime_account_id.bytes())?;
        require_live(self.foundation_receipt_id.bytes())?;
        require_live(self.recovery_funding_receipt_id.bytes())?;
        require_live(self.root_funding.rent_refund_owner.bytes())?;
        require_live(self.root_funding.neutral_sink.bytes())?;
        if self.root_funding.rent_principal_lamports == 0
            || self.root_funding.observed_balance_lamports
                != self
                    .root_funding
                    .rent_principal_lamports
                    .checked_add(self.root_funding.donation_floor_lamports)
                    .ok_or(Error::BindingMismatch)?
            || self.runtime_account_id == self.root_funding.rent_refund_owner
            || self.runtime_account_id == self.root_funding.neutral_sink
            || self.root_funding.rent_refund_owner == self.root_funding.neutral_sink
        {
            return Err(Error::BindingMismatch);
        }
        let active_pin = !self.active_session_pin_id().is_zero();
        let series_link = !self.series_link_authentication_id().is_zero();
        let session_state = !self.session_state_commitment().is_zero();
        let session_resolution = !self.session_resolution_receipt_id().is_zero();
        let interval_terminal = !self.interval_terminal_receipt_id().is_zero();
        let recovery_terminal = !self.recovery_terminal_receipt_id().is_zero();
        let family_terminal = !self.family_terminal_receipt_id().is_zero();
        let session_history = self.session_history_commitment().bytes() != [0; 32];
        let interval_funding = !self.active_interval_funding_receipt_id().is_zero();
        if (self.completed_session_count == 0) != !session_history {
            return Err(Error::WrongPhase);
        }
        match self.phase {
            FailureMarketRuntimePhaseV1::Ready => {
                if self.transition_sequence != 0 || self.session_ids.iter().any(|id| !id.is_zero())
                {
                    return Err(Error::WrongPhase);
                }
            }
            FailureMarketRuntimePhaseV1::IntervalActive => {
                if self.transition_sequence == 0
                    || !(active_pin && series_link && session_state)
                    || !interval_funding
                    || session_resolution
                    || interval_terminal
                    || recovery_terminal
                    || family_terminal
                {
                    return Err(Error::WrongPhase);
                }
            }
            FailureMarketRuntimePhaseV1::IntervalResolved => {
                if self.transition_sequence == 0
                    || !(active_pin && series_link && session_state && session_resolution)
                    || !interval_funding
                    || interval_terminal
                    || recovery_terminal
                    || family_terminal
                {
                    return Err(Error::WrongPhase);
                }
            }
            FailureMarketRuntimePhaseV1::IntervalArchived => {
                if self.transition_sequence == 0
                    || active_pin
                    || !(series_link && session_state && session_resolution && interval_terminal)
                    || !session_history
                    || interval_funding
                    || recovery_terminal
                    || family_terminal
                {
                    return Err(Error::WrongPhase);
                }
            }
            FailureMarketRuntimePhaseV1::RecoveryClosed => {
                if self.transition_sequence == 0
                    || active_pin
                    || !(series_link
                        && session_state
                        && session_resolution
                        && interval_terminal
                        && session_history
                        && recovery_terminal)
                    || interval_funding
                    || family_terminal
                {
                    return Err(Error::WrongPhase);
                }
            }
            FailureMarketRuntimePhaseV1::FamilyTerminal => {
                if self.transition_sequence == 0
                    || active_pin
                    || !(series_link
                        && session_state
                        && session_resolution
                        && interval_terminal
                        && session_history
                        && recovery_terminal
                        && family_terminal)
                    || interval_funding
                {
                    return Err(Error::WrongPhase);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn validate_against_admission(
        self,
        admission: FailureMarketAdmissionStateV1,
    ) -> Result<()> {
        self.validate()?;
        let policy = admission.binding().facts();
        let recovery_funding = admission.recovery_funding().facts();
        if self.policy_binding_id != admission.binding().id()
            || self.admission_state_id != admission.id()?
            || self.runtime_account_id.bytes() != policy.recovery_state_id.bytes()
            || self.runtime_account_id == admission.root_funding().facts().root_account_id
            || self.recovery_funding_receipt_id != admission.recovery_funding().id()
            || recovery_funding.failure_policy_binding_id != self.policy_binding_id
            || recovery_funding.recovery_compartment_account_id
                != policy.recovery_compartment_account_id
            || recovery_funding.liveness_policy_id != policy.liveness_policy_id
            || recovery_funding.liveness_lifecycle_id != policy.liveness_lifecycle_id
            || recovery_funding.recovery_quote_schedule_id != policy.recovery_quote_schedule_id
            || recovery_funding.generation != policy.generation
        {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }
}

/// One stale-checked shared-runtime and Product-link session transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketSessionTransitionPlanV1 {
    before: FailureMarketRuntimeV1,
    after: FailureMarketRuntimeV1,
    series_link_before: SeriesMarketLinkV1,
    series_link_after: SeriesMarketLinkV1,
    receipt_id: FailureMarketSessionTransitionReceiptIdV1,
}

impl FailureMarketSessionTransitionPlanV1 {
    /// Resulting complete shared runtime.
    pub const fn resulting_runtime(self) -> FailureMarketRuntimeV1 {
        self.after
    }

    /// Exact Product link prestate authenticated by the adapter.
    pub const fn series_link_before(self) -> SeriesMarketLinkV1 {
        self.series_link_before
    }

    /// Exact Product link poststate for the same atomic batch.
    pub const fn series_link_after(self) -> SeriesMarketLinkV1 {
        self.series_link_after
    }

    /// Unique session transition receipt.
    pub const fn receipt_id(self) -> FailureMarketSessionTransitionReceiptIdV1 {
        self.receipt_id
    }
}

/// One stale-checked shared-runtime terminal transition.
///
/// It never mutates a Product Series link: terminalization is Market-scoped
/// and is admitted only while the reusable cell is canonically Idle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketRuntimeTerminalPlanV2 {
    before: FailureMarketRuntimeV1,
    after: FailureMarketRuntimeV1,
}

impl FailureMarketRuntimeTerminalPlanV2 {
    /// Complete resulting shared runtime.
    pub const fn resulting_runtime(self) -> FailureMarketRuntimeV1 {
        self.after
    }
}

impl FailureMarketRuntimeV1 {
    /// Commit one exact Recovery-close or family-terminal transition.
    pub fn commit_terminal_plan(&mut self, plan: FailureMarketRuntimeTerminalPlanV2) -> Result<()> {
        self.validate()?;
        if *self != plan.before {
            return Err(Error::StalePlan);
        }
        plan.after.validate()?;
        *self = plan.after;
        Ok(())
    }
}

/// Plan one subordinate Series/Source session pin. A new session may begin
/// after the prior interval is atomically archived and its cell reset, but never after shared
/// Recovery or Failure-family terminalization.
pub fn plan_begin_failure_market_session_v1<A: AuthenticatedFailureMarketSessionV1 + ?Sized>(
    authority: &A,
    runtime: FailureMarketRuntimeV1,
    admission: FailureMarketAdmissionStateV1,
    series_link: SeriesMarketLinkV1,
    begin_preauthorization_id: ProductContentId,
    session: FailureMarketSessionDescriptorV1,
    interval_funding: FailureMarketIntervalFundingReceiptV2,
    interval_history: FailureMarketIntervalHistoryV2,
) -> Result<FailureMarketSessionTransitionPlanV1> {
    runtime.validate_against_admission(admission)?;
    if runtime.phase != FailureMarketRuntimePhaseV1::Ready
        && runtime.phase != FailureMarketRuntimePhaseV1::IntervalArchived
    {
        return Err(Error::WrongPhase);
    }
    require_live(begin_preauthorization_id.bytes())?;
    validate_session_descriptor(
        runtime,
        admission,
        series_link,
        session,
        interval_funding,
        interval_history,
    )?;
    let series_link_before = series_link.semantic_id()?;
    let runtime_before = runtime.commitment()?;
    let next_sequence = runtime
        .transition_sequence
        .checked_add(1)
        .ok_or(Error::BindingMismatch)?;
    let previous_session_history = runtime.session_history_commitment();
    let previous_interval_terminal_receipt_id = runtime.interval_terminal_receipt_id();
    if runtime.phase == FailureMarketRuntimePhaseV1::Ready {
        if previous_session_history.bytes() != [0; 32]
            || !previous_interval_terminal_receipt_id.is_zero()
        {
            return Err(Error::WrongPhase);
        }
    } else if previous_session_history.bytes() == [0; 32]
        || previous_interval_terminal_receipt_id.is_zero()
    {
        return Err(Error::WrongPhase);
    }
    let series_link_after_value = series_link.pin_failure_session(begin_preauthorization_id)?;
    let series_link_after = series_link_after_value.semantic_id()?;
    let session_binding_id = series_link_after_value.failure_session_transcript_id();
    require_live(session_binding_id.bytes())?;
    if session_binding_id == begin_preauthorization_id
        || session_binding_id == session.session_state_commitment
    {
        return Err(Error::BindingMismatch);
    }
    let mut hasher = Sha256::new();
    hasher.update(SESSION_BEGIN_DOMAIN_V1);
    hash_runtime_transition_prefix(&mut hasher, runtime, runtime_before, next_sequence);
    hasher.update(series_link_before.bytes());
    hasher.update(previous_session_history.bytes());
    hasher.update(previous_interval_terminal_receipt_id.bytes());
    hasher.update(begin_preauthorization_id.bytes());
    hasher.update(session_binding_id.bytes());
    hasher.update(interval_funding.id().bytes());
    hasher.update(interval_history.id()?.bytes());
    hasher.update(interval_history.completed_session_count().to_le_bytes());
    hasher.update(interval_funding.facts().work_account.bytes());
    hasher.update(interval_funding.facts().history_account.bytes());
    hash_session_descriptor(&mut hasher, session);
    let begin_receipt_id =
        FailureMarketSessionTransitionReceiptIdV1::from_bytes(hasher.finalize().into());
    require_live(begin_receipt_id.bytes())?;
    let facts = FailureMarketSessionBeginFactsV1 {
        runtime_before,
        series_link_before,
        series_link_after,
        previous_session_history,
        previous_interval_terminal_receipt_id,
        interval_work_account: interval_funding.facts().work_account,
        interval_history_account: interval_funding.facts().history_account,
        interval_history_state_id: interval_history.id()?,
        completed_session_count: runtime.completed_session_count,
        begin_preauthorization_id,
        session_binding_id,
        session,
        begin_receipt_id,
    };
    authority.authenticate_failure_market_session_begin(facts)?;
    let mut after = runtime;
    after.phase = FailureMarketRuntimePhaseV1::IntervalActive;
    after.transition_sequence = next_sequence;
    after.session_ids[ACTIVE_SESSION_PIN_INDEX_V1] = ProductContentId::ZERO;
    after.session_ids[SERIES_LINK_AUTHENTICATION_INDEX_V1] = ProductContentId::ZERO;
    after.session_ids[SESSION_STATE_COMMITMENT_INDEX_V1] = ProductContentId::ZERO;
    after.session_ids[SESSION_RESOLUTION_RECEIPT_INDEX_V1] = ProductContentId::ZERO;
    after.session_ids[INTERVAL_TERMINAL_RECEIPT_INDEX_V1] = ProductContentId::ZERO;
    after.session_ids[ACTIVE_INTERVAL_FUNDING_RECEIPT_INDEX_V1] =
        ProductContentId::from_bytes(interval_funding.id().bytes());
    after.session_ids[ACTIVE_SESSION_PIN_INDEX_V1] = session_binding_id;
    after.session_ids[SERIES_LINK_AUTHENTICATION_INDEX_V1] =
        ProductContentId::from_bytes(series_link_after.bytes());
    after.session_ids[SESSION_STATE_COMMITMENT_INDEX_V1] = session.session_state_commitment;
    after.validate_against_admission(admission)?;
    Ok(FailureMarketSessionTransitionPlanV1 {
        before: runtime,
        after,
        series_link_before: series_link,
        series_link_after: series_link_after_value,
        receipt_id: begin_receipt_id,
    })
}

/// Plan one bounded subordinate session/liveness advance.
pub fn plan_advance_failure_market_session_v1<A: AuthenticatedFailureMarketSessionV1 + ?Sized>(
    authority: &A,
    runtime: FailureMarketRuntimeV1,
    admission: FailureMarketAdmissionStateV1,
    series_link: SeriesMarketLinkV1,
    session_after: ProductContentId,
    liveness_work_receipt_id: ProductContentId,
) -> Result<FailureMarketSessionTransitionPlanV1> {
    runtime.validate_against_admission(admission)?;
    require_active_link(runtime, admission, series_link)?;
    if runtime.phase != FailureMarketRuntimePhaseV1::IntervalActive {
        return Err(Error::WrongPhase);
    }
    require_live(session_after.bytes())?;
    require_live(liveness_work_receipt_id.bytes())?;
    let session_before = runtime.session_state_commitment();
    if session_after == session_before || session_after == liveness_work_receipt_id {
        return Err(Error::BindingMismatch);
    }
    let runtime_before = runtime.commitment()?;
    let series_link_state_id = series_link.semantic_id()?;
    let next_sequence = runtime
        .transition_sequence
        .checked_add(1)
        .ok_or(Error::BindingMismatch)?;
    let mut hasher = Sha256::new();
    hasher.update(SESSION_ADVANCE_DOMAIN_V1);
    hash_runtime_transition_prefix(&mut hasher, runtime, runtime_before, next_sequence);
    hasher.update(series_link_state_id.bytes());
    hasher.update(session_before.bytes());
    hasher.update(session_after.bytes());
    hasher.update(liveness_work_receipt_id.bytes());
    let transition_receipt_id =
        FailureMarketSessionTransitionReceiptIdV1::from_bytes(hasher.finalize().into());
    require_live(transition_receipt_id.bytes())?;
    let facts = FailureMarketSessionAdvanceFactsV1 {
        runtime_before,
        series_link_state_id,
        session_before,
        session_after,
        liveness_work_receipt_id,
        transition_receipt_id,
    };
    authority.authenticate_failure_market_session_advance(facts)?;
    let mut after = runtime;
    after.transition_sequence = next_sequence;
    after.session_ids[SESSION_STATE_COMMITMENT_INDEX_V1] = session_after;
    after.validate_against_admission(admission)?;
    Ok(FailureMarketSessionTransitionPlanV1 {
        before: runtime,
        after,
        series_link_before: series_link,
        series_link_after: series_link,
        receipt_id: transition_receipt_id,
    })
}

/// Plan exact subordinate interval resolution while retaining the Product link pin.
pub fn plan_resolve_failure_market_session_v1<A: AuthenticatedFailureMarketSessionV1 + ?Sized>(
    authority: &A,
    runtime: FailureMarketRuntimeV1,
    admission: FailureMarketAdmissionStateV1,
    series_link: SeriesMarketLinkV1,
    session_after: ProductContentId,
    session_resolution_receipt_id: ProductContentId,
) -> Result<FailureMarketSessionTransitionPlanV1> {
    runtime.validate_against_admission(admission)?;
    require_active_link(runtime, admission, series_link)?;
    if runtime.phase != FailureMarketRuntimePhaseV1::IntervalActive {
        return Err(Error::WrongPhase);
    }
    require_live(session_after.bytes())?;
    require_live(session_resolution_receipt_id.bytes())?;
    let session_before = runtime.session_state_commitment();
    if session_after == session_before || session_after == session_resolution_receipt_id {
        return Err(Error::BindingMismatch);
    }
    let runtime_before = runtime.commitment()?;
    let series_link_state_id = series_link.semantic_id()?;
    let next_sequence = runtime
        .transition_sequence
        .checked_add(1)
        .ok_or(Error::BindingMismatch)?;
    let mut hasher = Sha256::new();
    hasher.update(SESSION_RESOLVE_DOMAIN_V1);
    hash_runtime_transition_prefix(&mut hasher, runtime, runtime_before, next_sequence);
    hasher.update(series_link_state_id.bytes());
    hasher.update(session_before.bytes());
    hasher.update(session_after.bytes());
    hasher.update(session_resolution_receipt_id.bytes());
    let transition_receipt_id =
        FailureMarketSessionTransitionReceiptIdV1::from_bytes(hasher.finalize().into());
    require_live(transition_receipt_id.bytes())?;
    let facts = FailureMarketSessionResolutionFactsV1 {
        runtime_before,
        series_link_state_id,
        session_before,
        session_after,
        session_resolution_receipt_id,
        transition_receipt_id,
    };
    authority.authenticate_failure_market_session_resolution(facts)?;
    let mut after = runtime;
    after.phase = FailureMarketRuntimePhaseV1::IntervalResolved;
    after.transition_sequence = next_sequence;
    after.session_ids[SESSION_STATE_COMMITMENT_INDEX_V1] = session_after;
    after.session_ids[SESSION_RESOLUTION_RECEIPT_INDEX_V1] = session_resolution_receipt_id;
    after.validate_against_admission(admission)?;
    Ok(FailureMarketSessionTransitionPlanV1 {
        before: runtime,
        after,
        series_link_before: series_link,
        series_link_after: series_link,
        receipt_id: transition_receipt_id,
    })
}

/// Plan terminal-history append, canonical reusable-cell reset, and Product
/// link release in one atomic batch.
pub fn plan_close_failure_market_session_v1<A: AuthenticatedFailureMarketSessionV1 + ?Sized>(
    authority: &A,
    runtime: FailureMarketRuntimeV1,
    admission: FailureMarketAdmissionStateV1,
    series_link: SeriesMarketLinkV1,
    history_append: FailureMarketIntervalHistoryAppendReceiptV2,
) -> Result<FailureMarketSessionTransitionPlanV1> {
    runtime.validate_against_admission(admission)?;
    require_active_link(runtime, admission, series_link)?;
    if runtime.phase != FailureMarketRuntimePhaseV1::IntervalResolved {
        return Err(Error::WrongPhase);
    }
    let session_after = history_append.idle_state_commitment();
    require_live(session_after.bytes())?;
    let interval_terminal_receipt_id = history_append.session_terminal_receipt_id();
    require_live(interval_terminal_receipt_id.bytes())?;
    let session_before = runtime.session_state_commitment();
    if session_after == session_before
        || session_after == interval_terminal_receipt_id
        || history_append.terminal_state_commitment() != session_before
        || history_append.session_binding_id() != runtime.active_session_pin_id()
        || history_append.failure_policy_binding_id() != runtime.policy_binding_id
        || history_append.market_instance_id() != admission.binding().facts().market_instance_id
        || history_append.generation() != admission.binding().facts().generation
        || history_append.funding_receipt_id().bytes()
            != runtime.active_interval_funding_receipt_id().bytes()
        || history_append.previous_root() != runtime.session_history_commitment()
        || history_append.completed_session_count()
            != runtime
                .completed_session_count
                .checked_add(1)
                .ok_or(Error::BindingMismatch)?
    {
        return Err(Error::BindingMismatch);
    }
    let series_link_before = series_link.semantic_id()?;
    let series_link_after_value =
        series_link.release_failure_session(interval_terminal_receipt_id)?;
    let series_link_after = series_link_after_value.semantic_id()?;
    let runtime_before = runtime.commitment()?;
    let next_sequence = runtime
        .transition_sequence
        .checked_add(1)
        .ok_or(Error::BindingMismatch)?;
    let mut hasher = Sha256::new();
    hasher.update(SESSION_CLOSE_DOMAIN_V1);
    hash_runtime_transition_prefix(&mut hasher, runtime, runtime_before, next_sequence);
    hasher.update(series_link_before.bytes());
    hasher.update(series_link_after.bytes());
    hasher.update(session_before.bytes());
    hasher.update(session_after.bytes());
    hasher.update(interval_terminal_receipt_id.bytes());
    hasher.update(history_append.id().bytes());
    hasher.update(history_append.history_before().bytes());
    hasher.update(history_append.history_after().bytes());
    hasher.update(history_append.resulting_root().bytes());
    hasher.update(history_append.completed_session_count().to_le_bytes());
    let transition_receipt_id =
        FailureMarketSessionTransitionReceiptIdV1::from_bytes(hasher.finalize().into());
    require_live(transition_receipt_id.bytes())?;
    let previous_session_history = history_append.previous_root();
    let resulting_session_history = history_append.resulting_root();
    let facts = FailureMarketSessionCloseFactsV1 {
        runtime_before,
        series_link_before,
        series_link_after,
        session_before,
        session_after,
        interval_terminal_receipt_id,
        previous_session_history,
        resulting_session_history,
        history_append_receipt_id: history_append.id(),
        history_before: history_append.history_before(),
        history_after: history_append.history_after(),
        completed_session_count: history_append.completed_session_count(),
        transition_receipt_id,
    };
    authority.authenticate_failure_market_session_close(facts)?;
    let mut after = runtime;
    after.phase = FailureMarketRuntimePhaseV1::IntervalArchived;
    after.transition_sequence = next_sequence;
    after.completed_session_count = history_append.completed_session_count();
    after.session_ids[ACTIVE_SESSION_PIN_INDEX_V1] = ProductContentId::ZERO;
    after.session_ids[SERIES_LINK_AUTHENTICATION_INDEX_V1] =
        ProductContentId::from_bytes(series_link_after.bytes());
    after.session_ids[SESSION_STATE_COMMITMENT_INDEX_V1] = session_after;
    after.session_ids[INTERVAL_TERMINAL_RECEIPT_INDEX_V1] = interval_terminal_receipt_id;
    after.session_ids[ACTIVE_INTERVAL_FUNDING_RECEIPT_INDEX_V1] = ProductContentId::ZERO;
    after.session_ids[INTERVAL_HISTORY_ROOT_INDEX_V1] =
        ProductContentId::from_bytes(resulting_session_history.bytes());
    after.validate_against_admission(admission)?;
    Ok(FailureMarketSessionTransitionPlanV1 {
        before: runtime,
        after,
        series_link_before: series_link,
        series_link_after: series_link_after_value,
        receipt_id: transition_receipt_id,
    })
}

/// Close the sole shared Recovery custody after Product authenticated one
/// exact Resolution V5 activation and every subordinate session write is
/// durably folded into the Idle interval pair.
#[allow(clippy::too_many_arguments)]
pub fn plan_close_failure_market_recovery_v2<
    A: AuthenticatedFailureMarketRecoveryCloseV2 + ?Sized,
>(
    authority: &A,
    runtime: FailureMarketRuntimeV1,
    admission: FailureMarketAdmissionStateV1,
    interval_funding: FailureMarketIntervalFundingReceiptV2,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    cell: FailureMarketIntervalCellV2,
    history: FailureMarketIntervalHistoryV2,
    recovery_terminal: FailureMarketRecoveryTerminalReceiptV2,
    closed_recovery_join_id: FailureMarketClosedRecoveryJoinIdV2,
) -> Result<(
    FailureMarketRuntimeTerminalPlanV2,
    FailureMarketRecoveryCloseReceiptV2,
)> {
    runtime.validate_against_admission(admission)?;
    if runtime.phase != FailureMarketRuntimePhaseV1::IntervalArchived {
        return Err(Error::WrongPhase);
    }
    let (interval_cell_state_id, interval_history_state_id) = validate_terminal_interval_pair(
        runtime,
        admission,
        interval_funding,
        quote,
        cell,
        history,
    )?;
    let terminal_facts = recovery_terminal.facts();
    for id in [
        recovery_terminal.id().bytes(),
        closed_recovery_join_id.bytes(),
    ] {
        require_live(id)?;
    }
    if terminal_facts.runtime_before != runtime.commitment()?
        || terminal_facts.admission_state_id != admission.id()?
        || terminal_facts.failure_policy_binding_id != admission.binding().id()
        || terminal_facts.market_instance_id != admission.binding().facts().market_instance_id
        || terminal_facts.generation != admission.binding().facts().generation
        || terminal_facts.receipt_account_id != runtime.runtime_account_id
        || terminal_facts.interval_cell_state_id != interval_cell_state_id
        || terminal_facts.interval_history_state_id != interval_history_state_id
        || terminal_facts.interval_history_root != history.history_root()
        || terminal_facts.completed_session_count != history.completed_session_count()
        || terminal_facts.completed_work_calls != history.completed_work_calls()
        || terminal_facts.exact_reward_lamports != history.exact_reward_lamports()
        || terminal_facts.latest_interval_terminal_receipt_id
            != history.latest_terminal_receipt_id()
        || terminal_facts.resolution_activation_receipt_id.bytes() == recovery_terminal.id().bytes()
        || terminal_facts.resolution_activation_receipt_id.bytes()
            == closed_recovery_join_id.bytes()
        || recovery_terminal.id().bytes() == closed_recovery_join_id.bytes()
    {
        return Err(Error::BindingMismatch);
    }
    let policy = admission.binding().facts();
    let runtime_before = runtime.commitment()?;
    let facts = FailureMarketRecoveryCloseFactsV2 {
        runtime_before,
        admission_state_id: admission.id()?,
        failure_policy_binding_id: admission.binding().id(),
        market_instance_id: policy.market_instance_id,
        generation: policy.generation,
        runtime_account_id: runtime.runtime_account_id,
        interval_cell_state_id,
        interval_history_state_id,
        interval_history_root: history.history_root(),
        completed_session_count: history.completed_session_count(),
        completed_work_calls: history.completed_work_calls(),
        exact_reward_lamports: history.exact_reward_lamports(),
        latest_interval_terminal_receipt_id: history.latest_terminal_receipt_id(),
        resolution_activation_receipt_id: terminal_facts.resolution_activation_receipt_id,
        recovery_terminal_receipt_id: recovery_terminal.id(),
        closed_recovery_join_id,
    };
    authority.authenticate_failure_market_recovery_close(facts)?;
    let mut hasher = Sha256::new();
    hasher.update(RECOVERY_CLOSE_DOMAIN_V2);
    hash_recovery_close_facts(&mut hasher, facts);
    let id = FailureMarketRecoveryCloseReceiptIdV2::from_bytes(hasher.finalize().into());
    require_live(id.bytes())?;
    let mut after = runtime;
    after.phase = FailureMarketRuntimePhaseV1::RecoveryClosed;
    after.transition_sequence = after
        .transition_sequence
        .checked_add(1)
        .ok_or(Error::BindingMismatch)?;
    after.session_ids[RECOVERY_TERMINAL_RECEIPT_INDEX_V1] =
        ProductContentId::from_bytes(id.bytes());
    after.validate_against_admission(admission)?;
    Ok((
        FailureMarketRuntimeTerminalPlanV2 {
            before: runtime,
            after,
        },
        FailureMarketRecoveryCloseReceiptV2 { id, facts },
    ))
}

/// Mint the exact pre-replay family aggregate from the Recovery-closed
/// runtime and canonical Idle interval pair.
#[allow(clippy::too_many_arguments)]
pub fn admit_failure_market_family_aggregate_v2<
    A: AuthenticatedFailureMarketFamilyAggregateV2 + ?Sized,
>(
    authority: &A,
    runtime: FailureMarketRuntimeV1,
    admission: FailureMarketAdmissionStateV1,
    interval_funding: FailureMarketIntervalFundingReceiptV2,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    cell: FailureMarketIntervalCellV2,
    history: FailureMarketIntervalHistoryV2,
    recovery_close: FailureMarketRecoveryCloseReceiptV2,
) -> Result<FailureMarketFamilyAggregateReceiptV2> {
    runtime.validate_against_admission(admission)?;
    if runtime.phase != FailureMarketRuntimePhaseV1::RecoveryClosed
        || runtime.recovery_terminal_receipt_id().bytes() != recovery_close.id().bytes()
    {
        return Err(Error::WrongPhase);
    }
    let (interval_cell_state_id, interval_history_state_id) = validate_terminal_interval_pair(
        runtime,
        admission,
        interval_funding,
        quote,
        cell,
        history,
    )?;
    let recovery_facts = recovery_close.facts();
    let runtime_before = runtime.commitment()?;
    if recovery_facts.admission_state_id != admission.id()?
        || recovery_facts.failure_policy_binding_id != admission.binding().id()
        || recovery_facts.runtime_account_id != runtime.runtime_account_id
        || recovery_facts.interval_cell_state_id != interval_cell_state_id
        || recovery_facts.interval_history_state_id != interval_history_state_id
        || recovery_facts.interval_history_root != history.history_root()
        || recovery_facts.completed_session_count != history.completed_session_count()
        || recovery_facts.completed_work_calls != history.completed_work_calls()
        || recovery_facts.exact_reward_lamports != history.exact_reward_lamports()
        || recovery_facts.latest_interval_terminal_receipt_id
            != history.latest_terminal_receipt_id()
    {
        return Err(Error::BindingMismatch);
    }
    let policy = admission.binding().facts();
    let facts = FailureMarketFamilyAggregateFactsV2 {
        disposition: FailureMarketFamilyTerminalDispositionV2::Resolved,
        runtime_before,
        admission_state_id: admission.id()?,
        failure_policy_binding_id: admission.binding().id(),
        market_instance_id: policy.market_instance_id,
        generation: policy.generation,
        admission_root_account_id: admission.root_funding().facts().root_account_id,
        runtime_root_account_id: runtime.runtime_account_id,
        interval_work_account_id: history.work_account(),
        interval_history_account_id: history.history_account(),
        interval_cell_state_id,
        interval_history_state_id,
        interval_history_root: history.history_root(),
        completed_session_count: history.completed_session_count(),
        completed_work_calls: history.completed_work_calls(),
        exact_reward_lamports: history.exact_reward_lamports(),
        recovery_close_receipt_id: recovery_close.id(),
        resolution_activation_receipt_id: recovery_facts.resolution_activation_receipt_id,
    };
    authority.authenticate_failure_market_family_aggregate(facts)?;
    let mut hasher = Sha256::new();
    hasher.update(FAMILY_TERMINAL_DOMAIN_V2);
    hasher.update(b"aggregate");
    hash_family_aggregate_facts(&mut hasher, facts);
    let id = FailureMarketFamilyAggregateReceiptIdV2::from_bytes(hasher.finalize().into());
    require_live(id.bytes())?;
    Ok(FailureMarketFamilyAggregateReceiptV2 { id, facts })
}

/// Join the family aggregate to its exact permanent replay terminal postimage
/// and project the only runtime `FamilyTerminal` transition.
#[allow(clippy::too_many_arguments)]
pub fn plan_finalize_failure_market_family_v2<
    A: AuthenticatedFailureMarketFamilyTerminalV2 + ?Sized,
>(
    authority: &A,
    runtime: FailureMarketRuntimeV1,
    admission: FailureMarketAdmissionStateV1,
    interval_funding: FailureMarketIntervalFundingReceiptV2,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    cell: FailureMarketIntervalCellV2,
    history: FailureMarketIntervalHistoryV2,
    aggregate: FailureMarketFamilyAggregateReceiptV2,
    replay_terminal: FailureMarketReplayTerminalReceiptV2,
) -> Result<(
    FailureMarketRuntimeTerminalPlanV2,
    FailureMarketFamilyTerminalReceiptV2,
)> {
    runtime.validate_against_admission(admission)?;
    if runtime.phase != FailureMarketRuntimePhaseV1::RecoveryClosed {
        return Err(Error::WrongPhase);
    }
    let (_, interval_history_state_id) = validate_terminal_interval_pair(
        runtime,
        admission,
        interval_funding,
        quote,
        cell,
        history,
    )?;
    let aggregate_facts = aggregate.facts();
    let replay_facts = replay_terminal.facts();
    let runtime_before = runtime.commitment()?;
    if aggregate_facts.runtime_before != runtime_before
        || aggregate_facts.admission_state_id != admission.id()?
        || aggregate_facts.failure_policy_binding_id != admission.binding().id()
        || aggregate_facts.interval_cell_state_id != cell.id()?
        || aggregate_facts.interval_history_state_id != interval_history_state_id
        || aggregate_facts.interval_history_root != history.history_root()
        || aggregate_facts.completed_session_count != history.completed_session_count()
        || replay_facts.family_aggregate_receipt_id != aggregate.id()
        || replay_facts.runtime_terminal_state_commitment != runtime_before
    {
        return Err(Error::BindingMismatch);
    }
    let policy = admission.binding().facts();
    let facts = FailureMarketFamilyTerminalFactsV2 {
        disposition: FailureMarketFamilyTerminalDispositionV2::Resolved,
        family_aggregate_receipt_id: aggregate.id(),
        failure_replay_account_id: replay_facts.replay_account,
        failure_replay_terminal_receipt_id: replay_terminal.id(),
        runtime_before,
        admission_state_id: admission.id()?,
        failure_policy_binding_id: admission.binding().id(),
        market_instance_id: policy.market_instance_id,
        generation: policy.generation,
        interval_history_state_id,
        interval_history_root: history.history_root(),
        completed_session_count: history.completed_session_count(),
    };
    authority.authenticate_failure_market_family_terminal(facts)?;
    let mut hasher = Sha256::new();
    hasher.update(FAMILY_TERMINAL_DOMAIN_V2);
    hasher.update(b"final");
    hash_family_terminal_facts(&mut hasher, facts);
    let id = FailureMarketFamilyTerminalReceiptIdV2::from_bytes(hasher.finalize().into());
    require_live(id.bytes())?;
    let mut after = runtime;
    after.phase = FailureMarketRuntimePhaseV1::FamilyTerminal;
    after.transition_sequence = after
        .transition_sequence
        .checked_add(1)
        .ok_or(Error::BindingMismatch)?;
    after.session_ids[FAMILY_TERMINAL_RECEIPT_INDEX_V1] = ProductContentId::from_bytes(id.bytes());
    after.validate_against_admission(admission)?;
    Ok((
        FailureMarketRuntimeTerminalPlanV2 {
            before: runtime,
            after,
        },
        FailureMarketFamilyTerminalReceiptV2 { id, facts },
    ))
}

pub(crate) fn validate_terminal_interval_pair(
    runtime: FailureMarketRuntimeV1,
    admission: FailureMarketAdmissionStateV1,
    interval_funding: FailureMarketIntervalFundingReceiptV2,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    cell: FailureMarketIntervalCellV2,
    history: FailureMarketIntervalHistoryV2,
) -> Result<(
    FailureMarketIntervalCellStateIdV2,
    FailureMarketIntervalHistoryStateIdV2,
)> {
    cell.validate_against(admission, interval_funding, history, quote)?;
    history.validate_against(admission, quote)?;
    if cell.phase() != FailureMarketIntervalCellPhaseV2::Idle
        || cell.failure_policy_binding_id() != runtime.policy_binding_id
        || cell.market_instance_id() != admission.binding().facts().market_instance_id
        || cell.generation() != admission.binding().facts().generation
        || cell.funding_receipt_id() != interval_funding.id()
        || cell.history_account() != history.history_account()
        || cell.completed_session_count() != history.completed_session_count()
        || history.completed_session_count() == 0
        || history.history_root() != runtime.session_history_commitment()
        || history.completed_session_count() != runtime.completed_session_count
        || history.latest_terminal_receipt_id() != runtime.interval_terminal_receipt_id()
        || history.family_terminal_receipt_id().bytes() != [0; 32]
    {
        return Err(Error::BindingMismatch);
    }
    Ok((cell.id()?, history.id()?))
}

fn hash_recovery_close_facts(hasher: &mut Sha256, facts: FailureMarketRecoveryCloseFactsV2) {
    hasher.update(facts.runtime_before.bytes());
    hasher.update(facts.admission_state_id.bytes());
    hasher.update(facts.failure_policy_binding_id.bytes());
    hasher.update(facts.market_instance_id.bytes());
    hasher.update(facts.generation.to_le_bytes());
    hasher.update(facts.runtime_account_id.bytes());
    hasher.update(facts.interval_cell_state_id.bytes());
    hasher.update(facts.interval_history_state_id.bytes());
    hasher.update(facts.interval_history_root.bytes());
    hasher.update(facts.completed_session_count.to_le_bytes());
    hasher.update(facts.completed_work_calls.to_le_bytes());
    hasher.update(facts.exact_reward_lamports.to_le_bytes());
    hasher.update(facts.latest_interval_terminal_receipt_id.bytes());
    hasher.update(facts.resolution_activation_receipt_id.bytes());
    hasher.update(facts.recovery_terminal_receipt_id.bytes());
    hasher.update(facts.closed_recovery_join_id.bytes());
}

fn hash_family_aggregate_facts(hasher: &mut Sha256, facts: FailureMarketFamilyAggregateFactsV2) {
    hasher.update([facts.disposition.byte()]);
    hasher.update(facts.runtime_before.bytes());
    hasher.update(facts.admission_state_id.bytes());
    hasher.update(facts.failure_policy_binding_id.bytes());
    hasher.update(facts.market_instance_id.bytes());
    hasher.update(facts.generation.to_le_bytes());
    hasher.update(facts.admission_root_account_id.bytes());
    hasher.update(facts.runtime_root_account_id.bytes());
    hasher.update(facts.interval_work_account_id.bytes());
    hasher.update(facts.interval_history_account_id.bytes());
    hasher.update(facts.interval_cell_state_id.bytes());
    hasher.update(facts.interval_history_state_id.bytes());
    hasher.update(facts.interval_history_root.bytes());
    hasher.update(facts.completed_session_count.to_le_bytes());
    hasher.update(facts.completed_work_calls.to_le_bytes());
    hasher.update(facts.exact_reward_lamports.to_le_bytes());
    hasher.update(facts.recovery_close_receipt_id.bytes());
    hasher.update(facts.resolution_activation_receipt_id.bytes());
}

fn hash_family_terminal_facts(hasher: &mut Sha256, facts: FailureMarketFamilyTerminalFactsV2) {
    hasher.update([facts.disposition.byte()]);
    hasher.update(facts.family_aggregate_receipt_id.bytes());
    hasher.update(facts.failure_replay_account_id.bytes());
    hasher.update(facts.failure_replay_terminal_receipt_id.bytes());
    hasher.update(facts.runtime_before.bytes());
    hasher.update(facts.admission_state_id.bytes());
    hasher.update(facts.failure_policy_binding_id.bytes());
    hasher.update(facts.market_instance_id.bytes());
    hasher.update(facts.generation.to_le_bytes());
    hasher.update(facts.interval_history_state_id.bytes());
    hasher.update(facts.interval_history_root.bytes());
    hasher.update(facts.completed_session_count.to_le_bytes());
}

fn validate_session_descriptor(
    runtime: FailureMarketRuntimeV1,
    admission: FailureMarketAdmissionStateV1,
    series_link: SeriesMarketLinkV1,
    session: FailureMarketSessionDescriptorV1,
    interval_funding: FailureMarketIntervalFundingReceiptV2,
    interval_history: FailureMarketIntervalHistoryV2,
) -> Result<()> {
    let policy = admission.binding().facts();
    let link = series_link.binding();
    series_link.semantic_id()?;
    require_live(session.series_plan_id.bytes())?;
    require_live(session.source_occurrence_id.bytes())?;
    require_live(session.schedule_id.bytes())?;
    require_live(session.session_state_commitment.bytes())?;
    interval_history.validate_internal()?;
    let funding = interval_funding.facts();
    if series_link.active_failure_sessions() != 0
        || link.market_instance_id != policy.market_instance_id
        || link.generation != policy.generation
        || link.series_plan_id != session.series_plan_id
        || link.ordinal != session.ordinal
        || link.source_occurrence_id != session.source_occurrence_id
        || session.interval_funding_receipt_id != interval_funding.id()
        || funding.failure_policy_binding_id != runtime.policy_binding_id
        || funding.market_instance_id != policy.market_instance_id
        || funding.generation != policy.generation
        || interval_history.failure_policy_binding_id() != runtime.policy_binding_id
        || interval_history.market_instance_id() != policy.market_instance_id
        || interval_history.generation() != policy.generation
        || interval_history.funding_receipt_id() != interval_funding.id()
        || interval_history.work_account() != funding.work_account
        || interval_history.history_account() != funding.history_account
        || interval_history.history_root() != runtime.session_history_commitment()
        || interval_history.completed_session_count() != runtime.completed_session_count
        || interval_history.family_terminal_receipt_id().bytes() != [0; 32]
        || (runtime.completed_session_count != 0
            && interval_history.latest_terminal_receipt_id()
                != runtime.interval_terminal_receipt_id())
        || funding.work_account == runtime.runtime_account_id
        || funding.history_account == runtime.runtime_account_id
        || funding.work_account == admission.root_funding().facts().root_account_id
        || funding.history_account == admission.root_funding().facts().root_account_id
        || funding.work_account == runtime.root_funding.rent_refund_owner
        || funding.work_account == runtime.root_funding.neutral_sink
        || funding.history_account == runtime.root_funding.rent_refund_owner
        || funding.history_account == runtime.root_funding.neutral_sink
    {
        return Err(Error::BindingMismatch);
    }
    Ok(())
}

fn require_active_link(
    runtime: FailureMarketRuntimeV1,
    admission: FailureMarketAdmissionStateV1,
    series_link: SeriesMarketLinkV1,
) -> Result<()> {
    let policy = admission.binding().facts();
    let binding = series_link.binding();
    let semantic_id = series_link.semantic_id()?;
    if series_link.active_failure_sessions() == 0
        || binding.market_instance_id != policy.market_instance_id
        || binding.generation != policy.generation
        || semantic_id.bytes() != runtime.series_link_authentication_id().bytes()
        || runtime.active_session_pin_id().is_zero()
    {
        return Err(Error::BindingMismatch);
    }
    Ok(())
}

fn hash_runtime_transition_prefix(
    hasher: &mut Sha256,
    runtime: FailureMarketRuntimeV1,
    runtime_before: FailureMarketRuntimeStateCommitmentV1,
    next_sequence: u64,
) {
    hasher.update(runtime.policy_binding_id.bytes());
    hasher.update(runtime.runtime_account_id.bytes());
    hasher.update(runtime_before.bytes());
    hasher.update(runtime.transition_sequence.to_le_bytes());
    hasher.update(next_sequence.to_le_bytes());
}

fn hash_session_descriptor(hasher: &mut Sha256, session: FailureMarketSessionDescriptorV1) {
    hasher.update(session.series_plan_id.bytes());
    hasher.update(session.ordinal.to_le_bytes());
    hasher.update(session.source_occurrence_id.bytes());
    hasher.update(session.schedule_id.bytes());
    hasher.update(session.interval_funding_receipt_id.bytes());
    hasher.update(session.session_state_commitment.bytes());
}

/// Admit the distinct mutable Market runtime from exact Product and liveness
/// authority. Per-Series schedules are intentionally absent; Begin pins the
/// exact subordinate session which owns its own schedule and recovery state.
pub fn admit_failure_market_runtime_v1<A: AuthenticatedFailureMarketRuntimeAdmissionV1 + ?Sized>(
    authority: &A,
    admission: FailureMarketAdmissionStateV1,
    runtime_account_id: FailureMarketAccountIdV1,
    foundation_receipt_id: ProductContentId,
    root_funding: FailureMarketRuntimeRootFundingFactsV1,
) -> Result<(
    FailureMarketRuntimeV1,
    FailureMarketRuntimeAdmissionReceiptV1,
)> {
    let policy = admission.binding().facts();
    let runtime = FailureMarketRuntimeV1 {
        policy_binding_id: admission.binding().id(),
        admission_state_id: admission.id()?,
        runtime_account_id,
        foundation_receipt_id,
        root_funding,
        recovery_funding_receipt_id: admission.recovery_funding().id(),
        phase: FailureMarketRuntimePhaseV1::Ready,
        transition_sequence: 0,
        completed_session_count: 0,
        session_ids: [ProductContentId::ZERO; SESSION_ID_COUNT_V1],
    };
    runtime.validate_against_admission(admission)?;
    let facts = FailureMarketRuntimeAdmissionFactsV1 {
        failure_policy_binding_id: runtime.policy_binding_id,
        market_instance_id: policy.market_instance_id,
        generation: policy.generation,
        admission_state_id: runtime.admission_state_id,
        runtime_account_id,
        foundation_receipt_id,
        root_funding,
        recovery_funding_receipt_id: admission.recovery_funding().id(),
        runtime_state_commitment: runtime.commitment()?,
    };
    authority.authenticate_failure_market_runtime_admission(facts)?;
    let mut hasher = Sha256::new();
    hasher.update(RUNTIME_ADMISSION_DOMAIN_V1);
    hash_admission_facts(&mut hasher, facts);
    let id = FailureMarketRuntimeAdmissionReceiptIdV1::from_bytes(hasher.finalize().into());
    if id.bytes().iter().all(|byte| *byte == 0) {
        return Err(Error::BindingMismatch);
    }
    Ok((
        runtime,
        FailureMarketRuntimeAdmissionReceiptV1 { id, facts },
    ))
}

fn hash_admission_facts(hasher: &mut Sha256, facts: FailureMarketRuntimeAdmissionFactsV1) {
    hasher.update(facts.failure_policy_binding_id.bytes());
    hasher.update(facts.market_instance_id.bytes());
    hasher.update(facts.generation.to_le_bytes());
    hasher.update(facts.admission_state_id.bytes());
    hasher.update(facts.runtime_account_id.bytes());
    hasher.update(facts.foundation_receipt_id.bytes());
    hasher.update(facts.root_funding.rent_refund_owner.bytes());
    hasher.update(facts.root_funding.neutral_sink.bytes());
    hasher.update(facts.root_funding.rent_principal_lamports.to_le_bytes());
    hasher.update(facts.root_funding.donation_floor_lamports.to_le_bytes());
    hasher.update(facts.root_funding.observed_balance_lamports.to_le_bytes());
    hasher.update(facts.recovery_funding_receipt_id.bytes());
    hasher.update(facts.runtime_state_commitment.bytes());
}

fn put_id(
    output: &mut [u8; FAILURE_MARKET_RUNTIME_BYTES_V1],
    cursor: &mut usize,
    value: [u8; ID_BYTES_V1],
) -> Result<()> {
    let end = cursor.checked_add(ID_BYTES_V1).ok_or(Error::WrongLength)?;
    output
        .get_mut(*cursor..end)
        .ok_or(Error::WrongLength)?
        .copy_from_slice(&value);
    *cursor = end;
    Ok(())
}

fn take_id(
    input: &[u8; FAILURE_MARKET_RUNTIME_BYTES_V1],
    cursor: &mut usize,
) -> Result<[u8; ID_BYTES_V1]> {
    let end = cursor.checked_add(ID_BYTES_V1).ok_or(Error::WrongLength)?;
    let value = input
        .get(*cursor..end)
        .ok_or(Error::WrongLength)?
        .try_into()
        .map_err(|_| Error::WrongLength)?;
    *cursor = end;
    Ok(value)
}

fn put_u64(
    output: &mut [u8; FAILURE_MARKET_RUNTIME_BYTES_V1],
    cursor: &mut usize,
    value: u64,
) -> Result<()> {
    let end = cursor.checked_add(8).ok_or(Error::WrongLength)?;
    output
        .get_mut(*cursor..end)
        .ok_or(Error::WrongLength)?
        .copy_from_slice(&value.to_le_bytes());
    *cursor = end;
    Ok(())
}

fn take_u64(input: &[u8; FAILURE_MARKET_RUNTIME_BYTES_V1], cursor: &mut usize) -> Result<u64> {
    let end = cursor.checked_add(8).ok_or(Error::WrongLength)?;
    let bytes = input
        .get(*cursor..end)
        .ok_or(Error::WrongLength)?
        .try_into()
        .map_err(|_| Error::WrongLength)?;
    *cursor = end;
    Ok(u64::from_le_bytes(bytes))
}

fn require_live(bytes: [u8; 32]) -> Result<()> {
    if bytes.iter().all(|byte| *byte == 0) {
        Err(Error::BindingMismatch)
    } else {
        Ok(())
    }
}

const _: () = assert!(
    HEADER_BYTES_V1
        + PREFIX_ID_COUNT_V1 * ID_BYTES_V1
        + ROOT_FUNDING_ID_COUNT_V1 * ID_BYTES_V1
        + ROOT_FUNDING_AMOUNT_COUNT_V1 * 8
        + PHASE_BYTES_V1
        + 8
        + 8
        + SESSION_ID_COUNT_V1 * ID_BYTES_V1
        <= FAILURE_MARKET_RUNTIME_BYTES_V1
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market_interval_history_v2::{runtime_test_append, runtime_test_fixture};
    use crate::market_policy_v1::{
        admit_failure_market_policy_v1, admit_failure_market_recovery_funding_v1,
        admit_failure_market_root_funding_v1, AuthenticatedFailureMarketPolicyV1,
        AuthenticatedFailureMarketRecoveryFundingV1, AuthenticatedFailureMarketRootFundingV1,
        FailureMarketPolicyFactsV1, FailureMarketPrepaidDebitReceiptIdV1,
        FailureMarketRecoveryFundingFactsV1, FailureMarketRootFundingFactsV1,
    };
    use clutch_evidence_recovery::Identity as RecoveryIdentity;
    use clutch_liveness::Id as LivenessId;
    use clutch_product_series::{
        ComponentDebitV1, EvidenceOnlyRecoveryPolicyId, MarketGenesisProfileV2Id,
        NativeClaimBasisId, PriceMeasurePolicyV1Id, ProductTemplateId,
        QuantizedIntervalConsensusProfileV1Id, RecoveryAttemptFundingV1,
        RegistryCapabilityProfileV4Id, RegistryProgramReleaseV2Id, SeriesFundingQuoteV1,
        SeriesFundingQuoteV2Id, SeriesFundingTermsV2Id, SeriesLinkObligationConfigurationV1,
        SeriesLinkObligationStatusV1, SeriesMarketDispositionV1, SeriesMarketLinkBindingV1,
        MAX_RECOVERY_ATTEMPTS,
    };
    use clutch_source_plane_v3::ContentId as SourceContentId;

    #[derive(Clone, Copy, Debug)]
    struct ExactPolicy(FailureMarketPolicyFactsV1);

    impl AuthenticatedFailureMarketPolicyV1 for ExactPolicy {
        fn authenticate_failure_market_policy(
            &self,
            expected: FailureMarketPolicyFactsV1,
        ) -> Result<()> {
            if self.0 == expected {
                Ok(())
            } else {
                Err(Error::BindingMismatch)
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct ExactRecovery(FailureMarketRecoveryFundingFactsV1);

    impl AuthenticatedFailureMarketRecoveryFundingV1 for ExactRecovery {
        fn authenticate_failure_market_recovery_funding(
            &self,
            expected: FailureMarketRecoveryFundingFactsV1,
        ) -> Result<()> {
            if self.0 == expected {
                Ok(())
            } else {
                Err(Error::BindingMismatch)
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct ExactRoot(FailureMarketRootFundingFactsV1);

    impl AuthenticatedFailureMarketRootFundingV1 for ExactRoot {
        fn authenticate_failure_market_root_funding(
            &self,
            expected: FailureMarketRootFundingFactsV1,
        ) -> Result<()> {
            if self.0 == expected {
                Ok(())
            } else {
                Err(Error::BindingMismatch)
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct ExactRuntime(FailureMarketRuntimeAdmissionFactsV1);

    impl AuthenticatedFailureMarketRuntimeAdmissionV1 for ExactRuntime {
        fn authenticate_failure_market_runtime_admission(
            &self,
            expected: FailureMarketRuntimeAdmissionFactsV1,
        ) -> Result<()> {
            if self.0 == expected {
                Ok(())
            } else {
                Err(Error::BindingMismatch)
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct Refusing;

    impl AuthenticatedFailureMarketRuntimeAdmissionV1 for Refusing {}

    #[derive(Clone, Copy, Debug)]
    struct RefusingSession;

    impl AuthenticatedFailureMarketSessionV1 for RefusingSession {}

    #[derive(Clone, Copy, Debug)]
    struct AcceptingSession;

    impl AuthenticatedFailureMarketSessionV1 for AcceptingSession {
        fn authenticate_failure_market_session_begin(
            &self,
            _expected: FailureMarketSessionBeginFactsV1,
        ) -> Result<()> {
            Ok(())
        }

        fn authenticate_failure_market_session_advance(
            &self,
            _expected: FailureMarketSessionAdvanceFactsV1,
        ) -> Result<()> {
            Ok(())
        }

        fn authenticate_failure_market_session_resolution(
            &self,
            _expected: FailureMarketSessionResolutionFactsV1,
        ) -> Result<()> {
            Ok(())
        }

        fn authenticate_failure_market_session_close(
            &self,
            _expected: FailureMarketSessionCloseFactsV1,
        ) -> Result<()> {
            Ok(())
        }
    }

    fn quote(recovery_policy_id: EvidenceOnlyRecoveryPolicyId) -> SeriesFundingQuoteV1 {
        let mut attempts = [RecoveryAttemptFundingV1::ZERO; MAX_RECOVERY_ATTEMPTS];
        attempts[0] = RecoveryAttemptFundingV1 {
            max_progress_units: 10,
            lamports_per_progress_unit: 100,
        };
        SeriesFundingQuoteV1 {
            evidence_only_recovery_policy_id: recovery_policy_id,
            market_core: ComponentDebitV1 {
                lamports: 600,
                collateral_atoms: 0,
            },
            failure_root_rent_principal_lamports: 300,
            failure_replay_tombstone_rent_principal_lamports: 200,
            recovery_reserve: ComponentDebitV1 {
                lamports: 1_200,
                collateral_atoms: 0,
            },
            source_work: ComponentDebitV1::ZERO,
            liquidity_facility: ComponentDebitV1::ZERO,
            wrapper_set: ComponentDebitV1::ZERO,
            recovery_attempt_count: 1,
            recovery_attempt_funding: attempts,
            recovery_rent_principal_lamports: 200,
        }
    }

    fn admission() -> FailureMarketAdmissionStateV1 {
        let recovery_policy_id = EvidenceOnlyRecoveryPolicyId::from_bytes([4; 32]);
        let quote = quote(recovery_policy_id);
        let quote_id = quote.id().unwrap();
        let facts = FailureMarketPolicyFactsV1 {
            market_instance_id: MarketInstanceV2Id::from_bytes([1; 32]),
            product_template_id: ProductTemplateId::from_bytes([2; 32]),
            native_claim_basis_id: NativeClaimBasisId::from_bytes([3; 32]),
            recovery_policy_id,
            price_measure_policy_id: PriceMeasurePolicyV1Id::from_bytes([5; 32]),
            market_genesis_profile_id: MarketGenesisProfileV2Id::from_bytes([6; 32]),
            relation_policy_id: ProductContentId::from_bytes([7; 32]),
            registry_release_id: RegistryProgramReleaseV2Id::from_bytes([8; 32]),
            capability_profile_id: RegistryCapabilityProfileV4Id::from_bytes([9; 32]),
            interval_consensus_profile_id: QuantizedIntervalConsensusProfileV1Id::from_bytes(
                [10; 32],
            ),
            maximum_interval_width: 1_000,
            maximum_coordinates_per_advance: 32,
            source_release_manifest_id: SourceContentId::from_bytes([11; 32]),
            source_release_authentication_id: SourceContentId::from_bytes([12; 32]),
            source_release_account_id: FailureMarketAccountIdV1::from_bytes([13; 32]),
            source_plane_contract_id: SourceContentId::from_bytes([14; 32]),
            source_spec_id: SourceContentId::from_bytes([15; 32]),
            summary_program_id: SourceContentId::from_bytes([16; 32]),
            primary_window_id: SourceContentId::from_bytes([17; 32]),
            statistic_key_id: SourceContentId::from_bytes([18; 32]),
            clock_policy_id: SourceContentId::from_bytes([19; 32]),
            recovery_state_id: RecoveryIdentity::from_bytes([20; 32]),
            recovery_compartment_account_id: LivenessId::from_bytes([21; 32]),
            liveness_policy_id: LivenessId::from_bytes([22; 32]),
            liveness_lifecycle_id: LivenessId::from_bytes([23; 32]),
            recovery_quote_schedule_id: LivenessId::from_bytes(quote_id.bytes()),
            recovery_receipt_program_id: LivenessId::from_bytes([24; 32]),
            recovery_refund_owner: LivenessId::from_bytes([25; 32]),
            neutral_sink: LivenessId::from_bytes([26; 32]),
            generation: 1,
        };
        let binding = admit_failure_market_policy_v1(&ExactPolicy(facts), facts).unwrap();
        let recovery_facts = FailureMarketRecoveryFundingFactsV1 {
            failure_policy_binding_id: binding.id(),
            prepaid_debit_receipt_id: FailureMarketPrepaidDebitReceiptIdV1::from_bytes([90; 32]),
            recovery_compartment_account_id: facts.recovery_compartment_account_id,
            liveness_policy_id: facts.liveness_policy_id,
            liveness_lifecycle_id: facts.liveness_lifecycle_id,
            recovery_quote_schedule_id: facts.recovery_quote_schedule_id,
            generation: 1,
            work_principal_lamports: 1_000,
            rent_principal_lamports: 200,
            donation_lamports: 7,
            observed_balance_lamports: 1_207,
            maximum_calls: 10,
            maximum_lamports_per_call: 100,
        };
        let recovery = admit_failure_market_recovery_funding_v1(
            &ExactRecovery(recovery_facts),
            binding,
            recovery_facts,
        )
        .unwrap();
        let root_facts = FailureMarketRootFundingFactsV1 {
            failure_policy_binding_id: binding.id(),
            prepaid_debit_receipt_id: FailureMarketPrepaidDebitReceiptIdV1::from_bytes([91; 32]),
            root_account_id: FailureMarketAccountIdV1::from_bytes([27; 32]),
            rent_payer: FailureMarketAccountIdV1::from_bytes([28; 32]),
            rent_principal_lamports: 3_000,
            donation_floor_lamports: 11,
            observed_balance_lamports: 3_011,
        };
        let root =
            admit_failure_market_root_funding_v1(&ExactRoot(root_facts), binding, root_facts)
                .unwrap();
        FailureMarketAdmissionStateV1::from_receipts(binding, recovery, root).unwrap()
    }

    fn runtime_root_funding() -> FailureMarketRuntimeRootFundingFactsV1 {
        FailureMarketRuntimeRootFundingFactsV1 {
            rent_refund_owner: FailureMarketAccountIdV1::from_bytes([29; 32]),
            neutral_sink: FailureMarketAccountIdV1::from_bytes([30; 32]),
            rent_principal_lamports: 4_000,
            donation_floor_lamports: 13,
            observed_balance_lamports: 4_013,
        }
    }

    fn active_series_link() -> SeriesMarketLinkV1 {
        let configuration = SeriesLinkObligationConfigurationV1 {
            capability_profile_id: ProductContentId::from_bytes([108; 32]),
            attachment_plan_id: ProductContentId::from_bytes([107; 32]),
            initial_statuses: [
                SeriesLinkObligationStatusV1::CapabilityDisabled,
                SeriesLinkObligationStatusV1::EnabledNeverFounded,
                SeriesLinkObligationStatusV1::Live,
                SeriesLinkObligationStatusV1::CapabilityDisabled,
            ],
        };
        let binding = SeriesMarketLinkBindingV1 {
            series_plan_id: SeriesPlanV5Id::from_bytes([101; 32]),
            ordinal: 4,
            market_instance_id: MarketInstanceV2Id::from_bytes([1; 32]),
            market_root_account_id: ProductContentId::from_bytes([103; 32]),
            market_binding_id: ProductContentId::from_bytes([104; 32]),
            disposition: SeriesMarketDispositionV1::Founder,
            funding_terms_id: SeriesFundingTermsV2Id::from_bytes([105; 32]),
            funding_quote_id: SeriesFundingQuoteV2Id::from_bytes([106; 32]),
            attachment_plan_id: ProductContentId::from_bytes([107; 32]),
            capability_profile_id: ProductContentId::from_bytes([108; 32]),
            obligation_configuration_id: configuration.id().unwrap(),
            compiler_output_id: ProductContentId::from_bytes([109; 32]),
            source_occurrence_id: SourceOccurrenceV1Id::from_bytes([110; 32]),
            source_occurrence_account_id: ProductContentId::from_bytes([111; 32]),
            source_occurrence_account_authentication_id: ProductContentId::from_bytes([112; 32]),
            source_occurrence_receipt_id: ProductContentId::from_bytes([113; 32]),
            source_release_id: ProductContentId::from_bytes([114; 32]),
            source_route_id: ProductContentId::from_bytes([115; 32]),
            clock_policy_id: ProductContentId::from_bytes([116; 32]),
            source_plane_contract_id: ProductContentId::from_bytes([117; 32]),
            source_spec_id: ProductContentId::from_bytes([118; 32]),
            window_spec_id: ProductContentId::from_bytes([119; 32]),
            statistic_key_id: ProductContentId::from_bytes([120; 32]),
            funding_state_account_id: ProductContentId::from_bytes([121; 32]),
            funding_debit_receipt_id: ProductContentId::from_bytes([122; 32]),
            rent_refund_owner: ProductContentId::from_bytes([123; 32]),
            neutral_lamport_sink: ProductContentId::from_bytes([124; 32]),
            generation: 1,
            source_repair_generation: 1,
            funding_transition_sequence: 1,
        };
        SeriesMarketLinkV1::initialize_pending(binding, configuration, 1, 0)
            .unwrap()
            .activate(1, ProductContentId::from_bytes([125; 32]))
            .unwrap()
    }

    fn admitted_runtime(admission: FailureMarketAdmissionStateV1) -> FailureMarketRuntimeV1 {
        FailureMarketRuntimeV1 {
            policy_binding_id: admission.binding().id(),
            admission_state_id: admission.id().unwrap(),
            runtime_account_id: FailureMarketAccountIdV1::from_bytes(
                admission.binding().facts().recovery_state_id.bytes(),
            ),
            foundation_receipt_id: ProductContentId::from_bytes([92; 32]),
            root_funding: runtime_root_funding(),
            recovery_funding_receipt_id: admission.recovery_funding().id(),
            phase: FailureMarketRuntimePhaseV1::Ready,
            transition_sequence: 0,
            completed_session_count: 0,
            session_ids: [ProductContentId::ZERO; SESSION_ID_COUNT_V1],
        }
    }

    fn session(
        seed: u8,
        interval_funding_receipt_id: crate::market_interval_history_v2::FailureMarketIntervalFundingReceiptIdV2,
    ) -> FailureMarketSessionDescriptorV1 {
        FailureMarketSessionDescriptorV1 {
            series_plan_id: SeriesPlanV5Id::from_bytes([101; 32]),
            ordinal: 4,
            source_occurrence_id: SourceOccurrenceV1Id::from_bytes([110; 32]),
            schedule_id: FailureMarketSessionScheduleIdV1::from_bytes([seed; 32]),
            interval_funding_receipt_id,
            session_state_commitment: ProductContentId::from_bytes([seed.wrapping_add(3); 32]),
        }
    }

    #[test]
    fn market_runtime_round_trips_and_refuses_root_alias_or_fake_authority() {
        let admission = admission();
        let runtime_account = FailureMarketAccountIdV1::from_bytes(
            admission.binding().facts().recovery_state_id.bytes(),
        );
        let foundation_receipt = ProductContentId::from_bytes([92; 32]);
        let initial = admit_failure_market_runtime_v1(
            &Refusing,
            admission,
            runtime_account,
            foundation_receipt,
            runtime_root_funding(),
        );
        assert_eq!(initial, Err(Error::BindingMismatch));
        let expected_runtime = FailureMarketRuntimeV1 {
            policy_binding_id: admission.binding().id(),
            admission_state_id: admission.id().unwrap(),
            runtime_account_id: runtime_account,
            foundation_receipt_id: foundation_receipt,
            root_funding: runtime_root_funding(),
            recovery_funding_receipt_id: admission.recovery_funding().id(),
            phase: FailureMarketRuntimePhaseV1::Ready,
            transition_sequence: 0,
            completed_session_count: 0,
            session_ids: [ProductContentId::ZERO; SESSION_ID_COUNT_V1],
        };
        let expected = FailureMarketRuntimeAdmissionFactsV1 {
            failure_policy_binding_id: admission.binding().id(),
            market_instance_id: admission.binding().facts().market_instance_id,
            generation: 1,
            admission_state_id: admission.id().unwrap(),
            runtime_account_id: runtime_account,
            foundation_receipt_id: foundation_receipt,
            root_funding: runtime_root_funding(),
            recovery_funding_receipt_id: admission.recovery_funding().id(),
            runtime_state_commitment: expected_runtime.commitment().unwrap(),
        };
        let (runtime, receipt) = admit_failure_market_runtime_v1(
            &ExactRuntime(expected),
            admission,
            runtime_account,
            foundation_receipt,
            runtime_root_funding(),
        )
        .unwrap();
        assert_eq!(receipt.facts(), expected);
        let mut encoded = [0; FAILURE_MARKET_RUNTIME_BYTES_V1];
        runtime.encode_into(&mut encoded).unwrap();
        assert_eq!(
            FailureMarketRuntimeV1::decode_for_admission(&encoded, admission),
            Ok(runtime)
        );
        encoded[FAILURE_MARKET_RUNTIME_BYTES_V1 - 1] = 1;
        assert_eq!(
            FailureMarketRuntimeV1::decode_for_admission(&encoded, admission),
            Err(Error::NonCanonicalReserved)
        );

        assert_eq!(
            admit_failure_market_runtime_v1(
                &ExactRuntime(expected),
                admission,
                admission.root_funding().facts().root_account_id,
                foundation_receipt,
                runtime_root_funding(),
            ),
            Err(Error::BindingMismatch)
        );

        let (interval_funding, interval_history) = runtime_test_fixture(admission);
        let session = FailureMarketSessionDescriptorV1 {
            series_plan_id: SeriesPlanV5Id::from_bytes([101; 32]),
            ordinal: 4,
            source_occurrence_id: SourceOccurrenceV1Id::from_bytes([110; 32]),
            schedule_id: FailureMarketSessionScheduleIdV1::from_bytes([126; 32]),
            interval_funding_receipt_id: interval_funding.id(),
            session_state_commitment: ProductContentId::from_bytes([129; 32]),
        };
        assert_eq!(
            plan_begin_failure_market_session_v1(
                &RefusingSession,
                runtime,
                admission,
                active_series_link(),
                ProductContentId::from_bytes([125; 32]),
                session,
                interval_funding,
                interval_history,
            ),
            Err(Error::BindingMismatch)
        );
    }

    #[test]
    fn archived_interval_reopens_without_overwriting_history_or_accepting_stale_plans() {
        let admission = admission();
        let mut runtime = admitted_runtime(admission);
        let (interval_funding, mut interval_history) = runtime_test_fixture(admission);
        let first_begin = plan_begin_failure_market_session_v1(
            &AcceptingSession,
            runtime,
            admission,
            active_series_link(),
            ProductContentId::from_bytes([125; 32]),
            session(126, interval_funding.id()),
            interval_funding,
            interval_history,
        )
        .unwrap();
        let first_begin_receipt = first_begin.receipt_id();
        assert_eq!(
            first_begin.resulting_runtime().active_session_pin_id(),
            first_begin
                .series_link_after()
                .failure_session_transcript_id()
        );
        assert_ne!(
            first_begin.resulting_runtime().active_session_pin_id(),
            ProductContentId::from_bytes([125; 32])
        );
        runtime.commit_plan(first_begin).unwrap();
        let first_link = first_begin.series_link_after();

        let first_resolve = plan_resolve_failure_market_session_v1(
            &AcceptingSession,
            runtime,
            admission,
            first_link,
            ProductContentId::from_bytes([140; 32]),
            ProductContentId::from_bytes([141; 32]),
        )
        .unwrap();
        runtime.commit_plan(first_resolve).unwrap();
        let (_, wrong_session_append) = runtime_test_append(
            interval_history,
            ProductContentId::from_bytes([199; 32]),
            ProductContentId::from_bytes([140; 32]),
            ProductContentId::from_bytes([142; 32]),
            ProductContentId::from_bytes([143; 32]),
            144,
        );
        assert_eq!(
            plan_close_failure_market_session_v1(
                &AcceptingSession,
                runtime,
                admission,
                first_link,
                wrong_session_append,
            ),
            Err(Error::BindingMismatch)
        );
        let (first_history_after, first_append) = runtime_test_append(
            interval_history,
            runtime.active_session_pin_id(),
            ProductContentId::from_bytes([140; 32]),
            ProductContentId::from_bytes([142; 32]),
            ProductContentId::from_bytes([143; 32]),
            144,
        );
        let first_close = plan_close_failure_market_session_v1(
            &AcceptingSession,
            runtime,
            admission,
            first_link,
            first_append,
        )
        .unwrap();
        runtime.commit_plan(first_close).unwrap();
        interval_history = first_history_after;
        assert_eq!(runtime.commit_plan(first_close), Err(Error::StalePlan));
        let first_history = runtime.session_history_commitment();
        assert_ne!(first_history.bytes(), [0; 32]);

        let second_begin = plan_begin_failure_market_session_v1(
            &AcceptingSession,
            runtime,
            admission,
            active_series_link(),
            ProductContentId::from_bytes([149; 32]),
            session(150, interval_funding.id()),
            interval_funding,
            interval_history,
        )
        .unwrap();
        assert_ne!(second_begin.receipt_id(), first_begin_receipt);
        let second_active = second_begin.resulting_runtime();
        assert_eq!(
            second_active.active_session_pin_id(),
            second_begin
                .series_link_after()
                .failure_session_transcript_id()
        );
        assert_ne!(
            second_active.active_session_pin_id(),
            ProductContentId::from_bytes([149; 32])
        );
        assert_eq!(second_active.session_history_commitment(), first_history);
        assert!(second_active.session_resolution_receipt_id().is_zero());
        assert!(second_active.interval_terminal_receipt_id().is_zero());
        runtime.commit_plan(second_begin).unwrap();
        assert_eq!(runtime.commit_plan(second_begin), Err(Error::StalePlan));

        let mut overwritten = second_active;
        overwritten.phase = FailureMarketRuntimePhaseV1::IntervalArchived;
        overwritten.session_ids[ACTIVE_SESSION_PIN_INDEX_V1] = ProductContentId::ZERO;
        overwritten.session_ids[SESSION_RESOLUTION_RECEIPT_INDEX_V1] =
            ProductContentId::from_bytes([151; 32]);
        overwritten.session_ids[INTERVAL_TERMINAL_RECEIPT_INDEX_V1] =
            ProductContentId::from_bytes([152; 32]);
        overwritten.session_ids[INTERVAL_HISTORY_ROOT_INDEX_V1] = ProductContentId::ZERO;
        assert_eq!(overwritten.validate(), Err(Error::WrongPhase));
    }

    #[test]
    fn final_family_identity_binds_replay_postimage_and_exact_history() {
        let facts = FailureMarketFamilyTerminalFactsV2 {
            disposition: FailureMarketFamilyTerminalDispositionV2::Resolved,
            family_aggregate_receipt_id: FailureMarketFamilyAggregateReceiptIdV2::from_bytes(
                [201; 32],
            ),
            failure_replay_account_id: FailureMarketAccountIdV1::from_bytes([202; 32]),
            failure_replay_terminal_receipt_id:
                crate::market_replay_v2::FailureMarketReplayTerminalReceiptIdV2::from_bytes(
                    [203; 32],
                ),
            runtime_before: FailureMarketRuntimeStateCommitmentV1::from_bytes([204; 32]),
            admission_state_id: FailureMarketAdmissionStateIdV1::from_bytes([205; 32]),
            failure_policy_binding_id: FailurePolicyBindingId::from_bytes([206; 32]),
            market_instance_id: MarketInstanceV2Id::from_bytes([207; 32]),
            generation: 208,
            interval_history_state_id: FailureMarketIntervalHistoryStateIdV2::from_bytes([209; 32]),
            interval_history_root: FailureMarketIntervalHistoryRootV2::from_bytes([210; 32]),
            completed_session_count: 211,
        };
        let mut first = Sha256::new();
        first.update(FAMILY_TERMINAL_DOMAIN_V2);
        first.update(b"final");
        hash_family_terminal_facts(&mut first, facts);

        let mut stale_replay = facts;
        stale_replay.failure_replay_terminal_receipt_id =
            crate::market_replay_v2::FailureMarketReplayTerminalReceiptIdV2::from_bytes([212; 32]);
        let mut second = Sha256::new();
        second.update(FAMILY_TERMINAL_DOMAIN_V2);
        second.update(b"final");
        hash_family_terminal_facts(&mut second, stale_replay);
        assert_ne!(first.finalize(), second.finalize());

        let mut overwritten_history = facts;
        overwritten_history.completed_session_count += 1;
        let mut third = Sha256::new();
        third.update(FAMILY_TERMINAL_DOMAIN_V2);
        third.update(b"final");
        hash_family_terminal_facts(&mut third, overwritten_history);
        let mut original = Sha256::new();
        original.update(FAMILY_TERMINAL_DOMAIN_V2);
        original.update(b"final");
        hash_family_terminal_facts(&mut original, facts);
        assert_ne!(original.finalize(), third.finalize());
    }
}
